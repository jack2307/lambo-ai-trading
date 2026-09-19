//! Price-bar levels: the marks a chart reader points at, as data.
//!
//! Everything else in this crate turns an OPTION tape into levels. This module
//! turns PRICE BARS into them, which is a different input and therefore a
//! different implementation — [`crate::profile`] builds a histogram across
//! strikes from prints, and the one below builds a histogram across price from
//! bars. The value-area walk is the same classic algorithm in both, written
//! twice on purpose: sharing it would mean one function whose two callers
//! disagree about what a "row" is.
//!
//! ## No verdict
//!
//! Nothing here scores, ranks, combines or labels. There is no composite, no
//! confluence count, no "bullish structure" and no ordering by importance.
//! That is a pre-commitment from `docs/hypotheses/2026-09-18-smc-context.md`
//! and not a matter of taste: this desk has closed forty registrations in
//! which levels of exactly this family were tested as mechanical rules and
//! none survived — the ICT chain at PF 0.75-0.77 over four out-of-sample
//! years, prior-day high and low at the 1st-6th percentile of their own
//! session null, twenty-five more on the options tape. A score on one of
//! these levels would smuggle back the claim those registrations refuted,
//! inside a route whose whole purpose is to report facts a model can read.
//!
//! ## Units
//!
//! Every price is in the market's own quote units. Everything that is not a
//! price says what it is in its name: `*_atr` is in ATR(14) of the timeframe
//! the levels were computed on, `*_bars` counts bars of that timeframe,
//! `*_ms` is UTC epoch milliseconds, `*_fraction` is 0..1. This desk spent
//! 2026-09-17 removing five numbers whose units lived only in prose
//! (`docs/decisions/2026-09-17-unit-carrying.md`).
//!
//! ## ATR is passed in, never recomputed here
//!
//! Every threshold in this module is a multiple of ATR(14), and the series is
//! a parameter rather than something this module computes. The route that
//! calls it publishes the same series' last value as `atr14`, so a reader can
//! check every threshold against the denominator the response shows. Two ATRs
//! — one for the thresholds and one for the wire — is a ratio nobody can
//! check, which is the same defect `htf.rs` guards with
//! `the_threshold_uses_the_published_atr`.
//!
//! **So the thresholds scale with the caller's timeframe on their own, and a
//! reader asking whether a 4h order block is measured against a 15m ATR has
//! the answer here: it is not.** Nothing in this module knows what a
//! timeframe is. It is handed a slice of bars and the ATR series OF THOSE
//! BARS, so a displacement over `1 x ATR(14)` on a 4h call is a 4h body over
//! a 4h ATR, the profile bucket is a quarter of that same 4h ATR, and two 4h
//! swing highs are equal within a tenth of it. The only way to get a 15m
//! number into a 4h answer is to pass a mismatched pair, which is why every
//! function takes the bars and the ATR as one argument list rather than
//! keeping either in state — and why `/api/paper/levels` computes exactly one
//! ATR series per response and publishes it (`crates/fd-api/src/levels.rs`).
//!
//! ## Causality
//!
//! Every function here reads CLOSED bars and nothing else, and no level is
//! reported before the bar that made it knowable has closed. A fair value gap
//! needs its third bar; a swing needs its `right` bars; a sweep is looked for
//! only after the bar that confirmed the level it swept. Where that costs
//! something the comment says so rather than hiding it.

use std::ops::Range;

use fd_core::types::Bar;
use serde::Serialize;

/* ------------------------------------------------------------ the shape */

/// Which rule family produced a level. A closed set, so a client can switch
/// on it exhaustively and a new kind is a compile error rather than a silent
/// default branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LevelKind {
    /// The busiest price bucket of the activity profile.
    Poc,
    /// Top of the value area.
    Vah,
    /// Bottom of the value area.
    Val,
    /// A three-bar imbalance that later bars have not closed.
    FairValueGap,
    /// The last opposing candle before a displacement.
    OrderBlock,
    /// Swing highs sitting within the equal-highs tolerance of each other.
    EqualHighs,
    EqualLows,
    PriorDayHigh,
    PriorDayLow,
    PriorWeekHigh,
    PriorWeekLow,
    SessionHigh,
    SessionLow,
    DayHigh,
    DayLow,
    WeekHigh,
    WeekLow,
}

/// What has happened to a level since it formed.
///
/// One enum rather than one per family, because a client rendering a list of
/// levels wants one thing to switch on. Which states a family can be in is
/// fixed and documented here, and the `rule` string on the level says which
/// family it is:
///
/// * profile (POC/VAH/VAL) — [`Self::Current`] only. A profile level is a
///   property of the window, not something price tests.
/// * fair value gaps — [`Self::Unfilled`] or [`Self::PartiallyFilled`]. A
///   fully filled gap is not reported at all.
/// * order blocks — [`Self::Untested`], [`Self::Tested`], [`Self::Broken`].
/// * liquidity — [`Self::Resting`] or [`Self::Swept`].
/// * extremes — [`Self::Forming`] while the period is still running,
///   [`Self::Complete`] once it has ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LevelState {
    Current,
    Forming,
    Complete,
    Unfilled,
    PartiallyFilled,
    Untested,
    Tested,
    Broken,
    Resting,
    Swept,
}

/// Which way a gap or a block faces.
///
/// The name says which side of price the imbalance is on and NOT what price
/// will do next. A `BULLISH` gap is a gap left by an up move; whether price
/// returns to it is exactly the question the closed registrations answered
/// no to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Direction {
    Bullish,
    Bearish,
}

/// Which side of the book a pool of stops sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiquiditySide {
    /// Above price: resting buy stops, taken out by trading THROUGH the highs.
    BuySide,
    /// Below price: resting sell stops.
    SellSide,
}

/// The five things the registration says every level carries, and nothing
/// else.
///
/// `docs/hypotheses/2026-09-18-smc-context.md` lines 63-66: "each carrying a
/// price or a band, the bar it formed on, its age, its state, and the rule
/// that produced it". Written as one struct and flattened into each family's
/// object so that a field cannot be present on gaps and missing on blocks.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PriceLevel {
    pub kind: LevelKind,
    /// The level's single price in the market's quote units, or `null` when
    /// the level is a BAND and has no single price — a gap and an order block
    /// are two prices and reporting a midpoint for them would invent a level
    /// the rule never produced.
    pub price: Option<f64>,
    /// The band's two prices, low first. `null` on a single-price level.
    pub band_low: Option<f64>,
    pub band_high: Option<f64>,
    /// The bar the level formed on, UTC epoch ms. What "formed" means is
    /// fixed per family and stated in `rule`.
    pub formed_at_bar_ms: i64,
    /// Bars of THIS timeframe from the forming bar to the newest closed bar.
    /// `0` means it formed on the newest bar — a measurement, not an absence.
    pub age_bars: usize,
    pub state: LevelState,
    /// The rule that produced it, spelled out with its parameters, so a
    /// reader can argue with the definition rather than guess it. This is the
    /// field the registration asks for by name.
    pub rule: String,
}

