//! Build a sealed copy of the bar store that physically ends at a cutoff.
//!
//! WHY THIS EXISTS. An agent asked to design a method on a design window and
//! keep away from a hold-out window can read the hold-out anyway - the parquet
//! is right there, and nothing about "please don't" is enforceable. A design
//! agent that has seen the hold-out produces a number that means nothing, and
//! the failure is invisible afterwards: the receipt shows the window the agent
//! chose to print, not the bars it read while thinking.
//!
//! So the hold-out is removed from the data the agent is given. Every bar file
//! is copied with its bars after the cutoff dropped. An agent pointed at the
//! sealed root cannot read the hold-out because it is not there.
//!
//! EVERY instrument is truncated, not just the one under study. XAUDUKA is
//! Dukascopy gold over the same calendar as XAUUSD, so leaving it whole would
//! hand over the hold-out under another name; XAGDUKA and EURDUKA are
//! different instruments but move with gold closely enough that the argument
//! would have to be made rather than assumed. Truncating all of them needs no
//! argument.
//!
//! The news calendar is NOT truncated, and that is deliberate: an economic
//! calendar is published ahead of time, the live desk has next year's events
//! today, and a method that knows when news is scheduled knows nothing the desk
//! does not already know. It is copied whole.
//!
//! Usage:
//!   seal-store --from=data --to=data-sealed --cutoff=2025-09-23
//!
//! `--cutoff` is a UTC date and is EXCLUSIVE: a cutoff of 2025-09-23 keeps the
//! last bar before 2025-09-23 00:00:00Z and drops that bar and everything after.

use std::path::{Path, PathBuf};

fn main() {
    if let Err(e) = run() {
        eprintln!("seal-store: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut from = PathBuf::from("data");
    let mut to = PathBuf::from("data-sealed");
    let mut cutoff: Option<String> = None;

    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--from=") {
            from = PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--to=") {
            to = PathBuf::from(v);
        } else if let Some(v) = arg.strip_prefix("--cutoff=") {
            cutoff = Some(v.to_string());
        } else {
            return Err(format!("unknown argument `{arg}`"));
        }
    }

    let cutoff = cutoff.ok_or("--cutoff=YYYY-MM-DD is required")?;
    let cutoff_ms = date_to_ms(&cutoff)?;

    let bars_in = from.join("bars");
    let bars_out = to.join("bars");
    std::fs::create_dir_all(&bars_out).map_err(|e| format!("{}: {e}", bars_out.display()))?;

    println!("sealing {} -> {}", bars_in.display(), bars_out.display());
    println!("cutoff  {cutoff} 00:00:00Z ({cutoff_ms} ms), exclusive\n");

    let mut files: Vec<PathBuf> = std::fs::read_dir(&bars_in)
        .map_err(|e| format!("{}: {e}", bars_in.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "parquet"))
        .collect();
    files.sort();

    if files.is_empty() {
        return Err(format!("no parquet files under {}", bars_in.display()));
    }

    println!(
        "{:<22} {:>9} {:>9}  {:<19} {:<19}",
        "file", "kept", "dropped", "first kept", "last kept"
    );

    let mut total_kept = 0usize;
    let mut total_dropped = 0usize;

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let bars = fd_store::read_bars(path).map_err(|e| format!("{name}: {e}"))?;
        let before = bars.len();
        let kept: Vec<_> = bars.into_iter().filter(|b| b.time < cutoff_ms).collect();
        let dropped = before - kept.len();

        let first = kept.first().map(|b| ms_to_utc(b.time)).unwrap_or_else(|| "-".into());
        let last = kept.last().map(|b| ms_to_utc(b.time)).unwrap_or_else(|| "-".into());

        // A file with nothing left is still written, empty, rather than left
        // out: a missing file reads as "this instrument was never here", and
        // the sealed root should say "this instrument ends before the cutoff".
        fd_store::write_bars(&bars_out.join(&name), &kept).map_err(|e| format!("{name}: {e}"))?;

        println!("{name:<22} {:>9} {:>9}  {first:<19} {last:<19}", kept.len(), dropped);
        total_kept += kept.len();
        total_dropped += dropped;
    }

    // The news calendar goes across whole - see the header.
    let news_in = from.join("news");
    if news_in.is_dir() {
        let news_out = to.join("news");
        std::fs::create_dir_all(&news_out).map_err(|e| format!("{}: {e}", news_out.display()))?;
        let mut copied = 0usize;
        for entry in std::fs::read_dir(&news_in).map_err(|e| format!("{}: {e}", news_in.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_file() {
                let name = entry.file_name();
                std::fs::copy(entry.path(), news_out.join(&name))
                    .map_err(|e| format!("{}: {e}", name.to_string_lossy()))?;
                copied += 1;
            }
        }
        println!("\nnews calendar copied whole: {copied} files (published ahead, not a leak)");
    }

    println!("\n{} files, {total_kept} bars kept, {total_dropped} bars sealed away", files.len());
    verify(&bars_out, cutoff_ms)?;
    Ok(())
}

