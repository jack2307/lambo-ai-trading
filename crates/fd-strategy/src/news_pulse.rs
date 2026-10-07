//! Entering **on** the economic calendar, which no method here could do.
//!
//! Every other calendar-aware thing in this crate avoids a release:
//! [`crate::filter::Filter::News`] knows how to black out a window and nothing
//! else. So "a scheduled release expands volatility at a minute known years in
//! advance; trade that" had never been spelled, and therefore had never been
//! falsified. This is the spelling.
//!
//! Measured on the stored bars before the rule was declared
//! (`docs/decisions/2026-10-06-news-entry.md` §2): the median 15m range of the
//! release bar of a USD impact-3 release is **5.96×** the day's median 15m
//! range on `XAUDUKA` (504 events) and **7.02×** on `XAUUSD` (137 events),
//! still 2–3× an hour later, and ~1.8× in the 15 minutes before. The
//! expansion is not in doubt. Whether it has a **direction** is the whole
//! question, and this strategy asks it twice, in opposite senses:
//!
//! * `mode = 0` — **breakout**: enter with the first move after the release.
//! * `mode = 1` — **reversion**: enter against it, once it has run.
//!
//! # The rule
//!
//! On bar `i`, let `j = i − (probeBars − 1)` and let `e` be the newest event
//! of `impact ≥ minImpact` at or before `bars[j]`'s open. Bar `j` is the
//! **release bar** when it is the first bar at or after `e`
//! (`bars[j−1].time < e`) and sits no more than `maxLagMin` past it — the lag
//! test is what keeps a release that fell inside a weekend gap from anchoring
//! on the Sunday open. Then
//!
//! * **impulse** = `bars[i].close − bars[j].open`, over the release bar and
//!   the `probeBars − 1` bars after it;
//! * the **gate** is `|impulse| ≥ minMoveAtr × ATR` read at `j − 1`, the
//!   **pre-release** ATR — reading it at `j` would measure the expansion with
//!   a ruler the expansion had already stretched;
//! * the **stop** is `stopImpulse` × the impulse's own range
//!   (`max high − min low` over `bars[j..=i]`) beyond the entry. A failed
//!   break is a return back through the impulse, so that is the invalidation.
//!   An ATR(14) stop would be about a sixth of the release bar alone and every
//!   trade would be stopped out by noise it was meant to be trading.
//!
//! The fill is the next bar's open and the engine's `max_hold_ms` applies
//! ([`Exits::Engine`]): one entry per release, by construction, because
//! exactly one bar per event sits at offset `probeBars − 1`.
//!
//! # Currency scope
//!
//! A [`Strategy`] cannot see `TradingRules`, so the anchor here is selected on
//! **impact alone**. The scope comes from the row's own
//! [`crate::filter::Filter::NewsOnly`] gate, which *is* scoped
//! (`Filter::for_market`) and which admits exactly the bar at offset
//! `probeBars − 1` after a release of the market's currencies. The two agree
//! because no non-USD impact-3 event in `data/news/events.parquet` shares a
//! timestamp with a USD one — counted, 0 of 747 — so whenever the gate admits
//! a bar, the event this file anchors on is the event the gate admitted it
//! for. A row that drops the `newsonly:` filter would trade every currency's
//! releases and is not what was registered.
//!
//! Spec and pre-registration: `docs/decisions/2026-10-06-news-entry.md`.

use fd_indicators::IndicatorSpec;

use crate::news;
use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct NewsPulse;

/// `mode` values, so a batch file's number has a name in one place.
const MODE_BREAKOUT: f64 = 0.0;

