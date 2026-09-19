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
//! ## This whole family was tested as a mechanical rule and it lost
//!
//! Market structure, the dealing range, order blocks, breakers and the gaps
//! below are the pieces of the ICT/SMC chain, and this desk did not merely
//! decline to trade it — it TRADED it and kept the receipt.
//! `docs/hypotheses/2026-09-13-ict-sweep-mss-fvg.md`: higher-timeframe gap →
//! liquidity sweep → a displacement close through structure → a retrace into
//! the impulse's gap, entered with the stop beyond the sweep. It passed its
//! in-sample gate on the only three months of broker minutes available, and
//! then, out of sample on four years of Dukascopy gold minutes: **1,428
//! trades, profit factor 0.746, expectancy −0.190R, sitting at the 95th
//! percentile of a matched null whose own p95 is 0.746.** Closed.
//!
//! So a `BOS` here is an event that happened and not a reason to be long.
//! The functions below describe; what any of it means is the reader's, and
//! the reader has the number above to hold that meaning against. The same
//! sentence is on the wire — `/api/paper/levels` publishes it as
//! `tested_as_a_rule`, because a reader of the JSON never opens this file.
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
//! One `*_fraction` is deliberately outside 0..1 and says so where it is
//! defined: [`DealingRange::close_fraction_of_range`], which is UNCLAMPED for
//! the reason `htf::D1Dto::close_pct_of_prior_week_range` is — a close above
//! the range's high is a fact, and clamping it at 1.0 would turn a breakout
//! into a ceiling.
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
/// * order blocks — [`Self::Untested`], [`Self::Tested`], [`Self::Broken`],
///   [`Self::Breaker`]. Four since 2026-09-19: a breaker is the fourth state
///   of the same block and NOT a second list, so a reader counting blocks
///   does not have to add two lists together.
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
    /// A block that was [`Self::Broken`] and then traded back into from the
    /// other side. The follow-on state of the same object, so a block is
    /// never counted twice; the bar that broke it and the bar that came back
    /// are both on the block ([`OrderBlock::broken_at_bar_ms`],
    /// [`OrderBlock::breaker_retested_at_bar_ms`]).
    Breaker,
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
    /// First bar AFTER the break that traded back into the band — what turns
    /// a broken block into a BREAKER. `null` on every other state.
    ///
    /// A separate field from [`Self::tested_at_bar_ms`] and not the same
    /// question asked twice: that one is price coming back to a block that
    /// still holds, this one is price coming back to a block that did not.
    /// Both stamps stay on the object when it becomes a breaker, so the whole
    /// history — the displacement, the test, the break, the return — is
    /// readable off one level.
    pub breaker_retested_at_bar_ms: Option<i64>,
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
/// * `BREAKER` — broken, and then a later bar traded back INTO the band. The
///   break was a close beyond the far edge, so price was outside on that
///   side, so any return to the band is a return from the other side: that
///   is the whole definition and it needs no extra test. A follow-on state
///   of the same block rather than a second list, so a count of order blocks
///   stays a count of order blocks.
///
///   **And it describes almost every broken block, which is worth knowing
///   before anybody builds on the word.** On the ten trading days of XAUUSD
///   15m in `docs/api-samples/paper-levels.json`, 57 of the 62 blocks that
///   had broken were then traded back into: 92%. On this tape "breaker" is
///   nearly a synonym for "broken", not a rare configuration, and a reader
///   treating one as a find is reading a property of gold's mean reversion
///   at 15m rather than a property of the block.
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
        let mut breaker_at = None;
        // The walk no longer stops at the break: a broken block has one more
        // thing that can happen to it. Before the break it is looking for a
        // test and for the break; after it, for the return that makes it a
        // breaker, and then it stops because a second return is the same
        // fact told twice.
        for after in &bars[(i + 1).min(bars.len())..] {
            let inside = after.low <= high && after.high >= low;
            if broken_at.is_none() {
                let broke = if up { after.close < low } else { after.close > high };
                if broke {
                    broken_at = Some(after.time);
                    continue;
                }
                if inside && tested_at.is_none() {
                    tested_at = Some(after.time);
                }
            } else if inside {
                breaker_at = Some(after.time);
                break;
            }
        }
        let state = if breaker_at.is_some() {
            LevelState::Breaker
        } else if broken_at.is_some() {
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
                    "order block: last {} candle before a body > {displacement_body_atr} x ATR(14) at its own bar{}",
                    if up { "down" } else { "up" },
                    // The breaker clause is appended only to the blocks that
                    // ARE breakers, so a reader who has one in front of them
                    // reads the rule that produced its state rather than a
                    // paragraph about states it is not in.
                    if breaker_at.is_some() {
                        "; BREAKER: a bar CLOSED beyond the far edge and a later bar traded back into the band"
                    } else {
                        ""
                    }
                ),
            },
            // The direction of the IMPULSE, so a bullish block is the down
            // candle an up move left behind.
            direction: if up { Direction::Bullish } else { Direction::Bearish },
            displacement_at_bar_ms: bar.time,
            displacement_body_atr: body.abs() / a,
            tested_at_bar_ms: tested_at,
            broken_at_bar_ms: broken_at,
            breaker_retested_at_bar_ms: breaker_at,
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
///
/// **WHICH fractal rule, because this repository has two.**
/// `fd_indicators::swing` is STRICT on both sides — a bar that TIES with a
/// neighbour is not a swing, and its own source says so
/// (`b.high < candidate.high` for every neighbour). It is the same rule
/// `fd_api::htf::fractal_swings` uses, so the swings behind a structure
/// event here and the swings behind an H4 structure row there are the same
/// kind of object. It is NOT `py/research/bias_defs.py::fractal_structure`,
/// which admits a FLAT TOP: a bar beaten by no neighbour that strictly beats
/// at least one.
///
/// On the 2,000 H1 bars the 2026-09-18 study measured the two agree exactly
/// — 272 swing highs, 278 lows, not one label differing — because an exact
/// tie never occurs in that slice at two decimals. **That is a fact about
/// those bars and not an equivalence.** A measured table produced with one
/// rule describes a slightly different object from a marker produced with
/// the other, and the only defence against nobody noticing when they diverge
/// is that both files name their rule out loud. The `rule` strings on the
/// wire say it too, for the reader who has neither file open.
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

