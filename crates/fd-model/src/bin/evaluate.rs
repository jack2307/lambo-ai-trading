//! What the model is actually worth.
//!
//! Prints the in-sample score beside the walk-forward one, because the gap
//! between them is the finding. An in-sample AUC is what a model scores on rows
//! it has already memorised: it looks exactly like a result, and reporting it
//! alone is the most common way a trading model gets believed.
//!
//! ```text
//! cargo run --release -p fd-model --bin evaluate -- --market=gold
//! ```

use std::path::{Path, PathBuf};

use fd_model::{LogisticModel, TrainOptions, auc, baseline_accuracy, calibration, walk_forward};
use serde_json::Value;

fn arg(name: &str, fallback: &str) -> String {
    std::env::args()
        .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

fn golden(market: &str, name: &str) -> Option<Value> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("golden")
        .join(format!("{market}-{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn num(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) if s == "NaN" => f64::NAN,
        _ => f64::NAN,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market = arg("market", "gold");
    let Some(features) = golden(&market, "features") else {
        println!("no feature table for {market} — run research/export-features.js");
        return Ok(());
    };
    let horizon = arg("horizon", "60m");
    let band = num(&features["settings"]["labelBandAtr"]);

    let columns = features["x"].as_array().expect("x");
    let Some(forward) = features["fwd"][&horizon].as_array() else {
        println!("no horizon {horizon} in the table");
        return Ok(());
    };
    let count = features["count"].as_u64().unwrap_or_default() as usize;

    let (mut x, mut y, mut neutral) = (Vec::<Vec<f64>>::new(), Vec::<bool>::new(), 0usize);
    // Indexed rather than zipped: this is a columnar table and every other
    // column is addressed by the same `i`.
    #[allow(clippy::needless_range_loop)]
    for i in 0..count {
        let move_atr = num(&forward[i]);
        if !move_atr.is_finite() {
            continue;
        }
        // A move inside the band is not a small signal, it is no signal.
        if move_atr.abs() < band {
            neutral += 1;
            continue;
        }
        x.push(columns.iter().map(|c| num(&c.as_array().unwrap()[i])).collect());
        y.push(move_atr >= band);
    }

    let names: Vec<String> = features["featureNames"]
        .as_array()
        .expect("names")
        .iter()
        .map(|n| n.as_str().unwrap().to_string())
        .collect();
    let options = TrainOptions { epochs: 800, lr: 0.1, l2: 2.0, class_weight: true };
    let rows: Vec<&[f64]> = x.iter().map(Vec::as_slice).collect();

    println!("{market}  horizon {horizon}, band {band} ATR");
    println!(
        "  {} labelled rows ({} up), {neutral} neutral dropped",
        x.len(),
        y.iter().filter(|v| **v).count()
    );
    println!("  baseline (majority class): {:.4}", baseline_accuracy(&y));
    println!();

    let mut model = LogisticModel::new(names.clone());
    model.fit(&rows, &y, options)?;
    let in_sample: Vec<f64> = x.iter().map(|r| model.predict_proba(r).unwrap_or(f64::NAN)).collect();
    println!("  in-sample AUC:    {:.4}   <- the model has seen every one of these rows", auc(&y, &in_sample));

    match walk_forward(&rows, &y, &names, options, 4, 120) {
        Ok(result) => {
            println!("  walk-forward AUC: {:.4}   <- {} predictions it had not seen", result.oos_auc, result.oos_samples);
            println!(
                "  walk-forward acc: {:.4}   (baseline {:.4})",
                result.oos_accuracy, result.oos_baseline
            );
            println!();
            println!("  {:<6}{:>8}{:>8}{:>9}{:>10}", "fold", "train", "test", "AUC", "accuracy");
            for fold in &result.folds {
                println!(
                    "  {:<6}{:>8}{:>8}{:>9.4}{:>10.4}",
                    fold.fold, fold.train_samples, fold.test_samples, fold.auc, fold.accuracy
                );
            }
        }
        Err(error) => println!("  walk-forward: {error}"),
    }

    println!();
    println!("  in-sample reliability (predicted vs actual):");
    for bin in calibration(&y, &in_sample, 5) {
        if bin.n == 0 {
            continue;
        }
        println!("    {:<10}{:>6}{:>12.3}{:>10.3}", bin.bin, bin.n, bin.predicted, bin.actual);
    }

    println!();
    println!("  strongest coefficients:");
    for (name, weight) in model.top_factors(6) {
        println!("    {name:<22}{weight:>9.4}");
    }
    Ok(())
}
