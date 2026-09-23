//! A gold move the second metal did not share, faded.
//!
//! # The mechanism, before any fitting
//!
//! Gold and silver share one factor — the price of precious metal — and the
//! sharing is large and contemporaneous: over 2010-06-01 → 2025-09-22 the
//! correlation of their fifteen-minute log returns on the same Dukascopy clock
//! is **+0.701 at lag zero and below +0.01 at every lag from −4 to +4 bars**
//! (356,617 paired bars). So when gold moves and silver does not move with it,
//! one of two things happened: news about gold alone, or a liquidity event in
//! gold alone. The second reverts and the first does not, and the second is
//! the commoner of the two at fifteen minutes.
//!
//! That is a statement about two markets' shared factor, not about a shape on
//! a chart, and it is the part of this method that existed before anything was
//! measured. What was measured is in
//! `docs/research/designs/2026-09-23-designed-4-two-series.md`, **including
//! the reason the desk did not adopt it**: the retracement is real in the mean
//! and is not tradable at this cost level. Read that note before reading a
//! number out of this file.
//!
//! # The rule
//!
//! On bar `i`, with `moveBars` of contiguous history behind it:
//!
//! * gold's change over `moveBars` bars, divided by its own ATR, exceeds
//!   `moveAtr` in absolute value;
//! * the companion's change over the same `moveBars` of **its own** bars,
//!   ending at the same timestamp, divided by **its own** ATR, is less than
//!   `companionAtr` when signed by gold's direction — that is, the companion
//!   did not go along;
//!
//! then enter against gold's move, stop `stopAtr` gold ATRs from the signal
//! bar's close. The engine fills at the next bar's open, derives the target
//! from `[trading] reward_risk`, and owns the maximum hold.
//!
//! # What makes this two series rather than one
//!
//! Only the second condition. The single-series version of the same rule —
//! fade any gold move larger than `moveAtr` — is the control, and on the
//! design window its gross mean is +0.00 to +0.03 points per trade while this
//! one's is +0.4 to +1.4. The control is in the note, with counts.
//!
//! # Where the companion comes from
//!
//! `fd_indicators::companion`, installed once by the binary that owns the data
//! directory (`search --companion=<SYMBOL>`), matched on an exact timestamp.
//! **Nothing installed means every companion series is NaN, and this strategy
//! then takes no trades at all** — not a trade at a fabricated zero. A run
//! that produced no trades because no companion was installed is not a run
//! that found no signals, and the binary's `companion:` receipt line is what
//! tells them apart.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct CompanionUnconfirmed;

/// Series slots, in the order [`Strategy::series`] declares them.
const S_ATR: usize = 0;
const S_CHANGE: usize = 1;
const S_CATR: usize = 2;

