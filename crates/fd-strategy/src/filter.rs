//! Filters that wrap a strategy without touching it.
//!
//! A hypothesis like "EMA cross, but only in the New York morning, and flat
//! before the CME break" is the same EMA cross with a gate in front of its
//! entries and a forced exit behind its position. Writing that as a new
//! strategy would copy the signal logic and let the two drift; writing it as a
//! wrapper keeps one signal and makes the gate the only thing that differs —
//! which is what a controlled experiment needs.
//!
//! Filters are expressed on the **New York clock** ([`fd_core::clock`]) because
//! that is the clock the gold session keeps. The same wrapper around the
//! random-entry control produces the matched null: noise that trades only in
//! the same hours, so a session effect is not mistaken for a strategy effect.

use std::collections::BTreeMap;

use fd_core::clock::new_york_local;
use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Strategy};

/// One gate. Minutes are New York minutes of day; a window whose `from` is
/// after its `to` wraps past midnight (an Asian session is 19:00 → 02:00).
#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    /// Entries only while the New York clock is inside `[from, to)`.
    Hours { from_min: u32, to_min: u32 },
    /// Entries only inside **any** of these windows — kill zones.
    Sessions(Vec<(u32, u32)>),
    /// Entries only on these New York weekdays (bit `1 << weekday`, 0 = Sunday).
    Weekdays { mask: u8 },
    /// Inside `[from, to)` an open position is closed and no entry is taken.
    /// Put it before the session close and the book is flat over the break —
    /// no swap, no gap.
    Flat { from_min: u32, to_min: u32 },
    /// Entries only while `ATR(fast) / ATR(slow)` is inside `[min, max]`.
    /// A regime gate: `min` above one asks for expansion, `max` below one for
    /// compression.
    VolRegime { fast: usize, slow: usize, min_ratio: f64, max_ratio: f64 },
}

impl Filter {
    #[must_use]
    pub const fn hours(from_hhmm: u32, to_hhmm: u32) -> Self {
        Self::Hours { from_min: hhmm(from_hhmm), to_min: hhmm(to_hhmm) }
    }

    /// Several `hhmm` windows, any of which admits an entry.
    #[must_use]
    pub fn sessions(windows: &[(u32, u32)]) -> Self {
        Self::Sessions(windows.iter().map(|(a, b)| (hhmm(*a), hhmm(*b))).collect())
    }

    #[must_use]
    pub const fn flat(from_hhmm: u32, to_hhmm: u32) -> Self {
        Self::Flat { from_min: hhmm(from_hhmm), to_min: hhmm(to_hhmm) }
    }

    /// Monday to Friday.
    #[must_use]
    pub const fn weekdays() -> Self {
        Self::Weekdays { mask: 0b011_1110 }
    }

    /// The label a report prints.
    #[must_use]
    pub fn describe(&self) -> String {
        let clock = |m: u32| format!("{:02}:{:02}", m / 60, m % 60);
        match self {
            Self::Hours { from_min, to_min } => format!("NY {}-{}", clock(*from_min), clock(*to_min)),
            Self::Sessions(windows) => {
                let parts: Vec<String> = windows.iter().map(|(a, b)| format!("{}-{}", clock(*a), clock(*b))).collect();
                format!("NY {}", parts.join("|"))
            }
            Self::Weekdays { mask } => {
                let days = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
                let on: Vec<&str> = (0..7).filter(|d| mask & (1 << d) != 0).map(|d| days[d as usize]).collect();
                on.join("")
            }
            Self::Flat { from_min, to_min } => format!("flat {}-{}", clock(*from_min), clock(*to_min)),
            Self::VolRegime { fast, slow, min_ratio, max_ratio } => {
                format!("ATR{fast}/ATR{slow} in [{min_ratio}, {max_ratio}]")
            }
        }
    }
}

const fn hhmm(v: u32) -> u32 {
    (v / 100) * 60 + v % 100
}

/// `minute` inside `[from, to)`, wrapping past midnight when `from > to`.
fn in_window(minute: u32, from: u32, to: u32) -> bool {
    if from <= to { minute >= from && minute < to } else { minute >= from || minute < to }
}

