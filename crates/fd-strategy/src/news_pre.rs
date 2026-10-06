//! Entering **before** a scheduled release — the branch the engine could
//! spell from 2026-10-06 and nobody measured.
//!
//! [`crate::news_pulse::NewsPulse`] anchors on a release and trades what comes
//! after it. That is the half of a scheduled event's life cycle where the
//! information is already public, and the only question is whether the move
//! persists. This file takes the other half, which is mechanically a different
//! claim: before a print **no information has arrived**, so the only things
//! that can carry an edge are *positioning* and *liquidity*.
//!
//! Measured on the stored bars before the rule was declared
//! (`docs/decisions/2026-10-07-news-pre.md` §0, reproducing n1 §2 to three
//! decimals): the median 15m range runs 1.06× the day's median at −60 minutes,
//! **1.33×** at −30, **1.78×** at −15 and **5.96×** on the release bar itself.
//! So the pre-release ramp is real. Two things measured here say what it is
//! made of, and they constrain the rules below:
//!
//! * The median **displacement** over the last two or four bars before a
//!   release is only **0.51–0.74 × ATR(14)**. The range expands while the net
//!   move does not: variance without drift, visible before the print.
//! * The −30..0 window is **1.56×** the same-day median two-bar span and
//!   **1.16×** the same-clock median on non-event days. There is no calm
//!   before this storm, so `mode = 1` below is a *relative* quiet and is
//!   documented as such rather than called a squeeze.
//!
//! # The three branches
//!
//! * `mode = 0`, `holdThrough = 0` — **pre-news breakout.** Enter with the
//!   drift of the last `preBars` bars, be **flat at the release**.
//! * `mode = 1`, `holdThrough = 0` — **compression break.** Enter whichever way
//!   price leaves a relatively quiet band, be **flat at the release**.
//! * `mode = 0`, `holdThrough = 1` — **the control.** The same entry bar, side,
//!   stop and sizing as the breakout branch, held deliberately **into** the
//!   5.96× bar. It differs from branch 1 in *nothing* but that, which is what
//!   licenses the reading: if the control passes while the other two fail, what
//!   was measured is **exposure to the event**, not a way of entering before it.
//!
//! # Being flat at the release
//!
//! [`news::next_event_at_or_after`] is the piece this needed and the reason it
//! had to be written: a bar inside a window cannot ask how far the deadline is,
//! because "am I inside?" reads the same on every bar of the window. With a
//! position open and `holdThrough = 0`, the rule emits [`Intent::Exit`] on the
//! last bar whose open is strictly before the next qualifying release, so the
//! engine's next-open fill lands on **the release bar's open**. The engine
//! consumes a pending intent (step 1) before it tests the bar's range against
//! stop and target (step 2), so the release bar's 5.96× range never touches the
//! position.
//!
//! Only the next bar's **timestamp** is read, never its prices — and that is
//! the same clock the engine itself uses to place a fill.
//!
//! This works inside a `newsonly:` gate only because
//! [`crate::filter::Filtered::on_bar`] gates [`Intent::Enter`] and passes every
//! other intent through. The release bar is outside `newsonly:-30/-15`, so a
//! filter that gated exits too would trap the position in exactly the bar the
//! branch exists to avoid.
//!
//! # Currency scope
//!
//! A [`Strategy`] cannot see `TradingRules`, so the release this file leaves
//! before is selected on **impact alone**, while the row's
//! [`crate::filter::Filter::NewsOnly`] gate is currency-scoped. Counted on
//! `data/news/events.parquet`: **2 of 562** USD impact-3 releases have a
//! non-USD impact-3 event inside the 30 or 45 minutes before them, and **0 of
//! 737** impact-3 events share a timestamp with another. So the two disagree on
//! 0.4% of events, and when they do the mismatch can only flatten the position
//! *earlier* — never hold it into a release. That is the conservative
//! direction, which is why it is left as it is.
//!
//! Spec and pre-registration: `docs/decisions/2026-10-07-news-pre.md`.

use fd_indicators::IndicatorSpec;

use crate::news;
use crate::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

pub struct NewsPre;

