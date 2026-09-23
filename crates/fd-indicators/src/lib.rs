//! Causal technical indicators.
//!
//! One property governs this crate, and a test enforces it for every registered
//! indicator: **the value at index `i` depends only on bars `0..=i`**. Computing
//! an indicator over a truncated series must equal the prefix of computing it
//! over the full series. That is what allows the same function to serve the
//! chart, the backtester and the live loop without a signal ever reading the
//! future.
//!
//! Warm-up periods are `f64::NAN`, never zero and never the first value. A
//! strategy that reads an unready indicator gets NaN and can skip the bar
//! instead of trading on a fabricated number.

use std::collections::BTreeMap;
use std::sync::Arc;

use fd_core::types::Bar;
use serde::{Deserialize, Serialize};

pub mod companion;

/// Which chart area an indicator belongs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pane {
    /// Drawn over the candles, in price units.
    Overlay,
    /// Drawn in its own pane, in its own units.
    Separate,
}

/// Price field an indicator reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Open,
    High,
    Low,
    #[default]
    Close,
    Hlc3,
}

impl Source {
    fn extract(self, bars: &[Bar]) -> Vec<f64> {
        bars.iter()
            .map(|b| match self {
                Self::Open => b.open,
                Self::High => b.high,
                Self::Low => b.low,
                Self::Close => b.close,
                Self::Hlc3 => b.hlc3(),
            })
            .collect()
    }
}

/// A request for one indicator instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndicatorSpec {
    pub id: String,
    /// Overrides for the definition's numeric parameters, by name.
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    #[serde(default)]
    pub source: Option<Source>,
}

impl IndicatorSpec {
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string(), params: BTreeMap::new(), source: None }
    }

    #[must_use]
    pub fn with(mut self, name: &str, value: f64) -> Self {
        self.params.insert(name.to_string(), value);
        self
    }
}

/// Static description of an indicator: its ordered numeric parameters and the
/// series it produces.
pub struct IndicatorDef {
    pub id: &'static str,
    pub name: &'static str,
    pub pane: Pane,
    /// Ordered — the key is built from these in declaration order, so changing
    /// the order silently renames every series.
    pub params: &'static [(&'static str, f64)],
    pub outputs: &'static [&'static str],
}

impl IndicatorDef {
    /// Resolve a spec's parameters against the defaults, in declaration order.
    fn resolve(&self, spec: &IndicatorSpec) -> Vec<f64> {
        self.params
            .iter()
            .map(|(name, default)| spec.params.get(*name).copied().unwrap_or(*default))
            .collect()
    }

    fn param(&self, values: &[f64], name: &str) -> f64 {
        self.params
            .iter()
            .position(|(n, _)| *n == name)
            .and_then(|i| values.get(i).copied())
            .unwrap_or_else(|| panic!("indicator {} has no parameter {name}", self.id))
    }
}

/// Every registered indicator.
pub static INDICATORS: &[IndicatorDef] = &[
    IndicatorDef { id: "sma", name: "SMA", pane: Pane::Overlay, params: &[("period", 20.0)], outputs: &["sma"] },
    IndicatorDef { id: "ema", name: "EMA", pane: Pane::Overlay, params: &[("period", 21.0)], outputs: &["ema"] },
    IndicatorDef { id: "rsi", name: "RSI", pane: Pane::Separate, params: &[("period", 14.0)], outputs: &["rsi"] },
    IndicatorDef {
        id: "macd",
        name: "MACD",
        pane: Pane::Separate,
        params: &[("fast", 12.0), ("slow", 26.0), ("signal", 9.0)],
        outputs: &["macd", "signal", "histogram"],
    },
    IndicatorDef {
        id: "bbands",
        name: "Bollinger Bands",
        pane: Pane::Overlay,
        params: &[("period", 20.0), ("mult", 2.0)],
        outputs: &["upper", "middle", "lower"],
    },
    IndicatorDef { id: "atr", name: "ATR", pane: Pane::Separate, params: &[("period", 14.0)], outputs: &["atr"] },
    IndicatorDef {
        id: "vwap",
        name: "VWAP (session)",
        pane: Pane::Overlay,
        params: &[("sessionMs", 86_400_000.0)],
        outputs: &["vwap"],
    },
    IndicatorDef {
        id: "stoch",
        name: "Stochastic",
        pane: Pane::Separate,
        params: &[("period", 14.0), ("smoothK", 3.0), ("smoothD", 3.0)],
        outputs: &["k", "d"],
    },
    IndicatorDef {
        id: "adx",
        name: "ADX",
        pane: Pane::Separate,
        params: &[("period", 14.0)],
        outputs: &["adx", "plusDi", "minusDi"],
    },
    IndicatorDef {
        id: "donchian",
        name: "Donchian Channel",
        pane: Pane::Overlay,
        params: &[("period", 20.0)],
        outputs: &["upper", "middle", "lower"],
    },
    IndicatorDef {
        id: "keltner",
        name: "Keltner Channel",
        pane: Pane::Overlay,
        params: &[("period", 20.0), ("atrPeriod", 10.0), ("mult", 1.5)],
        outputs: &["upper", "middle", "lower"],
    },
    // Confirmed swing points: the last swing high / low as of each bar, and
    // the bar it formed on. A swing is confirmed only once `right` later bars
    // have closed, so the series is causal and steps rather than repaints.
    IndicatorDef {
        id: "swing",
        name: "Swing points",
        pane: Pane::Overlay,
        params: &[("left", 3.0), ("right", 3.0)],
        outputs: &["high", "low", "highAt", "lowAt"],
    },
    /* ------------------------------------------------------------------
     * METHOD-LEVEL DEFINITIONS.
     *
     * The twelve above answer "what is this number now". The three below
     * answer "which way is this market, and where does that answer change"
     * — they carry a direction, so a trader runs a method on them rather
     * than reading a value off them. All three were MEASURED on this desk
     * before they were drawn: see [`MEASURED`], which the catalog serves
     * beside each one, and
     * `docs/decisions/2026-09-18-market-bias-definitions.md`.
     * ------------------------------------------------------------------ */
    IndicatorDef {
        id: "supertrend",
        name: "Supertrend",
        pane: Pane::Overlay,
        params: &[("period", 10.0), ("mult", 3.0)],
        outputs: &["supertrend", "direction"],
    },
    IndicatorDef {
        id: "zigzag",
        name: "ZigZag (ATR, live)",
        pane: Pane::Overlay,
        params: &[("k", 3.0), ("atrPeriod", 14.0)],
        outputs: &["zigzag", "direction"],
    },
    // `anchor` IS A NUMBER BECAUSE EVERY PARAMETER HERE IS, and the number
    // chosen is the anchor's length in days: 1 = the session, 7 = the week.
    // A string parameter would need a second kind of `params` map through
    // the spec, the key builder, the catalog and the picker; a day count is
    // the one encoding that reads correctly in the key it produces
    // (`avwap_1`, `avwap_7`). Anything that is neither yields all-NaN
    // rather than quietly falling back to the session — see [`Anchor`].
    IndicatorDef {
        id: "avwap",
        name: "Anchored VWAP",
        pane: Pane::Overlay,
        params: &[("anchor", 1.0)],
        outputs: &["avwap"],
    },
    // THE ONE DEFINITION THAT IS NOT A FUNCTION OF THESE BARS. It reads the
    // second instrument installed in [`companion`], matched on an exact
    // timestamp, and every output is NaN when nothing is installed. See that
    // module for why a second series enters the crate there rather than
    // through `compute_indicators`' signature, and for the alignment rules
    // that make reading it causal.
    IndicatorDef {
        id: "cmp",
        name: "Companion instrument",
        pane: Pane::Separate,
        params: &[("period", 2.0), ("atrPeriod", 14.0)],
        outputs: &["change", "atr", "close"],
    },
];

