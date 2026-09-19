//! Price-bar levels, computed once so the model and the desk read the same
//! marks.
//!
//! `GET /api/paper/levels?market=xauusd&tf=15m`. Everything here is derived
//! from CLOSED stored bars of one timeframe and nothing else: no forming bar,
//! no tick, no resample. Same discipline as [`crate::htf`] and for the same
//! reason — a prompt and a Desk panel that each computed their own levels
//! would eventually disagree, and the disagreement would surface as a model
//! explaining a gap the screen does not show.
//!
//! ## It carries no verdict, and that is a pre-commitment
//!
//! No score, no composite, no ranking, no confluence count, no zone labelled
//! with a word. `docs/hypotheses/2026-09-18-smc-context.md` was registered on
//! 2026-09-18 — **before this route existed** — and lines 68-72 say so in
//! advance, because the book that will read this route must be able to claim
//! the route did not decide for it. The prior behind that is not neutral:
//! every mechanical use of these levels this desk has tested has failed. The
//! full ICT chain closed at PF 0.75-0.77 and −0.17 to −0.19R over four
//! out-of-sample years; prior-day high and low closed at the 1st-6th
//! percentile of a null gated to their own session, which is WORSE than
//! random entry in the same hours; twenty-five more registrations tested the
//! options tape's POC, value area and walls and none survived. A score here
//! would smuggle that refuted claim back in under a new name.
//!
//! So the ordering of every list is chronological, oldest first. Not by
//! importance, not by distance from price: those are rankings, and the caller
//! that wants one applies its own and owns it.
//!
//! ## The receipt is on the wire, not only in this comment
//!
//! On 2026-09-19 this route grew the two pieces of the SMC vocabulary it was
//! missing — market structure (BOS and CHoCH) and the dealing range — plus
//! the breaker state on the order blocks. That is the whole of the chain
//! `docs/hypotheses/2026-09-13-ict-sweep-mss-fvg.md` tested and closed:
//! **1,428 out-of-sample trades, profit factor 0.746, expectancy -0.190R, at
//! the 95th percentile of a null whose own p95 is 0.746** — as good as the
//! least-losing coin flip. Every response carries those numbers as
//! `tested_as_a_rule`, on the ABSENT path too, because a reader of the JSON
//! never opens this file and a `BOS` field with nothing beside it reads like
//! a recommendation. Describing what the bars did is the job; what it means
//! is the reader's.
//!
//! ## One timeframe per response, and it is one the store HOLDS
//!
//! `?tf=` picks it and it defaults to `15m`, so every caller written before
//! 2026-09-19 asks the same question and gets the same answer. The accepted
//! set is not a list in this file: it is whatever the market has exported
//! under `data/bars`, because this route REFUSES to resample (see
//! [`stored_only`]) and can therefore only answer for a series that exists.
//! On 2026-09-19 that is 1m, 5m and 15m for XAUUSD and nothing coarser —
//! asking for `4h` gets the `unavailable` sentence naming 4h and both reasons
//! the bars are not rebuilt from 15m, not a 4h order block that never
//! existed. A spelling that is not a timeframe at all is a 400 listing what
//! the market does have, because that one is fixed by changing the call and
//! not by running an ingest.
//!
//! ## The window is a span of CLOCK, not a count of bars
//!
//! Ten trading days, on every timeframe: 879 bars of it on 15m, about 60 on
//! 4h, 10 on 1d. The response states which by publishing both numbers —
//! `window.days` is the trading days measured in the stamps and
//! `window.bars` is what they came to — so "ten days of 4h" (`days: 10,
//! bars: 60`) cannot be read as "ten bars of 4h" (`days: 2, bars: 10`).
//!
//! Clock rather than bars because the levels are CLOCK objects: the prior
//! day's high and the prior week's high are named after spans of clock, and
//! [`ANALYSIS_DAYS`] is ten because ten trading days is the smallest window
//! holding a complete prior week. A fixed bar count would silently mean two
//! weeks on 15m, two months on 4h and half a year on 1d — the prior-week
//! level would be the same object in all three while the gaps and blocks
//! around it came from different regimes, and nothing on the response would
//! say so. See [`trading_day_runs`] for how a trading day is found once the
//! bars grow longer than the hole that marks one.
//!
//! ## Units
//!
//! Every price is in the market's quote units. Everything else says what it
//! is in its name — `*_atr` in ATR(14) of this timeframe, `*_bars` in bars of
//! this timeframe, `*_ms` in UTC epoch milliseconds, `*_fraction` in 0..1.
//! `atr14` itself is published so every `*_atr` on the response has a visible
//! denominator (`docs/decisions/2026-09-17-unit-carrying.md`).
//!
//! **Every threshold on this route is in ATR of the series being read, so
//! they scale with `?tf=` on their own and none of them is a 15m number
//! wearing a 4h label.** A 4h order block is the last down candle before a 4h
//! body over one 4h ATR(14); the profile bucket is a quarter of a 4h ATR; two
//! 4h swing highs are "equal" within a tenth of a 4h ATR. One ATR series is
//! computed per response, published as `atr14`, and passed to every engine
//! function that needs a threshold — so the denominator of every `*_atr` on
//! the response is visible on the response, whatever `?tf=` was.
//!
//! ## Null is not zero, and an absent market is not an empty one
//!
//! When the stored bars are missing, every block is `null` and `unavailable`
//! carries the reason and the export command. It is not an empty list: an
//! empty list says "this market has no fair value gaps today", which is a
//! measurement, and the two must render differently.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use fd_core::types::Bar;
use fd_engine::price_levels::{
    BarProfile, DealingRange, FairValueGap, LevelKind, LiquidityPool, MarketStructure, OrderBlock, PeriodExtremes,
    activity_profile, bucket_size_price, dealing_range, liquidity_pools, market_structure, order_blocks,
    period_extremes, period_pool, runs_split_by_gap,
};
use fd_store::{read_bars, timeframe_ms};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::htf::weeks_of;
use crate::paper::now_ms;
use crate::state::AppState;