impl Strategy for CompanionUnconfirmed {
    fn id(&self) -> &'static str {
        "companion-unconfirmed"
    }
    fn name(&self) -> &'static str {
        "Companion-unconfirmed move"
    }
    fn description(&self) -> &'static str {
        "Fade a move the companion instrument did not share. Reads two series; needs `search --companion=<SYMBOL>` installed, and takes no trades without one."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            // 2 bars = 30 minutes on a 15m feed.
            ("moveBars", 2.0),
            ("moveAtr", 1.5),
            // ZERO, not a fitted level: the boundary the mechanism names is
            // "the companion did not go along", and zero is where going along
            // stops. The comparison is strict, so at zero the companion must
            // have moved the OTHER way. A positive value admits one that moved
            // a little the same way; a negative one demands more opposition.
            ("companionAtr", 0.0),
            ("stopAtr", 4.0),
            ("atrPeriod", 14.0),
            ("companionAtrPeriod", 14.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        BTreeMap::from([
            ("moveAtr".to_string(), vec![1.0, 1.5, 2.0]),
            ("companionAtr".to_string(), vec![0.3, 0.0, -0.3]),
            ("stopAtr".to_string(), vec![2.0, 4.0, 6.0]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
            IndicatorSpec::new("cmp")
                .with("period", p.get("moveBars"))
                .with("atrPeriod", p.get("companionAtrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let cmp = format!("cmp_{}_{}", p.get("moveBars"), p.get("companionAtrPeriod"));
        vec![format!("atr_{}", p.get("atrPeriod")), format!("{cmp}.change"), format!("{cmp}.atr")]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod").max(p.period("companionAtrPeriod")) + p.period("moveBars") + 2
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let k = p.period("moveBars");
        if k == 0 || ctx.i < k {
            return Intent::None;
        }

        // Gold's own move must be measured over CONTIGUOUS bars: a change
        // straddling the daily break or a weekend is a different quantity from
        // a thirty-minute move, and the companion's leg refuses the same case
        // (`companion::aligned_change`). Equal spacing is checked against the
        // window's own first step rather than a hard-coded interval, so the
        // rule holds on any feed.
        let first = ctx.bars[ctx.i - k + 1].time - ctx.bars[ctx.i - k].time;
        if first <= 0 {
            return Intent::None;
        }
        for back in 1..k {
            if ctx.bars[ctx.i - back + 1].time - ctx.bars[ctx.i - back].time != first {
                return Intent::None;
            }
        }

        let atr = ctx.s(S_ATR);
        let change = ctx.s(S_CHANGE);
        let catr = ctx.s(S_CATR);
        // NaN here is the companion having no bar at this timestamp, or no
        // companion at all. Either way there is nothing to decide on.
        if !(atr.is_finite() && atr > 0.0 && change.is_finite() && catr.is_finite() && catr > 0.0) {
            return Intent::None;
        }

        let gold_move = ctx.bar.close - ctx.bars[ctx.i - k].close;
        let gold_atrs = gold_move / atr;
        if gold_atrs.abs() <= p.get("moveAtr") {
            return Intent::None;
        }
        let direction = if gold_move > 0.0 { 1.0 } else { -1.0 };
        // The companion's move in gold's direction, in the companion's own
        // ATRs. Below the threshold is "it did not go along".
        let companion_atrs = direction * change / catr;
        if companion_atrs >= p.get("companionAtr") {
            return Intent::None;
        }

        let stop_distance = p.get("stopAtr") * atr;
        if !(stop_distance > 0.0) {
            return Intent::None;
        }
        let (side, stop) = if direction > 0.0 {
            (Side::Short, ctx.bar.close + stop_distance)
        } else {
            (Side::Long, ctx.bar.close - stop_distance)
        };
        Intent::Enter {
            side,
            stop: Some(stop),
            // The engine's `reward_risk` places it. A strategy-supplied target
            // here would make this method's geometry differ from every other
            // method's on the same leaderboard.
            target: None,
            reason: format!(
                "gold {gold_atrs:+.2} ATR over {k} bars, companion {companion_atrs:+.2} of its own ATR"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;

    const M15: i64 = 900_000;

    fn bars(n: usize, closes: &[(usize, f64)]) -> Vec<Bar> {
        let mut out: Vec<Bar> = (0..n)
            .map(|i| Bar { time: i as i64 * M15, open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: None })
            .collect();
        for (i, c) in closes {
            out[*i].close = *c;
            out[*i].high = out[*i].high.max(*c);
            out[*i].low = out[*i].low.min(*c);
        }
        out
    }

    fn decide(bars: &[Bar], i: usize, atr: f64, change: f64, catr: f64, params: &Params) -> Intent {
        let a = vec![atr; bars.len()];
        let ch = vec![change; bars.len()];
        let ca = vec![catr; bars.len()];
        let series: [&[f64]; 3] = [&a, &ch, &ca];
        let ind = fd_indicators::IndicatorSet::new();
        let ctx = BarContext {
            bar: &bars[i],
            i,
            bars,
            ind: &ind,
            series: &series,
            options: None,
            position: None,
            params,
        };
        CompanionUnconfirmed.on_bar(&ctx)
    }

    #[test]
    fn a_gold_move_the_companion_did_not_share_is_faded() {
        let p = CompanionUnconfirmed.default_params();
        // Gold +4 over two bars on an ATR of 2 = +2.0 ATR, past moveAtr 1.5.
        let b = bars(6, &[(3, 100.0), (5, 104.0)]);
        // Companion fell while gold rose: signed move −0.5 of its own ATR,
        // below companionAtr 0.0.
        let it = decide(&b, 5, 2.0, -1.0, 2.0, &p);
        let Intent::Enter { side, stop, target, .. } = it else { panic!("expected an entry, got {it:?}") };
        assert_eq!(side, Side::Short, "fade the move");
        assert_eq!(stop, Some(104.0 + 8.0), "stopAtr 4 x ATR 2 above the signal close");
        assert_eq!(target, None, "the engine places the target");
    }

    #[test]
    fn a_move_the_companion_shared_is_left_alone() {
        let p = CompanionUnconfirmed.default_params();
        let b = bars(6, &[(3, 100.0), (5, 104.0)]);
        // Companion rose 2 of its own ATRs with gold: 2.0 >= 0.0, refused.
        assert_eq!(decide(&b, 5, 2.0, 4.0, 2.0, &p), Intent::None);
    }

    #[test]
    fn a_missing_companion_bar_takes_no_trade_rather_than_treating_it_as_flat() {
        let p = CompanionUnconfirmed.default_params();
        let b = bars(6, &[(3, 100.0), (5, 104.0)]);
        // The comparison is strict, so an exactly flat companion is NOT a
        // signal at `companionAtr = 0`: the condition is "it moved the other
        // way", and a flat companion sits on the boundary. This matters only
        // for a contrived test — an exact zero change never occurs on a real
        // feed — but the boundary has to be stated somewhere.
        assert_eq!(decide(&b, 5, 2.0, 0.0, 2.0, &p), Intent::None);
        assert!(matches!(decide(&b, 5, 2.0, -0.01, 2.0, &p), Intent::Enter { .. }));
        // A companion with no bar at this timestamp is NaN, and is not flat.
        assert_eq!(decide(&b, 5, 2.0, f64::NAN, 2.0, &p), Intent::None);
        assert_eq!(decide(&b, 5, 2.0, -1.0, f64::NAN, &p), Intent::None);
    }

    #[test]
    fn a_move_straddling_a_break_is_not_a_thirty_minute_move() {
        let p = CompanionUnconfirmed.default_params();
        let mut b = bars(6, &[(3, 100.0), (5, 104.0)]);
        b[5].time = b[4].time + 4 * M15; // an hour's break before the signal bar
        assert_eq!(decide(&b, 5, 2.0, -1.0, 2.0, &p), Intent::None);
    }

    #[test]
    fn a_small_move_is_ignored_however_the_companion_behaved() {
        let p = CompanionUnconfirmed.default_params();
        let b = bars(6, &[(3, 100.0), (5, 102.0)]); // +1.0 ATR, under 1.5
        assert_eq!(decide(&b, 5, 2.0, -5.0, 2.0, &p), Intent::None);
    }

    #[test]
    fn the_declared_series_names_match_the_keys_the_specs_produce() {
        let p = CompanionUnconfirmed.default_params();
        let specs = CompanionUnconfirmed.indicators(&p);
        let bars = bars(60, &[]);
        let set = fd_indicators::compute_indicators(&bars, &specs).expect("specs resolve");
        for key in CompanionUnconfirmed.series(&p) {
            assert!(set.contains_key(&key), "series `{key}` is not among {:?}", set.keys().collect::<Vec<_>>());
        }
    }
}