impl Strategy for NewsPulse {
    fn id(&self) -> &'static str {
        "news-pulse"
    }
    fn name(&self) -> &'static str {
        "Scheduled-release pulse"
    }
    fn description(&self) -> &'static str {
        "Anchors on a scheduled release and enters with (mode 0) or against (mode 1) the first move after it, \
         stopped one impulse-range away. The only method here that enters BECAUSE of the calendar."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("probeBars", 1.0),
            ("mode", MODE_BREAKOUT),
            ("minMoveAtr", 0.5),
            ("stopImpulse", 1.0),
            ("minImpact", 3.0),
            ("atrPeriod", 14.0),
            ("maxLagMin", 15.0),
        ])
    }
    // `grid()` is deliberately left empty (the trait's default). Every row of
    // the registered batch is a separately declared rule and nothing is chosen
    // from the data: a grid here would turn 12 declared rows into a search,
    // which is the thing this desk keeps getting caught doing.
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", format_period(p.get("atrPeriod")))]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") + 5
    }
    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const ATR: usize = 0;
        if ctx.position.is_some() {
            return Intent::None;
        }
        let p = ctx.params;
        let probe = p.period("probeBars").max(1);
        // The candidate release bar, and the bar before it, which carries the
        // pre-release ATR. `probe` bars back of `i` plus one for the ATR, so a
        // bar too early in the series simply has no reading.
        let j = match ctx.i.checked_sub(probe - 1) {
            Some(j) if j >= 1 => j,
            _ => return Intent::None,
        };
        let (Some(release), Some(before)) = (ctx.bars.get(j), ctx.bars.get(j - 1)) else {
            return Intent::None;
        };
        let min_impact = p.get("minImpact");
        let min_impact = if min_impact.is_finite() && min_impact >= 1.0 { min_impact.round() as u8 } else { 3 };
        // Scope: impact only — see the module's "Currency scope".
        let Some(event) = news::last_event_at_or_before(release.time, min_impact, None) else {
            return Intent::None;
        };
        // `release` must be the FIRST bar at or after the event, and must not
        // sit a gap away from it.
        if before.time >= event {
            return Intent::None;
        }
        let lag_ms = (p.get("maxLagMin") * 60_000.0) as i64;
        if release.time - event > lag_ms {
            return Intent::None;
        }

        // The pre-release ATR: slot 0 at index `j - 1`, which is `probe` bars
        // back of `i`.
        let atr = ctx.s_back(ATR, probe);
        if !atr.is_finite() || atr <= 0.0 {
            return Intent::None;
        }

        let bar = ctx.bar;
        let impulse = bar.close - release.open;
        if !(impulse.abs() >= p.get("minMoveAtr") * atr) {
            return Intent::None;
        }
        // A dead-flat impulse has no direction to trade either way round.
        if impulse == 0.0 {
            return Intent::None;
        }

        // The impulse's own range is the invalidation distance.
        let window = &ctx.bars[j..=ctx.i];
        let high = window.iter().fold(f64::MIN, |m, b| m.max(b.high));
        let low = window.iter().fold(f64::MAX, |m, b| m.min(b.low));
        let range = high - low;
        if !(range > 0.0) {
            return Intent::None;
        }

        let with_impulse = p.get("mode") == MODE_BREAKOUT;
        let up = impulse > 0.0;
        let side = if up == with_impulse { Side::Long } else { Side::Short };
        let distance = range * p.get("stopImpulse");
        if !(distance > 0.0) {
            return Intent::None;
        }
        let stop = if side.is_long() { bar.close - distance } else { bar.close + distance };
        Intent::Enter {
            side,
            stop: Some(stop),
            target: None,
            reason: format!(
                "{} {:+.2} ({:.2} ATR) over {probe} bar(s) from a release",
                if with_impulse { "with" } else { "against" },
                impulse,
                impulse.abs() / atr
            ),
        }
    }
}