/* --------------------------------------------------------- the dials */

/// The profile's window, in TRADING DAYS of the market's own bars.
///
/// Days as the store counts them — runs of bars between the broker's daily
/// hole — and not 5 × 86,400,000 ms, so a bank holiday or a half session
/// shortens the window by the bars that are missing rather than silently
/// including a sixth day to make the arithmetic come out.
const DEFAULT_PROFILE_DAYS: usize = 5;

/// How many trading days of bars everything else is computed over.
///
/// Larger than the profile window on purpose: the prior WEEK's extremes are
/// in the registration's list, and a five-day window has no prior week in it
/// at all. Ten trading days is two broker weeks, which is the smallest window
/// in which "the last complete week" is a thing that exists.
///
/// **Trading DAYS on every timeframe, not bars** — ten of them is 879 bars of
/// 15m and about sixty of 4h, and the response publishes both. The reason is
/// the sentence above: this window is sized by what has to be INSIDE it, and
/// "a complete prior week" is a span of clock. A bar count that produced two
/// broker weeks on 15m would produce about half a year on 1d.
const ANALYSIS_DAYS: usize = 10;

/// A hard ceiling on the window a caller may ask for, so `?days=100000` reads
/// a hundred thousand day-runs' worth of bars into one response rather than
/// answering. Sixty trading days is about a quarter, which is past anything
/// anyone has asked an intraday profile for.
const MAX_PROFILE_DAYS: usize = 60;

/// Buckets per ATR(14) in the activity profile.
///
/// Four, so a typical bar of this timeframe spans several buckets and the
/// histogram has shape rather than one spike per bar. The number is published
/// on the response as `buckets_per_atr` beside the bucket height it produced,
/// so the derivation can be checked instead of believed.
const BUCKETS_PER_ATR: f64 = 4.0;

/// The share of activity the value area encloses. The market-profile
/// convention, unchanged, and named here rather than typed into a call.
const VALUE_AREA_PCT: f64 = 0.70;

/// A displacement is a bar whose body exceeds this many ATR(14) AT ITS OWN
/// BAR.
///
/// One ATR. A body of a whole ATR is the smallest thing that is not an
/// ordinary bar — the average bar's body is well under its true range — and a
/// smaller threshold turns every candle into an impulse and every candle
/// before it into an order block. NOT tuned: nothing here was fitted to a
/// result, because fitting a level rule to a result is precisely the family
/// of work this desk has forty closed registrations against.
const DISPLACEMENT_BODY_ATR: f64 = 1.0;

/// Two swing highs within this many ATR(14) of each other are "equal".
///
/// A tenth of an ATR. In ATR units and not dollars so the same rule reads the
/// same way in a quiet week and a violent one; on XAUUSD 15m an ATR of about
/// two dollars makes this about twenty cents, which is the order of a spread
/// plus a tick and therefore the order of "the same price" as a stop sitting
/// there would experience it.
const EQUAL_TOLERANCE_ATR: f64 = 0.10;

/// The fractal half-widths for the swings liquidity is built from.
///
/// `(2, 2)`: a swing high beats the two bars either side of it and is
/// confirmed two bars later. The same half-width `htf.rs` uses on its H4 row,
/// so the two routes' swings are the same KIND of object even where they run
/// on different bars — which is the only reason [`fd_engine::swing_id`] joins
/// anything.
const SWING_LEFT: usize = 2;
const SWING_RIGHT: usize = 2;

/// The hole that separates one trading day from the next.
///
/// Forty-five minutes, which is a measured fact rather than a convention: the
/// broker rolls its day at 21:00 UTC in US summer and 22:00 in winter, and
/// hour 21Z holds exactly zero 15m bars against 332-348 in every neighbouring
/// hour (measured 2026-09-18, recorded in `htf.rs`). So the boundary is an
/// hour-long hole. Forty-five minutes is under it and over the fifteen-minute
/// spacing of the bars themselves, and it moves with the changeover the way a
/// clock rule would not.
///
/// **It is only a threshold for bars SHORTER than the hole**, which is why
/// [`trading_day_runs`] checks the timeframe before reaching for it: 45
/// minutes is under the one-hour spacing of 1h bars, so on 1h this constant
/// would call every single bar its own trading day. That is the bug `?tf=`
/// would have shipped with had the day rule stayed a bare call to
/// `runs_split_by_gap`.
const DAY_GAP_MS: i64 = 45 * 60_000;

/// The hole itself: one clock hour, from the same 2026-09-18 measurement.
///
/// Named separately from [`DAY_GAP_MS`] because it answers a different
/// question — not "how big a gap counts" but "can a bar of this length show
/// the gap at all". A bar as long as the hole or longer hides it: see
/// [`trading_day_runs`].
const DAILY_HOLE_MS: i64 = 60 * 60_000;

/// One day of clock. The step the trading days are counted in where the hole
/// cannot be seen, and nothing else.
const DAY_MS: i64 = 86_400_000;