/* --------------------------------------------------- market structure */

/// Which way the structure reads: the SPINE the other families hang off.
///
/// The same three words `/api/paper/htf` publishes on its structure rows and
/// the same comparison behind them — `UP` needs a higher high AND a higher
/// low, `DOWN` a lower high and a lower low, anything else is `RANGE`. It is
/// a second copy of that comparison rather than a call into `htf.rs` for a
/// reason of layering and not of taste: `htf` lives in `fd-api`, which
/// depends on this crate, so this crate cannot reach it. What must not happen
/// is a THIRD definition, and the guard against that is that the comparison
/// below is the one written in
/// `docs/decisions/2026-09-18-market-bias-definitions.md` and measured there.
///
/// **What the study measured it doing, on 25,708 H1 bars:** `fractal(2)`
/// carries a median lag of 12 bars, misses 23% of turns entirely, and has 1%
/// of its labels undone within three bars. Slow and sticky — which is what it
/// was chosen for on the H4 row and is worth knowing here, because a label
/// that lags twelve bars is a label about where price HAS been.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StructureLabel {
    Up,
    Down,
    Range,
}

/// A break of structure or a change of character.
///
/// Both are one bar CLOSING beyond one confirmed swing. The only difference
/// is which way the structure was already pointing when it happened, which is
/// why they are one enum on one event and not two lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StructureEventKind {
    /// Continuation: a close beyond the last confirmed swing high while the
    /// structure was already `UP`, or beyond the last confirmed swing low
    /// while it was already `DOWN`.
    Bos,
    /// The first crack: a close beyond the last confirmed swing on the side
    /// OPPOSITE to the prevailing structure. It is the event that flips the
    /// label, and the next break in the new direction is a `BOS`.
    Choch,
}

/// One close through one confirmed swing.
///
/// Not a [`PriceLevel`]: a level is a price price can come back to and an
/// event is something that happened at a bar. Forcing it into the level
/// shape would have needed two more [`LevelState`] variants meaning "an
/// event", and a client switching on state to draw a band would have drawn
/// one across a moment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructureEvent {
    pub kind: StructureEventKind,
    /// Which side the close broke: `BULLISH` through a swing HIGH, `BEARISH`
    /// through a swing LOW. As everywhere else in this module the name says
    /// where the event was and NOT what price will do next.
    pub direction: Direction,
    /// The swing price the close went beyond, in quote units.
    pub broke_price: f64,
    /// The swing it broke, by [`swing_id`], so the event can be joined to the
    /// liquidity pool built out of the same swing and to `/api/paper/htf`.
    pub broke_swing_id: String,
    /// That swing's own bar. The id contains it; it is a field as well
    /// because a reader dating the event on a chart should not have to parse
    /// an id to do it.
    pub broke_swing_bar_ms: i64,
    /// The bar whose CLOSE did it — the event's own bar.
    pub closed_at_bar_ms: i64,
    /// That close, in quote units, so the break can be checked against
    /// `broke_price` without fetching the bar.
    pub close: f64,
    /// Bars of this timeframe from `closed_at_bar_ms` to the newest closed
    /// bar. `0` means it happened on the newest bar.
    pub age_bars: usize,
    /// Whether a LATER event has happened since. The newest event on the list
    /// is the only one that is not superseded; it is a fact about position in
    /// the list and not a judgement about importance.
    pub superseded: bool,
    /// The label the structure carried AFTER this event: unchanged by a
    /// `BOS`, flipped by a `CHOCH`. Published so the state machine can be
    /// replayed off the response instead of trusted.
    pub structure_after: StructureLabel,
    pub rule: String,
}

