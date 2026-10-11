//! **Layer L3**: the stop width comes from a range forecast *and* the target is
//! **a price**, not `reward_risk` x the risk.
//!
//! # The one mechanical reason this file exists
//!
//! `agent/range-sizing` measured layer **L2** — [`Exits::Engine`] with the
//! engine deriving the target as `reward_risk x risk` — and closed it, but it
//! closed it for a reason that names its own escape
//! (`docs/decisions/2026-10-11-range-sizing-result.md` section 4,
//! `--exit-mix`, W1 guards-off, `stopAtr` 2.0):
//!
//! ```text
//! atr/f2.0 : STOP 3615  TARGET 1530  TIMEOUT 2613
//! fc/f2.0  : STOP 3616  TARGET 1432  TIMEOUT 2686
//! ```
//!
//! The stop count did not move (3615 -> 3616). What moved was **98 trades out
//! of TARGET and 73 into TIMEOUT**, because at L2 a wider stop widens the
//! target by the same ratio and the further target has to be reached inside
//! `max_hold_ms = 4 h = 16 bars`. So the hypothesis "a wider stop on a bar
//! that is about to be wide gets swept less often" was never refuted — it was
//! **not paid**, because the saving at the stop was spent at the target.
//!
//! L3 cuts exactly that link: `targetSource = 1` writes a **price** into
//! [`Intent::Enter`], so `open_position` (`fd-backtest/src/engine.rs:735-744`)
//! takes it as given and the target **does not move with the stop**.
//!
//! # What makes the two target arms one comparison
//!
//! Both arms are this one method, and **both refuse a bar unless all four
//! series are finite** — including the companion slot the L2 arm does not read
//! the value of. Without that the two arms would trade different bars and the
//! difference between them would be partly a difference of samples; that is
//! the defect `agent/range-sizing` caught in its own first pass (its `atr/*`
//! labels printed n = 10,888 against 10,430 across two companion files, 4.4%
//! apart, because each file defined a different bar set).
//!
//! # Where the numbers come from
//!
//! One companion file carries **two numbers per bar**, because only one
//! companion can be installed per process (`fd_indicators::companion::install`):
//!
//! ```text
//! cmp.close = m(i)  the stop multiplier, mean-size-matched inside the file
//! cmp.high  = d(i)  the TARGET DISTANCE in quote units, > 0
//! ```
//!
//! and the stop and target are
//!
//! ```text
//! stop(i)   = close(i) -+ stopAtr x ATR(atrPeriod)(i) x m(i)
//! target(i) = close(i) +- d(i)                        (targetSource = 1)
//! target(i) = None -> engine derives reward_risk x risk (targetSource = 0)
//! ```
//!
//! `d(i)` is built in `py/research/l3_target_precheck.py` as the distance from
//! `close(i)` to the nearest **time-at-price** profile level (POC / VAH / VAL
//! of `fd_engine::price_levels::activity_profile`) beyond it, and the same
//! script also writes a time-shuffled and a constant-distance version of that
//! one column — which is why the three target sources differ in **one series
//! and nothing else**. The method cannot tell them apart and does not try to.
//!
//! **It is a TIME profile, not a volume profile.** `price_levels.rs:300`
//! declares `measure = "TIME_AT_PRICE"`, and `XAUDUKA-1m.volume` is null on
//! all 5,635,777 rows while the 15m file carries a manufactured zero
//! (addendum 9 section III), so no volume column is read anywhere in this axis.
//!
//! # Causality
//!
//! Nothing after bar `i` is read. The channel windows `bars[i - period..i]`
//! (the current bar excluded), the ATR is warm as of `i`, the profile window is
//! the 480 bars **before** `i`, and the companion bar stamped `t(i)` closes at
//! `t(i) + 15m` — the instant bar `i` closes, which is when the engine carries
//! the decision to the next bar's open. Whether the numbers in the companion
//! file were themselves built causally is a property of that file, measured by
//! that script's `--selftest`: five cut points, bit-equality, and **four**
//! deliberately look-ahead probes that must fail the same test.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct RangeStopL3;