/// A three-bar imbalance that later bars have not closed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FairValueGap {
    #[serde(flatten)]
    pub level: PriceLevel,
    pub direction: Direction,
    /// How much of the gap's height later bars have traded back into, 0..1.
    /// Exactly `0.0` means untouched; a gap that reached `1.0` is FILLED and
    /// is not in this list at all.
    pub filled_fraction: f64,
    /// The third bar of the three, which is the bar the gap became knowable
    /// on. `formed_at_bar_ms` is the middle bar — the imbalance itself — and
    /// the two differ by one bar. Both are carried because a reader ageing
    /// the gap wants the first and a reader checking causality wants the
    /// second.
    pub confirmed_at_bar_ms: i64,
}

/// The last opposing candle before a displacement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OrderBlock {
    #[serde(flatten)]
    pub level: PriceLevel,
    pub direction: Direction,
    /// The bar whose body cleared the displacement threshold.
    pub displacement_at_bar_ms: i64,
    /// That body in ATR(14) units AT ITS OWN BAR. Published because it is the
    /// number the threshold was compared against, and a threshold whose
    /// measurement is invisible cannot be checked.
    pub displacement_body_atr: f64,
    /// First bar after the displacement that traded back inside the block, or
    /// `null` while it is untested.
    pub tested_at_bar_ms: Option<i64>,
    /// First bar that CLOSED beyond the block's far edge, or `null`.
    pub broken_at_bar_ms: Option<i64>,
}

/// Swing highs or lows clustered within the equal-highs tolerance, or a
/// period extreme, with whether price has since traded through it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiquidityPool {
    #[serde(flatten)]
    pub level: PriceLevel,
    pub side: LiquiditySide,
    /// Whether a later bar traded beyond the pool. `swept: true` with a
    /// `swept_at_bar_ms` is the pair; neither is ever set without the other.
    pub swept: bool,
    pub swept_at_bar_ms: Option<i64>,
    /// The swings this pool is made of, by [`swing_id`]. Empty for the
    /// prior-day and prior-week pools, which are period extremes and not
    /// swings.
    pub swing_ids: Vec<String>,
    /// How far the cluster's members are spread, in ATR(14) units. `null` for
    /// a pool of one and for the period pools. Always below the tolerance the
    /// `rule` names, which is what makes it checkable.
    pub spread_atr: Option<f64>,
}

/// A time-at-price histogram over price buckets, with its value area.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BarProfile {
    /// `TIME_AT_PRICE`, always, and it is a field rather than a comment so a
    /// reader of the JSON knows what the histogram counts.
    ///
    /// Not volume. The bar feeds here carry `volume: None` on the gold tape
    /// and a tick count of unknown provenance elsewhere — `fd_indicators::vwap`
    /// already documents that it substitutes 1.0 per bar where the feed has
    /// none, which turns a volume-weighted average into a typical-price
    /// average without saying so at the call site. A profile built on that
    /// would be a time profile wearing a volume profile's name, so this one
    /// is a time profile wearing its own.
    pub measure: &'static str,
    pub poc: Option<PriceLevel>,
    pub vah: Option<PriceLevel>,
    pub val: Option<PriceLevel>,
    /// The bucket height in quote units.
    pub bucket_size_price: f64,
    /// How many buckets make one ATR(14). The bucket size is
    /// `atr14 / buckets_per_atr` and both ends are published so the
    /// derivation can be checked rather than believed.
    pub buckets_per_atr: f64,
    /// Buckets spanned by the window's full range.
    pub buckets: usize,
    /// Bars that went into the histogram.
    pub window_bars: usize,
    pub window_start_bar_ms: i64,
    pub window_end_bar_ms: i64,
    /// The share of activity the value area is built to enclose, 0..1.
    pub value_area_pct: f64,
    /// The histogram's total, in bar-buckets: one bar that traded through
    /// four buckets contributes 4.0. The denominator of the value area.
    pub activity_total_bar_buckets: f64,
    /// The part of that total inside VAL..VAH. Divided by the line above it
    /// is at least `value_area_pct`, which is the property
    /// `the_value_area_holds_seventy_percent_of_the_activity` pins.
    pub activity_in_value_area_bar_buckets: f64,
}

/// The high and the low of one period, each as a level.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PeriodExtremes {
    pub high: Option<PriceLevel>,
    pub low: Option<PriceLevel>,
    /// The period's first and last bar, so a reader can see which bars these
    /// are the extremes OF without trusting the label.
    pub start_bar_ms: Option<i64>,
    pub end_bar_ms: Option<i64>,
    pub bars: usize,
}

/* ------------------------------------------------------- shared handles */

/// A stable handle for one swing, so the same swing can be named by two
/// routes.
///
/// `<timeframe>-<side>-<bar_ms>`, e.g. `4h-hi-1757980800000`. Derived
/// entirely from facts the swing already carries, so it is stable across
/// restarts, across recomputations and across the two routes that build it —
/// there is no counter and no registry to keep in sync.
///
/// It exists because `/api/paper/levels` reports buy-side liquidity made of
/// swing highs and `/api/paper/htf` reports the swings its structure rule
/// found, and before this there was no way to say that two of them were the
/// same point: `SwingDto` carried a price and a bar stamp and nothing to join
/// on.
///
/// **The timeframe is part of the id, so ids from two timeframes never
/// compare equal even when they are the same physical high.** That is the
/// honest encoding — a 4h swing high at 08:00 and a 15m swing high at 10:15
/// are different measurements of the market and merging them would be a claim
/// — and a client that wants the looser join can split on `-` and compare the
/// side and the stamp.
#[must_use]
pub fn swing_id(timeframe: &str, high: bool, bar_ms: i64) -> String {
    format!("{timeframe}-{}-{bar_ms}", if high { "hi" } else { "lo" })
}

/* ------------------------------------------------------------- grouping */

/// Maximal runs of bars with no gap longer than `gap_ms` between consecutive
/// bars. Index ranges, oldest first.
///
/// **`htf::weeks_of` is this function with `gap_ms` = two days, and the route
/// calls THAT one for weeks rather than this one.** There is exactly one
/// weekend rule on this desk and it lives in `htf.rs`; this generalisation
/// exists for the DAILY hole, which is a different measured fact: the broker
/// rolls its day at 21:00 UTC in US summer and 22:00 in winter, and hour 21Z
/// holds exactly zero 15m bars against 332-348 in every neighbouring hour
/// (measured 2026-09-18, recorded in `htf.rs`). So a day boundary is a hole of
/// about an hour, and a calendar rule for it would be wrong twice a year at
/// the changeover while the hole is right whatever the clock did.
///
/// **A caller looking for trading DAYS may only use it on bars shorter than
/// that hole.** At 1h the hole is one bar wide, so a 45-minute threshold
/// splits at every bar and a threshold big enough not to cannot tell the day
/// roll from one missing bar; at 4h and 1d the hole is inside a bar and the
/// stamps step evenly straight across it. Above the hole a day has to be
/// counted from the weekend instead — `fd_api::levels::trading_day_runs` does
/// that, and it is documented there rather than here because this function's
/// contract is gaps and nothing else.
#[must_use]
pub fn runs_split_by_gap(bars: &[Bar], gap_ms: i64) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    if bars.is_empty() {
        return out;
    }
    let mut start = 0usize;
    for i in 1..bars.len() {
        if bars[i].time - bars[i - 1].time > gap_ms {
            out.push(start..i);
            start = i;
        }
    }
    out.push(start..bars.len());
    out
}