/// How a DEFINITION behaved on a measured sample. Not a property of it.
///
/// The idiom is `fd_api::htf::RuleMeasuredDto`'s, and the reason for keeping
/// the measurement out of the definition is the same one: a definition does
/// not go stale and a measurement does. So each row names the file it came
/// from, the sample it was measured over and the timeframe it was measured
/// on — a number measured on H1 is not a number about H4, and both are
/// carried rather than one being generalised.
// Serialize only: these are static rows this crate publishes, never
// something a caller sends back. `Deserialize` would need owned strings and
// an owned params list, which is a wire type's job — `fd_api::dto` has it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Measured {
    /// The study's own name for the cell, so a reader can find the row.
    pub definition: &'static str,
    /// The timeframe the numbers were measured ON. Not a recommendation of
    /// what to draw it on.
    pub timeframe: &'static str,
    /// The parameter cell measured. These numbers describe THIS cell and no
    /// other: `avwap` anchored on the week is a different row from `avwap`
    /// anchored on the day, and on H1 they are 9.2 flips against 19.7.
    pub params: &'static [(&'static str, f64)],
    /// Label changes per 100 bars.
    pub flips_per_100_bars: f64,
    /// Share of those flips reversed within three bars. THE COST, and the
    /// number most likely to be dropped from a summary.
    pub undone_within_3_pct: f64,
    /// Median bars from a reference (4×ATR, acausal) turn until the label
    /// agrees.
    pub median_lag_bars: f64,
    /// Share of reference turns the label never agreed with before the next
    /// one. Counted, never dropped — dropping them flatters a slow rule.
    pub missed_pct: f64,
    pub sample_bars: usize,
    /// Where these came from, so they can be re-derived or contradicted.
    pub source: &'static str,
    /// The study's own warning about this cell, verbatim in substance.
    /// Present only where the decision names the cell as one not to offer.
    pub caution: Option<&'static str>,
}

const BIAS_NOTE: &str = "docs/decisions/2026-09-18-market-bias-definitions.md";
/// 25,708 H1 bars and 6,728 H4 bars of XAUUSD, one broker year each.
const H1_BARS: usize = 25_708;
const H4_BARS: usize = 6_728;

/// What the 2026-09-18 study measured, by indicator id.
///
/// AN INDICATOR MISSING FROM THIS TABLE IS UNMEASURED, and the catalog says
/// `null` rather than an empty list: a blank where evidence goes is read as
/// evidence, and an empty array reads as "measured, nothing found".
///
/// Anyone registering a new definition in [`INDICATORS`] adds nothing here
/// until somebody measures it. That is the intended asymmetry.
pub static MEASURED: &[(&str, &[Measured])] = &[
    (
        "supertrend",
        &[
            Measured {
                definition: "supertrend(10,3)",
                timeframe: "1h",
                params: &[("period", 10.0), ("mult", 3.0)],
                flips_per_100_bars: 2.5,
                undone_within_3_pct: 2.0,
                median_lag_bars: 13.0,
                missed_pct: 19.0,
                sample_bars: H1_BARS,
                source: BIAS_NOTE,
                caution: None,
            },
            Measured {
                definition: "supertrend(10,3)",
                timeframe: "4h",
                params: &[("period", 10.0), ("mult", 3.0)],
                flips_per_100_bars: 2.4,
                undone_within_3_pct: 1.0,
                median_lag_bars: 10.0,
                missed_pct: 12.0,
                sample_bars: H4_BARS,
                source: BIAS_NOTE,
                caution: None,
            },
        ],
    ),
    (
        "zigzag",
        &[
            Measured {
                definition: "zigzag 3xATR",
                timeframe: "1h",
                params: &[("k", 3.0), ("atrPeriod", 14.0)],
                flips_per_100_bars: 6.1,
                undone_within_3_pct: 16.0,
                median_lag_bars: 6.0,
                missed_pct: 1.0,
                sample_bars: H1_BARS,
                source: BIAS_NOTE,
                caution: None,
            },
            Measured {
                definition: "zigzag 3xATR",
                timeframe: "4h",
                params: &[("k", 3.0), ("atrPeriod", 14.0)],
                flips_per_100_bars: 5.6,
                undone_within_3_pct: 9.0,
                median_lag_bars: 6.0,
                missed_pct: 4.0,
                sample_bars: H4_BARS,
                source: BIAS_NOTE,
                caution: None,
            },
        ],
    ),
    (
        "avwap",
        &[
            // THE STUDY SAYS IN AS MANY WORDS NOT TO PUT THIS ONE ON A CARD.
            // It is still offered, because a chart tool that hides what its
            // owner measured is worse than one that shows it with the cost
            // attached — but it is never offered silently.
            Measured {
                definition: "avwap day",
                timeframe: "1h",
                params: &[("anchor", 1.0)],
                flips_per_100_bars: 19.7,
                undone_within_3_pct: 57.0,
                median_lag_bars: 3.0,
                missed_pct: 2.0,
                sample_bars: H1_BARS,
                source: BIAS_NOTE,
                caution: Some(
                    "The study says not to put this on a card: it turns every 5 bars and 57% of \
                     those turns are reversed within three. Three bars of lag is not speed, it is \
                     noise with a direction attached.",
                ),
            },
            Measured {
                definition: "avwap day",
                timeframe: "4h",
                params: &[("anchor", 1.0)],
                flips_per_100_bars: 34.3,
                undone_within_3_pct: 72.0,
                median_lag_bars: 2.0,
                missed_pct: 0.0,
                sample_bars: H4_BARS,
                source: BIAS_NOTE,
                caution: Some(
                    "Worse on H4 than on H1: 34.3 flips per 100 bars and 72% of them undone \
                     within three. The study names this one as not to be put on a card.",
                ),
            },
            Measured {
                definition: "avwap week",
                timeframe: "1h",
                params: &[("anchor", 7.0)],
                flips_per_100_bars: 9.2,
                undone_within_3_pct: 53.0,
                median_lag_bars: 6.0,
                missed_pct: 15.0,
                sample_bars: H1_BARS,
                source: BIAS_NOTE,
                caution: None,
            },
        ],
    ),
];

/// What the desk measured this definition doing, or an empty slice.
#[must_use]
pub fn measured(id: &str) -> &'static [Measured] {
    MEASURED.iter().find(|(key, _)| *key == id).map_or(&[], |(_, rows)| *rows)
}

/// Look up a definition by id.
#[must_use]
pub fn definition(id: &str) -> Option<&'static IndicatorDef> {
    INDICATORS.iter().find(|d| d.id == id)
}

/// Stable key for an indicator instance: `ema_21`, `macd_12_26_9`.
///
/// Note the float case: `keltner` with `mult = 1.5` yields `keltner_20_10_1.5`,
/// a key that itself contains a dot. Keys are therefore built and compared
/// whole — never split on `.` to recover the output name.
#[must_use]
pub fn indicator_key(def: &IndicatorDef, values: &[f64]) -> String {
    if values.is_empty() {
        return def.id.to_string();
    }
    let joined = values.iter().map(format_param).collect::<Vec<_>>().join("_");
    format!("{}_{joined}", def.id)
}

/// Format a parameter the way the JavaScript oracle does, so keys match.
/// `20.0 -> "20"`, `1.5 -> "1.5"`, `86400000.0 -> "86400000"`.
fn format_param(value: &f64) -> String {
    format!("{value}")
}

#[derive(Debug, thiserror::Error)]
pub enum IndicatorError {
    #[error("unknown indicator `{0}` (have: {1})")]
    Unknown(String, String),
}

/// Compute a set of specs over one bar series.
///
/// Keys are `{instance}.{output}`, e.g. `macd_12_26_9.histogram`.
/// One computed series.
///
/// Shared rather than owned: a single-output indicator is filed under two
/// spellings, and a sweep asks for the same series once per parameter cell.
/// Copying 673 floats each time was a measurable share of a whole sweep, and
/// the copies were all of identical data.
pub type Series = Arc<[f64]>;

