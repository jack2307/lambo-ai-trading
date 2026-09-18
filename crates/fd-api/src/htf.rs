//! Higher-timeframe facts, computed once so the model and the desk read the
//! same numbers.
//!
//! `GET /api/paper/htf?market=xauusd`. Everything here is derived from CLOSED
//! H1, H4 and D1 bars and nothing else: no 15m bar, no forming bar, no tick.
//! The
//! point of the route is that there is exactly one implementation of these
//! facts — an AI prompt and a Desk panel that each computed their own would
//! eventually disagree, and the disagreement would surface as a model
//! explaining a level the screen does not show.
//!
//! **No verdict beyond the structure label.** There is no composite score and
//! no "trend = UP" beyond what the swing rule itself says. The facts are the
//! product; combining them is the reader's job, model or human.
//!
//! ## Units
//!
//! Every price field is in the market's own quote units, as everywhere else on
//! this API. Every field that is NOT a price carries its unit in its name —
//! `dist_ema21_atr`, `adx14`, `efficiency_20`, `close_pct_of_prior_week_range`,
//! `..._ms`. This desk spent 2026-09-17 removing five numbers whose units lived
//! only in prose.
//!
//! ## Null is not zero
//!
//! Every fact is `Option`, every one is always PRESENT in the JSON, and `null`
//! means the fact could not be computed — a warmup not met, a swing leg that
//! has not formed, a Donchian window with no new high in it. Zero is a
//! measurement. A reader must render them differently.
//!
//! ## Two intraday blocks, and no vote between them
//!
//! `h1` and `h4` are the SAME facts on different bars: one type, one code
//! path, with `timeframe` and `bar_ms` on each object saying which. There is
//! deliberately no alignment score, no agreement flag and no combined bias
//! word. When the two disagree, the two labels side by side ARE the
//! information, and a number averaging them would destroy the thing the second
//! block was added to show.
//!
//! The running books are unaffected. Their prompt reads H4 only, exactly as
//! registered; adding H1 to it would be a prompt change and therefore a new
//! campaign, which is not what this is.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use fd_core::types::Bar;
use fd_indicators::{IndicatorSpec, compute_indicators};
use fd_store::read_bars;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::paper::now_ms;
use crate::state::AppState;

/// Which file the bars came from, and how many were read.
///
/// Named on the response because this route REFUSES to resample. `AppState`
/// will happily rebucket a 15m series into "4h" on request, anchored to the
/// Unix epoch — 00/04/08/12/16/20 UTC. The broker's H4 candles start at
/// 21:00 UTC (verified 2026-09-18: hour 21Z holds exactly zero 15m bars
/// against 332-348 in every neighbouring hour, because the server is UTC+3 and
/// its day starts there), and the anchor moves to 22:00 UTC outside US summer
/// time. Those are different candles with different highs and lows, not offset
/// versions of the same ones, and "prior-day high" computed on the wrong
/// anchor is a level a trader would recognise the name of and not the value.
#[derive(Debug, Serialize)]
pub struct HtfSourceDto {
    /// The parquet the bars were read from, relative to the data root.
    pub file: String,
    /// Bars read, after any range limit.
    pub bars: usize,
    /// The stored timeframe of that file. Always equal to the timeframe asked
    /// for; a mismatch is refused rather than resampled.
    pub timeframe: String,
}

/// `UP` needs a higher high AND a higher low, `DOWN` a lower high and a lower
/// low; anything else is `RANGE`. A closed set, so a client can switch on it.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Structure {
    Up,
    Down,
    Range,
}

/// Which side of `break_level` breaks the structure.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BreakSide {
    /// Trading BELOW this price ends the structure (an up-structure's last
    /// higher low).
    Below,
    /// Trading ABOVE it does (a down-structure's last lower high).
    Above,
}

/// Swing structure by the fractal rule.
#[derive(Debug, Serialize)]
pub struct StructureDto {
    pub label: Structure,
    /// The rule, spelled out: `"fractal(2)"` means a swing high is a bar whose
    /// high exceeds the two bars either side of it.
    pub rule: String,
    /// **The bar that CONFIRMED the newest swing, not the swing's own bar.**
    ///
    /// A fractal needs `n` bars after it before it is a fractal, so the newest
    /// swing on an H4 chart is confirmed up to eight hours after it happened
    /// and the label is as of here. A reader told "structure is UP" without
    /// this is being told something stronger than is known.
    pub confirmed_at_bar_ms: Option<i64>,
    pub last_high: Option<SwingDto>,
    pub prior_high: Option<SwingDto>,
    pub last_low: Option<SwingDto>,
    pub prior_low: Option<SwingDto>,
    /// The price that would change the label, and which side breaks it.
    /// `null` on `RANGE`, which has no single level to break.
    pub break_level: Option<f64>,
    pub break_side: Option<BreakSide>,
}

/// One swing point: its price and the bar it happened on.
#[derive(Debug, Serialize)]
pub struct SwingDto {
    pub price: f64,
    pub bar_ms: i64,
}

/// Donchian(20) on an intraday timeframe, as a state rather than a channel.
#[derive(Debug, Serialize)]
pub struct DonchianDto {
    pub upper: Option<f64>,
    pub lower: Option<f64>,
    /// Bars since the last NEW high of the window. `0` means this bar made one
    /// — a measurement. `null` means the window is not full yet.
    pub bars_since_new_high: Option<usize>,
    pub bars_since_new_low: Option<usize>,
}

