//! Hypotheses: a base method, a set of filters, and a matched null.
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
//! A batch is declared in code, not searched for. Thirteen hypotheses is a
//! list someone wrote down with a reason each; a thousand is a search, and a
//! search always finds something.

use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::filter::{Filter, Filtered};
use fd_strategy::registry::{BarContext, Intent, Params, Registry, Strategy};
use rayon::prelude::*;

use crate::control::RandomEntry;
use crate::engine::{Metrics, TradingRules};
use crate::sweep::{PromisingGate, SelectBy, Verdict, verdict, walk_forward};

pub struct Hypothesis {
    pub label: &'static str,
    pub base: &'static str,
    pub filters: Vec<Filter>,
    /// Parameter defaults changed from the method's own — a preset. The grid
    /// still sweeps around them.
    pub overrides: Vec<(&'static str, f64)>,
    /// The reason it is on the list. A hypothesis without one is a search.
    pub why: &'static str,
}

/// The batches that can be asked for by name.
#[must_use]
pub fn batch(name: &str) -> Option<Vec<Hypothesis>> {
    match name {
        "gold-intraday" => Some(gold_intraday_batch()),
        "ict-m1" => Some(ict_batch(15, "M1 entries, M15 gaps")),
        "ict-m5" => Some(ict_batch(3, "M5 entries, M15 gaps")),
        // Pre-registered before the out-of-sample run: only the two presets
        // that survived on the broker's three months of minutes.
        "ict-oos" => Some(ict_batch(15, "M1 entries, M15 gaps").into_iter().filter(|h| h.label.starts_with("ict-B")).collect()),
        _ => None,
    }
}

/// The ICT sweep → MSS → FVG expert's three presets, in and out of its kill
/// zones.
///
/// Sessions are the expert's defaults on the broker's clock (UTC+3): London
/// 08:00–12:00 and New York 13:00–17:00 server time are 01:00–05:00 and
/// 06:00–10:00 in New York. `htf_factor` is how many entry bars make one
/// higher-timeframe bar.
#[must_use]
pub fn ict_batch(htf_factor: usize, note: &'static str) -> Vec<Hypothesis> {
    let base = "ict-sweep-mss-fvg";
    let zones = || Filter::sessions(&[(100, 500), (600, 1000)]);
    let factor = htf_factor as f64;
    let tight = move || vec![("htfFactor", factor), ("minHtfFvgPips", 25.0), ("swingLeft", 5.0), ("swingRight", 5.0), ("displacementMult", 2.0), ("riskReward", 3.0)];
    let balanced = move || vec![("htfFactor", factor), ("minHtfFvgPips", 15.0), ("swingLeft", 3.0), ("swingRight", 3.0), ("displacementMult", 1.5), ("riskReward", 2.0)];
    let loose = move || vec![("htfFactor", factor), ("minHtfFvgPips", 8.0), ("swingLeft", 2.0), ("swingRight", 2.0), ("displacementMult", 1.2), ("riskReward", 1.5)];
    let _ = note;
    vec![
        Hypothesis { label: "ict-A-tight", base, filters: vec![Filter::weekdays(), zones()], overrides: tight(), why: "the expert's strict preset, kill zones only" },
        Hypothesis { label: "ict-B-balanced", base, filters: vec![Filter::weekdays(), zones()], overrides: balanced(), why: "the expert's default preset, kill zones only" },
        Hypothesis { label: "ict-C-loose", base, filters: vec![Filter::weekdays()], overrides: loose(), why: "the expert's loose preset, sessions off as it ships" },
        Hypothesis { label: "ict-B-allday", base, filters: vec![Filter::weekdays()], overrides: balanced(), why: "the default preset without the session gate: is the kill zone doing anything?" },
    ]
}

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
        // overnight hold and the swap change the picture at all?
        Hypothesis { label: "intraday", base: "ema-cross", filters: flat(), overrides: vec![], why: "the swap-free version of the trend baseline" },
        Hypothesis { label: "intraday", base: "rsi-reversion", filters: flat(), overrides: vec![], why: "the swap-free version of the reversion baseline" },
        Hypothesis { label: "intraday", base: "donchian-breakout", filters: flat(), overrides: vec![], why: "the swap-free version of the breakout baseline" },
        Hypothesis { label: "intraday", base: "bb-fade", filters: flat(), overrides: vec![], why: "the swap-free version of the fade baseline" },
        // Sessions. Gold's volume lives in the New York morning; London's
        // open sets the day's range; Asia is thin and mean-reverting by repute.
        Hypothesis { label: "ny-morning", base: "ema-cross", filters: with(Filter::hours(800, 1200)), overrides: vec![], why: "trend into the session with the volume" },
        Hypothesis { label: "ny-morning", base: "donchian-breakout", filters: with(Filter::hours(800, 1200)), overrides: vec![], why: "breakouts where the liquidity is" },
        Hypothesis { label: "london-open", base: "donchian-breakout", filters: with(Filter::hours(200, 600)), overrides: vec![], why: "London sets the range; trade the break of the Asian one" },
        Hypothesis { label: "asia", base: "rsi-reversion", filters: with(Filter::hours(1900, 200)), overrides: vec![], why: "thin hours are said to mean-revert" },
        Hypothesis { label: "asia", base: "bb-fade", filters: with(Filter::hours(1900, 200)), overrides: vec![], why: "same claim, band-based" },
        // Regimes. The same signal reads differently when the range is
        // expanding versus compressing.
        Hypothesis { label: "expansion", base: "donchian-breakout", filters: with(expansion()), overrides: vec![], why: "breakouts only when volatility is already rising" },
        Hypothesis { label: "expansion", base: "ema-cross", filters: with(expansion()), overrides: vec![], why: "trend only when there is range to trend in" },
        Hypothesis { label: "compression", base: "rsi-reversion", filters: with(compression()), overrides: vec![], why: "fade only when the range is tight" },
        Hypothesis { label: "compression", base: "bb-fade", filters: with(compression()), overrides: vec![], why: "same, band-based" },
    ]
}

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