/* -------------------------------------------------------------- profile */

/// The bucket height for the activity profile, in quote units.
///
/// `atr14 / buckets_per_atr`, and derived from ATR rather than set to a round
/// number of dollars because a fixed bucket means a different resolution in
/// every volatility regime: five dollars is a fine bucket on a quiet day in
/// gold and one bucket for the whole morning on a violent one, and the POC
/// would change meaning without anything in the response saying so.
///
/// `None` when ATR is not finite or not positive — a market that has not
/// moved has no scale to bucket by, and a default here would publish a
/// profile whose resolution nobody chose.
#[must_use]
pub fn bucket_size_price(atr14: f64, buckets_per_atr: f64) -> Option<f64> {
    if !(atr14.is_finite() && atr14 > 0.0) || !(buckets_per_atr.is_finite() && buckets_per_atr > 0.0) {
        return None;
    }
    Some(atr14 / buckets_per_atr)
}

/// A hard ceiling on the histogram's width, so a bucket size that came out
/// absurdly small cannot allocate the process to death. Five thousand buckets
/// at a quarter of an ATR is a window spanning 1,250 ATRs, which no window of
/// days ever is; hitting it means the inputs are wrong and the honest answer
/// is no profile rather than a slow one.
const MAX_BUCKETS: usize = 5_000;

/// The activity profile over price bars, with its point of control and value
/// area.
///
/// **Time at price, not volume.** Each bar adds 1.0 to EVERY bucket its
/// low-to-high range touches, which is the market-profile definition: a wide
/// bar contributes more total activity than a narrow one because it spent its
/// period at more prices. That is a property of the measure and not a
/// weighting chosen here.
///
/// The value area is the classic walk: start at the busiest bucket, take the
/// heavier of the two neighbours repeatedly until `value_area_pct` of the
/// total is enclosed. Same algorithm as [`crate::profile::value_area`] on the
/// options tape, written again because a bucket of price and a strike are not
/// interchangeable rows.
#[must_use]
pub fn activity_profile(bars: &[Bar], bucket_size_price: f64, value_area_pct: f64, buckets_per_atr: f64) -> Option<BarProfile> {
    if bars.is_empty() || !(bucket_size_price.is_finite() && bucket_size_price > 0.0) {
        return None;
    }
    let pct = if value_area_pct.is_finite() { value_area_pct.clamp(0.0, 1.0) } else { return None };

    let min_low = bars.iter().map(|b| b.low).filter(|v| v.is_finite()).fold(f64::INFINITY, f64::min);
    let max_high = bars.iter().map(|b| b.high).filter(|v| v.is_finite()).fold(f64::NEG_INFINITY, f64::max);
    if !min_low.is_finite() || !max_high.is_finite() || max_high < min_low {
        return None;
    }
    let span = ((max_high - min_low) / bucket_size_price).floor();
    if !span.is_finite() || span < 0.0 || span > MAX_BUCKETS as f64 {
        return None;
    }
    let count = span as usize + 1;

    let index_of = |price: f64| -> usize {
        let raw = ((price - min_low) / bucket_size_price).floor();
        (raw.max(0.0) as usize).min(count - 1)
    };

    let mut activity = vec![0.0f64; count];
    // The first bar of the window that traded in each bucket. A bucket that
    // has been busy since Monday and one that filled up this morning are
    // different facts, and this is where the difference shows.
    let mut first_touch = vec![usize::MAX; count];
    let mut window_bars = 0usize;
    for (i, bar) in bars.iter().enumerate() {
        if !(bar.low.is_finite() && bar.high.is_finite()) || bar.high < bar.low {
            continue;
        }
        window_bars += 1;
        for k in index_of(bar.low)..=index_of(bar.high) {
            activity[k] += 1.0;
            if first_touch[k] == usize::MAX {
                first_touch[k] = i;
            }
        }
    }
    let total: f64 = activity.iter().sum();
    if window_bars == 0 || total <= 0.0 {
        return None;
    }

    // Ties go to the LOWER bucket. Deterministic rather than right: two
    // buckets carrying identical activity is a real state on a thin window,
    // and there is no measurement that breaks it — so the rule is stated
    // instead of being an accident of iteration order.
    let mut poc = 0usize;
    for i in 1..count {
        if activity[i] > activity[poc] {
            poc = i;
        }
    }

    let target = total * pct;
    let (mut lo, mut hi) = (poc, poc);
    let mut acc = activity[poc];
    while acc < target && (lo > 0 || hi < count - 1) {
        let below = if lo > 0 { activity[lo - 1] } else { f64::NEG_INFINITY };
        let above = if hi < count - 1 { activity[hi + 1] } else { f64::NEG_INFINITY };
        if above >= below {
            hi += 1;
            acc += activity[hi];
        } else {
            lo -= 1;
            acc += activity[lo];
        }
    }

    let last = bars.len() - 1;
    let edge = |k: usize| min_low + k as f64 * bucket_size_price;
    let stamp = |k: usize| -> (i64, usize) {
        let i = if first_touch[k] == usize::MAX { last } else { first_touch[k] };
        (bars[i].time, last - i)
    };
    let rule = format!(
        "activity profile over {window_bars} bars, time-at-price, bucket = ATR(14)/{buckets_per_atr}, \
         value area {:.0}% by the classic walk",
        pct * 100.0
    );

    let (poc_ms, poc_age) = stamp(poc);
    let (vah_ms, vah_age) = stamp(hi);
    let (val_ms, val_age) = stamp(lo);

    Some(BarProfile {
        measure: "TIME_AT_PRICE",
        poc: Some(PriceLevel {
            kind: LevelKind::Poc,
            // The bucket's MIDPOINT as the single price, with the bucket's own
            // edges as the band. A POC is a bucket and not a price, and a
            // reader drawing one line wants the middle of it while a reader
            // checking the number wants its width.
            price: Some(edge(poc) + bucket_size_price / 2.0),
            band_low: Some(edge(poc)),
            band_high: Some(edge(poc + 1)),
            formed_at_bar_ms: poc_ms,
            age_bars: poc_age,
            state: LevelState::Current,
            rule: rule.clone(),
        }),
        vah: Some(PriceLevel {
            kind: LevelKind::Vah,
            // The OUTER edge of the outermost bucket in the area, so VAL..VAH
            // encloses every bucket it counted. Taking the midpoint would
            // publish a band narrower than the activity it claims to hold.
            price: Some(edge(hi + 1)),
            band_low: None,
            band_high: None,
            formed_at_bar_ms: vah_ms,
            age_bars: vah_age,
            state: LevelState::Current,
            rule: rule.clone(),
        }),
        val: Some(PriceLevel {
            kind: LevelKind::Val,
            price: Some(edge(lo)),
            band_low: None,
            band_high: None,
            formed_at_bar_ms: val_ms,
            age_bars: val_age,
            state: LevelState::Current,
            rule: rule.clone(),
        }),
        bucket_size_price,
        buckets_per_atr,
        buckets: count,
        window_bars,
        window_start_bar_ms: bars[0].time,
        window_end_bar_ms: bars[last].time,
        value_area_pct: pct,
        activity_total_bar_buckets: total,
        activity_in_value_area_bar_buckets: acc,
    })
}

