//! Dataset construction.
//!
//! Walks the tape forward in fixed steps, extracts a feature row from what
//! existed at each step, and labels it with the realised forward move. The
//! replay cursor is the only source of "now", so a label cannot leak into its
//! own features.
//!
//! Labels are ATR-relative: a move counts as up only if it clears a fraction of
//! ATR. Neutral rows are kept in the file — they are useful for calibration —
//! and filtered at training time, which is a different decision made in a
//! different place on purpose.

use std::collections::{BTreeMap, HashMap};

use fd_core::config::Config;
use fd_core::types::{Bar, OptionTrade};
use fd_engine::engine::{ContractMeta, EngineSettings, OptionsEngine};

use crate::{FEATURE_NAMES, FeatureRow, extract, price_context};

#[derive(Debug, Clone)]
pub struct Row {
    /// Epoch milliseconds of the decision point.
    pub t: i64,
    /// Feature values, in [`FEATURE_NAMES`] order.
    pub x: Vec<f64>,
    /// Forward move in ATR units, per horizon.
    pub fwd: BTreeMap<String, f64>,
    pub spot: f64,
    pub atr: f64,
    pub front_symbol: String,
    /// `-1` when the front contract has no usable DTE, matching the oracle.
    pub front_dte: f64,
    pub cluster_score: f64,
}

#[derive(Debug, Clone)]
pub struct Dataset {
    pub rows: Vec<Row>,
    pub feature_names: Vec<&'static str>,
    pub step_ms: i64,
    pub horizons: BTreeMap<String, i64>,
}

/// Build a dataset by replaying `trades` against `candles`.
///
/// A row is only emitted when **every** horizon can be resolved from data that
/// exists. A row near the end of the tape has no future, and labelling it from
/// the last available price would teach the model that the tape ending is a
/// market event.
#[must_use]
pub fn build(
    trades: &[OptionTrade],
    candles: &[Bar],
    config: &Config,
    settings: &EngineSettings,
    meta: &HashMap<String, ContractMeta>,
) -> Dataset {
    let horizons: BTreeMap<String, i64> = config.ai.horizons_ms.clone();
    let step_ms = config.ai.dataset_step_ms;
    let empty = Dataset {
        rows: Vec::new(),
        feature_names: FEATURE_NAMES.to_vec(),
        step_ms,
        horizons: horizons.clone(),
    };
    if trades.is_empty() || candles.is_empty() || step_ms <= 0 {
        return empty;
    }

    let mut sorted: Vec<&OptionTrade> = trades.iter().collect();
    sorted.sort_by_key(|t| t.timestamp);
    let mut series: Vec<Bar> = candles.to_vec();
    series.sort_by_key(|b| b.time);

    let start = sorted[0].timestamp + config.ai.warmup_ms;
    let end = sorted[sorted.len() - 1].timestamp;
    let max_horizon = horizons.values().copied().max().unwrap_or_default();
    let last_candle = series[series.len() - 1].time;

    let mut engine = OptionsEngine::new(settings.clone());
    let mut cursor = 0usize;
    let mut rows = Vec::new();
    let mut t = start;

    while t <= end {
        let from = cursor;
        while cursor < sorted.len() && sorted[cursor].timestamp <= t {
            cursor += 1;
        }
        if cursor > from {
            let batch: Vec<OptionTrade> = sorted[from..cursor].iter().map(|t| (*t).clone()).collect();
            engine.ingest(&batch);
        }
        if cursor == 0 {
            t += step_ms;
            continue;
        }

        let price = price_context(&series, t, config.ai.atr_period, config.ai.atr_bar_ms);
        if !price.price.is_finite() || !price.atr.is_finite() {
            t += step_ms;
            continue;
        }

        let snapshot = engine.snapshot(meta, None);
        let Some(FeatureRow { values, meta: row_meta }) = extract(&snapshot, &price, config) else {
            t += step_ms;
            continue;
        };

        // Past this point the tape has no future left to label against.
        if t + max_horizon > last_candle {
            break;
        }

        let mut fwd = BTreeMap::new();
        let mut resolved = true;
        for (name, ms) in &horizons {
            match close_at_or_after(&series, t + ms) {
                Some(future) => {
                    fwd.insert(name.clone(), (future - price.price) / price.atr);
                }
                None => {
                    resolved = false;
                    break;
                }
            }
        }
        if !resolved {
            t += step_ms;
            continue;
        }

        rows.push(Row {
            t,
            x: values,
            fwd,
            spot: price.price,
            atr: price.atr,
            front_symbol: row_meta.front_symbol,
            front_dte: if row_meta.front_dte.is_finite() { row_meta.front_dte } else { -1.0 },
            cluster_score: row_meta.cluster_score,
        });
        t += step_ms;
    }

    Dataset { rows, feature_names: FEATURE_NAMES.to_vec(), step_ms, horizons }
}