/// ATR's period, everywhere on this route. One number, published as `atr14`.
const ATR_PERIOD: usize = 14;

/* ------------------------------------------------------- the response */

/// Which file the bars came from, and how many were read.
///
/// The same three facts `HtfSourceDto` carries, and a separate type because
/// the two routes are allowed to diverge — this one may one day read a second
/// timeframe and `htf`'s may not. Named on the response because this route,
/// like `htf`, REFUSES to resample: see [`stored_only`].
#[derive(Debug, Serialize)]
pub struct LevelsSourceDto {
    /// The parquet the bars were read from, relative to the data root.
    pub file: String,
    /// Bars in the file, before the analysis window narrowed them.
    pub bars: usize,
    /// The stored timeframe of that file. Always equal to the timeframe asked
    /// for; a mismatch is refused rather than resampled.
    pub timeframe: String,
}

/// The slice of the file everything on this response was computed from.
///
/// Published because "the last ten trading days" is a different number of
/// bars every week, and a reader comparing two responses needs to know
/// whether a level disappeared or merely fell out of the window.
#[derive(Debug, Serialize)]
pub struct LevelsWindowDto {
    /// Bars in the analysis window. The COUNT; `days` is the span, and the
    /// two together say which of the two the window was cut by — see the
    /// module doc. On 15m these are 879 and 10; on 4h the same ten days are
    /// about sixty bars.
    pub bars: usize,
    /// Trading days in it, measured in the stamps by [`trading_day_runs`] and
    /// never assumed from the bar count. This is the number the window was
    /// asked for in ([`ANALYSIS_DAYS`]), or fewer when the store holds fewer.
    pub days: usize,
    pub start_bar_ms: i64,
    pub end_bar_ms: i64,
    /// The profile's own, narrower, window in trading days — the one dial a
    /// caller can turn, via `?days=`.
    pub profile_days: usize,
}

/// Session, day and week extremes.
///
/// **`session` is the run IN PROGRESS and `day` is the last COMPLETE one**,
/// which is the same convention `htf.rs` uses when it calls the bar before
/// the newest one the "prior day". The two are separate fields rather than
/// one labelled object because a forming extreme and a finished one are
/// different facts and a reader must not have to infer which they are
/// looking at; each also carries its own `state`, `FORMING` or `COMPLETE`.
///
/// **"Session" here means the broker's trading day, not Asia/London/NY.**
/// Those three need clock boundaries this desk has never measured for this
/// feed, and a guessed boundary is a level that would be wrong twice a year
/// at the daylight-saving changeover with nothing saying so. The hole between
/// runs is measured; a clock is not.
#[derive(Debug, Serialize)]
pub struct ExtremesDto {
    pub session: PeriodExtremes,
    pub day: PeriodExtremes,
    pub week: PeriodExtremes,
}

/// What happened when this family of levels was traded as a mechanical rule.
///
/// **It is on every response, including the ones with no bars**, because it
/// is a fact about the METHOD and not about today's tape. A reader meeting
/// `market_structure` for the first time meets this in the same object.
///
/// The desk did not decline to trade the ICT/SMC chain on principle; it
/// traded it, out of sample, and kept the receipt. Static text and static
/// numbers: nothing here is computed from the bars, and if these figures ever
/// change it is because a new registration closed and somebody edited this
/// struct on purpose.
#[derive(Debug, Serialize)]
pub struct TestedAsARuleDto {
    /// The chain, named in full, so a reader can tell whether what they are
    /// about to build is the thing that was tested.
    pub what: &'static str,
    pub hypothesis: &'static str,
    pub decision: &'static str,
    pub sample: &'static str,
    /// Trades in the out-of-sample run.
    pub out_of_sample_trades: u32,
    /// Gross profit over gross loss. Under 1.0 is a losing method.
    pub profit_factor: f64,
    /// Expectancy per trade in R — risk units, where 1R is the distance to
    /// the stop.
    pub expectancy_r: f64,
    /// The 95th percentile of the matched null's profit factor. It is the
    /// same number as `profit_factor`, which is the whole finding: the method
    /// landed exactly where the least-losing coin flip did.
    pub null_p95_profit_factor: f64,
    pub note: &'static str,
}

/// The one instance of it, spelled out where a reader of the route will meet
/// it.
///
/// ASCII only, like every other string this route writes to be displayed:
/// the clients' encodings are not this crate's to know.
const TESTED_AS_A_RULE: TestedAsARuleDto = TestedAsARuleDto {
    what: "the full ICT/SMC chain as a mechanical entry: a higher-timeframe fair value gap, a liquidity \
           sweep that closes back inside, a displacement close through structure, a retrace into the \
           impulse's gap, stop beyond the sweep",
    hypothesis: "docs/hypotheses/2026-09-13-ict-sweep-mss-fvg.md",
    decision: "docs/decisions/2026-09-13-ict-sweep-mss-fvg.md",
    sample: "four years of Dukascopy gold minutes, out of sample, pre-registered (batch ict-oos, 100 null seeds)",
    out_of_sample_trades: 1_428,
    profit_factor: 0.746,
    expectancy_r: -0.190,
    null_p95_profit_factor: 0.746,
    note: "It passed its gate on the only three months of broker minutes available and then lost out of \
           sample at the rate of noise traded at the same cost. Closed. Everything this route serves is \
           a description of what the bars did; none of it is a signal, and the numbers here are what \
           happened the last time this desk treated it as one.",
};

