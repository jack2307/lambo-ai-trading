//! Hypotheses: a base method, a set of filters, a preset, and a matched null.
//!
//! The leaderboard asks "which method?"; a hypothesis asks "does *this gate*
//! change anything?" — trade only in the New York morning, stay flat over the
//! CME break, trade breakouts only when volatility is expanding. Each one is
//! the unchanged base method wrapped in [`Filtered`], run through the same
//! walk-forward as everything else, and read against **its own** null: the
//! random-entry control wrapped in the same filters. A session with a drift
//! would flatter any method traded inside it; the matched null is what keeps
//! that from being reported as an edge.
//!
//! A batch is declared, not searched for: in code for the built-in ones, or
//! in a TOML file (`docs/hypotheses/<id>.toml`) written **before** the run —
//! which is what lets a batch be pre-registered. Thirteen hypotheses is a
//! list someone wrote down with a reason each; a thousand is a search, and a
//! search always finds something.

use std::path::Path;

use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::filter::{Filter, Filtered};
use fd_strategy::registry::{BarContext, Intent, Params, Registry, Strategy};
use rayon::prelude::*;
use serde::Deserialize;

use std::collections::BTreeMap;

use crate::control::RandomEntry;
use crate::rebate::Rebate;
use crate::control_hold::RandomHold;
use crate::engine::{Metrics, Trade, TradingRules};
use crate::guards::Guards;
use crate::sweep::{PromisingGate, SelectBy, Verdict, verdict, walk_forward_guarded};

#[derive(Debug, Clone)]
pub struct Hypothesis {
    pub label: String,
    pub base: String,
    pub filters: Vec<Filter>,
    /// Parameter defaults changed from the method's own — a preset. The grid
    /// still sweeps around them.
    pub overrides: Vec<(String, f64)>,
    /// The reason it is on the list. A hypothesis without one is a search.
    pub why: String,
}

fn hyp(label: &str, base: &str, filters: Vec<Filter>, overrides: &[(&str, f64)], why: &str) -> Hypothesis {
    Hypothesis {
        label: label.to_string(),
        base: base.to_string(),
        filters,
        overrides: overrides.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
        why: why.to_string(),
    }
}

/// The batches that can be asked for by name.
#[must_use]
pub fn batch(name: &str) -> Option<Vec<Hypothesis>> {
    match name {
        "gold-intraday" => Some(gold_intraday_batch()),
        "ict-m1" => Some(ict_batch(15)),
        "ict-m5" => Some(ict_batch(3)),
        // Pre-registered before the out-of-sample run: only the two presets
        // that survived on the broker's three months of minutes.
        "ict-oos" => Some(ict_batch(15).into_iter().filter(|h| h.label.starts_with("ict-B")).collect()),
        _ => None,
    }
}

/* ------------------------------------------------------------ from a file */

/// One `[[hypothesis]]` table of a batch file.
#[derive(Debug, Deserialize)]
struct HypothesisFile {
    label: String,
    base: String,
    #[serde(default)]
    filters: Vec<String>,
    #[serde(default)]
    overrides: std::collections::BTreeMap<String, f64>,
    why: String,
}

#[derive(Debug, Deserialize)]
struct BatchFile {
    #[serde(default)]
    hypothesis: Vec<HypothesisFile>,
}

/// Read a batch from a TOML file.
///
/// ```toml
/// [[hypothesis]]
/// label = "orb/ny"
/// base = "donchian-breakout"
/// filters = ["weekdays", "sessions:0930-1130", "flat:1630-1815", "vol:14/100:1.2-99"]
/// overrides = { period = 12 }
/// why = "the first hour's range is the day's liquidity"
/// ```
///
/// Other tables (a `[run]` block naming markets and seeds, say) are ignored
/// here and read by the scripts that drive a run.
pub fn batch_from_file(path: &Path) -> Result<Vec<Hypothesis>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: BatchFile = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.hypothesis.is_empty() {
        return Err(format!("{}: no [[hypothesis]] tables", path.display()));
    }
    file.hypothesis
        .into_iter()
        .map(|h| {
            if h.why.trim().is_empty() {
                return Err(format!("{}/{}: a hypothesis needs a reason", h.label, h.base));
            }
            let filters = h.filters.iter().map(|s| Filter::parse(s)).collect::<Result<Vec<_>, _>>()?;
            Ok(Hypothesis {
                label: h.label,
                base: h.base,
                filters,
                overrides: h.overrides.into_iter().collect(),
                why: h.why,
            })
        })
        .collect()
}

/* ------------------------------------------------------------ built-in batches */

/// The first batch: gold, intraday, flat over the break.
///
/// Every entry stays out of the 16:30–18:15 New York window (the book is flat
/// across the 17:00 CME break, so no swap and no gap) and trades weekdays
/// only. On top of that, one gate each.
#[must_use]
pub fn gold_intraday_batch() -> Vec<Hypothesis> {
    let flat = || vec![Filter::weekdays(), Filter::flat(1630, 1815)];
    let with = |extra: Filter| {
        let mut f = flat();
        f.push(extra);
        f
    };
    let expansion = || Filter::VolRegime { fast: 14, slow: 100, min_ratio: 1.2, max_ratio: 99.0 };
    let compression = || Filter::VolRegime { fast: 14, slow: 100, min_ratio: 0.0, max_ratio: 0.8 };

    vec![
        // The flat rule on its own, on every base method: does removing the
        // overnight hold change the picture at all?
        hyp("intraday", "ema-cross", flat(), &[], "the swap-free version of the trend baseline"),
        hyp("intraday", "rsi-reversion", flat(), &[], "the swap-free version of the reversion baseline"),
        hyp("intraday", "donchian-breakout", flat(), &[], "the swap-free version of the breakout baseline"),
        hyp("intraday", "bb-fade", flat(), &[], "the swap-free version of the fade baseline"),
        // Sessions. Gold's volume lives in the New York morning; London's
        // open sets the day's range; Asia is thin and mean-reverting by repute.
        hyp("ny-morning", "ema-cross", with(Filter::hours(800, 1200)), &[], "trend into the session with the volume"),
        hyp("ny-morning", "donchian-breakout", with(Filter::hours(800, 1200)), &[], "breakouts where the liquidity is"),
        hyp("london-open", "donchian-breakout", with(Filter::hours(200, 600)), &[], "London sets the range; trade the break of the Asian one"),
        hyp("asia", "rsi-reversion", with(Filter::hours(1900, 200)), &[], "thin hours are said to mean-revert"),
        hyp("asia", "bb-fade", with(Filter::hours(1900, 200)), &[], "same claim, band-based"),
        // Regimes. The same signal reads differently when the range is
        // expanding versus compressing.
        hyp("expansion", "donchian-breakout", with(expansion()), &[], "breakouts only when volatility is already rising"),
        hyp("expansion", "ema-cross", with(expansion()), &[], "trend only when there is range to trend in"),
        hyp("compression", "rsi-reversion", with(compression()), &[], "fade only when the range is tight"),
        hyp("compression", "bb-fade", with(compression()), &[], "same, band-based"),
    ]
}

/// The ICT sweep → MSS → FVG expert's three presets, in and out of its kill
/// zones.
///
/// Sessions are the expert's defaults on the broker's clock (UTC+3): London
/// 08:00–12:00 and New York 13:00–17:00 server time are 01:00–05:00 and
/// 06:00–10:00 in New York. `htf_factor` is how many entry bars make one
/// higher-timeframe bar.
#[must_use]
pub fn ict_batch(htf_factor: usize) -> Vec<Hypothesis> {
    let base = "ict-sweep-mss-fvg";
    let zones = || Filter::sessions(&[(100, 500), (600, 1000)]);
    let factor = htf_factor as f64;
    let tight = [("htfFactor", factor), ("minHtfFvgPips", 25.0), ("swingLeft", 5.0), ("swingRight", 5.0), ("displacementMult", 2.0), ("riskReward", 3.0)];
    let balanced = [("htfFactor", factor), ("minHtfFvgPips", 15.0), ("swingLeft", 3.0), ("swingRight", 3.0), ("displacementMult", 1.5), ("riskReward", 2.0)];
    let loose = [("htfFactor", factor), ("minHtfFvgPips", 8.0), ("swingLeft", 2.0), ("swingRight", 2.0), ("displacementMult", 1.2), ("riskReward", 1.5)];
    vec![
        hyp("ict-A-tight", base, vec![Filter::weekdays(), zones()], &tight, "the expert's strict preset, kill zones only"),
        hyp("ict-B-balanced", base, vec![Filter::weekdays(), zones()], &balanced, "the expert's default preset, kill zones only"),
        hyp("ict-C-loose", base, vec![Filter::weekdays()], &loose, "the expert's loose preset, sessions off as it ships"),
        hyp("ict-B-allday", base, vec![Filter::weekdays()], &balanced, "the default preset without the session gate: is the kill zone doing anything?"),
    ]
}

/* ------------------------------------------------------------ running one */

/// One hypothesis, measured.
#[derive(Debug, Clone)]
pub struct HypothesisReport {
    pub label: String,
    pub base: String,
    pub filters: String,
    pub why: String,
    pub oos: Metrics,
    /// Financing paid across the out-of-sample trades. Zero is the proof that
    /// a flat window did its job.
    pub swap_usd: f64,
    /// Out-of-sample profit factors of the matched null, ascending.
    pub null_pf: Vec<f64>,
    /// Trade counts of those same null runs, ascending — what the control's
    /// calibration ACHIEVED, so a reader can check the match instead of
    /// trusting the comment that claims it. Sorted independently of
    /// `null_pf`, so the two vectors are the same runs but not aligned;
    /// nothing here needs them paired and a median wants them sorted.
    pub null_trades: Vec<usize>,
    /// Share of null runs the hypothesis beat, 0–100.
    pub percentile: f64,
    /// The stop the control was given and where it came from — so a reader
    /// can check the cost match the way `null_trades` lets them check the
    /// count match, instead of trusting a comment that claims it. `None` on a
    /// hold null, which has no stop.
    pub control_stop: Option<ControlStop>,
    /// Which sides the null took, so a receipt states which null its
    /// percentile belongs to rather than leaving a reader to infer it.
    pub null_sides: NullSides,
    /// The method's own long share on the trades this row's percentile was
    /// read from, 0.0-1.0, or `NaN` when it took none. The scope criterion of
    /// the drift control and the quantity the control's `longShare` is set
    /// from.
    pub long_share: f64,
    /// The long shares the null runs ACHIEVED, ascending — the side match, in
    /// the same spirit as [`Self::null_trades`]: measured on the control's own
    /// trades so a reader can check the match instead of trusting that it
    /// happened. On [`NullSides::CoinFlip`] these sit near 0.50 whatever the
    /// method did, which is the defect, visible.
    pub null_long_share: Vec<f64>,
    /// The method's **signed time in the market**, in long-minutes: minutes
    /// held long minus minutes held short, summed over the trades the
    /// percentile is read from. `NaN` when it took no trades. This is the
    /// quantity the instrument's drift is collected in proportion to, so it is
    /// what a drift control has to match — a control with the method's side
    /// ratio but three times its time in the market collects three times the
    /// drift.
    pub signed_minutes: f64,
    /// The same figure for each null run, ascending. Sorted independently of
    /// `null_pf`, like `null_trades`.
    pub null_signed_minutes: Vec<f64>,
    /// The method's **gross** time in the market, in minutes, over the same
    /// trades: how long it was exposed at all, without regard to side.
    ///
    /// The exposure the drift pays has two factors and this is the one that is
    /// always informative. See [`Self::exposure_matched`] for why the signed
    /// figure alone is not enough.
    pub gross_minutes: f64,
    /// The same figure for each null run, ascending.
    pub null_gross_minutes: Vec<f64>,
    /// The spread the method actually paid across those trades, in USD at each
    /// trade's own carried basis — the cost match, as a measured figure rather
    /// than as an inference from the stop.
    ///
    /// `control_stop` says what the control's stop was set to, which is the
    /// cost match on the entry branch and is all that branch needs. The hold
    /// branch has no stop at all, so it had no cost figure to check: there, the
    /// cost of a book is its trade count times the spread, and a control taking
    /// 3.4x the method's trades paid 3.4x the cost whatever its side ratio was.
    /// This field measures the thing itself on both branches.
    pub cost_usd: f64,
    /// The same figure for each null run, ascending.
    pub null_cost_usd: Vec<f64>,
    pub verdict: Verdict,
    /// How often a guard acted on the hypothesis's own out-of-sample runs
    /// (not the null's): entries refused by label, positions closed by
    /// label, entries sized down. All empty/zero on an unguarded run.
    pub skipped_by_guard: BTreeMap<String, usize>,
    pub closed_by_guard: BTreeMap<String, usize>,
    pub sized_down_by_guard: usize,
    /// What the method's RESTING entry orders did, when the run asked for
    /// them (`--limit=`). All zero on a market-entry run, which is every run
    /// before 2026-10-06.
    ///
    /// This is not decoration. A limit entry buys its better price with the
    /// signals it never gets into, and it is adversely selected by
    /// construction — it fills on the moves that come back and misses the ones
    /// that run. **A profit factor read on the fills alone is a profit factor
    /// on a subset the market chose**, so the share of signals that subset is
    /// has to sit on the same row as the profit factor.
    pub fills: crate::engine::LimitFills,
    /// The fill rates the NULL runs achieved, ascending.
    ///
    /// The one number that separates "the limit order is cheap" from "the
    /// limit order is selective": a random entry under the same price rule has
    /// no information in it, so its fill rate is the unconditional base rate.
    /// A method whose fill rate is BELOW its null's is one whose signals lead
    /// moves that do not come back — and the trades it is missing are the ones
    /// it was right about.
    pub null_fill_rate: Vec<f64>,
}

/// Which sides the matched null takes, and on what terms.
///
/// **This is a property of the measuring instrument, not a threshold, and it
/// is reported on every row it touches.** Both controls have always drawn a
/// side from a coin, so both carried an expected long share of 0.50 and an
/// expected drift term of about zero. Gold's unconditional drift is +0.3946
/// ATR20 per 5 sessions (t = +6.74) on the window this desk measures, and the
/// price on file went 1,200 to 3,700 USD per ounce, so a long-leaning
/// multi-day gold method beat such a null on its side ratio alone. That was
/// found and published as defect 1 of
/// `docs/decisions/2026-09-24-designed-methods.md`; the control, its two
/// rejected alternatives and the falsifiers are registered in
/// `docs/hypotheses/2026-10-02-drift-null.md` (and before it, on
/// `agent/repair-a`, in `docs/research/notes/2026-09-24-drift-control-choice.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NullSides {
    /// A coin, 50/50. **Every percentile published before 2026-10-02 was read
    /// against this**, so it is the default: a reader who does not ask for the
    /// drift control gets the number the record already carries.
    #[default]
    CoinFlip,
    /// The method's own **measured** long share, as [`side_distribution`]
    /// reads it off the method's realised trades, drawn independently per
    /// entry. Matched on the ratio and on nothing else: the timing is still
    /// random and the side *sequence* is destroyed.
    ///
    /// On the hold branch this leaves `entryRate` at 1.0, so the control is in
    /// the market for the whole gate and its ratio is matched while its
    /// exposure is not. Kept as a named setting because it is what
    /// `agent/repair-a` built and the collapse it reported has to be
    /// reproducible.
    MatchedRatio,
    /// [`Self::MatchedRatio`] plus a count-matched entry rate on the hold
    /// branch, so the control's signed time in the market follows the
    /// method's and not the gate's. This is the drift control the record is
    /// to be read against.
    MatchedExposure,
}

