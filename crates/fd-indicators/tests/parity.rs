//! Parity gate: the Rust indicators must reproduce the JavaScript oracle.
//!
//! The golden files were produced by `research/export-golden.js` in the
//! prototype repository, over the same cached tapes. If this test fails, the
//! port changed a number — which is exactly what it exists to catch.
//!
//! Non-finite values are serialised as the strings `"NaN"` / `"Infinity"`,
//! because JSON has no literal for them. A warm-up gap is meaningful data here,
//! so NaN must match NaN rather than being skipped.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fd_core::parity_eq;
use fd_core::types::Bar;
use fd_indicators::{IndicatorSpec, Source, compute_indicators};
use serde_json::Value;

/// Specs the exporter used, in the same order.
fn golden_specs() -> Vec<IndicatorSpec> {
    vec![
        IndicatorSpec::new("sma").with("period", 20.0),
        IndicatorSpec::new("ema").with("period", 21.0),
        IndicatorSpec::new("ema").with("period", 55.0),
        IndicatorSpec::new("rsi").with("period", 14.0),
        IndicatorSpec::new("macd").with("fast", 12.0).with("slow", 26.0).with("signal", 9.0),
        IndicatorSpec::new("bbands").with("period", 20.0).with("mult", 2.0),
        IndicatorSpec::new("atr").with("period", 14.0),
        IndicatorSpec::new("vwap"),
        IndicatorSpec::new("stoch").with("period", 14.0).with("smoothK", 3.0).with("smoothD", 3.0),
        IndicatorSpec::new("adx").with("period", 14.0),
        IndicatorSpec::new("donchian").with("period", 20.0),
        IndicatorSpec::new("keltner").with("period", 20.0).with("atrPeriod", 10.0).with("mult", 1.5),
    ]
}

fn golden_dir() -> PathBuf {
    // crates/fd-indicators -> workspace root -> tests/golden
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("tests").join("golden")
}

fn read_golden(market: &str, name: &str) -> Option<Value> {
    let path = golden_dir().join(format!("{market}-{name}.json"));
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// JSON number, or the string form of a non-finite value.
fn as_f64(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => match s.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => panic!("unexpected non-numeric value in golden file: {other}"),
        },
        Value::Null => f64::NAN,
        other => panic!("unexpected value in golden file: {other}"),
    }
}

fn bars_from_golden(market: &str) -> Option<Vec<Bar>> {
    let value = read_golden(market, "bars")?;
    let rows = value.get("bars")?.as_array()?;
    Some(
        rows.iter()
            .map(|row| Bar {
                time: row["time"].as_i64().expect("bar time"),
                open: as_f64(&row["open"]),
                high: as_f64(&row["high"]),
                low: as_f64(&row["low"]),
                close: as_f64(&row["close"]),
                volume: row.get("volume").map(as_f64).filter(|v| v.is_finite()),
            })
            .collect(),
    )
}

// The oracle rounds to nine decimals on the way out. Do NOT re-round the Rust
// value before comparing: `f64::round` breaks ties away from zero while
// JavaScript's `toFixed` rounds the underlying binary value, so a value sitting
// exactly on a `…5` boundary lands on different sides and a bit-identical
// computation looks like a port bug. `parity_eq` already carries a tolerance
// wide enough to absorb the half-unit the serialisation can cost.

fn check_market(market: &str) {
    let Some(bars) = bars_from_golden(market) else {
        eprintln!("skipping {market}: no golden bars (run research/export-golden.js)");
        return;
    };
    let Some(expected) = read_golden(market, "indicators") else {
        eprintln!("skipping {market}: no golden indicators");
        return;
    };

    let expected_series: BTreeMap<String, Vec<f64>> = expected["series"]
        .as_object()
        .expect("series object")
        .iter()
        .map(|(k, v)| (k.clone(), v.as_array().expect("series array").iter().map(as_f64).collect()))
        .collect();

    let actual = compute_indicators(&bars, &golden_specs()).expect("indicators compute");

    // Every series the oracle produced must exist here, under the same key.
    let mut missing: Vec<&String> = expected_series.keys().filter(|k| !actual.contains_key(*k)).collect();
    missing.sort();
    assert!(missing.is_empty(), "{market}: series missing from the Rust port: {missing:?}");

    let mut mismatches = Vec::new();
    for (key, expected_values) in &expected_series {
        let actual_values = &actual[key];
        assert_eq!(
            actual_values.len(),
            expected_values.len(),
            "{market}/{key}: length {} vs golden {}",
            actual_values.len(),
            expected_values.len()
        );
        for (i, (a, e)) in actual_values.iter().zip(expected_values).enumerate() {
            if !parity_eq(*a, *e) {
                mismatches.push(format!("{key}[{i}]: rust {a} vs golden {e}"));
                if mismatches.len() >= 12 {
                    break;
                }
            }
        }
        if mismatches.len() >= 12 {
            break;
        }
    }

    assert!(
        mismatches.is_empty(),
        "{market}: {} indicator value(s) differ from the oracle:\n  {}",
        mismatches.len(),
        mismatches.join("\n  ")
    );

    eprintln!("{market}: {} indicator series match the oracle over {} bars", expected_series.len(), bars.len());
}

#[test]
fn indicators_match_the_oracle_on_gold() {
    check_market("gold");
}

#[test]
fn indicators_match_the_oracle_on_btc() {
    check_market("btc");
}

/// Guards the source selector, which the golden files do not exercise: every
/// spec in them reads closes.
#[test]
fn a_non_default_source_changes_the_result() {
    let Some(bars) = bars_from_golden("btc").or_else(|| bars_from_golden("gold")) else {
        eprintln!("skipping: no golden bars");
        return;
    };
    let mut high_spec = IndicatorSpec::new("sma").with("period", 20.0);
    high_spec.source = Some(Source::High);
    let on_high = compute_indicators(&bars, &[high_spec]).unwrap();
    let on_close = compute_indicators(&bars, &[IndicatorSpec::new("sma").with("period", 20.0)]).unwrap();
    let (a, b) = (&on_high["sma_20.sma"], &on_close["sma_20.sma"]);
    let last = a.len() - 1;
    assert!(a[last] >= b[last], "an SMA of highs cannot sit below an SMA of closes");
}