/// `mode` values, so a batch file's number has a name in one place.
const MODE_PRE_MOVE: f64 = 0.0;

impl Strategy for NewsPre {
    fn id(&self) -> &'static str {
        "news-pre"
    }
    fn name(&self) -> &'static str {
        "Pre-release positioning"
    }
    fn description(&self) -> &'static str {
        "Enters in the half hour BEFORE a scheduled release -- with the pre-release drift (mode 0) or on a break \
         out of a relatively quiet band (mode 1) -- and is flat at the release unless holdThrough = 1, which is \
         the control that takes the release bar on purpose."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("preBars", 4.0),
            ("mode", MODE_PRE_MOVE),
            ("minMoveAtr", 0.0),
            ("quietBars", 4.0),
            ("maxSqueeze", 0.46),
            ("stopImpulse", 1.0),
            ("holdThrough", 0.0),
            ("minImpact", 3.0),
            ("atrPeriod", 14.0),
        ])
    }
    // `grid()` is deliberately left empty (the trait's default), as in
    // `news_pulse`: every row of the registered batch is a separately declared
    // rule, and a grid here would turn 12 declarations into a search.
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![format!("atr_{}", format_period(p.get("atrPeriod")))]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") + p.period("preBars").max(p.period("quietBars")) + 5
    }
    fn exits(&self) -> Exits {
        // The engine's stop, 1.8R target and 4-hour cap, so branch 1 and
        // branch 3 are identical in every respect but the exit this file
        // issues, and so R means the same thing in both.
        Exits::Engine
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const ATR: usize = 0;
        let p = ctx.params;
        let min_impact = p.get("minImpact");
        let min_impact = if min_impact.is_finite() && min_impact >= 1.0 { min_impact.round() as u8 } else { 3 };

        // With a position open there is exactly one decision: leave before the
        // print, or ride it. Nothing else may re-enter, which is what makes
        // "one entry per release" true by construction.
        if ctx.position.is_some() {
            if p.get("holdThrough") != 0.0 {
                return Intent::None;
            }
            return match self.flat_before_release(ctx, min_impact) {
                true => Intent::Exit { reason: "flat into the release".into() },
                false => Intent::None,
            };
        }

        if p.get("mode") == MODE_PRE_MOVE {
            self.pre_move_entry(ctx, ATR)
        } else {
            self.compression_entry(ctx, ATR)
        }
    }
}

impl NewsPre {
    /// True when the next qualifying release lands at or before the open of the
    /// bar after this one, so an exit issued now fills at the release bar's
    /// open.
    ///
    /// Reads `bars[i + 1].time` and no other field of it. A missing next bar is
    /// the end of the series, where the engine's own `END_OF_DATA` closes the
    /// book, so there is nothing to do and nothing to pretend.
    fn flat_before_release(&self, ctx: &BarContext, min_impact: u8) -> bool {
        let Some(next) = ctx.bars.get(ctx.i + 1) else {
            return false;
        };
        // Strictly after this bar's open: on the release bar itself the two
        // calendar readers agree, and asking `+ 1` ms keeps a position that
        // somehow opened ON a release from reading that same release as its
        // own deadline forever.
        match news::next_event_at_or_after(ctx.bar.time + 1, min_impact, None) {
            Some(event) => next.time >= event,
            None => false,
        }
    }