impl NullSides {
    /// The word a receipt prints, and the word `--null-sides=` accepts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CoinFlip => "coin",
            Self::MatchedRatio => "ratio",
            Self::MatchedExposure => "exposure",
        }
    }

    /// One line for a receipt header, so a file quoted on its own says what
    /// its percentiles were read against.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::CoinFlip => "50/50, carries no drift; the null every published percentile was read against",
            Self::MatchedRatio => "the method's measured long share; ratio matched, time in market not",
            Self::MatchedExposure => "the method's measured long share AND its trade count on the hold branch",
        }
    }

    /// Whether the control takes the method's measured long share at all.
    #[must_use]
    pub const fn matches_ratio(self) -> bool {
        matches!(self, Self::MatchedRatio | Self::MatchedExposure)
    }

    /// Whether the hold branch's entry rate is calibrated to the method's
    /// trade count. `false` on both older settings, which is the recorded
    /// defect rather than a choice.
    #[must_use]
    pub const fn matches_hold_count(self) -> bool {
        matches!(self, Self::MatchedExposure)
    }

    /// Parsed from `--null-sides=`. `Err` names what was understood rather
    /// than falling back to the default: a misspelt flag that silently ran the
    /// unrepaired null would be published as a repaired number.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "coin" => Ok(Self::CoinFlip),
            "ratio" => Ok(Self::MatchedRatio),
            "exposure" => Ok(Self::MatchedExposure),
            other => Err(format!("unknown --null-sides `{other}` (have: coin, ratio, exposure)")),
        }
    }
}

/// The share of `trades` that were long, 0.0-1.0. `None` when there are none,
/// because a side ratio of nothing is not 0.5 and is not zero either.
///
/// The sibling of `hold_distribution`, and for the same reason: a control is
/// matched to a **measured** property of the method rather than to a guess.
/// The comments on `hold_distribution` record two occasions when a guessed
/// hold was the null and was wrong; a guessed side ratio is the same mistake
/// with the drift term instead of the tail.
///
/// This is the *ratio* and deliberately not the sequence. Handing a control
/// which trade was long would hand it the method's timing, which is the thing
/// under test.
#[must_use]
pub fn side_distribution(trades: &[crate::engine::Trade]) -> Option<f64> {
    if trades.is_empty() {
        return None;
    }
    let longs = trades.iter().filter(|t| t.direction.is_long()).count();
    Some(longs as f64 / trades.len() as f64)
}

/// Gross time in the market, in minutes: how long the book was exposed at all,
/// whichever side. `None` on an empty book.
#[must_use]
pub fn gross_exposure_minutes(trades: &[crate::engine::Trade]) -> Option<f64> {
    if trades.is_empty() {
        return None;
    }
    Some(trades.iter().map(|t| (t.exit_time - t.entry_time).max(0) as f64 / 60_000.0).sum())
}

/// Signed time in the market, in **long-minutes**: minutes held long minus
/// minutes held short. `None` on an empty book.
///
/// This is the quantity an instrument's unconditional drift is collected in
/// proportion to, which is why the drift control has to match it and not only
/// the side ratio. Two books can share a long share of 1.000 and differ
/// threefold in what the drift paid them.
#[must_use]
pub fn signed_exposure_minutes(trades: &[crate::engine::Trade]) -> Option<f64> {
    if trades.is_empty() {
        return None;
    }
    Some(
        trades
            .iter()
            .map(|t| {
                let minutes = (t.exit_time - t.entry_time).max(0) as f64 / 60_000.0;
                if t.direction.is_long() { minutes } else { -minutes }
            })
            .sum(),
    )
}

/// The spread paid across `trades`, in USD, at each trade's **own** carried
/// basis where it has one.
///
/// A trade that carries neither a `spread` nor a `contract_size` is priced
/// from the market's config and is an ESTIMATE, exactly as
/// `docs/decisions/2026-09-21-restated-pnl-contract-size.md` requires: a
/// number computed from a config table that may since have been corrected is
/// not the number the trade was booked at.
#[must_use]
pub fn gross_cost_usd(trades: &[crate::engine::Trade], rules: &TradingRules) -> f64 {
    trades
        .iter()
        .map(|t| {
            let spread = t.spread.unwrap_or(rules.spread);
            let contract = t.contract_size.unwrap_or(rules.contract_size);
            spread * t.lots * contract
        })
        .sum()
}

/// The long share a control must be drawn at for its **signed share of time**
/// to come out at `signed_share`, assuming its holds do not depend on which
/// side it drew.
///
/// `(1 + signed_share) / 2`, clamped. It is the inverse of
/// `signed_share = 2 * long_share - 1`, which holds exactly when a control's
/// hold draw is independent of its side — true of
/// [`crate::control_hold::RandomHold`] by construction, since the hold is a
/// function of the entry time and the seed on streams 2 and 3 while the side is
/// stream 1 — and approximately of [`crate::control::RandomEntry`], whose
/// trades end at a stop and so last longer or shorter depending on which way
/// the market went. Which is why the achieved signed share is MEASURED on the
/// control's own trades and printed on the row, and a row outside the band is
/// reported unmatched rather than quoted.
///
/// **Why the control is drawn at this and not at the method's long share.**
/// Drift is paid on signed TIME in the market, not on a count of trades, and
/// the two part company whenever a method holds its longs and its shorts for
/// different lengths. Measured on the 32 rows of
/// `2026-09-13-recent-year-sessions.toml`, 2025-09-13 to 2026-09-12: the median
/// gap between a row's signed share of time and the `2 * long_share - 1` its
/// trade count implies is **0.055**, and **10 of 31 measurable rows exceed
/// 0.10** — two side-match bands. The largest is `doji-reversal/asia`, 52.6% of
/// trades long and a signed share of time of **+0.478** against the +0.052 its
/// count implies, because its longs are held nine times as long as its shorts.
/// A control drawn at 0.526 would have been given a twentieth of that row's
/// drift exposure while reporting a side match of 0.526 against 0.531.
#[must_use]
pub fn long_share_for_signed_share(signed_share: f64) -> f64 {
    ((1.0 + signed_share) / 2.0).clamp(0.0, 1.0)
}

/// Whether a long share sits outside the 40-60% band that
/// `docs/hypotheses/2026-10-02-drift-null.md` uses to decide which published
/// rows the drift control can move.
///
/// A row inside the band is two-sided enough that its drift term is near the
/// coin's, so the repaired control should leave it where it was — which is the
/// registration's own test of the repair. `NaN` (no trades) is **not** outside
/// the band: a row with no book has no side ratio, and claiming one would be
/// the assumption this desk keeps being caught making.
#[must_use]
pub fn side_ratio_is_one_sided(long_share: f64) -> bool {
    long_share.is_finite() && !(0.40..=0.60).contains(&long_share)
}

/// How close two long shares have to be before the side match counts as
/// achieved: five points of share, wider than the sampling error of a
/// Bernoulli draw at any count these runs produce and narrow enough that a
/// mis-set control cannot hide inside it. A check on the instrument, not a
/// gate on anything.
pub const SIDE_MATCH_BAND: f64 = 0.05;

/// How far a control's trade count may sit from its method's before the
/// count-matching has failed, as a fraction: 0.25 is "within about a
/// quarter", which is what the 2026-09-23 registration
/// (`docs/hypotheses/2026-09-23-matched-null-repair.md`) named as the line
/// between a repair and a repair that did not work. It is not a gate on a
/// strategy and it is not a threshold anyone may move to make a number
/// pass; it is a check on the measuring instrument. Held by
/// `crates/fd-backtest/tests/matched_null.rs`.
pub const COUNT_MATCH_BAND: f64 = 0.25;

/// The median of an ascending list of counts, as a float. `NaN` when empty,
/// because a median of nothing is not zero.
#[must_use]
pub fn median_count(ascending: &[usize]) -> f64 {
    if ascending.is_empty() {
        return f64::NAN;
    }
    let n = ascending.len();
    if n % 2 == 1 { ascending[n / 2] as f64 } else { (ascending[n / 2 - 1] + ascending[n / 2]) as f64 / 2.0 }
}

/// The median of a list of floats, which this one sorts itself. `NaN` when
/// empty, for the same reason [`median_count`] is: a median of nothing is not
/// zero.
#[must_use]
pub fn median_f64(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let mut s = values.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = s.len();
    if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) / 2.0 }
}

/// The control's median trade count as a fraction of the method's. 1.0 is a
/// perfect match, 0.5 is a control taking half the method's trades, and
/// `NaN` when either side has nothing to compare.
#[must_use]
pub fn count_match_ratio(method_trades: usize, null_trades: &[usize]) -> f64 {
    if method_trades == 0 || null_trades.is_empty() {
        return f64::NAN;
    }
    median_count(null_trades) / method_trades as f64
}

impl HypothesisReport {
    /// What the count-matching achieved on this row: the control's median
    /// out-of-sample trade count over the method's. See
    /// [`COUNT_MATCH_BAND`]; a figure outside `1 ± band` means the null this
    /// row's percentile was read against is not the method's size.
    #[must_use]
    pub fn count_match(&self) -> f64 {
        count_match_ratio(self.oos.trades, &self.null_trades)
    }

    /// Whether [`Self::count_match`] is inside the registration's band. A
    /// row with no trades or no null runs is `false`: it has no match to
    /// report, and reporting one would be the assumption the registration
    /// forbids.
    #[must_use]
    pub fn count_matched(&self) -> bool {
        let r = self.count_match();
        r.is_finite() && (r - 1.0).abs() <= COUNT_MATCH_BAND
    }

    /// The control's median achieved long share, or `NaN` when it has no runs.
    /// Printed beside [`Self::long_share`] on every row: the side match,
    /// stated as achieved rather than as intended.
    #[must_use]
    pub fn null_long_share_median(&self) -> f64 {
        median_f64(&self.null_long_share)
    }

    /// Whether the control's side ratio landed on the method's, within
    /// [`SIDE_MATCH_BAND`]. Only asked of a run that claims to match it; on a
    /// coin flip there is no side match to achieve and this is `false` for any
    /// row the method leaned on one side, which is the true statement about
    /// those rows.
    #[must_use]
    pub fn side_matched(&self) -> bool {
        let (m, n) = (self.long_share, self.null_long_share_median());
        m.is_finite() && n.is_finite() && (m - n).abs() <= SIDE_MATCH_BAND
    }

    /// The method's signed share of its own time in the market: signed minutes
    /// over gross minutes, so +1.000 is "long the whole time", -1.000 is
    /// "short the whole time" and 0.000 is "long and short for equal time".
    /// `NaN` when it was never in the market.
    ///
    /// This is NOT the long share. The long share counts trades; this weights
    /// them by how long each was held, and the two differ whenever a method's
    /// longs and shorts are held for different lengths. The drift is paid on
    /// this one.
    #[must_use]
    pub fn signed_share(&self) -> f64 {
        if !self.gross_minutes.is_finite() || self.gross_minutes <= 0.0 {
            return f64::NAN;
        }
        self.signed_minutes / self.gross_minutes
    }

    /// The control's median signed share of time, the same way.
    #[must_use]
    pub fn null_signed_share_median(&self) -> f64 {
        let shares: Vec<f64> = self
            .null_signed_minutes
            .iter()
            .zip(&self.null_gross_minutes)
            .filter(|(_, g)| g.is_finite() && **g > 0.0)
            .map(|(s, g)| s / g)
            .collect();
        median_f64(&shares)
    }

    /// What the time-in-market match achieved: the control's median gross
    /// minutes over the method's. 1.0 is a match; `NaN` when either side has
    /// nothing to compare. **This was 3.1-3.7 before the hold branch was
    /// count-matched**, and it is the factor by which such a control
    /// over-collected the drift.
    #[must_use]
    pub fn time_in_market_match(&self) -> f64 {
        let n = median_f64(&self.null_gross_minutes);
        if !n.is_finite() || !self.gross_minutes.is_finite() || self.gross_minutes <= 0.0 {
            return f64::NAN;
        }
        n / self.gross_minutes
    }

    /// Whether the control's exposure to the instrument's drift matches the
    /// method's. **Two conditions, because the exposure has two factors**, and
    /// one of them alone is an instrument that lies in both directions:
    ///
    /// - the **signed share** of time, within [`SIDE_MATCH_BAND`], and
    /// - the **gross time in the market**, within [`COUNT_MATCH_BAND`].
    ///
    /// Neither is sufficient. A ratio of signed minutes alone — which this
    /// accessor computed until 2026-10-02 — is a ratio of two small
    /// differences of large numbers on any two-sided row, so it printed
    /// figures like -4.80 and 0.02 on rows whose side match was 0.502 against
    /// 0.500 and whose count match was 0.86: noise reported as a failure.
    /// Measured on the 32 rows of `2026-09-13-recent-year-sessions.toml` and
    /// corrected there and then, with the reason recorded in
    /// `docs/decisions/2026-10-02-drift-null.md` rather than absorbed. And a
    /// signed share alone misses the other half: a long-only control with the
    /// method's side ratio exactly, in the market 1.35x as long, has a signed
    /// share of +1.000 against the method's +1.000 and collects 35% more
    /// drift.
    #[must_use]
    pub fn exposure_matched(&self) -> bool {
        let (ms, ns) = (self.signed_share(), self.null_signed_share_median());
        let t = self.time_in_market_match();
        ms.is_finite()
            && ns.is_finite()
            && (ms - ns).abs() <= SIDE_MATCH_BAND
            && t.is_finite()
            && (t - 1.0).abs() <= COUNT_MATCH_BAND
    }

    /// What the cost match achieved: the control's median spread bill over the
    /// method's, both in USD. 1.0 is a match; `NaN` when either side has
    /// nothing to compare. On the entry branch this follows from the stop the
    /// control was given; on the hold branch it follows from the trade count,
    /// and it was 3.1-3.7x before the hold branch was count-matched.
    #[must_use]
    pub fn cost_match(&self) -> f64 {
        let n = median_f64(&self.null_cost_usd);
        if !n.is_finite() || !self.cost_usd.is_finite() || self.cost_usd == 0.0 {
            return f64::NAN;
        }
        n / self.cost_usd
    }

