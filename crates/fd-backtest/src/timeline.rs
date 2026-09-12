//! Building an options timeline from a tape.
//!
//! A [`Frame`] is the options picture at one instant, compacted to what the
//! strategies actually read. Building them is a replay: advance the engine to
//! `t`, take a snapshot, keep the parts a strategy is allowed to see.
//!
//! Two properties this has to preserve, both of which are easy to lose:
//!
//! * **Nothing may look forward.** Each frame consumes only prints at or before
//!   its own timestamp. The engine is fed incrementally rather than rebuilt
//!   from a filtered slice, which is both faster and harder to get wrong.
//! * **Live and historical frames must be the same shape.** A level that means
//!   one thing in a backtest and another on screen is the kind of difference
//!   nobody notices until a decision has been made on it. [`frame_from_snapshot`]
//!   is the single place a snapshot becomes a frame, for both paths.

use std::collections::HashMap;

use fd_core::classify::{is_bear, is_bull};
use fd_core::types::OptionTrade;
use fd_engine::engine::{ContractMeta, EngineSettings, OptionsEngine, Snapshot};
use fd_strategy::registry::{ClusterView, ContextView};

use crate::context::{Frame, OptionsTimeline};

/// How a tape is turned into frames.
#[derive(Debug, Clone)]
pub struct TimelineOptions {
    /// Distance between frames, in milliseconds.
    pub step_ms: i64,
    /// Window the big-print imbalance is measured over.
    ///
    /// Deliberately windowed: cumulative flow hides the moment a tape turns,
    /// which is the only moment the number is useful.
    pub big_trade_window_ms: i64,
}

impl Default for TimelineOptions {
    fn default() -> Self {
        Self { step_ms: 300_000, big_trade_window_ms: 900_000 }
    }
}

/// Replay `trades` and take a frame every `step_ms`.
///
/// Frames before the first contract exists are skipped rather than emitted
/// empty: a strategy reading a frame with no contexts would see every level as
/// missing, which is indistinguishable from a market with no levels.
#[must_use]
pub fn build_timeline(
    trades: &[OptionTrade],
    settings: &EngineSettings,
    meta: &HashMap<String, ContractMeta>,
    options: &TimelineOptions,
) -> OptionsTimeline {
    if trades.is_empty() || options.step_ms <= 0 {
        return OptionsTimeline::new(Vec::new());
    }
    // The caller may hand over any order; the replay depends on time order.
    let mut sorted: Vec<&OptionTrade> = trades.iter().collect();
    sorted.sort_by_key(|t| t.timestamp);

    let start = sorted[0].timestamp;
    let end = sorted[sorted.len() - 1].timestamp;

    let mut engine = OptionsEngine::new(settings.clone());
    let mut frames = Vec::new();
    let mut cursor = 0usize;
    let mut t = start;

    while t <= end {
        // Everything at or before `t`, and nothing after it.
        let from = cursor;
        while cursor < sorted.len() && sorted[cursor].timestamp <= t {
            cursor += 1;
        }
        if cursor > from {
            let batch: Vec<OptionTrade> = sorted[from..cursor].iter().map(|t| (*t).clone()).collect();
            engine.ingest(&batch);
        } else if frames.is_empty() {
            // Nothing has happened yet; there is no picture to take.
            t += options.step_ms;
            continue;
        }

        let snapshot = engine.snapshot(meta, None);
        if !snapshot.contexts.is_empty() {
            frames.push(frame_from_snapshot(&snapshot, options.big_trade_window_ms));
        }
        t += options.step_ms;
    }
    OptionsTimeline::new(frames)
}

/// Compact one engine snapshot into the frame the strategies read.
#[must_use]
pub fn frame_from_snapshot(snapshot: &Snapshot, big_trade_window_ms: i64) -> Frame {
    let fifteen = snapshot.flow_windows.get("15m").unwrap_or(&snapshot.flow_overall);
    let total_premium =
        (snapshot.flow_overall.bull_premium + snapshot.flow_overall.bear_premium).max(1.0);

    let cutoff = snapshot.as_of - big_trade_window_ms;
    let (mut big_bull, mut big_bear) = (0.0, 0.0);
    for print in &snapshot.big_trades {
        if print.timestamp < cutoff {
            continue;
        }
        if is_bull(print.flow_class) {
            big_bull += print.premium_usd;
        } else if is_bear(print.flow_class) {
            big_bear += print.premium_usd;
        }
    }
    let big_total = big_bull + big_bear;

    Frame {
        t: snapshot.as_of,
        spot: snapshot.spot,
        bull_ratio: snapshot.flow_overall.bull_ratio,
        bull_ratio_15m: fifteen.bull_ratio,
        net_flow_velocity_norm: clamp(snapshot.velocity.velocity / total_premium, -1.0, 1.0),
        big_trade_imbalance: if big_total > 0.0 { (big_bull - big_bear) / big_total } else { 0.0 },
        clusters: snapshot
            .clusters
            .iter()
            .map(|c| ClusterView { low: c.low, high: c.high, center: c.center, score: c.score })
            .collect(),
        contexts: snapshot
            .contexts
            .iter()
            .map(|c| ContextView {
                symbol: c.symbol.clone(),
                dte: c.dte,
                max_pain: c.max_pain,
                poc: c.poc,
                w_sup: c.w_sup,
                w_res: c.w_res,
                call_be: c.call_be,
                put_be: c.put_be,
                bull_ratio: c.flow.bull_ratio,
            })
            .collect(),
    }
}

/// Clamp, treating a non-finite reading as zero.
///
/// A velocity of NaN means the tape has not moved enough to measure, not that
/// the market is at an extreme — and every comparison against NaN is false, so
/// passing it through would make a strategy's guard silently fail open.
fn clamp(value: f64, low: f64, high: f64) -> f64 {
    if value.is_finite() { value.clamp(low, high) } else { 0.0 }
}