/// Every series a set of specs produced, by key.
pub type IndicatorSet = BTreeMap<String, Series>;

pub fn compute_indicators(bars: &[Bar], specs: &[IndicatorSpec]) -> Result<IndicatorSet, IndicatorError> {
    let mut out = BTreeMap::new();
    for spec in specs {
        let def = definition(&spec.id).ok_or_else(|| {
            IndicatorError::Unknown(
                spec.id.clone(),
                INDICATORS.iter().map(|d| d.id).collect::<Vec<_>>().join(", "),
            )
        })?;
        let params = def.resolve(spec);
        let key = indicator_key(def, &params);
        let source = spec.source.unwrap_or_default();
        let single_output = def.outputs.len() == 1;
        for (name, series) in compute_one(def, &params, source, bars) {
            // Single-output indicators also get a bare alias, so a strategy can
            // say `atr_14` rather than `atr_14.atr`. Both spellings are in use —
            // dropping the alias makes every guard that reads one see NaN, and a
            // strategy that silently never trades looks like a strategy with no
            // signals rather than a broken lookup.
            let series: Series = Series::from(series);
            if single_output {
                out.insert(key.clone(), Series::clone(&series));
            }
            out.insert(format!("{key}.{name}"), series);
        }
    }
    Ok(out)
}

fn compute_one(def: &IndicatorDef, p: &[f64], source: Source, bars: &[Bar]) -> Vec<(&'static str, Vec<f64>)> {
    let period = |name: &str| def.param(p, name).round() as usize;
    match def.id {
        "sma" => vec![("sma", sma(&source.extract(bars), period("period")))],
        "ema" => vec![("ema", ema(&source.extract(bars), period("period")))],
        "rsi" => vec![("rsi", rsi(bars, period("period")))],
        "macd" => {
            let (macd_line, signal, histogram) = macd(bars, period("fast"), period("slow"), period("signal"));
            vec![("macd", macd_line), ("signal", signal), ("histogram", histogram)]
        }
        "bbands" => {
            let (upper, middle, lower) = bollinger(bars, period("period"), def.param(p, "mult"));
            vec![("upper", upper), ("middle", middle), ("lower", lower)]
        }
        "atr" => vec![("atr", atr(bars, period("period")))],
        "vwap" => vec![("vwap", vwap(bars, def.param(p, "sessionMs") as i64))],
        "stoch" => {
            let (k, d) = stochastic(bars, period("period"), period("smoothK"), period("smoothD"));
            vec![("k", k), ("d", d)]
        }
        "adx" => {
            let (adx_line, plus_di, minus_di) = adx(bars, period("period"));
            vec![("adx", adx_line), ("plusDi", plus_di), ("minusDi", minus_di)]
        }
        "donchian" => {
            let (upper, middle, lower) = donchian(bars, period("period"));
            vec![("upper", upper), ("middle", middle), ("lower", lower)]
        }
        "keltner" => {
            let (upper, middle, lower) = keltner(bars, period("period"), period("atrPeriod"), def.param(p, "mult"));
            vec![("upper", upper), ("middle", middle), ("lower", lower)]
        }
        "swing" => {
            let (high, low, high_at, low_at) = swing(bars, period("left"), period("right"));
            vec![("high", high), ("low", low), ("highAt", high_at), ("lowAt", low_at)]
        }
        "supertrend" => {
            let (line, direction) = supertrend(bars, period("period"), def.param(p, "mult"));
            vec![("supertrend", line), ("direction", direction)]
        }
        "zigzag" => {
            let (line, direction) = zigzag(bars, def.param(p, "k"), period("atrPeriod"));
            vec![("zigzag", line), ("direction", direction)]
        }
        "avwap" => vec![("avwap", anchored_vwap(bars, Anchor::from_days(def.param(p, "anchor"))))],
        // The second instrument. All NaN when nothing is installed, which is
        // the honest answer and not a fallback to these bars' own values.
        "cmp" => match companion::installed() {
            None => {
                let nan = || vec![f64::NAN; bars.len()];
                vec![("change", nan()), ("atr", nan()), ("close", nan())]
            }
            Some(c) => {
                let (change, atr, close) =
                    companion::aligned_change(bars, c, period("period"), period("atrPeriod"));
                vec![("change", change), ("atr", atr), ("close", close)]
            }
        },
        other => unreachable!("indicator {other} is registered but not implemented"),
    }
}

/* ---------------- primitives ---------------- */

/// Last confirmed swing high and low as of each bar, with the bar each formed on.
///
/// Bar `k` is a swing high when its high is strictly above every high within
/// `left` bars before and `right` bars after it; it becomes known at bar
/// `k + right`, and from that bar on the series carries its level (and `k` as
/// a float in the `*At` series) until a later swing replaces it. Nothing is
/// known before the first confirmation, so the series start as NaN.
#[must_use]
pub fn swing(bars: &[Bar], left: usize, right: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = bars.len();
    let (mut high, mut low, mut high_at, mut low_at) = (nans(n), nans(n), nans(n), nans(n));
    let (mut last_high, mut last_low) = (f64::NAN, f64::NAN);
    let (mut last_high_at, mut last_low_at) = (f64::NAN, f64::NAN);
    for i in 0..n {
        // The swing confirmed on this bar, if any, formed `right` bars ago.
        if i >= right && i - right >= left {
            let k = i - right;
            let window = &bars[k - left..=k + right];
            let candidate = &bars[k];
            let is_high = window.iter().enumerate().all(|(j, b)| j == left || b.high < candidate.high);
            let is_low = window.iter().enumerate().all(|(j, b)| j == left || b.low > candidate.low);
            if is_high {
                last_high = candidate.high;
                last_high_at = k as f64;
            }
            if is_low {
                last_low = candidate.low;
                last_low_at = k as f64;
            }
        }
        high[i] = last_high;
        low[i] = last_low;
        high_at[i] = last_high_at;
        low_at[i] = last_low_at;
    }
    (high, low, high_at, low_at)
}

fn nans(len: usize) -> Vec<f64> {
    vec![f64::NAN; len]
}

/// Simple moving average.
#[must_use]
pub fn sma(values: &[f64], period: usize) -> Vec<f64> {
    let mut out = nans(values.len());
    if period == 0 {
        return out;
    }
    let mut sum = 0.0;
    let mut count = 0usize;
    for i in 0..values.len() {
        if values[i].is_finite() {
            sum += values[i];
            count += 1;
        }
        if i >= period {
            let old = values[i - period];
            if old.is_finite() {
                sum -= old;
                count -= 1;
            }
        }
        if i + 1 >= period && count == period {
            out[i] = sum / period as f64;
        }
    }
    out
}

/// Exponential moving average, seeded with the SMA of the first `period` values.
#[must_use]
pub fn ema(values: &[f64], period: usize) -> Vec<f64> {
    let mut out = nans(values.len());
    if period == 0 {
        return out;
    }
    let k = 2.0 / (period as f64 + 1.0);
    let mut prev = f64::NAN;
    let mut seed_sum = 0.0;
    let mut seed_count = 0usize;
    for (i, &v) in values.iter().enumerate() {
        if !v.is_finite() {
            continue;
        }
        if !prev.is_finite() {
            seed_sum += v;
            seed_count += 1;
            if seed_count == period {
                prev = seed_sum / period as f64;
                out[i] = prev;
            }
            continue;
        }
        prev = v * k + prev * (1.0 - k);
        out[i] = prev;
    }
    out
}

