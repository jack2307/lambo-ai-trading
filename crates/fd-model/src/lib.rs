//! Logistic regression with L2 regularisation, written out rather than pulled
//! in.
//!
//! Deliberately a linear model. With a few thousand samples and twenty-four
//! correlated features, a linear boundary with strong regularisation is the
//! honest choice — and its coefficients are readable, which matters here: a
//! human has to be able to check that "bull flow at support" carries a positive
//! weight rather than trust a number that came out of a black box.
//!
//! Written out rather than taken from a crate for the same reason the engine
//! formulas are: the whole decision path has to stay auditable, and a model
//! file that is plain JSON can be diffed, reviewed and rejected.
//!
//! Training is full-batch gradient descent and is **deterministic** — the same
//! data produces the same model, every time, on every machine. That is not a
//! nicety. A model that cannot be reproduced cannot be compared against the one
//! it replaced, and a walk-forward over a stochastic trainer measures the seed
//! as much as the market.

pub mod metrics;
pub mod walk_forward;

use serde::{Deserialize, Serialize};

pub use metrics::{Calibration, auc, baseline_accuracy, calibration, log_loss};
pub use walk_forward::{Fold, WalkForward, WalkForwardError, accuracy_at, walk_forward};

/// Per-feature mean and standard deviation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Standardizer {
    pub mean: Vec<f64>,
    pub std: Vec<f64>,
}

impl Standardizer {
    /// Fit over rows.
    ///
    /// A constant feature gets a standard deviation of 1 rather than 0: the
    /// alternative is a division by zero that turns the whole row into NaN, and
    /// a feature that never varies should contribute nothing, not poison
    /// everything.
    #[must_use]
    pub fn fit(x: &[&[f64]]) -> Self {
        let n = x.len();
        let d = x.first().map_or(0, |row| row.len());
        let mut mean = vec![0.0; d];
        let mut std = vec![0.0; d];

        for row in x {
            for j in 0..d {
                mean[j] += row[j];
            }
        }
        for value in &mut mean {
            *value /= n.max(1) as f64;
        }
        for row in x {
            for j in 0..d {
                std[j] += (row[j] - mean[j]).powi(2);
            }
        }
        for value in &mut std {
            *value = (*value / (n.saturating_sub(1)).max(1) as f64).sqrt();
            // Negated on purpose, and clippy is wrong to want `<= 1e-9`: a NaN
            // must also land on 1.0, and every comparison with NaN is false.
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            if !(*value > 1e-9) {
                *value = 1.0;
            }
        }
        Self { mean, std }
    }

    #[must_use]
    pub fn apply(&self, row: &[f64]) -> Vec<f64> {
        row.iter().enumerate().map(|(j, v)| (v - self.mean[j]) / self.std[j]).collect()
    }
}

/// Numerically stable sigmoid.
///
/// The branch is not decoration: `1/(1+exp(-z))` overflows for large negative
/// `z`, and a trainer that produces `inf` gradients on one row throws away
/// every epoch after it.
#[must_use]
pub fn sigmoid(z: f64) -> f64 {
    if z >= 0.0 { 1.0 / (1.0 + (-z).exp()) } else { z.exp() / (1.0 + z.exp()) }
}

/// How a model is trained.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrainOptions {
    pub epochs: usize,
    pub lr: f64,
    pub l2: f64,
    /// Reweight classes so a lopsided sample cannot be fitted by always
    /// predicting the majority.
    pub class_weight: bool,
}

