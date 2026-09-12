//! Chronological walk-forward for a model.
//!
//! Exists because an in-sample AUC is not a result and looks exactly like one.
//! A logistic model with twenty-four features fitted to a few hundred rows will
//! score beautifully on those same rows; the only question worth asking is how
//! it does on rows it has never seen, in the order they actually arrived.
//!
//! Two rules, both of which a random split would break:
//!
//! * **Train on the past, test on the future.** Shuffling rows lets the model
//!   learn from Tuesday to predict Monday, which no live system can do.
//! * **Every fold trains from the beginning.** The training window grows rather
//!   than sliding, matching how a deployed model would be refitted as history
//!   accumulates.

use crate::{LogisticModel, TrainOptions, auc, baseline_accuracy, log_loss};

#[derive(Debug, Clone, PartialEq)]
pub struct Fold {
    pub fold: usize,
    pub train_samples: usize,
    pub test_samples: usize,
    pub auc: f64,
    pub log_loss: f64,
    /// Majority class of the *test* window. The number the fold has to beat.
    pub baseline: f64,
    pub accuracy: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WalkForward {
    pub folds: Vec<Fold>,
    /// Every out-of-sample prediction, pooled.
    pub oos_auc: f64,
    pub oos_log_loss: f64,
    pub oos_baseline: f64,
    pub oos_accuracy: f64,
    pub oos_samples: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum WalkForwardError {
    #[error("only {have} labelled samples; need {need}")]
    TooFew { have: usize, need: usize },
}

/// Run `folds` expanding-window folds over rows already in time order.
///
/// A fold with fewer than ten test rows is skipped rather than reported: an AUC
/// over three samples is noise with a decimal point.
pub fn walk_forward(
    x: &[&[f64]],
    y: &[bool],
    feature_names: &[String],
    options: TrainOptions,
    folds: usize,
    min_train: usize,
) -> Result<WalkForward, WalkForwardError> {
    if x.len() < min_train {
        return Err(WalkForwardError::TooFew { have: x.len(), need: min_train });
    }
    let fold_size = x.len() / (folds + 1);
    let mut results = Vec::new();
    let mut oos_probs = Vec::new();
    let mut oos_labels = Vec::new();

    for f in 1..=folds {
        let train_end = fold_size * f;
        let test_end = (fold_size * (f + 1)).min(x.len());
        if test_end.saturating_sub(train_end) < 10 {
            continue;
        }
        let mut model = LogisticModel::new(feature_names.to_vec());
        if model.fit(&x[..train_end], &y[..train_end], options).is_err() {
            continue;
        }

        let mut probs = Vec::new();
        let labels = &y[train_end..test_end];
        for row in &x[train_end..test_end] {
            let Ok(p) = model.predict_proba(row) else { continue };
            probs.push(p);
        }
        if probs.len() != labels.len() {
            continue;
        }
        results.push(Fold {
            fold: f,
            train_samples: train_end,
            test_samples: probs.len(),
            auc: auc(labels, &probs),
            log_loss: log_loss(labels, &probs),
            baseline: baseline_accuracy(labels),
            accuracy: accuracy_at(labels, &probs, 0.5),
        });
        oos_probs.extend_from_slice(&probs);
        oos_labels.extend_from_slice(labels);
    }

    Ok(WalkForward {
        oos_auc: auc(&oos_labels, &oos_probs),
        oos_log_loss: log_loss(&oos_labels, &oos_probs),
        oos_baseline: baseline_accuracy(&oos_labels),
        oos_accuracy: accuracy_at(&oos_labels, &oos_probs, 0.5),
        oos_samples: oos_labels.len(),
        folds: results,
    })
}

/// Share of predictions on the right side of a threshold.
#[must_use]
pub fn accuracy_at(y_true: &[bool], y_prob: &[f64], threshold: f64) -> f64 {
    if y_true.is_empty() {
        return f64::NAN;
    }
    let right = y_true.iter().zip(y_prob).filter(|(truth, p)| **truth == (**p >= threshold)).count();
    right as f64 / y_true.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rows whose label is a clean function of the first feature.
    fn learnable(n: usize) -> (Vec<Vec<f64>>, Vec<bool>) {
        let mut x = Vec::new();
        let mut y = Vec::new();
        for i in 0..n {
            let up = i % 2 == 0;
            x.push(vec![if up { 1.0 } else { -1.0 }, (i % 11) as f64]);
            y.push(up);
        }
        (x, y)
    }

    fn names() -> Vec<String> {
        vec!["signal".into(), "noise".into()]
    }

    fn rows(x: &[Vec<f64>]) -> Vec<&[f64]> {
        x.iter().map(Vec::as_slice).collect()
    }

    #[test]
    fn a_learnable_pattern_survives_out_of_sample() {
        let (x, y) = learnable(400);
        let result =
            walk_forward(&rows(&x), &y, &names(), TrainOptions::default(), 4, 100).expect("walk-forward");
        assert_eq!(result.folds.len(), 4);
        assert!(result.oos_auc > 0.95, "a clean pattern must generalise: {}", result.oos_auc);
    }

    #[test]
    fn noise_does_not_survive_out_of_sample() {
        // Labels independent of the features. In sample a 24-weight model would
        // fit some of this; out of sample it must land near a coin flip.
        let mut x = Vec::new();
        let mut y = Vec::new();
        for i in 0..400u64 {
            let hashed = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 33;
            x.push(vec![(hashed % 97) as f64, (hashed % 31) as f64]);
            y.push(i % 3 == 0);
        }
        let result =
            walk_forward(&rows(&x), &y, &names(), TrainOptions::default(), 4, 100).expect("walk-forward");
        assert!(
            (result.oos_auc - 0.5).abs() < 0.2,
            "noise must not generalise, got {}",
            result.oos_auc
        );
    }

    #[test]
    fn too_few_samples_is_refused_rather_than_guessed_at() {
        let (x, y) = learnable(20);
        assert!(matches!(
            walk_forward(&rows(&x), &y, &names(), TrainOptions::default(), 4, 120),
            Err(WalkForwardError::TooFew { .. })
        ));
    }

    #[test]
    fn every_fold_tests_on_rows_after_the_ones_it_trained_on() {
        let (x, y) = learnable(400);
        let result =
            walk_forward(&rows(&x), &y, &names(), TrainOptions::default(), 4, 100).expect("walk-forward");
        let mut previous_train = 0;
        for fold in &result.folds {
            assert!(fold.train_samples > previous_train, "the training window must grow");
            previous_train = fold.train_samples;
            assert!(fold.test_samples > 0);
        }
    }

    #[test]
    fn accuracy_counts_both_sides_of_the_threshold() {
        assert!((accuracy_at(&[true, false], &[0.9, 0.1], 0.5) - 1.0).abs() < 1e-12);
        assert!(accuracy_at(&[true, false], &[0.1, 0.9], 0.5).abs() < 1e-12);
        assert!(accuracy_at(&[], &[], 0.5).is_nan());
    }
}