    /// **This ratio is CONFOUNDED BY EQUITY GROWTH and must not be read as a
    /// cost mismatch on its own.** Measured 2026-10-02 on
    /// `2026-10-02-drift-reproduction.toml`: the `long-day` row paid 36,124.98
    /// USD of spread against its coin-flip control's 5,400.83 — a ratio of 0.15
    /// — on trade counts of 1,751 against 1,760. The control did not pay a
    /// seventh of the method's cost per trade. The engine sizes from equity, the
    /// method compounded at a profit factor of 1.138 over 1,751 trades and the
    /// control at 1.071, so the method's later trades were about seven times
    /// larger and paid about seven times the spread. The same pair under the
    /// exposure-matched control, whose profit factor comes out at 1.146 against
    /// the method's 1.138, reads 0.98.
    ///
    /// So the figure is informative where the two profit factors are close and
    /// is a performance difference where they are not. **The cost statement that
    /// is free of equity is cost as a fraction of risk, `spread / stop`,** which
    /// is `control_stop.cost_fraction_of_r` on the entry branch and is what
    /// `docs/hypotheses/2026-09-24-cost-matched-null.md` repaired. This field
    /// exists because the hold branch has no stop and therefore had no cost
    /// figure of any kind; it is a weaker instrument than the stop and is
    /// recorded as one.
    ///
    /// Whether [`Self::cost_match`] is inside [`COUNT_MATCH_BAND`] — the same
    /// quarter, because it is the same kind of claim about the same control.
    #[must_use]
    pub fn cost_matched(&self) -> bool {
        let r = self.cost_match();
        r.is_finite() && (r - 1.0).abs() <= COUNT_MATCH_BAND
    }

    /// Whether the null is a **distribution** at all: whether its profit
    /// factors differ between seeds.
    ///
    /// Worth stating plainly, because it was true before the drift control
    /// existed and nothing reported it. A window-hold null runs at
    /// `entryRate = 1.0` and, when the preset names a window, at a fixed hold
    /// — so **its only random input is the side coin.** Give it the method's
    /// side ratio at 0 or 1 and every seed replays the same run: `null p50`
    /// equals `null p95`, and the "percentile" is a single comparison dressed
    /// as a quantile. For such a row the coin-flip null's whole spread WAS the
    /// side lottery, which is a lottery the method never faced.
    ///
    /// It is not a defect of the ratio match; the match makes it visible. A
    /// method in the market on every bar of its gate, on one side, has made no
    /// timing decision, and there is nothing for a timing control to destroy.
    #[must_use]
    pub fn null_has_spread(&self) -> bool {
        let Some(first) = self.null_pf.first() else { return false };
        self.null_pf.len() > 1 && self.null_pf.iter().any(|pf| pf != first)
    }

    /// The percentile, or **`None` when the null it would be read against is
    /// not a distribution**. A measurement that was not made is `null`; it is
    /// not 0 and it is not 100. Every caller that prints a percentile goes
    /// through this.
    #[must_use]
    pub fn percentile_or_null(&self) -> Option<f64> {
        (self.null_has_spread() && self.percentile.is_finite()).then_some(self.percentile)
    }

    #[must_use]
    pub fn null_quantile(&self, q: f64) -> f64 {
        if self.null_pf.is_empty() {
            return f64::NAN;
        }
        self.null_pf[(((self.null_pf.len() - 1) as f64) * q).round() as usize]
    }

    /// Outside the noise *and* past the gate. Both, or it is not a finding.
    ///
    /// A row whose null has no spread cannot be outside it, so it does not
    /// survive — on `null`, not on a failed comparison.
    #[must_use]
    pub fn survives(&self) -> bool {
        self.verdict.promising && self.percentile_or_null().is_some_and(|p| p >= 95.0)
    }

    /// The guard activity in one line for a receipt: `guards: refused
    /// WEEKEND_FLAT 3, DAILY_TRADE_CAP 12; closed OPEN_LOSS_CAP 2; sized down
    /// 0`. `guards: nothing acted` when every count is zero.
    #[must_use]
    pub fn guard_activity(&self) -> String {
        let list = |m: &BTreeMap<String, usize>| m.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ");
        if self.skipped_by_guard.is_empty() && self.closed_by_guard.is_empty() && self.sized_down_by_guard == 0 {
            return "guards: nothing acted".to_string();
        }
        let refused = if self.skipped_by_guard.is_empty() { "none".to_string() } else { list(&self.skipped_by_guard) };
        let closed = if self.closed_by_guard.is_empty() { "none".to_string() } else { list(&self.closed_by_guard) };
        format!("guards: refused {refused}; closed {closed}; sized down {}", self.sized_down_by_guard)
    }
}

/// The control matched to the base method's shape, at one seed.
///
/// A method with stops and targets is read against random *entries* with
/// the same; a method that only holds through a window (`Exits::Strategy`,
/// no grid — a drift claim) is read against random *holds* of the same
/// length. Reading a drift against random entries with ATR stops is how
/// the first drift test produced a null with a 95th percentile near 3.
/// The entry rate at which the random-entry control produces about as many
/// trades as the method under test, over the same bars with the same
/// filters.
///
/// A control with five times the method's trades has a much tighter
/// profit-factor distribution, and a percentile read against it flatters a
/// thin method — the adversary caught a 61-trade row measured against a
/// 300-trade null. So the control is calibrated: one probe run at the
/// default rate, then the rate scaled to the method's count. Pinned, so the
/// control's grid loses its rate axis too.
///
/// `stop_atr` is the control's stop, which has to be **set before the probe
/// runs**: it is how long the control's trades last and therefore how many it
/// takes on the same bars, so a rate calibrated at one stop and then used at
/// another trades the count match away for the cost match
/// (`docs/hypotheses/2026-09-24-cost-matched-null.md`, pre-commitment 5).
fn matched_rate(
    bars: &[Bar],
    rules: &TradingRules,
    filters: &[Filter],
    target_trades: usize,
    guards: Option<&Guards>,
    stop_atr: f64,
) -> f64 {
    use crate::engine::{Range, run_backtest_guarded};
    const PROBE: f64 = 0.02;
    let mut p = RandomEntry.default_params();
    p.set("entryRate", PROBE);
    p.set("seed", 1.0);
    if stop_atr.is_finite() && stop_atr > 0.0 {
        p.set("stopAtr", stop_atr);
    }
    let probe = Filtered { inner: &RandomEntry, filters: scoped(filters, rules) };
    let got = run_backtest_guarded(bars, &probe, &p, rules, guards, None, Range::default(), None).trades.len();
    if got == 0 || target_trades == 0 {
        return PROBE;
    }
    (PROBE * target_trades as f64 / got as f64).clamp(0.0005, 1.0)
}

/// The entry rate at which the **hold** control takes about as many trades as
/// the method, over the same bars with the same filters and at the hold it has
/// already been given.
///
/// This is the recorded defect the drift control cannot be built without.
/// `RandomHold::default_params` sets `entryRate = 1.0`, which means "re-enter
/// on the first bar the filters allow after every exit", so the control is in
/// the market for the whole of the gate and took **3.1-3.7x** the trade count
/// of the methods it was a control for (measured 2026-09-23; the hold null was
/// explicitly out of scope of that day's repair and of the cost match the day
/// after). Trade count is not a cosmetic difference here: the instrument's
/// drift is collected in proportion to time in the market, so a control in the
/// market three times as long collects three times the drift however carefully
/// its side ratio is matched.
///
/// **Why this is a search and not the one-probe scaling `matched_rate` uses.**
/// The hold control's count is not proportional to its rate. A cycle is the
/// hold plus the wait for the next draw, so the count goes as
/// `gate_bars / (hold_bars + 1/rate)` — flat in `rate` once `1/rate` is small
/// against the hold, and bounded above by `gate_bars / hold_bars` at
/// `rate = 1`. Scaling a probe linearly would overshoot by whatever the hold
/// contributes. So the rate is found by bisection on the real engine, on `ln`
/// of the rate because the useful range spans three decades, and **the
/// achieved count is reported on the row rather than assumed** — the one
/// arithmetic claim this function makes is monotonicity, and a row whose match
/// lands outside the band is printed as unmatched.
///
/// `matched_rate`'s own arithmetic is deliberately untouched: it was frozen by
/// `docs/hypotheses/2026-09-23-matched-null-repair.md` and moving it would
/// restate every published entry-branch percentile.
///
/// Returns `None` when the control cannot be brought down to the method's
/// count at all — when even `entryRate = 1.0`, the most trades this control can
/// take, is still fewer than the method's. That is a method trading faster than
/// its own hold allows, there is no rate that fixes it, and saying `None` keeps
/// it from being reported as a match that happened.
const HOLD_RATE_FLOOR: f64 = 0.0005;
const HOLD_RATE_STEPS: usize = 14;

fn matched_hold_rate(
    bars: &[Bar],
    rules: &TradingRules,
    filters: &[Filter],
    target_trades: usize,
    guards: Option<&Guards>,
    hold_params: &Params,
) -> Option<f64> {
    use crate::engine::{Range, run_backtest_guarded};
    if target_trades == 0 {
        return None;
    }
    let count_at = |rate: f64| -> usize {
        let mut p = hold_params.clone();
        p.set("entryRate", rate);
        p.set("seed", 1.0);
        let probe = Filtered { inner: &RandomHold, filters: scoped(filters, rules) };
        run_backtest_guarded(bars, &probe, &p, rules, guards, None, Range::default(), None).trades.len()
    };
    // The ceiling first: at rate 1.0 this control takes every hold the gate
    // has room for, and nothing can make it take more.
    if count_at(1.0) <= target_trades {
        return None;
    }
    let (mut lo, mut hi): (f64, f64) = (HOLD_RATE_FLOOR, 1.0);
    let mut best = (1.0f64, count_at(1.0));
    for _ in 0..HOLD_RATE_STEPS {
        let mid = (0.5 * (lo.ln() + hi.ln())).exp();
        let got = count_at(mid);
        if (got as f64 - target_trades as f64).abs() < (best.1 as f64 - target_trades as f64).abs() {
            best = (mid, got);
        }
        if got > target_trades { hi = mid } else { lo = mid }
    }
    Some(best.0)
}

/// The hypothesis's filters with every unscoped `news:` gate scoped to the
/// market's `news_currencies` (`Filter::for_market`). A batch file is
/// parsed before it knows its market, so the scope is applied here, where
/// the rules are — for the method and, through [`matched_rate`], for its
/// null, so both read the same calendar.
fn scoped(filters: &[Filter], rules: &TradingRules) -> Vec<Filter> {
    filters.iter().cloned().map(|f| f.for_market(&rules.news_currencies)).collect()
}

/* ------------------------------------------------ the cost match, 2026-09-24 */

/// Which control a percentile is read against.
///
/// Registered in `docs/hypotheses/2026-09-24-cost-matched-null.md`. Cost as a
/// fraction of risk is `spread / stop`, so a control whose stop is not the
/// method's is not the method's cost either, and a method that merely widens
/// its stop clears such a control without predicting anything: `struct-80`
/// measured **profit factor 0.973 — losing money — at the 98th percentile**,
/// count match 0.95 and inside the band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostMatch {
    /// The control takes the method's own stop. Every new measurement.
    Method,
    /// The control keeps `RandomEntry`'s registered stop (1.5 ATR) whatever
    /// the method uses — what every receipt written before 2026-09-24
    /// measured. **Only** for reproducing a published number so that the
    /// corrected one can be published beside it; never for a new claim.
    RegisteredStop,
}

/// How the control's stop was matched to the method's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopSource {
    /// The method names `stopAtr` and it governs the trades it took: copied
    /// exactly (case 1 of the registration).
    Named,
    /// The method's invalidation is structural — a channel edge, a swing, a
    /// level, or the engine's own ATR fallback — so there is no multiple to
    /// copy and the control takes the method's **realised** median stop in
    /// ATRs (case 2).
    Realised,
    /// Nothing to measure from and no governing `stopAtr`: the control keeps
    /// its registered stop and the cost match on this row is UNMEASURED.
    /// `null` is not `0`, and this is not a match.
    Unmeasured,
    /// The control was deliberately left at its registered stop to reproduce
    /// a published number ([`CostMatch::RegisteredStop`]).
    Registered,
}

impl StopSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Named => "named stopAtr, copied",
            Self::Realised => "realised median, structural",
            Self::Unmeasured => "UNMEASURED: no trade to measure and no governing stopAtr",
            Self::Registered => "the control's registered stop (pre-2026-09-24 reading)",
        }
    }
}

/// The stop the control takes, and what it was taken from.
#[derive(Debug, Clone, Copy)]
pub struct ControlStop {
    /// In ATRs of the series the engine sized the method's trades with.
    pub atr: f64,
    pub source: StopSource,
    /// What the method's own parameters declared, when it declares one.
    pub named: Option<f64>,
    /// The method's realised median stop distance, in ATRs and in points.
    /// `NaN` when it could not be measured — not zero.
    pub realised_atr: f64,
    pub realised_points: f64,
    /// How many of the method's trades the realised figure came from.
    pub measured_trades: usize,
}

impl ControlStop {
    /// The control's stop in points, at the method's own median ATR — the
    /// same distance the ATR column states, in the units a spread is quoted
    /// in. `NaN` when nothing was measured.
    #[must_use]
    pub fn points(&self) -> f64 {
        if self.realised_atr.is_finite() && self.realised_atr > 0.0 {
            self.atr * self.realised_points / self.realised_atr
        } else {
            f64::NAN
        }
    }

    /// Cost as a fraction of risk at this stop: `spread / stop`, the quantity
    /// the whole defect is about. `NaN` when the stop is not in points.
    #[must_use]
    pub fn cost_fraction_of_r(&self, rules: &TradingRules) -> f64 {
        let points = self.points();
        if points.is_finite() && points > 0.0 { rules.spread / points } else { f64::NAN }
    }
}

/// How far a realised median stop may sit from the method's declared
/// `stopAtr` before the declared value is read as **not governing** the stop
/// the method actually placed.
///
/// `far-stop-break` is why this is a measurement and not a lookup: it
/// declares `stopAtr = 1.2` and, in `stopMode = 0`, ignores it and stops at
/// the opposite edge of the channel — three to seven times further out. A fix
/// that asked only "does the method name `stopAtr`?" would have copied 1.2
/// onto the control of the very row that exposed the defect. An ATR-stop
/// method's realised median differs from its declared multiple only by the
/// gap between the signal bar's close and the next bar's open plus half a
/// spread, which is a few percent of a stop; a structural one differs by
/// multiples.
pub const STOP_GOVERNS_BAND: f64 = 0.25;