/// Series slots, in the order [`Strategy::series`] declares them.
const S_UPPER: usize = 0;
const S_LOWER: usize = 1;
const S_ATR: usize = 2;
/// `cmp.close` — the stop multiplier `m(i)`.
const S_M: usize = 3;
/// `cmp.high` — the target distance `d(i)`, in quote units.
const S_D: usize = 4;

impl Strategy for RangeStopL3 {
    fn id(&self) -> &'static str {
        "range-stop-l3"
    }
    fn name(&self) -> &'static str {
        "Breakout, forecast stop width, price-level target"
    }
    fn description(&self) -> &'static str {
        "Prior-channel breakout with no exit rule of its own; stop = stopAtr x ATR x the companion's multiplier, target = a PRICE from the companion's distance column (targetSource 1) or the engine's reward_risk x risk (targetSource 0). Needs `search --companion=<SYMBOL>` and takes no trades without one."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            // The registry's own Donchian default, not a fitted level; the same
            // entry rule `range-stop` used, so L2 and L3 are the same method.
            ("period", 20.0),
            ("atrPeriod", 14.0),
            ("stopAtr", 1.0),
            // 0 = flat ATR stop (control), 1 = the companion's forecast.
            ("stopSource", 0.0),
            // 0 = L2 (engine derives reward_risk x risk), 1 = L3 (a price).
            ("targetSource", 0.0),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        // Empty on purpose: every cell this method is run in is declared by
        // label in docs/decisions/2026-10-11-l3-target.md, which counted 64
        // before this file existed. A grid here would let a sweep spend cells
        // registration did not count.
        BTreeMap::new()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("donchian").with("period", p.get("period")),
            IndicatorSpec::new("atr").with("period", p.get("atrPeriod")),
            // `period` is the companion's own lookback for `.change`, which
            // this method does not read; 1 is the cheapest legal value. On a
            // packed research companion `.change` and `.atr` are meaningless
            // (see the module docs) and neither is in `series` below.
            IndicatorSpec::new("cmp").with("period", 1.0).with("atrPeriod", p.get("atrPeriod")),
        ]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let d = format!("donchian_{}", p.get("period"));
        let cmp = format!("cmp_{}_{}", 1.0, p.get("atrPeriod"));
        vec![
            format!("{d}.upper"),
            format!("{d}.lower"),
            format!("atr_{}.atr", p.get("atrPeriod")),
            format!("{cmp}.close"),
            format!("{cmp}.high"),
        ]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("period").max(p.period("atrPeriod")) + 5
    }
    fn exits(&self) -> Exits {
        // THE declaration the whole axis rests on. With `Exits::Strategy`,
        // `check_exit` (engine.rs:772-777) returns None before it looks at
        // anything, so neither the stop nor the target would ever be enforced
        // and both would be units rather than orders.
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        // No exit rule of its own: while a position is open the engine owns it
        // entirely, so the stop, the target and the clock are the only things
        // that can end a trade.
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let (upper, lower, atr) = (ctx.s(S_UPPER), ctx.s(S_LOWER), ctx.s(S_ATR));
        let (m_raw, d_raw) = (ctx.s(S_M), ctx.s(S_D));

        // BOTH companion slots are required by BOTH target arms and BOTH stop
        // arms. The arms that do not read a value do read its EXISTENCE, so
        // every label trades exactly the same bars; see the module docs.
        if ![upper, lower, atr, m_raw, d_raw].iter().all(|v| v.is_finite()) {
            return Intent::None;
        }
        if !(atr > 0.0) || !(m_raw > 0.0) || !(d_raw > 0.0) {
            return Intent::None;
        }

        let m = if p.get("stopSource") >= 0.5 { m_raw } else { 1.0 };
        let stop_distance = p.get("stopAtr") * atr * m;
        if !(stop_distance > 0.0) {
            return Intent::None;
        }
        let l3 = p.get("targetSource") >= 0.5;

        let close = ctx.bar.close;
        if close > upper {
            return Intent::Enter {
                side: Side::Long,
                stop: Some(close - stop_distance),
                // `None` is NOT "no target": `open_position` then derives
                // `reward_risk` x the risk, which is the L2 shape this axis is
                // measured against.
                target: if l3 { Some(close + d_raw) } else { None },
                reason: format!("close above the prior {}-bar high", p.period("period")),
            };
        }
        if close < lower {
            return Intent::Enter {
                side: Side::Short,
                stop: Some(close + stop_distance),
                target: if l3 { Some(close - d_raw) } else { None },
                reason: format!("close below the prior {}-bar low", p.period("period")),
            };
        }
        Intent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::OpenPosition;
    use fd_core::types::Bar;

    fn params(stop_source: f64, target_source: f64, stop_atr: f64) -> Params {
        Params::new(&[
            ("period", 2.0),
            ("atrPeriod", 14.0),
            ("stopAtr", stop_atr),
            ("stopSource", stop_source),
            ("targetSource", target_source),
        ])
    }

    fn bar(close: f64) -> Bar {
        Bar { time: 0, open: close, high: close, low: close, close, volume: None }
    }

    /// Series in the declared order: `upper`, `lower`, `atr`, `cmp.close`,
    /// `cmp.high`.
    fn intent(bars: &[Bar], series: &[&[f64]], p: &Params, position: Option<OpenPosition>) -> Intent {
        let ind = fd_indicators::IndicatorSet::new();
        let i = bars.len() - 1;
        let ctx = BarContext { bar: &bars[i], i, bars, ind: &ind, series, options: None, position, params: p };
        RangeStopL3.on_bar(&ctx)
    }

    #[test]
    fn the_engine_owns_the_exit_and_the_registry_says_so_in_the_type() {
        // If this ever reads `Exits::Strategy`, every number this axis produces
        // is a number about two denominators and not about a stop and a target.
        assert_eq!(RangeStopL3.exits(), Exits::Engine);
    }

    #[test]
    fn l3_writes_a_price_and_l2_writes_none_on_the_same_bar() {
        // The whole axis in one assertion: same bars, same stop, and the only
        // difference is whether the target is a price or is left for the engine
        // to derive from `reward_risk`.
        let bars = [bar(100.0), bar(100.0), bar(110.0)];
        let (upper, lower, atr) = ([f64::NAN, f64::NAN, 105.0], [f64::NAN, f64::NAN, 95.0], [2.0, 2.0, 2.0]);
        let (m, d) = ([f64::NAN, f64::NAN, 1.5], [f64::NAN, f64::NAN, 3.25]);
        let series: [&[f64]; 5] = [&upper, &lower, &atr, &m, &d];

        let l2 = intent(&bars, &series, &params(1.0, 0.0, 1.0), None);
        let l3 = intent(&bars, &series, &params(1.0, 1.0, 1.0), None);
        match (l2, l3) {
            (
                Intent::Enter { stop: Some(s2), target: t2, .. },
                Intent::Enter { stop: Some(s3), target: Some(t3), .. },
            ) => {
                assert!((s2 - 107.0).abs() < 1e-9, "1.0 x ATR 2.0 x m 1.5 below 110: {s2}");
                assert!((s3 - s2).abs() < 1e-12, "the two target arms must have the SAME stop: {s3} vs {s2}");
                assert!(t2.is_none(), "L2 leaves the target to the engine");
                assert!((t3 - 113.25).abs() < 1e-9, "L3 target is close + d = 110 + 3.25: {t3}");
            }
            other => panic!("expected two long entries, got {other:?}"),
        }
    }

    #[test]
    fn the_target_distance_does_not_move_when_the_stop_does() {
        // This is the property L2 does not have and the reason the layer
        // exists: at L2 the target is `reward_risk` x the risk, so widening
        // the stop widens the target; here it cannot.
        let bars = [bar(100.0), bar(100.0), bar(110.0)];
        let (upper, lower, atr) = ([f64::NAN, f64::NAN, 105.0], [f64::NAN, f64::NAN, 95.0], [2.0, 2.0, 2.0]);
        let (m, d) = ([f64::NAN, f64::NAN, 1.5], [f64::NAN, f64::NAN, 3.25]);
        let series: [&[f64]; 5] = [&upper, &lower, &atr, &m, &d];
        let mut targets = Vec::new();
        let mut stops = Vec::new();
        for f in [1.0, 2.0, 4.0] {
            let Intent::Enter { stop: Some(s), target: Some(t), .. } = intent(&bars, &series, &params(1.0, 1.0, f), None)
            else {
                panic!("no entry at stopAtr {f}");
            };
            stops.push(s);
            targets.push(t);
        }
        assert!(stops[0] > stops[1] && stops[1] > stops[2], "three different stops: {stops:?}");
        assert!(
            targets.iter().all(|t| (t - targets[0]).abs() < 1e-12),
            "the target is a PRICE and must not follow the stop: {targets:?}"
        );
    }

    #[test]
    fn a_missing_companion_slot_refuses_the_trade_in_every_arm() {
        // The property that makes four labels one comparison: no label trades a
        // bar the companion does not carry BOTH numbers for - not even the ones
        // that never read the value. See the module docs for the 4.4% sample
        // split this prevents.
        let bars = [bar(100.0), bar(100.0), bar(110.0)];
        let (upper, lower, atr) = ([f64::NAN, f64::NAN, 105.0], [f64::NAN, f64::NAN, 95.0], [1.0, 1.0, 1.0]);
        let good = [f64::NAN, f64::NAN, 1.5];
        let missing = [f64::NAN, f64::NAN, f64::NAN];
        for (label, m, d) in [("m missing", &missing, &good), ("d missing", &good, &missing)] {
            for stop_source in [0.0, 1.0] {
                for target_source in [0.0, 1.0] {
                    let series: [&[f64]; 5] = [&upper, &lower, &atr, m, d];
                    assert!(
                        matches!(intent(&bars, &series, &params(stop_source, target_source, 1.0), None), Intent::None),
                        "{label}: stopSource {stop_source} / targetSource {target_source} traded it anyway"
                    );
                }
            }
        }
    }

    #[test]
    fn a_non_positive_target_distance_is_refused_rather_than_flipped() {
        // A zero or negative distance would put a long's target at or below its
        // entry, which `check_exit` would fill at the bar's open as a "target"
        // - a win printed on a trade that never went anywhere. Refuse instead.
        let bars = [bar(100.0), bar(100.0), bar(110.0)];
        let (upper, lower, atr) = ([f64::NAN, f64::NAN, 105.0], [f64::NAN, f64::NAN, 95.0], [1.0, 1.0, 1.0]);
        let m = [f64::NAN, f64::NAN, 1.0];
        for bad in [0.0, -1.0] {
            let d = [f64::NAN, f64::NAN, bad];
            let series: [&[f64]; 5] = [&upper, &lower, &atr, &m, &d];
            assert!(
                matches!(intent(&bars, &series, &params(1.0, 1.0, 1.0), None), Intent::None),
                "target distance {bad} was traded"
            );
        }
    }

    #[test]
    fn it_proposes_nothing_while_a_position_is_open() {
        let bars = [bar(100.0), bar(100.0), bar(80.0)];
        let (upper, lower) = ([f64::NAN, f64::NAN, 105.0], [f64::NAN, f64::NAN, 95.0]);
        let (atr, m, d) = ([1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 1.0]);
        let series: [&[f64]; 5] = [&upper, &lower, &atr, &m, &d];
        let p = params(1.0, 1.0, 1.0);
        let held = OpenPosition { side: Side::Long, entry_price: 100.0, entry_time: 0, stop: None, target: None };
        assert!(matches!(intent(&bars, &series, &p, Some(held)), Intent::None));
        assert!(matches!(intent(&bars, &series, &p, None), Intent::Enter { .. }));
    }
}
