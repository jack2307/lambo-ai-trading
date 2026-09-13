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

use crate::control::RandomEntry;
use crate::control_hold::RandomHold;
use crate::engine::{Metrics, TradingRules};
use crate::sweep::{PromisingGate, SelectBy, Verdict, verdict, walk_forward};

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
    /// Share of null runs the hypothesis beat, 0–100.
    pub percentile: f64,
    pub verdict: Verdict,
}

impl HypothesisReport {
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
fn matched_rate(bars: &[Bar], rules: &TradingRules, filters: &[Filter], target_trades: usize) -> f64 {
    use crate::engine::{Range, run_backtest};
    const PROBE: f64 = 0.02;
    let mut p = RandomEntry.default_params();
    p.set("entryRate", PROBE);
    p.set("seed", 1.0);
    let probe = Filtered { inner: &RandomEntry, filters: filters.to_vec() };
    let got = run_backtest(bars, &probe, &p, rules, None, Range::default()).trades.len();
    if got == 0 || target_trades == 0 {
        return PROBE;
    }
    (PROBE * target_trades as f64 / got as f64).clamp(0.0005, 1.0)
}

fn control_for(base: &dyn Strategy, overrides: &[(String, f64)], seed: f64, realised_hold: Option<f64>) -> (&'static dyn Strategy, Params) {
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
        // run it (the mean realised hold); failing that, half the lookback
        // for a rebalanced hold, 390 minutes otherwise. The guess was the
        // null for tsmom until 2026-09-13: it held 27% less than the 20-day
        // row and 2.4x more than the 120-day row (`2026-09-13-tsmom-silver.md`).
        let hold = match (get("from"), get("to"), realised_hold, get("lookbackDays")) {
            (Some(from), Some(to), _, _) => {
                let (a, b) = (minutes(from), minutes(to));
                if b > a { b - a } else { 1440 - a + b }
            }
            (_, _, Some(minutes), _) if minutes.is_finite() && minutes > 0.0 => minutes.round() as i64,
            (_, _, _, Some(days)) => (days * 1440.0 / 2.0) as i64,
            _ => 390,
        };
        let mut p = RandomHold.default_params();
        p.set("holdMinutes", hold as f64);
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

/// The mean hold of a trade list in minutes; `None` when there are no trades.
fn mean_hold_minutes(trades: &[crate::engine::Trade]) -> Option<f64> {
    if trades.is_empty() {
        return None;
    }
    let total: i64 = trades.iter().map(|t| t.exit_time - t.entry_time).sum();
    Some(total as f64 / trades.len() as f64 / 60_000.0)
}

/// `control_for`, with the random-entry rate matched to the method's count.
fn matched_control_for(
    base: &dyn Strategy,
    overrides: &[(String, f64)],
    seed: f64,
    rate: Option<f64>,
    realised_hold: Option<f64>,
) -> (&'static dyn Strategy, Params) {
    let (inner, mut p) = control_for(base, overrides, seed, realised_hold);
    if let Some(rate) = rate
        && p.contains("entryRate")
    {
        p.set("entryRate", rate);
    }
    (inner, p)
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
#[allow(clippy::too_many_arguments)]
pub fn run_hypothesis_fixed(
    registry: &Registry,
    hypothesis: &Hypothesis,
    bars: &[Bar],
    rules: &TradingRules,
    gate: &PromisingGate,
    seeds: usize,
) -> Result<HypothesisReport, String> {
    use crate::engine::{Range, run_backtest};
    let base = registry.get(&hypothesis.base).map_err(|e| e.to_string())?;
    let preset = Preset::new(base, &hypothesis.overrides)?;
    let filtered = Filtered { inner: &preset, filters: hypothesis.filters.clone() };
    let result = run_backtest(bars, &filtered, &preset.defaults, rules, None, Range::default());
    let drift = base.exits() == fd_strategy::registry::Exits::Strategy && preset.grid().is_empty();
    let rate = (!drift).then(|| matched_rate(bars, rules, &hypothesis.filters, result.trades.len()));
    let hold = drift.then(|| mean_hold_minutes(&result.trades)).flatten();

    let mut null_pf: Vec<f64> = (0..seeds)
        .into_par_iter()
        .filter_map(|seed| {
            let (inner, defaults) = matched_control_for(base, &hypothesis.overrides, seed as f64 + 1.0, rate, hold);
            let control = Preset::bare(inner, defaults.clone());
            let matched = Filtered { inner: &control, filters: hypothesis.filters.clone() };
            let pf = run_backtest(bars, &matched, &defaults, rules, None, Range::default()).metrics.profit_factor;
            pf.is_finite().then_some(pf)
        })
        .collect();
    null_pf.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

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
        percentile,
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
    let base = registry.get(&hypothesis.base).map_err(|e| e.to_string())?;
    let preset = Preset::new(base, &hypothesis.overrides)?;
    let filtered = Filtered { inner: &preset, filters: hypothesis.filters.clone() };
    let Some(result) = walk_forward(&filtered, bars, rules, None, folds, select_by, min_trades_per_cell) else {
        return Ok(None);
    };

    // The control's trade count follows the method's, measured over the whole
    // window at the registered parameters (the walk-forward's own count is a
    // fifth of the bars per fold and would under-match).
    let whole = crate::engine::run_backtest(bars, &filtered, &preset.defaults, rules, None, crate::engine::Range::default());
    let drift = base.exits() == fd_strategy::registry::Exits::Strategy && preset.grid().is_empty();
    let rate = (!drift).then(|| matched_rate(bars, rules, &hypothesis.filters, whole.trades.len()));
    let hold = drift.then(|| mean_hold_minutes(&whole.trades)).flatten();

    let mut null_pf: Vec<f64> = (0..seeds)
        .into_par_iter()
        .filter_map(|seed| {
            let (inner, defaults) = matched_control_for(base, &hypothesis.overrides, seed as f64 + 1.0, rate, hold);
            let control = Preset::bare(inner, defaults);
            let matched = Filtered { inner: &control, filters: hypothesis.filters.clone() };
            walk_forward(&matched, bars, rules, None, folds, select_by, min_trades_per_cell)
                .map(|r| r.oos.profit_factor)
                .filter(|pf| pf.is_finite())
        })
        .collect();
    null_pf.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

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
        percentile,
    }))
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
        let (_, realised) = super::control_for(base, &overrides, 1.0, Some(19_829.4));
        assert_eq!(realised.get("holdMinutes"), 19_829.0, "the method's own mean hold, rounded");
        assert_eq!(realised.get("riskDailyRanges"), 2.0, "sized like the method");
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
            percentile: 100.0,
        };
        assert_eq!(report.null_quantile(0.5), 1.0);
        assert!(report.survives());
        let inside = HypothesisReport { percentile: 60.0, ..report.clone() };
        assert!(!inside.survives(), "a gate pass inside the noise is not a finding");
    }
}
