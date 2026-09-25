//! RSI exhaustion, a reversal candle, both oscillators turning, and volume.
//!
//! Built 2026-09-25 to the owner's description of an MT5 expert advisor he was
//! shown, reproduced here so it can be measured against this desk's own
//! machinery instead of against a screenshot. His words, and every one is a
//! condition below: "no danh theo rsi qua ban va qua mua ket hop mo hinh nen
//! dao chieu + volume", then the BUY rule spelled out:
//!
//!   reversal candle
//!   + RSI below the BUY threshold
//!   + RSI just turned up
//!   + Stoch RSI just turned up
//!   + volume above the average of the last 30 bars
//!   -> BUY
//!
//! SELL is the mirror. ALL FIVE must hold on the same bar; this is an AND, not
//! a count, and nothing here scores anything.
//!
//! WHAT THE SCREENSHOT FIXED, and is reproduced rather than chosen: RSI 14,
//! Stoch 14 with K 3 and D 3, thresholds 30 and 70, stop 16.0 and target 6.0 in
//! price units, one timeframe per instance (he runs M1, M5 and M15 separately).
//!
//! THE REWARD-TO-RISK IS 6/16 = 0.375, AND THAT IS THE WHOLE CHARACTER OF THIS
//! RULE. Risking sixteen to make six needs a win rate above 16/(16+6) = 72.7%
//! before costs just to break even. It is a high-win-rate mean-reversion scalp,
//! so its equity curve climbs smoothly and its losses arrive in lumps almost
//! three times the size of its wins. Read its drawdown, not its win rate.
//!
//! THE CANDLE PATTERNS ARE DEFINED HERE RATHER THAN ASSUMED, because "Morning
//! Star" names a family and the boundaries decide how often it fires. Each
//! definition below is the textbook one with its tolerances written as
//! parameters, so a reader can see what was chosen and a sweep can move it.
//!
//! Needs real volume. The Vantage XAUUSD.sc feed carries tick volume (measured
//! 2026-09-25: forty consecutive 15m bars, none zero, 4,132 to 8,278); the
//! Dukascopy gold series does not, and on it this strategy takes no trade
//! rather than pretending the filter passed.

use std::collections::BTreeMap;

use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

// `enter` and `grid` are per-module helpers on this desk rather than shared
// exports - screen.rs, builtin.rs and volman_box.rs each carry their own. These
// are screen.rs's, copied verbatim so this strategy's entries are built by the
// same arithmetic as the ones it will be compared against.
fn key(id: &str, values: &[f64]) -> String {
    let joined = values.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join("_");
    format!("{id}_{joined}")
}

fn grid(entries: &[(&str, &[f64])]) -> BTreeMap<String, Vec<f64>> {
    entries.iter().map(|(k, v)| ((*k).to_string(), v.to_vec())).collect()
}

fn enter(side: Side, close: f64, stop: f64, rr: f64, reason: String) -> Intent {
    let risk = (close - stop).abs();
    if risk.is_nan() || risk <= 0.0 {
        return Intent::None;
    }
    let target = if side.is_long() { close + risk * rr } else { close - risk * rr };
    Intent::Enter { side, stop: Some(stop), target: Some(target), reason }
}

pub struct RsiReversalVol;