impl Default for TrainOptions {
    fn default() -> Self {
        Self { epochs: 600, lr: 0.1, l2: 1.0, class_weight: true }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMeta {
    pub samples: usize,
    pub positives: usize,
    pub epochs: usize,
    pub lr: f64,
    pub l2: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogisticModel {
    pub weights: Vec<f64>,
    pub bias: f64,
    pub standardizer: Option<Standardizer>,
    pub feature_names: Vec<String>,
    pub meta: ModelMeta,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("cannot fit an empty dataset")]
    Empty,
    #[error("model is not fitted")]
    Unfitted,
    #[error("expected {expected} features, got {actual}")]
    Shape { expected: usize, actual: usize },
}

/// One feature's share of a single prediction.
#[derive(Debug, Clone, PartialEq)]
pub struct Contribution {
    pub feature: String,
    /// In standardized units — comparable across features, which raw values
    /// are not.
    pub contribution: f64,
    pub value: f64,
}

impl LogisticModel {
    #[must_use]
    pub fn new(feature_names: Vec<String>) -> Self {
        Self { feature_names, ..Self::default() }
    }

    /// Full-batch gradient descent.
    pub fn fit(&mut self, x: &[&[f64]], y: &[bool], options: TrainOptions) -> Result<(), ModelError> {
        let n = x.len();
        let d = x.first().map_or(0, |row| row.len());
        if n == 0 || d == 0 {
            return Err(ModelError::Empty);
        }

        let standardizer = Standardizer::fit(x);
        let z: Vec<Vec<f64>> = x.iter().map(|row| standardizer.apply(row)).collect();

        let positives = y.iter().filter(|label| **label).count();
        let negatives = n - positives;
        let (w_pos, w_neg) = if !options.class_weight || positives == 0 || negatives == 0 {
            (1.0, 1.0)
        } else {
            (n as f64 / (2.0 * positives as f64), n as f64 / (2.0 * negatives as f64))
        };

        let mut weights = vec![0.0; d];
        let mut bias = 0.0;
        for _ in 0..options.epochs {
            let mut grad = vec![0.0; d];
            let mut grad_b = 0.0;
            let mut weight_sum = 0.0;

            for (row, label) in z.iter().zip(y) {
                let p = sigmoid(dot(&weights, row) + bias);
                let w = if *label { w_pos } else { w_neg };
                let error = (p - f64::from(u8::from(*label))) * w;
                for (j, value) in row.iter().enumerate() {
                    grad[j] += error * value;
                }
                grad_b += error;
                weight_sum += w;
            }

            let scale = 1.0 / weight_sum.max(1.0);
            for j in 0..d {
                // The penalty is divided by `n`, not by the weighted sum: that
                // is what the oracle does, and changing it would retune every
                // model rather than port it.
                weights[j] -= options.lr * (grad[j] * scale + options.l2 * weights[j] / n as f64);
            }
            bias -= options.lr * grad_b * scale;
        }

        self.weights = weights;
        self.bias = bias;
        self.standardizer = Some(standardizer);
        self.meta = ModelMeta {
            samples: n,
            positives,
            epochs: options.epochs,
            lr: options.lr,
            l2: options.l2,
        };
        Ok(())
    }

    /// Probability for one raw (unstandardized) row.
    pub fn predict_proba(&self, row: &[f64]) -> Result<f64, ModelError> {
        let standardizer = self.standardizer.as_ref().ok_or(ModelError::Unfitted)?;
        if row.len() != self.weights.len() {
            return Err(ModelError::Shape { expected: self.weights.len(), actual: row.len() });
        }
        Ok(sigmoid(dot(&self.weights, &standardizer.apply(row)) + self.bias))
    }

    /// Coefficients by absolute influence, for explanation and audit.
    #[must_use]
    pub fn top_factors(&self, limit: usize) -> Vec<(String, f64)> {
        let mut factors: Vec<(String, f64)> = self
            .weights
            .iter()
            .enumerate()
            .map(|(j, w)| (self.name_of(j), *w))
            .collect();
        factors.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap_or(std::cmp::Ordering::Equal));
        factors.truncate(limit);
        factors
    }

    /// Why this row got this probability.
    ///
    /// The only thing an explanation layer is allowed to quote. It is the
    /// model's actual arithmetic rather than a story told about it.
    pub fn explain(&self, row: &[f64], limit: usize) -> Result<Vec<Contribution>, ModelError> {
        let standardizer = self.standardizer.as_ref().ok_or(ModelError::Unfitted)?;
        let z = standardizer.apply(row);
        let mut out: Vec<Contribution> = self
            .weights
            .iter()
            .enumerate()
            .map(|(j, w)| Contribution {
                feature: self.name_of(j),
                contribution: w * z[j],
                value: row[j],
            })
            .collect();
        out.sort_by(|a, b| {
            b.contribution.abs().partial_cmp(&a.contribution.abs()).unwrap_or(std::cmp::Ordering::Equal)
        });
        out.truncate(limit);
        Ok(out)
    }

    fn name_of(&self, index: usize) -> String {
        self.feature_names.get(index).cloned().unwrap_or_else(|| format!("f{index}"))
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn separable() -> (Vec<Vec<f64>>, Vec<bool>) {
        // Two clouds a line can separate, with a second feature that carries
        // nothing — a model that weights it has learned noise.
        let mut x = Vec::new();
        let mut y = Vec::new();
        for i in 0..100 {
            let t = i as f64 / 100.0;
            x.push(vec![-1.0 + t * 0.5, (i % 7) as f64]);
            y.push(false);
            x.push(vec![1.0 + t * 0.5, (i % 5) as f64]);
            y.push(true);
        }
        (x, y)
    }

    fn rows(x: &[Vec<f64>]) -> Vec<&[f64]> {
        x.iter().map(Vec::as_slice).collect()
    }

    #[test]
    fn the_sigmoid_does_not_overflow_at_either_end() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-12);
        assert!(sigmoid(-800.0) >= 0.0 && sigmoid(-800.0) < 1e-300);
        assert!((sigmoid(800.0) - 1.0).abs() < 1e-12);
        assert!(sigmoid(-800.0).is_finite() && sigmoid(800.0).is_finite());
    }