/* ---------------------------------------------------- fair value gaps */

/// Unfilled three-bar imbalances, oldest first.
///
/// A bullish gap is `bars[i-1].high < bars[i+1].low`: the middle bar moved far
/// enough that the two bars either side of it never traded the same prices,
/// and the untraded band between them is the gap. A bearish gap is the mirror,
/// `bars[i-1].low > bars[i+1].high`.
///
/// **Only unfilled gaps are returned**, per the registration. A gap price has
/// traded all the way back through is not a level any more, and reporting it
/// with `filled_fraction: 1.0` would leave the caller to apply the rule this
/// function is the one place for.
///
/// Filling is measured from bar `i+2` onward — the gap is not knowable until
/// bar `i+1` has closed, and the third bar's own range is what DEFINES the
/// gap's edge rather than a retrace into it.
#[must_use]
pub fn unfilled_fair_value_gaps(bars: &[Bar]) -> Vec<FairValueGap> {
    let mut out = Vec::new();
    if bars.len() < 3 {
        return out;
    }
    let last = bars.len() - 1;
    for i in 1..bars.len() - 1 {
        let (before, mid, after) = (&bars[i - 1], &bars[i], &bars[i + 1]);
        if ![before.high, before.low, after.high, after.low].iter().all(|v| v.is_finite()) {
            continue;
        }
        let (direction, low, high) = if before.high < after.low {
            (Direction::Bullish, before.high, after.low)
        } else if before.low > after.high {
            (Direction::Bearish, after.high, before.low)
        } else {
            continue;
        };
        let height = high - low;
        if !(height.is_finite() && height > 0.0) {
            continue;
        }

        // How far into the gap price has come back. A bullish gap is filled
        // from ABOVE — price drops into it — so the deepest low after the
        // third bar is what measures it, and the mirror for a bearish one.
        let rest = &bars[(i + 2).min(bars.len())..];
        let filled = match direction {
            Direction::Bullish => {
                let deepest = rest.iter().map(|b| b.low).filter(|v| v.is_finite()).fold(f64::INFINITY, f64::min);
                if deepest.is_finite() { ((high - deepest) / height).clamp(0.0, 1.0) } else { 0.0 }
            }
            Direction::Bearish => {
                let deepest =
                    rest.iter().map(|b| b.high).filter(|v| v.is_finite()).fold(f64::NEG_INFINITY, f64::max);
                if deepest.is_finite() { ((deepest - low) / height).clamp(0.0, 1.0) } else { 0.0 }
            }
        };
        if filled >= 1.0 {
            continue;
        }

        out.push(FairValueGap {
            level: PriceLevel {
                kind: LevelKind::FairValueGap,
                // A band and no single price: the gap IS the two edges, and a
                // midpoint would be a level the rule never produced.
                price: None,
                band_low: Some(low),
                band_high: Some(high),
                formed_at_bar_ms: mid.time,
                age_bars: last - i,
                state: if filled > 0.0 { LevelState::PartiallyFilled } else { LevelState::Unfilled },
                rule: match direction {
                    Direction::Bullish => "fvg: 3-bar imbalance, bar[i-1].high < bar[i+1].low".to_string(),
                    Direction::Bearish => "fvg: 3-bar imbalance, bar[i-1].low > bar[i+1].high".to_string(),
                },
            },
            direction,
            filled_fraction: filled,
            confirmed_at_bar_ms: after.time,
        });
    }
    out
}

/* ------------------------------------------------------- order blocks */

/// Order blocks: the last opposing candle before each displacement.
///
/// A displacement is a bar whose BODY (`|close - open|`) exceeds
/// `displacement_body_atr` times ATR(14) **at that bar**, not at the newest
/// one. Judging an old bar by today's volatility would make the set of order
/// blocks change every time ATR moved, which is a repaint: a level that was
/// on the chart yesterday would quietly not have been. `htf.rs` makes the
/// same choice for its zigzag threshold and for the same reason.
///
/// The block itself is the last candle before the displacement that closed
/// the other way — down before an up move, up before a down move — and its
/// band is that candle's full low-to-high range.
///
/// State, from the bars after the displacement:
///
/// * `BROKEN` — a bar CLOSED beyond the far edge (below a bullish block's low,
///   above a bearish block's high). Close and not wick, because a wick through
///   a block and back is the thing the block is supposed to describe, and
///   calling that broken would empty the list on every volatile session.
/// * `TESTED` — a bar traded back inside the band without breaking it.
/// * `UNTESTED` — neither.
///
/// One block per opposing candle: several displacements in a row all point
/// back at the same candle, and reporting it three times would make a count
/// of order blocks a count of impulses.
#[must_use]
pub fn order_blocks(bars: &[Bar], atr: &[f64], displacement_body_atr: f64) -> Vec<OrderBlock> {
    let mut out: Vec<OrderBlock> = Vec::new();
    if bars.is_empty() || !(displacement_body_atr.is_finite() && displacement_body_atr > 0.0) {
        return out;
    }
    let last = bars.len() - 1;
    let mut claimed: Vec<usize> = Vec::new();

    for i in 1..bars.len() {
        let bar = &bars[i];
        let Some(a) = atr.get(i).copied().filter(|v| v.is_finite() && *v > 0.0) else { continue };
        let body = bar.close - bar.open;
        if !body.is_finite() || body.abs() <= displacement_body_atr * a {
            continue;
        }
        let up = body > 0.0;
        // The last candle the other way. `close == open` is neither, so a doji
        // is skipped rather than counted as both.
        let Some(j) = (0..i).rev().find(|&j| {
            let b = &bars[j];
            b.close.is_finite() && b.open.is_finite() && if up { b.close < b.open } else { b.close > b.open }
        }) else {
            continue;
        };
        if claimed.contains(&j) {
            continue;
        }
        claimed.push(j);

        let (low, high) = (bars[j].low, bars[j].high);
        if !(low.is_finite() && high.is_finite() && high >= low) {
            continue;
        }

        let mut tested_at = None;
        let mut broken_at = None;
        for after in &bars[(i + 1).min(bars.len())..] {
            let broke = if up { after.close < low } else { after.close > high };
            if broke && broken_at.is_none() {
                broken_at = Some(after.time);
                break;
            }
            let inside = after.low <= high && after.high >= low;
            if inside && tested_at.is_none() {
                tested_at = Some(after.time);
            }
        }
        let state = if broken_at.is_some() {
            LevelState::Broken
        } else if tested_at.is_some() {
            LevelState::Tested
        } else {
            LevelState::Untested
        };

        out.push(OrderBlock {
            level: PriceLevel {
                kind: LevelKind::OrderBlock,
                price: None,
                band_low: Some(low),
                band_high: Some(high),
                formed_at_bar_ms: bars[j].time,
                age_bars: last - j,
                state,
                rule: format!(
                    "order block: last {} candle before a body > {displacement_body_atr} x ATR(14) at its own bar",
                    if up { "down" } else { "up" }
                ),
            },
            // The direction of the IMPULSE, so a bullish block is the down
            // candle an up move left behind.
            direction: if up { Direction::Bullish } else { Direction::Bearish },
            displacement_at_bar_ms: bar.time,
            displacement_body_atr: body.abs() / a,
            tested_at_bar_ms: tested_at,
            broken_at_bar_ms: broken_at,
        });
    }
    // Oldest first, by the block's own bar rather than by the displacement
    // that found it: `claimed` walks displacements forward, and a later
    // displacement can reach further back than an earlier one.
    out.sort_by_key(|b| b.level.formed_at_bar_ms);
    out
}