/// A bullish or bearish reversal candle, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reversal {
    Bull(&'static str),
    Bear(&'static str),
    None,
}

/// The body, the total range, and the two shadows of a bar.
fn shape(b: &Bar) -> (f64, f64, f64, f64) {
    let body = (b.close - b.open).abs();
    let range = b.high - b.low;
    let upper = b.high - b.close.max(b.open);
    let lower = b.close.min(b.open) - b.low;
    (body, range, upper, lower)
}

/// Marubozu: a body that is nearly the whole range, so both shadows are small.
///
/// `max_shadow` is each shadow's allowance as a fraction of the range, so 0.05
/// means "no shadow longer than five per cent of the bar".
fn marubozu(b: &Bar, max_shadow: f64) -> Reversal {
    let (body, range, upper, lower) = shape(b);
    if range <= 0.0 || body / range < 1.0 - 2.0 * max_shadow {
        return Reversal::None;
    }
    if upper / range > max_shadow || lower / range > max_shadow {
        return Reversal::None;
    }
    if b.close > b.open {
        Reversal::Bull("bullish marubozu")
    } else if b.close < b.open {
        Reversal::Bear("bearish marubozu")
    } else {
        Reversal::None
    }
}

/// Outside bar: this bar engulfs the previous bar's whole RANGE - not just its
/// body - and closes in the new direction.
///
/// The range form is the stricter of the two common definitions and is chosen
/// deliberately: a body-engulfing bar inside the previous range is a much more
/// frequent event, and the looser the pattern the more this rule is simply
/// trading the RSI filter.
fn outside_bar(prev: &Bar, b: &Bar) -> Reversal {
    if !(b.high > prev.high && b.low < prev.low) {
        return Reversal::None;
    }
    if b.close > b.open && b.close > prev.high.min(prev.close.max(prev.open)) {
        Reversal::Bull("bullish outside bar")
    } else if b.close < b.open && b.close < prev.low.max(prev.close.min(prev.open)) {
        Reversal::Bear("bearish outside bar")
    } else {
        Reversal::None
    }
}

/// Morning star (and its evening mirror), with the doji variant named apart.
///
/// Three bars: a long bar down, a small-bodied bar that gaps or stalls below
/// it, and a bar closing back above the midpoint of the first. `doji_body` is
/// the fraction of the middle bar's range under which the middle bar is called
/// a doji, which is the only difference between "Morning Star" and "Morning
/// Doji Star" - the owner's list names both, so both are here and the reason is
/// reported separately.
fn star(first: &Bar, middle: &Bar, last: &Bar, doji_body: f64, small_body: f64) -> Reversal {
    let (fb, fr, _, _) = shape(first);
    let (mb, mr, _, _) = shape(middle);
    let (lb, lr, _, _) = shape(last);
    if fr <= 0.0 || mr <= 0.0 || lr <= 0.0 {
        return Reversal::None;
    }
    // The middle bar must be indecisive: a small body relative to its own range.
    let middle_frac = mb / mr;
    if middle_frac > small_body {
        return Reversal::None;
    }
    let is_doji = middle_frac <= doji_body;
    // The first bar must be a real move, not a doji of its own.
    if fb / fr < small_body {
        return Reversal::None;
    }
    let first_mid = (first.open + first.close) / 2.0;

    // Morning star: first falls, middle sits below it, last closes back above
    // the first bar's midpoint.
    if first.close < first.open
        && middle.high < first.close.max(first.open)
        && last.close > last.open
        && last.close > first_mid
        && lb / lr >= small_body
    {
        return Reversal::Bull(if is_doji { "morning doji star" } else { "morning star" });
    }
    // Evening star, the mirror.
    if first.close > first.open
        && middle.low > first.close.min(first.open)
        && last.close < last.open
        && last.close < first_mid
        && lb / lr >= small_body
    {
        return Reversal::Bear(if is_doji { "evening doji star" } else { "evening star" });
    }
    Reversal::None
}

/// The reversal candle on this bar, if any. The three-bar patterns are checked
/// first so a star that is also an outside bar is named as the star.
fn reversal_at(bars: &[Bar], i: usize, doji_body: f64, small_body: f64, max_shadow: f64) -> Reversal {
    if i >= 2 {
        let r = star(&bars[i - 2], &bars[i - 1], &bars[i], doji_body, small_body);
        if r != Reversal::None {
            return r;
        }
    }
    if i >= 1 {
        let r = outside_bar(&bars[i - 1], &bars[i]);
        if r != Reversal::None {
            return r;
        }
    }
    marubozu(&bars[i], max_shadow)
}

/// Mean volume of the `n` bars BEFORE `i`, or `None` when the feed has none.
///
/// `None`, not zero and not one: a series without volume must make this rule
/// take no trade, rather than pass a filter it cannot evaluate. Dukascopy gold
/// is exactly that series.
fn mean_volume(bars: &[Bar], i: usize, n: usize) -> Option<f64> {
    if n == 0 || i < n {
        return None;
    }
    let mut total = 0.0;
    for b in &bars[i - n..i] {
        match b.volume {
            Some(v) if v > 0.0 => total += v,
            _ => return None,
        }
    }
    Some(total / n as f64)
}

impl Strategy for RsiReversalVol {
    fn id(&self) -> &'static str {
        "rsi-reversal-vol"
    }

    fn name(&self) -> &'static str {
        "RSI reversal with volume"
    }

    fn description(&self) -> &'static str {
        "Five conditions on one bar: a reversal candle, RSI past its threshold, RSI turning back, \
         Stoch RSI turning back, and volume above its 30-bar mean. Stop and target are fixed price \
         distances, so the reward-to-risk is 0.375 and the rule needs a win rate above 72.7% to \
         break even before costs."
    }

    fn default_params(&self) -> Params {
        Params::new(&[
            ("rsiPeriod", 14.0),
            ("stochPeriod", 14.0),
            ("smoothK", 3.0),
            ("smoothD", 3.0),
            ("buyRsi", 30.0),
            ("sellRsi", 70.0),
            ("volBars", 30.0),
            ("volMult", 1.0),
            ("stopPrice", 16.0),
            ("targetPrice", 6.0),
            ("dojiBody", 0.1),
            ("smallBody", 0.35),
            ("maxShadow", 0.05),
        ])
    }

    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        // Only the two the owner's screenshot leaves open. The candle
        // tolerances are NOT swept: a pattern whose definition moves with the
        // sweep is not a pattern, it is a free parameter wearing a name.
        grid(&[("buyRsi", &[25.0, 30.0, 35.0]), ("volMult", &[1.0, 1.2, 1.5])])
    }

    fn series(&self, p: &Params) -> Vec<String> {
        // A DOT separates an indicator key from its output, not a colon. The
        // first version of this file used `rsi_14:rsi` and `stochrsi_..:k`,
        // which match nothing, so `ctx.s()` returned NaN on every bar and the
        // rule took ZERO trades over 100,586 gold bars and 100,798 BTC ones
        // without a single error anywhere. The keys the set really holds are
        // `rsi_14.rsi` and `stochrsi_14_14_3_3.k`, printed by
        // crates/fd-backtest/src/bin/why_no_trade.rs, which is how this was
        // found rather than guessed.
        let base_rsi = key("rsi", &[p.get("rsiPeriod")]);
        let base_stoch = key(
            "stochrsi",
            &[p.get("rsiPeriod"), p.get("stochPeriod"), p.get("smoothK"), p.get("smoothD")],
        );
        vec![
            format!("{base_rsi}.rsi"),
            format!("{base_stoch}.k"),
            format!("{base_stoch}.d"),
        ]
    }

    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("rsi").with("period", p.get("rsiPeriod")),
            IndicatorSpec::new("stochrsi")
                .with("rsiPeriod", p.get("rsiPeriod"))
                .with("period", p.get("stochPeriod"))
                .with("smoothK", p.get("smoothK"))
                .with("smoothD", p.get("smoothD")),
        ]
    }

    fn warmup(&self, p: &Params) -> usize {
        // RSI, then the stochastic window on top of it, then the two smoothings,
        // and the volume mean needs its own bars. Plus two for the three-bar
        // star and one for the "just turned" comparison.
        p.period("rsiPeriod")
            + p.period("stochPeriod")
            + p.period("smoothK")
            + p.period("smoothD")
            + p.period("volBars")
            + 3
    }

    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let i = ctx.i;
        if i < 2 {
            return Intent::None;
        }

        // RSI now and one bar back; Stoch RSI k now and one bar back.
        let (rsi, k) = (ctx.s(0), ctx.s(1));
        let (rsi_prev, k_prev) = (ctx.s_back(0, 1), ctx.s_back(1, 1));
        if ![rsi, k, rsi_prev, k_prev].iter().all(|v| v.is_finite()) {
            return Intent::None;
        }

        // Volume above its own recent mean. No volume on the feed means no
        // trade - the filter is a condition, not a formality.
        let Some(mean) = mean_volume(ctx.bars, i, p.period("volBars")) else {
            return Intent::None;
        };
        let Some(vol) = ctx.bar.volume else { return Intent::None };
        if vol <= mean * p.get("volMult") {
            return Intent::None;
        }

        let pattern = reversal_at(
            ctx.bars,
            i,
            p.get("dojiBody"),
            p.get("smallBody"),
            p.get("maxShadow"),
        );

        let close = ctx.bar.close;
        let stop_distance = p.get("stopPrice");
        let target_distance = p.get("targetPrice");
        if stop_distance <= 0.0 || target_distance <= 0.0 {
            return Intent::None;
        }
        // The engine derives the target from reward_risk, so the two fixed
        // price distances become one ratio. 6 / 16 = 0.375.
        let rr = target_distance / stop_distance;

        // BUY: all five, on this bar.
        if let Reversal::Bull(what) = pattern {
            if rsi < p.get("buyRsi") && rsi > rsi_prev && k > k_prev {
                return enter(
                    Side::Long,
                    close,
                    close - stop_distance,
                    rr,
                    format!("{what}, RSI {rsi:.1} under {:.0} and turning up, Stoch RSI up, volume {:.0} over {:.0}",
                            p.get("buyRsi"), vol, mean),
                );
            }
        }
        // SELL: the mirror.
        if let Reversal::Bear(what) = pattern {
            if rsi > p.get("sellRsi") && rsi < rsi_prev && k < k_prev {
                return enter(
                    Side::Short,
                    close,
                    close + stop_distance,
                    rr,
                    format!("{what}, RSI {rsi:.1} over {:.0} and turning down, Stoch RSI down, volume {:.0} over {:.0}",
                            p.get("sellRsi"), vol, mean),
                );
            }
        }
        Intent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(o: f64, h: f64, l: f64, c: f64, v: f64) -> Bar {
        Bar { time: 0, open: o, high: h, low: l, close: c, volume: Some(v) }
    }

    #[test]
    fn a_marubozu_needs_both_shadows_small() {
        // Body 10, range 10.2: shadows 0.1 each, one per cent of the range.
        assert_eq!(marubozu(&bar(100.0, 110.1, 99.9, 110.0, 1.0), 0.05), Reversal::Bull("bullish marubozu"));
        // Same body, a long upper shadow: not a marubozu.
        assert_eq!(marubozu(&bar(100.0, 118.0, 99.9, 110.0, 1.0), 0.05), Reversal::None);
        // Bearish mirror.
        assert_eq!(marubozu(&bar(110.0, 110.1, 99.9, 100.0, 1.0), 0.05), Reversal::Bear("bearish marubozu"));
    }

    #[test]
    fn an_outside_bar_must_engulf_the_whole_range_not_just_the_body() {
        let prev = bar(100.0, 105.0, 95.0, 96.0, 1.0);
        // Engulfs high and low, closes up.
        assert_eq!(outside_bar(&prev, &bar(96.0, 106.0, 94.0, 105.5, 1.0)), Reversal::Bull("bullish outside bar"));
        // Bigger body but INSIDE the previous range: refused.
        assert_eq!(outside_bar(&prev, &bar(96.0, 104.0, 95.5, 103.0, 1.0)), Reversal::None);
    }

    #[test]
    fn the_doji_variant_is_named_apart_from_the_plain_star() {
        let first = bar(110.0, 110.5, 99.5, 100.0, 1.0);
        let last = bar(101.0, 108.0, 100.5, 107.0, 1.0);
        // Middle body 0.2 of a 4.0 range = 5%, under dojiBody 0.1.
        let doji_mid = bar(99.0, 101.0, 97.0, 99.2, 1.0);
        assert_eq!(star(&first, &doji_mid, &last, 0.1, 0.35), Reversal::Bull("morning doji star"));
        // Middle body 0.8 of 4.0 = 20%: small enough to be a star, too big for a doji.
        let plain_mid = bar(99.0, 101.0, 97.0, 98.2, 1.0);
        assert_eq!(star(&first, &plain_mid, &last, 0.1, 0.35), Reversal::Bull("morning star"));
    }

    #[test]
    fn a_feed_without_volume_yields_no_mean_and_therefore_no_trade() {
        let mut bars = vec![bar(1.0, 1.0, 1.0, 1.0, 5.0); 40];
        assert!(mean_volume(&bars, 35, 30).is_some());
        bars[10].volume = None;
        // The window 5..35 now contains a bar with no volume.
        assert_eq!(mean_volume(&bars, 35, 30), None);
        // Zero is treated the same way: it is not a measurement.
        bars[10].volume = Some(0.0);
        assert_eq!(mean_volume(&bars, 35, 30), None);
    }

    #[test]
    fn the_series_keys_are_the_ones_the_indicator_set_really_holds() {
        // Pinned because getting this wrong is SILENT: a key that matches
        // nothing yields NaN, every condition fails, and the rule takes no
        // trade with no error to notice. That is exactly what happened.
        let s = RsiReversalVol.series(&RsiReversalVol.default_params());
        assert_eq!(s[0], "rsi_14.rsi");
        assert_eq!(s[1], "stochrsi_14_14_3_3.k");
        assert_eq!(s[2], "stochrsi_14_14_3_3.d");
        assert!(s.iter().all(|k| !k.contains(':')), "a colon matches no key on this desk");
    }

    #[test]
    fn the_reward_to_risk_is_the_two_fixed_distances() {
        let p = RsiReversalVol.default_params();
        assert!((p.get("targetPrice") / p.get("stopPrice") - 0.375).abs() < 1e-12);
    }

    #[test]
    fn warmup_covers_every_window_the_rule_reads() {
        let p = RsiReversalVol.default_params();
        let w = RsiReversalVol.warmup(&p);
        assert!(w >= p.period("volBars") + 3, "the volume mean and the three-bar star must fit");
        assert!(w >= p.period("rsiPeriod") + p.period("stochPeriod"), "stoch rsi stacks on rsi");
    }
}