/// The method's realised stop distance on the trades it took: (median in
/// ATRs, median in points, trades measured).
///
/// Measured exactly the way the engine measured risk when it sized those
/// trades — `|entry_price - stop|` over the ATR of the bar **before** the
/// fill, from the same series `sizing_atr_key` names — so the figure is the
/// method's own risk unit and not a second opinion about it.
///
/// `None` when there is nothing to measure, and `None` when a trailing stop
/// is enabled: `Trade::stop` is then wherever the stop was trailed to and no
/// longer the distance the trade was sized on. A guessed value would be the
/// null, which is the mistake `hold_distribution`'s comments record twice.
fn realised_stop(
    bars: &[Bar],
    params: &Params,
    rules: &TradingRules,
    trades: &[Trade],
) -> Option<(f64, f64, usize)> {
    if trades.is_empty() || bars.is_empty() || rules.trail.enabled {
        return None;
    }
    let key = crate::engine::sizing_atr_key(params, rules);
    let period: f64 = key.trim_start_matches("atr_").trim_end_matches(".atr").parse().ok()?;
    let series = fd_indicators::compute_indicators(bars, &[IndicatorSpec::new("atr").with("period", period)]).ok()?;
    let atr = series.get(&key)?;

    let mut in_atrs: Vec<f64> = Vec::with_capacity(trades.len());
    let mut in_points: Vec<f64> = Vec::with_capacity(trades.len());
    for trade in trades {
        // The bars are ascending in time, so the fill is found rather than
        // scanned for; a trade whose entry bar is not in this window is not
        // this window's evidence and is skipped.
        let Ok(i) = bars.binary_search_by(|b| b.time.cmp(&trade.entry_time)) else { continue };
        let Some(&a) = atr.get(i.saturating_sub(1)) else { continue };
        let distance = (trade.entry_price - trade.stop).abs();
        if !a.is_finite() || a <= 0.0 || !distance.is_finite() || distance <= 0.0 {
            continue;
        }
        in_atrs.push(distance / a);
        in_points.push(distance);
    }
    if in_atrs.is_empty() {
        return None;
    }
    let n = in_atrs.len();
    let median = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if v.len() % 2 == 1 { v[v.len() / 2] } else { (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0 }
    };
    Some((median(in_atrs), median(in_points), n))
}

/// The stop to give the control, so that its cost as a fraction of risk is
/// the method's.
///
/// Case 1, the method names `stopAtr` **and it governs**: the value, exact.
/// Case 2, structural: the method's realised median stop in ATRs, which is
/// what `hold_distribution` already does for the hold null — take the
/// method's own realised figure rather than a guess.
fn control_stop(
    method_params: &Params,
    bars: &[Bar],
    rules: &TradingRules,
    trades: &[Trade],
    cost_match: CostMatch,
) -> ControlStop {
    let registered = RandomEntry.default_params().get("stopAtr");
    let named = method_params.0.get("stopAtr").copied().filter(|v| v.is_finite() && *v > 0.0);
    let measured = realised_stop(bars, method_params, rules, trades);
    let (realised_atr, realised_points, measured_trades) =
        measured.map_or((f64::NAN, f64::NAN, 0), |(a, p, n)| (a, p, n));

    if cost_match == CostMatch::RegisteredStop {
        return ControlStop {
            atr: registered,
            source: StopSource::Registered,
            named,
            realised_atr,
            realised_points,
            measured_trades,
        };
    }
    // A declared multiple that the realised distance agrees with is copied
    // exactly; one the realised distance contradicts did not place the stop.
    let governs = match (named, realised_atr.is_finite()) {
        (Some(v), true) => (realised_atr / v - 1.0).abs() <= STOP_GOVERNS_BAND,
        (Some(_), false) => true, // nothing measured: the declared value is all there is
        (None, _) => false,
    };
    match (governs, named, realised_atr.is_finite()) {
        (true, Some(v), _) => {
            ControlStop { atr: v, source: StopSource::Named, named, realised_atr, realised_points, measured_trades }
        }
        (false, _, true) => ControlStop {
            atr: realised_atr,
            source: StopSource::Realised,
            named,
            realised_atr,
            realised_points,
            measured_trades,
        },
        _ => ControlStop {
            atr: registered,
            source: StopSource::Unmeasured,
            named,
            realised_atr,
            realised_points,
            measured_trades,
        },
    }
}

/// The stop in ATRs a control was given, or `NaN` when it has none (the hold
/// null). `NaN` is what [`matched_rate`] reads as "leave the probe's own".
fn stop_atr(stop: Option<&ControlStop>) -> f64 {
    stop.map_or(f64::NAN, |s| s.atr)
}

/// One control, built for one method at one seed.
struct Control {
    inner: &'static dyn Strategy,
    params: Params,
    /// Parameters computed from the method, which a sweep must not overwrite.
    calibrated: Vec<String>,
    /// Grid axes whose VALUES were scaled to the method's stop. The control
    /// keeps the axis — and therefore the same room to find something
    /// flattering that a real method gets — around the method's stop instead
    /// of around 1.5.
    rescaled: BTreeMap<String, Vec<f64>>,
}

impl Control {
    fn preset(&self) -> Preset<'static> {
        Preset {
            inner: self.inner,
            defaults: self.params.clone(),
            pinned: self.calibrated.clone(),
            rescaled: self.rescaled.clone(),
        }
    }
}

/// `realised_hold` is the method's own hold distribution on this window, as
/// `hold_distribution` measures it: (geometric mean in minutes, standard
/// deviation of ln minutes). It sets the null's `holdMinutes` (log-median)
/// and `holdLogSd`; every other source of a hold length is a fixed hold.
///
/// `stop` is the stop the random-entry control takes, in ATRs — the method's
/// own, so that the control pays the same `spread / stop`. The hold null does
/// not take one: `RandomHold` has no stop, which is the point of it.
/// `realised_long_share` is the method's own long share on this window, when
/// the caller has measured it and asked for a control that matches it. It is
/// `None` on the coin-flip default, and `None` is the only thing that leaves
/// the control's `longShare` at 0.5 — so a caller that forgets to measure gets
/// the old number rather than a half-repaired one.
///
/// `hold_rate` is the hold control's calibrated entry rate, from
/// [`matched_hold_rate`]. `None` leaves it at 1.0, which is the rate every
/// published hold-null percentile was read against and is the recorded
/// count-match defect.
fn control_for(
    base: &dyn Strategy,
    overrides: &[(String, f64)],
    seed: f64,
    realised_hold: Option<(f64, f64)>,
    stop: Option<&ControlStop>,
    realised_long_share: Option<f64>,
    hold_rate: Option<f64>,
) -> (&'static dyn Strategy, Params) {
    // Judged on the preset, not the bare method: a pinned grid is empty.
    let preset = Preset::new(base, overrides).ok();
    let pinned = preset.as_ref().map(|p| p.defaults.clone());
    let grid_empty = preset.as_ref().is_some_and(|p| p.grid().is_empty());
    let drift = base.exits() == fd_strategy::registry::Exits::Strategy && grid_empty;
    if drift {
        let get = |k: &str| overrides.iter().find(|(key, _)| key == k).map(|(_, v)| *v);
        let minutes = |v: f64| (v as i64 / 100) * 60 + v as i64 % 100;
        // Hold length: a window's span when the preset names one; otherwise
        // what the method actually held on this window, when the caller has
        // run it — the realised hold *distribution*, geometric mean as the
        // log-median and the sd of the logs as the spread; failing that, half
        // the lookback for a rebalanced hold, 390 minutes otherwise. The
        // guess was the null for tsmom until 2026-09-13: it held 27% less
        // than the 20-day row and 2.4x more than the 120-day row
        // (`2026-09-13-tsmom-silver.md`). The mean, fixed, was the null until
        // 2026-09-14: it could not produce the 356-day hold that carried the
        // EURUSD row, so its PF was bounded where the method's was not
        // (`2026-09-14-tsmom-eurusd.md`).
        let (hold, log_sd) = match (get("from"), get("to"), realised_hold, get("lookbackDays")) {
            (Some(from), Some(to), _, _) => {
                let (a, b) = (minutes(from), minutes(to));
                (if b > a { b - a } else { 1440 - a + b }, 0.0)
            }
            (_, _, Some((median, log_sd)), _) if median.is_finite() && median > 0.0 => {
                (median.round() as i64, if log_sd.is_finite() && log_sd > 0.0 { log_sd } else { 0.0 })
            }
            (_, _, _, Some(days)) => ((days * 1440.0 / 2.0) as i64, 0.0),
            _ => (390, 0.0),
        };
        let mut p = RandomHold.default_params();
        p.set("holdMinutes", hold as f64);
        p.set("holdLogSd", log_sd);
        p.set("seed", seed);
        // Sized like the method when the method sizes on daily ranges.
        if let Some(pinned) = &pinned
            && pinned.contains("riskDailyRanges")
        {
            p.set("riskDailyRanges", pinned.get("riskDailyRanges"));
            p.set("rangeDays", pinned.get("rangeDays"));
        }
        set_long_share(&mut p, realised_long_share);
        // The time in the market, which is the other half of the drift
        // exposure. Left at 1.0 the control fills the gate; see
        // `matched_hold_rate`.
        if let Some(rate) = hold_rate
            && rate.is_finite()
            && rate > 0.0
            && rate <= 1.0
        {
            p.set("entryRate", rate);
        }
        (&RandomHold, p)
    } else {
        let mut p = RandomEntry.default_params();
        p.set("seed", seed);
        // The method's own stop, so the control pays the same cost as a
        // fraction of risk. Until 2026-09-24 this branch overrode `seed` and
        // nothing else, so the control stopped at 1.5 ATR whatever the method
        // did, and a method that merely widened its stop cleared its own null
        // (`docs/hypotheses/2026-09-24-cost-matched-null.md`).
        if let Some(stop) = stop
            && stop.atr.is_finite()
            && stop.atr > 0.0
        {
            p.set("stopAtr", stop.atr);
        }
        set_long_share(&mut p, realised_long_share);
        (&RandomEntry, p)
    }
}

/// Put a measured long share on a control, or leave the coin alone.
///
/// A share outside 0-1 or non-finite is **refused rather than clamped**: it can
/// only come from a measurement fault, and clamping would publish a control
/// whose side ratio silently is not the method's. Refusing leaves 0.5, which
/// the receipt then prints as the achieved share, so the mismatch is visible
/// instead of being absorbed.
fn set_long_share(p: &mut Params, share: Option<f64>) {
    if let Some(share) = share
        && share.is_finite()
        && (0.0..=1.0).contains(&share)
    {
        p.set("longShare", share);
    }
}

/// The realised hold distribution of a trade list: (geometric mean of the
/// holds in minutes, population standard deviation of ln minutes). `None`
/// when no trade held for a positive time.
///
/// Geometric, not arithmetic: the null draws its holds log-normally, and
/// the geometric mean is the log-median that draw is centred on — so the
/// null's holds sit where the method's typical hold sits and spread as far
/// as the method's longest. The arithmetic mean would centre the null above
/// the typical hold and still stop short of the tail.
fn hold_distribution(trades: &[crate::engine::Trade]) -> Option<(f64, f64)> {
    let logs: Vec<f64> = trades
        .iter()
        .map(|t| (t.exit_time - t.entry_time) as f64 / 60_000.0)
        .filter(|minutes| *minutes > 0.0)
        .map(f64::ln)
        .collect();
    if logs.is_empty() {
        return None;
    }
    let n = logs.len() as f64;
    let mean = logs.iter().sum::<f64>() / n;
    let variance = logs.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / n;
    Some((mean.exp(), variance.sqrt()))
}

/// `control_for`, with the random-entry rate matched to the method's count
/// and its stop matched to the method's distance.
///
/// `calibrated` is the list of parameters this function computed from the
/// method, which is what [`Preset::calibrated`] must pin so that a
/// walk-forward's sweep cannot put a grid value back. It is empty when
/// nothing was calibrated — a hold null has no rate, and its grid is empty
/// anyway.
///
/// The stop is handled differently from the rate, and the difference is the
/// whole of the judgement in this repair. **Pinning** the stop would empty
/// the control's grid (the rate is already pinned), so the control would lose
/// the selection advantage it is deliberately given, and every walk-forward
/// percentile in the record would move — including the rows at 1.5 ATR, which
/// the registration names as the falsifier. **Leaving** it alone would let
/// `RandomEntry::grid`'s `stopAtr` axis put 1.0/1.5/2.0 back into every cell
/// a sweep can choose, which is exactly how the rate calibration was thrown
/// away before 2026-09-23, and the repair would be inert on every swept path.
/// So the axis is **rescaled**: the same three relative values the control
/// has always swept, around the method's stop instead of around 1.5. At a
/// method stop of 1.5 the scale factor is exactly 1.0 and every cell is
/// bit-identical to what the record measured.
#[allow(clippy::too_many_arguments)]
fn matched_control_for(
    base: &dyn Strategy,
    overrides: &[(String, f64)],
    seed: f64,
    rate: Option<f64>,
    realised_hold: Option<(f64, f64)>,
    stop: Option<&ControlStop>,
    realised_long_share: Option<f64>,
    hold_rate: Option<f64>,
) -> Control {
    let (inner, mut p) = control_for(base, overrides, seed, realised_hold, stop, realised_long_share, hold_rate);
    let mut calibrated = Vec::new();
    if let Some(rate) = rate
        && p.contains("entryRate")
    {
        p.set("entryRate", rate);
        calibrated.push("entryRate".to_string());
    }
    // `longShare` and the hold branch's rate are calibrated from the method
    // too, so they are named for the same reason `entryRate` is: neither
    // control's grid carries either axis today, and a grid that gained one
    // later would otherwise silently discard the measurement — which is
    // exactly how the entry-rate match was lost between 2026-09-13 and
    // 2026-09-23.
    if realised_long_share.is_some() && p.contains("longShare") {
        calibrated.push("longShare".to_string());
    }
    if hold_rate.is_some() && rate.is_none() && p.contains("entryRate") {
        calibrated.push("entryRate".to_string());
    }
    let mut rescaled = BTreeMap::new();
    if let Some(stop) = stop
        && p.contains("stopAtr")
    {
        let registered = RandomEntry.default_params().get("stopAtr");
        let scale = stop.atr / registered;
        // Exactly 1.0 multiplies to bit-identical values; anything else is a
        // new axis and is stated as one.
        if scale.is_finite() && scale > 0.0 && scale != 1.0 {
            if let Some(axis) = inner.grid().get("stopAtr") {
                rescaled.insert("stopAtr".to_string(), axis.iter().map(|v| v * scale).collect());
            }
        }
    }
    Control { inner, params: p, calibrated, rescaled }
}