    #[test]
    fn a_constant_feature_does_not_produce_nan() {
        let x = vec![vec![1.0, 5.0], vec![2.0, 5.0], vec![3.0, 5.0]];
        let standardizer = Standardizer::fit(&rows(&x));
        assert_eq!(standardizer.std[1], 1.0, "a constant column must not divide by zero");
        assert!(standardizer.apply(&x[0]).iter().all(|v| v.is_finite()));
    }

    #[test]
    fn training_is_deterministic() {
        let (x, y) = separable();
        let mut first = LogisticModel::new(vec!["signal".into(), "noise".into()]);
        let mut second = first.clone();
        first.fit(&rows(&x), &y, TrainOptions::default()).expect("fit");
        second.fit(&rows(&x), &y, TrainOptions::default()).expect("fit");
        assert_eq!(first.weights, second.weights, "the same data must give the same model");
        assert_eq!(first.bias, second.bias);
    }

    #[test]
    fn a_separable_problem_is_learned_and_the_noise_feature_is_not() {
        let (x, y) = separable();
        let mut model = LogisticModel::new(vec!["signal".into(), "noise".into()]);
        model.fit(&rows(&x), &y, TrainOptions::default()).expect("fit");

        assert!(model.predict_proba(&[2.0, 3.0]).expect("predict") > 0.5);
        assert!(model.predict_proba(&[-2.0, 3.0]).expect("predict") < 0.5);
        assert!(
            model.weights[0].abs() > model.weights[1].abs() * 3.0,
            "the informative feature must dominate: {:?}",
            model.weights
        );
    }

    #[test]
    fn an_unfitted_model_refuses_to_predict() {
        let model = LogisticModel::new(vec!["a".into()]);
        assert!(matches!(model.predict_proba(&[1.0]), Err(ModelError::Unfitted)));
    }

    #[test]
    fn a_row_of_the_wrong_width_is_refused_rather_than_truncated() {
        let (x, y) = separable();
        let mut model = LogisticModel::new(vec!["signal".into(), "noise".into()]);
        model.fit(&rows(&x), &y, TrainOptions::default()).expect("fit");
        assert!(matches!(model.predict_proba(&[1.0]), Err(ModelError::Shape { .. })));
    }

    #[test]
    fn an_empty_dataset_is_refused() {
        let mut model = LogisticModel::new(vec![]);
        assert!(matches!(model.fit(&[], &[], TrainOptions::default()), Err(ModelError::Empty)));
    }

    #[test]
    fn class_weighting_stops_a_lopsided_sample_collapsing() {
        // Ninety-five negatives to five positives: without reweighting, always
        // saying "down" scores 95% and the model learns exactly that.
        let mut x = Vec::new();
        let mut y = Vec::new();
        for i in 0..95 {
            x.push(vec![-1.0 + (i % 10) as f64 * 0.01]);
            y.push(false);
        }
        for i in 0..5 {
            x.push(vec![1.0 + (i % 3) as f64 * 0.01]);
            y.push(true);
        }
        let mut model = LogisticModel::new(vec!["signal".into()]);
        model.fit(&rows(&x), &y, TrainOptions::default()).expect("fit");
        assert!(
            model.predict_proba(&[1.0]).expect("predict") > 0.5,
            "the minority class must still be reachable"
        );
    }

    #[test]
    fn an_explanation_is_the_models_own_arithmetic() {
        let (x, y) = separable();
        let mut model = LogisticModel::new(vec!["signal".into(), "noise".into()]);
        model.fit(&rows(&x), &y, TrainOptions::default()).expect("fit");

        let explained = model.explain(&[2.0, 3.0], 2).expect("explain");
        assert_eq!(explained[0].feature, "signal", "the dominant factor must lead");
        // The contributions plus the bias are the logit the prediction came
        // from; an explanation that does not add up is a story.
        let total: f64 = explained.iter().map(|c| c.contribution).sum::<f64>() + model.bias;
        let expected = model.predict_proba(&[2.0, 3.0]).expect("predict");
        assert!((sigmoid(total) - expected).abs() < 1e-12);
    }
}