/// Wilder's smoothing, used by RSI, ATR and ADX.
#[must_use]
pub fn rma(values: &[f64], period: usize) -> Vec<f64> {
    let mut out = nans(values.len());
    if period == 0 {
        return out;
    }
    let mut prev = f64::NAN;
    let mut seed_sum = 0.0;
    let mut seed_count = 0usize;
    for (i, &v) in values.iter().enumerate() {
        if !v.is_finite() {
            continue;
        }
        if !prev.is_finite() {
            seed_sum += v;
            seed_count += 1;
            if seed_count == period {
                prev = seed_sum / period as f64;
                out[i] = prev;
            }
            continue;
        }
        prev = (prev * (period as f64 - 1.0) + v) / period as f64;
        out[i] = prev;
    }
    out
}

/// Rolling population standard deviation (the Bollinger convention).
#[must_use]
pub fn stdev(values: &[f64], period: usize) -> Vec<f64> {
    let mut out = nans(values.len());
    if period == 0 || values.len() < period {
        return out;
    }
    for i in (period - 1)..values.len() {
        let window = &values[i + 1 - period..=i];
        if window.iter().any(|v| !v.is_finite()) {
            continue;
        }
        let mean = window.iter().sum::<f64>() / period as f64;
        let variance = window.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / period as f64;
        out[i] = variance.sqrt();
    }
    out
}

/// True range per bar. The first bar has no previous close, so it degenerates
/// to the bar's own range.
#[must_use]
pub fn true_range(bars: &[Bar]) -> Vec<f64> {
    let mut out = nans(bars.len());
    for (i, bar) in bars.iter().enumerate() {
        out[i] = if i == 0 {
            bar.high - bar.low
        } else {
            let prev_close = bars[i - 1].close;
            (bar.high - bar.low)
                .max((bar.high - prev_close).abs())
                .max((bar.low - prev_close).abs())
        };
    }
    out
}

/* ---------------- indicators ---------------- */

#[must_use]
pub fn atr(bars: &[Bar], period: usize) -> Vec<f64> {
    rma(&true_range(bars), period)
}

#[must_use]
pub fn rsi(bars: &[Bar], period: usize) -> Vec<f64> {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let mut gains = nans(closes.len());
    let mut losses = nans(closes.len());
    for i in 1..closes.len() {
        let diff = closes[i] - closes[i - 1];
        gains[i] = diff.max(0.0);
        losses[i] = (-diff).max(0.0);
    }
    // The first element has no change; smoothing starts from index 1.
    let avg_gain = rma(&gains[1..], period);
    let avg_loss = rma(&losses[1..], period);
    let mut out = nans(closes.len());
    for i in 0..avg_gain.len() {
        let (g, l) = (avg_gain[i], avg_loss[i]);
        if !g.is_finite() || !l.is_finite() {
            continue;
        }
        out[i + 1] = if l == 0.0 { 100.0 } else { 100.0 - 100.0 / (1.0 + g / l) };
    }
    out
}

#[must_use]
pub fn macd(bars: &[Bar], fast: usize, slow: usize, signal: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let fast_ema = ema(&closes, fast);
    let slow_ema = ema(&closes, slow);
    let line: Vec<f64> = (0..closes.len())
        .map(|i| if fast_ema[i].is_finite() && slow_ema[i].is_finite() { fast_ema[i] - slow_ema[i] } else { f64::NAN })
        .collect();
    let signal_line = ema(&line, signal);
    let histogram: Vec<f64> = (0..line.len())
        .map(|i| if line[i].is_finite() && signal_line[i].is_finite() { line[i] - signal_line[i] } else { f64::NAN })
        .collect();
    (line, signal_line, histogram)
}

#[must_use]
pub fn bollinger(bars: &[Bar], period: usize, mult: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let middle = sma(&closes, period);
    let sd = stdev(&closes, period);
    let upper = middle.iter().zip(&sd).map(|(m, s)| if m.is_finite() { m + mult * s } else { f64::NAN }).collect();
    let lower = middle.iter().zip(&sd).map(|(m, s)| if m.is_finite() { m - mult * s } else { f64::NAN }).collect();
    (upper, middle, lower)
}

/// Session VWAP. Volume defaults to 1 per bar where the feed has none, which
/// turns it into a typical-price average — stated here rather than hidden.
#[must_use]
pub fn vwap(bars: &[Bar], session_ms: i64) -> Vec<f64> {
    let mut out = nans(bars.len());
    if session_ms <= 0 {
        return out;
    }
    let mut session = i64::MIN;
    let mut pv = 0.0;
    let mut vol = 0.0;
    for (i, bar) in bars.iter().enumerate() {
        let start = bar.time.div_euclid(session_ms);
        if start != session {
            session = start;
            pv = 0.0;
            vol = 0.0;
        }
        let v = match bar.volume {
            Some(v) if v > 0.0 => v,
            _ => 1.0,
        };
        pv += bar.hlc3() * v;
        vol += v;
        out[i] = pv / vol;
    }
    out
}

#[must_use]
pub fn stochastic(bars: &[Bar], period: usize, smooth_k: usize, smooth_d: usize) -> (Vec<f64>, Vec<f64>) {
    let mut raw = nans(bars.len());
    if period > 0 && bars.len() >= period {
        for i in (period - 1)..bars.len() {
            let window = &bars[i + 1 - period..=i];
            let hh = window.iter().fold(f64::NEG_INFINITY, |acc, b| acc.max(b.high));
            let ll = window.iter().fold(f64::INFINITY, |acc, b| acc.min(b.low));
            raw[i] = if (hh - ll).abs() < f64::EPSILON { 50.0 } else { (bars[i].close - ll) / (hh - ll) * 100.0 };
        }
    }
    let k = sma(&raw, smooth_k);
    let d = sma(&k, smooth_d);
    (k, d)
}

#[must_use]
pub fn adx(bars: &[Bar], period: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let len = bars.len();
    let mut plus_dm = nans(len);
    let mut minus_dm = nans(len);
    for i in 1..len {
        let up = bars[i].high - bars[i - 1].high;
        let down = bars[i - 1].low - bars[i].low;
        plus_dm[i] = if up > down && up > 0.0 { up } else { 0.0 };
        minus_dm[i] = if down > up && down > 0.0 { down } else { 0.0 };
    }
    if len < 2 {
        return (nans(len), nans(len), nans(len));
    }
    let tr = rma(&true_range(bars)[1..], period);
    let plus = rma(&plus_dm[1..], period);
    let minus = rma(&minus_dm[1..], period);

    let mut plus_di = nans(len);
    let mut minus_di = nans(len);
    let mut dx = nans(len);
    for i in 0..tr.len() {
        if !tr[i].is_finite() || tr[i] == 0.0 {
            continue;
        }
        let pd = 100.0 * plus[i] / tr[i];
        let md = 100.0 * minus[i] / tr[i];
        plus_di[i + 1] = pd;
        minus_di[i + 1] = md;
        dx[i + 1] = if pd + md == 0.0 { 0.0 } else { 100.0 * (pd - md).abs() / (pd + md) };
    }
    let smoothed = rma(&dx[1..], period);
    let mut adx_line = nans(len);
    for (i, v) in smoothed.iter().enumerate() {
        adx_line[i + 1] = *v;
    }
    (adx_line, plus_di, minus_di)
}

/// Donchian channel over the `period` bars **before** the current one.
///
/// Excluding the current bar is what makes a breakout measurable: the level a
/// close is compared against was known before that close printed.
#[must_use]
pub fn donchian(bars: &[Bar], period: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let len = bars.len();
    let (mut upper, mut middle, mut lower) = (nans(len), nans(len), nans(len));
    if period == 0 {
        return (upper, middle, lower);
    }
    for i in period..len {
        let window = &bars[i - period..i];
        let hh = window.iter().fold(f64::NEG_INFINITY, |acc, b| acc.max(b.high));
        let ll = window.iter().fold(f64::INFINITY, |acc, b| acc.min(b.low));
        upper[i] = hh;
        lower[i] = ll;
        middle[i] = (hh + ll) / 2.0;
    }
    (upper, middle, lower)
}