/// A method with some defaults replaced: a preset, as a strategy.
///
/// A preset on a parameter the method also sweeps **pins** it: the grid
/// loses that axis. Otherwise the walk-forward would replace the registered
/// value with the grid's and the row would test something else — which it
/// did, once (`2026-09-13-vwap-fade.md`). Pinned means *named in the
/// overrides*, whatever the value: a preset that names the default is still
/// a preset (the first gap-fade and tsmom-2 receipts were re-selected because
/// the pin was read as "differs from the default", 2026-09-13).
pub struct Preset<'a> {
    pub inner: &'a dyn Strategy,
    pub defaults: Params,
    pub pinned: Vec<String>,
    /// Grid axes whose **values** are replaced rather than removed. A pin
    /// takes an axis away; this keeps the axis and moves it. It exists for one
    /// reason: a cost-matched control has to sweep its stop around the
    /// method's stop rather than around its own registered 1.5, and taking
    /// the axis away instead would change what every swept control in the
    /// record was allowed to select (`matched_control_for`). Empty on every
    /// method; only a control ever carries one.
    pub rescaled: BTreeMap<String, Vec<f64>>,
}

impl<'a> Preset<'a> {
    /// The method with `overrides` applied and every override pinned.
    pub fn new(inner: &'a dyn Strategy, overrides: &[(String, f64)]) -> Result<Self, String> {
        Ok(Self {
            inner,
            defaults: preset_params(inner, overrides)?,
            pinned: overrides.iter().map(|(k, _)| k.clone()).collect(),
            rescaled: BTreeMap::new(),
        })
    }
    /// The method as it is: nothing overridden, nothing pinned.
    pub fn bare(inner: &'a dyn Strategy, defaults: Params) -> Self {
        Self { inner, defaults, pinned: Vec::new(), rescaled: BTreeMap::new() }
    }
    /// A control at defaults some of which were **calibrated** rather than
    /// registered, with those named so a sweep cannot overwrite them.
    ///
    /// [`Preset::bare`] pins nothing, which is right for a method being
    /// replayed as it stands and wrong for a control whose whole point is a
    /// parameter computed from the method it is a control for. Wrapping the
    /// calibrated control in `bare` and then walking it forward let
    /// `RandomEntry::grid`'s `entryRate` axis replace the calibrated rate in
    /// every cell the selection could choose, so the count-matching was
    /// discarded on every walk-forward matched null this desk published
    /// before 2026-09-23
    /// (`docs/decisions/2026-09-23-matched-null-repair.md`). The axis that is
    /// *not* named here — `stopAtr` — stays in the grid on purpose: the
    /// control is meant to get the same selection advantage a real method
    /// gets, and only the rate is calibrated. Since 2026-09-24 that axis is
    /// **rescaled** to the method's stop rather than pinned, for the same
    /// reason: pinning it would take the advantage away.
    pub fn calibrated(inner: &'a dyn Strategy, defaults: Params, pinned: Vec<String>) -> Self {
        Self { inner, defaults, pinned, rescaled: BTreeMap::new() }
    }
}

impl Strategy for Preset<'_> {
    fn id(&self) -> &'static str {
        self.inner.id()
    }
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn description(&self) -> &'static str {
        self.inner.description()
    }
    fn default_params(&self) -> Params {
        self.defaults.clone()
    }
    fn grid(&self) -> std::collections::BTreeMap<String, Vec<f64>> {
        self.inner
            .grid()
            .into_iter()
            .filter(|(key, _)| !self.pinned.contains(key))
            .map(|(key, values)| match self.rescaled.get(&key) {
                Some(moved) => (key, moved.clone()),
                None => (key, values),
            })
            .collect()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        self.inner.indicators(p)
    }
    fn warmup(&self, p: &Params) -> usize {
        self.inner.warmup(p)
    }
    fn series(&self, p: &Params) -> Vec<String> {
        self.inner.series(p)
    }
    fn needs_options(&self) -> bool {
        self.inner.needs_options()
    }
    fn exits(&self) -> fd_strategy::registry::Exits {
        self.inner.exits()
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        self.inner.on_bar(ctx)
    }
}

/// The hypothesis's defaults: the base method's, with the preset applied.
///
/// An override naming a parameter the method does not declare is an error,
/// not a no-op — a misspelt preset that silently tested the defaults would
/// be reported as if the preset had been tested.
pub fn preset_params(base: &dyn Strategy, overrides: &[(String, f64)]) -> Result<Params, String> {
    let mut defaults = base.default_params();
    for (key, value) in overrides {
        if !defaults.contains(key) {
            return Err(format!("{} has no parameter `{key}`", base.id()));
        }
        defaults.set(key, *value);
    }
    Ok(defaults)
}

/// The hypothesis at its registered parameters over the whole window — no
/// folds, no selection — against a null that gets none either.
///
/// A walk-forward on an out-of-sample window re-selects parameters on that
/// window's own training folds, which is a more generous test than replaying
/// what was registered. This is the replay. `percentile` is against
/// `seeds` random-entry runs wrapped in the same filters, each at the
/// control's defaults with a different seed.
pub fn run_hypothesis_fixed(
    registry: &Registry,
    hypothesis: &Hypothesis,
    bars: &[Bar],
    rules: &TradingRules,
    gate: &PromisingGate,
    seeds: usize,
) -> Result<HypothesisReport, String> {
    run_hypothesis_fixed_guarded(registry, hypothesis, bars, rules, gate, seeds, None)
}

/// [`run_hypothesis_fixed`] with the risk guards applied to the hypothesis
/// and to every null run: the null goes through the same bounded pipeline.
#[allow(clippy::too_many_arguments)]
pub fn run_hypothesis_fixed_guarded(
    registry: &Registry,
    hypothesis: &Hypothesis,
    bars: &[Bar],
    rules: &TradingRules,
    gate: &PromisingGate,
    seeds: usize,
    guards: Option<&Guards>,
) -> Result<HypothesisReport, String> {
    run_hypothesis_fixed_as(registry, hypothesis, bars, rules, gate, seeds, guards, CostMatch::Method, NullSides::default())
}

/// [`run_hypothesis_fixed_guarded`] with the null's sides chosen explicitly.
///
/// The seven-argument form delegates here at [`NullSides::CoinFlip`], so every
/// caller and every published figure keeps the null it was measured against
/// and the drift control is something a caller has to ask for by name.
#[allow(clippy::too_many_arguments)]
pub fn run_hypothesis_fixed_sides(
    registry: &Registry,
    hypothesis: &Hypothesis,
    bars: &[Bar],
    rules: &TradingRules,
    gate: &PromisingGate,
    seeds: usize,
    guards: Option<&Guards>,
    null_sides: NullSides,
) -> Result<HypothesisReport, String> {
    run_hypothesis_fixed_as(registry, hypothesis, bars, rules, gate, seeds, guards, CostMatch::Method, null_sides)
}

/// [`run_hypothesis_fixed_guarded`] against a named control.
///
/// [`CostMatch::Method`] is the measurement; [`CostMatch::RegisteredStop`]
/// reproduces what a receipt written before 2026-09-24 measured, so the two
/// can be printed beside each other. The method's own run is identical either
/// way — it is the same call with the same parameters — so a difference
/// between the two reports in a gate figure is a bug and not a reading.
#[allow(clippy::too_many_arguments)]
pub fn run_hypothesis_fixed_as(
    registry: &Registry,
    hypothesis: &Hypothesis,
    bars: &[Bar],
    rules: &TradingRules,
    gate: &PromisingGate,
    seeds: usize,
    guards: Option<&Guards>,
    cost_match: CostMatch,
    null_sides: NullSides,
) -> Result<HypothesisReport, String> {
    use crate::engine::{Range, run_backtest_guarded};
    let base = registry.get(&hypothesis.base).map_err(|e| e.to_string())?;
    let preset = Preset::new(base, &hypothesis.overrides)?;
    let filtered = Filtered { inner: &preset, filters: scoped(&hypothesis.filters, rules) };
    let result = run_backtest_guarded(bars, &filtered, &preset.defaults, rules, guards, None, Range::default(), None);
    let drift = base.exits() == fd_strategy::registry::Exits::Strategy && preset.grid().is_empty();
    // The stop first, then the rate: the control's stop changes how long its
    // trades last and therefore how many it takes, so a rate calibrated
    // before the stop was set is a rate for a different control.
    let stop = (!drift).then(|| control_stop(&preset.defaults, bars, rules, &result.trades, cost_match));
    let rate = (!drift)
        .then(|| matched_rate(bars, rules, &hypothesis.filters, result.trades.len(), guards, stop_atr(stop.as_ref())));
    let hold = drift.then(|| hold_distribution(&result.trades)).flatten();
    // The method's own long share, on the same trades the percentile is read
    // from. Measured either way so the receipt can print it; fed to the
    // control only when the caller asked for a control that matches it.
    let long_share = side_distribution(&result.trades);
    // WHAT THE CONTROL IS DRAWN AT, and the two settings differ in it.
    // `ratio` takes the method's long share by trade COUNT, which is what
    // `agent/repair-a` built. `exposure` takes the share that reproduces the
    // method's signed share of TIME, because that is the quantity the drift is
    // paid on; see `long_share_for_signed_share` for the measured size of the
    // gap between the two.
    let signed_share = match (signed_exposure_minutes(&result.trades), gross_exposure_minutes(&result.trades)) {
        (Some(signed), Some(gross)) if gross > 0.0 => Some(signed / gross),
        _ => None,
    };
    let control_share = if null_sides.matches_hold_count() {
        signed_share.map(long_share_for_signed_share)
    } else if null_sides.matches_ratio() {
        long_share
    } else {
        None
    };
    // The hold branch's rate, calibrated AFTER the hold and the share are
    // fixed, because both change how many trades a given rate produces. The
    // entry branch keeps `matched_rate`, whose arithmetic is frozen.
    let hold_rate = (drift && null_sides.matches_hold_count())
        .then(|| {
            let (_, probe) = control_for(base, &hypothesis.overrides, 1.0, hold, stop.as_ref(), control_share, None);
            matched_hold_rate(bars, rules, &hypothesis.filters, result.trades.len(), guards, &probe)
        })
        .flatten();

    // This path never sweeps — the control is run at the explicit params
    // below — so the pin costs nothing here and is carried only so that the
    // two paths build their control the same way.
    let mut null: Vec<(f64, usize, f64, f64, f64, f64, Option<f64>)> = (0..seeds)
        .into_par_iter()
        .filter_map(|seed| {
            let built = matched_control_for(
                base,
                &hypothesis.overrides,
                seed as f64 + 1.0,
                rate,
                hold,
                stop.as_ref(),
                control_share,
                hold_rate,
            );
            let control = built.preset();
            let matched = Filtered { inner: &control, filters: scoped(&hypothesis.filters, rules) };
            let run =
                run_backtest_guarded(bars, &matched, &built.params, rules, guards, None, Range::default(), None);
            let pf = run.metrics.profit_factor;
            pf.is_finite().then_some((
                pf,
                run.metrics.trades,
                side_distribution(&run.trades).unwrap_or(f64::NAN),
                signed_exposure_minutes(&run.trades).unwrap_or(f64::NAN),
                gross_cost_usd(&run.trades, rules),
                gross_exposure_minutes(&run.trades).unwrap_or(f64::NAN),
                // The control's OWN fill rate under the same resting order.
                // `None` on a market-entry run, where there is no order.
                run.fills.rate(),
            ))
        })
        .collect();
    null.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let null_pf: Vec<f64> = null.iter().map(|p| p.0).collect();
    let null_long_share: Vec<f64> = null.iter().map(|p| p.2).filter(|s| s.is_finite()).collect();
    // `null_signed_minutes` and `null_gross_minutes` are the ONE pair here that
    // must stay index-aligned, because the signed SHARE is a per-run quotient
    // of the two. So neither is filtered and neither is sorted; `median_f64`
    // sorts its own copy. Sorting these two independently, the way
    // `null_trades` and `null_pf` are sorted, would divide one run's signed
    // minutes by another run's gross minutes and call it a share.
    let null_signed_minutes: Vec<f64> = null.iter().map(|p| p.3).collect();
    let null_gross_minutes: Vec<f64> = null.iter().map(|p| p.5).collect();
    let mut null_cost_usd: Vec<f64> = null.iter().map(|p| p.4).filter(|c| c.is_finite()).collect();
    null_cost_usd.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut null_trades: Vec<usize> = null.iter().map(|p| p.1).collect();
    null_trades.sort_unstable();
    // Sorted on its own, like `null_trades`: a median is all that is read off
    // it, and pairing it with a run's profit factor is a different question
    // from the one this row asks.
    let mut null_fill_rate: Vec<f64> = null.iter().filter_map(|p| p.6).collect();
    null_fill_rate.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let pf = result.metrics.profit_factor;
    let percentile = if null_pf.is_empty() || !pf.is_finite() {
        f64::NAN
    } else {
        100.0 * null_pf.iter().filter(|v| **v < pf).count() as f64 / null_pf.len() as f64
    };
    Ok(HypothesisReport {
        label: hypothesis.label.clone(),
        base: hypothesis.base.clone(),
        filters: filtered.describe(),
        why: hypothesis.why.clone(),
        verdict: verdict(&result.metrics, gate),
        swap_usd: result.trades.iter().map(|t| t.swap_usd).sum(),
        oos: result.metrics,
        null_pf,
        null_trades,
        percentile,
        control_stop: stop,
        null_sides,
        long_share: long_share.unwrap_or(f64::NAN),
        null_long_share,
        signed_minutes: signed_exposure_minutes(&result.trades).unwrap_or(f64::NAN),
        null_signed_minutes,
        gross_minutes: gross_exposure_minutes(&result.trades).unwrap_or(f64::NAN),
        null_gross_minutes,
        cost_usd: gross_cost_usd(&result.trades, rules),
        null_cost_usd,
        skipped_by_guard: result.skipped_by_guard,
        closed_by_guard: result.closed_by_guard,
        sized_down_by_guard: result.sized_down_by_guard,
        fills: result.fills,
        null_fill_rate,
    })
}

/// Walk the hypothesis forward and its matched null `seeds` times.
#[allow(clippy::too_many_arguments)]
pub fn run_hypothesis(
    registry: &Registry,
    hypothesis: &Hypothesis,
    bars: &[Bar],
    rules: &TradingRules,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &PromisingGate,
    seeds: usize,
) -> Result<Option<HypothesisReport>, String> {
    run_hypothesis_guarded(registry, hypothesis, bars, rules, folds, select_by, min_trades_per_cell, gate, seeds, None)
}

/// [`run_hypothesis`] with the risk guards applied to the hypothesis and to
/// every null run.
#[allow(clippy::too_many_arguments)]
pub fn run_hypothesis_guarded(
    registry: &Registry,
    hypothesis: &Hypothesis,
    bars: &[Bar],
    rules: &TradingRules,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &PromisingGate,
    seeds: usize,
    guards: Option<&Guards>,
) -> Result<Option<HypothesisReport>, String> {
    run_hypothesis_sides(
        registry,
        hypothesis,
        bars,
        rules,
        folds,
        select_by,
        min_trades_per_cell,
        gate,
        seeds,
        guards,
        NullSides::default(),
    )
}