/// `GET /api/paper/levels?market=<id>&tf=<timeframe>&days=<n>`
///
/// 200 whenever the market is known, so a card or a prompt block can say WHY
/// it has nothing rather than decoding a status code.
#[derive(Debug, Serialize)]
pub struct LevelsResponse {
    pub market: String,
    /// The stored timeframe these levels are computed on. Every `age_bars`
    /// and every `*_atr` on this response is in units of THIS timeframe.
    pub timeframe: String,
    /// That timeframe's length in ms, so a reader ageing a `*_bar_ms` never
    /// has to reach for a sibling object to learn how long a bar is.
    pub bar_ms: i64,
    /// The newest CLOSED bar everything here describes.
    pub computed_at_bar_ms: Option<i64>,
    /// Wall clock when the response was computed. Its distance from
    /// `computed_at_bar_ms` is how stale the levels are.
    pub computed_at_ms: i64,
    pub last_close: Option<f64>,
    /// ATR(14) on this timeframe, in quote units. THE DENOMINATOR of every
    /// `*_atr` field below, published so each of them can be checked.
    pub atr14: Option<f64>,
    pub profile: Option<BarProfile>,
    /// Unfilled only, oldest first. An EMPTY list means the window has no
    /// unfilled gaps, which is a measurement; `null` means there were no bars
    /// to look at.
    pub fair_value_gaps: Option<Vec<FairValueGap>>,
    pub order_blocks: Option<Vec<OrderBlock>>,
    /// Equal-high and equal-low pools plus the prior day's and prior week's
    /// extremes, each with `swept` and the bar that swept it.
    pub liquidity: Option<Vec<LiquidityPool>>,
    /// The label as of the newest closed bar and every BOS and CHoCH behind
    /// it, oldest first. The events are on the SAME `fractal(2,2)` swings the
    /// liquidity pools are built from and name them by the same `swing_id`,
    /// so the two blocks can be joined rather than compared by eye.
    ///
    /// **`label` is an input to the event definitions, not a recommendation.**
    /// A BOS is only a BOS because the structure was already pointing that
    /// way, so the label has to be on the wire or no reader can check why an
    /// event was called one thing and not the other. What it is worth is in
    /// `tested_as_a_rule`.
    pub market_structure: Option<MarketStructure>,
    /// The last confirmed swing high to the last confirmed swing low, its
    /// midpoint, and where the last close sits in it — unclamped, so a close
    /// outside the range reads as a fraction outside 0..1.
    ///
    /// `null` when the window has not produced both a confirmed swing high
    /// and a confirmed swing low above it, which on a short or a one-way
    /// window is a real state and not an error.
    pub dealing_range: Option<DealingRange>,
    pub extremes: Option<ExtremesDto>,
    pub source: Option<LevelsSourceDto>,
    pub window: Option<LevelsWindowDto>,
    /// What happened when this family was traded as a rule. Constant, and
    /// present even when every block above is `null`.
    pub tested_as_a_rule: TestedAsARuleDto,
    /// Why there is nothing, or `null` when there is something.
    ///
    /// A sentence written to be displayed. **A prompt must not print it
    /// verbatim**: it is a join written in another crate, and `9a5dbfb` is
    /// the receipt for what that costs — `htf_context` printed a route's
    /// `unavailable` sentence into a registered campaign's prompt, so adding
    /// a timeframe to the route would have changed the prompt's wording with
    /// nobody editing the book. Read the field, say it in your own words.
    pub unavailable: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LevelsQuery {
    pub market: String,
    /// The stored timeframe. Defaults to `15m`, which is the timeframe the
    /// books decide on, so a caller that never passes this sees the response
    /// it saw before `?tf=` existed.
    ///
    /// Accepted: any timeframe the market has on disk. A timeframe that is
    /// not exported answers 200 with `unavailable` naming it — the fix is an
    /// ingest — and a string that is not a timeframe at all is a 400 listing
    /// what the market does have, because the fix is a different call.
    pub tf: Option<String>,
    /// The PROFILE's window in trading days. Everything else uses
    /// [`ANALYSIS_DAYS`], which is not a dial: the prior week has to be in
    /// view for the prior-week levels to exist at all, and letting a caller
    /// shrink the window below that would make those levels vanish for a
    /// reason the response could not state.
    pub days: Option<usize>,
}

/* ------------------------------------------------ loading, and refusing */

/// The stored series for a timeframe, and nothing else.
///
/// **This refuses where [`AppState::bars`] resamples**, exactly as
/// `htf::stored_only` does, and the reason is worth repeating rather than
/// cross-referencing because a reader here will not have `htf.rs` open:
/// `AppState::bars` falls back to rebucketing a finer series when no file
/// matches, anchored to the Unix epoch, and `fd_store::resample` emits the
/// TRAILING PARTIAL BUCKET as an ordinary bar. A still-forming 15m candle
/// indistinguishable from a closed one would make every level on this
/// response repaint, four times an hour, invisibly — and "computed from
/// closed bars only" is the property the whole file is documented on.
///
/// It is a second copy rather than a call into `htf.rs` because the two
/// routes own their own files: `htf` refuses a missing H4 with an export
/// command naming H4, and this one names the timeframe it was asked for.
///
/// `finer` is the stored series a resample would have been built from — the
/// coarsest one still shorter than `timeframe` — or `None` when there is no
/// such file. It is passed in rather than looked up here so the refusal can say WHICH
/// file it is declining to rebucket: "we do not resample" is a policy, and
/// "we are not turning the 15m file you can see on disk into 4h candles" is
/// the answer to the question an operator staring at that file will actually
/// ask. With nothing finer on disk there is nothing to resample from and the
/// clause is left off rather than printed as boilerplate.
fn stored_only(
    state: &AppState,
    market: &str,
    timeframe: &str,
    finer: Option<&str>,
) -> Result<(Vec<Bar>, LevelsSourceDto), String> {
    let spec = state.config.market(market).map_err(|e| e.to_string())?;
    let name = format!("{}-{timeframe}.parquet", spec.bar_symbol);
    let path = state.data.join("bars").join(&name);
    if !path.exists() {
        let mut why = format!(
            "no {timeframe} bars for {market}: bars/{name} has not been exported yet \
             (py/ingest/mt5_export.py --symbols {} --timeframes {})",
            spec.bar_symbol,
            mt5_name(timeframe)
        );
        if let Some(finer) = finer {
            // BOTH reasons, because each one alone is refutable and someone
            // will refute it. The anchor argument does not apply to 1h — the
            // server is a whole number of hours from UTC, so an epoch-anchored
            // hour really is the broker's hour (`htf.rs` measured it: all
            // 6,727 H4 stamps and all 1,122 D1 stamps carry minute 0 back to
            // 2022) — and the partial-bucket argument applies to every
            // timeframe including that one. Printing only the anchor would
            // invite "but H1 is fine", which is true and is not permission.
            //
            // ASCII only, like the sentence it extends and like every other
            // `unavailable` on this desk: this string is written to be
            // DISPLAYED, by clients whose encoding this crate does not know.
            // The em dashes elsewhere in this file are in comments, which
            // only ever reach a reader of the source.
            why.push_str(&format!(
                ". The {finer} bars on disk are NOT rebucketed to fill it, for two reasons. The \
                 broker anchors its higher-timeframe candles at the 21:00Z day roll (22:00Z \
                 outside US summer) and not at the epoch an ad-hoc resample buckets from: measured \
                 2026-09-18, real H4 stamps fall on hours {{1,2,5,6,9,10,13,14,17,18,21,22}}Z \
                 against {{0,4,8,12,16,20}}Z for an epoch-anchored resample, not one hour in \
                 common. And fd_store::resample closes a bucket only when a row lands past it, so \
                 it emits the trailing PARTIAL bucket as an ordinary bar, which would put a \
                 still-forming candle in a response documented as closed bars only. A {timeframe} \
                 order block off those candles is a level that never existed"
            ));
        }
        return Err(why);
    }
    let bars = read_bars(&path).map_err(|e| format!("bars/{name}: {e}"))?;
    if bars.is_empty() {
        return Err(format!("bars/{name} is empty"));
    }
    let source = LevelsSourceDto { file: format!("bars/{name}"), bars: bars.len(), timeframe: timeframe.to_string() };
    Ok((bars, source))
}

/// The MT5 name for one of our timeframes, for the export hint in a refusal.
///
/// Exhaustive rather than a two-branch guess, for the reason `htf::mt5_name`
/// records: the two-branch version was correct for its callers and silently
/// wrong for the next one, and the sentence telling an operator how to fix a
/// missing file read perfectly while naming the wrong timeframe.
fn mt5_name(timeframe: &str) -> &str {
    match timeframe {
        "1m" => "M1",
        "5m" => "M5",
        "15m" => "M15",
        "30m" => "M30",
        "1h" => "H1",
        "4h" => "H4",
        "1d" => "D1",
        other => other,
    }
}

/// The timeframes this market actually holds on disk, finest first.
///
/// The accepted set of `?tf=`, and it is read from the directory rather than
/// listed in code for the reason the whole file refuses to resample: a
/// timeframe this route can answer for is exactly a timeframe somebody
/// exported. A hard-coded list would name `4h` as available on a desk that
/// has not exported it - and the two desks differ, which is the point.
/// Checked 2026-09-19: the DEVELOPMENT store holds only 1m, 5m and 15m (21
/// files, seven symbols of each), while the VPS that trades holds 1m, 5m,
/// 15m, 1h, 4h and 1d for XAUUSD, because the hourly `flowdesk-htf-export`
/// task writes the coarse three there and nowhere else. So `?tf=4h` is
/// refused at home and answered in production, off the same code, which a
/// list in the source could not have got right for both.
///
/// Only exact `*.parquet` names count, which is not pedantry — the live store
/// keeps `XAUUSD-15m.parquet.bak` beside the live file, and a suffix test
/// looser than this one would advertise a timeframe named `15m.parquet`.
/// Sorted by bar length so the sentence a caller reads is in an order they
/// can scan, and de-duplicated so two files that resolve to the same
/// timeframe cannot list it twice.
fn stored_timeframes(state: &AppState, market: &str) -> Vec<String> {
    let Ok(spec) = state.config.market(market) else {
        return Vec::new();
    };
    let prefix = format!("{}-", spec.bar_symbol);
    let Ok(dir) = std::fs::read_dir(state.data.join("bars")) else {
        // No `bars` directory at all is "this market has nothing", not an
        // error: the caller is already being told its timeframe is unknown.
        return Vec::new();
    };
    let mut found: Vec<(i64, String)> = dir
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            let tf = name.strip_prefix(&prefix)?.strip_suffix(".parquet")?;
            Some((timeframe_ms(tf)?, tf.to_string()))
        })
        .collect();
    found.sort();
    found.dedup();
    found.into_iter().map(|(_, tf)| tf).collect()
}