/// What the 2026-09-19 measurement found these markers doing, so a reader
/// meets the numbers in the same object as the markers.
///
/// Same idea as `htf::RuleMeasuredDto` and the same reason: a rule published
/// without its measured behaviour is a stronger claim than the measurement
/// supports. Static, from
/// `docs/decisions/2026-09-19-smc-structure-measured.md`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct StructureMeasured {
    pub source: &'static str,
    /// Bars from a 4 x ATR(14) turn to the CHoCH that marks it, median. It
    /// is FASTER than the fractal label below, which is the one thing these
    /// markers have going for them.
    pub choch_median_lag_bars: f64,
    /// And the tail of the same distribution, which is where the speed goes.
    pub choch_p90_lag_bars: f64,
    /// The `fractal(2)` LABEL's median lag at the same turns, for scale.
    pub fractal_label_median_lag_bars: f64,
    /// The live ATR-zigzag's p90 at the same turns — under half the CHoCH's.
    pub zigzag_p90_lag_bars: f64,
    /// Share of those turns with no CHoCH at all. Half of them.
    pub choch_absent_at_turns_pct: f64,
    /// The zigzag's own miss rate on the same turns, for the same scale.
    pub zigzag_missed_turns_pct: f64,
    /// Share of events where the old direction broke back through within ten
    /// bars.
    pub broken_back_within_10_bars_pct: f64,
    pub note: &'static str,
}

/// The one instance. ASCII only, like every other string this route writes
/// to be displayed.
/// FROM THE FULL EXPORT, NOT THE FIXTURE. The first version of the note was
/// measured on the 2,000-bar fixture, because a worktree has no `data/`; it
/// was re-run the same day on the real 25,722-bar H1 export and four of these
/// seven numbers moved. The one that matters moved furthest: the zigzag row
/// missed 13% of 4xATR turns on the fixture and 1% on the full file, so a
/// CHoCH being absent at half of them is not a worse score on one scale -
/// the zigzag sees essentially every turn and this event does not.
const STRUCTURE_MEASURED: StructureMeasured = StructureMeasured {
    source: "docs/decisions/2026-09-19-smc-structure-measured.md",
    choch_median_lag_bars: 7.0,
    choch_p90_lag_bars: 37.0,
    fractal_label_median_lag_bars: 12.0,
    zigzag_p90_lag_bars: 15.0,
    choch_absent_at_turns_pct: 47.0,
    zigzag_missed_turns_pct: 1.0,
    broken_back_within_10_bars_pct: 29.0,
    note: "Measured on 25,722 H1 bars. A CHoCH marks a 4xATR turn a median 7 bars after it, against 12 for the fractal(2) label - but it is absent at 47% of those turns, where the zigzag row misses 1%, and its p90 lag is 37 bars against the zigzag's 15. Within 10 bars the old direction breaks back through 29% of the time. On bigger turns it gets WORSE, alone among the three: at 8xATR its median lag doubles to 17 bars while the other two improve. What follows a BOS or a CHoCH is indistinguishable from what follows any bar - the gap to baseline is 0.2 to 0.5 standard errors, and H4's sign is negative where H1's is positive. Its absence is not evidence that nothing turned. These markers PUNCTUATE a structure row; they do not replace one, and anything on screen that lets them look like a faster structure label will mislead."
};

/// The structure label as of the newest closed bar, and every event that got
/// it there.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarketStructure {
    pub label: StructureLabel,
    /// The bar the label last CHANGED on — the seeding bar, or the `CHOCH`
    /// that flipped it. `null` while the label is still `RANGE`, which means
    /// no swing pair has yet compared and no close has yet broken one.
    pub label_since_bar_ms: Option<i64>,
    pub rule: String,
    /// Oldest first, like every other list on this route. Not by size, not by
    /// recency of importance: those are rankings.
    pub events: Vec<StructureEvent>,
    /// Closes that broke a confirmed swing while the label was `RANGE` and
    /// therefore produced NO event.
    ///
    /// **This number exists because the silence can be large and would
    /// otherwise be invisible.** On the slice measured on 2026-09-19, 43% of
    /// 204 level breaks landed on a bar whose prevailing structure was flat
    /// — 204 breaks, 117 classified — and a reader seeing a short event list
    /// with nothing beside it would conclude the tape was quiet.
    ///
    /// **It will usually be small HERE, and that is a difference of
    /// definition rather than of tape.** That study recomputes the swing
    /// comparison at every bar; this route seeds the label once and then
    /// carries it by closes, so once seeded it is never `RANGE` again (see
    /// [`market_structure`]). On the ten trading days in
    /// `docs/api-samples/paper-levels.json` this field reads 1 against 81
    /// breaks. The two numbers answer different questions and a reader
    /// putting them in one sentence is comparing two objects.
    pub unclassified_breaks: usize,
    pub measured: StructureMeasured,
}

