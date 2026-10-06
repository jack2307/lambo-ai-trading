//! Scheduled-news blackout: the event list every `news:` filter reads.
//!
//! # Why this is a process-wide global
//!
//! A [`crate::filter::Filter`] is a value parsed from a string
//! (`news:60-30`) and carries no IO: a batch file, a Workbench request and a
//! matched null all spell the same gate and must see the same calendar. The
//! list is therefore installed **once**, by the binary that owns the data
//! directory (`search`, `fd-api`), from `data/news/events.parquet`, and read
//! by every filter through [`in_blackout`]. Nothing here touches a file.
//!
//! Sweeps run filters in parallel, so the list is immutable after
//! [`install`]: a second install with the same list is accepted (idempotent),
//! a different one is refused rather than silently swapping the calendar under
//! a running search. When nothing is installed, every `news:` filter is a
//! no-op and [`in_blackout`] is `false` — the binaries print which case holds
//! so a receipt can quote it.
//!
//! # Semantics
//!
//! [`in_blackout`]`(t, before, after, min_impact, currencies)` is true when
//! some installed event with `impact >= min_impact` **of the market's
//! currencies** satisfies `event.time - before <= t < event.time + after` —
//! closed at the front, open at the back, so a bar stamped exactly `after`
//! ms past the release is already tradable. The `t` a filter passes is the
//! **signal bar's open time**; the fill is the next bar's open (the engine's
//! rule), so a bar that opens just before the window and would fill inside
//! it is *not* blocked. Pad `before` by one bar if that matters to a
//! hypothesis.
//!
//! [`in_news_window`]`(t, from, to, …)` is the same arithmetic with **signed**
//! offsets and the opposite sense: true when `t` is inside
//! `[event + from, event + to)`. It is what a gate that **requires** an event
//! reads ([`crate::filter::Filter::NewsOnly`]), and it is what
//! [`in_blackout`] is implemented as, at `(-before, +after)` — so the two can
//! never drift apart. Until 2026-10-06 this file could only say "stay away
//! from a release"; a method that enters *because* of one could not be
//! spelled, so it had never been measured.
//! [`last_event_at_or_before`] is the third piece: a window tells a bar that
//! it is near some release, and only this tells it **which**, which is what a
//! rule anchored on the release itself needs
//! (`docs/decisions/2026-10-06-news-entry.md`).
//!
//! # Currencies
//!
//! The calendar carries every currency, and until 2026-09-14 every reader
//! matched all of them: a Canadian rate decision flattened a gold bot. Each
//! market now names its list (`[markets.<id>.trading] news_currencies`,
//! `["USD"]` on gold) and every reader passes it: `None` or an empty list is
//! every currency, otherwise an event counts when its currency is in the
//! list (case-insensitive) or is the calendar's global `All`
//! ([`NewsEvent::concerns`]).

use std::sync::OnceLock;

use fd_core::clock::civil_from_days;
pub use fd_core::types::NewsEvent;

static EVENTS: OnceLock<Vec<NewsEvent>> = OnceLock::new();

/// Install the calendar for this process. Sorts by time.
///
/// Returns the number of events. Installing the same list twice is fine;
/// a different list once one is installed is an error, because filters that
/// already ran read the first one.
pub fn install(mut events: Vec<NewsEvent>) -> Result<usize, String> {
    events.sort_by_key(|e| e.time);
    let n = events.len();
    match EVENTS.set(events) {
        Ok(()) => Ok(n),
        Err(rejected) => {
            let current = EVENTS.get().expect("set failed, so a list is installed");
            if *current == rejected {
                Ok(n)
            } else {
                Err(format!(
                    "news: a different calendar is already installed ({} events; refused {} events)",
                    current.len(),
                    rejected.len()
                ))
            }
        }
    }
}

/// How many events are installed, or `None` when nothing has been.
#[must_use]
pub fn installed() -> Option<usize> {
    EVENTS.get().map(Vec::len)
}

/// The installed calendar, sorted by time; empty when nothing is installed.
#[must_use]
pub fn events() -> &'static [NewsEvent] {
    EVENTS.get().map_or(&[], Vec::as_slice)
}

/// True when `t_ms` is inside the blackout of any installed event with
/// `impact >= min_impact` whose currency the market cares about
/// (`currencies`: `None` or empty = every currency; `All` events always
/// count). False when nothing is installed.
#[must_use]
pub fn in_blackout(t_ms: i64, before_ms: i64, after_ms: i64, min_impact: u8, currencies: Option<&[String]>) -> bool {
    blackout_in(events(), t_ms, before_ms, after_ms, min_impact, currencies)
}

