//! ICT "A+" setup: liquidity sweep → market structure shift → fair value gap.
//!
//! Ported from the `ICT_APlus_Sweep_MSS_FVG` MetaTrader expert's written
//! specification (Vũ Trụ EA Forex, v1.00, September 2026), not from its code,
//! which was not available. The seven-step chain it describes, in order:
//!
//! 1. A higher-timeframe fair value gap (three-bar imbalance) that is still
//!    unfilled and not too old.
//! 2. Price near that gap.
//! 3. A **liquidity sweep**: a bar trades through the last confirmed swing
//!    high (for a sell) and *closes back inside*.
//! 4. A **market structure shift**: a later bar closes through the protected
//!    swing low, with **displacement** — a body larger than the recent
//!    average and than a fraction of ATR.
//! 5. A lower-timeframe FVG left inside that impulse.
//! 6. A retracement to a level inside the gap (50% by default).
//! 7. Entry there, stop just beyond the sweep extreme, target at a multiple
//!    of the risk.
//!
//! Every step has a deadline, and a close back through the sweep extreme or
//! through the whole gap cancels the setup. One sweep produces at most one
//! trade.
//!
//! What differs from the expert, stated so the numbers are read correctly:
//!
//! * **Fills.** The expert places a limit order at the level; this engine
//!   fills at the next bar's open after the bar that touched it (the expert's
//!   own `MARKET_ON_TOUCH` mode, one bar late). Slightly worse entries, never
//!   better.
//! * **Higher timeframe.** Built from the entry bars by bucketing, so a
//!   15-minute gap on 1-minute bars is `htfFactor = 15`. Only completed
//!   buckets count.
//! * **Break-even and partial take-profit** are not modelled; the engine's
//!   stop, target and maximum hold are the exits.
//! * **Sessions** are not inside the strategy; wrap it in a
//!   [`crate::filter::Filtered`] to trade only the kill zones, and read it
//!   against a null wrapped the same way.
//!
//! Everything is recomputed from the closed bars on every call — the strategy
//! trait has no state — so the chain can never depend on what an earlier call
//! remembered, and a replay from any bar gives the same answer.

use std::collections::BTreeMap;

use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct IctSweepMssFvg;

/// One unfilled higher-timeframe gap.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Gap {
    bottom: f64,
    top: f64,
    bearish: bool,
}

/// A complete chain up to the moment of entry.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Setup {
    side: Side,
    /// The sweep bar's extreme: the stop goes just beyond it.
    sweep_extreme: f64,
    /// Entry level inside the lower-timeframe gap.
    entry: f64,
}