/// [`run_hypothesis_guarded`] with the null's sides chosen explicitly. The
/// ten-argument form delegates here at [`NullSides::CoinFlip`], so every
/// published walk-forward percentile keeps the null it was read against.
#[allow(clippy::too_many_arguments)]
pub fn run_hypothesis_sides(
    registry: &Registry,
    hypothesis: &Hypothesis,
    bars: &[Bar],
    rules: &TradingRules,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &PromisingGate,
    seeds: usize,
    guards: Option<&Guards>,
    null_sides: NullSides,
) -> Result<Option<HypothesisReport>, String> {
    let base = registry.get(&hypothesis.base).map_err(|e| e.to_string())?;
    let preset = Preset::new(base, &hypothesis.overrides)?;
    let filtered = Filtered { inner: &preset, filters: scoped(&hypothesis.filters, rules) };
    let Some(result) = walk_forward_guarded(&filtered, bars, rules, None, folds, select_by, min_trades_per_cell, guards)
    else {
        return Ok(None);
    };

    // The control's trade count follows the method's, measured over the whole
    // window at the registered parameters (the walk-forward's own count is a
    // fifth of the bars per fold and would under-match).
    let whole = crate::engine::run_backtest_guarded(
        bars,
        &filtered,
        &preset.defaults,
        rules,
        guards,
        None,
        crate::engine::Range::default(),
        None,
    );
    let drift = base.exits() == fd_strategy::registry::Exits::Strategy && preset.grid().is_empty();
    // The stop before the rate: see `run_hypothesis_fixed_as`. The stop is
    // measured from the same whole-window run the count follows, so both
    // halves of the match are read off one set of the method's own trades.
    let stop = (!drift).then(|| control_stop(&preset.defaults, bars, rules, &whole.trades, CostMatch::Method));
    let rate = (!drift)
        .then(|| matched_rate(bars, rules, &hypothesis.filters, whole.trades.len(), guards, stop_atr(stop.as_ref())));
    let hold = drift.then(|| hold_distribution(&whole.trades)).flatten();
    // The side ratio comes from the **out-of-sample book**, not from the
    // whole-window run the rate is calibrated on. The two differ in what they
    // are: a trade *count* depends on the span of bars, so matching it needs
    // the same span; a *ratio* does not, and the exposure to be matched is the
    // one carried by the trades whose profit factor this row's percentile
    // actually compares.
    let long_share = side_distribution(&result.oos_trades);
    // See `run_hypothesis_fixed_as`: `exposure` is drawn at the share that
    // reproduces the method's signed share of TIME, `ratio` at its long share
    // by trade count.
    let signed_share = match (signed_exposure_minutes(&result.oos_trades), gross_exposure_minutes(&result.oos_trades)) {
        (Some(signed), Some(gross)) if gross > 0.0 => Some(signed / gross),
        _ => None,
    };
    let control_share = if null_sides.matches_hold_count() {
        signed_share.map(long_share_for_signed_share)
    } else if null_sides.matches_ratio() {
        long_share
    } else {
        None
    };
    // The hold branch's rate follows the whole-window count for the same
    // reason `matched_rate` does: a fold's own count is a fifth of the bars
    // and would under-match.
    let hold_rate = (drift && null_sides.matches_hold_count())
        .then(|| {
            let (_, probe) = control_for(base, &hypothesis.overrides, 1.0, hold, stop.as_ref(), control_share, None);
            matched_hold_rate(bars, rules, &hypothesis.filters, whole.trades.len(), guards, &probe)
        })
        .flatten();

    // `Preset::calibrated`, not `Preset::bare`: the walk-forward below sweeps
    // the control's grid, and `RandomEntry`'s grid carries `entryRate`. Wrap
    // the calibrated control in a preset that pins nothing and every cell the
    // sweep can choose carries a grid rate instead of the calibrated one,
    // which is what every matched null published before 2026-09-23 did. The
    // stop axis is not pinned but rescaled to the method's stop, so the sweep
    // cannot put 1.5 back either.
    let mut null: Vec<(f64, usize, f64, f64, f64, f64)> = (0..seeds)
        .into_par_iter()
        .filter_map(|seed| {
            let built = matched_control_for(
                base,
                &hypothesis.overrides,
                seed as f64 + 1.0,
                rate,
                hold,
                stop.as_ref(),
                control_share,
                hold_rate,
            );
            let control = built.preset();
            let matched = Filtered { inner: &control, filters: scoped(&hypothesis.filters, rules) };
            let run = walk_forward_guarded(&matched, bars, rules, None, folds, select_by, min_trades_per_cell, guards)?;
            run.oos.profit_factor.is_finite().then_some((
                run.oos.profit_factor,
                run.oos.trades,
                side_distribution(&run.oos_trades).unwrap_or(f64::NAN),
                signed_exposure_minutes(&run.oos_trades).unwrap_or(f64::NAN),
                gross_cost_usd(&run.oos_trades, rules),
                gross_exposure_minutes(&run.oos_trades).unwrap_or(f64::NAN),
            ))
        })
        .collect();
    null.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let null_pf: Vec<f64> = null.iter().map(|p| p.0).collect();
    let null_long_share: Vec<f64> = null.iter().map(|p| p.2).filter(|s| s.is_finite()).collect();
    // `null_signed_minutes` and `null_gross_minutes` are the ONE pair here that
    // must stay index-aligned, because the signed SHARE is a per-run quotient
    // of the two. So neither is filtered and neither is sorted; `median_f64`
    // sorts its own copy. Sorting these two independently, the way
    // `null_trades` and `null_pf` are sorted, would divide one run's signed
    // minutes by another run's gross minutes and call it a share.
    let null_signed_minutes: Vec<f64> = null.iter().map(|p| p.3).collect();
    let null_gross_minutes: Vec<f64> = null.iter().map(|p| p.5).collect();
    let mut null_cost_usd: Vec<f64> = null.iter().map(|p| p.4).filter(|c| c.is_finite()).collect();
    null_cost_usd.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut null_trades: Vec<usize> = null.iter().map(|p| p.1).collect();
    null_trades.sort_unstable();

    let pf = result.oos.profit_factor;
    let percentile = if null_pf.is_empty() || !pf.is_finite() {
        f64::NAN
    } else {
        100.0 * null_pf.iter().filter(|v| **v < pf).count() as f64 / null_pf.len() as f64
    };

    Ok(Some(HypothesisReport {
        label: hypothesis.label.clone(),
        base: hypothesis.base.clone(),
        filters: filtered.describe(),
        why: hypothesis.why.clone(),
        verdict: verdict(&result.oos, gate),
        swap_usd: result.oos_trades.iter().map(|t| t.swap_usd).sum(),
        oos: result.oos,
        null_pf,
        null_trades,
        percentile,
        control_stop: stop,
        null_sides,
        long_share: long_share.unwrap_or(f64::NAN),
        null_long_share,
        signed_minutes: signed_exposure_minutes(&result.oos_trades).unwrap_or(f64::NAN),
        null_signed_minutes,
        gross_minutes: gross_exposure_minutes(&result.oos_trades).unwrap_or(f64::NAN),
        null_gross_minutes,
        cost_usd: gross_cost_usd(&result.oos_trades, rules),
        null_cost_usd,
        skipped_by_guard: result.skipped_by_guard,
        closed_by_guard: result.closed_by_guard,
        sized_down_by_guard: result.sized_down_by_guard,
        // The walk-forward path aggregates folds and does not carry the
        // per-run order counts. Left at zero rather than guessed: the
        // limit-entry registration reads `--fixed`, and a fill rate this path
        // cannot measure must print as absent and not as 0%.
        fills: crate::engine::LimitFills::default(),
        null_fill_rate: Vec::new(),
    }))
}

/* ----------------------------------- the rebate rescore, 2026-09-23 */

/// One construct measured twice: as the record closed it, and with the
/// introducing-broker rebate credited beside the book.
///
/// Registered in `docs/hypotheses/2026-09-23-rebate-rescore.md`. The gross
/// columns are the ones every receipt in `docs/decisions/` was written from
/// and this type never recomputes them — `oos` is the walk-forward's own
/// `Metrics`, carried through untouched, and `oos_net` is a separate figure
/// taken from a credited *copy* of the same trades.
///
/// **BOTH NULLS CARRY THE REBATE.** `matched_null_net` and `dir_null_net` are
/// the controls' own profit factors with the identical credit applied to the
/// controls' own trades. A credit given to the method and withheld from its
/// control lifts every row equally and produces a percentile that means
/// nothing; the registration fails on that point rather than reporting one.
#[derive(Debug, Clone)]
pub struct RescoreRow {
    pub label: String,
    pub base: String,
    pub filters: String,
    pub why: String,
    /// Walk-forward, out of sample, gross — the number as closed.
    pub oos: Metrics,
    /// The same out-of-sample trades with the credit added.
    pub oos_net: Metrics,
    /// Total credit over those trades, USD.
    pub rebate_usd: f64,
    /// The credit as a fraction of the risk taken, averaged per trade. The
    /// registration's own unit: at a 12-point stop it is about 1% of R.
    pub rebate_frac_r: f64,
    /// The matched null's out-of-sample profit factors, ascending, without
    /// and with the same credit. Same runs, same seeds, two columns.
    pub matched_null_gross: Vec<f64>,
    pub matched_null_net: Vec<f64>,
    /// Out-of-sample trade counts of those same control runs, ascending —
    /// what the calibration achieved rather than what it intended. Read
    /// through [`Self::count_match`].
    pub matched_null_trades: Vec<usize>,
    pub matched_pct_gross: f64,
    pub matched_pct_net: f64,
    /// The whole window at the registered parameters — what `--mode=null-dir`
    /// has always measured the direction control against, and therefore what
    /// the direction percentiles below are percentiles OF.
    pub whole: Metrics,
    pub whole_net: Metrics,
    pub dir_null_gross: Vec<f64>,
    pub dir_null_net: Vec<f64>,
    pub dir_pct_gross: f64,
    pub dir_pct_net: f64,
    /// Whether the direction control permuted the method's own sides (a
    /// method that manages its own exits) or re-ran it with a mirrored side.
    pub dir_self_managed: bool,
    pub verdict_gross: Verdict,
    pub verdict_net: Verdict,
}

impl RescoreRow {
    /// What the count-matching achieved: the control's median out-of-sample
    /// trade count over the method's. See [`COUNT_MATCH_BAND`].
    #[must_use]
    pub fn count_match(&self) -> f64 {
        count_match_ratio(self.oos.trades, &self.matched_null_trades)
    }

    /// Whether [`Self::count_match`] is inside the registration's band.
    #[must_use]
    pub fn count_matched(&self) -> bool {
        let r = self.count_match();
        r.is_finite() && (r - 1.0).abs() <= COUNT_MATCH_BAND
    }

    /// The falsifier, in full: the registry's standing profit-factor gate on
    /// the net figure, and the 95th of **both** nulls, each carrying the same
    /// rebate. All three, or the construct stays closed.
    #[must_use]
    pub fn passes_net(&self) -> bool {
        self.verdict_net.promising && self.matched_pct_net >= 95.0 && self.dir_pct_net >= 95.0
    }

    /// The same three legs without the credit, so a record can say whether a
    /// pass is the rebate's doing or was there all along.
    #[must_use]
    pub fn passes_gross(&self) -> bool {
        self.verdict_gross.promising && self.matched_pct_gross >= 95.0 && self.dir_pct_gross >= 95.0
    }

    #[must_use]
    pub fn null_quantile(curve: &[f64], q: f64) -> f64 {
        if curve.is_empty() {
            return f64::NAN;
        }
        curve[(((curve.len() - 1) as f64) * q).round() as usize]
    }
}

/// Share of `curve` strictly below `value`, 0–100. `NaN` propagates rather
/// than being scored as a percentile, because a profit factor that is not a
/// number is a run that produced no losses, not a run that beat everything.
fn percentile_in(curve: &[f64], value: f64) -> f64 {
    if curve.is_empty() || !value.is_finite() {
        return f64::NAN;
    }
    100.0 * curve.iter().filter(|v| **v < value).count() as f64 / curve.len() as f64
}