/// Trend facts on ONE intraday timeframe, all from closed bars.
///
/// One type for `h1` and `h4` rather than two, because they are the same
/// facts under the same rules and the only differences are the two fields
/// that say so. Two structs would let them drift apart a field at a time, and
/// the drift would show up as a card whose H1 row quietly stopped matching its
/// H4 row for a reason nobody chose.
#[derive(Debug, Serialize)]
pub struct TrendDto {
    /// The CLOSED bar these facts describe, UTC epoch milliseconds. The
    /// book's clock, not the broker's: `mt5_bars.py` converts with `to_utc_ms`
    /// before anything is stored.
    pub computed_at_bar_ms: i64,
    /// Wall clock when this response was computed, UTC epoch ms. Differs from
    /// `computed_at_bar_ms` by however long the bar has been closed, and a
    /// card showing the facts should show that age.
    pub computed_at_ms: i64,
    /// This object's own timeframe and bar length, so a reader ageing
    /// `computed_at_bar_ms` never has to reach for a sibling field to learn
    /// how long a bar is. It is also what tells `h1` from `h4` in a value that
    /// has been handed around on its own. The matching `*_source` cannot be
    /// null while this object is not — they are built from the same `Ok` — but
    /// a client should not have to know that to divide by a bar.
    pub timeframe: String,
    pub bar_ms: i64,
    pub structure: StructureDto,
    pub ema21: Option<f64>,
    pub ema55: Option<f64>,
    /// `-1`, `0` or `+1`: the sign of the EMA's change over the last three
    /// closed bars. `0` is a genuine flat, not an absence.
    pub ema21_slope_sign: Option<i8>,
    pub ema55_slope_sign: Option<i8>,
    /// ATR(14) on this timeframe, in quote units. Published because it is the
    /// unit
    /// `dist_ema21_atr` is measured in, and a ratio whose denominator is
    /// invisible cannot be checked.
    pub atr14: Option<f64>,
    /// Signed: positive when the last close is ABOVE the EMA, in ATR(14).
    pub dist_ema21_atr: Option<f64>,
    pub adx14: Option<f64>,
    pub plus_di14: Option<f64>,
    pub minus_di14: Option<f64>,
    /// Kaufman efficiency ratio over 20 bars OF THIS TIMEFRAME: |net move| /
    /// sum|bar moves|, so
    /// 1.0 is a straight line and 0.0 is pure churn. Unitless by construction.
    pub efficiency_20: Option<f64>,
    pub donchian20: DonchianDto,
    /// The last closed close on this timeframe, so every level above can be
    /// read against it without a second request.
    pub last_close: Option<f64>,
}

/// D1 facts: the levels a trader names out loud.
#[derive(Debug, Serialize)]
pub struct D1Dto {
    pub computed_at_bar_ms: i64,
    pub computed_at_ms: i64,
    /// As on [`H4Dto`]: the period these stamps are measured in.
    pub timeframe: String,
    pub bar_ms: i64,
    pub prior_day_high: Option<f64>,
    pub prior_day_low: Option<f64>,
    /// The prior DAY's own bar stamp, so "prior day" is checkable rather than
    /// inferred. On this broker a day runs 21:00 UTC to 21:00 UTC in summer.
    pub prior_day_bar_ms: Option<i64>,
    pub prior_week_high: Option<f64>,
    pub prior_week_low: Option<f64>,
    /// The midpoint of the prior week's range, which is the level itself and
    /// not a statistic about it.
    pub prior_week_mid: Option<f64>,
    /// First D1 bar of the prior week, UTC epoch ms.
    pub prior_week_start_ms: Option<i64>,
    /// Where the last close sits in the prior week's range, in percent.
    ///
    /// **Deliberately not clamped to 0..100.** Above 100 means price has left
    /// the prior week's range to the upside and below 0 to the downside, which
    /// is the most informative thing this number ever says. Clamping would
    /// turn a breakout into a ceiling.
    pub close_pct_of_prior_week_range: Option<f64>,
    pub last_close: Option<f64>,
}

/// `GET /api/paper/htf?market=<id>`
///
/// Always 200 when the market is known, so a card can say WHY it has nothing
/// rather than handling a status code. A block is `null` only when the stored
/// bars for that timeframe are missing entirely; `unavailable_by_tf` then
/// carries the reason, keyed by the same timeframe string that block's own
/// `timeframe` field would have held.
///
/// Individual facts inside a PRESENT block go `null` on their own when their
/// warmup is not met — ADX(14) needs its bars, a fractal needs a leg that has
/// formed. That is a different condition from the timeframe being absent, and
/// the two are reported differently on purpose.
#[derive(Debug, Serialize)]
pub struct HtfResponse {
    pub market: String,
    pub h1: Option<TrendDto>,
    pub h4: Option<TrendDto>,
    pub d1: Option<D1Dto>,
    pub h1_source: Option<HtfSourceDto>,
    pub h4_source: Option<HtfSourceDto>,
    pub d1_source: Option<HtfSourceDto>,
    /// Every reason joined into one sentence, or `null` when nothing is
    /// missing.
    ///
    /// **This cannot say which timeframe it is about, so a caption placed
    /// against one row must not use it.** It was written when there were two
    /// timeframes and the realistic state was that both were absent together.
    /// With three the realistic state is MIXED — H4 answering from its
    /// exported file while H1 is still missing — and one joined sentence under
    /// a good row is a caption confidently wrong about the row above it. Kept,
    /// with its old meaning, for anything already reading it; anything placed
    /// next to a specific block reads `unavailable_by_tf`. (Raised by -48
    /// before building the card rather than after.)
    pub unavailable: Option<String>,
    /// Why each ABSENT timeframe is absent, keyed by `"1h"`, `"4h"`, `"1d"`.
    ///
    /// ALWAYS PRESENT, `{}` when nothing is missing, so a client indexes it
    /// without first testing for null. A key is present exactly when the
    /// matching block is `null`, and an absent key means that timeframe is
    /// fine.
    pub unavailable_by_tf: BTreeMap<String, String>,
}

/* ------------------------------------------------ the facts themselves */

/// A swing high or low by the fractal rule: a bar whose high exceeds the `n`
/// bars either side of it, or whose low undercuts them.
///
/// Strict inequality on both sides, so a flat top is not a swing. Returned
/// oldest first, as `(index, price)`.
///
/// Chosen over an ATR-ZigZag deliberately. ZigZag has a threshold to tune and
/// its newest leg REPAINTS as price moves, so a structure label taken from it
/// is not the label a reader would have seen at the time — which is the one
/// property this route cannot afford, since a model is going to be told the
/// structure and asked to act on it. A fractal is causal and parameter-poor;
/// the price is the confirmation lag, and the route states it rather than
/// hiding it.
fn fractal_swings(bars: &[Bar], n: usize, high: bool) -> Vec<(usize, f64)> {
    if n == 0 || bars.len() < 2 * n + 1 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in n..bars.len() - n {
        let centre = if high { bars[i].high } else { bars[i].low };
        if !centre.is_finite() {
            continue;
        }
        let beats = (1..=n).all(|k| {
            let (a, b) = if high { (bars[i - k].high, bars[i + k].high) } else { (bars[i - k].low, bars[i + k].low) };
            if high { centre > a && centre > b } else { centre < a && centre < b }
        });
        if beats {
            out.push((i, centre));
        }
    }
    out
}