impl Strategy for IctSweepMssFvg {
    fn id(&self) -> &'static str {
        "ict-sweep-mss-fvg"
    }
    fn name(&self) -> &'static str {
        "ICT sweep → MSS → FVG"
    }
    fn description(&self) -> &'static str {
        "Liquidity sweep of a confirmed swing, a displacement close through structure, a fair value gap in the \
         impulse, entry on the retrace into it. Stop beyond the sweep, target at a multiple of the risk."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("pipSize", 0.1),
            ("htfFactor", 15.0),
            ("minHtfFvgPips", 15.0),
            ("maxHtfFvgAge", 60.0),
            ("htfTolerancePips", 25.0),
            ("swingLeft", 3.0),
            ("swingRight", 3.0),
            ("liquidityLookback", 150.0),
            ("minSweepPips", 1.0),
            ("maxSweepPips", 60.0),
            ("sweepBufferPips", 0.5),
            ("mssBufferPips", 0.5),
            ("structureLookback", 30.0),
            ("displacementLookback", 20.0),
            ("displacementMult", 1.5),
            ("minBodyAtrRatio", 0.5),
            ("atrPeriod", 14.0),
            ("maxBarsSweepToMss", 20.0),
            ("maxBarsMssToFvg", 5.0),
            ("maxBarsFvgToEntry", 30.0),
            ("minEntryFvgPips", 1.0),
            ("entryPercent", 50.0),
            ("slBufferPips", 3.0),
            ("riskReward", 2.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        // The same nine-cell shape as every other method, on the two knobs the
        // expert's own presets vary most.
        BTreeMap::from([
            ("displacementMult".to_string(), vec![1.2, 1.5, 2.0]),
            ("riskReward".to_string(), vec![1.5, 2.0, 3.0]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("swing").with("left", p.get("swingLeft")).with("right", p.get("swingRight")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let swing = format!("swing_{}_{}", p.get("swingLeft"), p.get("swingRight"));
        vec![
            format!("{swing}.high"),
            format!("{swing}.low"),
            format!("{swing}.highAt"),
            format!("{swing}.lowAt"),
            format!("atr_{}", p.get("atrPeriod")),
        ]
    }
    fn warmup(&self, p: &Params) -> usize {
        let htf = p.period("htfFactor") * (p.period("maxHtfFvgAge") + 3);
        htf.max(p.period("liquidityLookback")).max(p.period("atrPeriod") + p.period("displacementLookback")) + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let Some(setup) = find_setup(ctx) else { return Intent::None };
        let p = ctx.params;
        let pip = p.get("pipSize");
        let (stop, target) = match setup.side {
            Side::Short => {
                let stop = setup.sweep_extreme + p.get("slBufferPips") * pip;
                (stop, setup.entry - (stop - setup.entry) * p.get("riskReward"))
            }
            Side::Long => {
                let stop = setup.sweep_extreme - p.get("slBufferPips") * pip;
                (stop, setup.entry + (setup.entry - stop) * p.get("riskReward"))
            }
        };
        Intent::Enter {
            side: setup.side,
            stop: Some(stop),
            target: Some(target),
            reason: format!(
                "{} sweep at {:.2}, MSS with displacement, retrace into the gap at {:.2}",
                if setup.side.is_long() { "sell-side" } else { "buy-side" },
                setup.sweep_extreme,
                setup.entry
            ),
        }
    }
}

const SWING_HIGH: usize = 0;
const SWING_LOW: usize = 1;
const SWING_HIGH_AT: usize = 2;
const SWING_LOW_AT: usize = 3;
const ATR: usize = 4;

/// The most recent sweep whose chain completes with a first touch of the
/// entry level on this bar, if any.
fn find_setup(ctx: &BarContext) -> Option<Setup> {
    let p = ctx.params;
    let i = ctx.i;
    let bars = &ctx.bars[..=i];
    let pip = p.get("pipSize");

    let to_mss = p.period("maxBarsSweepToMss");
    let to_fvg = p.period("maxBarsMssToFvg");
    let to_entry = p.period("maxBarsFvgToEntry");
    let horizon = to_mss + to_fvg + to_entry + 3;
    let sweep_buffer = p.get("sweepBufferPips") * pip;
    let (min_sweep, max_sweep) = (p.get("minSweepPips") * pip, p.get("maxSweepPips") * pip);

    // Latest sweep first: if two chains would both fire on this bar, the more
    // recent sweep is the one the expert would be in.
    let earliest = i.saturating_sub(horizon).max(1);
    for s in (earliest..=i.saturating_sub(4)).rev() {
        for side in [Side::Short, Side::Long] {
            let (level, level_at) = match side {
                Side::Short => (ctx.s_back(SWING_HIGH, i - (s - 1)), ctx.s_back(SWING_HIGH_AT, i - (s - 1))),
                Side::Long => (ctx.s_back(SWING_LOW, i - (s - 1)), ctx.s_back(SWING_LOW_AT, i - (s - 1))),
            };
            if !level.is_finite() || !level_at.is_finite() {
                continue;
            }
            if s - level_at as usize > p.period("liquidityLookback") {
                continue;
            }
            let bar = &bars[s];
            let (extreme, depth, closed_inside) = match side {
                Side::Short => (bar.high, bar.high - level, bar.close < level),
                Side::Long => (bar.low, level - bar.low, bar.close > level),
            };
            if !(closed_inside && depth > sweep_buffer && depth >= min_sweep && depth <= max_sweep) {
                continue;
            }
            if let Some(setup) = chain_from_sweep(ctx, bars, s, side, extreme, level) {
                return Some(setup);
            }
        }
    }
    None
}

/// Walk the chain forward from a confirmed sweep at `s`.
fn chain_from_sweep(ctx: &BarContext, bars: &[Bar], s: usize, side: Side, extreme: f64, swept: f64) -> Option<Setup> {
    let p = ctx.params;
    let i = bars.len() - 1;
    let pip = p.get("pipSize");
    let short = matches!(side, Side::Short);

    // 1–2. A higher-timeframe gap in the trade's direction, unfilled when the
    // sweep happened, with the sweep extreme inside or near it.
    let tolerance = p.get("htfTolerancePips") * pip;
    let gap = nearest_htf_gap(bars, s, p, !short, extreme)?;
    if extreme < gap.bottom - tolerance || extreme > gap.top + tolerance {
        return None;
    }

    // Protected structure: the last confirmed swing on the other side, else
    // the extreme of the bars leading into the sweep.
    let structure_slot = if short { SWING_LOW } else { SWING_HIGH };
    let mut protected = ctx.s_back(structure_slot, i - s);
    if !protected.is_finite() {
        let from = s.saturating_sub(p.period("structureLookback"));
        protected = if short {
            bars[from..s].iter().map(|b| b.low).fold(f64::INFINITY, f64::min)
        } else {
            bars[from..s].iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max)
        };
    }
    // The protected level must sit on the far side of the swept one, or the
    // "shift" would be a close back to where price already was.
    if short && protected >= swept || !short && protected <= swept {
        return None;
    }

    // 4. Market structure shift with displacement, before the deadline, with
    // no close back through the sweep extreme on the way.
    let mss_buffer = p.get("mssBufferPips") * pip;
    let lookback = p.period("displacementLookback");
    let mult = p.get("displacementMult");
    let atr_ratio = p.get("minBodyAtrRatio");
    let deadline = (s + p.period("maxBarsSweepToMss")).min(i.saturating_sub(2));
    let mut mss = None;
    for m in (s + 1)..=deadline {
        let b = &bars[m];
        if extreme_broken(b, short, extreme) {
            return None;
        }
        let shifted = if short { b.close < protected - mss_buffer } else { b.close > protected + mss_buffer };
        if !shifted {
            continue;
        }
        let body = (b.close - b.open).abs();
        let from = m.saturating_sub(lookback);
        let avg_body = bars[from..m].iter().map(|x| (x.close - x.open).abs()).sum::<f64>() / (m - from).max(1) as f64;
        let atr = ctx.s_back(ATR, i - m);
        let displaced = body >= avg_body * mult && (atr_ratio <= 0.0 || (atr.is_finite() && body >= atr * atr_ratio));
        if displaced {
            mss = Some(m);
            break;
        }
    }
    let m = mss?;

    // 5. The first gap the impulse leaves, within a few bars of the shift.
    let min_gap = p.get("minEntryFvgPips") * pip;
    let fvg_deadline = (m + p.period("maxBarsMssToFvg")).min(i - 1);
    let mut fvg = None;
    for c in (m + 1)..=fvg_deadline {
        if extreme_broken(&bars[c], short, extreme) {
            return None;
        }
        let (a, cc) = (&bars[c - 2], &bars[c]);
        let zone = if short {
            (a.low > cc.high).then_some((cc.high, a.low))
        } else {
            (a.high < cc.low).then_some((a.high, cc.low))
        };
        if let Some((bottom, top)) = zone
            && top - bottom >= min_gap
        {
            fvg = Some((c, bottom, top));
            break;
        }
    }
    let (c, bottom, top) = fvg?;
    let entry = bottom + (top - bottom) * p.get("entryPercent") / 100.0;

    // 6–7. The retrace: this bar is the first to reach the level, nothing in
    // between cancelled the setup, and the deadline has not passed.
    if i - c > p.period("maxBarsFvgToEntry") {
        return None;
    }
    for b in &bars[c + 1..i] {
        if extreme_broken(b, short, extreme) || gap_invalidated(b, short, bottom, top) {
            return None;
        }
        let touched = if short { b.high >= entry } else { b.low <= entry };
        if touched {
            // One sweep, one trade: the first touch was the entry, whatever
            // happened to it.
            return None;
        }
    }
    let now = &bars[i];
    let touched = if short { now.high >= entry } else { now.low <= entry };
    if !touched || extreme_broken(now, short, extreme) || gap_invalidated(now, short, bottom, top) {
        return None;
    }
    Some(Setup { side, sweep_extreme: extreme, entry })
}

fn extreme_broken(bar: &Bar, short: bool, extreme: f64) -> bool {
    if short { bar.close > extreme } else { bar.close < extreme }
}

/// A close through the whole gap, on the far side from the entry.
fn gap_invalidated(bar: &Bar, short: bool, bottom: f64, top: f64) -> bool {
    if short { bar.close > top } else { bar.close < bottom }
}

/// Higher-timeframe gaps as they stood at bar `s`, the one nearest `price`.
///
/// Buckets the entry bars by `htfFactor × step`; only completed buckets are
/// read, as the expert reads closed bars. A gap is unfilled while no later
/// bucket has traded through its far edge.
fn nearest_htf_gap(bars: &[Bar], s: usize, p: &Params, bullish: bool, price: f64) -> Option<Gap> {
    let factor = p.period("htfFactor").max(1) as i64;
    let max_age = p.period("maxHtfFvgAge");
    let min_size = p.get("minHtfFvgPips") * p.get("pipSize");
    let step = bar_step(bars)?;
    let htf_ms = step * factor;

    let from = s.saturating_sub((factor as usize) * (max_age + 3));
    // Aggregate into buckets, dropping the one the sweep bar sits in: it is
    // still forming from the higher timeframe's point of view.
    let current_bucket = bars[s].time.div_euclid(htf_ms);
    let mut buckets: Vec<Bar> = Vec::with_capacity(max_age + 4);
    for bar in &bars[from..=s] {
        let bucket = bar.time.div_euclid(htf_ms);
        if bucket == current_bucket {
            break;
        }
        match buckets.last_mut() {
            Some(last) if last.time == bucket * htf_ms => {
                last.high = last.high.max(bar.high);
                last.low = last.low.min(bar.low);
                last.close = bar.close;
            }
            _ => buckets.push(Bar { time: bucket * htf_ms, open: bar.open, high: bar.high, low: bar.low, close: bar.close, volume: None }),
        }
    }
    if buckets.len() < 3 {
        return None;
    }

    let mut best: Option<(f64, Gap)> = None;
    for c in 2..buckets.len() {
        let (a, cc) = (&buckets[c - 2], &buckets[c]);
        let gap = if bullish {
            (a.high < cc.low).then_some(Gap { bottom: a.high, top: cc.low, bearish: false })
        } else {
            (a.low > cc.high).then_some(Gap { bottom: cc.high, top: a.low, bearish: true })
        };
        let Some(gap) = gap else { continue };
        if gap.top - gap.bottom < min_size || buckets.len() - 1 - c > max_age {
            continue;
        }
        let filled = buckets[c + 1..].iter().any(|b| if bullish { b.low <= gap.bottom } else { b.high >= gap.top });
        if filled {
            continue;
        }
        let distance = if price < gap.bottom {
            gap.bottom - price
        } else if price > gap.top {
            price - gap.top
        } else {
            0.0
        };
        if best.is_none_or(|(d, _)| distance < d) {
            best = Some((distance, gap));
        }
    }
    best.map(|(_, gap)| gap)
}

/// The bar interval, from the first two bars.
fn bar_step(bars: &[Bar]) -> Option<i64> {
    bars.windows(2).map(|w| w[1].time - w[0].time).find(|d| *d > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_indicators::{IndicatorSet, compute_indicators};

    const MIN: i64 = 60_000;

    fn bar(i: usize, o: f64, h: f64, l: f64, c: f64) -> Bar {
        Bar { time: i as i64 * MIN, open: o, high: h, low: l, close: c, volume: None }
    }

    /// A minute series that walks the whole sell chain:
    /// a bearish 15-minute gap overhead, a rally into it that sweeps a
    /// confirmed swing high and closes back, a displacement close through
    /// the protected low, a one-minute gap in that impulse, and a retrace
    /// into it.
    fn sell_chain() -> (Vec<Bar>, usize) {
        let mut bars = Vec::new();
        let mut i = 0usize;
        // Bucket 0..3 (45 minutes): a sell-off leaving a bearish HTF gap
        // between bucket 0's low (4410) and bucket 2's high (4400).
        for _ in 0..15 {
            bars.push(bar(i, 4412.0, 4413.0, 4410.0, 4411.0));
            i += 1;
        }
        for _ in 0..15 {
            bars.push(bar(i, 4408.0, 4409.0, 4402.0, 4403.0));
            i += 1;
        }
        for _ in 0..15 {
            bars.push(bar(i, 4399.0, 4400.0, 4396.0, 4397.0));
            i += 1;
        }
        // Chop below the gap for a while so the buckets complete and the swing
        // low is confirmed: a dip to 4390 is the protected low.
        for k in 0..30 {
            let l = if k == 10 { 4390.0 } else { 4394.0 };
            bars.push(bar(i, 4396.0, 4397.0, l, 4396.0));
            i += 1;
        }
        // A local swing high at 4401 (confirmed by three lower bars either side).
        bars.push(bar(i, 4396.0, 4401.0, 4395.0, 4398.0));
        i += 1;
        for _ in 0..4 {
            bars.push(bar(i, 4398.0, 4399.0, 4396.0, 4397.0));
            i += 1;
        }
        // The sweep: trades through 4401 into the HTF gap and closes back inside.
        bars.push(bar(i, 4397.0, 4403.0, 4396.0, 4399.0));
        let sweep = i;
        i += 1;
        // Drift, then the displacement bar closing through 4390.
        bars.push(bar(i, 4399.0, 4399.5, 4397.0, 4398.0));
        i += 1;
        bars.push(bar(i, 4398.0, 4398.5, 4388.0, 4388.5)); // MSS: body 9.5, huge
        i += 1;
        // The impulse continues. The first gap after the shift has the MSS bar
        // as its middle candle: low(A = drift bar) 4397 > high(C) 4388.6, so
        // the zone is 4388.6..4397 and its midpoint 4392.8.
        bars.push(bar(i, 4388.5, 4388.6, 4385.0, 4385.5));
        i += 1;
        bars.push(bar(i, 4385.5, 4386.0, 4383.0, 4384.0));
        i += 1;
        // Retrace touches 4392.8 on this bar.
        bars.push(bar(i, 4384.0, 4393.0, 4383.5, 4390.0));
        (bars, sweep)
    }

    fn context<'a>(bars: &'a [Bar], ind: &'a IndicatorSet, series: &'a [&'a [f64]], params: &'a Params, i: usize) -> BarContext<'a> {
        BarContext { bar: &bars[i], i, bars, ind, series, options: None, position: None, params }
    }

    #[test]
    fn the_sell_chain_enters_on_the_first_touch_of_the_gap_midpoint() {
        let (bars, _) = sell_chain();
        let strategy = IctSweepMssFvg;
        let mut params = strategy.default_params();
        params.set("maxHtfFvgAge", 10.0);
        params.set("minHtfFvgPips", 50.0); // the HTF gap is 4400..4410 = 100 pips
        let ind = compute_indicators(&bars, &strategy.indicators(&params)).unwrap();
        let keys = strategy.series(&params);
        let series: Vec<&[f64]> = keys.iter().map(|k| &ind[k][..]).collect();

        let last = bars.len() - 1;
        let intent = strategy.on_bar(&context(&bars, &ind, &series, &params, last));
        let Intent::Enter { side, stop, target, .. } = intent else { panic!("expected an entry, got {intent:?}") };
        assert_eq!(side, Side::Short);
        // Stop just beyond the sweep high 4403 (+3 pips); entry at the gap
        // midpoint 4392.8; target two risks below.
        let stop = stop.unwrap();
        assert!((stop - 4403.3).abs() < 1e-9, "stop {stop}");
        let target = target.unwrap();
        let risk = stop - 4392.8;
        assert!((target - (4392.8 - 2.0 * risk)).abs() < 1e-9, "target {target}");

        // The bar before the touch has no signal, and a second touch later
        // would not fire again.
        assert_eq!(strategy.on_bar(&context(&bars, &ind, &series, &params, last - 1)), Intent::None);
        let mut again = bars.clone();
        again.push(bar(bars.len(), 4390.0, 4393.5, 4389.0, 4391.0));
        let ind2 = compute_indicators(&again, &strategy.indicators(&params)).unwrap();
        let series2: Vec<&[f64]> = keys.iter().map(|k| &ind2[k][..]).collect();
        assert_eq!(strategy.on_bar(&context(&again, &ind2, &series2, &params, again.len() - 1)), Intent::None);
    }

    #[test]
    fn a_close_back_through_the_sweep_high_cancels_the_setup() {
        let (mut bars, _) = sell_chain();
        // Replace the drift bar after the sweep with a close above 4403.
        let n = bars.len();
        bars[n - 5] = bar(n - 5, 4399.0, 4405.0, 4398.0, 4404.0);
        let strategy = IctSweepMssFvg;
        let mut params = strategy.default_params();
        params.set("maxHtfFvgAge", 10.0);
        params.set("minHtfFvgPips", 50.0);
        let ind = compute_indicators(&bars, &strategy.indicators(&params)).unwrap();
        let keys = strategy.series(&params);
        let series: Vec<&[f64]> = keys.iter().map(|k| &ind[k][..]).collect();
        assert_eq!(strategy.on_bar(&context(&bars, &ind, &series, &params, n - 1)), Intent::None);
    }

    #[test]
    fn without_a_higher_timeframe_gap_there_is_no_setup() {
        let (bars, _) = sell_chain();
        let strategy = IctSweepMssFvg;
        let mut params = strategy.default_params();
        params.set("maxHtfFvgAge", 10.0);
        params.set("minHtfFvgPips", 500.0); // demand a gap larger than exists
        let ind = compute_indicators(&bars, &strategy.indicators(&params)).unwrap();
        let keys = strategy.series(&params);
        let series: Vec<&[f64]> = keys.iter().map(|k| &ind[k][..]).collect();
        assert_eq!(strategy.on_bar(&context(&bars, &ind, &series, &params, bars.len() - 1)), Intent::None);
    }
}