/// First close at or after `target_ms`.
#[must_use]
pub fn close_at_or_after(candles: &[Bar], target_ms: i64) -> Option<f64> {
    let index = candles.partition_point(|c| c.time < target_ms);
    candles.get(index).map(|c| c.close)
}

/// Binary label for a horizon: `Some(true)` up beyond the band, `Some(false)`
/// down beyond it, `None` neutral.
///
/// Neutral is not a third class — it is a row that says nothing, and training on
/// it teaches the model to predict the middle of a range it was never asked
/// about.
#[must_use]
pub fn label_of(row: &Row, horizon: &str, band_atr: f64) -> Option<bool> {
    let move_atr = *row.fwd.get(horizon)?;
    if !move_atr.is_finite() {
        return None;
    }
    if move_atr >= band_atr {
        Some(true)
    } else if move_atr <= -band_atr {
        Some(false)
    } else {
        None
    }
}

/// Rows split into a feature matrix and a label vector for one horizon.
#[must_use]
pub fn to_training_set<'a>(
    rows: &'a [Row],
    horizon: &str,
    band_atr: f64,
) -> (Vec<&'a [f64]>, Vec<bool>, Vec<&'a Row>) {
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut kept = Vec::new();
    for row in rows {
        let Some(label) = label_of(row, horizon, band_atr) else { continue };
        x.push(row.x.as_slice());
        y.push(label);
        kept.push(row);
    }
    (x, y, kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_with(fwd: &[(&str, f64)]) -> Row {
        Row {
            t: 0,
            x: vec![0.0; FEATURE_NAMES.len()],
            fwd: fwd.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
            spot: 100.0,
            atr: 1.0,
            front_symbol: "X".into(),
            front_dte: 1.0,
            cluster_score: 0.0,
        }
    }

    #[test]
    fn a_move_inside_the_band_is_not_a_label() {
        let row = row_with(&[("60m", 0.2)]);
        assert_eq!(label_of(&row, "60m", 0.5), None, "a small move says nothing");
        assert_eq!(label_of(&row_with(&[("60m", 0.6)]), "60m", 0.5), Some(true));
        assert_eq!(label_of(&row_with(&[("60m", -0.6)]), "60m", 0.5), Some(false));
    }

    #[test]
    fn the_band_edge_counts_as_a_label() {
        assert_eq!(label_of(&row_with(&[("60m", 0.5)]), "60m", 0.5), Some(true));
        assert_eq!(label_of(&row_with(&[("60m", -0.5)]), "60m", 0.5), Some(false));
    }

    #[test]
    fn an_unknown_horizon_yields_nothing() {
        assert_eq!(label_of(&row_with(&[("60m", 1.0)]), "240m", 0.5), None);
    }

    #[test]
    fn neutral_rows_are_kept_in_the_dataset_and_dropped_from_training() {
        let rows = vec![row_with(&[("60m", 1.0)]), row_with(&[("60m", 0.1)]), row_with(&[("60m", -1.0)])];
        let (x, y, kept) = to_training_set(&rows, "60m", 0.5);
        assert_eq!(x.len(), 2);
        assert_eq!(y, vec![true, false]);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn the_forward_close_is_the_first_at_or_after_the_target() {
        let candles = vec![Bar::flat(0, 10.0), Bar::flat(60_000, 11.0), Bar::flat(120_000, 12.0)];
        assert_eq!(close_at_or_after(&candles, 60_000), Some(11.0));
        assert_eq!(close_at_or_after(&candles, 59_999), Some(11.0));
        assert_eq!(close_at_or_after(&candles, 120_001), None, "no future is not a future of zero");
    }
}
