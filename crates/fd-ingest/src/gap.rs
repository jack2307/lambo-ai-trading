//! Finding what a feed missed.
//!
//! A websocket that reconnects leaves a hole. The hole is invisible — the
//! series simply carries on — so it has to be looked for deliberately, and the
//! search has to look *inside* the series rather than only at its end.
//!
//! That distinction is not theoretical. Measuring only the distance from the
//! newest bar to now finds nothing at all when the feed dropped for two hours
//! and then recovered: the tail is fresh, and a 135-minute hole sits happily in
//! the middle of the series. This module scans backwards instead.

use fd_core::types::Bar;

/// A bar is late if it is this far behind its predecessor.
///
/// One minute is the native bar, so two minutes is a break rather than a slow
/// frame. Anything tighter turns ordinary jitter into a resync storm.
pub const BREAK_MS: i64 = 120_000;

/// Where a backfill should start to repair `series`.
///
/// Returns the earliest point that needs refetching, never earlier than
/// `now_ms - lookback_ms`: history beyond the lookback is the store's business,
/// not a live feed's.
#[must_use]
pub fn resync_start(series: &[Bar], now_ms: i64, lookback_ms: i64) -> i64 {
    let cutoff = now_ms - lookback_ms;
    let Some(last) = series.last() else { return cutoff };

    let mut from = last.time;
    // Backwards, so the *earliest* break inside the window wins: repairing from
    // the latest one would leave every older hole in place.
    for i in (1..series.len()).rev() {
        if series[i].time < cutoff {
            break;
        }
        if series[i].time - series[i - 1].time > BREAK_MS {
            from = series[i - 1].time;
        }
    }
    from.max(cutoff)
}

/// Every hole in `series`, as `(after, before)` pairs of bar times.
///
/// Reported rather than repaired: a caller may want to refetch, or may want to
/// know that a venue was simply closed.
#[must_use]
pub fn gaps(series: &[Bar], step_ms: i64) -> Vec<(i64, i64)> {
    series
        .windows(2)
        .filter(|w| w[1].time - w[0].time > step_ms * 2)
        .map(|w| (w[0].time, w[1].time))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(times: &[i64]) -> Vec<Bar> {
        times.iter().map(|t| Bar::flat(*t, 1.0)).collect()
    }

    const MINUTE: i64 = 60_000;

    #[test]
    fn an_unbroken_series_needs_no_repair_before_its_last_bar() {
        let bars = series(&[0, MINUTE, 2 * MINUTE, 3 * MINUTE]);
        assert_eq!(resync_start(&bars, 4 * MINUTE, 10 * MINUTE), 3 * MINUTE);
    }

    #[test]
    fn an_interior_hole_is_found_even_when_the_tail_is_fresh() {
        // The failure this function exists for: the newest bar is current, so a
        // tail-only check sees nothing, while 135 minutes are missing inside.
        let mut times = vec![0, MINUTE, 2 * MINUTE];
        let resume = 2 * MINUTE + 135 * MINUTE;
        times.extend([resume, resume + MINUTE, resume + 2 * MINUTE]);
        let bars = series(&times);

        let now = resume + 3 * MINUTE;
        assert_eq!(
            resync_start(&bars, now, 24 * 60 * MINUTE),
            2 * MINUTE,
            "the repair must start before the hole, not at the newest bar"
        );
    }

    #[test]
    fn the_earliest_break_in_the_window_wins() {
        let bars = series(&[0, 10 * MINUTE, 11 * MINUTE, 30 * MINUTE, 31 * MINUTE]);
        assert_eq!(resync_start(&bars, 32 * MINUTE, 60 * MINUTE), 0);
    }

    #[test]
    fn a_repair_never_reaches_further_back_than_the_lookback() {
        let bars = series(&[0, 500 * MINUTE]);
        let now = 501 * MINUTE;
        let lookback = 10 * MINUTE;
        assert_eq!(resync_start(&bars, now, lookback), now - lookback);
    }

    #[test]
    fn an_empty_series_asks_for_the_whole_lookback() {
        assert_eq!(resync_start(&[], 1000, 400), 600);
    }

    #[test]
    fn holes_are_reported_as_the_bars_either_side() {
        let bars = series(&[0, MINUTE, 10 * MINUTE, 11 * MINUTE]);
        assert_eq!(gaps(&bars, MINUTE), vec![(MINUTE, 10 * MINUTE)]);
    }
}
