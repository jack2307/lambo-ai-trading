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
/// Binary search for the first event whose window has not yet closed
/// (`time > t - after`), then scan forward while the window has opened
/// (`time - before <= t`). The scan is bounded by how many events share one
/// span, which is a handful on any real calendar.
#[must_use]
pub fn blackout_in(
    events: &[NewsEvent],
    t_ms: i64,
    before_ms: i64,
    after_ms: i64,
    min_impact: u8,
    currencies: Option<&[String]>,
) -> bool {
    let first = events.partition_point(|e| e.time <= t_ms - after_ms);
    events[first..]
        .iter()
        .take_while(|e| e.time - before_ms <= t_ms)
        .any(|e| e.impact >= min_impact && e.concerns(currencies))
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

    #[test]
    fn the_summary_quotes_the_span_and_the_source() {
        install(test_events()).expect("shared install");
        assert_eq!(summary("data/news/events.parquet"), "news: 3 events (2026-09-14 → 2026-09-14) from data/news/events.parquet");
    }
}
