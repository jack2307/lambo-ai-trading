//! Turning a fine bar series into a coarser one.
//!
//! The store keeps the finest series a source will give and every other
//! timeframe is derived, because keeping both and letting them drift is how a
//! chart and a backtest end up disagreeing about what happened.
//!
//! Two details carried over from the prototype deliberately:
//!
//! * **A close-only row can only contribute its close.** The gold feed has no
//!   highs or lows, so a bucket built from it has `open == high == low ==
//!   close` — flat by construction rather than by a quiet market. That is what
//!   [`Bar::is_synthetic`] reports, and why gold's fine timeframes are marked
//!   synthetic while BTC's are not.
//! * **A missing volume counts as one, not as zero.** A zero would make a
//!   resampled bucket's volume meaningless the moment any row lacked the field,
//!   and the prototype's charts already read it this way.

use fd_core::types::Bar;

/// Bucket `rows` into bars of `step_ms`.
///
/// Input must be ascending in time; a bucket is closed as soon as a row lands
/// past it, so out-of-order input would silently split a bucket in two.
#[must_use]
pub fn resample(rows: &[Bar], step_ms: i64) -> Vec<Bar> {
    if step_ms <= 0 {
        return rows.to_vec();
    }
    let mut out: Vec<Bar> = Vec::new();
    for row in rows {
        let open = if row.open.is_finite() { row.open } else { row.close };
        let high = if row.high.is_finite() { row.high } else { row.close };
        let low = if row.low.is_finite() { row.low } else { row.close };
        let volume = row.volume.filter(|v| v.is_finite()).unwrap_or(1.0);
        let start = row.time.div_euclid(step_ms) * step_ms;

        match out.last_mut() {
            Some(current) if current.time == start => {
                current.high = current.high.max(high);
                current.low = current.low.min(low);
                current.close = row.close;
                current.volume = Some(current.volume.unwrap_or_default() + volume);
            }
            _ => out.push(Bar {
                time: start,
                open,
                high,
                low,
                close: row.close,
                volume: Some(volume),
            }),
        }
    }
    out
}

/// Named timeframes, in milliseconds.
#[must_use]
pub fn timeframe_ms(timeframe: &str) -> Option<i64> {
    Some(match timeframe {
        "1m" => 60_000,
        "5m" => 300_000,
        "15m" => 900_000,
        "30m" => 1_800_000,
        "1h" => 3_600_000,
        "4h" => 14_400_000,
        "1d" => 86_400_000,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60_000;

    #[test]
    fn a_bucket_takes_the_first_open_and_the_last_close() {
        let rows = vec![
            Bar { time: 0, open: 10.0, high: 12.0, low: 9.0, close: 11.0, volume: Some(1.0) },
            Bar { time: MINUTE, open: 11.0, high: 15.0, low: 10.0, close: 14.0, volume: Some(2.0) },
            Bar { time: 2 * MINUTE, open: 14.0, high: 14.5, low: 8.0, close: 9.0, volume: Some(3.0) },
        ];
        let out = resample(&rows, 5 * MINUTE);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].open, 10.0, "the open is the first row's, not the last");
        assert_eq!(out[0].close, 9.0, "the close is the last row's");
        assert_eq!(out[0].high, 15.0);
        assert_eq!(out[0].low, 8.0);
        assert_eq!(out[0].volume, Some(6.0), "volume adds across the bucket");
    }

    #[test]
    fn buckets_start_on_the_step_not_on_the_first_row() {
        // A series beginning at 03:07 must still produce 15-minute buckets that
        // start at 03:00, or the same minute lands in different buckets
        // depending on where the data happened to begin.
        let rows = vec![
            Bar::flat(7 * MINUTE, 1.0),
            Bar::flat(14 * MINUTE, 2.0),
            Bar::flat(15 * MINUTE, 3.0),
        ];
        let out = resample(&rows, 15 * MINUTE);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].time, 0);
        assert_eq!(out[1].time, 15 * MINUTE);
    }

    #[test]
    fn close_only_rows_stay_flat_and_say_so() {
        let rows = vec![Bar::flat(0, 4500.0), Bar::flat(MINUTE, 4510.0)];
        let out = resample(&rows, 5 * MINUTE);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].open, 4500.0);
        assert_eq!(out[0].close, 4510.0);
        // Two flat rows at different prices still make a bucket with a range —
        // the range comes from the closes, which is the most the feed supports.
        assert_eq!(out[0].high, 4510.0);
        assert_eq!(out[0].low, 4500.0);
    }

    #[test]
    fn a_row_without_volume_counts_as_one() {
        let rows = vec![Bar::flat(0, 1.0), Bar::flat(MINUTE, 1.0)];
        assert_eq!(resample(&rows, 5 * MINUTE)[0].volume, Some(2.0));
    }

    #[test]
    fn an_empty_series_resamples_to_nothing() {
        assert!(resample(&[], 900_000).is_empty());
    }

    #[test]
    fn timeframes_are_the_ones_the_project_uses() {
        assert_eq!(timeframe_ms("15m"), Some(900_000));
        assert_eq!(timeframe_ms("3m"), None);
    }
}