#[must_use]
pub fn keltner(bars: &[Bar], period: usize, atr_period: usize, mult: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let middle = ema(&closes, period);
    let a = atr(bars, atr_period);
    let upper = middle
        .iter()
        .zip(&a)
        .map(|(m, v)| if m.is_finite() && v.is_finite() { m + mult * v } else { f64::NAN })
        .collect();
    let lower = middle
        .iter()
        .zip(&a)
        .map(|(m, v)| if m.is_finite() && v.is_finite() { m - mult * v } else { f64::NAN })
        .collect();
    (upper, middle, lower)
}

/* ---------------- method indicators ---------------- */

/*
 * THREE DEFINITIONS THAT WERE MEASURED BEFORE THEY WERE DRAWN.
 *
 * Each of the three below is a port of a function in
 * `py/research/bias_defs.py`, the file every number in
 * `docs/decisions/2026-09-18-market-bias-definitions.md` was produced by.
 * The ports are pinned against that Python on real XAUUSD H1 bars, bar for
 * bar, by `crates/fd-api/tests/method_indicators.rs`. A port that agrees
 * with the prose and not with the function would carry numbers that were
 * measured on something else, which is the failure mode this desk has
 * already paid for once (see the zigzag's four pinned choices below).
 */

/// Supertrend: the ATR band in force, and which side of it price is on.
///
/// Port of `bias_defs.py::supertrend` (line 311), which produced the
/// `supertrend(10,3)` row: on 25,708 H1 bars, 2.5 flips per 100 bars, 2% of
/// them undone within three, 13 bars of median lag, 19% of turns missed. It
/// is the slowest of the three and the least twitchy, and the study names it
/// the best compromise if one definition has to serve both rows.
///
/// Returns `(line, direction)`. `direction` is `+1` / `-1` and is the series
/// that was measured; `line` is the band in force under the same recursion —
/// the lower band while up, the upper while down.
///
/// TWO THINGS THIS DOES NOT DO, both of them common elsewhere and both of
/// them a different series:
///
/// * It does NOT reset the opposite band when direction flips, because the
///   Python does not. TradingView's does. Resetting it moves the drawn line
///   on the flip bar and would make this a lookalike rather than a port.
/// * It does not examine a bar at all while ATR is in warmup, so both series
///   are NaN there rather than carrying a direction seeded from nothing.
#[must_use]
pub fn supertrend(bars: &[Bar], period: usize, mult: f64) -> (Vec<f64>, Vec<f64>) {
    let n = bars.len();
    let (mut line, mut dir) = (nans(n), nans(n));
    let a = atr(bars, period);
    let (mut upper, mut lower) = (f64::NAN, f64::NAN);
    // Seeded UP, exactly as the Python is: until a close breaks the lower
    // band there has been no down-cross, and calling that FLAT would invent
    // a third state this definition does not have (its measured mix is
    // 52/48/0 — no flat bars at all once ATR is warm).
    let mut direction = 1.0f64;
    for t in 0..n {
        if !a[t].is_finite() {
            continue;
        }
        let bar = &bars[t];
        let mid = (bar.high + bar.low) / 2.0;
        let (basic_upper, basic_lower) = (mid + mult * a[t], mid - mult * a[t]);
        if upper.is_finite() {
            // `upper` is only finite because an earlier iteration set it, so
            // `t >= 1` here and the previous close exists.
            let prev_close = bars[t - 1].close;
            if basic_upper < upper || prev_close > upper {
                upper = basic_upper;
            }
            if basic_lower > lower || prev_close < lower {
                lower = basic_lower;
            }
        } else {
            upper = basic_upper;
            lower = basic_lower;
        }
        if bar.close > upper {
            direction = 1.0;
        } else if bar.close < lower {
            direction = -1.0;
        }
        dir[t] = direction;
        line[t] = if direction > 0.0 { lower } else { upper };
    }
    (line, dir)
}

/// One confirmed zigzag pivot.
///
/// `at` is the bar the extreme happened on and `confirmed_at` the bar the
/// reversal proved it. Unlike a fractal the distance between them is not a
/// constant: it is however long price took to retrace `k` ATRs, which may be
/// the next bar or twenty later. Nothing downstream may draw a pivot before
/// `confirmed_at` — that is the whole difference between this and the
/// zigzag on a retail chart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZigzagPivot {
    pub at: usize,
    pub confirmed_at: usize,
    pub price: f64,
    pub high: bool,
}

/// A LIVE ATR-zigzag. Pivots oldest first, each with the bar that confirmed it.
///
/// The leg extends while price makes new extremes and reverses when price
/// retraces `k` × ATR from the leg's extreme. Port of
/// `bias_defs.py::atr_zigzag` (line 130), the definition the H1 structure row
/// runs and the one the study recommends: 6.1 flips per 100 H1 bars, 16%
/// undone within three, median lag 6, and decisively **1% of turns missed**
/// against `fractal(2)`'s 23%.
///
/// MOVED HERE FROM `fd_api::htf` ON 2026-09-19, unchanged, because a second
/// implementation is a desk that will one day show two answers to one
/// question. `htf.rs` now calls this, and
/// `zigzag_labels_match_the_research_definition` still pins it against the
/// Python on 2,000 real bars.
///
/// # Four things the study's prose did not pin
///
/// Each one silently changes the label series, and a wrong one still
/// reproduces a morning's table while missing the aggregates. Three of the
/// four were guessed wrong on the first port, and all three guesses were
/// reasonable.
///
/// * **Highs and lows, not closes**, for both the extension of a leg and the
///   reversal test.
/// * **ATR at the CURRENT bar**, not at the pivot. The threshold therefore
///   MOVES under an open leg: a leg opened in a quiet tape needs a bigger
///   retrace to end it once volatility rises. That is a real consequence,
///   not a bug, and freezing it at the pivot would give a different table.
/// * **Strictly greater, not greater-or-equal.** A retrace of exactly `k`
///   ATRs does not turn the leg. One character.
/// * **On a bar that could seed either direction, UP wins**, and **a warmup
///   bar is skipped whole** — while ATR is not finite the bar is not
///   examined and `extreme` does not move. Measured on the 25,708-bar H1
///   file: FLAT on the first 13 bars and never again.
/// * **The seed is the first bar's CLOSE**, and before a direction exists one
///   scalar wanders up on a bar that closes at or above it and down
///   otherwise.
///
/// # Why it cannot repaint
///
/// A pivot is appended only when the reversal that confirms it has already
/// happened, and nothing ever pops one. The extreme of the leg IN PROGRESS
/// is not a pivot and is not published as one. So the pivot list at bar `t`
/// is a prefix of the list at any later bar, and the label at `t` is the leg
/// in force at `t` forever after.
#[must_use]
pub fn zigzag_pivots(bars: &[Bar], k: f64, atr: Option<&[f64]>) -> Vec<ZigzagPivot> {
    let mut out: Vec<ZigzagPivot> = Vec::new();
    if !(k.is_finite() && k > 0.0) || bars.is_empty() {
        return out;
    }

    let mut extreme = bars[0].close;
    let mut extreme_at = 0usize;
    let mut direction = 0i8;

    for (t, bar) in bars.iter().enumerate() {
        // SKIPPED ENTIRELY during the ATR warmup, so `extreme` does not drift
        // before there is a threshold to judge it against. Continuing past
        // the update rather than only past the test is one of the four places
        // this could silently diverge.
        let Some(a) = atr.and_then(|s| s.get(t)).copied().filter(|v| v.is_finite()) else { continue };
        let thr = k * a;

        match direction {
            0 => {
                // UP IS TESTED FIRST, and on a bar that satisfies both it
                // wins. A tie is not hypothetical on a wide bar, and the two
                // answers are opposite labels.
                if bar.high - extreme > thr {
                    out.push(ZigzagPivot { at: extreme_at, confirmed_at: t, price: extreme, high: false });
                    direction = 1;
                    extreme = bar.high;
                    extreme_at = t;
                } else if extreme - bar.low > thr {
                    out.push(ZigzagPivot { at: extreme_at, confirmed_at: t, price: extreme, high: true });
                    direction = -1;
                    extreme = bar.low;
                    extreme_at = t;
                } else if bar.close >= extreme {
                    if bar.high > extreme {
                        extreme = bar.high;
                        extreme_at = t;
                    }
                } else if bar.low < extreme {
                    extreme = bar.low;
                    extreme_at = t;
                }
            }
            1 => {
                // The leg extends BEFORE the reversal test, so one bar can
                // make a new high and then give back `thr` from it and turn.
                if bar.high > extreme {
                    extreme = bar.high;
                    extreme_at = t;
                }
                if extreme - bar.low > thr {
                    out.push(ZigzagPivot { at: extreme_at, confirmed_at: t, price: extreme, high: true });
                    direction = -1;
                    extreme = bar.low;
                    extreme_at = t;
                }
            }
            _ => {
                if bar.low < extreme {
                    extreme = bar.low;
                    extreme_at = t;
                }
                if bar.high - extreme > thr {
                    out.push(ZigzagPivot { at: extreme_at, confirmed_at: t, price: extreme, high: false });
                    direction = 1;
                    extreme = bar.high;
                    extreme_at = t;
                }
            }
        }
    }
    out
}