/// The arithmetic behind [`in_blackout`] over an explicit, time-sorted list.
///
/// A blackout is the window `[event - before, event + after)`, so this is
/// [`window_in`] at offsets `(-before, +after)` and the offset arithmetic
/// lives in exactly one place. Semantics are unchanged: the front edge is
/// closed, the back edge open, and `before = after = 0` blocks nothing.
#[must_use]
pub fn blackout_in(
    events: &[NewsEvent],
    t_ms: i64,
    before_ms: i64,
    after_ms: i64,
    min_impact: u8,
    currencies: Option<&[String]>,
) -> bool {
    window_in(events, t_ms, -before_ms, after_ms, min_impact, currencies)
}

/// True when `t_ms` is inside `[event + from_ms, event + to_ms)` of some
/// installed event the market cares about — the **requirement** a blackout is
/// the negation of, and the thing no filter could say before 2026-10-06.
///
/// False when nothing is installed, like every other reader here, so a
/// `newsonly:` gate on a run with no calendar takes **no** trades rather than
/// every trade.
///
/// Offsets are **signed** and measured from the release: `(0, 15 min)` is the
/// release bar of a 15m series, `(45 min, 60 min)` the fourth bar after it,
/// `(-30 min, 0)` the half hour before. `from_ms > to_ms` is an empty window
/// and is never inside.
#[must_use]
pub fn in_news_window(t_ms: i64, from_ms: i64, to_ms: i64, min_impact: u8, currencies: Option<&[String]>) -> bool {
    window_in(events(), t_ms, from_ms, to_ms, min_impact, currencies)
}

/// The arithmetic behind [`in_news_window`] over an explicit, time-sorted
/// list.
///
/// Binary search for the first event whose window has not yet closed
/// (`event + to > t`), then scan forward while it has opened
/// (`event + from <= t`). Both bounds are monotone in `event.time`, which is
/// what makes the search valid on a list sorted by time; the scan is bounded
/// by how many events share one span, a handful on any real calendar.
#[must_use]
pub fn window_in(
    events: &[NewsEvent],
    t_ms: i64,
    from_ms: i64,
    to_ms: i64,
    min_impact: u8,
    currencies: Option<&[String]>,
) -> bool {
    if from_ms > to_ms {
        return false;
    }
    let first = events.partition_point(|e| e.time.saturating_add(to_ms) <= t_ms);
    events[first..]
        .iter()
        .take_while(|e| e.time.saturating_add(from_ms) <= t_ms)
        .any(|e| e.impact >= min_impact && e.concerns(currencies))
}

/// The time of the newest installed event at or before `t_ms` that passes
/// `min_impact` and the market's currency scope; `None` when there is none
/// (including when no calendar is installed).
///
/// What a strategy that **anchors on a release** needs, as opposed to one that
/// only asks whether it is inside a window: an entry offset has to be counted
/// from the release itself, and a bar cannot ask "which event am I after?"
/// through [`in_news_window`].
#[must_use]
pub fn last_event_at_or_before(t_ms: i64, min_impact: u8, currencies: Option<&[String]>) -> Option<i64> {
    last_event_in(events(), t_ms, min_impact, currencies)
}

/// [`last_event_at_or_before`] over an explicit, time-sorted list.
#[must_use]
pub fn last_event_in(events: &[NewsEvent], t_ms: i64, min_impact: u8, currencies: Option<&[String]>) -> Option<i64> {
    let end = events.partition_point(|e| e.time <= t_ms);
    events[..end]
        .iter()
        .rev()
        .find(|e| e.impact >= min_impact && e.concerns(currencies))
        .map(|e| e.time)
}

/// The one-line receipt a binary prints after trying to load the calendar:
/// `news: N events (YYYY-MM-DD → YYYY-MM-DD) from <source>` or, when nothing
/// is installed, `news: none loaded — news: filters are no-ops`.
#[must_use]
pub fn summary(source: &str) -> String {
    let events = events();
    match (events.first(), events.last()) {
        (Some(first), Some(last)) => {
            format!("news: {} events ({} → {}) from {source}", events.len(), ymd(first.time), ymd(last.time))
        }
        _ => "news: none loaded — news: filters are no-ops".to_string(),
    }
}