/// Kaufman's efficiency ratio over `period` closes: |net move| / sum of the
/// absolute bar-to-bar moves. 1.0 is a straight line, 0.0 is pure churn.
///
/// Computed here rather than added to `fd-indicators` on purpose. Everything in
/// that registry appears in the Desk's indicator picker, and adding a chart
/// indicator nobody asked for as a side effect of an API route is how a UI
/// grows things no one chose. If a second caller ever wants it, that is the
/// moment to move it.
///
/// `None` rather than zero when the denominator is zero — a window where price
/// never moved has no efficiency, and 0.0 would claim the opposite of 1.0
/// about a perfectly still market.
fn efficiency_ratio(bars: &[Bar], period: usize) -> Option<f64> {
    if period == 0 || bars.len() < period + 1 {
        return None;
    }
    let window = &bars[bars.len() - period - 1..];
    let net = (window[window.len() - 1].close - window[0].close).abs();
    let path: f64 = window.windows(2).map(|w| (w[1].close - w[0].close).abs()).sum();
    if !net.is_finite() || !path.is_finite() || path <= 0.0 {
        return None;
    }
    Some(net / path)
}

/// `-1`, `0` or `+1` for the change over the last `span` values of a series.
/// `None` when the series is too short or either end is not finite.
fn slope_sign(series: &[f64], span: usize) -> Option<i8> {
    if span == 0 || series.len() <= span {
        return None;
    }
    let last = *series.last()?;
    let before = series[series.len() - 1 - span];
    if !last.is_finite() || !before.is_finite() {
        return None;
    }
    Some(match last.partial_cmp(&before)? {
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
    })
}

/// The last finite value of an indicator series, or `None` before its warmup.
fn last_finite(series: Option<&[f64]>) -> Option<f64> {
    series?.iter().rev().copied().find(|v| v.is_finite())
}

/// Bars since the window last made a new extreme, counting the newest bar as
/// zero. `None` when there are fewer than `period` bars to judge against.
fn bars_since_extreme(bars: &[Bar], period: usize, high: bool) -> Option<usize> {
    if period == 0 || bars.len() < period {
        return None;
    }
    let window = &bars[bars.len() - period..];
    let mut best = if high { f64::NEG_INFINITY } else { f64::INFINITY };
    let mut at = 0usize;
    for (i, b) in window.iter().enumerate() {
        let v = if high { b.high } else { b.low };
        if !v.is_finite() {
            continue;
        }
        // `>=` so the NEWEST bar wins a tie: a bar that matches the window's
        // high has made the high, and "0 bars since" is the honest reading.
        if (high && v >= best) || (!high && v <= best) {
            best = v;
            at = i;
        }
    }
    Some(window.len() - 1 - at)
}

fn structure_of(bars: &[Bar], n: usize) -> StructureDto {
    let highs = fractal_swings(bars, n, true);
    let lows = fractal_swings(bars, n, false);
    let swing = |v: &[(usize, f64)], back: usize| -> Option<SwingDto> {
        let (i, price) = *v.get(v.len().checked_sub(back)?)?;
        Some(SwingDto { price, bar_ms: bars[i].time })
    };
    let last_high = swing(&highs, 1);
    let prior_high = swing(&highs, 2);
    let last_low = swing(&lows, 1);
    let prior_low = swing(&lows, 2);

    let higher_high = matches!((&last_high, &prior_high), (Some(a), Some(b)) if a.price > b.price);
    let higher_low = matches!((&last_low, &prior_low), (Some(a), Some(b)) if a.price > b.price);
    let lower_high = matches!((&last_high, &prior_high), (Some(a), Some(b)) if a.price < b.price);
    let lower_low = matches!((&last_low, &prior_low), (Some(a), Some(b)) if a.price < b.price);

    let label = if higher_high && higher_low {
        Structure::Up
    } else if lower_high && lower_low {
        Structure::Down
    } else {
        Structure::Range
    };

    // The level whose break ends the structure, and nothing on a RANGE: a
    // range has no single price that changes the label, and inventing one
    // would claim a precision the rule does not have.
    let (break_level, break_side) = match label {
        Structure::Up => (last_low.as_ref().map(|s| s.price), Some(BreakSide::Below)),
        Structure::Down => (last_high.as_ref().map(|s| s.price), Some(BreakSide::Above)),
        Structure::Range => (None, None),
    };

    // The bar that CONFIRMED the newest swing, which is `n` bars after the
    // swing itself. This is how old the label is, and it is not the same
    // number as the bar the facts are computed on.
    let newest = [
        highs.last().map(|(i, _)| *i),
        lows.last().map(|(i, _)| *i),
    ]
    .into_iter()
    .flatten()
    .max();
    let confirmed_at_bar_ms = newest.and_then(|i| bars.get(i + n)).map(|b| b.time);

    StructureDto {
        label,
        rule: format!("fractal({n})"),
        confirmed_at_bar_ms,
        last_high,
        prior_high,
        last_low,
        prior_low,
        break_level,
        break_side,
    }
}

/// Split D1 bars into weeks by the weekend HOLE rather than by the calendar.
///
/// A broker week runs from the Sunday reopen to the Friday close, and the
/// stamps move with US daylight saving — the exported D1 bars sit at 21:00 UTC
/// in summer and 22:00 UTC in winter. Any calendar rule has to know that and
/// would be wrong twice a year at the changeover. The gap does not: two
/// consecutive daily bars more than two days apart is a weekend, whatever the
/// clock did.
///
/// Returns the index ranges, oldest first.
fn weeks_of(bars: &[Bar]) -> Vec<std::ops::Range<usize>> {
    const TWO_DAYS_MS: i64 = 2 * 86_400_000;
    let mut out = Vec::new();
    let mut start = 0usize;
    for i in 1..bars.len() {
        if bars[i].time - bars[i - 1].time > TWO_DAYS_MS {
            out.push(start..i);
            start = i;
        }
    }
    if start < bars.len() {
        out.push(start..bars.len());
    }
    out
}

/* ------------------------------------------------ loading, and refusing */

