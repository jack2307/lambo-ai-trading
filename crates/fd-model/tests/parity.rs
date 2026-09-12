//! Parity gate for the logistic baseline.
//!
//! A linear model has nowhere to hide. If the standardiser, the class weights,
//! the L2 penalty or the gradient step differ at all, the coefficients diverge
//! and this fails — which is why the gate compares **weights**, not scores. Two
//! implementations can reach a similar AUC by different arithmetic; they cannot
//! reach the same twenty-four coefficients by accident.
//!
//! Trained on the golden feature table rather than on a freshly built one, so a
//! failure here is about the trainer and not about the features. Those have
//! their own gate in `fd-features`.

use std::path::{Path, PathBuf};

use fd_core::parity_eq;
use fd_model::{LogisticModel, TrainOptions, auc, baseline_accuracy, log_loss};
use serde_json::Value;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("tests").join("golden")
}

fn read_golden(market: &str, name: &str) -> Option<Value> {
    let path = golden_dir().join(format!("{market}-{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn num(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => match s.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => panic!("unexpected string where a number was expected: {other}"),
        },
        Value::Null => f64::NAN,
        other => panic!("unexpected value: {other}"),
    }
}

/// Rebuild the labelled training set exactly as the oracle did.
fn training_set(market: &str, horizon: &str, band: f64) -> Option<(Vec<Vec<f64>>, Vec<bool>)> {
    let features = read_golden(market, "features")?;
    let columns = features["x"].as_array()?;
    let forward = features["fwd"][horizon].as_array()?;
    let count = features["count"].as_u64()? as usize;

    let mut x = Vec::new();
    let mut y = Vec::new();
    // Indexed rather than zipped: this is a columnar table and every column is
    // addressed by the same `i`.
    #[allow(clippy::needless_range_loop)]
    for i in 0..count {
        let move_atr = num(&forward[i]);
        if !move_atr.is_finite() {
            continue;
        }
        // Neutral rows are in the file and out of the training set — the same
        // split the oracle makes, and in the same place.
        let label = if move_atr >= band {
            true
        } else if move_atr <= -band {
            false
        } else {
            continue;
        };
        x.push(columns.iter().map(|column| num(&column.as_array().unwrap()[i])).collect::<Vec<f64>>());
        y.push(label);
    }
    Some((x, y))
}

fn check_market(market: &str) {
    let Some(expected) = read_golden(market, "model") else {
        eprintln!("skipping {market}: no golden model — run research/export-model.js");
        return;
    };
    let horizon = expected["horizon"].as_str().expect("horizon");
    let band = num(&expected["labelBandAtr"]);
    let Some((x, y)) = training_set(market, horizon, band) else {
        eprintln!("skipping {market}: no feature table");
        return;
    };

    assert_eq!(
        x.len(),
        expected["samples"].as_u64().unwrap_or_default() as usize,
        "{market}: the labelled row count differs — the split, not the trainer"
    );
    assert_eq!(
        y.iter().filter(|v| **v).count(),
        expected["positives"].as_u64().unwrap_or_default() as usize,
        "{market}: the positive count differs"
    );

    let train = &expected["train"];
    let options = TrainOptions {
        epochs: num(&train["epochs"]) as usize,
        lr: num(&train["lr"]),
        l2: num(&train["l2"]),
        class_weight: train["classWeight"].as_bool().unwrap_or(true),
    };
    let names: Vec<String> = expected["featureNames"]
        .as_array()
        .expect("names")
        .iter()
        .map(|n| n.as_str().unwrap().to_string())
        .collect();

    let rows: Vec<&[f64]> = x.iter().map(Vec::as_slice).collect();
    let mut model = LogisticModel::new(names.clone());
    model.fit(&rows, &y, options).expect("fit");

    let mut diffs: Vec<String> = Vec::new();

    // The standardiser first: every weight is expressed in its units, so a
    // difference here explains every later one.
    let standardizer = model.standardizer.as_ref().expect("fitted");
    for (j, name) in names.iter().enumerate() {
        let want_mean = num(&expected["standardizer"]["mean"][j]);
        let want_std = num(&expected["standardizer"]["std"][j]);
        if !parity_eq(standardizer.mean[j], want_mean) {
            diffs.push(format!("mean[{name}]: rust {} vs golden {want_mean}", standardizer.mean[j]));
        }
        if !parity_eq(standardizer.std[j], want_std) {
            diffs.push(format!("std[{name}]: rust {} vs golden {want_std}", standardizer.std[j]));
        }
    }

    for (j, name) in names.iter().enumerate() {
        let want = num(&expected["weights"][j]);
        if !parity_eq(model.weights[j], want) {
            diffs.push(format!("weight[{name}]: rust {} vs golden {want}", model.weights[j]));
        }
    }
    let want_bias = num(&expected["bias"]);
    if !parity_eq(model.bias, want_bias) {
        diffs.push(format!("bias: rust {} vs golden {want_bias}", model.bias));
    }

    let probs: Vec<f64> = x.iter().map(|row| model.predict_proba(row).expect("predict")).collect();
    for (i, want) in expected["sampleProbabilities"].as_array().expect("samples").iter().enumerate() {
        let want = num(want);
        if !parity_eq(probs[i], want) {
            diffs.push(format!("probability[{i}]: rust {} vs golden {want}", probs[i]));
        }
    }

    let metrics = &expected["metrics"];
    for (label, actual, want) in [
        ("auc", auc(&y, &probs), num(&metrics["auc"])),
        ("logLoss", log_loss(&y, &probs), num(&metrics["logLoss"])),
        ("baselineAccuracy", baseline_accuracy(&y), num(&metrics["baselineAccuracy"])),
    ] {
        if !parity_eq(actual, want) {
            diffs.push(format!("{label}: rust {actual} vs golden {want}"));
        }
    }

    assert!(
        diffs.is_empty(),
        "{market}: the model differs from the oracle:\n  {}",
        diffs.iter().take(15).cloned().collect::<Vec<_>>().join("\n  ")
    );
    println!(
        "{market}: {} rows, {} coefficients and AUC {:.4} match the oracle",
        x.len(),
        model.weights.len(),
        auc(&y, &probs)
    );
}

#[test]
fn the_gold_model_matches_the_oracle() {
    check_market("gold");
}

#[test]
fn the_btc_model_matches_the_oracle() {
    check_market("btc");
}
