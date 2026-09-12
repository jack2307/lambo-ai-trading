//! New York time, because that is the clock gold trades on.
//!
//! Every timestamp in this workspace is UTC milliseconds. But the things a
//! session strategy cares about — the CME daily break, the COMEX pit open, the
//! Friday close, the swap rollover — happen at fixed *New York* times and move
//! against UTC twice a year. A filter written in UTC hours is wrong for eight
//! months of the year; this module is what makes "17:00 New York" a thing the
//! engine can compute for any bar.
//!
//! Only the US daylight-saving rule since 2007 is implemented (second Sunday of
//! March to first Sunday of November, 02:00 local). Nothing here reaches back
//! further than the bars do.

const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 86_400_000;

/// Days since 1970-01-01 for a civil date (proleptic Gregorian).
#[must_use]
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let (y, m, d) = (year, i64::from(month), i64::from(day));
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil date `(year, month, day)` for days since 1970-01-01.
#[must_use]
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Day of week for days since the epoch: 0 = Sunday … 6 = Saturday.
#[must_use]
pub fn weekday_of_days(days: i64) -> u32 {
    // 1970-01-01 was a Thursday (4).
    (days + 4).rem_euclid(7) as u32
}

/// The `n`-th Sunday of a month, as days since the epoch.
fn nth_sunday(year: i64, month: u32, n: i64) -> i64 {
    let first = days_from_civil(year, month, 1);
    let to_sunday = (7 - i64::from(weekday_of_days(first))) % 7;
    first + to_sunday + 7 * (n - 1)
}

/// True when New York observes daylight time at this UTC instant.
#[must_use]
pub fn new_york_is_dst(utc_ms: i64) -> bool {
    let (year, _, _) = civil_from_days(utc_ms.div_euclid(DAY_MS));
    // DST starts 02:00 EST = 07:00 UTC on the second Sunday of March and ends
    // 02:00 EDT = 06:00 UTC on the first Sunday of November.
    let start = nth_sunday(year, 3, 2) * DAY_MS + 7 * HOUR_MS;
    let end = nth_sunday(year, 11, 1) * DAY_MS + 6 * HOUR_MS;
    utc_ms >= start && utc_ms < end
}

/// Milliseconds to add to UTC to get New York wall-clock time.
#[must_use]
pub fn new_york_offset_ms(utc_ms: i64) -> i64 {
    if new_york_is_dst(utc_ms) { -4 * HOUR_MS } else { -5 * HOUR_MS }
}

/// New York wall clock for a UTC instant: `(weekday 0=Sun, minute of day)`.
#[must_use]
pub fn new_york_local(utc_ms: i64) -> (u32, u32) {
    let local = utc_ms + new_york_offset_ms(utc_ms);
    let days = local.div_euclid(DAY_MS);
    let minute = (local.rem_euclid(DAY_MS) / 60_000) as u32;
    (weekday_of_days(days), minute)
}

/// Swap rollovers crossed by a position held from `entry` to `exit` (UTC ms),
/// weighted the way brokers charge them: one per 17:00 New York crossed, and
/// three for the Wednesday one, which carries the weekend.
///
/// A position opened and closed inside one session crosses none — which is
/// exactly what an intraday rule buys, and why the count is worth a function.
#[must_use]
pub fn swap_nights(entry_utc_ms: i64, exit_utc_ms: i64) -> u32 {
    if exit_utc_ms <= entry_utc_ms {
        return 0;
    }
    // Index of the trading day, counted from 17:00 New York.
    let session_day = |utc: i64| (utc + new_york_offset_ms(utc) - 17 * HOUR_MS).div_euclid(DAY_MS);
    let (first, last) = (session_day(entry_utc_ms), session_day(exit_utc_ms));
    let mut nights = 0u32;
    for day in first..last {
        // The boundary crossed at the end of `day` falls on weekday of day+1's
        // 17:00 … which is the same calendar day as `day`'s 17:00 boundary end.
        let boundary_weekday = weekday_of_days(day + 1);
        nights += if boundary_weekday == 3 { 3 } else { 1 };
    }
    nights
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(year: i64, month: u32, day: u32, hour: i64, minute: i64) -> i64 {
        days_from_civil(year, month, day) * DAY_MS + hour * HOUR_MS + minute * 60_000
    }

    #[test]
    fn civil_round_trips() {
        for days in [-719_468, -1, 0, 1, 19_000, 20_698, 60_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days, "{y}-{m}-{d}");
        }
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(weekday_of_days(0), 4, "1970-01-01 was a Thursday");
        assert_eq!(weekday_of_days(days_from_civil(2026, 9, 13)), 0, "2026-09-13 is a Sunday");
    }

    #[test]
    fn dst_switches_on_the_2026_dates() {
        // 2026: DST begins Sunday March 8 at 07:00 UTC, ends Sunday November 1 at 06:00 UTC.
        assert!(!new_york_is_dst(utc(2026, 3, 8, 6, 59)));
        assert!(new_york_is_dst(utc(2026, 3, 8, 7, 0)));
        assert!(new_york_is_dst(utc(2026, 11, 1, 5, 59)));
        assert!(!new_york_is_dst(utc(2026, 11, 1, 6, 0)));
        assert_eq!(new_york_offset_ms(utc(2026, 7, 1, 12, 0)), -4 * HOUR_MS);
        assert_eq!(new_york_offset_ms(utc(2026, 1, 1, 12, 0)), -5 * HOUR_MS);
    }

    #[test]
    fn the_cme_break_is_17_00_new_york_in_both_seasons() {
        // Summer: 21:00 UTC. Winter: 22:00 UTC. Both read as 17:00 local.
        assert_eq!(new_york_local(utc(2026, 7, 1, 21, 0)), (3, 17 * 60));
        assert_eq!(new_york_local(utc(2026, 1, 7, 22, 0)), (3, 17 * 60));
        // Friday 20:45 UTC in September is Friday 16:45 New York.
        assert_eq!(new_york_local(utc(2026, 9, 11, 20, 45)), (5, 16 * 60 + 45));
    }

    #[test]
    fn swap_nights_count_rollovers_and_triple_wednesday() {
        // Monday 10:00 -> Monday 15:00 New York: intraday, nothing charged.
        assert_eq!(swap_nights(utc(2026, 9, 14, 14, 0), utc(2026, 9, 14, 19, 0)), 0);
        // Monday 10:00 -> Tuesday 10:00: one rollover (Monday 17:00).
        assert_eq!(swap_nights(utc(2026, 9, 14, 14, 0), utc(2026, 9, 15, 14, 0)), 1);
        // Wednesday 10:00 -> Thursday 10:00: crosses Wednesday 17:00, charged thrice.
        assert_eq!(swap_nights(utc(2026, 9, 16, 14, 0), utc(2026, 9, 17, 14, 0)), 3);
        // Monday 10:00 -> Friday 10:00: Mon, Tue, Wed(x3), Thu = 6.
        assert_eq!(swap_nights(utc(2026, 9, 14, 14, 0), utc(2026, 9, 18, 14, 0)), 6);
        // A trade that straddles 17:00 by a minute is still one night.
        assert_eq!(swap_nights(utc(2026, 9, 14, 20, 59), utc(2026, 9, 14, 21, 1)), 1);
        assert_eq!(swap_nights(5, 5), 0);
    }
}