/// Market structure: the confirmed swings, walked forward, and the closes
/// that broke them.
///
/// **One swing rule, the route's own.** The swings are
/// [`confirmed_swings`]`(left, right)` — the same `fractal(2,2)` the liquidity
/// pools are built from, so an event and a pool can name the same swing by
/// the same [`swing_id`]. There is no second swing rule here.
///
/// **Close-based, never wick-based.** A wick through a swing and back is the
/// thing a swing is supposed to describe, exactly as it is for
/// [`order_blocks`]'s `BROKEN`, and the reason is written there: a
/// wick-based rule empties the chart of structure on every volatile session.
///
/// **How the label moves.** It is `RANGE` until two swing highs and two swing
/// lows have been confirmed, at which point the higher-high-and-higher-low
/// comparison seeds it. From then on the CLOSES carry it: a `BOS` leaves it
/// where it is and a `CHOCH` flips it. The seed is the same comparison
/// `/api/paper/htf` makes; the carry is the SMC definition, and the two are
/// separated here because a reader has to know which part is which. Letting
/// the swing comparison keep overriding would mean the close after a `CHOCH`
/// produced a second `CHOCH` instead of the `BOS` the definition asks for —
/// the swing pair is still the old shape at that moment, which is precisely
/// why the change of character is worth a name.
///
/// **A level is SPENT once broken.** The FIRST close beyond a swing is the
/// event and there is never a second one, because price holding above an old
/// swing high for forty bars would otherwise print forty `BOS` and a count
/// of events would be a count of bars. The 2026-09-19 measurement pinned the
/// same rule as a definition choice; this is that rule and not a second one.
///
/// **A break while the label is `RANGE` yields NO event, and is counted.**
/// There is no prevailing structure for it to continue or to crack, so it is
/// neither, and the swing is spent all the same. It is published as
/// [`MarketStructure::unclassified_breaks`] rather than left to look like a
/// quiet tape.
///
/// **How often this route is silent is NOT the measurement's 43%, and the
/// difference is the definition and not the tape.** The 2026-09-19 study
/// recomputes the swing comparison at EVERY bar, so its label is flat 43% of
/// the time and 87 of its 204 breaks go unclassified. This route seeds once
/// and then carries the label by closes, so after the seed it is never
/// `RANGE` again: on the ten trading days in
/// `docs/api-samples/paper-levels.json` that is ONE unclassified break out of
/// 81. The carry is what the brief for this route asked for and it is what
/// makes the close after a CHoCH a `BOS` in the new direction rather than a
/// second CHoCH — but it means the study's flat-rate row and this field are
/// not the same quantity, and neither is comparable to the other without
/// this paragraph. Both are stated where they are produced, which is the
/// same defence the two fractal rules get in [`confirmed_swings`].
///
/// **And the markers are not rare.** On those same ten days: 80 events, 42
/// of them CHoCH, so the label flipped about every twenty bars. A CHoCH is
/// punctuation on this timeframe, not an announcement.
///
/// **Causality is free here and it is worth saying why.** A `fractal(l,r)`
/// swing high is only a swing because the `r` bars after it have LOWER highs,
/// so the bar that confirms it cannot itself close above it. Admitting a
/// swing at its confirmation bar and testing that same bar's close therefore
/// cannot invent an event out of a bar the trader had not seen.
#[must_use]
pub fn market_structure(bars: &[Bar], timeframe: &str, left: usize, right: usize) -> MarketStructure {
    let rule = format!(
        "market structure on swings by fractal({left},{right}), STRICT on both sides (fd_indicators::swing, \
         the same rule /api/paper/htf uses - a flat top is not a swing, unlike bias_defs.py's variant): the \
         label is seeded by the same higher-high-and-higher-low comparison the fractal rule there makes, \
         then carried by CLOSES - a BOS keeps it, a CHoCH flips it. Close-based, never wick-based. A swing \
         is SPENT by the first close beyond it, and a break while the label is RANGE is no event at all \
         and is counted in unclassified_breaks"
    );
    let mut out = MarketStructure {
        label: StructureLabel::Range,
        label_since_bar_ms: None,
        rule,
        events: Vec::new(),
        unclassified_breaks: 0,
        measured: STRUCTURE_MEASURED,
    };
    if bars.is_empty() {
        return out;
    }
    let last = bars.len() - 1;
    let highs = confirmed_swings(bars, left, right, true);
    let lows = confirmed_swings(bars, left, right, false);

    let (mut hi_cursor, mut lo_cursor) = (0usize, 0usize);
    let (mut last_high, mut prior_high): (Option<ConfirmedSwing>, Option<ConfirmedSwing>) = (None, None);
    let (mut last_low, mut prior_low): (Option<ConfirmedSwing>, Option<ConfirmedSwing>) = (None, None);
    // The swing each side has already spent on an event, by its own bar
    // index. A swing produces at most one.
    let (mut spent_high, mut spent_low): (Option<usize>, Option<usize>) = (None, None);

    for (i, bar) in bars.iter().enumerate() {
        while hi_cursor < highs.len() && highs[hi_cursor].confirmed_at <= i {
            prior_high = last_high;
            last_high = Some(highs[hi_cursor]);
            hi_cursor += 1;
        }
        while lo_cursor < lows.len() && lows[lo_cursor].confirmed_at <= i {
            prior_low = last_low;
            last_low = Some(lows[lo_cursor]);
            lo_cursor += 1;
        }

        if out.label == StructureLabel::Range {
            let higher_high = matches!((last_high, prior_high), (Some(a), Some(b)) if a.price > b.price);
            let higher_low = matches!((last_low, prior_low), (Some(a), Some(b)) if a.price > b.price);
            let lower_high = matches!((last_high, prior_high), (Some(a), Some(b)) if a.price < b.price);
            let lower_low = matches!((last_low, prior_low), (Some(a), Some(b)) if a.price < b.price);
            let seeded = if higher_high && higher_low {
                StructureLabel::Up
            } else if lower_high && lower_low {
                StructureLabel::Down
            } else {
                StructureLabel::Range
            };
            if seeded != StructureLabel::Range {
                out.label = seeded;
                out.label_since_bar_ms = Some(bar.time);
            }
        }

        let close = bar.close;
        if !close.is_finite() {
            continue;
        }

        // The high side first, then the low side, and at most one event per
        // bar. Both can only be true at once when the last confirmed swing
        // low sits ABOVE the last confirmed swing high — a whipsaw, rare and
        // real — and the order is stated here rather than left to whichever
        // branch happened to be written first.
        let broke_high = last_high.filter(|s| Some(s.at) != spent_high && close > s.price);
        let broke_low = last_low.filter(|s| Some(s.at) != spent_low && close < s.price);
        let (swing, high) = match (broke_high, broke_low) {
            (Some(s), _) => (s, true),
            (None, Some(s)) => (s, false),
            (None, None) => continue,
        };
        // SPENT by the first close beyond it, whichever way that close is
        // classified — including not at all. A swing that stayed spendable
        // through a flat stretch would produce a BOS naming a price broken
        // twenty bars earlier, dated to a bar that broke nothing.
        if high {
            spent_high = Some(swing.at);
        } else {
            spent_low = Some(swing.at);
        }
        let kind = match (out.label, high) {
            (StructureLabel::Up, true) | (StructureLabel::Down, false) => StructureEventKind::Bos,
            (StructureLabel::Up, false) | (StructureLabel::Down, true) => StructureEventKind::Choch,
            // No prevailing structure: nothing to continue and nothing to
            // crack, so it is neither — counted, never guessed at.
            (StructureLabel::Range, _) => {
                out.unclassified_breaks += 1;
                continue;
            }
        };
        if kind == StructureEventKind::Choch {
            out.label = if high { StructureLabel::Up } else { StructureLabel::Down };
            out.label_since_bar_ms = Some(bar.time);
        }

        out.events.push(StructureEvent {
            kind,
            direction: if high { Direction::Bullish } else { Direction::Bearish },
            broke_price: swing.price,
            broke_swing_id: swing_id(timeframe, high, bars[swing.at].time),
            broke_swing_bar_ms: bars[swing.at].time,
            closed_at_bar_ms: bar.time,
            close,
            age_bars: last - i,
            // Fixed below, once it is known whether anything came after.
            superseded: false,
            structure_after: out.label,
            rule: match kind {
                StructureEventKind::Bos => format!(
                    "BOS: a bar CLOSED beyond the last confirmed swing {} while the structure was \
                     already {} - continuation; swings by fractal({left},{right}), close-based",
                    if high { "high" } else { "low" },
                    if high { "UP" } else { "DOWN" }
                ),
                StructureEventKind::Choch => format!(
                    "CHoCH: the first close beyond the last confirmed swing {} while the structure \
                     was {} - the opposite side, so the label flips here; swings by \
                     fractal({left},{right}), close-based",
                    if high { "high" } else { "low" },
                    if high { "DOWN" } else { "UP" }
                ),
            },
        });
    }

    // Everything but the newest has been superseded. One pass at the end
    // rather than a rewrite as each event lands, because "superseded" is a
    // fact about the finished list.
    let n = out.events.len();
    for (k, event) in out.events.iter_mut().enumerate() {
        event.superseded = k + 1 < n;
    }
    out
}