/* ---------------------------------------------------------- liquidity */

/// One confirmed swing, with the bar it formed on and the bar that confirmed
/// it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfirmedSwing {
    pub at: usize,
    pub confirmed_at: usize,
    pub price: f64,
}

/// Confirmed swing highs (or lows), oldest first.
///
/// Reads `fd_indicators::swing`, which is the crate whose one guarantee is
/// causality (`every_indicator_is_causal`): a swing at bar `k` is known only
/// at `k + right` and never repaints. This turns its step series back into the
/// list of distinct swings, keeping the confirmation bar, because a level
/// nobody could see yet cannot have been swept.
#[must_use]
pub fn confirmed_swings(bars: &[Bar], left: usize, right: usize, high: bool) -> Vec<ConfirmedSwing> {
    let (highs, lows, high_at, low_at) = fd_indicators::swing(bars, left, right);
    let (level, at) = if high { (highs, high_at) } else { (lows, low_at) };
    let mut out: Vec<ConfirmedSwing> = Vec::new();
    for i in 0..bars.len() {
        let (Some(&k), Some(&p)) = (at.get(i), level.get(i)) else { continue };
        if !k.is_finite() || !p.is_finite() {
            continue;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let k = k as usize;
        if out.last().is_some_and(|s| s.at == k) {
            continue;
        }
        out.push(ConfirmedSwing { at: k, confirmed_at: i, price: p });
    }
    out
}

/// Buy-side (equal highs) or sell-side (equal lows) liquidity pools.
///
/// Swings whose prices sit within `tolerance_atr` ATRs of each other are ONE
/// pool: that is what "equal highs" means, and the tolerance is in ATR units
/// rather than dollars so the same rule reads the same way in a quiet week and
/// a violent one. A swing with no neighbour inside the tolerance is a pool of
/// one — the registration asks for equal highs AND for the swings themselves
/// to be referenceable, and dropping singletons would hide most of the chart.
///
/// The tolerance is evaluated against ATR **at the later swing's confirmation
/// bar**, for the same no-repaint reason as the order blocks above.
///
/// A pool is `SWEPT` when a bar after the newest member's CONFIRMATION bar
/// traded beyond the pool's far edge. Using the confirmation bar rather than
/// the swing's own bar costs nothing: by the definition of a fractal swing no
/// bar within `right` bars after a swing high exceeds it, so there is no sweep
/// to miss in the gap.
#[must_use]
pub fn liquidity_pools(
    bars: &[Bar],
    atr: &[f64],
    timeframe: &str,
    left: usize,
    right: usize,
    tolerance_atr: f64,
    high: bool,
) -> Vec<LiquidityPool> {
    let mut out = Vec::new();
    if bars.is_empty() || !(tolerance_atr.is_finite() && tolerance_atr >= 0.0) {
        return out;
    }
    let last = bars.len() - 1;
    let mut swings = confirmed_swings(bars, left, right, high);
    if swings.is_empty() {
        return out;
    }
    // Clustered by PRICE, not by adjacency in time: two highs a week apart at
    // the same price are the same pool of stops, which is the whole point of
    // the construct.
    swings.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal));

    let atr_at = |i: usize| -> Option<f64> {
        atr.get(i).copied().filter(|v| v.is_finite() && *v > 0.0).or_else(|| {
            atr.iter().rev().copied().find(|v| v.is_finite() && *v > 0.0)
        })
    };

    let mut cluster: Vec<ConfirmedSwing> = vec![swings[0]];
    let flush = |cluster: &[ConfirmedSwing], out: &mut Vec<LiquidityPool>| {
        let min = cluster.iter().map(|s| s.price).fold(f64::INFINITY, f64::min);
        let max = cluster.iter().map(|s| s.price).fold(f64::NEG_INFINITY, f64::max);
        if !(min.is_finite() && max.is_finite()) {
            return;
        }
        let first = cluster.iter().map(|s| s.at).min().unwrap_or(0);
        let newest_confirm = cluster.iter().map(|s| s.confirmed_at).max().unwrap_or(0);
        let mean = cluster.iter().map(|s| s.price).sum::<f64>() / cluster.len() as f64;

        let swept_at = bars[(newest_confirm + 1).min(bars.len())..]
            .iter()
            .find(|b| if high { b.high > max } else { b.low < min })
            .map(|b| b.time);
        let spread_atr = if cluster.len() > 1 { atr_at(newest_confirm).map(|a| (max - min) / a) } else { None };

        out.push(LiquidityPool {
            level: PriceLevel {
                kind: if high { LevelKind::EqualHighs } else { LevelKind::EqualLows },
                // The cluster's MEAN as the one price a chart draws, with the
                // members' min and max as the band. A pool of one has a band
                // of zero width, which is honest rather than null: one high is
                // a band that happens to be a point.
                price: Some(mean),
                band_low: Some(min),
                band_high: Some(max),
                // The EARLIEST member's bar: the pool is as old as the first
                // high that made it, and a later equal high joins a level
                // rather than creating one.
                formed_at_bar_ms: bars[first].time,
                age_bars: last - first,
                state: if swept_at.is_some() { LevelState::Swept } else { LevelState::Resting },
                rule: format!(
                    "{} within {tolerance_atr} x ATR(14) of each other, swings by fractal({left},{right})",
                    if high { "equal highs" } else { "equal lows" }
                ),
            },
            side: if high { LiquiditySide::BuySide } else { LiquiditySide::SellSide },
            swept: swept_at.is_some(),
            swept_at_bar_ms: swept_at,
            swing_ids: cluster.iter().map(|s| swing_id(timeframe, high, bars[s.at].time)).collect(),
            spread_atr,
        });
    };

    for s in swings.iter().skip(1) {
        let tol = atr_at(s.confirmed_at).map_or(0.0, |a| a * tolerance_atr);
        let base = cluster.iter().map(|c| c.price).fold(f64::INFINITY, f64::min);
        if (s.price - base).abs() <= tol {
            cluster.push(*s);
        } else {
            flush(&cluster, &mut out);
            cluster = vec![*s];
        }
    }
    flush(&cluster, &mut out);

    out.sort_by_key(|p| p.level.formed_at_bar_ms);
    out
}

