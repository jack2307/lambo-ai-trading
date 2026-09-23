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
use crate::engine::{Metrics, TradingRules};
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
    pub verdict: Verdict,
    /// How often a guard acted on the hypothesis's own out-of-sample runs
    /// (not the null's): entries refused by label, positions closed by
    /// label, entries sized down. All empty/zero on an unguarded run.
    pub skipped_by_guard: BTreeMap<String, usize>,
    pub closed_by_guard: BTreeMap<String, usize>,
    pub sized_down_by_guard: usize,
}

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

    #[must_use]
    pub fn null_quantile(&self, q: f64) -> f64 {
        if self.null_pf.is_empty() {
            return f64::NAN;
        }
        self.null_pf[(((self.null_pf.len() - 1) as f64) * q).round() as usize]
    }

    /// Outside the noise *and* past the gate. Both, or it is not a finding.
    #[must_use]
    pub fn survives(&self) -> bool {
        self.verdict.promising && self.percentile >= 95.0
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
fn matched_rate(bars: &[Bar], rules: &TradingRules, filters: &[Filter], target_trades: usize, guards: Option<&Guards>) -> f64 {
    use crate::engine::{Range, run_backtest_guarded};
    const PROBE: f64 = 0.02;
    let mut p = RandomEntry.default_params();
    p.set("entryRate", PROBE);
    p.set("seed", 1.0);
    let probe = Filtered { inner: &RandomEntry, filters: scoped(filters, rules) };
    let got = run_backtest_guarded(bars, &probe, &p, rules, guards, None, Range::default(), None).trades.len();
    if got == 0 || target_trades == 0 {
        return PROBE;
    }
    (PROBE * target_trades as f64 / got as f64).clamp(0.0005, 1.0)
}

/// The hypothesis's filters with every unscoped `news:` gate scoped to the
/// market's `news_currencies` (`Filter::for_market`). A batch file is
/// parsed before it knows its market, so the scope is applied here, where
/// the rules are — for the method and, through [`matched_rate`], for its
/// null, so both read the same calendar.
fn scoped(filters: &[Filter], rules: &TradingRules) -> Vec<Filter> {
    filters.iter().cloned().map(|f| f.for_market(&rules.news_currencies)).collect()
}

/// `realised_hold` is the method's own hold distribution on this window, as
/// `hold_distribution` measures it: (geometric mean in minutes, standard
/// deviation of ln minutes). It sets the null's `holdMinutes` (log-median)
/// and `holdLogSd`; every other source of a hold length is a fixed hold.
fn control_for(
    base: &dyn Strategy,
    overrides: &[(String, f64)],
    seed: f64,
    realised_hold: Option<(f64, f64)>,
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
        (&RandomHold, p)
    } else {
        let mut p = RandomEntry.default_params();
        p.set("seed", seed);
        (&RandomEntry, p)
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

/// `control_for`, with the random-entry rate matched to the method's count.
///
/// The third element is the list of parameters this function **calibrated**,
/// which is what [`Preset::calibrated`] must pin so that a walk-forward's
/// sweep cannot put a grid value back. It is empty when nothing was
/// calibrated — a hold null has no rate, and its grid is empty anyway.
fn matched_control_for(
    base: &dyn Strategy,
    overrides: &[(String, f64)],
    seed: f64,
    rate: Option<f64>,
    realised_hold: Option<(f64, f64)>,
) -> (&'static dyn Strategy, Params, Vec<String>) {
    let (inner, mut p) = control_for(base, overrides, seed, realised_hold);
    let mut calibrated = Vec::new();
    if let Some(rate) = rate
        && p.contains("entryRate")
    {
        p.set("entryRate", rate);
        calibrated.push("entryRate".to_string());
    }
    (inner, p, calibrated)
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
}

impl<'a> Preset<'a> {
    /// The method with `overrides` applied and every override pinned.
    pub fn new(inner: &'a dyn Strategy, overrides: &[(String, f64)]) -> Result<Self, String> {
        Ok(Self { inner, defaults: preset_params(inner, overrides)?, pinned: overrides.iter().map(|(k, _)| k.clone()).collect() })
    }
    /// The method as it is: nothing overridden, nothing pinned.
    pub fn bare(inner: &'a dyn Strategy, defaults: Params) -> Self {
        Self { inner, defaults, pinned: Vec::new() }
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
    /// gets, and only the rate is calibrated.
    pub fn calibrated(inner: &'a dyn Strategy, defaults: Params, pinned: Vec<String>) -> Self {
        Self { inner, defaults, pinned }
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
        self.inner.grid().into_iter().filter(|(key, _)| !self.pinned.contains(key)).collect()
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
    use crate::engine::{Range, run_backtest_guarded};
    let base = registry.get(&hypothesis.base).map_err(|e| e.to_string())?;
    let preset = Preset::new(base, &hypothesis.overrides)?;
    let filtered = Filtered { inner: &preset, filters: scoped(&hypothesis.filters, rules) };
    let result = run_backtest_guarded(bars, &filtered, &preset.defaults, rules, guards, None, Range::default(), None);
    let drift = base.exits() == fd_strategy::registry::Exits::Strategy && preset.grid().is_empty();
    let rate = (!drift).then(|| matched_rate(bars, rules, &hypothesis.filters, result.trades.len(), guards));
    let hold = drift.then(|| hold_distribution(&result.trades)).flatten();

    // This path never sweeps — the control is run at the explicit params
    // below — so the pin costs nothing here and is carried only so that the
    // two paths build their control the same way.
    let mut null: Vec<(f64, usize)> = (0..seeds)
        .into_par_iter()
        .filter_map(|seed| {
            let (inner, defaults, calibrated) =
                matched_control_for(base, &hypothesis.overrides, seed as f64 + 1.0, rate, hold);
            let control = Preset::calibrated(inner, defaults.clone(), calibrated);
            let matched = Filtered { inner: &control, filters: scoped(&hypothesis.filters, rules) };
            let run = run_backtest_guarded(bars, &matched, &defaults, rules, guards, None, Range::default(), None);
            let pf = run.metrics.profit_factor;
            pf.is_finite().then_some((pf, run.metrics.trades))
        })
        .collect();
    null.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let null_pf: Vec<f64> = null.iter().map(|p| p.0).collect();
    let mut null_trades: Vec<usize> = null.iter().map(|p| p.1).collect();
    null_trades.sort_unstable();

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
        skipped_by_guard: result.skipped_by_guard,
        closed_by_guard: result.closed_by_guard,
        sized_down_by_guard: result.sized_down_by_guard,
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
    let rate = (!drift).then(|| matched_rate(bars, rules, &hypothesis.filters, whole.trades.len(), guards));
    let hold = drift.then(|| hold_distribution(&whole.trades)).flatten();

    // `Preset::calibrated`, not `Preset::bare`: the walk-forward below sweeps
    // the control's grid, and `RandomEntry`'s grid carries `entryRate`. Wrap
    // the calibrated control in a preset that pins nothing and every cell the
    // sweep can choose carries a grid rate instead of the calibrated one,
    // which is what every matched null published before 2026-09-23 did.
    let mut null: Vec<(f64, usize)> = (0..seeds)
        .into_par_iter()
        .filter_map(|seed| {
            let (inner, defaults, calibrated) =
                matched_control_for(base, &hypothesis.overrides, seed as f64 + 1.0, rate, hold);
            let control = Preset::calibrated(inner, defaults, calibrated);
            let matched = Filtered { inner: &control, filters: scoped(&hypothesis.filters, rules) };
            let run = walk_forward_guarded(&matched, bars, rules, None, folds, select_by, min_trades_per_cell, guards)?;
            run.oos.profit_factor.is_finite().then_some((run.oos.profit_factor, run.oos.trades))
        })
        .collect();
    null.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let null_pf: Vec<f64> = null.iter().map(|p| p.0).collect();
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
        skipped_by_guard: result.skipped_by_guard,
        closed_by_guard: result.closed_by_guard,
        sized_down_by_guard: result.sized_down_by_guard,
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
    let rate = (!drift).then(|| matched_rate(bars, rules, &hypothesis.filters, whole.trades.len(), guards));
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
            let (inner, defaults, calibrated) =
                matched_control_for(base, &hypothesis.overrides, seed as f64 + 1.0, rate, hold);
            let control = Preset::calibrated(inner, defaults, calibrated);
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
        let (_, guessed) = super::control_for(base, &overrides, 1.0, None);
        assert_eq!(guessed.get("holdMinutes"), 20.0 * 1440.0 / 2.0, "no realised hold: half the lookback");
        assert_eq!(guessed.get("holdLogSd"), 0.0, "a guessed hold is a fixed hold");
        let (_, realised) = super::control_for(base, &overrides, 1.0, Some((19_829.4, 0.8)));
        assert_eq!(realised.get("holdMinutes"), 19_829.0, "the method's own log-median hold, rounded");
        assert_eq!(realised.get("holdLogSd"), 0.8, "and the spread of its logs");
        assert_eq!(realised.get("riskDailyRanges"), 2.0, "sized like the method");
        let (_, degenerate) = super::control_for(base, &overrides, 1.0, Some((19_829.4, f64::NAN)));
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
            skipped_by_guard: BTreeMap::new(),
            closed_by_guard: BTreeMap::new(),
            sized_down_by_guard: 0,
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
            skipped_by_guard: BTreeMap::new(),
            closed_by_guard: BTreeMap::new(),
            sized_down_by_guard: 0,
        };
        assert!(row(vec![100]).count_matched());
        assert!(row(vec![75]).count_matched(), "a quarter under is the edge and the edge is inside");
        assert!(row(vec![125]).count_matched(), "and so is a quarter over");
        assert!(!row(vec![74]).count_matched());
        assert!(!row(vec![126]).count_matched());
        assert!(!row(vec![]).count_matched(), "no null runs is not a match, it is no measurement");
    }
}