    /// `mode = 0`: the direction of the drift over the `preBars` bars ending at
    /// this one, gated on that drift being worth at least `minMoveAtr` of the
    /// ATR read **before** the span — reading it inside the span would measure
    /// the pre-release expansion with a ruler the expansion had already
    /// stretched.
    fn pre_move_entry(&self, ctx: &BarContext, atr_slot: usize) -> Intent {
        let p = ctx.params;
        let pre = p.period("preBars").max(1);
        // `start` is the first bar of the span; one more bar back carries the
        // ATR, so a bar too early in the series simply has no reading.
        let Some(start) = ctx.i.checked_sub(pre - 1).filter(|s| *s >= 1) else {
            return Intent::None;
        };
        let Some(first) = ctx.bars.get(start) else {
            return Intent::None;
        };
        let atr = ctx.s_back(atr_slot, pre);
        if !atr.is_finite() || atr <= 0.0 {
            return Intent::None;
        }

        let bar = ctx.bar;
        let move_ = bar.close - first.open;
        if !(move_.abs() >= p.get("minMoveAtr") * atr) {
            return Intent::None;
        }
        // A dead-flat drift has no direction to position with.
        if move_ == 0.0 {
            return Intent::None;
        }

        // The drift's own range is the invalidation distance: a failed
        // pre-positioning is a return back through the move that suggested it.
        let window = &ctx.bars[start..=ctx.i];
        let range = span_of(window);
        let distance = range * p.get("stopImpulse");
        if !(distance > 0.0) {
            return Intent::None;
        }

        let side = if move_ > 0.0 { Side::Long } else { Side::Short };
        let stop = if side.is_long() { bar.close - distance } else { bar.close + distance };
        Intent::Enter {
            side,
            stop: Some(stop),
            target: None,
            reason: format!(
                "pre-release drift {:+.2} ({:.2} ATR) over {pre} bar(s) before a release",
                move_,
                move_.abs() / atr
            ),
        }
    }

    /// `mode = 1`: a relatively quiet band over the `quietBars` bars **before**
    /// this one, which this bar then closes outside of.
    ///
    /// `maxSqueeze` is read against `quietBars × ATR`, not against 1.0: a span
    /// over `n` bars grows about `sqrt(n)` rather than `n`, so the measured
    /// distribution of this ratio sits well below 1 for every `n` and the
    /// threshold is calibrated from that distribution
    /// (`quietBars = 2`: p25 0.55, p50 0.69; `quietBars = 4`: p25 0.35,
    /// p50 0.46 over 504 releases). At the p50 this selects **the quieter half
    /// of an already-expanded window** and is not a volatility contraction;
    /// the registration says so, and the strict p25 spellings were dropped
    /// before any run because they cap the book at 23–30 trades against a
    /// 40-trade floor.
    fn compression_entry(&self, ctx: &BarContext, atr_slot: usize) -> Intent {
        let p = ctx.params;
        let quiet = p.period("quietBars").max(1);
        // The band is the `quiet` bars ENDING AT THE BAR BEFORE this one, so
        // the bar that breaks out is not itself part of what it broke out of.
        let Some(start) = ctx.i.checked_sub(quiet).filter(|s| *s >= 1) else {
            return Intent::None;
        };
        let band = &ctx.bars[start..ctx.i];
        if band.is_empty() {
            return Intent::None;
        }
        // ATR from before the band, for the same reason the pre-move branch
        // reads it from before its span.
        let atr = ctx.s_back(atr_slot, quiet + 1);
        if !atr.is_finite() || atr <= 0.0 {
            return Intent::None;
        }

        let height = span_of(band);
        if !(height > 0.0) {
            return Intent::None;
        }
        let squeeze = height / (quiet as f64 * atr);
        if !(squeeze < p.get("maxSqueeze")) {
            return Intent::None;
        }

        let high = band.iter().fold(f64::MIN, |m, b| m.max(b.high));
        let low = band.iter().fold(f64::MAX, |m, b| m.min(b.low));
        let bar = ctx.bar;
        let side = if bar.close > high {
            Side::Long
        } else if bar.close < low {
            Side::Short
        } else {
            return Intent::None;
        };

        // The band's own height is the invalidation: a failed break is a
        // return into the band it left.
        let distance = height * p.get("stopImpulse");
        if !(distance > 0.0) {
            return Intent::None;
        }
        let stop = if side.is_long() { bar.close - distance } else { bar.close + distance };
        Intent::Enter {
            side,
            stop: Some(stop),
            target: None,
            reason: format!(
                "break {} a {quiet}-bar band {:.2} of {quiet}xATR before a release",
                if side.is_long() { "above" } else { "below" },
                squeeze
            ),
        }
    }
}

/// `max high − min low` over a slice of bars: the distance a return through the
/// move would have to cover, which is what both branches use as their stop.
fn span_of(bars: &[fd_core::types::Bar]) -> f64 {
    let high = bars.iter().fold(f64::MIN, |m, b| m.max(b.high));
    let low = bars.iter().fold(f64::MAX, |m, b| m.min(b.low));
    high - low
}