/// The random-entry control with a fixed seed as its default.
///
/// `walk_forward` expands the grid from `default_params`, so the seed has to
/// live there for each null run to be a different world.
struct SeededControl {
    seed: f64,
}

impl Strategy for SeededControl {
    fn id(&self) -> &'static str {
        RandomEntry.id()
    }
    fn name(&self) -> &'static str {
        RandomEntry.name()
    }
    fn description(&self) -> &'static str {
        RandomEntry.description()
    }
    fn default_params(&self) -> Params {
        let mut p = RandomEntry.default_params();
        p.set("seed", self.seed);
        p
    }
    fn grid(&self) -> std::collections::BTreeMap<String, Vec<f64>> {
        RandomEntry.grid()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        RandomEntry.indicators(p)
    }
    fn warmup(&self, p: &Params) -> usize {
        RandomEntry.warmup(p)
    }
    fn series(&self, p: &Params) -> Vec<String> {
        RandomEntry.series(p)
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        RandomEntry.on_bar(ctx)
    }
}

/// A method with some defaults replaced: the expert's preset, as a strategy.
struct Preset<'a> {
    inner: &'a dyn Strategy,
    defaults: Params,
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
        self.inner.grid()
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
) -> Option<HypothesisReport> {
    let base = registry.get(hypothesis.base).ok()?;
    let mut defaults = base.default_params();
    for (key, value) in &hypothesis.overrides {
        defaults.set(key, *value);
    }
    let preset = Preset { inner: base, defaults };
    let filtered = Filtered { inner: &preset, filters: hypothesis.filters.clone() };
    let result = walk_forward(&filtered, bars, rules, None, folds, select_by, min_trades_per_cell)?;

    let mut null_pf: Vec<f64> = (0..seeds)
        .into_par_iter()
        .filter_map(|seed| {
            let control = SeededControl { seed: seed as f64 + 1.0 };
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

    Some(HypothesisReport {
        label: hypothesis.label.to_string(),
        base: hypothesis.base.to_string(),
        filters: filtered.describe(),
        why: hypothesis.why.to_string(),
        verdict: verdict(&result.oos, gate),
        swap_usd: result.oos_trades.iter().map(|t| t.swap_usd).sum(),
        oos: result.oos,
        null_pf,
        percentile,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_batch_names_only_registered_methods_and_gives_every_entry_a_reason() {
        let registry = Registry::with_builtins();
        for h in gold_intraday_batch() {
            assert!(registry.get(h.base).is_ok(), "{} is not a strategy", h.base);
            assert!(!h.why.is_empty(), "{}/{} has no reason", h.label, h.base);
            assert!(h.filters.iter().any(|f| matches!(f, Filter::Flat { .. })), "{}/{} is not intraday", h.label, h.base);
        }
        for name in ["ict-m1", "ict-m5"] {
            for h in batch(name).unwrap() {
                let strategy = registry.get(h.base).expect(h.base);
                let known = strategy.default_params();
                for (key, _) in &h.overrides {
                    assert!(known.contains(key), "{}/{} overrides unknown parameter {key}", h.label, h.base);
                }
            }
        }
        assert!(batch("nope").is_none());
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