/// The indicator crate formats a whole-number period without a decimal point;
/// the series key has to match exactly or the lookup silently misses.
fn format_period(value: f64) -> String {
    if value.fract() == 0.0 { format!("{}", value as i64) } else { format!("{value}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::types::Bar;

    const BAR: i64 = 15 * 60_000;

    /// The shared process calendar's high-impact USD release
    /// (`news::test_events`): 2026-09-14 12:30 UTC.
    fn release_time() -> i64 {
        news::install(news::test_events()).expect("shared install");
        news::test_events()[1].time
    }

    /// Bars every 15 minutes starting one bar before the release, so index 1
    /// is the release bar and index 0 carries the pre-release ATR.
    fn series_of(closes: &[(f64, f64, f64, f64)]) -> Vec<Bar> {
        let start = release_time() - BAR;
        closes
            .iter()
            .enumerate()
            .map(|(k, (o, h, l, c))| Bar {
                time: start + k as i64 * BAR,
                open: *o,
                high: *h,
                low: *l,
                close: *c,
                volume: None,
            })
            .collect()
    }

    fn intent(bars: &[Bar], i: usize, atr: f64, overrides: &[(&str, f64)]) -> Intent {
        let a: Vec<f64> = vec![atr; bars.len()];
        let series: [&[f64]; 1] = [&a];
        let ind = fd_indicators::IndicatorSet::new();
        let mut params = NewsPulse.default_params();
        for (k, v) in overrides {
            params.set(k, *v);
        }
        let ctx = BarContext {
            bar: &bars[i],
            i,
            bars,
            ind: &ind,
            series: &series,
            options: None,
            position: None,
            params: &params,
        };
        NewsPulse.on_bar(&ctx)
    }

    /// A release that moves up: breakout buys it, reversion sells it, and the
    /// stop sits one impulse-range from the signal close either way.
    #[test]
    fn the_release_bar_is_bought_by_breakout_and_sold_by_reversion() {
        // index 0 pre-release, index 1 the release bar: opens 4000, runs to
        // 4012, high 4014, low 3998 — a range of 16 and an impulse of +12.
        let bars = series_of(&[(4000.0, 4001.0, 3999.0, 4000.0), (4000.0, 4014.0, 3998.0, 4012.0), (4012.0, 4013.0, 4011.0, 4012.0)]);
        let it = intent(&bars, 1, 4.0, &[("probeBars", 1.0), ("mode", 0.0)]);
        let Intent::Enter { side, stop, target, ref reason } = it else { panic!("expected an entry, got {it:?}") };
        assert_eq!(side, Side::Long, "breakout goes with the move");
        assert_eq!(stop, Some(4012.0 - 16.0), "one impulse range below the signal close");
        assert_eq!(target, None, "the engine's reward:risk");
        assert!(reason.contains("with"), "{reason}");

        let it = intent(&bars, 1, 4.0, &[("probeBars", 1.0), ("mode", 1.0)]);
        let Intent::Enter { side, stop, ref reason, .. } = it else { panic!("expected an entry, got {it:?}") };
        assert_eq!(side, Side::Short, "reversion goes against it");
        assert_eq!(stop, Some(4012.0 + 16.0));
        assert!(reason.contains("against"), "{reason}");
    }

    /// Only the bar at offset `probeBars − 1` fires, which is what makes the
    /// `newsonly:` null a timing match rather than a window match.
    #[test]
    fn exactly_one_bar_per_release_fires_and_it_is_the_probe_s_own() {
        let bars = series_of(&[
            (4000.0, 4001.0, 3999.0, 4000.0),
            (4000.0, 4014.0, 3998.0, 4012.0),
            (4012.0, 4016.0, 4010.0, 4015.0),
            (4015.0, 4018.0, 4013.0, 4016.0),
            (4016.0, 4020.0, 4014.0, 4018.0),
        ]);
        // probe 1 fires on the release bar (index 1) and nowhere else.
        assert!(matches!(intent(&bars, 1, 4.0, &[("probeBars", 1.0)]), Intent::Enter { .. }));
        for i in [2, 3, 4] {
            assert_eq!(intent(&bars, i, 4.0, &[("probeBars", 1.0)]), Intent::None, "bar {i}");
        }
        // probe 2 fires on index 2 and nowhere else.
        assert!(matches!(intent(&bars, 2, 4.0, &[("probeBars", 2.0)]), Intent::Enter { .. }));
        for i in [1, 3, 4] {
            assert_eq!(intent(&bars, i, 4.0, &[("probeBars", 2.0)]), Intent::None, "bar {i}");
        }
        // probe 4 fires on index 4 and nowhere else.
        assert!(matches!(intent(&bars, 4, 4.0, &[("probeBars", 4.0)]), Intent::Enter { .. }));
        for i in [1, 2, 3] {
            assert_eq!(intent(&bars, i, 4.0, &[("probeBars", 4.0)]), Intent::None, "bar {i}");
        }
    }

    /// The impulse over a longer probe is measured from the RELEASE bar's
    /// open, not from the signal bar's, and the stop from the whole window's
    /// range.
    #[test]
    fn a_longer_probe_measures_from_the_release_open_over_the_whole_window() {
        let bars = series_of(&[
            (4000.0, 4001.0, 3999.0, 4000.0),
            (4000.0, 4010.0, 3990.0, 4008.0), // release bar: low 3990
            (4008.0, 4020.0, 4007.0, 4018.0), // signal bar: high 4020
        ]);
        let it = intent(&bars, 2, 4.0, &[("probeBars", 2.0), ("mode", 0.0)]);
        let Intent::Enter { side, stop, .. } = it else { panic!("expected an entry, got {it:?}") };
        assert_eq!(side, Side::Long);
        // impulse = 4018 − 4000 = +18; range = 4020 − 3990 = 30.
        assert_eq!(stop, Some(4018.0 - 30.0));
    }

    #[test]
    fn a_move_smaller_than_the_gate_is_not_traded() {
        let bars = series_of(&[(4000.0, 4001.0, 3999.0, 4000.0), (4000.0, 4003.0, 3999.0, 4002.0)]);
        // impulse +2; the gate at 0.5 ATR of 4.0 is 2.0, so it just passes...
        assert!(matches!(intent(&bars, 1, 4.0, &[("minMoveAtr", 0.5)]), Intent::Enter { .. }));
        // ...and at 1.5 ATR (6.0) it does not.
        assert_eq!(intent(&bars, 1, 4.0, &[("minMoveAtr", 1.5)]), Intent::None);
        // A flat impulse has no direction either way round.
        let flat = series_of(&[(4000.0, 4001.0, 3999.0, 4000.0), (4000.0, 4009.0, 3991.0, 4000.0)]);
        assert_eq!(intent(&flat, 1, 4.0, &[("minMoveAtr", 0.0)]), Intent::None, "a round trip to the open is not a move");
    }

    /// The gate reads the PRE-release ATR. If it read the release bar's own,
    /// the expansion would be measuring itself and the gate would never bite.
    #[test]
    fn the_gate_reads_the_atr_before_the_release_and_not_after_it() {
        let bars = series_of(&[(4000.0, 4001.0, 3999.0, 4000.0), (4000.0, 4014.0, 3998.0, 4012.0)]);
        // The series handed in is flat, so this test pins which INDEX is read
        // by making the reading itself decide: at ATR 4.0 the 1.5 gate (6.0)
        // passes on an impulse of 12, at ATR 24.0 (gate 36.0) it does not.
        assert!(matches!(intent(&bars, 1, 4.0, &[("minMoveAtr", 1.5)]), Intent::Enter { .. }));
        assert_eq!(intent(&bars, 1, 24.0, &[("minMoveAtr", 1.5)]), Intent::None);
        // A non-finite or non-positive ATR refuses the trade rather than
        // trading on a NaN comparison.
        assert_eq!(intent(&bars, 1, f64::NAN, &[]), Intent::None);
        assert_eq!(intent(&bars, 1, 0.0, &[]), Intent::None);
    }

    /// A release that fell inside a gap in the bars must not anchor on the
    /// first bar after the gap — that bar is the Sunday open, hours or days
    /// later, and its "impulse" has nothing to do with the release.
    #[test]
    fn a_release_inside_a_gap_in_the_bars_does_not_anchor() {
        let release = release_time();
        let bars = vec![
            Bar { time: release - 3 * 86_400_000, open: 4000.0, high: 4001.0, low: 3999.0, close: 4000.0, volume: None },
            // The first bar after the release is two days late: a weekend.
            Bar { time: release + 2 * 86_400_000, open: 4000.0, high: 4014.0, low: 3998.0, close: 4012.0, volume: None },
        ];
        assert_eq!(intent(&bars, 1, 4.0, &[]), Intent::None, "a two-day lag is not a release bar");
        // The same geometry one bar past the release does anchor.
        let close = vec![
            Bar { time: release - BAR, open: 4000.0, high: 4001.0, low: 3999.0, close: 4000.0, volume: None },
            Bar { time: release, open: 4000.0, high: 4014.0, low: 3998.0, close: 4012.0, volume: None },
        ];
        assert!(matches!(intent(&close, 1, 4.0, &[]), Intent::Enter { .. }));
    }

    #[test]
    fn a_bar_with_no_release_behind_it_takes_nothing() {
        let release = release_time();
        // A whole day before the release: the newest qualifying event at or
        // before it is the medium EUR one, invisible at impact 3.
        let bars = vec![
            Bar { time: release - 10 * BAR, open: 4000.0, high: 4001.0, low: 3999.0, close: 4000.0, volume: None },
            Bar { time: release - 9 * BAR, open: 4000.0, high: 4014.0, low: 3998.0, close: 4012.0, volume: None },
        ];
        assert_eq!(intent(&bars, 1, 4.0, &[]), Intent::None);
        // At impact 2 the 09:00 EUR release is visible, and the bar that is
        // the first one after it anchors.
        let eur = news::test_events()[2].time;
        let on_eur = vec![
            Bar { time: eur - BAR, open: 4000.0, high: 4001.0, low: 3999.0, close: 4000.0, volume: None },
            Bar { time: eur, open: 4000.0, high: 4014.0, low: 3998.0, close: 4012.0, volume: None },
        ];
        assert_eq!(intent(&on_eur, 1, 4.0, &[("minImpact", 3.0)]), Intent::None);
        assert!(matches!(intent(&on_eur, 1, 4.0, &[("minImpact", 2.0)]), Intent::Enter { .. }));
    }

    #[test]
    fn an_open_position_is_left_alone() {
        let bars = series_of(&[(4000.0, 4001.0, 3999.0, 4000.0), (4000.0, 4014.0, 3998.0, 4012.0)]);
        let a: Vec<f64> = vec![4.0; bars.len()];
        let series: [&[f64]; 1] = [&a];
        let ind = fd_indicators::IndicatorSet::new();
        let params = NewsPulse.default_params();
        let ctx = BarContext {
            bar: &bars[1],
            i: 1,
            bars: &bars,
            ind: &ind,
            series: &series,
            options: None,
            position: Some(crate::registry::OpenPosition {
                side: Side::Long,
                entry_price: 4000.0,
                entry_time: 0,
                stop: None,
                target: None,
            }),
            params: &params,
        };
        assert_eq!(NewsPulse.on_bar(&ctx), Intent::None, "the engine owns the exit, not this");
    }
}