/// Read every written file back and refuse to report success if any bar at or
/// after the cutoff survived. The whole point of this tool is a guarantee, and
/// a guarantee that is not checked is a claim.
fn verify(bars_out: &Path, cutoff_ms: i64) -> Result<(), String> {
    let mut checked = 0usize;
    let mut worst: Option<(String, i64)> = None;
    for entry in std::fs::read_dir(bars_out).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|x| x == "parquet") {
            let bars = fd_store::read_bars(&path).map_err(|e| e.to_string())?;
            checked += 1;
            if let Some(b) = bars.iter().find(|b| b.time >= cutoff_ms) {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                worst = Some((name, b.time));
                break;
            }
        }
    }
    match worst {
        Some((name, ts)) => Err(format!(
            "VERIFY FAILED: {name} still holds a bar at {} (>= cutoff). The sealed root is not sealed.",
            ms_to_utc(ts)
        )),
        None => {
            println!("verified: {checked} files re-read, no bar at or after the cutoff survives");
            Ok(())
        }
    }
}

fn date_to_ms(date: &str) -> Result<i64, String> {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("cutoff `{date}` is not YYYY-MM-DD"));
    }
    let y: i64 = parts[0].parse().map_err(|_| format!("bad year in `{date}`"))?;
    let m: i64 = parts[1].parse().map_err(|_| format!("bad month in `{date}`"))?;
    let d: i64 = parts[2].parse().map_err(|_| format!("bad day in `{date}`"))?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(format!("cutoff `{date}` is not a date"));
    }
    Ok(days_from_civil(y, m, d) * 86_400_000)
}

/// Howard Hinnant's days_from_civil: civil date to days since 1970-01-01.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn ms_to_utc(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let (h, min) = (rem / 3_600_000, (rem % 3_600_000) / 60_000);
    format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}")
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutoff_parses_to_midnight_utc() {
        // 2025-09-23T00:00:00Z is 1758585600 seconds since the epoch.
        assert_eq!(date_to_ms("2025-09-23").unwrap(), 1_758_585_600_000);
        assert_eq!(date_to_ms("1970-01-01").unwrap(), 0);
    }

    #[test]
    fn a_bad_cutoff_is_refused_rather_than_guessed() {
        assert!(date_to_ms("2025-09").is_err());
        assert!(date_to_ms("2025-13-01").is_err());
        assert!(date_to_ms("not-a-date").is_err());
    }

    #[test]
    fn the_round_trip_holds_on_the_cutoff_itself() {
        let ms = date_to_ms("2025-09-23").unwrap();
        assert_eq!(ms_to_utc(ms), "2025-09-23 00:00");
        // One millisecond earlier is still the previous day, so the cutoff
        // being exclusive keeps 22 September and drops 23 September.
        assert_eq!(ms_to_utc(ms - 1), "2025-09-22 23:59");
    }
}
