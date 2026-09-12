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
];

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
        other => unreachable!("indicator {other} is registered but not implemented"),
    }
}

/* ---------------- primitives ---------------- */

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
}