fn ymd(ms: i64) -> String {
    let (y, m, d) = civil_from_days(ms.div_euclid(86_400_000));
    format!("{y:04}-{m:02}-{d:02}")
}

/// The calendar every test in this crate's unit-test binary installs.
///
/// The global is set once per process and the tests share one, so a test
/// that needs the installed path must install *this* list (idempotent) and
/// never another. Arithmetic on other lists goes through [`blackout_in`].
#[cfg(test)]
pub(crate) fn test_events() -> Vec<NewsEvent> {
    use fd_core::clock::days_from_civil;
    // 2026-09-14 (a Monday): high USD at 12:30 UTC (the NFP/CPI slot) and
    // high CAD at 15:00 UTC, far enough apart that their 60/30 windows do
    // not touch; medium EUR at 09:00 UTC.
    let day = days_from_civil(2026, 9, 14) * 86_400_000;
    vec![
        NewsEvent { time: day + 15 * 3_600_000, impact: 3, currency: "CAD".into(), name: "BoC Rate Statement".into() },
        NewsEvent { time: day + 12 * 3_600_000 + 30 * 60_000, impact: 3, currency: "USD".into(), name: "CPI m/m".into() },
        NewsEvent { time: day + 9 * 3_600_000, impact: 2, currency: "EUR".into(), name: "German ZEW".into() },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    fn ev(time: i64, impact: u8) -> NewsEvent {
        NewsEvent { time, impact, currency: "USD".into(), name: String::new() }
    }

    fn ev_ccy(time: i64, impact: u8, currency: &str) -> NewsEvent {
        NewsEvent { time, impact, currency: currency.into(), name: String::new() }
    }

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn the_window_is_closed_before_and_open_after() {
        // Three events: a high one at 1000 min, a medium one at 2000 min,
        // and a high one at 2030 min (two windows overlapping).
        let events = vec![ev(1000 * MIN, 3), ev(2000 * MIN, 2), ev(2030 * MIN, 3)];
        let (before, after) = (60 * MIN, 30 * MIN);
        let hit = |t: i64, min_impact: u8| blackout_in(&events, t, before, after, min_impact, None);

        // Front edge is inclusive: exactly 60 min before is blocked, one ms
        // earlier is not.
        assert!(hit(940 * MIN, 3));
        assert!(!hit(940 * MIN - 1, 3));
        // The event instant itself is blocked.
        assert!(hit(1000 * MIN, 3));
        // Back edge is exclusive: one ms before 30 min after is blocked,
        // exactly 30 min after is free.
        assert!(hit(1030 * MIN - 1, 3));
        assert!(!hit(1030 * MIN, 3));
        // Far away on either side.
        assert!(!hit(0, 3));
        assert!(!hit(1500 * MIN, 3));
        assert!(!hit(10_000 * MIN, 3));
    }

    #[test]
    fn min_impact_selects_which_events_count() {
        let events = vec![ev(1000 * MIN, 3), ev(2000 * MIN, 2), ev(2030 * MIN, 3)];
        let (before, after) = (60 * MIN, 30 * MIN);
        // 1950 min is inside the medium event's window only.
        assert!(!blackout_in(&events, 1950 * MIN, before, after, 3, None));
        assert!(blackout_in(&events, 1950 * MIN, before, after, 2, None));
        assert!(blackout_in(&events, 1950 * MIN, before, after, 1, None));
        // 2020 min is inside both the medium event's [1940, 2030) and the
        // high one's [1970, 2060): blocked at every threshold.
        assert!(blackout_in(&events, 2020 * MIN, before, after, 3, None));
        // 2040 min: the medium window has closed, the high one is open.
        assert!(blackout_in(&events, 2040 * MIN, before, after, 3, None));
        // 2070 min: both closed.
        assert!(!blackout_in(&events, 2070 * MIN, before, after, 1, None));
    }

    #[test]
    fn an_empty_list_never_blocks() {
        assert!(!blackout_in(&[], 0, 60 * MIN, 30 * MIN, 1, None));
        assert!(!blackout_in(&[], i64::MAX / 2, 0, 0, 0, None));
    }

    #[test]
    fn zero_widths_block_only_the_instant_before_the_release() {
        // before = 0, after = 0: [time, time) is empty — nothing is blocked.
        let events = vec![ev(1000 * MIN, 3)];
        assert!(!blackout_in(&events, 1000 * MIN, 0, 0, 3, None));
        // before = 0, after = 1 ms: only the release instant.
        assert!(blackout_in(&events, 1000 * MIN, 0, 1, 3, None));
        assert!(!blackout_in(&events, 1000 * MIN - 1, 0, 1, 3, None));
    }

    #[test]
    fn a_currency_list_selects_which_events_count() {
        let events = vec![ev_ccy(1000 * MIN, 3, "USD"), ev_ccy(2000 * MIN, 3, "CAD"), ev_ccy(3000 * MIN, 3, "All")];
        let (before, after) = (60 * MIN, 30 * MIN);
        let usd = list(&["USD"]);
        let hit = |t: i64, ccy: Option<&[String]>| blackout_in(&events, t, before, after, 3, ccy);
        // A USD event blocks a USD market; a CAD one does not; `All` does.
        assert!(hit(1000 * MIN, Some(&usd)));
        assert!(!hit(2000 * MIN, Some(&usd)));
        assert!(hit(3000 * MIN, Some(&usd)));
        // `None` and an empty list are every currency.
        assert!(hit(2000 * MIN, None));
        assert!(hit(2000 * MIN, Some(&[])));
        // Case-insensitive on both sides, `All` in any spelling.
        let lower = list(&["usd", "cad"]);
        assert!(hit(2000 * MIN, Some(&lower)));
        let shouting = vec![ev_ccy(1000 * MIN, 3, "ALL")];
        assert!(blackout_in(&shouting, 1000 * MIN, before, after, 3, Some(&usd)));
        // A pair's list sees both legs.
        let eurusd = list(&["USD", "EUR"]);
        let eur = vec![ev_ccy(1000 * MIN, 3, "EUR")];
        assert!(blackout_in(&eur, 1000 * MIN, before, after, 3, Some(&eurusd)));
        assert!(!blackout_in(&eur, 1000 * MIN, before, after, 3, Some(&usd)));
    }

    #[test]
    fn install_sorts_and_is_idempotent_for_the_same_list() {
        let n = install(test_events()).expect("first (or identical) install");
        assert_eq!(n, 3);
        assert_eq!(installed(), Some(3));
        let times: Vec<i64> = events().iter().map(|e| e.time).collect();
        assert!(times.windows(2).all(|w| w[0] <= w[1]), "installed list is sorted: {times:?}");
        // The same list, in a different order, is the same calendar.
        let mut again = test_events();
        again.reverse();
        assert_eq!(install(again), Ok(3));
    }

    #[test]
    fn a_different_calendar_is_refused_once_one_is_installed() {
        install(test_events()).expect("shared install");
        let other = vec![ev(1, 3)];
        let err = install(other).unwrap_err();
        assert!(err.contains("different calendar"), "{err}");
        assert_eq!(installed(), Some(3), "the first list stays");
    }

    #[test]
    fn the_global_path_reads_the_installed_list() {
        install(test_events()).expect("shared install");
        let release = test_events()[1].time; // 12:30 UTC, high
        assert!(in_blackout(release - 60 * MIN, 60 * MIN, 30 * MIN, 3, None));
        assert!(!in_blackout(release - 60 * MIN - 1, 60 * MIN, 30 * MIN, 3, None));
        assert!(!in_blackout(release + 30 * MIN, 60 * MIN, 30 * MIN, 3, None));
        // The 09:00 EUR event is medium: invisible at the default threshold.
        let medium = test_events()[2].time;
        assert!(!in_blackout(medium, 60 * MIN, 30 * MIN, 3, None));
        assert!(in_blackout(medium, 60 * MIN, 30 * MIN, 2, None));
        // The 15:00 CAD event is high, but not a USD market's business.
        let cad = test_events()[0].time;
        let usd = list(&["USD"]);
        assert!(in_blackout(cad, 60 * MIN, 30 * MIN, 3, None));
        assert!(!in_blackout(cad, 60 * MIN, 30 * MIN, 3, Some(&usd)));
        assert!(in_blackout(release, 60 * MIN, 30 * MIN, 3, Some(&usd)));
    }

    /// The requirement is the exact negation of the blackout on the same
    /// offsets — which is the whole point of adding it, and the one property
    /// that keeps `newsonly:` from drifting away from `news:`.
    #[test]
    fn a_required_window_is_the_negation_of_the_blackout_on_the_same_offsets() {
        let events = vec![ev(1000 * MIN, 3), ev(2000 * MIN, 2), ev(2030 * MIN, 3)];
        let (before, after) = (60 * MIN, 30 * MIN);
        for t in (0..3_000).map(|m| m * MIN) {
            for impact in 1..=3 {
                let blocked = blackout_in(&events, t, before, after, impact, None);
                let required = window_in(&events, t, -before, after, impact, None);
                assert_eq!(blocked, required, "t = {t}, impact = {impact}");
            }
        }
    }

    /// Signed offsets are what let a row admit exactly the bar its own probe
    /// fires on: `[0, 15)` is the release bar of a 15m series, `[45, 60)` the
    /// fourth bar after it, and the two windows never overlap.
    #[test]
    fn signed_offsets_name_one_bar_of_a_fifteen_minute_series() {
        let release = 1000 * MIN;
        let events = vec![ev(release, 3)];
        let bar = |n: i64| release + n * 15 * MIN;
        let hit = |t: i64, from: i64, to: i64| window_in(&events, t, from * MIN, to * MIN, 3, None);

        // `newsonly:0/15` — the release bar, and only it.
        assert!(hit(bar(0), 0, 15));
        assert!(!hit(bar(1), 0, 15));
        assert!(!hit(bar(-1), 0, 15));
        // `newsonly:15/30` — the bar after.
        assert!(hit(bar(1), 15, 30));
        assert!(!hit(bar(0), 15, 30));
        assert!(!hit(bar(2), 15, 30));
        // `newsonly:45/60` — the fourth bar.
        assert!(hit(bar(3), 45, 60));
        assert!(!hit(bar(2), 45, 60));
        assert!(!hit(bar(4), 45, 60));
        // A window entirely before the release, which `news:` cannot express
        // either way round.
        assert!(hit(bar(-2), -30, 0));
        assert!(!hit(bar(0), -30, 0), "the back edge is open, so the release bar itself is out");
        // An inverted window is empty rather than everything.
        assert!(!hit(bar(0), 60, 0));
        // Nothing installed is nothing required: a `newsonly:` gate with no
        // calendar must take no trades, not every trade.
        assert!(!window_in(&[], bar(0), 0, 15 * MIN, 3, None));
    }

    #[test]
    fn the_required_window_honours_impact_and_currency_like_the_blackout() {
        let events = vec![ev_ccy(1000 * MIN, 3, "USD"), ev_ccy(2000 * MIN, 2, "USD"), ev_ccy(3000 * MIN, 3, "CAD")];
        let usd = list(&["USD"]);
        let hit = |t: i64, imp: u8, ccy: Option<&[String]>| window_in(&events, t, 0, 15 * MIN, imp, ccy);
        assert!(hit(1000 * MIN, 3, Some(&usd)));
        assert!(!hit(2000 * MIN, 3, Some(&usd)), "medium is invisible at impact 3");
        assert!(hit(2000 * MIN, 2, Some(&usd)));
        assert!(!hit(3000 * MIN, 3, Some(&usd)), "a Canadian release is not a USD market's business");
        assert!(hit(3000 * MIN, 3, None));
    }

    #[test]
    fn the_newest_qualifying_event_at_or_before_a_time_is_found() {
        let events = vec![ev_ccy(1000 * MIN, 3, "USD"), ev_ccy(2000 * MIN, 2, "USD"), ev_ccy(3000 * MIN, 3, "CAD")];
        let usd = list(&["USD"]);
        let at = |t: i64, imp: u8, ccy: Option<&[String]>| last_event_in(&events, t, imp, ccy);
        // The release instant itself counts: `at or before`.
        assert_eq!(at(1000 * MIN, 3, None), Some(1000 * MIN));
        assert_eq!(at(1000 * MIN - 1, 3, None), None, "nothing before the first one");
        // A later time skips back over events the scope rejects.
        assert_eq!(at(2500 * MIN, 3, None), Some(1000 * MIN), "the medium one is not high-impact");
        assert_eq!(at(2500 * MIN, 2, None), Some(2000 * MIN));
        assert_eq!(at(3500 * MIN, 3, Some(&usd)), Some(1000 * MIN), "the CAD one is out of scope");
        assert_eq!(at(3500 * MIN, 3, None), Some(3000 * MIN));
        // And no calendar is no event, rather than a panic or a zero.
        assert_eq!(last_event_in(&[], 3500 * MIN, 3, None), None);
    }

    #[test]
    fn the_summary_quotes_the_span_and_the_source() {
        install(test_events()).expect("shared install");
        assert_eq!(summary("data/news/events.parquet"), "news: 3 events (2026-09-14 → 2026-09-14) from data/news/events.parquet");
    }
}