/* ----------------------------------------------------- dealing range */

/// Where the last close sits relative to the range's midpoint.
///
/// A restatement of [`DealingRange::close_fraction_of_range`] in SMC's own
/// vocabulary and nothing more: above the midpoint is `PREMIUM`, below it is
/// `DISCOUNT`, exactly on it is `EQUILIBRIUM`. It is not advice — the same
/// half of a range is where one reader sells and another buys — and the
/// fraction beside it is what the word was computed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RangeZone {
    Premium,
    Discount,
    Equilibrium,
}

/// The last confirmed swing high to the last confirmed swing low, with its
/// midpoint and where the last close sits in it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DealingRange {
    /// Quote units, all three.
    pub high: f64,
    pub low: f64,
    /// The midpoint, `(high + low) / 2`. The level itself and not a statistic
    /// about the range, which is the same thing `htf`'s `prior_week_mid` is.
    pub equilibrium: f64,
    /// Which leg the range came from: the two swings by [`swing_id`], so the
    /// range can be joined to the pools and the structure events built on the
    /// same two swings.
    pub high_swing_id: String,
    pub low_swing_id: String,
    pub high_bar_ms: i64,
    pub low_bar_ms: i64,
    /// `(close - low) / (high - low)` for the newest closed bar.
    ///
    /// **Deliberately NOT clamped.** Above 1.0 means the close has left the
    /// range upward and below 0.0 downward, which is the most informative
    /// thing this number ever says; the same choice, for the same reason, as
    /// `close_pct_of_prior_week_range` on `/api/paper/htf`. Clamping would
    /// turn a breakout into a ceiling.
    pub close_fraction_of_range: f64,
    pub close_zone: RangeZone,
    pub rule: String,
}

