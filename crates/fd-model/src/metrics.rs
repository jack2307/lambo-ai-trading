//! How good is a model, and is it honest about it.
//!
//! Three different questions that are easy to confuse:
//!
//! * [`auc`] asks whether the model **ranks** — does a higher score mean a
//!   higher chance of being right? A model can rank perfectly and still be
//!   uncalibrated.
//! * [`calibration`] asks whether a predicted 0.6 actually wins about 60% of
//!   the time. This is the one that decides position sizing, and a model that
//!   ranks well but is badly calibrated sizes every trade wrong.
//! * [`baseline_accuracy`] is the majority class. Anything that cannot beat it
//!   is not a model.

use serde::Serialize;

/// Mean negative log-likelihood.
///
/// Probabilities are clamped away from 0 and 1 before the logarithm: a single
/// confident mistake would otherwise produce an infinite loss and hide every
/// other number in the run.
#[must_use]
pub fn log_loss(y_true: &[bool], y_prob: &[f64]) -> f64 {
    let mut sum = 0.0;
    for (truth, p) in y_true.iter().zip(y_prob) {
        let p = p.clamp(1e-9, 1.0 - 1e-9);
        sum += if *truth { -p.ln() } else { -(1.0 - p).ln() };
    }
    sum / y_true.len().max(1) as f64
}

/// Rank-based AUC (Mann-Whitney U).
///
/// Ties share an averaged rank. Without that, a model that outputs the same
/// probability for every row would score 0 or 1 depending on sort order rather
/// than the 0.5 it deserves.
///
/// Returns NaN when one class is absent: AUC is undefined there, and returning
/// 0.5 would be a claim rather than an admission.
#[must_use]
pub fn auc(y_true: &[bool], y_prob: &[f64]) -> f64 {
    let mut pairs: Vec<(f64, bool)> =
        y_prob.iter().zip(y_true).map(|(p, y)| (*p, *y)).collect();
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let (mut rank, mut i) = (1.0_f64, 0usize);
    let (mut sum_ranks_pos, mut pos, mut neg) = (0.0, 0usize, 0usize);

    while i < pairs.len() {
        let mut j = i;
        while j + 1 < pairs.len() && pairs[j + 1].0 == pairs[i].0 {
            j += 1;
        }
        let average = (rank + (rank + (j - i) as f64)) / 2.0;
        for pair in &pairs[i..=j] {
            if pair.1 {
                sum_ranks_pos += average;
                pos += 1;
            } else {
                neg += 1;
            }
        }
        rank += (j - i + 1) as f64;
        i = j + 1;
    }

    if pos == 0 || neg == 0 {
        return f64::NAN;
    }
    (sum_ranks_pos - (pos * (pos + 1)) as f64 / 2.0) / (pos as f64 * neg as f64)
}

/// One reliability bin.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Calibration {
    pub bin: String,
    pub n: usize,
    /// Mean predicted probability in the bin.
    pub predicted: f64,
    /// Share that actually happened.
    pub actual: f64,
}

/// Does a predicted 0.6 win about 60% of the time?
#[must_use]
pub fn calibration(y_true: &[bool], y_prob: &[f64], bins: usize) -> Vec<Calibration> {
    let mut out = Vec::with_capacity(bins);
    for b in 0..bins {
        let lo = b as f64 / bins as f64;
        let hi = (b + 1) as f64 / bins as f64;
        let (mut n, mut wins, mut sum_p) = (0usize, 0usize, 0.0);
        for (p, truth) in y_prob.iter().zip(y_true) {
            // The last bin closes on the right so a prediction of exactly 1.0
            // lands somewhere instead of vanishing.
            let inside = *p >= lo && (*p < hi || (b == bins - 1 && *p <= hi));
            if inside {
                n += 1;
                sum_p += p;
                if *truth {
                    wins += 1;
                }
            }
        }
        out.push(Calibration {
            bin: format!("{lo:.1}-{hi:.1}"),
            n,
            predicted: if n > 0 { sum_p / n as f64 } else { f64::NAN },
            actual: if n > 0 { wins as f64 / n as f64 } else { f64::NAN },
        });
    }
    out
}

/// Majority-class accuracy. Any model that cannot beat this is not a model.
#[must_use]
pub fn baseline_accuracy(y: &[bool]) -> f64 {
    let pos = y.iter().filter(|v| **v).count();
    pos.max(y.len() - pos) as f64 / y.len().max(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_perfect_ranking_scores_one_and_a_reversed_one_scores_zero() {
        let truth = [false, false, true, true];
        assert!((auc(&truth, &[0.1, 0.2, 0.8, 0.9]) - 1.0).abs() < 1e-12);
        assert!(auc(&truth, &[0.9, 0.8, 0.2, 0.1]).abs() < 1e-12);
    }

    #[test]
    fn a_model_with_no_opinion_scores_a_half() {
        // Every probability identical: the averaged tie rank is what makes this
        // 0.5 instead of an artefact of sort order.
        let truth = [true, false, true, false];
        assert!((auc(&truth, &[0.5; 4]) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn auc_is_undefined_with_one_class_and_says_so() {
        assert!(auc(&[true, true], &[0.2, 0.8]).is_nan());
        assert!(auc(&[], &[]).is_nan());
    }

    #[test]
    fn log_loss_survives_a_confidently_wrong_prediction() {
        let loss = log_loss(&[true], &[0.0]);
        assert!(loss.is_finite(), "a clamp is what keeps one mistake from hiding the run");
        assert!(loss > 20.0);
    }

    #[test]
    fn a_calibrated_model_matches_its_own_claims() {
        // Eight of ten rows at p=0.8 actually happen.
        let truth: Vec<bool> = (0..10).map(|i| i < 8).collect();
        let probs = vec![0.8; 10];
        let bins = calibration(&truth, &probs, 5);
        let bin = bins.iter().find(|b| b.n > 0).expect("one populated bin");
        assert!((bin.predicted - 0.8).abs() < 1e-12);
        assert!((bin.actual - 0.8).abs() < 1e-12);
    }

    #[test]
    fn an_empty_bin_reports_nothing_rather_than_zero() {
        let bins = calibration(&[true], &[0.9], 5);
        assert!(bins[0].actual.is_nan(), "an empty bin has no rate, and zero is not one");
        assert_eq!(bins[4].n, 1);
    }

    #[test]
    fn the_baseline_is_the_majority_class() {
        assert!((baseline_accuracy(&[true, true, true, false]) - 0.75).abs() < 1e-12);
        assert!((baseline_accuracy(&[false, false, true, true]) - 0.5).abs() < 1e-12);
    }
}
