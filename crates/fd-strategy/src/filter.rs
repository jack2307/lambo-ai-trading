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

use crate::news;
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
    /// Entries only while `ATR(period) / close`, in percent, is inside
    /// `[min, max]`. An **absolute** volatility gate, unlike `VolRegime`,
    /// which is relative to the recent past: a year twice as volatile as
    /// another reads as "high" here and as "normal" there.
    VolAbs { period: usize, min_pct: f64, max_pct: f64 },
    /// No entries from `before_min` minutes before to `after_min` minutes
    /// after any scheduled event of `impact >= min_impact` (3 = high) in the
    /// calendar the binary installed through [`news::install`].
    ///
    /// The check reads the **signal bar's open time**: a bar whose time is
    /// inside `[event − before, event + after)` produces no entry. The fill
    /// is the engine's, one bar later, so a bar that opens just before the
    /// window and would fill inside it is **not** blocked — pad `before_min`
    /// by one bar when the hypothesis needs the fill outside the window too.
    /// Exits are never gated: a position opened before the release is closed
    /// by its stop, target or the strategy exactly as without the filter.
    /// With no calendar installed the filter is a no-op (the receipt says
    /// `news: none loaded`).
    ///
    /// `currencies` scopes the calendar: only events of these currencies
    /// (and the calendar's global `All`) count. Empty means **every**
    /// currency. A spelling may name them (`news:60-30:3:USD|EUR`); when it
    /// does not, [`Filter::parse_for_market`] fills in the market's
    /// configured list, so a `news:60-30` on gold reads USD releases only.
    News { before_min: u32, after_min: u32, min_impact: u8, currencies: Vec<String> },
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

    /// A filter from its one-line spelling, as a batch file writes it:
    /// `weekdays`, `hours:0800-1200`, `sessions:0100-0500|0600-1000`,
    /// `flat:1630-1815`, `vol:14/100:1.2-99`, `volabs:14:0.075-9`,
    /// `news:60-30` (60 min before to 30 min after high-impact news),
    /// `news:60-30:2` (impact ≥ 2) or `news:60-30:3:USD|EUR` (those
    /// currencies only; the impact is required when currencies are given).
    /// Times are New York `hhmm`; news widths are minutes.
    ///
    /// A `news:` filter parsed here with no currency list reads **every**
    /// currency. Callers that know the market use [`Filter::parse_for_market`].
    pub fn parse(spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        let bad = |why: &str| Err(format!("filter `{spec}`: {why}"));
        let window = |text: &str| -> Result<(u32, u32), String> {
            let (a, b) = text.split_once('-').ok_or_else(|| format!("filter `{spec}`: expected hhmm-hhmm"))?;
            let parse = |t: &str| t.trim().parse::<u32>().map_err(|_| format!("filter `{spec}`: `{t}` is not hhmm"));
            let (a, b) = (parse(a)?, parse(b)?);
            if a % 100 >= 60 || b % 100 >= 60 || a / 100 > 24 || b / 100 > 24 {
                return Err(format!("filter `{spec}`: `{text}` is not a clock time"));
            }
            Ok((a, b))
        };
        match spec.split_once(':') {
            None if spec == "weekdays" => Ok(Self::weekdays()),
            None => bad("unknown filter"),
            // `weekdays:MoTuWeTh` — a chosen set of New York weekdays, so a
            // Friday leg that runs into the weekend can be left out of a row.
            Some(("weekdays", names)) => {
                const NAMES: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
                let text = names.trim();
                if text.is_empty() || text.len() % 2 != 0 {
                    return bad("expected weekday names such as MoTuWeTh");
                }
                let mut mask = 0u8;
                for i in (0..text.len()).step_by(2) {
                    let name = &text[i..i + 2];
                    let day = NAMES.iter().position(|n| n.eq_ignore_ascii_case(name)).ok_or_else(|| format!("filter `{spec}`: `{name}` is not a weekday"))?;
                    mask |= 1 << day;
                }
                Ok(Self::Weekdays { mask })
            }
            Some(("hours", w)) => window(w).map(|(a, b)| Self::hours(a, b)),
            Some(("flat", w)) => window(w).map(|(a, b)| Self::flat(a, b)),
            Some(("sessions", list)) => {
                let windows = list.split('|').map(window).collect::<Result<Vec<_>, _>>()?;
                if windows.is_empty() {
                    return bad("no windows");
                }
                Ok(Self::sessions(&windows))
            }
            Some(("volabs", rest)) => {
                let (period, range) = rest.split_once(':').ok_or_else(|| format!("filter `{spec}`: expected volabs:P:min-max"))?;
                let (lo, hi) = range.split_once('-').ok_or_else(|| format!("filter `{spec}`: expected min-max"))?;
                let num = |t: &str| t.trim().parse::<f64>().map_err(|_| format!("filter `{spec}`: `{t}` is not a number"));
                let period = period.trim().parse::<usize>().map_err(|_| format!("filter `{spec}`: `{period}` is not a period"))?;
                Ok(Self::VolAbs { period, min_pct: num(lo)?, max_pct: num(hi)? })
            }
            // `news:B-A[:I[:C1|C2]]` — B minutes before to A minutes after
            // events of impact ≥ I (default 3, high) of currencies C (default:
            // every currency, or the market's list through
            // `parse_for_market`). Both widths are required so that `news:60`
            // cannot be read as "and nothing after".
            Some(("news", rest)) => {
                let mut parts = rest.splitn(3, ':');
                let widths = parts.next().unwrap_or("");
                let impact = parts.next();
                let currencies = parts.next();
                let (b, a) = widths
                    .split_once('-')
                    .ok_or_else(|| format!("filter `{spec}`: expected news:before-after[:impact[:CCY|CCY]] in minutes"))?;
                let minutes = |t: &str| t.trim().parse::<u32>().map_err(|_| format!("filter `{spec}`: `{t}` is not a number of minutes"));
                let (before_min, after_min) = (minutes(b)?, minutes(a)?);
                let min_impact = match impact {
                    None => 3,
                    Some(i) => match i.trim().parse::<u8>() {
                        Ok(v @ 1..=3) => v,
                        _ => return bad("impact must be 1, 2 or 3 (3 = high)"),
                    },
                };
                let currencies = match currencies {
                    None => Vec::new(),
                    Some(list) => {
                        let codes: Vec<String> = list.split('|').map(|c| c.trim().to_ascii_uppercase()).filter(|c| !c.is_empty()).collect();
                        if codes.is_empty() {
                            return bad("expected currencies such as USD|EUR after the impact");
                        }
                        if let Some(odd) = codes.iter().find(|c| !c.chars().all(|ch| ch.is_ascii_alphabetic())) {
                            return Err(format!("filter `{spec}`: `{odd}` is not a currency code"));
                        }
                        codes
                    }
                };
                Ok(Self::News { before_min, after_min, min_impact, currencies })
            }
            Some(("vol", rest)) => {
                let (periods, range) = rest.split_once(':').ok_or_else(|| format!("filter `{spec}`: expected vol:F/S:min-max"))?;
                let (fast, slow) = periods.split_once('/').ok_or_else(|| format!("filter `{spec}`: expected F/S"))?;
                let (lo, hi) = range.split_once('-').ok_or_else(|| format!("filter `{spec}`: expected min-max"))?;
                let num = |t: &str| t.trim().parse::<f64>().map_err(|_| format!("filter `{spec}`: `{t}` is not a number"));
                let period = |t: &str| t.trim().parse::<usize>().map_err(|_| format!("filter `{spec}`: `{t}` is not a period"));
                Ok(Self::VolRegime { fast: period(fast)?, slow: period(slow)?, min_ratio: num(lo)?, max_ratio: num(hi)? })
            }
            Some(_) => bad("unknown filter"),
        }
    }

    /// [`Filter::parse`], then scope a `news:` filter that names no
    /// currencies to the market's configured list (`news_currencies`). A
    /// spelling that names its own currencies keeps them; every other filter
    /// is untouched. This is what `search` and the API call, so a batch
    /// file's `news:60-30` means "the market's releases" and not "everyone's".
    pub fn parse_for_market(spec: &str, news_currencies: &[String]) -> Result<Self, String> {
        Self::parse(spec).map(|f| f.for_market(news_currencies))
    }

    /// The same filter, with an unscoped `news:` gate scoped to
    /// `news_currencies`. Idempotent; a no-op on every other variant and on a
    /// `news:` filter that already names its currencies.
    #[must_use]
    pub fn for_market(self, news_currencies: &[String]) -> Self {
        match self {
            Self::News { before_min, after_min, min_impact, currencies } if currencies.is_empty() => Self::News {
                before_min,
                after_min,
                min_impact,
                currencies: news_currencies.iter().map(|c| c.to_ascii_uppercase()).collect(),
            },
            other => other,
        }
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
            Self::VolAbs { period, min_pct, max_pct } => format!("ATR{period}/close in [{min_pct}%, {max_pct}%]"),
            Self::News { before_min, after_min, min_impact, currencies } => {
                let which = match min_impact {
                    3 => "high-impact".to_string(),
                    i => format!("impact≥{i}"),
                };
                let scope = if currencies.is_empty() { String::new() } else { format!(" ({})", currencies.join("|")) };
                format!("no entries {before_min} min before to {after_min} min after {which} news{scope}")
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

    /// The single ATRs the absolute-volatility filters need, after the regime
    /// pairs.
    fn absolutes(&self) -> impl Iterator<Item = usize> + '_ {
        self.filters.iter().filter_map(|f| match f {
            Filter::VolAbs { period, .. } => Some(*period),
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
        for period in self.absolutes() {
            specs.push(IndicatorSpec::new("atr").with("period", period as f64));
        }
        specs
    }
    fn warmup(&self, p: &Params) -> usize {
        let regime = self.regimes().map(|(_, slow, _, _)| slow + 5).max().unwrap_or(0);
        let absolute = self.absolutes().map(|p| p + 5).max().unwrap_or(0);
        self.inner.warmup(p).max(regime).max(absolute)
    }
    fn series(&self, p: &Params) -> Vec<String> {
        let mut series = self.inner.series(p);
        for (fast, slow, _, _) in self.regimes() {
            series.push(atr_key(fast));
            series.push(atr_key(slow));
        }
        for period in self.absolutes() {
            series.push(atr_key(period));
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
        let mut abs_slot = inner_slots + 2 * self.regimes().count();
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
                Filter::VolAbs { min_pct, max_pct, .. } => {
                    let atr = ctx.s(abs_slot);
                    abs_slot += 1;
                    let pct = 100.0 * atr / ctx.bar.close;
                    pct.is_finite() && pct >= *min_pct && pct <= *max_pct
                }
                // Signal-bar time; the fill lands one bar later (see the
                // variant's doc). No calendar installed → never blocked.
                Filter::News { before_min, after_min, min_impact, currencies } => !news::in_blackout(
                    ctx.bar.time,
                    i64::from(*before_min) * 60_000,
                    i64::from(*after_min) * 60_000,
                    *min_impact,
                    Some(currencies),
                ),
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
    fn named_weekdays_parse_to_a_mask() {
        assert_eq!(Filter::parse("weekdays:MoTuWeTh").unwrap(), Filter::Weekdays { mask: 0b001_1110 });
        assert_eq!(Filter::parse("weekdays:fr").unwrap(), Filter::Weekdays { mask: 0b010_0000 });
        assert!(Filter::parse("weekdays:Mon").is_err());
    }

    #[test]
    fn weekdays_exclude_the_weekend_on_the_new_york_clock() {
        let f = Filtered { inner: &Always, filters: vec![Filter::weekdays()] };
        assert!(matches!(intent_at(&f, monday_utc(13, 0), None), Intent::Enter { .. }));
        // Sunday 23:00 UTC is Sunday 19:00 New York — the open, but a Sunday.
        assert!(matches!(intent_at(&f, monday_utc(-1, 0), None), Intent::None));
    }

    #[test]
    fn filters_parse_from_their_batch_file_spelling() {
        assert_eq!(Filter::parse("weekdays").unwrap(), Filter::weekdays());
        assert_eq!(Filter::parse("hours:0800-1200").unwrap(), Filter::hours(800, 1200));
        assert_eq!(Filter::parse("flat:1630-1815").unwrap(), Filter::flat(1630, 1815));
        assert_eq!(Filter::parse("sessions:0100-0500|0600-1000").unwrap(), Filter::sessions(&[(100, 500), (600, 1000)]));
        assert_eq!(
            Filter::parse("vol:14/100:1.2-99").unwrap(),
            Filter::VolRegime { fast: 14, slow: 100, min_ratio: 1.2, max_ratio: 99.0 }
        );
        assert_eq!(Filter::parse("volabs:14:0.075-9").unwrap(), Filter::VolAbs { period: 14, min_pct: 0.075, max_pct: 9.0 });
        assert!(Filter::parse("hours:0860-1200").unwrap_err().contains("clock"));
        assert!(Filter::parse("moon:full").unwrap_err().contains("unknown"));
    }

    fn news(before_min: u32, after_min: u32, min_impact: u8, currencies: &[&str]) -> Filter {
        Filter::News { before_min, after_min, min_impact, currencies: currencies.iter().map(|c| (*c).to_string()).collect() }
    }

    #[test]
    fn the_news_filter_parses_widths_in_minutes_and_an_optional_impact() {
        assert_eq!(Filter::parse("news:60-30").unwrap(), news(60, 30, 3, &[]));
        assert_eq!(Filter::parse("news:60-30:2").unwrap(), news(60, 30, 2, &[]));
        assert_eq!(Filter::parse("news:0-15:1").unwrap(), news(0, 15, 1, &[]));
        assert!(Filter::parse("news:").unwrap_err().contains("before-after"));
        assert!(Filter::parse("news:60").unwrap_err().contains("before-after"));
        assert!(Filter::parse("news:60-x").unwrap_err().contains("minutes"));
        assert!(Filter::parse("news:60-30:0").unwrap_err().contains("impact"));
        assert!(Filter::parse("news:60-30:4").unwrap_err().contains("impact"));
        assert_eq!(Filter::parse("news:60-30").unwrap().describe(), "no entries 60 min before to 30 min after high-impact news");
        assert_eq!(Filter::parse("news:60-30:2").unwrap().describe(), "no entries 60 min before to 30 min after impact≥2 news");
    }

    #[test]
    fn the_news_filter_parses_an_optional_currency_list_after_the_impact() {
        assert_eq!(Filter::parse("news:60-30:3:USD|EUR").unwrap(), news(60, 30, 3, &["USD", "EUR"]));
        assert_eq!(Filter::parse("news:60-30:2:usd").unwrap(), news(60, 30, 2, &["USD"]), "codes are upper-cased");
        assert_eq!(
            Filter::parse("news:60-30:3:USD|EUR").unwrap().describe(),
            "no entries 60 min before to 30 min after high-impact news (USD|EUR)"
        );
        // The impact is required before a currency list: `news:60-30:USD`
        // reads as an impact and is refused as one.
        assert!(Filter::parse("news:60-30:USD").unwrap_err().contains("impact"));
        assert!(Filter::parse("news:60-30:3:").unwrap_err().contains("currencies"));
        assert!(Filter::parse("news:60-30:3:US1").unwrap_err().contains("currency code"));
    }

    #[test]
    fn parse_for_market_scopes_an_unscoped_news_filter_and_nothing_else() {
        let usd = vec!["usd".to_string()];
        assert_eq!(Filter::parse_for_market("news:60-30", &usd).unwrap(), news(60, 30, 3, &["USD"]));
        assert_eq!(Filter::parse_for_market("news:60-30", &[]).unwrap(), news(60, 30, 3, &[]), "an empty market list stays empty: every currency");
        // A spelled list wins over the market's.
        assert_eq!(Filter::parse_for_market("news:60-30:3:EUR", &usd).unwrap(), news(60, 30, 3, &["EUR"]));
        assert_eq!(Filter::parse_for_market("weekdays", &usd).unwrap(), Filter::weekdays());
        assert_eq!(Filter::parse_for_market("news:60-30", &usd).unwrap().describe(), "no entries 60 min before to 30 min after high-impact news (USD)");
    }

    /// Wants out on every bar it holds; never enters.
    struct ExitNow;
    impl Strategy for ExitNow {
        fn id(&self) -> &'static str {
            "exit-now"
        }
        fn name(&self) -> &'static str {
            "exit-now"
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
            if ctx.position.is_some() { Intent::Exit { reason: "done".into() } } else { Intent::None }
        }
    }

    #[test]
    fn the_news_filter_gates_entries_on_the_signal_bar_and_never_exits() {
        // The shared calendar (see `news::test_events`): a high-impact
        // release at 12:30 UTC on the Monday. Window: [11:30, 13:00).
        news::install(news::test_events()).expect("shared install");
        let f = Filtered { inner: &Always, filters: vec![Filter::parse("news:60-30").unwrap()] };
        let release = monday_utc(12, 30);

        // Inside the window: the signal is blocked.
        assert!(matches!(intent_at(&f, release - 60 * 60_000, None), Intent::None), "front edge is inclusive");
        assert!(matches!(intent_at(&f, release, None), Intent::None), "the release bar");
        assert!(matches!(intent_at(&f, release + 29 * 60_000, None), Intent::None), "last minute after");

        // One (one-minute) bar outside on either side: not blocked. The bar
        // at 11:29 opens before the window and would FILL at 11:30, inside
        // it — that is by design: the filter reads the signal bar's time,
        // and a hypothesis that needs the fill outside pads `before`.
        assert!(matches!(intent_at(&f, release - 61 * 60_000, None), Intent::Enter { .. }));
        assert!(matches!(intent_at(&f, release + 30 * 60_000, None), Intent::Enter { .. }), "back edge is exclusive");

        // The medium EUR event at 09:00 does not count at the default
        // threshold, and does at `:2`.
        let medium = monday_utc(9, 0);
        assert!(matches!(intent_at(&f, medium, None), Intent::Enter { .. }));
        let f2 = Filtered { inner: &Always, filters: vec![Filter::parse("news:60-30:2").unwrap()] };
        assert!(matches!(intent_at(&f2, medium, None), Intent::None));

        // The high CAD event at 15:00 blocks an unscoped filter and not a
        // USD-scoped one — the whole point of the scope: a Canadian rate
        // decision is not gold's business.
        let cad = monday_utc(15, 0);
        assert!(matches!(intent_at(&f, cad, None), Intent::None));
        let usd = Filtered { inner: &Always, filters: vec![Filter::parse_for_market("news:60-30", &["USD".to_string()]).unwrap()] };
        assert!(matches!(intent_at(&usd, cad, None), Intent::Enter { .. }));
        assert!(matches!(intent_at(&usd, release, None), Intent::None), "the USD release still blocks");

        // An exit inside the window is not gated: a strategy that wants out
        // gets out.
        let g = Filtered { inner: &ExitNow, filters: vec![Filter::parse("news:60-30").unwrap()] };
        let open = OpenPosition { side: Side::Long, entry_price: 100.0, entry_time: 0, stop: None, target: None };
        assert!(matches!(intent_at(&g, release, Some(open)), Intent::Exit { .. }));
        // And the filter adds no series or warm-up of its own.
        assert!(f.series(&Params::default()).is_empty());
        assert_eq!(f.warmup(&Params::default()), 0);
    }

    #[test]
    fn the_absolute_volatility_filter_reads_atr_over_price() {
        let f = Filtered { inner: &Always, filters: vec![Filter::VolAbs { period: 14, min_pct: 0.075, max_pct: 9.0 }] };
        assert_eq!(f.series(&Params::default()), vec!["atr_14".to_string()]);
        let bar = Bar::flat(monday_utc(13, 0), 4000.0);
        let bars = [bar];
        let ind = fd_indicators::IndicatorSet::new();
        let params = Params::default();
        let low: &[f64] = &[2.0]; // 0.05% of price: below the gate
        let high: &[f64] = &[4.0]; // 0.10%: above it
        let ctx_low = BarContext { bar: &bars[0], i: 0, bars: &bars, ind: &ind, series: &[low], options: None, position: None, params: &params };
        let ctx_high = BarContext { bar: &bars[0], i: 0, bars: &bars, ind: &ind, series: &[high], options: None, position: None, params: &params };
        assert!(matches!(f.on_bar(&ctx_low), Intent::None));
        assert!(matches!(f.on_bar(&ctx_high), Intent::Enter { .. }));
    }

    #[test]
    fn the_regime_filter_adds_its_series_after_the_strategy_s_own() {
        let f = Filtered { inner: &Always, filters: vec![Filter::VolRegime { fast: 14, slow: 100, min_ratio: 1.2, max_ratio: 9.0 }] };
        assert_eq!(f.series(&Params::default()), vec!["atr_14".to_string(), "atr_100".to_string()]);
        assert_eq!(f.warmup(&Params::default()), 105);
        assert_eq!(f.describe(), "ATR14/ATR100 in [1.2, 9]");
    }
}