/// The dealing range: the last confirmed swing high and the last confirmed
/// swing low, whichever order they came in.
///
/// Same swings as everything else on this route, `fractal(left,right)`, so
/// the range's two ends are levels that appear elsewhere on the response
/// rather than a third view of the chart.
///
/// `None` when either end is missing, when the high is not above the low, or
/// when there is no finite close to place in it. A zero-width or inverted
/// range has no position in it to report, and a fraction with a zero or
/// negative denominator would be a number that looks like a measurement.
#[must_use]
pub fn dealing_range(bars: &[Bar], timeframe: &str, left: usize, right: usize) -> Option<DealingRange> {
    let high = *confirmed_swings(bars, left, right, true).last()?;
    let low = *confirmed_swings(bars, left, right, false).last()?;
    let close = bars.last().map(|b| b.close).filter(|v| v.is_finite())?;
    if !(high.price.is_finite() && low.price.is_finite()) || high.price <= low.price {
        return None;
    }
    let fraction = (close - low.price) / (high.price - low.price);
    Some(DealingRange {
        high: high.price,
        low: low.price,
        equilibrium: (high.price + low.price) / 2.0,
        high_swing_id: swing_id(timeframe, true, bars[high.at].time),
        low_swing_id: swing_id(timeframe, false, bars[low.at].time),
        high_bar_ms: bars[high.at].time,
        low_bar_ms: bars[low.at].time,
        close_fraction_of_range: fraction,
        close_zone: if fraction > 0.5 {
            RangeZone::Premium
        } else if fraction < 0.5 {
            RangeZone::Discount
        } else {
            RangeZone::Equilibrium
        },
        rule: format!(
            "dealing range: the last confirmed swing high to the last confirmed swing low, swings by \
             fractal({left},{right}); equilibrium is the midpoint, above it is premium and below it is \
             discount; close_fraction_of_range is (close - low) / (high - low) and is NOT clamped, so \
             over 1.0 is a close above the range"
        ),
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
    fn a_block_broken_and_then_traded_back_into_is_a_breaker_and_one_merely_broken_is_not() {
        // Bar 1 is the down candle, bar 2 the displacement up, bar 4 closes
        // at 93 — below the block's low of 96 — so the block breaks. Bar 5
        // stays below it entirely (high 94 < 96, so no return). Bar 6 trades
        // up to 97, back inside 96..100, and from the other side by
        // construction: the break was a CLOSE below the band, so price was
        // underneath it.
        let broken_then_back = vec![
            bar(0, 100.0, 100.5, 99.5, 100.0),
            bar(1, 100.0, 100.0, 96.0, 97.0),
            bar(2, 97.0, 106.0, 97.0, 105.0),
            bar(3, 105.0, 105.5, 97.0, 98.0),
            bar(4, 98.0, 98.0, 92.0, 93.0),
            bar(5, 93.0, 94.0, 91.0, 92.0),
            bar(6, 92.0, 97.0, 92.0, 96.5),
        ];
        let blocks = order_blocks(&broken_then_back, &flat_atr(broken_then_back.len(), 1.0), 1.0);
        let b = blocks
            .iter()
            .find(|b| b.level.band_low == Some(96.0) && b.level.band_high == Some(100.0))
            .expect("the 96..100 block");
        assert_eq!(b.level.state, LevelState::Breaker);
        assert_eq!(b.broken_at_bar_ms, Some(4 * M15), "the close that broke it is still on the block");
        assert_eq!(b.breaker_retested_at_bar_ms, Some(6 * M15), "and the bar that came back");
        // The whole history stays readable off the one object: the test
        // BEFORE the break is not overwritten by the return after it.
        assert_eq!(b.tested_at_bar_ms, Some(3 * M15));
        assert!(b.level.rule.contains("BREAKER"), "the rule that produced the state: {}", b.level.rule);

        // The same tape without the return: broken, and nothing more. A
        // BREAKER is not what every broken block eventually becomes.
        let merely_broken = &broken_then_back[..6];
        let blocks = order_blocks(merely_broken, &flat_atr(merely_broken.len(), 1.0), 1.0);
        let b = blocks
            .iter()
            .find(|b| b.level.band_low == Some(96.0) && b.level.band_high == Some(100.0))
            .expect("the 96..100 block");
        assert_eq!(b.level.state, LevelState::Broken);
        assert_eq!(b.breaker_retested_at_bar_ms, None);
        assert!(!b.level.rule.contains("BREAKER"), "{}", b.level.rule);
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

    /// A chart with the swings written out, so every assertion below can be
    /// checked by reading the numbers rather than by trusting the function.
    ///
    /// Swing highs by fractal(2,2): bar 5 at 110, bar 11 at 115, bar 17 at
    /// 120. Swing lows: bar 2 at 90, bar 8 at 95, bar 13 at 103, bar 20 at
    /// 90. Higher high (115 > 110) with a higher low (95 > 90) seeds UP on
    /// bar 13, which is the bar that CONFIRMS the swing high at bar 11.
    ///
    /// Then, in order: bar 14 WICKS to 117 through the 115 swing and closes
    /// at 114, bar 15 CLOSES at 116 through it, bar 18 closes at 102 through
    /// the 103 swing low the other way, and bar 23 closes at 89 through the
    /// 90 swing low made after that.
    fn structure_bars() -> Vec<Bar> {
        vec![
            bar(0, 99.0, 100.0, 98.0, 99.5),
            bar(1, 99.5, 101.0, 99.0, 100.0),
            bar(2, 100.0, 100.5, 90.0, 92.0),
            bar(3, 92.0, 102.0, 91.5, 101.0),
            bar(4, 101.0, 103.0, 99.0, 102.0),
            bar(5, 102.0, 110.0, 101.0, 104.0),
            bar(6, 104.0, 105.0, 99.0, 100.0),
            bar(7, 100.0, 104.0, 98.0, 99.0),
            bar(8, 99.0, 103.0, 95.0, 102.0),
            bar(9, 102.0, 106.0, 100.0, 105.0),
            bar(10, 105.0, 108.0, 101.0, 107.0),
            bar(11, 107.0, 115.0, 105.0, 110.0),
            bar(12, 110.0, 112.0, 104.0, 106.0),
            bar(13, 106.0, 111.0, 103.0, 105.0),
            bar(14, 105.0, 117.0, 104.0, 114.0),
            bar(15, 114.0, 118.0, 113.0, 116.0),
            bar(16, 116.0, 119.0, 115.0, 118.0),
            bar(17, 118.0, 120.0, 112.0, 113.0),
            bar(18, 113.0, 114.0, 100.0, 102.0),
            bar(19, 102.0, 105.0, 98.0, 99.0),
            bar(20, 99.0, 101.0, 90.0, 92.0),
            bar(21, 92.0, 96.0, 91.0, 95.0),
            bar(22, 95.0, 97.0, 92.0, 93.0),
            bar(23, 93.0, 94.0, 88.0, 89.0),
        ]
    }

    #[test]
    fn a_close_through_the_swing_high_in_an_uptrend_is_a_bos_and_a_wick_through_is_not() {
        // Only the bars up to and including the wick: 117 is through the 115
        // swing high and the close at 114 is not. Wick-based would report a
        // break here, which is the thing this rule refuses for the same
        // reason the order block's BROKEN refuses it.
        let wick_only = &structure_bars()[..15];
        let s = market_structure(wick_only, "15m", 2, 2);
        assert_eq!(s.label, StructureLabel::Up, "higher high and higher low: {s:?}");
        assert_eq!(s.label_since_bar_ms, Some(13 * M15), "seeded on the bar that confirmed the 115 swing");
        assert!(s.events.is_empty(), "a wick through a swing is not an event: {:?}", s.events);

        // One more bar, and the close is through.
        let closed_through = &structure_bars()[..16];
        let s = market_structure(closed_through, "15m", 2, 2);
        assert_eq!(s.events.len(), 1, "{:?}", s.events);
        let e = &s.events[0];
        assert_eq!(e.kind, StructureEventKind::Bos, "the structure was already UP: continuation");
        assert_eq!(e.direction, Direction::Bullish);
        assert_eq!(e.broke_price, 115.0);
        assert_eq!(e.broke_swing_bar_ms, 11 * M15);
        assert_eq!(e.broke_swing_id, swing_id("15m", true, 11 * M15), "joinable to the pool made of the same swing");
        assert_eq!(e.closed_at_bar_ms, 15 * M15);
        assert_eq!(e.close, 116.0);
        assert_eq!(e.age_bars, 0, "it happened on the newest bar");
        assert!(!e.superseded, "nothing has happened since");
        assert_eq!(e.structure_after, StructureLabel::Up, "a BOS leaves the label where it was");
        assert_eq!(s.label_since_bar_ms, Some(13 * M15), "and does not restamp it");
    }

    #[test]
    fn the_first_close_through_the_opposite_swing_is_a_choch_and_the_next_one_is_a_bos() {
        let s = market_structure(&structure_bars(), "15m", 2, 2);
        let kinds: Vec<_> = s.events.iter().map(|e| (e.kind, e.direction, e.closed_at_bar_ms / M15)).collect();
        assert_eq!(
            kinds,
            vec![
                (StructureEventKind::Bos, Direction::Bullish, 15),
                (StructureEventKind::Choch, Direction::Bearish, 18),
                (StructureEventKind::Bos, Direction::Bearish, 23),
            ],
            "{:?}",
            s.events
        );

        let choch = &s.events[1];
        assert_eq!(choch.broke_price, 103.0, "the last confirmed swing LOW, the side opposite to UP");
        assert_eq!(choch.broke_swing_id, swing_id("15m", false, 13 * M15));
        assert_eq!(choch.structure_after, StructureLabel::Down, "the crack is what flips the label");
        assert!(choch.superseded, "a later event has happened");
        assert_eq!(s.label, StructureLabel::Down);
        assert_eq!(s.label_since_bar_ms, Some(18 * M15), "the label is as old as the CHoCH that set it");

        // The second break DOWN is a BOS and not a second CHoCH: the label
        // is carried by the closes, so the swing pair still reading UP at
        // that moment does not get to override it. That is the one place
        // this rule and the swing comparison disagree, and it is the reason
        // the change of character is worth a name at all.
        let second = &s.events[2];
        assert_eq!(second.broke_price, 90.0, "the swing low made AFTER the CHoCH");
        assert_eq!(second.structure_after, StructureLabel::Down);

        // ONE EVENT PER SWING. Bar 16 closes at 118, above the same 115
        // swing high bar 15 already broke, and it is not a second BOS — a
        // trend would otherwise report one on every bar and a count of
        // events would be a count of bars.
        assert!(
            !s.events.iter().any(|e| e.closed_at_bar_ms == 16 * M15),
            "the 115 swing was already spent: {:?}",
            s.events
        );
    }

    #[test]
    fn a_break_with_no_prevailing_structure_is_counted_and_not_guessed_at() {
        // One swing high at 106 (bar 2, confirmed bar 4) and no swing low at
        // all, so the label never leaves RANGE. Bar 5 closes at 107, through
        // the swing: a break, and neither a continuation nor a change of
        // something that does not exist. Bar 6 closes higher still and is
        // not a second anything, because the swing was spent by the first
        // close beyond it.
        let flat = vec![
            bar(0, 100.0, 101.0, 99.0, 100.0),
            bar(1, 100.0, 102.0, 98.0, 101.0),
            bar(2, 101.0, 106.0, 100.0, 105.0),
            bar(3, 105.0, 105.5, 100.0, 101.0),
            bar(4, 101.0, 104.0, 99.0, 103.0),
            bar(5, 103.0, 108.0, 102.0, 107.0),
            bar(6, 107.0, 110.0, 106.0, 109.0),
        ];
        let s = market_structure(&flat, "15m", 2, 2);
        assert_eq!(s.label, StructureLabel::Range, "no pair of highs and lows to compare: {s:?}");
        assert!(s.events.is_empty(), "{:?}", s.events);
        assert_eq!(s.unclassified_breaks, 1, "the silence is published rather than left to look like calm");
        // And the measurement travels with the markers, so a reader meets
        // the lag and the absence in the same object.
        assert!(s.measured.choch_absent_at_turns_pct > 0.0);
        assert!(s.measured.source.contains("2026-09-19-smc-structure-measured"));

        // The classified tape reports zero rather than nothing: a count and
        // an absence are different facts.
        assert_eq!(market_structure(&structure_bars(), "15m", 2, 2).unclassified_breaks, 0);
    }

    #[test]
    fn the_dealing_range_is_the_last_two_swings_and_its_fraction_is_not_clamped() {
        // Swing high 105 at bar 2, swing low 90 at bar 5, and a close at 108
        // — eighteen dollars up a fifteen-dollar range, which is 1.2 and not
        // 1.0. Clamping would turn a breakout into a ceiling, which is the
        // same choice `close_pct_of_prior_week_range` makes on /api/paper/htf.
        let out_the_top = vec![
            bar(0, 100.0, 101.0, 99.0, 100.0),
            bar(1, 100.0, 102.0, 98.0, 101.0),
            bar(2, 101.0, 105.0, 100.0, 104.0),
            bar(3, 104.0, 104.5, 99.0, 100.0),
            bar(4, 100.0, 102.0, 95.0, 96.0),
            bar(5, 96.0, 99.0, 90.0, 92.0),
            bar(6, 92.0, 97.0, 91.0, 96.0),
            bar(7, 96.0, 110.0, 95.0, 108.0),
        ];
        let r = dealing_range(&out_the_top, "15m", 2, 2).expect("a range");
        assert_eq!((r.high, r.low), (105.0, 90.0));
        assert_eq!(r.equilibrium, 97.5, "the midpoint IS the level, not a statistic about it");
        assert_eq!(r.high_swing_id, swing_id("15m", true, 2 * M15));
        assert_eq!(r.low_swing_id, swing_id("15m", false, 5 * M15));
        assert_eq!(r.high_bar_ms, 2 * M15);
        assert_eq!(r.low_bar_ms, 5 * M15);
        assert!((r.close_fraction_of_range - 1.2).abs() < 1e-9, "{}", r.close_fraction_of_range);
        assert!(r.close_fraction_of_range > 1.0, "UNCLAMPED: a close above the range is a fact");
        assert_eq!(r.close_zone, RangeZone::Premium);

        // And below the range, the mirror: bar 23 closes at 89 under a 90
        // swing low, so the fraction is negative rather than floored at 0.
        let r = dealing_range(&structure_bars(), "15m", 2, 2).expect("a range");
        assert_eq!((r.high, r.low), (120.0, 90.0));
        assert!(r.close_fraction_of_range < 0.0, "{}", r.close_fraction_of_range);
        assert_eq!(r.close_zone, RangeZone::Discount);
    }

    #[test]
    fn an_empty_window_produces_nothing_rather_than_a_panic() {
        assert!(activity_profile(&[], 1.0, 0.7, 4.0).is_none());
        assert!(unfilled_fair_value_gaps(&[]).is_empty());
        assert!(order_blocks(&[], &[], 1.0).is_empty());
        assert!(liquidity_pools(&[], &[], "15m", 2, 2, 0.1, true).is_empty());
        assert!(runs_split_by_gap(&[], 1).is_empty());
        let s = market_structure(&[], "15m", 2, 2);
        assert_eq!(s.label, StructureLabel::Range);
        assert!(s.events.is_empty());
        assert_eq!(s.unclassified_breaks, 0);
        assert_eq!(s.label_since_bar_ms, None);
        assert!(!s.rule.is_empty(), "even with nothing to report, the rule says what was looked for");
        assert!(dealing_range(&[], "15m", 2, 2).is_none());
        // One bar is not a swing and a range of one point is not a range.
        assert!(dealing_range(&[bar(0, 100.0, 101.0, 99.0, 100.0)], "15m", 2, 2).is_none());
        assert_eq!(period_extremes(&[], 0..0, true, LevelKind::DayHigh, LevelKind::DayLow, "x").bars, 0);
    }
}