/// The zigzag as two chart series: the confirmed pivot level, and the leg.
///
/// THE LINE STEPS; IT IS NOT A POLYLINE BACK TO THE PIVOT'S OWN BAR, and the
/// difference is the crate's invariant rather than a drawing preference.
/// Joining pivot A at bar 40 to pivot B at bar 58 means writing a value into
/// bars 40..58 at the moment B is confirmed — bar 65, say. Those bars already
/// had values when the chart was 60 bars long, so the series would change
/// under its own history: `every_indicator_is_causal` fails, and on a live
/// chart the line would visibly redraw over bars the trader had already
/// traded. What a trader can act on is the level of the last CONFIRMED pivot,
/// which is what this holds until the next one replaces it.
///
/// `direction` is `+1` after a confirmed pivot low (the leg is up), `-1`
/// after a confirmed pivot high, and NaN before the first pivot — the FLAT
/// that the study measured on the first 13 bars of the H1 file and never
/// again.
#[must_use]
pub fn zigzag(bars: &[Bar], k: f64, atr_period: usize) -> (Vec<f64>, Vec<f64>) {
    let n = bars.len();
    let (mut line, mut dir) = (nans(n), nans(n));
    // The SAME ATR the `atr` indicator publishes. If the threshold came from
    // a second computation the zigzag would be measured in a unit the reader
    // cannot put on the chart.
    let a = atr(bars, atr_period);
    let pivots = zigzag_pivots(bars, k, Some(&a));

    let mut next = 0usize;
    let (mut price, mut leg) = (f64::NAN, f64::NAN);
    for t in 0..n {
        while next < pivots.len() && pivots[next].confirmed_at <= t {
            price = pivots[next].price;
            leg = if pivots[next].high { -1.0 } else { 1.0 };
            next += 1;
        }
        line[t] = price;
        dir[t] = leg;
    }
    (line, dir)
}

/// Where an anchored VWAP restarts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// The broker session — a new day on the server's clock.
    Day,
    /// The broker week — the Sunday reopen.
    Week,
}

impl Anchor {
    /// From the `anchor` parameter, read as a length in days.
    ///
    /// `None` for anything that is neither, and the series is then all NaN.
    /// A fallback to `Day` would draw a session line for somebody who asked
    /// for a month, and they would have no way to tell.
    #[must_use]
    pub fn from_days(days: f64) -> Option<Self> {
        match days.round() as i64 {
            1 => Some(Self::Day),
            7 => Some(Self::Week),
            _ => None,
        }
    }
}

/// The broker's clock, as `bias_defs.py::shift_hours` defines it: UTC+3 while
/// New York is on daylight time, UTC+2 otherwise.
///
/// A fixed offset would be wrong for five months of the year and a UTC
/// calendar day would cut the session in the middle of the New York
/// afternoon. The switch is New York's because the broker's is.
fn broker_offset_ms(utc_ms: i64) -> i64 {
    const HOUR_MS: i64 = 3_600_000;
    if fd_core::clock::new_york_is_dst(utc_ms) { 3 * HOUR_MS } else { 2 * HOUR_MS }
}

/// VWAP anchored at the session or the week open.
///
/// Port of `bias_defs.py::anchored_vwap` (line 335). Measured on 25,708 H1
/// bars: the DAY anchor turns 19.7 times per 100 bars with **57% of those
/// turns reversed within three**, which is why the study says not to put it
/// on a card and why [`MEASURED`] carries that sentence to the picker. The
/// WEEK anchor is 9.2 and 53%.
///
/// ⚠ **It is a typical-price mean, not a volume-weighted one.** The bar files
/// carry tick counts, not traded volume, so weighting by them would be a
/// units claim the data cannot support. Every bar counts once. The name is
/// the study's; this sentence is what keeps it from being a lie.
///
/// The first bucket of a series is PARTIAL — the anchor it belongs to began
/// before the first bar in hand. The Python does the same, so the two agree,
/// and a chart that starts mid-session shows a mean of the part it has.
///
/// The week runs from the Sunday reopen, which is Monday 00:00 on the
/// broker's clock in both seasons because the broker's offset tracks New
/// York's daylight rule exactly as the reopen does. That is the same weekend
/// hole `fd_api::htf::weeks_of` finds by looking for a gap of more than two
/// days between daily bars — located here by the clock that makes it, since
/// intraday bars have no gap that large to look for.
#[must_use]
pub fn anchored_vwap(bars: &[Bar], anchor: Option<Anchor>) -> Vec<f64> {
    const DAY_MS: i64 = 86_400_000;
    let mut out = nans(bars.len());
    let Some(anchor) = anchor else { return out };
    let mut key = i64::MIN;
    let (mut total, mut count) = (0.0f64, 0.0f64);
    for (i, bar) in bars.iter().enumerate() {
        let server_ms = bar.time + broker_offset_ms(bar.time);
        let day = server_ms.div_euclid(DAY_MS);
        let bucket = match anchor {
            // Epoch day 0 is a Thursday, so `+3` puts the week boundary on
            // Monday — the ISO week the study's `isocalendar()` groups by.
            Anchor::Week => (day + 3).div_euclid(7),
            Anchor::Day => day,
        };
        if bucket != key {
            key = bucket;
            total = 0.0;
            count = 0.0;
        }
        total += bar.hlc3();
        count += 1.0;
        out[i] = total / count;
    }
    out
}

/* ---------------- tests ---------------- */

#[cfg(test)]
mod tests {
    use super::*;

    fn wavy(n: usize) -> Vec<Bar> {
        let mut out = Vec::with_capacity(n);
        let mut price = 4300.0;
        for i in 0..n {
            let i_f = i as f64;
            price += (i_f / 9.0).sin() * 3.0 + (i_f / 23.0).cos() * 5.0 + ((i % 13) as f64 - 6.0) * 0.4;
            let open = price - (i_f / 7.0).sin() * 1.5;
            out.push(Bar {
                time: i as i64 * 900_000,
                open,
                high: open.max(price) + 1.2,
                low: open.min(price) - 1.2,
                close: price,
                volume: Some(100.0 + (i % 17) as f64),
            });
        }
        out
    }