/// The trading days of a series, oldest first, as index ranges.
///
/// TWO rules, because the measured fact that finds a trading day stops being
/// visible partway up the timeframe ladder, and one rule pretending otherwise
/// would report a WEEK as a day.
///
/// * Bars shorter than the hole (1m, 5m, 15m, 30m): the hole itself,
///   [`DAY_GAP_MS`]. Measured evidence, and it follows the 21:00Z/22:00Z
///   changeover the way a clock rule cannot.
/// * Bars as long as the hole or longer (1h, 4h, 1d): no gap rule can find
///   it. At 1h the hole is exactly one bar wide, so the day boundary and a
///   single missing bar leave the same two-hour gap and nothing in the stamps
///   tells them apart. At 4h and 1d the hole is INSIDE a bar and the stamps
///   are evenly spaced straight across it — real H4 stamps sit on
///   {1,2,5,6,9,10,13,14,17,18,21,22}Z, six a day, four hours apart including
///   over the roll (measured 2026-09-18, `htf.rs`). So the day is counted
///   from the one hole that is visible on every timeframe, the WEEKEND, in
///   steps of `DAY_MS` from the first bar of each week.
///
/// The anchor is re-taken at every week, which is what makes the second rule
/// safe: the broker's roll moves by an hour twice a year and it moves at a
/// weekend, so a changeover can never land inside a counted day. A single
/// epoch anchor would put the changeover day itself an hour out, twice a
/// year, and nothing on the response would say which day that was.
///
/// `weeks_of` is `htf.rs`'s weekend rule, the only one on this desk, and it
/// reads nothing but the gap between consecutive stamps — so a 4h series
/// splits on the same weekends a D1 series does.
fn trading_day_runs(bars: &[Bar], bar_ms: i64) -> Vec<std::ops::Range<usize>> {
    if bar_ms < DAILY_HOLE_MS {
        return runs_split_by_gap(bars, DAY_GAP_MS);
    }
    let mut out = Vec::new();
    for week in weeks_of(bars) {
        let anchor = bars[week.start].time;
        let mut start = week.start;
        for i in week.clone() {
            if (bars[i].time - anchor) / DAY_MS != (bars[start].time - anchor) / DAY_MS {
                out.push(start..i);
                start = i;
            }
        }
        out.push(start..week.end);
    }
    out
}