/// How far back the intraday facts are computed from, in BARS of whatever
/// timeframe was asked for.
///
/// The bound is WARMUP, not calendar. EMA55 is the longest thing here and is
/// within a rounding error of converged well inside 400 bars; ADX(14),
/// Donchian(20), efficiency(20) and fractal(2) need far fewer. So the same
/// number serves H1 and H4.
///
/// The consequence, said out loud because it is the part a reader would
/// otherwise assume away: the two blocks look back over different LENGTHS of
/// TIME — about ten weeks on H4, about three on H1. That is the intended
/// reading. `h1` is a statement about the last few weeks of hours and `h4` a
/// statement about the last few months of sessions, and forcing them onto a
/// common calendar window would make one of them shorter than its own warmup
/// or the other longer than anybody would call intraday.
const TREND_BARS: usize = 400;
/// 200 D1 is ten months.
const D1_BARS: usize = 200;
/// The fractal half-width. A parameter because the brief asked for one, not
/// because anything here tunes it.
const FRACTAL_N: usize = 2;

/// The MT5 name for one of our timeframes, for the export hint in a refusal.
///
/// Exhaustive rather than `if 4h { H4 } else { D1 }`, which is what this was
/// while the route served two timeframes. That spelling was correct for its
/// two callers and silently wrong for the third: adding `1h` to the caller
/// list would have printed `--timeframes D1` in the sentence telling an
/// operator how to fix a missing H1 file, and the sentence would have read
/// perfectly.
fn mt5_name(timeframe: &str) -> &str {
    match timeframe {
        "1h" => "H1",
        "4h" => "H4",
        "1d" => "D1",
        other => other,
    }
}

/// The stored series for a timeframe, and nothing else.
///
/// **This refuses where [`AppState::bars`] resamples**, and that is the whole
/// reason it exists rather than calling it. `AppState::bars` falls back to
/// rebucketing a finer series when no file matches, anchored to the Unix
/// epoch — and the broker's higher-timeframe candles are not on that grid.
/// Measured 2026-09-18 from the terminal's own export: real H4 bars fall on
/// hours {1,2,5,6,9,10,13,14,17,18,21,22} UTC and real D1 on {21,22}, against
/// {0,4,8,12,16,20} and {0} for an epoch-anchored resample. Not one hour in
/// common. Those would be different candles with different highs and lows, and
/// a "prior-day high" taken from them is a level a trader would recognise the
/// name of and not the value.
///
/// It also reads the file directly rather than through the bar cache, because
/// that cache is keyed `market/timeframe` and would hand back a resampled
/// series under the same key if any other route had asked for one first.
///
/// ## H1 refuses too, for a DIFFERENT reason
///
/// The anchor argument above does not apply to H1, and it should not be
/// repeated as though it did. The server is a whole number of hours from UTC,
/// so its hour boundaries and the epoch's coincide: measured 2026-09-18 over
/// the stored XAUUSD files, all 6,727 H4 stamps and all 1,122 D1 stamps carry
/// minute 0 and second 0, back to 2022. An epoch-anchored 1h bucket really
/// would be the broker's 1h bucket.
///
/// It refuses anyway, because [`fd_store::resample`] closes a bucket only when
/// a row lands past it and therefore emits the TRAILING PARTIAL BUCKET as an
/// ordinary bar. Resampling 1h from 15m five minutes into the hour would hand
/// this route a still-forming hour indistinguishable from a closed one, and
/// every fact in this file is documented as closed-bars-only. The structure
/// label would repaint — the exact property the fractal rule was chosen to
/// avoid — and it would repaint invisibly, four times an hour.
///
/// So: same refusal, different reason. Written down because "H1 is whole
/// hours, so there is no anchor question" is true and is not a reason to
/// resample, and the next person to notice it is true will need the second
/// half of the sentence.
fn stored_only(state: &AppState, market: &str, timeframe: &str) -> Result<(Vec<Bar>, HtfSourceDto), String> {
    let spec = state.config.market(market).map_err(|e| e.to_string())?;
    let name = format!("{}-{timeframe}.parquet", spec.bar_symbol);
    let path = state.data.join("bars").join(&name);
    if !path.exists() {
        return Err(format!(
            "no {timeframe} bars for {market}: bars/{name} has not been exported yet \
             (py/ingest/mt5_export.py --symbols {} --timeframes {})",
            spec.bar_symbol,
            mt5_name(timeframe)
        ));
    }
    let bars = read_bars(&path).map_err(|e| format!("bars/{name}: {e}"))?;
    if bars.is_empty() {
        return Err(format!("bars/{name} is empty"));
    }
    let source = HtfSourceDto { file: format!("bars/{name}"), bars: bars.len(), timeframe: timeframe.to_string() };
    Ok((bars, source))
}

/// The trend facts for one intraday timeframe.
///
/// `timeframe` and `bar_ms` are passed in rather than derived, and they are
/// the ONLY things that differ between `h1` and `h4`: one code path computes
/// both, so a change to the rules cannot reach one block and miss the other.
/// They are not cross-checked against each other here because the single
/// caller is the route, three lines below the constants they come from.
///
/// `all` is never empty: [`stored_only`] refuses an empty file before this is
/// called, and that is the only caller. The `map_or(0, ..)` on the bar stamp
/// below is therefore unreachable rather than a default — a zero there would
/// render as 1970 downstream, which is why it is named here instead of left
/// to be discovered. (Raised by b5 reading the code rather than the summary.)
fn trend_facts(all: &[Bar], timeframe: &str, bar_ms: i64) -> TrendDto {
    let bars = &all[all.len().saturating_sub(TREND_BARS)..];
    let specs = [
        IndicatorSpec::new("ema").with("period", 21.0),
        IndicatorSpec::new("ema").with("period", 55.0),
        IndicatorSpec::new("atr").with("period", 14.0),
        IndicatorSpec::new("adx").with("period", 14.0),
        IndicatorSpec::new("donchian").with("period", 20.0),
    ];
    let ind = compute_indicators(bars, &specs).unwrap_or_default();
    // Keys are `indicator_key` + output: `ema_21` (single-output indicators
    // also get the bare alias), `adx_14.plusDi`, `donchian_20.upper`. Spelled
    // out here rather than guessed - `fd-indicators` pins these against the
    // JavaScript oracle in `keys_match_the_javascript_oracle`.
    let at = |k: &str| -> Option<&[f64]> { ind.get(k).map(|s| &s[..]) };
    let ema21_series = at("ema_21");
    let ema55_series = at("ema_55");

    let ema21 = last_finite(ema21_series);
    let atr14 = last_finite(at("atr_14"));
    let last_close = bars.last().map(|b| b.close).filter(|v| v.is_finite());
    let dist_ema21_atr = match (last_close, ema21, atr14) {
        // Guarded rather than silently infinite: an ATR of zero is a market
        // that did not move, and dividing by it would publish a distance of
        // infinity as though it were measured.
        (Some(c), Some(e), Some(a)) if a > 0.0 => Some((c - e) / a),
        _ => None,
    };

    TrendDto {
        computed_at_bar_ms: bars.last().map_or(0, |b| b.time),
        computed_at_ms: now_ms(),
        timeframe: timeframe.to_string(),
        bar_ms,
        structure: structure_of(bars, FRACTAL_N),
        ema21,
        ema55: last_finite(ema55_series),
        ema21_slope_sign: ema21_series.and_then(|s| slope_sign(s, 3)),
        ema55_slope_sign: ema55_series.and_then(|s| slope_sign(s, 3)),
        atr14,
        dist_ema21_atr,
        adx14: last_finite(at("adx_14.adx")),
        plus_di14: last_finite(at("adx_14.plusDi")),
        minus_di14: last_finite(at("adx_14.minusDi")),
        efficiency_20: efficiency_ratio(bars, 20),
        donchian20: DonchianDto {
            upper: last_finite(at("donchian_20.upper")),
            lower: last_finite(at("donchian_20.lower")),
            bars_since_new_high: bars_since_extreme(bars, 20, true),
            bars_since_new_low: bars_since_extreme(bars, 20, false),
        },
        last_close,
    }
}