/// The high or the low of one closed period as a liquidity pool, with whether
/// price has traded through it since the period ended.
///
/// Prior-day and prior-week extremes are in the registration's liquidity list
/// beside the equal highs and lows, and they are here rather than only in the
/// extremes block because a level with a swept state is a different fact from
/// a level without one. The PRICES are the same on both, deliberately: one
/// number, reported twice under two questions, rather than two numbers a
/// reader has to reconcile.
#[must_use]
pub fn period_pool(bars: &[Bar], period: Range<usize>, high: bool, kind: LevelKind, label: &str) -> Option<LiquidityPool> {
    let slice = bars.get(period.clone())?;
    if slice.is_empty() || bars.is_empty() {
        return None;
    }
    let last = bars.len() - 1;
    let (price, at) = extreme_of(slice, high)?;
    let at = period.start + at;

    let swept_at =
        bars[period.end.min(bars.len())..].iter().find(|b| if high { b.high > price } else { b.low < price }).map(|b| b.time);

    Some(LiquidityPool {
        level: PriceLevel {
            kind,
            price: Some(price),
            band_low: None,
            band_high: None,
            formed_at_bar_ms: bars[at].time,
            age_bars: last - at,
            state: if swept_at.is_some() { LevelState::Swept } else { LevelState::Resting },
            rule: label.to_string(),
        },
        side: if high { LiquiditySide::BuySide } else { LiquiditySide::SellSide },
        swept: swept_at.is_some(),
        swept_at_bar_ms: swept_at,
        // Not swings, so nothing to join against. Empty rather than absent:
        // a client iterating the field must not have to test for null first.
        swing_ids: Vec::new(),
        spread_atr: None,
    })
}

/* ----------------------------------------------------------- extremes */

/// The extreme price of a slice and the index within it that made it.
///
/// `>=` and `<=`, so the NEWEST bar wins a tie — the same rule `htf.rs` uses
/// for `bars_since_extreme`, and for the same reason: a bar that matches the
/// period's high has made the high.
fn extreme_of(slice: &[Bar], high: bool) -> Option<(f64, usize)> {
    let mut best: Option<(f64, usize)> = None;
    for (i, b) in slice.iter().enumerate() {
        let v = if high { b.high } else { b.low };
        if !v.is_finite() {
            continue;
        }
        let better = best.is_none_or(|(p, _)| if high { v >= p } else { v <= p });
        if better {
            best = Some((v, i));
        }
    }
    best
}