/// Run one construct through the registered rescore.
///
/// Everything about the gross path is [`run_hypothesis_guarded`] — the same
/// walk-forward, the same matched null calibrated to the method's trade count,
/// the same seeds — and the net path is that path's own trades with
/// [`Rebate`] credited onto them. The direction control follows
/// `--mode=null-dir`: the whole window at the registered parameters, the
/// method's own sides permuted where the method manages its own exits and a
/// mirrored re-run where the engine does.
///
/// `Ok(None)` means the walk-forward could not run at all (not enough bars),
/// which is reported as "not run" and never as a failure to pass.
#[allow(clippy::too_many_arguments)]
pub fn rescore_hypothesis(
    registry: &Registry,
    hypothesis: &Hypothesis,
    bars: &[Bar],
    rules: &TradingRules,
    folds: usize,
    select_by: SelectBy,
    min_trades_per_cell: usize,
    gate: &PromisingGate,
    seeds: usize,
    direction_samples: usize,
    rebate: Rebate,
    guards: Option<&Guards>,
) -> Result<Option<RescoreRow>, String> {
    use crate::direction::{DirectionFlipped, permuted_sides_profit_factors};
    use crate::engine::{Range, run_backtest_guarded};

    let base = registry.get(&hypothesis.base).map_err(|e| e.to_string())?;
    let preset = Preset::new(base, &hypothesis.overrides)?;
    let filtered = Filtered { inner: &preset, filters: scoped(&hypothesis.filters, rules) };
    let Some(result) = walk_forward_guarded(&filtered, bars, rules, None, folds, select_by, min_trades_per_cell, guards)
    else {
        return Ok(None);
    };
    let equity = rules.starting_equity_usd;

    // The whole window at the registered parameters: the control's trade
    // count follows this rather than the walk-forward's, which is a fifth of
    // the bars per fold and would under-match.
    let whole = run_backtest_guarded(bars, &filtered, &preset.defaults, rules, guards, None, Range::default(), None);
    let drift = base.exits() == fd_strategy::registry::Exits::Strategy && preset.grid().is_empty();
    // The stop before the rate: see `run_hypothesis_fixed_as`.
    let stop = (!drift).then(|| control_stop(&preset.defaults, bars, rules, &whole.trades, CostMatch::Method));
    let rate = (!drift)
        .then(|| matched_rate(bars, rules, &hypothesis.filters, whole.trades.len(), guards, stop_atr(stop.as_ref())));
    let hold = drift.then(|| hold_distribution(&whole.trades)).flatten();

    // ---- the matched null, run once and read twice -----------------------
    //
    // Each seed is one complete walk-forward of the control through the same
    // pipeline. Its OUT-OF-SAMPLE TRADES are what the credit is applied to,
    // so the control is paid exactly what the method is paid. A pair is kept
    // only when both of its figures are finite, so the two curves are the
    // same runs in the same order and the percentiles are comparable.
    // `Preset::calibrated`, not `Preset::bare`: this walk-forward sweeps the
    // control's grid and `RandomEntry`'s grid carries `entryRate`, so a
    // control that pins nothing throws the calibration away
    // (`docs/decisions/2026-09-23-matched-null-repair.md`).
    let mut matched: Vec<(f64, f64, usize)> = (0..seeds)
        .into_par_iter()
        .filter_map(|seed| {
            // `None, None`: the rescore keeps the COIN-FLIP null it published,
            // so this path reproduces `2026-09-23-rebate-rescore.md` exactly.
            // It is not extended to the drift control here because the drift
            // control's scope is settled on the hypothesis paths above and
            // adding a second surface in the same change would make an
            // unattributable result; named rather than left as an omission,
            // and listed in `docs/decisions/2026-10-02-drift-null.md` as not
            // re-run.
            let built =
                matched_control_for(base, &hypothesis.overrides, seed as f64 + 1.0, rate, hold, stop.as_ref(), None, None);
            let control = built.preset();
            let gated = Filtered { inner: &control, filters: scoped(&hypothesis.filters, rules) };
            let run = walk_forward_guarded(&gated, bars, rules, None, folds, select_by, min_trades_per_cell, guards)?;
            let gross = run.oos.profit_factor;
            let net = rebate.credited_metrics(&run.oos_trades, rules, equity).profit_factor;
            (gross.is_finite() && net.is_finite()).then_some((gross, net, run.oos.trades))
        })
        .collect();
    matched.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let matched_null_gross: Vec<f64> = matched.iter().map(|p| p.0).collect();
    let mut matched_null_net: Vec<f64> = matched.iter().map(|p| p.1).collect();
    matched_null_net.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut matched_null_trades: Vec<usize> = matched.iter().map(|p| p.2).collect();
    matched_null_trades.sort_unstable();

    // ---- the direction null, likewise ------------------------------------
    let mut dir: Vec<(f64, f64)> = (0..direction_samples)
        .into_par_iter()
        .filter_map(|seed| {
            let (gross, net) = if drift_sides(base) {
                permuted_sides_profit_factors(&whole.trades, rules, seed as u64 + 1, rebate)
            } else {
                let flipped = DirectionFlipped { inner: &preset, seed: seed as u64 + 1 };
                let gated = Filtered { inner: &flipped, filters: scoped(&hypothesis.filters, rules) };
                let run =
                    run_backtest_guarded(bars, &gated, &preset.defaults, rules, guards, None, Range::default(), None);
                (run.metrics.profit_factor, rebate.credited_metrics(&run.trades, rules, equity).profit_factor)
            };
            (gross.is_finite() && net.is_finite()).then_some((gross, net))
        })
        .collect();
    dir.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let dir_null_gross: Vec<f64> = dir.iter().map(|p| p.0).collect();
    let mut dir_null_net: Vec<f64> = dir.iter().map(|p| p.1).collect();
    dir_null_net.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let oos_net = rebate.credited_metrics(&result.oos_trades, rules, equity);
    let whole_net = rebate.credited_metrics(&whole.trades, rules, equity);

    Ok(Some(RescoreRow {
        label: hypothesis.label.clone(),
        base: hypothesis.base.clone(),
        filters: filtered.describe(),
        why: hypothesis.why.clone(),
        verdict_gross: verdict(&result.oos, gate),
        verdict_net: verdict(&oos_net, gate),
        rebate_usd: rebate.total(&result.oos_trades, rules),
        rebate_frac_r: rebate.mean_fraction_of_r(&result.oos_trades, rules),
        matched_pct_gross: percentile_in(&matched_null_gross, result.oos.profit_factor),
        matched_pct_net: percentile_in(&matched_null_net, oos_net.profit_factor),
        dir_pct_gross: percentile_in(&dir_null_gross, whole.metrics.profit_factor),
        dir_pct_net: percentile_in(&dir_null_net, whole_net.profit_factor),
        dir_self_managed: drift_sides(base),
        oos: result.oos,
        oos_net,
        whole: whole.metrics,
        whole_net,
        matched_null_gross,
        matched_null_net,
        matched_null_trades,
        dir_null_gross,
        dir_null_net,
    }))
}