/// Same invariant as [`trend_facts`]: `all` is non-empty because `stored_only`
/// refused an empty file, so the zero stamp is unreachable.
fn d1_facts(all: &[Bar]) -> D1Dto {
    let bars = &all[all.len().saturating_sub(D1_BARS)..];
    let last = bars.last();
    // The PRIOR day is the one before the newest CLOSED daily bar. Everything
    // in this file is closed bars only, so the newest bar here is yesterday's
    // and the prior day is the one before it.
    let prior_day = bars.len().checked_sub(2).and_then(|i| bars.get(i));

    let weeks = weeks_of(bars);
    // The last complete week: the newest group is the week in progress, so
    // "prior week" is the one before it. With only one group there is no prior
    // week yet and every week field is null rather than the current week's.
    let prior_week = weeks.len().checked_sub(2).and_then(|i| weeks.get(i)).cloned();
    let (pw_high, pw_low, pw_start) = match prior_week {
        Some(ref r) => {
            let slice = &bars[r.clone()];
            let high = slice.iter().map(|b| b.high).filter(|v| v.is_finite()).fold(f64::NEG_INFINITY, f64::max);
            let low = slice.iter().map(|b| b.low).filter(|v| v.is_finite()).fold(f64::INFINITY, f64::min);
            (high.is_finite().then_some(high), low.is_finite().then_some(low), slice.first().map(|b| b.time))
        }
        None => (None, None, None),
    };
    let pw_mid = match (pw_high, pw_low) {
        (Some(h), Some(l)) => Some((h + l) / 2.0),
        _ => None,
    };
    let last_close = last.map(|b| b.close).filter(|v| v.is_finite());
    let close_pct_of_prior_week_range = match (last_close, pw_high, pw_low) {
        // NOT clamped to 0..100. Above 100 means price has left the prior
        // week's range upward and below 0 downward, which is the most
        // informative thing this number ever says; clamping it would turn a
        // breakout into a ceiling. A zero-width week has no position in it.
        (Some(c), Some(h), Some(l)) if h > l => Some((c - l) / (h - l) * 100.0),
        _ => None,
    };

    D1Dto {
        computed_at_bar_ms: last.map_or(0, |b| b.time),
        computed_at_ms: now_ms(),
        timeframe: "1d".to_string(),
        // The broker's day, not 24 hours: its length varies with the session
        // and the changeover. Published as the nominal period for ageing, and
        // a reader comparing two `computed_at_bar_ms` values should use those
        // rather than assume this.
        bar_ms: 86_400_000,
        prior_day_high: prior_day.map(|b| b.high).filter(|v| v.is_finite()),
        prior_day_low: prior_day.map(|b| b.low).filter(|v| v.is_finite()),
        prior_day_bar_ms: prior_day.map(|b| b.time),
        prior_week_high: pw_high,
        prior_week_low: pw_low,
        prior_week_mid: pw_mid,
        prior_week_start_ms: pw_start,
        close_pct_of_prior_week_range,
        last_close,
    }
}