/// The last `days` trading days of a series, as an index range.
///
/// `None` when there are no bars. Fewer days than asked for is not an error —
/// a store holding three days answers with three, and `window.days` on the
/// response says so rather than the request's number.
fn last_days(bars: &[Bar], days: usize, bar_ms: i64) -> Option<std::ops::Range<usize>> {
    if bars.is_empty() || days == 0 {
        return None;
    }
    let runs = trading_day_runs(bars, bar_ms);
    let first = runs.len().saturating_sub(days);
    Some(runs[first].start..bars.len())
}

/* ----------------------------------------------------------- the route */

/// `GET /api/paper/levels?market=xauusd`
///
/// See [`LevelsResponse`]. 200 whenever the market is known.
pub async fn levels(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LevelsQuery>,
) -> Result<Json<LevelsResponse>, ApiError> {
    let market = query.market;
    let timeframe = query.tf.unwrap_or_else(|| "15m".to_string());
    let profile_days = query.days.unwrap_or(DEFAULT_PROFILE_DAYS).clamp(1, MAX_PROFILE_DAYS);
    // An unknown MARKET is a different failure from a missing FILE and gets a
    // different answer, which is the distinction `ApiError` is documented on:
    // a missing file is fixed by running an ingest, a market that does not
    // exist is fixed by changing the call. Folding both into `unavailable`
    // would tell an operator to export bars for a symbol this desk does not
    // have.
    //
    // It is checked BEFORE the timeframe so the 400 below can name the
    // market's own stored timeframes: `?market=nosuch&tf=7s` that answered
    // "nosuchmarket has no exported bars at all" would send an operator to
    // run an ingest for a symbol this desk does not have.
    state.config.market(&market).map_err(|e| ApiError::NotFound(e.to_string()))?;

    // A spelling that is not a timeframe is a 400 and not an `unavailable`:
    // no ingest fixes `?tf=7s`, only a different call does. The sentence
    // carries what this market DOES have, because the caller cannot know it —
    // the accepted set is whatever was exported, not a constant in this file.
    let stored = stored_timeframes(&state, &market);
    let Some(bar_ms) = timeframe_ms(&timeframe) else {
        let has = if stored.is_empty() {
            format!("{market} has no exported bars at all")
        } else {
            format!("{market} has {}", stored.join(", "))
        };
        return Err(ApiError::BadRequest(format!(
            "unknown timeframe {timeframe:?}: {has}. This route serves stored bars only and never \
             resamples, so it can answer for a timeframe on disk and no other."
        )));
    };
    // What a resample WOULD have been built from, named in the refusal so it
    // is clear the route saw the file and declined it rather than missed it.
    //
    // The COARSEST stored series still fine enough, because that is the one
    // `AppState::source_for` would have picked — its own comment says why:
    // "resampling a week of minutes into hours is not a better hour than the
    // stored hourly series covering a year, it is a shorter one". Naming the
    // finest would tell an operator holding 1m, 5m and 15m that the route
    // declined the 1m file, which is not the file it declined.
    let finer = stored.iter().rev().find(|tf| timeframe_ms(tf).is_some_and(|ms| ms < bar_ms));

    let (all, source) = match stored_only(&state, &market, &timeframe, finer.map(String::as_str)) {
        Ok(pair) => pair,
        Err(why) => {
            // EVERY block null, not an empty list. An empty `fair_value_gaps`
            // is a measurement — this window has none — and a market with no
            // file has not been measured at all.
            return Ok(Json(LevelsResponse {
                market,
                timeframe,
                bar_ms,
                computed_at_bar_ms: None,
                computed_at_ms: now_ms(),
                last_close: None,
                atr14: None,
                profile: None,
                fair_value_gaps: None,
                order_blocks: None,
                liquidity: None,
                market_structure: None,
                dealing_range: None,
                extremes: None,
                source: None,
                window: None,
                tested_as_a_rule: TESTED_AS_A_RULE,
                unavailable: Some(why),
            }));
        }
    };

    // The analysis window: the last two broker weeks of bars, so "the prior
    // week's high" is a level that exists. `last_days` cannot return None
    // here — `stored_only` refused an empty file — and the fallback is named
    // rather than unwrapped so a future caller cannot inherit a panic.
    let range = last_days(&all, ANALYSIS_DAYS, bar_ms).unwrap_or(0..all.len());
    let bars = &all[range.clone()];
    let last = bars.len() - 1;

    // ONE ATR series, computed here and published as `atr14`, passed to every
    // engine function that needs a threshold. A second ATR would make every
    // `*_atr` on this response a ratio whose denominator is invisible, which
    // is the defect `htf.rs` pins with `the_threshold_uses_the_published_atr`.
    let atr = fd_indicators::atr(bars, ATR_PERIOD);
    let atr14 = atr.iter().rev().copied().find(|v| v.is_finite());

    // Weeks come from `htf::weeks_of` — THE weekend rule on this desk lives
    // there and there is exactly one of it. It splits on the weekend HOLE
    // rather than the calendar, so it is right through the daylight-saving
    // changeover, and it is written against D1 bars but reads only the gap
    // between consecutive stamps, so a 15m series splits on the same weekend.
    let weeks = weeks_of(bars);
    // Days are the same idea with the broker's one-hour daily hole — where a
    // bar is short enough to show it. Not a second weekend rule:
    // `runs_split_by_gap(bars, 2 days)` IS `weeks_of`, and the route calls
    // `weeks_of` for weeks precisely so that stays true.
    let day_runs = trading_day_runs(bars, bar_ms);

    // The newest run is the one in progress; the one before it is the last
    // COMPLETE one. Same convention as `htf::d1_facts`, whose "prior day" is
    // the bar before the newest for the same reason: a run of bars looks the
    // same whether its day has ended or not, and only its position says
    // which.
    let session_run = day_runs.last().cloned();
    let prior_day = day_runs.len().checked_sub(2).and_then(|i| day_runs.get(i)).cloned();
    let prior_week = weeks.len().checked_sub(2).and_then(|i| weeks.get(i)).cloned();

    let profile = last_days(bars, profile_days, bar_ms)
        .and_then(|r| {
            let window = &bars[r];
            atr14
                .and_then(|a| bucket_size_price(a, BUCKETS_PER_ATR))
                .and_then(|size| activity_profile(window, size, VALUE_AREA_PCT, BUCKETS_PER_ATR))
        });

    let mut liquidity = liquidity_pools(bars, &atr, &timeframe, SWING_LEFT, SWING_RIGHT, EQUAL_TOLERANCE_ATR, true);
    liquidity.extend(liquidity_pools(bars, &atr, &timeframe, SWING_LEFT, SWING_RIGHT, EQUAL_TOLERANCE_ATR, false));
    // The prior day's and the prior week's extremes are in the registration's
    // liquidity list beside the equal highs, and they carry the same
    // `swept` pair. Their PRICES also appear in `extremes` — one number
    // reported under two questions, deliberately, rather than two numbers a
    // reader would have to reconcile.
    for (period, high, kind, label) in [
        (prior_day.clone(), true, LevelKind::PriorDayHigh, "prior day high: the last complete trading day's high"),
        (prior_day.clone(), false, LevelKind::PriorDayLow, "prior day low: the last complete trading day's low"),
        (prior_week.clone(), true, LevelKind::PriorWeekHigh, "prior week high: the last complete broker week's high"),
        (prior_week.clone(), false, LevelKind::PriorWeekLow, "prior week low: the last complete broker week's low"),
    ] {
        if let Some(pool) = period.and_then(|p| period_pool(bars, p, high, kind, label)) {
            liquidity.push(pool);
        }
    }
    // Oldest first. NOT by distance from price and NOT by importance: those
    // are rankings, and this route does not rank.
    liquidity.sort_by_key(|p| p.level.formed_at_bar_ms);

    // The SPINE, on the same swings the pools above are made of and with the
    // same half-widths, so an event and a pool that name the same swing
    // really are naming the same point. No second swing rule, and no ATR: a
    // close is beyond a swing or it is not, and a threshold here would be a
    // dial nobody measured.
    let structure = market_structure(bars, &timeframe, SWING_LEFT, SWING_RIGHT);
    let range = dealing_range(bars, &timeframe, SWING_LEFT, SWING_RIGHT);

    let none = 0..0;
    let extremes = ExtremesDto {
        session: period_extremes(
            bars,
            session_run.unwrap_or(none.clone()),
            false,
            LevelKind::SessionHigh,
            LevelKind::SessionLow,
            "session: the trading-day run in progress, split by the broker's daily hole",
        ),
        day: period_extremes(
            bars,
            prior_day.unwrap_or(none.clone()),
            true,
            LevelKind::DayHigh,
            LevelKind::DayLow,
            "day: the last COMPLETE trading-day run, split by the broker's daily hole",
        ),
        week: period_extremes(
            bars,
            prior_week.unwrap_or(none),
            true,
            LevelKind::WeekHigh,
            LevelKind::WeekLow,
            "week: the last COMPLETE broker week, split by the weekend hole (htf::weeks_of)",
        ),
    };

    Ok(Json(LevelsResponse {
        market,
        timeframe,
        bar_ms,
        computed_at_bar_ms: Some(bars[last].time),
        computed_at_ms: now_ms(),
        last_close: Some(bars[last].close).filter(|v| v.is_finite()),
        atr14,
        profile,
        fair_value_gaps: Some(fd_engine::price_levels::unfilled_fair_value_gaps(bars)),
        order_blocks: Some(order_blocks(bars, &atr, DISPLACEMENT_BODY_ATR)),
        liquidity: Some(liquidity),
        market_structure: Some(structure),
        dealing_range: range,
        extremes: Some(extremes),
        window: Some(LevelsWindowDto {
            bars: bars.len(),
            days: day_runs.len(),
            start_bar_ms: bars[0].time,
            end_bar_ms: bars[last].time,
            profile_days,
        }),
        source: Some(source),
        tested_as_a_rule: TESTED_AS_A_RULE,
        unavailable: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_export_hint_names_the_timeframe_that_is_missing() {
        // The two-branch version of this was right for its callers and
        // silently wrong for the next one. Every timeframe this route can be
        // asked for has a name here.
        assert_eq!(mt5_name("15m"), "M15");
        assert_eq!(mt5_name("1h"), "H1");
        assert_eq!(mt5_name("1d"), "D1");
    }

    #[test]
    fn the_window_is_counted_in_trading_days_and_not_in_milliseconds() {
        // Three runs of four bars with an hour's hole between them. Asking
        // for two days gets the last two runs; asking for ten gets all three
        // rather than an error, because a store holding three days holds
        // three days.
        let mut bars: Vec<Bar> = Vec::new();
        for day in 0..3i64 {
            for i in 0..4i64 {
                bars.push(Bar::flat(day * 10 * 900_000 + i * 900_000, 100.0));
            }
        }
        assert_eq!(last_days(&bars, 2, 900_000), Some(4..12));
        assert_eq!(last_days(&bars, 10, 900_000), Some(0..12));
        assert_eq!(last_days(&bars, 0, 900_000), None);
        assert_eq!(last_days(&[], 5, 900_000), None);
    }

    #[test]
    fn a_four_hour_bar_hides_the_daily_hole_so_the_day_is_counted_from_the_week() {
        // Two broker weeks of 4h bars: six a day, five days, then a weekend.
        // Stamps are the broker's, 21:00Z onwards, so the series crosses the
        // day roll the way the exported file does.
        const H4: i64 = 4 * 3_600_000;
        let mut bars: Vec<Bar> = Vec::new();
        for week in 0..2i64 {
            for day in 0..5i64 {
                for i in 0..6i64 {
                    bars.push(Bar::flat(week * 7 * DAY_MS + day * DAY_MS + i * H4, 100.0));
                }
            }
        }

        // The premise, asserted rather than asserted-in-prose: at 4h the gap
        // ACROSS the day roll is the same four hours as the gap inside the
        // session, so no threshold anywhere can separate the two and a
        // gap-based day rule is not merely mistuned, it is impossible.
        let inside: Vec<i64> =
            bars.windows(2).map(|w| w[1].time - w[0].time).filter(|gap| *gap < 2 * DAY_MS).collect();
        assert!(inside.iter().all(|gap| *gap == H4), "every non-weekend gap is one bar: {inside:?}");
        assert_eq!(trading_day_runs(&bars, H4).len(), 10);
        assert!(trading_day_runs(&bars, H4).iter().all(|r| r.len() == 6), "six 4h bars to a trading day");

        // And the window is ten DAYS of them — sixty bars — rather than ten
        // bars. This is the distinction the response publishes as `days`
        // beside `bars`.
        assert_eq!(last_days(&bars, 10, H4), Some(0..60));
        assert_eq!(last_days(&bars, 2, H4), Some(48..60));
    }

    #[test]
    fn an_hour_bar_is_as_long_as_the_hole_so_the_hole_rule_would_call_every_bar_a_day() {
        // 45 minutes is under the one-hour spacing of 1h bars. Left to
        // `DAY_GAP_MS`, a 1h series would report `window.days` equal to
        // `window.bars` — ten days of levels labelled as two hundred and
        // thirty — which is the bug this branch exists to prevent.
        const H1: i64 = 3_600_000;
        let mut bars: Vec<Bar> = Vec::new();
        for day in 0..5i64 {
            // 23 bars: the broker's day is 24 hours with the one-hour hole in
            // it, which at 1h is one absent bar.
            for i in 0..23i64 {
                bars.push(Bar::flat(day * DAY_MS + i * H1, 100.0));
            }
        }
        assert_eq!(runs_split_by_gap(&bars, DAY_GAP_MS).len(), bars.len(), "the premise, and it is the bug");
        assert_eq!(trading_day_runs(&bars, H1).len(), 5);
    }
}