fn atr_key(period: usize) -> String {
    format!("atr_{period}")
}

/// A strategy with filters in front of its entries and a flat window behind
/// its position.
pub struct Filtered<'a> {
    pub inner: &'a dyn Strategy,
    pub filters: Vec<Filter>,
}

impl Filtered<'_> {
    /// The ATR pairs the regime filters need, after the inner strategy's own
    /// series so its slot numbers still hold.
    fn regimes(&self) -> impl Iterator<Item = (usize, usize, f64, f64)> + '_ {
        self.filters.iter().filter_map(|f| match f {
            Filter::VolRegime { fast, slow, min_ratio, max_ratio } => Some((*fast, *slow, *min_ratio, *max_ratio)),
            _ => None,
        })
    }

    #[must_use]
    pub fn describe(&self) -> String {
        self.filters.iter().map(Filter::describe).collect::<Vec<_>>().join(" + ")
    }
}

impl Strategy for Filtered<'_> {
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
        self.inner.default_params()
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        self.inner.grid()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        let mut specs = self.inner.indicators(p);
        for (fast, slow, _, _) in self.regimes() {
            specs.push(IndicatorSpec::new("atr").with("period", fast as f64));
            specs.push(IndicatorSpec::new("atr").with("period", slow as f64));
        }
        specs
    }
    fn warmup(&self, p: &Params) -> usize {
        let regime = self.regimes().map(|(_, slow, _, _)| slow + 5).max().unwrap_or(0);
        self.inner.warmup(p).max(regime)
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let mut series = self.inner.series(p);
        for (fast, slow, _, _) in self.regimes() {
            series.push(atr_key(fast));
            series.push(atr_key(slow));
        }
        series
    }
    fn needs_options(&self) -> bool {
        self.inner.needs_options()
    }
    fn exits(&self) -> Exits {
        self.inner.exits()
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        let (weekday, minute) = new_york_local(ctx.bar.time);
        let flat = self.filters.iter().any(|f| matches!(f, Filter::Flat { from_min, to_min } if in_window(minute, *from_min, *to_min)));

        // The flat window overrides the strategy: it may not hold, so it may
        // not be asked. An exit it wanted anyway is the same exit.
        if flat {
            return if ctx.position.is_some() { Intent::Exit { reason: "flat window".into() } } else { Intent::None };
        }

        let intent = self.inner.on_bar(ctx);
        if !matches!(intent, Intent::Enter { .. }) {
            return intent;
        }

        let inner_slots = self.inner.series(ctx.params).len();
        let mut regime_slot = inner_slots;
        for filter in &self.filters {
            let allowed = match filter {
                Filter::Hours { from_min, to_min } => in_window(minute, *from_min, *to_min),
                Filter::Sessions(windows) => windows.iter().any(|(a, b)| in_window(minute, *a, *b)),
                Filter::Weekdays { mask } => mask & (1 << weekday) != 0,
                Filter::Flat { .. } => true,
                Filter::VolRegime { min_ratio, max_ratio, .. } => {
                    let (fast, slow) = (ctx.s(regime_slot), ctx.s(regime_slot + 1));
                    regime_slot += 2;
                    let ratio = fast / slow;
                    // NaN fails closed: no regime reading, no trade.
                    ratio.is_finite() && ratio >= *min_ratio && ratio <= *max_ratio
                }
            };
            if !allowed {
                return Intent::None;
            }
        }
        intent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_core::clock::days_from_civil;
    use fd_core::types::Bar;
    use crate::registry::{OpenPosition, Side};

    /// Wants in on every bar, long, with a stop; never exits on its own.
    struct Always;
    impl Strategy for Always {
        fn id(&self) -> &'static str {
            "always"
        }
        fn name(&self) -> &'static str {
            "always"
        }
        fn description(&self) -> &'static str {
            ""
        }
        fn default_params(&self) -> Params {
            Params::default()
        }
        fn indicators(&self, _: &Params) -> Vec<IndicatorSpec> {
            Vec::new()
        }
        fn warmup(&self, _: &Params) -> usize {
            0
        }
        fn on_bar(&self, ctx: &BarContext) -> Intent {
            if ctx.position.is_some() {
                return Intent::None;
            }
            Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 1.0), target: None, reason: "always".into() }
        }
    }

    /// A UTC instant on 2026-09-14 (a Monday, EDT: New York = UTC - 4h).
    fn monday_utc(hour: i64, minute: i64) -> i64 {
        days_from_civil(2026, 9, 14) * 86_400_000 + hour * 3_600_000 + minute * 60_000
    }

    fn intent_at(filtered: &Filtered, time: i64, position: Option<OpenPosition>) -> Intent {
        let bar = Bar::flat(time, 100.0);
        let bars = [bar];
        let ind = fd_indicators::IndicatorSet::new();
        let params = Params::default();
        let ctx = BarContext { bar: &bars[0], i: 0, bars: &bars, ind: &ind, series: &[], options: None, position, params: &params };
        filtered.on_bar(&ctx)
    }

    #[test]
    fn hours_are_read_on_the_new_york_clock() {
        let f = Filtered { inner: &Always, filters: vec![Filter::hours(800, 1200)] };
        // 13:00 UTC in September is 09:00 New York: inside.
        assert!(matches!(intent_at(&f, monday_utc(13, 0), None), Intent::Enter { .. }));
        // 16:00 UTC is 12:00 New York: the window is half-open, so outside.
        assert!(matches!(intent_at(&f, monday_utc(16, 0), None), Intent::None));
        // 08:00 UTC is 04:00 New York: outside.
        assert!(matches!(intent_at(&f, monday_utc(8, 0), None), Intent::None));
    }

    #[test]
    fn a_window_past_midnight_wraps() {
        let f = Filtered { inner: &Always, filters: vec![Filter::hours(1900, 200)] };
        // 23:30 UTC Monday = 19:30 New York: inside.
        assert!(matches!(intent_at(&f, monday_utc(23, 30), None), Intent::Enter { .. }));
        // 05:00 UTC Tuesday = 01:00 New York Monday night: inside.
        assert!(matches!(intent_at(&f, monday_utc(24 + 5, 0), None), Intent::Enter { .. }));
        // 07:00 UTC = 03:00 New York: outside.
        assert!(matches!(intent_at(&f, monday_utc(24 + 7, 0), None), Intent::None));
    }

    #[test]
    fn the_flat_window_closes_the_book_and_refuses_entries() {
        let f = Filtered { inner: &Always, filters: vec![Filter::flat(1630, 1800)] };
        let open = OpenPosition { side: Side::Long, entry_price: 100.0, entry_time: 0, stop: None, target: None };
        // 20:45 UTC = 16:45 New York: inside the flat window.
        assert!(matches!(intent_at(&f, monday_utc(20, 45), Some(open)), Intent::Exit { .. }));
        assert!(matches!(intent_at(&f, monday_utc(20, 45), None), Intent::None));
        // 22:15 UTC = 18:15 New York: the window has passed, trading resumes.
        assert!(matches!(intent_at(&f, monday_utc(22, 15), None), Intent::Enter { .. }));
    }

    #[test]
    fn weekdays_exclude_the_weekend_on_the_new_york_clock() {
        let f = Filtered { inner: &Always, filters: vec![Filter::weekdays()] };
        assert!(matches!(intent_at(&f, monday_utc(13, 0), None), Intent::Enter { .. }));
        // Sunday 23:00 UTC is Sunday 19:00 New York — the open, but a Sunday.
        assert!(matches!(intent_at(&f, monday_utc(-1, 0), None), Intent::None));
    }

    #[test]
    fn the_regime_filter_adds_its_series_after_the_strategy_s_own() {
        let f = Filtered { inner: &Always, filters: vec![Filter::VolRegime { fast: 14, slow: 100, min_ratio: 1.2, max_ratio: 9.0 }] };
        assert_eq!(f.series(&Params::default()), vec!["atr_14".to_string(), "atr_100".to_string()]);
        assert_eq!(f.warmup(&Params::default()), 105);
        assert_eq!(f.describe(), "ATR14/ATR100 in [1.2, 9]");
    }
}