/// The high and the low of one period, as levels.
///
/// `complete` is what separates `FORMING` from `COMPLETE`, and it is the
/// caller's fact rather than something this function can see: a run of bars
/// looks the same whether the period that produced it has ended or is still
/// running, and guessing from the newest stamp would call Friday's finished
/// session forming until Sunday.
#[must_use]
pub fn period_extremes(
    bars: &[Bar],
    period: Range<usize>,
    complete: bool,
    high_kind: LevelKind,
    low_kind: LevelKind,
    rule: &str,
) -> PeriodExtremes {
    let empty = PeriodExtremes { high: None, low: None, start_bar_ms: None, end_bar_ms: None, bars: 0 };
    let Some(slice) = bars.get(period.clone()) else { return empty };
    if slice.is_empty() || bars.is_empty() {
        return empty;
    }
    let last = bars.len() - 1;
    let state = if complete { LevelState::Complete } else { LevelState::Forming };
    let build = |kind: LevelKind, high: bool| -> Option<PriceLevel> {
        let (price, i) = extreme_of(slice, high)?;
        let at = period.start + i;
        Some(PriceLevel {
            kind,
            price: Some(price),
            band_low: None,
            band_high: None,
            formed_at_bar_ms: bars[at].time,
            age_bars: last - at,
            state,
            rule: rule.to_string(),
        })
    };
    PeriodExtremes {
        high: build(high_kind, true),
        low: build(low_kind, false),
        start_bar_ms: slice.first().map(|b| b.time),
        end_bar_ms: slice.last().map(|b| b.time),
        bars: slice.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M15: i64 = 900_000;

    /// A bar, spelled out, so every test below reads as a chart rather than as
    /// a constructor call.
    fn bar(i: i64, open: f64, high: f64, low: f64, close: f64) -> Bar {
        Bar { time: i * M15, open, high, low, close, volume: None }
    }

    /// A flat ATR series, so a threshold in ATR units is a threshold in
    /// dollars and the expected answer is obvious by inspection.
    fn flat_atr(n: usize, value: f64) -> Vec<f64> {
        vec![value; n]
    }

    #[test]
    fn a_swing_id_names_its_timeframe_its_side_and_its_bar() {
        assert_eq!(swing_id("4h", true, 1_757_980_800_000), "4h-hi-1757980800000");
        assert_eq!(swing_id("15m", false, 42), "15m-lo-42");
        // Stable: the same swing always produces the same id, which is the
        // only property a join needs.
        assert_eq!(swing_id("15m", false, 42), swing_id("15m", false, 42));
    }

    #[test]
    fn a_filled_gap_is_not_reported() {
        // Bars 0-2 leave a bullish gap between 101 (bar 0's high) and 104
        // (bar 2's low). Bar 4 then trades down to 100, through the whole of
        // it, so there is no untraded band left and nothing to report.
        let bars = vec![
            bar(0, 100.0, 101.0, 99.0, 100.5),
            bar(1, 101.0, 106.0, 101.0, 105.0),
            bar(2, 105.0, 107.0, 104.0, 106.0),
            bar(3, 106.0, 106.5, 105.0, 105.5),
            bar(4, 105.5, 105.5, 100.0, 100.5),
        ];
        assert!(unfilled_fair_value_gaps(&bars).is_empty(), "{:?}", unfilled_fair_value_gaps(&bars));
    }

    #[test]
    fn a_partly_filled_gap_reports_the_fraction() {
        // The same 101..104 gap, three dollars tall, and price comes back to
        // 103 — one dollar of the three, so a third of it.
        let bars = vec![
            bar(0, 100.0, 101.0, 99.0, 100.5),
            bar(1, 101.0, 106.0, 101.0, 105.0),
            bar(2, 105.0, 107.0, 104.0, 106.0),
            bar(3, 106.0, 106.5, 103.0, 104.0),
        ];
        let gaps = unfilled_fair_value_gaps(&bars);
        assert_eq!(gaps.len(), 1, "{gaps:?}");
        let g = &gaps[0];
        assert_eq!(g.direction, Direction::Bullish);
        assert_eq!(g.level.band_low, Some(101.0));
        assert_eq!(g.level.band_high, Some(104.0));
        assert!(g.level.price.is_none(), "a gap is a band and has no single price");
        assert!((g.filled_fraction - 1.0 / 3.0).abs() < 1e-9, "{}", g.filled_fraction);
        assert_eq!(g.level.state, LevelState::PartiallyFilled);
        // The middle bar formed it; the third bar is what made it knowable.
        assert_eq!(g.level.formed_at_bar_ms, M15);
        assert_eq!(g.confirmed_at_bar_ms, 2 * M15);
        assert_eq!(g.level.age_bars, 2, "bars from the imbalance to the newest bar");
    }

    #[test]
    fn an_untouched_gap_is_unfilled_and_a_bearish_one_mirrors_it() {
        let bars = vec![
            bar(0, 100.0, 101.0, 99.0, 99.5),
            bar(1, 99.0, 99.0, 94.0, 95.0),
            bar(2, 95.0, 96.0, 94.0, 95.0),
            bar(3, 95.0, 95.5, 94.5, 95.0),
        ];
        let gaps = unfilled_fair_value_gaps(&bars);
        assert_eq!(gaps.len(), 1, "{gaps:?}");
        assert_eq!(gaps[0].direction, Direction::Bearish);
        assert_eq!(gaps[0].level.band_low, Some(96.0));
        assert_eq!(gaps[0].level.band_high, Some(99.0));
        assert_eq!(gaps[0].filled_fraction, 0.0);
        assert_eq!(gaps[0].level.state, LevelState::Unfilled);
    }

    #[test]
    fn an_order_block_price_closed_through_is_broken() {
        // Bar 1 is the down candle. Bar 2 is a displacement up (a body of 8
        // against an ATR of 1). Bar 4 closes at 93, below the block's low of
        // 96, so the block is BROKEN rather than merely tested.
        let bars = vec![
            bar(0, 100.0, 100.5, 99.5, 100.0),
            bar(1, 100.0, 100.0, 96.0, 97.0),
            bar(2, 97.0, 106.0, 97.0, 105.0),
            bar(3, 105.0, 105.5, 97.0, 98.0),
            bar(4, 98.0, 98.0, 92.0, 93.0),
        ];
        let blocks = order_blocks(&bars, &flat_atr(bars.len(), 1.0), 1.0);
        // TWO blocks, and the second one is the point of writing the count
        // down: bars 3 and 4 are themselves displacements the other way, and
        // they both reach back to bar 2 as their last up candle. One block per
        // opposing candle, so bar 2 appears once and not twice — a count of
        // order blocks must not become a count of impulses.
        assert_eq!(blocks.len(), 2, "{blocks:?}");
        assert_eq!(blocks[1].level.formed_at_bar_ms, 2 * M15);
        assert_eq!(blocks[1].direction, Direction::Bearish);
        let b = &blocks[0];
        assert_eq!(b.direction, Direction::Bullish);
        assert_eq!(b.level.band_low, Some(96.0));
        assert_eq!(b.level.band_high, Some(100.0));
        assert_eq!(b.level.state, LevelState::Broken);
        assert_eq!(b.broken_at_bar_ms, Some(4 * M15));
        assert_eq!(b.displacement_at_bar_ms, 2 * M15);
        assert!((b.displacement_body_atr - 8.0).abs() < 1e-9, "{}", b.displacement_body_atr);
    }

    #[test]
    fn an_order_block_nothing_came_back_to_is_untested() {
        let bars = vec![
            bar(0, 100.0, 100.5, 99.5, 100.0),
            bar(1, 100.0, 100.0, 96.0, 97.0),
            bar(2, 97.0, 106.0, 97.0, 105.0),
            bar(3, 105.0, 107.0, 104.0, 106.0),
            bar(4, 106.0, 108.0, 105.0, 107.0),
        ];
        let blocks = order_blocks(&bars, &flat_atr(bars.len(), 1.0), 1.0);
        assert_eq!(blocks.len(), 1, "{blocks:?}");
        assert_eq!(blocks[0].level.state, LevelState::Untested);
        assert_eq!(blocks[0].tested_at_bar_ms, None);
    }

    #[test]
    fn a_body_under_the_atr_threshold_is_not_a_displacement() {
        // The same shape with an ATR of 20: the 8-dollar body is now 0.4 ATR
        // and nothing here displaced anything. A fixed dollar threshold would
        // have found a block; the whole reason the threshold is in ATR units
        // is that the answer must move with the market's own scale.
        let bars = vec![
            bar(0, 100.0, 100.5, 99.5, 100.0),
            bar(1, 100.0, 100.0, 96.0, 97.0),
            bar(2, 97.0, 106.0, 97.0, 105.0),
            bar(3, 105.0, 105.5, 104.0, 105.0),
        ];
        assert!(order_blocks(&bars, &flat_atr(bars.len(), 20.0), 1.0).is_empty());
    }

    /// Four swing highs: two at 110 and 110.05 (five cents apart) and two far
    /// below, so the clustering question is only about the pair.
    fn equal_high_bars() -> Vec<Bar> {
        let mut bars: Vec<Bar> =
            (0..20).map(|i| bar(i, 100.0, 101.0, 99.0, 100.0)).collect();
        bars[5].high = 110.00;
        bars[12].high = 110.05;
        bars
    }

    #[test]
    fn equal_highs_inside_the_tolerance_are_one_pool_and_outside_it_are_two() {
        let bars = equal_high_bars();
        let atr = flat_atr(bars.len(), 1.0);

        // 0.1 ATR is ten cents and the pair is five cents apart: one pool.
        let together = liquidity_pools(&bars, &atr, "15m", 2, 2, 0.1, true);
        let pair: Vec<_> = together.iter().filter(|p| p.level.band_high.unwrap_or(0.0) > 109.0).collect();
        assert_eq!(pair.len(), 1, "{together:?}");
        assert_eq!(pair[0].swing_ids.len(), 2);
        assert_eq!(pair[0].level.band_low, Some(110.00));
        assert_eq!(pair[0].level.band_high, Some(110.05));
        assert_eq!(pair[0].side, LiquiditySide::BuySide);
        // Named by the shared id scheme, so the same swing can be found on
        // /api/paper/htf.
        assert!(pair[0].swing_ids.contains(&swing_id("15m", true, 5 * M15)), "{:?}", pair[0].swing_ids);

        // 0.01 ATR is one cent and the same pair is five cents apart: two.
        let apart = liquidity_pools(&bars, &atr, "15m", 2, 2, 0.01, true);
        let split: Vec<_> = apart.iter().filter(|p| p.level.band_high.unwrap_or(0.0) > 109.0).collect();
        assert_eq!(split.len(), 2, "{apart:?}");
        assert!(split.iter().all(|p| p.swing_ids.len() == 1));
    }

    #[test]
    fn a_pool_price_traded_through_names_the_bar_that_swept_it() {
        let mut bars = equal_high_bars();
        // A bar well after the second high, trading above both.
        bars[18].high = 112.0;
        let pools = liquidity_pools(&bars, &flat_atr(bars.len(), 1.0), "15m", 2, 2, 0.1, true);
        let pair = pools.iter().find(|p| p.level.band_high.unwrap_or(0.0) > 109.0).expect("the pair");
        assert!(pair.swept);
        assert_eq!(pair.swept_at_bar_ms, Some(18 * M15));
        assert_eq!(pair.level.state, LevelState::Swept);

        // Without that bar nothing took it out and it is still resting.
        let resting = liquidity_pools(&equal_high_bars(), &flat_atr(20, 1.0), "15m", 2, 2, 0.1, true);
        let pair = resting.iter().find(|p| p.level.band_high.unwrap_or(0.0) > 109.0).expect("the pair");
        assert!(!pair.swept);
        assert_eq!(pair.level.state, LevelState::Resting);
    }

    #[test]
    fn the_value_area_holds_seventy_percent_of_the_activity() {
        // Twenty bars, sixteen of them pinned in a one-dollar band and four
        // spread away from it, so the value area has an obvious answer.
        let mut bars: Vec<Bar> = (0..16).map(|i| bar(i, 100.0, 100.4, 100.0, 100.2)).collect();
        bars.push(bar(16, 100.0, 105.0, 100.0, 104.0));
        bars.push(bar(17, 104.0, 108.0, 104.0, 107.0));
        bars.push(bar(18, 107.0, 107.0, 96.0, 97.0));
        bars.push(bar(19, 97.0, 97.0, 94.0, 95.0));

        let profile = activity_profile(&bars, 0.25, 0.7, 4.0).expect("a profile");
        let share = profile.activity_in_value_area_bar_buckets / profile.activity_total_bar_buckets;
        assert!(share >= 0.7, "the area must ENCLOSE 70%, got {share}");
        // And not everything: a walk that swallowed the whole histogram would
        // pass the line above while saying nothing.
        assert!(share < 1.0, "got {share}");
        // The pinned band is the busiest place on the chart.
        let poc = profile.poc.as_ref().and_then(|l| l.price).expect("a poc");
        assert!((100.0..100.5).contains(&poc), "poc {poc}");
        let vah = profile.vah.as_ref().and_then(|l| l.price).expect("vah");
        let val = profile.val.as_ref().and_then(|l| l.price).expect("val");
        assert!(val <= poc && poc <= vah, "val {val} poc {poc} vah {vah}");
        assert_eq!(profile.measure, "TIME_AT_PRICE");
    }

    #[test]
    fn the_bucket_size_follows_atr() {
        // Twice the volatility, twice the bucket: the profile's resolution is
        // a measurement of the market and not a constant somebody typed.
        assert_eq!(bucket_size_price(2.0, 4.0), Some(0.5));
        assert_eq!(bucket_size_price(4.0, 4.0), Some(1.0));
        // A market with no ATR has no scale to bucket by, and a default here
        // would publish a resolution nobody chose.
        assert_eq!(bucket_size_price(0.0, 4.0), None);
        assert_eq!(bucket_size_price(f64::NAN, 4.0), None);

        let bars: Vec<Bar> = (0..20).map(|i| bar(i, 100.0, 100.0 + (i % 5) as f64, 100.0, 100.0)).collect();
        let fine = activity_profile(&bars, bucket_size_price(2.0, 4.0).unwrap(), 0.7, 4.0).expect("fine");
        let coarse = activity_profile(&bars, bucket_size_price(4.0, 4.0).unwrap(), 0.7, 4.0).expect("coarse");
        assert!(fine.buckets > coarse.buckets, "{} vs {}", fine.buckets, coarse.buckets);
        assert_eq!(fine.bucket_size_price, 0.5);
        assert_eq!(coarse.bucket_size_price, 1.0);
        assert!(fine.poc.as_ref().unwrap().rule.contains("ATR(14)/4"), "{:?}", fine.poc);
    }

    #[test]
    fn a_run_is_split_by_the_hole_and_not_by_the_clock() {
        // Four bars, an hour's hole, three more. The stamps never touch a
        // calendar.
        let mut bars: Vec<Bar> = (0..4).map(|i| bar(i, 100.0, 101.0, 99.0, 100.0)).collect();
        bars.extend((8..11).map(|i| bar(i, 100.0, 101.0, 99.0, 100.0)));
        let runs = runs_split_by_gap(&bars, 45 * 60_000);
        assert_eq!(runs, vec![0..4, 4..7]);
        // A hole shorter than the threshold is not a boundary.
        assert_eq!(runs_split_by_gap(&bars, 2 * 3_600_000), vec![0..7]);
    }

    #[test]
    fn a_period_extreme_carries_the_bar_that_made_it_and_its_state() {
        let mut bars: Vec<Bar> = (0..10).map(|i| bar(i, 100.0, 101.0, 99.0, 100.0)).collect();
        bars[3].high = 120.0;
        bars[6].low = 80.0;
        let done = period_extremes(&bars, 0..8, true, LevelKind::DayHigh, LevelKind::DayLow, "the prior day");
        assert_eq!(done.high.as_ref().unwrap().price, Some(120.0));
        assert_eq!(done.high.as_ref().unwrap().formed_at_bar_ms, 3 * M15);
        assert_eq!(done.high.as_ref().unwrap().age_bars, 6);
        assert_eq!(done.low.as_ref().unwrap().price, Some(80.0));
        assert_eq!(done.high.as_ref().unwrap().state, LevelState::Complete);

        let running = period_extremes(&bars, 8..10, false, LevelKind::SessionHigh, LevelKind::SessionLow, "today");
        assert_eq!(running.high.as_ref().unwrap().state, LevelState::Forming);
        assert_eq!(running.bars, 2);
    }

    #[test]
    fn a_period_pool_is_swept_only_by_bars_after_the_period_ended() {
        let mut bars: Vec<Bar> = (0..10).map(|i| bar(i, 100.0, 101.0, 99.0, 100.0)).collect();
        bars[3].high = 120.0;
        // Nothing after bar 7 goes above 120, so the prior period's high rests.
        let resting =
            period_pool(&bars, 0..8, true, LevelKind::PriorDayHigh, "prior day high").expect("a pool");
        assert!(!resting.swept);
        assert!(resting.swing_ids.is_empty(), "a period extreme is not a swing");

        bars[9].high = 121.0;
        let swept = period_pool(&bars, 0..8, true, LevelKind::PriorDayHigh, "prior day high").expect("a pool");
        assert!(swept.swept);
        assert_eq!(swept.swept_at_bar_ms, Some(9 * M15));
    }

    #[test]
    fn an_empty_window_produces_nothing_rather_than_a_panic() {
        assert!(activity_profile(&[], 1.0, 0.7, 4.0).is_none());
        assert!(unfilled_fair_value_gaps(&[]).is_empty());
        assert!(order_blocks(&[], &[], 1.0).is_empty());
        assert!(liquidity_pools(&[], &[], "15m", 2, 2, 0.1, true).is_empty());
        assert!(runs_split_by_gap(&[], 1).is_empty());
        assert_eq!(period_extremes(&[], 0..0, true, LevelKind::DayHigh, LevelKind::DayLow, "x").bars, 0);
    }
}