/// Whether the direction control permutes the method's own sides.
///
/// A method that manages its own exits decides them from the side it holds,
/// so replaying it with a coin-flip side keeps the method's exit rule inside
/// the null. `--mode=null-dir` has branched on exactly this since the tsmom-2
/// pass and this is the same test.
fn drift_sides(base: &dyn Strategy) -> bool {
    base.exits() == fd_strategy::registry::Exits::Strategy
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_hold_null_takes_the_realised_hold_over_the_lookback_guess() {
        let registry = Registry::with_builtins();
        let base = registry.get("tsmom").unwrap();
        let overrides = vec![("lookbackDays".to_string(), 20.0), ("riskDailyRanges".to_string(), 2.0), ("rangeDays".to_string(), 20.0)];
        let (_, guessed) = super::control_for(base, &overrides, 1.0, None, None, None, None);
        assert_eq!(guessed.get("holdMinutes"), 20.0 * 1440.0 / 2.0, "no realised hold: half the lookback");
        assert_eq!(guessed.get("holdLogSd"), 0.0, "a guessed hold is a fixed hold");
        let (_, realised) = super::control_for(base, &overrides, 1.0, Some((19_829.4, 0.8)), None, None, None);
        assert_eq!(realised.get("holdMinutes"), 19_829.0, "the method's own log-median hold, rounded");
        assert_eq!(realised.get("holdLogSd"), 0.8, "and the spread of its logs");
        assert_eq!(realised.get("riskDailyRanges"), 2.0, "sized like the method");
        let (_, degenerate) = super::control_for(base, &overrides, 1.0, Some((19_829.4, f64::NAN)), None, None, None);
        assert_eq!(degenerate.get("holdLogSd"), 0.0, "a spread that is not a number falls back to the fixed hold");
    }

    /// A trade that held for `minutes`; nothing else about it matters here.
    fn held_for(minutes: i64) -> crate::engine::Trade {
        crate::engine::Trade {
            direction: fd_strategy::registry::Side::Long,
            entry_time: 1_600_000_000_000,
            entry_price: 1.0,
            exit_time: 1_600_000_000_000 + minutes * 60_000,
            exit_price: 1.0,
            exit_reason: "hold elapsed".into(),
            exit_kind: crate::engine::ExitKind::Signal,
            stop: f64::NAN,
            target: None,
            lots: 1.0,
            pnl_usd: 0.0,
            swap_usd: 0.0,
            r: 0.0,
            mae: 0.0,
            mfe: 0.0,
            hold_ms: minutes * 60_000,
            reason: "test".into(),
            contract_size: Some(1.0),
            spread: Some(0.0),
        }
    }

    /// The same trade with a side, so a side ratio and a signed exposure can be
    /// built from it.
    fn sided(long: bool, minutes: i64) -> crate::engine::Trade {
        crate::engine::Trade {
            direction: if long { fd_strategy::registry::Side::Long } else { fd_strategy::registry::Side::Short },
            ..held_for(minutes)
        }
    }

    #[test]
    fn the_hold_distribution_is_the_geometric_mean_and_the_sd_of_the_logs() {
        assert_eq!(super::hold_distribution(&[]), None, "no trades, no distribution");
        let (median, log_sd) = super::hold_distribution(&[held_for(1_000), held_for(10_000)]).unwrap();
        assert!((median - 3_162.28).abs() < 0.01, "geometric mean of 1,000 and 10,000 is sqrt(10^7) = 3,162.28, got {median}");
        assert!((log_sd - 1.1513).abs() < 0.001, "population sd of ln(1000), ln(10000) is ln(10)/2 = 1.1513, got {log_sd}");
        let (same, none) = super::hold_distribution(&[held_for(390), held_for(390), held_for(390)]).unwrap();
        assert!((same - 390.0).abs() < 1e-9 && none < 1e-9, "identical holds: their value, no spread (got {same}, {none})");
        assert_eq!(super::hold_distribution(&[held_for(0)]), None, "a zero-length hold has no log and is not a hold");
    }

    use super::*;

    #[test]
    fn the_batches_name_only_registered_methods_and_give_every_entry_a_reason() {
        let registry = Registry::with_builtins();
        for h in gold_intraday_batch() {
            assert!(registry.get(&h.base).is_ok(), "{} is not a strategy", h.base);
            assert!(!h.why.is_empty(), "{}/{} has no reason", h.label, h.base);
            assert!(h.filters.iter().any(|f| matches!(f, Filter::Flat { .. })), "{}/{} is not intraday", h.label, h.base);
        }
        for name in ["ict-m1", "ict-m5", "ict-oos"] {
            for h in batch(name).unwrap() {
                let strategy = registry.get(&h.base).expect(&h.base);
                preset_params(strategy, &h.overrides).unwrap();
            }
        }
        assert!(batch("nope").is_none());
    }

    #[test]
    fn a_batch_file_parses_filters_and_refuses_a_reasonless_hypothesis() {
        let dir = std::env::temp_dir().join(format!("fd-batch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.toml");
        std::fs::write(
            &good,
            r#"
[run]
in_sample = "xauusd:1m"

[[hypothesis]]
label = "orb/ny"
base = "donchian-breakout"
filters = ["weekdays", "sessions:0930-1130|1300-1500", "flat:1630-1815", "vol:14/100:1.2-99"]
overrides = { period = 12 }
why = "the first hour's range is the day's liquidity"
"#,
        )
        .unwrap();
        let batch = batch_from_file(&good).unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].filters.len(), 4);
        assert!(matches!(batch[0].filters[1], Filter::Sessions(ref w) if w.len() == 2));
        assert_eq!(batch[0].overrides, vec![("period".to_string(), 12.0)]);

        let bad = dir.join("bad.toml");
        std::fs::write(&bad, "[[hypothesis]]\nlabel = \"x\"\nbase = \"ema-cross\"\nwhy = \"  \"\n").unwrap();
        assert!(batch_from_file(&bad).unwrap_err().contains("reason"));

        let registry = Registry::with_builtins();
        let err = preset_params(registry.get("ema-cross").unwrap(), &[("nope".into(), 1.0)]).unwrap_err();
        assert!(err.contains("nope"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_preset_on_a_gridded_parameter_pins_it() {
        let registry = Registry::with_builtins();
        let base = registry.get("ema-cross").unwrap();
        let untouched = Preset::bare(base, base.default_params());
        assert_eq!(untouched.grid().len(), base.grid().len());
        let pinned = Preset::new(base, &[("fast".into(), 13.0)]).unwrap();
        assert!(!pinned.grid().contains_key("fast"), "a pinned axis leaves the grid");
        assert!(pinned.grid().contains_key("slow"));
        // Naming the default is still a pin.
        let at_default = base.default_params().get("fast");
        let named = Preset::new(base, &[("fast".into(), at_default)]).unwrap();
        assert!(!named.grid().contains_key("fast"), "a preset at the default value is still pinned");
    }

    #[test]
    fn a_report_reads_its_null_by_quantile_and_survives_only_on_both_counts() {
        let mut oos = Metrics::empty();
        oos.trades = 100;
        oos.profit_factor = 1.5;
        oos.expectancy = 0.2;
        let report = HypothesisReport {
            label: "x".into(),
            base: "y".into(),
            filters: String::new(),
            why: String::new(),
            verdict: verdict(&oos, &PromisingGate::default()),
            swap_usd: 0.0,
            oos,
            null_pf: vec![0.8, 0.9, 1.0, 1.1, 1.2],
            null_trades: vec![90, 95, 100, 104, 110],
            percentile: 100.0,
            control_stop: None,
            null_sides: NullSides::CoinFlip,
            long_share: 0.5,
            null_long_share: vec![0.48, 0.5, 0.52],
            signed_minutes: 0.0,
            null_signed_minutes: vec![0.0],
            gross_minutes: 600.0,
            null_gross_minutes: vec![600.0],
            cost_usd: 1.0,
            null_cost_usd: vec![1.0],
            skipped_by_guard: BTreeMap::new(),
            closed_by_guard: BTreeMap::new(),
            sized_down_by_guard: 0,
            fills: crate::engine::LimitFills::default(),
            null_fill_rate: Vec::new(),
        };
        assert_eq!(report.null_quantile(0.5), 1.0);
        assert!(report.survives());
        let inside = HypothesisReport { percentile: 60.0, ..report.clone() };
        assert!(!inside.survives(), "a gate pass inside the noise is not a finding");
        assert_eq!(report.guard_activity(), "guards: nothing acted");
        let mut acted = report.clone();
        acted.skipped_by_guard.insert("WEEKEND_FLAT".into(), 3);
        acted.closed_by_guard.insert("OPEN_LOSS_CAP".into(), 2);
        acted.closed_by_guard.insert("WEEKEND_FLAT".into(), 40);
        assert_eq!(acted.guard_activity(), "guards: refused WEEKEND_FLAT 3; closed OPEN_LOSS_CAP 2, WEEKEND_FLAT 40; sized down 0");
    }

    /// The `--null-sides=` flag has to round-trip, and a misspelling has to be
    /// refused rather than silently run the unrepaired null and be published as
    /// a repaired number.
    #[test]
    fn the_null_sides_flag_round_trips_and_refuses_a_misspelling() {
        assert_eq!(NullSides::default(), NullSides::CoinFlip, "the default is what the record was measured against");
        for sides in [NullSides::CoinFlip, NullSides::MatchedRatio, NullSides::MatchedExposure] {
            assert_eq!(NullSides::parse(sides.as_str()).unwrap(), sides);
        }
        assert!(NullSides::parse("drift").is_err(), "a misspelt flag must not fall back to the unrepaired null");
        assert!(!NullSides::CoinFlip.matches_ratio() && !NullSides::CoinFlip.matches_hold_count());
        assert!(NullSides::MatchedRatio.matches_ratio() && !NullSides::MatchedRatio.matches_hold_count());
        assert!(NullSides::MatchedExposure.matches_ratio() && NullSides::MatchedExposure.matches_hold_count());
    }

    #[test]
    fn the_side_ratio_is_the_long_share_of_the_realised_trades() {
        assert_eq!(super::side_distribution(&[]), None, "no trades, no side ratio - and not 0.5 either");
        assert_eq!(super::side_distribution(&[sided(true, 60)]), Some(1.0));
        assert_eq!(super::side_distribution(&[sided(false, 60)]), Some(0.0));
        let mixed: Vec<_> = (0..10).map(|i| sided(i < 7, 60)).collect();
        assert_eq!(super::side_distribution(&mixed), Some(0.7), "seven longs of ten");
    }

    /// Signed time in the market, which is what the drift is paid on. Two books
    /// can share a long share and differ in this, which is the whole reason the
    /// exposure setting exists beside the ratio one.
    #[test]
    fn the_signed_exposure_is_long_minutes_minus_short_minutes() {
        assert_eq!(super::signed_exposure_minutes(&[]), None, "an empty book has no exposure, and that is not zero");
        assert_eq!(super::signed_exposure_minutes(&[sided(true, 90)]), Some(90.0));
        assert_eq!(super::signed_exposure_minutes(&[sided(false, 90)]), Some(-90.0));
        assert_eq!(
            super::signed_exposure_minutes(&[sided(true, 90), sided(false, 90)]),
            Some(0.0),
            "a book that is long and short for the same time collects no drift"
        );
        // Same long share, three times the exposure: the case the ratio match
        // on its own cannot see.
        let thin: Vec<_> = (0..2).map(|_| sided(true, 60)).collect();
        let fat: Vec<_> = (0..6).map(|_| sided(true, 60)).collect();
        assert_eq!(super::side_distribution(&thin), super::side_distribution(&fat), "the same ratio");
        assert_eq!(super::signed_exposure_minutes(&fat).unwrap() / super::signed_exposure_minutes(&thin).unwrap(), 3.0);
    }

    /// The share a control is drawn at under `exposure`, and why it is not the
    /// method's long share.
    #[test]
    fn the_control_is_drawn_at_the_share_that_reproduces_the_signed_time() {
        assert_eq!(super::long_share_for_signed_share(0.0), 0.5, "no signed exposure: the coin");
        assert_eq!(super::long_share_for_signed_share(1.0), 1.0, "long the whole time: long only");
        assert_eq!(super::long_share_for_signed_share(-1.0), 0.0, "short the whole time: short only");
        assert_eq!(super::long_share_for_signed_share(0.478), 0.739, "doji-reversal/asia, measured");
        for impossible in [2.0, -3.0] {
            let got = super::long_share_for_signed_share(impossible);
            assert!((0.0..=1.0).contains(&got), "{impossible} clamps into a share, got {got}");
        }
        // The case the count long share cannot see: equal counts, unequal holds.
        let book = vec![sided(true, 900), sided(false, 100)];
        assert_eq!(super::side_distribution(&book), Some(0.5), "half the TRADES are long");
        let (signed, gross) =
            (super::signed_exposure_minutes(&book).unwrap(), super::gross_exposure_minutes(&book).unwrap());
        assert_eq!(signed / gross, 0.8, "but 80% of the TIME is long, and the drift is paid on the time");
        assert_eq!(super::long_share_for_signed_share(signed / gross), 0.9, "so the control is drawn at 0.9, not 0.5");
    }

    #[test]
    fn the_scope_band_is_forty_to_sixty_inclusive_and_no_book_is_not_one_sided() {
        assert!(!super::side_ratio_is_one_sided(0.50), "an even method is not one-sided");
        assert!(!super::side_ratio_is_one_sided(0.40), "the edges of the band are inside it");
        assert!(!super::side_ratio_is_one_sided(0.60));
        assert!(super::side_ratio_is_one_sided(0.39));
        assert!(super::side_ratio_is_one_sided(0.61));
        assert!(super::side_ratio_is_one_sided(1.0), "long-only is the case this was written for");
        assert!(!super::side_ratio_is_one_sided(f64::NAN), "a row with no trades has no ratio to be outside a band");
    }

    /// The control has to be given the method's measured share and nothing
    /// else, and a share that cannot be a share must not reach it.
    #[test]
    fn the_control_takes_the_measured_long_share_and_refuses_an_impossible_one() {
        let registry = Registry::with_builtins();
        // The stop-and-target branch.
        let entry = registry.get("ema-cross").unwrap();
        let (_, coin) = super::control_for(entry, &[], 1.0, None, None, None, None);
        assert_eq!(coin.get("longShare"), 0.5, "no measured share: the coin the record was read against");
        let (_, one_sided) = super::control_for(entry, &[], 1.0, None, None, Some(0.83), None);
        assert_eq!(one_sided.get("longShare"), 0.83);
        for impossible in [f64::NAN, -0.1, 1.2, f64::INFINITY] {
            let (_, refused) = super::control_for(entry, &[], 1.0, None, None, Some(impossible), None);
            assert_eq!(refused.get("longShare"), 0.5, "{impossible} is not a share and is refused, not clamped");
        }
        // And the hold branch, which is the one every drift row uses.
        let hold_base = registry.get("session-hold").unwrap();
        let (_, hold) = super::control_for(hold_base, &[], 1.0, Some((390.0, 0.4)), None, Some(1.0), None);
        assert_eq!(hold.get("longShare"), 1.0, "a long-only drift method gets a long-only null");
        assert_eq!(hold.get("holdMinutes"), 390.0, "and still the realised hold");
        assert_eq!(hold.get("entryRate"), 1.0, "and without a calibrated rate it still fills its gate, as the record did");
        let (_, counted) = super::control_for(hold_base, &[], 1.0, Some((390.0, 0.4)), None, Some(1.0), Some(0.31));
        assert_eq!(counted.get("entryRate"), 0.31, "with one, it waits");
        for impossible in [f64::NAN, 0.0, -0.2, 1.4] {
            let (_, refused) = super::control_for(hold_base, &[], 1.0, Some((390.0, 0.4)), None, Some(1.0), Some(impossible));
            assert_eq!(refused.get("entryRate"), 1.0, "{impossible} is not a rate and is refused, not clamped");
        }
    }

    /// `longShare` and the hold branch's rate are named as calibrated for the
    /// same reason `entryRate` is on the entry branch: so a grid that later
    /// gained either axis could not discard the measurement.
    #[test]
    fn a_measured_long_share_is_named_as_calibrated_and_the_coin_is_not() {
        let registry = Registry::with_builtins();
        let base = registry.get("ema-cross").unwrap();
        let coin = super::matched_control_for(base, &[], 1.0, Some(0.03), None, None, None, None);
        assert_eq!(coin.calibrated, vec!["entryRate".to_string()], "nothing was calibrated about the coin");
        let ratio = super::matched_control_for(base, &[], 1.0, Some(0.03), None, None, Some(0.9), None);
        assert_eq!(ratio.calibrated, vec!["entryRate".to_string(), "longShare".to_string()]);
        let hold_base = registry.get("session-hold").unwrap();
        let hold = super::matched_control_for(hold_base, &[], 1.0, None, Some((390.0, 0.0)), None, Some(1.0), Some(0.4));
        assert_eq!(hold.calibrated, vec!["longShare".to_string(), "entryRate".to_string()]);
    }

    /// A null every seed of which produced the same profit factor is not a
    /// distribution, and the row has to say so rather than print a percentile.
    #[test]
    fn a_null_whose_seeds_all_agree_is_not_a_distribution() {
        let mut oos = Metrics::empty();
        oos.trades = 100;
        oos.profit_factor = 1.5;
        oos.expectancy = 0.2;
        let row = |curve: Vec<f64>| HypothesisReport {
            label: "x".into(),
            base: "y".into(),
            filters: String::new(),
            why: String::new(),
            verdict: verdict(&oos, &PromisingGate::default()),
            swap_usd: 0.0,
            oos: oos.clone(),
            null_pf: curve,
            null_trades: vec![100],
            percentile: 100.0,
            control_stop: None,
            null_sides: NullSides::MatchedExposure,
            long_share: 1.0,
            null_long_share: vec![1.0],
            signed_minutes: 600.0,
            null_signed_minutes: vec![600.0],
            gross_minutes: 600.0,
            null_gross_minutes: vec![600.0],
            cost_usd: 1.0,
            null_cost_usd: vec![1.0],
            skipped_by_guard: BTreeMap::new(),
            closed_by_guard: BTreeMap::new(),
            sized_down_by_guard: 0,
            fills: crate::engine::LimitFills::default(),
            null_fill_rate: Vec::new(),
        };
        assert!(!row(vec![]).null_has_spread(), "no runs is not a distribution");
        assert!(!row(vec![1.07]).null_has_spread(), "one run is not a distribution");
        assert!(!row(vec![1.07; 300]).null_has_spread(), "300 identical runs are one run 300 times");
        assert!(row(vec![1.07, 1.07, 1.08]).null_has_spread(), "one seed that differs is a distribution");

        // And the reporting rule: `null`, not 0 and not 100.
        assert_eq!(row(vec![1.07; 300]).percentile_or_null(), None);
        assert!(!row(vec![1.07; 300]).survives(), "a gate pass against a point is not a finding");
        assert_eq!(row(vec![1.0, 1.1, 1.2]).percentile_or_null(), Some(100.0));
        assert!(row(vec![1.0, 1.1, 1.2]).survives());
    }

    /// The side match and the exposure match, read off the control's own
    /// achieved figures rather than from what it was asked for.
    #[test]
    fn the_side_and_exposure_matches_are_read_from_the_controls_own_runs() {
        let mut oos = Metrics::empty();
        oos.trades = 100;
        let row = |method: f64, null: Vec<f64>, signed: f64, null_signed: Vec<f64>| HypothesisReport {
            label: "x".into(),
            base: "y".into(),
            filters: String::new(),
            why: String::new(),
            verdict: verdict(&oos, &PromisingGate::default()),
            swap_usd: 0.0,
            oos: oos.clone(),
            null_pf: vec![1.0, 1.1],
            null_trades: vec![100],
            percentile: 50.0,
            control_stop: None,
            null_sides: NullSides::MatchedExposure,
            long_share: method,
            null_long_share: null,
            signed_minutes: signed,
            null_signed_minutes: null_signed.clone(),
            gross_minutes: signed.abs().max(1.0),
            null_gross_minutes: null_signed.iter().map(|m: &f64| m.abs().max(1.0)).collect(),
            cost_usd: 1.0,
            null_cost_usd: vec![1.0],
            skipped_by_guard: BTreeMap::new(),
            closed_by_guard: BTreeMap::new(),
            sized_down_by_guard: 0,
            fills: crate::engine::LimitFills::default(),
            null_fill_rate: Vec::new(),
        };
        let r = |m: f64, n: Vec<f64>| row(m, n, 600.0, vec![600.0]);
        assert_eq!(r(1.0, vec![0.98, 1.0, 1.0]).null_long_share_median(), 1.0);
        assert_eq!(r(1.0, vec![0.9, 1.0]).null_long_share_median(), 0.95, "an even list takes the middle pair");
        assert!(r(1.0, vec![]).null_long_share_median().is_nan(), "no runs is no measurement");
        assert!(r(1.0, vec![0.99]).side_matched());
        assert!(!r(1.0, vec![0.5]).side_matched(), "the coin against a long-only method is the defect, not a match");
        assert!(!r(f64::NAN, vec![0.5]).side_matched(), "a row with no book has no side match");

        // The exposure, which needs BOTH halves. The fixture's gross minutes are
        // |signed| here, so a signed share of +/-1.000 on both sides and the
        // time-in-market ratio carries the whole of the claim.
        assert_eq!(row(1.0, vec![1.0], 600.0, vec![600.0]).signed_share(), 1.0);
        assert!(row(1.0, vec![1.0], 600.0, vec![600.0]).exposure_matched());
        assert!(row(1.0, vec![1.0], 600.0, vec![720.0]).exposure_matched(), "a fifth over is inside the band");
        assert!(
            !row(1.0, vec![1.0], 600.0, vec![2040.0]).exposure_matched(),
            "3.4x the time in the market is the recorded defect, not a match, even at the same signed share"
        );
        assert_eq!(
            row(1.0, vec![1.0], 600.0, vec![2040.0]).null_signed_share_median(),
            1.0,
            "and the signed share ALONE says it is matched, which is why it is not asked alone"
        );
        // A book long and short for equal time has a signed share of 0.000,
        // which is a measurement and not a division by nothing.
        let two_sided = row(0.5, vec![0.5], 0.0, vec![0.0]);
        assert_eq!(two_sided.signed_share(), 0.0, "zero signed exposure is a number, not a NaN");
        assert!(two_sided.exposure_matched(), "and a control with the same zero matches it");
    }

    #[test]
    fn the_cost_match_is_the_spread_the_control_actually_paid() {
        let rules = TradingRules { contract_size: 1.0, spread: 0.28, ..TradingRules::default() };
        assert_eq!(super::gross_cost_usd(&[], &rules), 0.0, "no trades, no spread paid");
        let carried = crate::engine::Trade { lots: 0.5, spread: Some(0.40), contract_size: Some(2.0), ..sided(true, 60) };
        assert_eq!(super::gross_cost_usd(&[carried], &rules), 0.40 * 0.5 * 2.0, "a trade is priced from its own basis");
        let estimate = crate::engine::Trade { lots: 0.5, spread: None, contract_size: None, ..sided(true, 60) };
        assert_eq!(super::gross_cost_usd(&[estimate], &rules), 0.28 * 0.5, "and one that carries none is an estimate");
    }

    #[test]
    fn the_achieved_count_match_is_the_controls_median_over_the_methods_count() {
        assert!(median_count(&[]).is_nan(), "a median of nothing is not zero");
        assert_eq!(median_count(&[7]), 7.0);
        assert_eq!(median_count(&[10, 20]), 15.0, "an even list takes the mean of the middle pair");
        assert_eq!(median_count(&[1, 2, 3]), 2.0);

        assert_eq!(count_match_ratio(100, &[90, 100, 110]), 1.0);
        assert_eq!(count_match_ratio(100, &[40, 50, 60]), 0.5, "a control at half the method's size");
        assert!(count_match_ratio(0, &[10]).is_nan(), "a method that took no trade has no match to report");
        assert!(count_match_ratio(10, &[]).is_nan(), "and neither has a null that produced no run");

        // The registration's band, read the way the test that guards it
        // reads it: within about a quarter either way, and the edges are in.
        let mut oos = Metrics::empty();
        oos.trades = 100;
        let row = |counts: Vec<usize>| HypothesisReport {
            label: "x".into(),
            base: "y".into(),
            filters: String::new(),
            why: String::new(),
            verdict: verdict(&oos, &PromisingGate::default()),
            swap_usd: 0.0,
            oos: oos.clone(),
            null_pf: vec![1.0],
            null_trades: counts,
            percentile: 50.0,
            control_stop: None,
            null_sides: NullSides::CoinFlip,
            long_share: 0.5,
            null_long_share: vec![0.5],
            signed_minutes: 0.0,
            null_signed_minutes: vec![0.0],
            gross_minutes: 600.0,
            null_gross_minutes: vec![600.0],
            cost_usd: 1.0,
            null_cost_usd: vec![1.0],
            skipped_by_guard: BTreeMap::new(),
            closed_by_guard: BTreeMap::new(),
            sized_down_by_guard: 0,
            fills: crate::engine::LimitFills::default(),
            null_fill_rate: Vec::new(),
        };
        assert!(row(vec![100]).count_matched());
        assert!(row(vec![75]).count_matched(), "a quarter under is the edge and the edge is inside");
        assert!(row(vec![125]).count_matched(), "and so is a quarter over");
        assert!(!row(vec![74]).count_matched());
        assert!(!row(vec![126]).count_matched());
        assert!(!row(vec![]).count_matched(), "no null runs is not a match, it is no measurement");
    }
}