/// The indicator crate formats a whole-number period without a decimal point;
/// the series key has to match exactly or the lookup silently misses.
fn format_period(value: f64) -> String {
    if value.fract() == 0.0 { format!("{}", value as i64) } else { format!("{value}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{OpenPosition, Params};
    use fd_core::types::Bar;

    const BAR: i64 = 15 * 60_000;

    /// The shared process calendar's high-impact USD release
    /// (`news::test_events`): 2026-09-14 12:30 UTC.
    fn release_time() -> i64 {
        news::install(news::test_events()).expect("shared install");
        news::test_events()[1].time
    }

    fn bar(time: i64, open: f64, high: f64, low: f64, close: f64) -> Bar {
        Bar { time, open, high, low, close, volume: Some(0.0) }
    }

    fn params(pairs: &[(&str, f64)]) -> Params {
        let mut p = NewsPre.default_params();
        for (k, v) in pairs {
            p.set(k, *v);
        }
        p
    }

    /// Eight bars ending on the bar at offset −15 from the release, so index 6
    /// opens at −30 and index 7 opens at −15.
    fn drifting_bars(release: i64) -> Vec<Bar> {
        let start = release - 8 * BAR;
        (0..8)
            .map(|k| {
                let base = 100.0 + k as f64;
                bar(start + k * BAR, base, base + 0.5, base - 0.5, base + 0.4)
            })
            .collect()
    }

    fn intent_at(bars: &[Bar], i: usize, atr: &[f64], p: &Params, position: Option<OpenPosition>) -> Intent {
        let ind = fd_indicators::IndicatorSet::default();
        let series: Vec<&[f64]> = vec![atr];
        let ctx = BarContext {
            bar: &bars[i],
            i,
            bars,
            ind: &ind,
            series: &series,
            options: None,
            position,
            params: p,
        };
        NewsPre.on_bar(&ctx)
    }

    #[test]
    fn the_pre_move_branch_takes_the_side_of_the_drift() {
        let release = release_time();
        let bars = drifting_bars(release);
        let atr = vec![1.0; bars.len()];
        let p = params(&[("mode", 0.0), ("preBars", 4.0), ("minMoveAtr", 0.0)]);
        // Price rises through the window, so the drift is long.
        match intent_at(&bars, 7, &atr, &p, None) {
            Intent::Enter { side, stop, .. } => {
                assert_eq!(side, Side::Long);
                assert!(stop.expect("structural stop") < bars[7].close);
            }
            other => panic!("expected an entry, got {other:?}"),
        }
        // Falling bars flip it, on the same geometry.
        let falling: Vec<Bar> = bars
            .iter()
            .enumerate()
            .map(|(k, b)| {
                let base = 100.0 - k as f64;
                bar(b.time, base, base + 0.5, base - 0.5, base - 0.4)
            })
            .collect();
        match intent_at(&falling, 7, &atr, &p, None) {
            Intent::Enter { side, stop, .. } => {
                assert_eq!(side, Side::Short);
                assert!(stop.expect("structural stop") > falling[7].close);
            }
            other => panic!("expected an entry, got {other:?}"),
        }
    }

    #[test]
    fn the_pre_move_gate_refuses_a_drift_smaller_than_the_atr_multiple() {
        let release = release_time();
        let bars = drifting_bars(release);
        // The drift over four bars is about 3.4 price units against ATR 1.0,
        // so 0.5 admits it and 10.0 cannot.
        let atr = vec![1.0; bars.len()];
        assert!(matches!(
            intent_at(&bars, 7, &atr, &params(&[("mode", 0.0), ("preBars", 4.0), ("minMoveAtr", 0.5)]), None),
            Intent::Enter { .. }
        ));
        assert!(matches!(
            intent_at(&bars, 7, &atr, &params(&[("mode", 0.0), ("preBars", 4.0), ("minMoveAtr", 10.0)]), None),
            Intent::None
        ));
    }

    #[test]
    fn the_exit_fires_on_the_last_bar_before_the_release_and_not_earlier() {
        let release = release_time();
        // Bars at -30, -15 and the release bar itself.
        let bars = vec![
            bar(release - 2 * BAR, 100.0, 100.5, 99.5, 100.0),
            bar(release - BAR, 100.0, 100.5, 99.5, 100.0),
            bar(release, 100.0, 106.0, 94.0, 105.0),
        ];
        let atr = vec![1.0; bars.len()];
        let p = params(&[("holdThrough", 0.0)]);
        let held = Some(OpenPosition {
            side: Side::Long,
            entry_price: 100.0,
            entry_time: bars[0].time,
            stop: Some(99.0),
            target: None,
        });
        // At -30 the next bar opens at -15, still before the print: hold.
        assert!(matches!(intent_at(&bars, 0, &atr, &p, held), Intent::None));
        // At -15 the next bar IS the release bar, so leave now and let the
        // engine fill at its open.
        assert!(matches!(intent_at(&bars, 1, &atr, &p, held), Intent::Exit { .. }));
    }

    #[test]
    fn hold_through_never_issues_that_exit() {
        let release = release_time();
        let bars = vec![
            bar(release - 2 * BAR, 100.0, 100.5, 99.5, 100.0),
            bar(release - BAR, 100.0, 100.5, 99.5, 100.0),
            bar(release, 100.0, 106.0, 94.0, 105.0),
        ];
        let atr = vec![1.0; bars.len()];
        let p = params(&[("holdThrough", 1.0)]);
        let held = Some(OpenPosition {
            side: Side::Long,
            entry_price: 100.0,
            entry_time: bars[0].time,
            stop: Some(99.0),
            target: None,
        });
        // Same bar, same calendar, opposite decision: this is the only
        // difference between branch 1 and the control.
        assert!(matches!(intent_at(&bars, 1, &atr, &p, held), Intent::None));
    }

    #[test]
    fn the_compression_branch_needs_a_quiet_band_and_a_close_outside_it() {
        let release = release_time();
        let start = release - 8 * BAR;
        // Five flat bars, then a bar that closes above all of them.
        let mut bars: Vec<Bar> = (0..7).map(|k| bar(start + k * BAR, 100.0, 100.1, 99.9, 100.0)).collect();
        bars.push(bar(start + 7 * BAR, 100.0, 101.0, 99.95, 100.9));
        let atr = vec![1.0; bars.len()];
        // Band height 0.2 over 4 bars against ATR 1.0 -> squeeze 0.05.
        let p = params(&[("mode", 1.0), ("quietBars", 4.0), ("maxSqueeze", 0.46)]);
        match intent_at(&bars, 7, &atr, &p, None) {
            Intent::Enter { side, .. } => assert_eq!(side, Side::Long),
            other => panic!("expected a break above, got {other:?}"),
        }
        // A band that is not quiet enough refuses, however clean the break.
        let tight = params(&[("mode", 1.0), ("quietBars", 4.0), ("maxSqueeze", 0.01)]);
        assert!(matches!(intent_at(&bars, 7, &atr, &tight, None), Intent::None));
        // And a close back INSIDE the band is not a break.
        let mut inside = bars.clone();
        inside[7] = bar(start + 7 * BAR, 100.0, 101.0, 99.9, 100.0);
        assert!(matches!(intent_at(&inside, 7, &atr, &p, None), Intent::None));
    }

    #[test]
    fn no_calendar_means_no_exit_rather_than_every_exit() {
        // Fail-closed: `next_event_at_or_after` is None without a calendar, so
        // a rule that flattens before a release must flatten never. The shared
        // calendar is installed by other tests in this process, so this checks
        // the branch through a time far past every installed event instead.
        let release = release_time();
        let far = release + 1_000 * BAR;
        let bars = vec![bar(far, 100.0, 100.5, 99.5, 100.0), bar(far + BAR, 100.0, 100.5, 99.5, 100.0)];
        let atr = vec![1.0; bars.len()];
        let held = Some(OpenPosition {
            side: Side::Long,
            entry_price: 100.0,
            entry_time: far,
            stop: Some(99.0),
            target: None,
        });
        assert!(matches!(intent_at(&bars, 0, &atr, &params(&[("holdThrough", 0.0)]), held), Intent::None));
    }
}
