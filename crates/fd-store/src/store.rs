//! A day-partitioned, append-only tape dataset.
//!
//! Parquet files are immutable once closed, so "appending" means writing
//! another file. Each append becomes one part inside the UTC day its prints
//! belong to:
//!
//! ```text
//! data/btc/tape/date=2026-09-12/part-000003.parquet
//! ```
//!
//! Two things fall out of that layout, both of which the prototype's single
//! JSON blob could not do. A time-range read opens only the days it overlaps.
//! And a live feed can flush what it has without rewriting history, which is
//! what makes an ingest process safe to kill at any moment.
//!
//! The cost is duplicates: a reconnect that backfills over a gap re-delivers
//! prints already on disk. Reads therefore de-duplicate by venue trade id, and
//! that — not the writer — is what makes the store idempotent.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use fd_core::types::OptionTrade;

use crate::error::StoreError;
use crate::tape::{read_tape, write_tape};

/// Milliseconds in a day.
const DAY_MS: i64 = 86_400_000;

/// A tape dataset rooted at a directory.
#[derive(Debug, Clone)]
pub struct TapeStore {
    root: PathBuf,
}

impl TapeStore {
    /// Open (or create) the store for `market` under `root`.
    pub fn open(root: impl AsRef<Path>, market: &str) -> Result<Self, StoreError> {
        let root = root.as_ref().join(market).join("tape");
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Write `trades` as new parts, one per UTC day they fall in.
    ///
    /// Returns the files written. An empty slice writes nothing rather than an
    /// empty file: a feed with nothing to say should leave no trace.
    pub fn append(&self, trades: &[OptionTrade]) -> Result<Vec<PathBuf>, StoreError> {
        if trades.is_empty() {
            return Ok(Vec::new());
        }
        let mut by_day: std::collections::BTreeMap<i64, Vec<OptionTrade>> = std::collections::BTreeMap::new();
        for trade in trades {
            by_day.entry(day_start(trade.timestamp)).or_default().push(trade.clone());
        }

        let mut written = Vec::with_capacity(by_day.len());
        for (day, mut rows) in by_day {
            // Sorted on the way in so that a reader gets time order without
            // sorting, and so a row group's min/max statistics are tight enough
            // for a range scan to skip it.
            rows.sort_by_key(|t| t.timestamp);
            let dir = self.root.join(format!("date={}", utc_date(day)));
            let path = dir.join(format!("part-{:06}.parquet", next_part(&dir)?));
            write_tape(&path, &rows)?;
            written.push(path);
        }
        Ok(written)
    }

    /// Every print in `[from, to)`, in time order, de-duplicated by id.
    ///
    /// The bounds are milliseconds; `from` is inclusive and `to` exclusive, the
    /// same convention the engines use for a bar.
    pub fn range(&self, from: i64, to: i64) -> Result<Vec<OptionTrade>, StoreError> {
        let mut out = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for day in self.day_dirs()? {
            let Some(start) = day_of_dir(&day) else { continue };
            // A day's file can only hold prints in [start, start + DAY_MS).
            if start >= to || start + DAY_MS <= from {
                continue;
            }
            for part in parts_of(&day)? {
                for trade in read_tape(&part)? {
                    if trade.timestamp < from || trade.timestamp >= to {
                        continue;
                    }
                    // First writer of an id wins. A backfill that re-delivers a
                    // print must not turn into two prints, or every premium
                    // total downstream is wrong.
                    if seen.insert(trade.id.clone()) {
                        out.push(trade);
                    }
                }
            }
        }
        out.sort_by_key(|t| t.timestamp);
        Ok(out)
    }

    /// Every print in the store.
    pub fn all(&self) -> Result<Vec<OptionTrade>, StoreError> {
        self.range(i64::MIN, i64::MAX)
    }

    fn day_dirs(&self) -> Result<Vec<PathBuf>, StoreError> {
        let mut days: Vec<PathBuf> = std::fs::read_dir(&self.root)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() && day_of_dir(path).is_some())
            .collect();
        days.sort();
        Ok(days)
    }
}

fn parts_of(dir: &Path) -> Result<Vec<PathBuf>, StoreError> {
    let mut parts: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "parquet"))
        .collect();
    parts.sort();
    Ok(parts)
}

fn next_part(dir: &Path) -> Result<usize, StoreError> {
    if !dir.exists() {
        return Ok(0);
    }
    Ok(parts_of(dir)?.len())
}

/// Midnight UTC of the day containing `ms`.
fn day_start(ms: i64) -> i64 {
    ms.div_euclid(DAY_MS) * DAY_MS
}

/// `YYYY-MM-DD` for a UTC instant.
///
/// Howard Hinnant's civil-from-days, which is exact for every representable
/// date and needs no calendar table. Written out rather than pulled from a date
/// library because this is the only calendar arithmetic the store does.
fn utc_date(ms: i64) -> String {
    let days = ms.div_euclid(DAY_MS);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

/// Midnight of the day a `date=YYYY-MM-DD` directory holds, or `None` if the
/// directory is not one of ours.
fn day_of_dir(path: &Path) -> Option<i64> {
    let name = path.file_name()?.to_str()?.strip_prefix("date=")?;
    let mut parts = name.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(days_from_civil(year, month, day) * DAY_MS)
}

/// Inverse of the civil-from-days above.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip_through_the_partition_name() {
        for ms in [0_i64, 1_788_757_760_000, -86_400_000, 951_782_400_000, 4_102_444_800_000] {
            let date = utc_date(ms);
            let dir = PathBuf::from(format!("date={date}"));
            assert_eq!(day_of_dir(&dir), Some(day_start(ms)), "{date} did not round trip");
        }
    }

    #[test]
    fn dates_match_known_instants() {
        assert_eq!(utc_date(0), "1970-01-01");
        // 2000-02-29: the leap day the naive rule gets wrong.
        assert_eq!(utc_date(951_782_400_000), "2000-02-29");
        assert_eq!(utc_date(-1), "1969-12-31");
    }

    #[test]
    fn a_directory_that_is_not_ours_is_ignored() {
        assert_eq!(day_of_dir(Path::new("notes")), None);
        assert_eq!(day_of_dir(Path::new("date=2026-09")), None);
        assert_eq!(day_of_dir(Path::new("date=2026-09-12-13")), None);
    }
}