/// One intraday block, or its reason recorded against its own timeframe.
///
/// The reason is keyed here rather than pushed onto a list, because a list
/// joined into a sentence cannot say which timeframe it is about and the
/// realistic state with three timeframes is a mixed one.
fn intraday(
    state: &AppState,
    market: &str,
    timeframe: &'static str,
    bar_ms: i64,
    missing: &mut Vec<(&'static str, String)>,
) -> (Option<TrendDto>, Option<HtfSourceDto>) {
    match stored_only(state, market, timeframe) {
        Ok((bars, source)) => (Some(trend_facts(&bars, timeframe, bar_ms)), Some(source)),
        Err(why) => {
            missing.push((timeframe, why));
            (None, None)
        }
    }
}

/// `GET /api/paper/htf?market=xauusd`
///
/// 200 whenever the market is known, so a card can say why it has nothing
/// instead of decoding a status code. See [`HtfResponse`].
pub async fn htf(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HtfQuery>,
) -> Result<Json<HtfResponse>, ApiError> {
    let market = query.market;
    // Kept in CALL order — finest first — so the joined `unavailable` sentence
    // reads the way the card is laid out. The map below is ordered by key
    // instead, which is a different and equally deterministic thing.
    let mut missing: Vec<(&'static str, String)> = Vec::new();

    let (h1, h1_source) = intraday(&state, &market, "1h", 3_600_000, &mut missing);
    let (h4, h4_source) = intraday(&state, &market, "4h", 14_400_000, &mut missing);
    let (d1, d1_source) = match stored_only(&state, &market, "1d") {
        Ok((bars, source)) => (Some(d1_facts(&bars)), Some(source)),
        Err(why) => {
            missing.push(("1d", why));
            (None, None)
        }
    };

    Ok(Json(HtfResponse {
        market,
        h1,
        h4,
        d1,
        h1_source,
        h4_source,
        d1_source,
        unavailable: (!missing.is_empty())
            .then(|| missing.iter().map(|(_, why)| why.as_str()).collect::<Vec<_>>().join("; ")),
        unavailable_by_tf: missing
            .into_iter()
            .map(|(tf, why)| (tf.to_string(), why))
            .collect(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct HtfQuery {
    pub market: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 14_400_000;
    const DAY: i64 = 86_400_000;

    /// A bar whose high and low sit either side of `mid` by `half`.
    fn bar(i: i64, mid: f64, half: f64) -> Bar {
        Bar { time: i * H, open: mid, high: mid + half, low: mid - half, close: mid, volume: Some(1.0) }
    }

    /// Bars at the given midpoints, one bar apart, each 1.0 wide.
    fn series(mids: &[f64]) -> Vec<Bar> {
        mids.iter().enumerate().map(|(i, m)| bar(i as i64, *m, 1.0)).collect()
    }

    #[test]
    fn a_fractal_needs_n_bars_on_both_sides() {
        // Peak at index 2, and the two bars after it are what confirm it.
        let bars = series(&[10.0, 11.0, 20.0, 11.0, 10.0]);
        assert_eq!(fractal_swings(&bars, 2, true), vec![(2, 21.0)]);

        // The same peak with only one bar after it is not yet a fractal: a
        // swing that has not been confirmed must not be reported, because the
        // whole point of choosing this rule over a ZigZag is that it does not
        // repaint.
        assert!(fractal_swings(&bars[..4], 2, true).is_empty());
    }

    #[test]
    fn a_flat_top_is_not_a_swing() {
        // Strict on both sides: two equal highs are not a peak, and calling
        // one would invent a structure break out of a pause.
        let bars = series(&[10.0, 20.0, 20.0, 10.0, 10.0]);
        assert!(fractal_swings(&bars, 1, true).is_empty());
    }

    #[test]
    fn higher_highs_and_higher_lows_are_up_and_the_last_low_is_the_break() {
        //                    0     1     2     3     4     5     6     7     8
        let bars = series(&[10.0, 12.0, 9.0, 14.0, 11.0, 16.0, 13.0, 18.0, 15.0]);
        let s = structure_of(&bars, 1);
        assert!(matches!(s.label, Structure::Up), "{:?}", s.label);
        assert_eq!(s.rule, "fractal(1)");
        // Highs at 1, 3, 5, 7; lows at 2, 4, 6. Last high 18+1, prior 16+1.
        assert_eq!(s.last_high.as_ref().map(|x| x.price), Some(19.0));
        assert_eq!(s.prior_high.as_ref().map(|x| x.price), Some(17.0));
        assert_eq!(s.last_low.as_ref().map(|x| x.price), Some(12.0));
        assert_eq!(s.prior_low.as_ref().map(|x| x.price), Some(10.0));
        // An up-structure ends when its last higher low gives way.
        assert_eq!(s.break_level, Some(12.0));
        assert!(matches!(s.break_side, Some(BreakSide::Below)));
    }

    #[test]
    fn lower_highs_and_lower_lows_are_down_and_the_last_high_is_the_break() {
        let bars = series(&[18.0, 16.0, 19.0, 14.0, 17.0, 12.0, 15.0, 10.0, 13.0]);
        let s = structure_of(&bars, 1);
        assert!(matches!(s.label, Structure::Down), "{:?}", s.label);
        assert_eq!(s.break_level, s.last_high.as_ref().map(|x| x.price));
        assert!(matches!(s.break_side, Some(BreakSide::Above)));
    }

    #[test]
    fn a_higher_high_with_a_lower_low_is_a_range_and_has_no_break_level() {
        // Expanding, not trending: the rule needs BOTH legs to agree, and a
        // range has no single price whose break changes the label. Inventing
        // one would be the card claiming a precision the rule does not have.
        let bars = series(&[10.0, 14.0, 9.0, 16.0, 6.0, 18.0, 12.0]);
        let s = structure_of(&bars, 1);
        assert!(matches!(s.label, Structure::Range), "{:?}", s.label);
        assert_eq!(s.break_level, None);
        assert!(s.break_side.is_none());
    }

    #[test]
    fn the_label_is_stamped_with_the_bar_that_confirmed_it_not_the_swing() {
        // The honesty field. The newest swing here is the high at index 5; with
        // n = 2 it became a fractal only at index 7, so the label is as of
        // bar 7 and a reader ageing it from the swing would think it fresher
        // than it is.
        let bars = series(&[10.0, 12.0, 9.0, 14.0, 11.0, 20.0, 13.0, 12.0]);
        let s = structure_of(&bars, 2);
        assert_eq!(s.last_high.as_ref().map(|x| x.bar_ms), Some(5 * H));
        assert_eq!(s.confirmed_at_bar_ms, Some(7 * H), "confirmed two bars after the swing");
    }

    #[test]
    fn efficiency_is_one_on_a_straight_line_and_small_on_churn() {
        let straight = series(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(efficiency_ratio(&straight, 4), Some(1.0));

        // Up one, down one, up one, down one: net zero over a path of four.
        let churn = series(&[1.0, 2.0, 1.0, 2.0, 1.0]);
        assert_eq!(efficiency_ratio(&churn, 4), Some(0.0));

        // A market that did not move has no efficiency. Zero would claim the
        // opposite of one about a perfectly still tape, so it is None.
        let flat = series(&[5.0, 5.0, 5.0, 5.0, 5.0]);
        assert_eq!(efficiency_ratio(&flat, 4), None);

        // Warmup: it needs period + 1 closes to have period moves.
        assert_eq!(efficiency_ratio(&straight, 10), None);
    }

    #[test]
    fn a_slope_of_zero_is_flat_and_not_absent() {
        assert_eq!(slope_sign(&[1.0, 2.0, 3.0, 4.0], 3), Some(1));
        assert_eq!(slope_sign(&[4.0, 3.0, 2.0, 1.0], 3), Some(-1));
        assert_eq!(slope_sign(&[5.0, 1.0, 9.0, 5.0], 3), Some(0), "same value three bars ago is flat");
        assert_eq!(slope_sign(&[1.0, 2.0], 3), None, "too short is absent, which is a different thing");
    }

    #[test]
    fn this_bar_making_the_high_is_zero_bars_since_and_not_null() {
        // The distinction 48's card renders differently: 0 is a measurement.
        let rising = series(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(bars_since_extreme(&rising, 5, true), Some(0));
        assert_eq!(bars_since_extreme(&rising, 5, false), Some(4));

        // Not enough bars to judge the window is absent, not zero.
        assert_eq!(bars_since_extreme(&rising, 9, true), None);

        // A tie goes to the NEWEST bar: a bar that matches the window's high
        // has made that high today.
        let tie = series(&[5.0, 3.0, 5.0]);
        assert_eq!(bars_since_extreme(&tie, 3, true), Some(0));
    }

    #[test]
    fn weeks_split_on_the_weekend_hole_and_not_on_the_calendar() {
        // Five daily bars, a gap, five more. The rule reads the gap because the
        // stamps move with daylight saving - these sit at 21:00 UTC in summer
        // and 22:00 in winter - and any calendar rule would be wrong twice a
        // year at the changeover.
        let mut bars: Vec<Bar> = Vec::new();
        for i in 0..5 {
            bars.push(Bar { time: i * DAY, open: 1.0, high: 2.0, low: 0.5, close: 1.5, volume: None });
        }
        for i in 0..5 {
            bars.push(Bar { time: (i + 8) * DAY, open: 1.0, high: 2.0, low: 0.5, close: 1.5, volume: None });
        }
        let weeks = weeks_of(&bars);
        assert_eq!(weeks, vec![0..5, 5..10]);
    }

    #[test]
    fn a_close_outside_the_prior_week_reads_past_the_ends() {
        // NOT clamped, and this is the test that stops someone clamping it
        // later. Above 100 is a breakout and below 0 is a breakdown, and both
        // are the most informative thing this number ever says.
        let week = |start: i64, hi: f64, lo: f64| -> Vec<Bar> {
            (0..5)
                .map(|i| Bar { time: start + i * DAY, open: lo, high: hi, low: lo, close: lo, volume: None })
                .collect()
        };
        let mut bars = week(0, 110.0, 90.0);
        bars.extend(week(8 * DAY, 200.0, 190.0));
        // Last close is 190, prior week ran 90..110, so it is far above.
        let d1 = d1_facts(&bars);
        assert_eq!(d1.prior_week_high, Some(110.0));
        assert_eq!(d1.prior_week_low, Some(90.0));
        assert_eq!(d1.prior_week_mid, Some(100.0));
        let pct = d1.close_pct_of_prior_week_range.expect("a position");
        assert!(pct > 100.0, "a breakout must read past 100, got {pct}");
        assert!((pct - 500.0).abs() < 1e-9, "(190 - 90) / 20 * 100");
    }

    #[test]
    fn a_week_with_no_range_has_no_position_in_it() {
        // Division guarded rather than silently infinite.
        let flat = |start: i64| -> Vec<Bar> {
            (0..5).map(|i| Bar { time: start + i * DAY, open: 5.0, high: 5.0, low: 5.0, close: 5.0, volume: None }).collect()
        };
        let mut bars = flat(0);
        bars.extend(flat(8 * DAY));
        assert_eq!(d1_facts(&bars).close_pct_of_prior_week_range, None);
    }

    #[test]
    fn one_week_of_data_has_no_prior_week_rather_than_this_one() {
        // The failure that would be invisible: reporting the CURRENT week's
        // high as "prior week" reads as a number and is the wrong one.
        let bars: Vec<Bar> = (0..5)
            .map(|i| Bar { time: i * DAY, open: 1.0, high: 2.0, low: 0.5, close: 1.5, volume: None })
            .collect();
        let d1 = d1_facts(&bars);
        assert_eq!(d1.prior_week_high, None);
        assert_eq!(d1.prior_week_low, None);
        assert_eq!(d1.prior_week_mid, None);
        assert_eq!(d1.close_pct_of_prior_week_range, None);
    }

    /* ------------------------------------------ h1 beside h4, on the route */

    fn state_with(dir: &std::path::Path, files: &[(&str, Vec<Bar>)]) -> Arc<AppState> {
        for (tf, bars) in files {
            let path = dir.join("bars").join(format!("BTCUSDT-{tf}.parquet"));
            fd_store::write_bars(&path, bars).expect("write");
        }
        let config_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config");
        let config = fd_core::config::Config::load(config_dir).expect("config");
        Arc::new(AppState::new(config, dir.to_path_buf()))
    }

    /// Rising bars at `step` apart, each `i` higher than the last.
    fn ladder(step: i64, n: i64, base: f64) -> Vec<Bar> {
        (0..n)
            .map(|i| {
                let mid = base + i as f64;
                Bar { time: i * step, open: mid, high: mid + 1.0, low: mid - 1.0, close: mid, volume: Some(1.0) }
            })
            .collect()
    }

    #[test]
    fn the_two_intraday_blocks_are_the_same_facts_with_their_own_period() {
        // One code path, two labels. Fed the SAME bars, everything but the two
        // fields that say which timeframe it is must come out identical - that
        // is what makes a single type honest rather than merely convenient.
        let bars = ladder(3_600_000, 120, 100.0);
        let h1 = trend_facts(&bars, "1h", 3_600_000);
        let h4 = trend_facts(&bars, "4h", 14_400_000);

        assert_eq!(h1.timeframe, "1h");
        assert_eq!(h1.bar_ms, 3_600_000);
        assert_eq!(h4.timeframe, "4h");
        assert_eq!(h4.bar_ms, 14_400_000);

        assert_eq!(h1.computed_at_bar_ms, h4.computed_at_bar_ms);
        assert_eq!(h1.ema21, h4.ema21);
        assert_eq!(h1.ema55, h4.ema55);
        assert_eq!(h1.atr14, h4.atr14);
        assert_eq!(h1.adx14, h4.adx14);
        assert_eq!(h1.efficiency_20, h4.efficiency_20);
        assert_eq!(h1.last_close, h4.last_close);
        assert_eq!(h1.structure.rule, h4.structure.rule);
        assert_eq!(h1.donchian20.upper, h4.donchian20.upper);
        assert_eq!(h1.donchian20.bars_since_new_high, h4.donchian20.bars_since_new_high);
    }

    #[tokio::test]
    async fn h1_and_h4_are_read_from_different_files() {
        // a5 asked for this one by name, and it is worth having: the failure it
        // catches is `h1` being served from the H4 parquet, which would look
        // completely plausible on a card - a trend label, a real level, a fresh
        // stamp - and would be a statement about four-hour bars sitting in a
        // row captioned H1. The two series are deliberately hundreds of points
        // apart so no value could be mistaken for the other's.
        let dir = tempfile::tempdir().expect("temp dir");
        let state = state_with(
            dir.path(),
            &[
                ("1h", ladder(3_600_000, 300, 100.0)),
                ("4h", ladder(14_400_000, 300, 5_000.0)),
                ("1d", ladder(86_400_000, 40, 9_000.0)),
            ],
        );

        let res = htf(State(state), Query(HtfQuery { market: "btc".to_string() }))
            .await
            .expect("200")
            .0;

        let h1 = res.h1.expect("h1");
        let h4 = res.h4.expect("h4");
        let h1_source = res.h1_source.expect("h1_source");
        let h4_source = res.h4_source.expect("h4_source");

        assert_eq!(h1_source.file, "bars/BTCUSDT-1h.parquet");
        assert_eq!(h4_source.file, "bars/BTCUSDT-4h.parquet");
        assert_ne!(h1_source.file, h4_source.file, "different files");
        assert_eq!(h1_source.timeframe, "1h");
        assert_eq!(h4_source.timeframe, "4h");

        // And the facts followed the files rather than the labels.
        assert_eq!(h1.timeframe, "1h");
        assert_eq!(h4.timeframe, "4h");
        assert!(h1.last_close.expect("close") < 1_000.0, "h1 close came from the 1h ladder");
        assert!(h4.last_close.expect("close") > 4_000.0, "h4 close came from the 4h ladder");
        assert_ne!(h1.computed_at_bar_ms, h4.computed_at_bar_ms, "different last bars");

        assert!(res.unavailable.is_none());
        assert!(res.unavailable_by_tf.is_empty(), "nothing missing, so no keys");
    }

    #[tokio::test]
    async fn a_missing_timeframe_is_named_beside_the_one_that_answered() {
        // The mixed state, which is the state at launch: H4 exported, H1 not.
        // One joined sentence cannot say which row it is about, so the reason is
        // keyed by timeframe and a client puts it under the row it belongs to.
        let dir = tempfile::tempdir().expect("temp dir");
        let state = state_with(dir.path(), &[("4h", ladder(14_400_000, 300, 5_000.0))]);

        let res = htf(State(state), Query(HtfQuery { market: "btc".to_string() }))
            .await
            .expect("200")
            .0;

        assert!(res.h4.is_some(), "H4 answered");
        assert!(res.h1.is_none());
        assert!(res.d1.is_none());

        // A key exists exactly for the blocks that are null, and for no others.
        let keys: Vec<&str> = res.unavailable_by_tf.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["1d", "1h"], "only the absent ones, and not 4h");

        // Each reason names its OWN timeframe and its OWN export flag. The
        // `--timeframes D1` that the old two-armed `if` would have printed into
        // the H1 sentence is the thing this asserts against.
        let h1_why = &res.unavailable_by_tf["1h"];
        assert!(h1_why.contains("BTCUSDT-1h.parquet"), "{h1_why}");
        assert!(h1_why.contains("--timeframes H1"), "names H1, not D1: {h1_why}");
        let d1_why = &res.unavailable_by_tf["1d"];
        assert!(d1_why.contains("--timeframes D1"), "{d1_why}");

        // The joined field still exists with its old meaning, and still cannot
        // say which row it is about - which is why the doc forbids using it as
        // a per-row caption.
        let joined = res.unavailable.expect("a joined sentence");
        assert!(joined.contains(h1_why) && joined.contains(d1_why));
    }

    #[test]
    fn every_timeframe_this_route_serves_has_an_mt5_name() {
        // The refusal sentence is an instruction an operator pastes. A wrong
        // flag in it reads perfectly and does not fix the problem.
        assert_eq!(mt5_name("1h"), "H1");
        assert_eq!(mt5_name("4h"), "H4");
        assert_eq!(mt5_name("1d"), "D1");
    }

    #[test]
    fn the_served_json_carries_h1_beside_h4_at_the_top_level() {
        // Serve it and read the KEY PATHS, because that is the half that was
        // missed this morning: the field names were agreed and right, and the
        // nesting was never stated. -48 parses `h1`, `h1_source` and
        // `unavailable_by_tf` as siblings of `h4`, so this pins where they sit
        // and not merely that they exist.
        let bars = ladder(3_600_000, 120, 100.0);
        let res = HtfResponse {
            market: "xauusd".to_string(),
            h1: Some(trend_facts(&bars, "1h", 3_600_000)),
            h4: None,
            d1: None,
            h1_source: Some(HtfSourceDto {
                file: "bars/XAUUSD-1h.parquet".to_string(),
                bars: 120,
                timeframe: "1h".to_string(),
            }),
            h4_source: None,
            d1_source: None,
            unavailable: Some("nope".to_string()),
            unavailable_by_tf: [("4h".to_string(), "nope".to_string())].into_iter().collect(),
        };
        let v = serde_json::to_value(&res).expect("json");
        let obj = v.as_object().expect("an object");

        for key in ["market", "h1", "h4", "d1", "h1_source", "h4_source", "d1_source", "unavailable", "unavailable_by_tf"] {
            assert!(obj.contains_key(key), "top level is missing {key}");
        }
        assert_eq!(obj.len(), 9, "and nothing else at the top level: {:?}", obj.keys().collect::<Vec<_>>());

        // snake_case, not camelCase. A decorative `rename_all` renamed only the
        // new fields once already.
        assert!(!obj.contains_key("h1Source"));
        assert_eq!(v["h1"]["timeframe"], "1h");
        assert_eq!(v["h1"]["bar_ms"], 3_600_000);
        assert!(v["h1"]["donchian20"]["bars_since_new_high"].is_number());
        assert_eq!(v["h1_source"]["timeframe"], "1h");
        assert_eq!(v["unavailable_by_tf"]["4h"], "nope");
        // Absent means fine, and is absent rather than null.
        assert!(v["unavailable_by_tf"].get("1h").is_none());
        // A null block is PRESENT and null, never missing.
        assert!(v["h4"].is_null());
    }

    #[test]
    fn the_prior_day_is_the_one_before_the_newest_closed_bar() {
        let bars = vec![
            Bar { time: 0, open: 1.0, high: 9.0, low: 1.0, close: 5.0, volume: None },
            Bar { time: DAY, open: 1.0, high: 7.0, low: 2.0, close: 6.0, volume: None },
            Bar { time: 2 * DAY, open: 1.0, high: 8.0, low: 3.0, close: 7.0, volume: None },
        ];
        let d1 = d1_facts(&bars);
        assert_eq!(d1.prior_day_high, Some(7.0), "the middle bar, not the newest");
        assert_eq!(d1.prior_day_low, Some(2.0));
        assert_eq!(d1.prior_day_bar_ms, Some(DAY));
        assert_eq!(d1.last_close, Some(7.0), "the newest closed bar");
    }
}