    /// The property the whole crate exists to guarantee.
    #[test]
    fn every_indicator_is_causal() {
        let full = wavy(260);
        let cut = 180;
        for def in INDICATORS {
            let spec = IndicatorSpec::new(def.id);
            let whole = compute_indicators(&full, std::slice::from_ref(&spec)).unwrap();
            let prefix = compute_indicators(&full[..cut], std::slice::from_ref(&spec)).unwrap();
            for (key, series) in &whole {
                let other = prefix.get(key).unwrap_or_else(|| panic!("{key} missing from the truncated run"));
                for i in 0..cut {
                    assert!(
                        fd_core::parity_eq(series[i], other[i]),
                        "{key} at {i} changed when future bars were added: {} -> {}",
                        other[i],
                        series[i]
                    );
                }
            }
        }
    }

    #[test]
    fn swings_are_confirmed_late_and_never_repaint() {
        // A peak at bar 5 (high 110) and a trough at bar 12 (low 90) in an
        // otherwise flat series.
        let mut bars: Vec<Bar> = (0..20).map(|i| Bar { time: i * 60_000, open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: None }).collect();
        bars[5].high = 110.0;
        bars[12].low = 90.0;
        let (high, low, high_at, low_at) = swing(&bars, 3, 3);
        // Known only from bar 8 = 5 + 3 onward.
        assert!(high[7].is_nan());
        assert_eq!(high[8], 110.0);
        assert_eq!(high_at[8], 5.0);
        assert_eq!(high[19], 110.0, "the level holds until a later swing replaces it");
        assert!(low[14].is_nan());
        assert_eq!(low[15], 90.0);
        assert_eq!(low_at[15], 12.0);
        // A flat top is not a swing: no bar is strictly above its neighbours.
        let flat = swing(&bars[0..4], 1, 1);
        assert!(flat.0.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn keys_match_the_javascript_oracle() {
        let ema_def = definition("ema").unwrap();
        assert_eq!(indicator_key(ema_def, &[21.0]), "ema_21");
        let macd_def = definition("macd").unwrap();
        assert_eq!(indicator_key(macd_def, &[12.0, 26.0, 9.0]), "macd_12_26_9");
        let vwap_def = definition("vwap").unwrap();
        assert_eq!(indicator_key(vwap_def, &[86_400_000.0]), "vwap_86400000");
        // A float parameter puts a dot inside the key; this is expected and is
        // why keys are never split on '.'.
        let keltner_def = definition("keltner").unwrap();
        assert_eq!(indicator_key(keltner_def, &[20.0, 10.0, 1.5]), "keltner_20_10_1.5");
    }

    #[test]
    fn single_output_indicators_answer_to_both_spellings() {
        let bars = wavy(60);
        let series = compute_indicators(&bars, &[IndicatorSpec::new("atr").with("period", 14.0)]).unwrap();
        let bare = series.get("atr_14").expect("bare alias");
        let qualified = series.get("atr_14.atr").expect("qualified key");
        // Element-wise, because the warm-up NaNs make `==` on the whole vector
        // false however identical the two series are.
        assert_eq!(bare.len(), qualified.len());
        assert!(
            bare.iter().zip(qualified.iter()).all(|(a, b)| fd_core::parity_eq(*a, *b)),
            "the bare alias must be the same series as the qualified key"
        );

        // A multi-output indicator has no bare alias: there would be no way to
        // say which of its series it meant.
        let macd = compute_indicators(&bars, &[IndicatorSpec::new("macd")]).unwrap();
        assert!(!macd.contains_key("macd_12_26_9"));
    }

    #[test]
    fn output_keys_are_fully_qualified() {
        let bars = wavy(60);
        let series = compute_indicators(&bars, &[IndicatorSpec::new("macd")]).unwrap();
        let keys: Vec<_> = series.keys().cloned().collect();
        assert!(keys.contains(&"macd_12_26_9.macd".to_string()));
        assert!(keys.contains(&"macd_12_26_9.signal".to_string()));
        assert!(keys.contains(&"macd_12_26_9.histogram".to_string()));
    }

    #[test]
    fn two_instances_of_one_indicator_do_not_collide() {
        let bars = wavy(120);
        let series = compute_indicators(
            &bars,
            &[IndicatorSpec::new("ema").with("period", 9.0), IndicatorSpec::new("ema").with("period", 21.0)],
        )
        .unwrap();
        assert!(series.contains_key("ema_9.ema") && series.contains_key("ema_21.ema"));
        assert_ne!(series["ema_9.ema"][100], series["ema_21.ema"][100]);
    }

    #[test]
    fn sma_and_ema_match_hand_computed_values() {
        let v = [1.0, 2.0, 3.0, 4.0, 5.0];
        let s = sma(&v, 3);
        assert!(s[0].is_nan() && s[1].is_nan());
        assert!((s[2] - 2.0).abs() < 1e-12);
        assert!((s[4] - 4.0).abs() < 1e-12);

        let e = ema(&v, 3);
        assert!(e[1].is_nan());
        assert!((e[2] - 2.0).abs() < 1e-12, "EMA seeds with the SMA");
        assert!((e[3] - (4.0 * 0.5 + 2.0 * 0.5)).abs() < 1e-12);
    }

    #[test]
    fn wilder_smoothing_and_stdev_are_textbook() {
        let r = rma(&[2.0, 4.0, 6.0, 8.0], 2);
        assert!((r[1] - 3.0).abs() < 1e-12);
        assert!((r[2] - (3.0 + 6.0) / 2.0).abs() < 1e-12);
        let sd = stdev(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0], 8);
        assert!((sd[7] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn true_range_uses_the_gap_when_it_dominates() {
        let bars = vec![
            Bar { time: 0, open: 9.0, high: 10.0, low: 8.0, close: 9.0, volume: None },
            Bar { time: 1, open: 11.0, high: 12.0, low: 11.0, close: 11.5, volume: None },
        ];
        let tr = true_range(&bars);
        assert!((tr[0] - 2.0).abs() < 1e-12);
        assert!((tr[1] - 3.0).abs() < 1e-12, "gap up: |high - prevClose| wins");
    }

    #[test]
    fn rsi_saturates_at_100_and_stays_bounded() {
        let rising: Vec<Bar> = (0..40).map(|i| Bar::flat(i as i64 * 60_000, 100.0 + i as f64)).collect();
        let values = rsi(&rising, 14);
        assert!((values[39] - 100.0).abs() < 1e-9);

        for v in rsi(&wavy(120), 14) {
            if v.is_nan() {
                continue;
            }
            assert!((0.0..=100.0).contains(&v), "RSI out of range: {v}");
        }
    }

    #[test]
    fn donchian_excludes_the_current_bar() {
        let bars = wavy(60);
        let (upper, _, _) = donchian(&bars, 20);
        let manual = bars[20..40].iter().fold(f64::NEG_INFINITY, |acc, b| acc.max(b.high));
        assert!((upper[40] - manual).abs() < 1e-12);
    }

    #[test]
    fn an_unknown_indicator_lists_the_real_ones() {
        let err = compute_indicators(&wavy(10), &[IndicatorSpec::new("nope")]).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("nope"), "{message}");
        assert!(message.contains("macd"), "{message}");
    }

    #[test]
    fn warmup_is_nan_rather_than_a_fabricated_number() {
        let bars = wavy(40);
        let series = compute_indicators(&bars, &[IndicatorSpec::new("atr").with("period", 14.0)]).unwrap();
        let atr = &series["atr_14.atr"];
        assert!(atr[0].is_nan() && atr[5].is_nan(), "early ATR must be NaN, not 0");
        assert!(atr[30].is_finite());
    }

    /* ---------------- the method-level three ---------------- */

    /// One hourly bar per hour from a given UTC instant.
    fn hourly(start_ms: i64, n: usize) -> Vec<Bar> {
        (0..n)
            .map(|i| {
                let t = start_ms + i as i64 * 3_600_000;
                // STRICTLY RISING, so the running mean is strictly below the
                // newest bar's typical price on every bar except the one that
                // restarts the anchor. That makes "the mean equals this bar"
                // an exact test for a restart. A cycling price gives the same
                // equality by coincidence a few bars in - it did, on the
                // first draft of this test.
                let p = 100.0 + i as f64;
                Bar { time: t, open: p, high: p + 1.0, low: p - 1.0, close: p, volume: None }
            })
            .collect()
    }

    fn utc_ms(year: i64, month: u32, day: u32, hour: i64) -> i64 {
        (fd_core::clock::days_from_civil(year, month, day) * 24 + hour) * 3_600_000
    }

    #[test]
    fn the_avwap_day_rolls_on_the_brokers_clock_in_both_seasons() {
        // THE BRANCH THE REAL-BAR FIXTURE CANNOT REACH. The 2,000 H1 bars
        // `crates/fd-api/tests/method_indicators.rs` pins against the Python
        // run May to September 2026 and are New York daylight time
        // throughout, so they exercise the +3 offset only. Here is the +2.
        //
        // The boundary moves with the offset and that is the whole point of
        // not using a UTC calendar day: in summer the broker's day rolls at
        // 21:00 UTC, in winter at 22:00. Both verified against
        // `bias_defs.py::shift_hours` on 2026-09-19.
        let restarts = |bars: &[Bar]| -> Vec<usize> {
            let vwap = anchored_vwap(bars, Some(Anchor::Day));
            // A restart is a bar whose value equals its own typical price:
            // the mean of one bar.
            (0..bars.len()).filter(|&i| (vwap[i] - bars[i].hlc3()).abs() < 1e-12).collect()
        };

        // 2026-10-29 (summer): the day must roll at 21:00 UTC, bar 3 of four
        // starting at 18:00.
        let summer = hourly(utc_ms(2026, 10, 29, 18), 6);
        assert_eq!(restarts(&summer), vec![0, 3], "summer rolls at 21:00 UTC");

        // 2026-11-03 (winter, after the 2026-11-01 changeover): 22:00 UTC.
        let winter = hourly(utc_ms(2026, 11, 3, 18), 6);
        assert_eq!(restarts(&winter), vec![0, 4], "winter rolls at 22:00 UTC");
    }

    #[test]
    fn the_avwap_week_starts_at_the_sunday_reopen() {
        // The weekend hole, found by the clock that makes it. The broker week
        // opens Sunday 17:00 New York, which is Monday 00:00 on the server's
        // clock in both seasons - 21:00 UTC in summer, 22:00 in winter -
        // because the broker's offset tracks New York's daylight rule.
        // `fd_api::htf::weeks_of` gets the same boundary from a gap of more
        // than two days between DAILY bars; intraday bars have no such gap to
        // look for, so the clock is the only way to see it.
        //
        // 2026-11-08 is a Sunday. Bars every hour from Friday 2026-11-06 18:00
        // UTC: the week must restart on the bar at Sunday 22:00 UTC and
        // nowhere else, in particular NOT at Saturday or Sunday midnight UTC.
        let bars = hourly(utc_ms(2026, 11, 6, 18), 60);
        let vwap = anchored_vwap(&bars, Some(Anchor::Week));
        let restarts: Vec<i64> = (0..bars.len())
            .filter(|&i| (vwap[i] - bars[i].hlc3()).abs() < 1e-12)
            .map(|i| bars[i].time)
            .collect();
        assert_eq!(restarts, vec![bars[0].time, utc_ms(2026, 11, 8, 22)]);
    }

    #[test]
    fn an_unknown_avwap_anchor_draws_nothing() {
        // A fallback to the session would put a line on the chart that
        // answers a question nobody asked, and it would look exactly like the
        // one that does.
        let bars = hourly(utc_ms(2026, 6, 1, 0), 48);
        assert!(Anchor::from_days(30.0).is_none());
        assert!(anchored_vwap(&bars, Anchor::from_days(30.0)).iter().all(|v| v.is_nan()));
        assert!(anchored_vwap(&bars, Anchor::from_days(1.0)).iter().all(|v| v.is_finite()));
    }

    #[test]
    fn the_zigzag_line_never_repaints() {
        // The property the whole crate is built on, asserted on the one
        // series in it that a naive implementation WOULD repaint: a zigzag
        // drawn back to the pivot's own bar rewrites bars the trader has
        // already seen. `every_indicator_is_causal` covers this too; this
        // says it in the place where somebody tempted to "join the pivots"
        // will read it.
        let bars = wavy(260);
        let spec = IndicatorSpec::new("zigzag");
        let whole = compute_indicators(&bars, std::slice::from_ref(&spec)).unwrap();
        for cut in [120, 150, 180, 210] {
            let early = compute_indicators(&bars[..cut], std::slice::from_ref(&spec)).unwrap();
            for key in ["zigzag_3_14.zigzag", "zigzag_3_14.direction"] {
                for i in 0..cut {
                    assert!(
                        fd_core::parity_eq(whole[key][i], early[key][i]),
                        "{key} at {i} changed once bar {cut} printed: {} -> {}",
                        early[key][i],
                        whole[key][i]
                    );
                }
            }
        }
        // And it is a step function, not a diagonal: between two confirmed
        // pivots the level does not move.
        let line = &whole["zigzag_3_14.zigzag"];
        let steps = (1..line.len()).filter(|&i| line[i] != line[i - 1] && line[i].is_finite()).count();
        assert!(steps > 0 && steps < line.len() / 8, "{steps} changes in {} bars is not a step line", line.len());
    }

    #[test]
    fn the_supertrend_direction_is_two_valued_once_atr_is_warm() {
        // 52/48/0 on the study's H1 file: this definition has no flat state.
        // A third value here would mean the port had invented one.
        let bars = wavy(260);
        let out = compute_indicators(&bars, &[IndicatorSpec::new("supertrend")]).unwrap();
        let dir = &out["supertrend_10_3.direction"];
        assert!(dir[8].is_nan(), "ATR(10) is not warm at bar 8");
        for (i, v) in dir.iter().enumerate().skip(9) {
            assert!(*v == 1.0 || *v == -1.0, "direction at {i} is {v}");
        }
        // The line is the band in force, so it sits below price in an up-leg
        // and above it in a down-leg. Stated as a test because a sign slip
        // here draws a plausible-looking line on the wrong side.
        let line = &out["supertrend_10_3.supertrend"];
        for i in 9..bars.len() {
            if dir[i] > 0.0 {
                assert!(line[i] < bars[i].high, "up-leg band above the bar at {i}");
            } else {
                assert!(line[i] > bars[i].low, "down-leg band below the bar at {i}");
            }
        }
    }

    #[test]
    fn every_measured_row_names_a_registered_indicator_and_its_own_sample() {
        // The table is data the catalog serves; a row for an id nobody
        // registered would be a measurement of nothing, shown to nobody, and
        // discovered by nobody either.
        for (id, rows) in MEASURED {
            assert!(definition(id).is_some(), "MEASURED has a row for unregistered `{id}`");
            assert!(!rows.is_empty(), "`{id}` has an empty measurement, which reads as measured-and-blank");
            for row in *rows {
                assert!(matches!(row.timeframe, "1h" | "4h"), "{id}: {}", row.timeframe);
                assert!(row.sample_bars > 1_000, "{id}: a sample of {} is not a measurement", row.sample_bars);
                assert!(row.source.starts_with("docs/decisions/"), "{id}: {}", row.source);
                // Every parameter named must exist on the definition, or the
                // picker cannot tell which cell the viewer is looking at.
                for (name, _) in row.params {
                    let def = definition(id).expect("registered");
                    assert!(def.params.iter().any(|(n, _)| n == name), "{id} has no parameter {name}");
                }
            }
        }
    }
}
