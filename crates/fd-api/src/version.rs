//! What this binary was built from, so a deploy can prove what is running.
//!
//! The desk copies `target/release/fd-api.exe` to a server that separately
//! runs `git pull`. Source HEAD and running binary can therefore differ with
//! nothing anywhere disagreeing: the readiness probe polls
//! `/api/paper/status`, which a binary from any earlier build answers
//! perfectly, and `ls` reports an mtime that a copy has already refreshed.
//! The full account is in `docs/decisions/2026-09-17-deploy-staleness.md`.
//!
//! The values here come from `build.rs` at COMPILE time and describe the
//! source the code came from — never the checkout the process is sitting in.
//! A runtime `git rev-parse` would report the latter, which on the server is
//! the one thing already known and never the thing in doubt, and would answer
//! "up to date" in exactly the case this endpoint exists to catch.

use axum::Json;
use serde_json::json;

/// The commit this binary was compiled from, or `"unknown"`.
pub const GIT_HASH: &str = env!("FD_GIT_HASH");

/// Whether the tree had uncommitted changes at compile time.
///
/// `"true"` means the hash is true and the CONTENT is not — a binary built
/// from edits on top of a commit. That is a state worth naming rather than
/// hiding, because it is the one most likely to be running during an incident.
pub const GIT_DIRTY: &str = env!("FD_GIT_DIRTY");

/// Compile time, epoch milliseconds UTC. The unit is in the name.
pub const BUILT_AT_MS: &str = env!("FD_BUILT_AT_MS");

/// One line for a console, and the same facts the endpoint serves.
pub fn summary() -> String {
    let short = if GIT_HASH.len() >= 7 { &GIT_HASH[..7] } else { GIT_HASH };
    let dirty = if GIT_DIRTY == "true" { " +uncommitted-changes" } else { "" };
    format!("built from {short}{dirty}, {} UTC", iso_utc(built_at_ms()))
}

pub fn built_at_ms() -> i64 {
    BUILT_AT_MS.parse().unwrap_or(0)
}

/// `YYYY-MM-DDTHH:MM:SSZ` from epoch milliseconds, without a date crate.
///
/// Civil-from-days after Howard Hinnant's algorithm. Written out rather than
/// pulled in because this crate has no date dependency and one stamp does not
/// justify one; it is exercised by the tests below.
fn iso_utc(ms: i64) -> String {
    if ms <= 0 {
        return "unknown".into();
    }
    let secs = ms / 1000;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// `GET /api/version`.
pub async fn version() -> Json<serde_json::Value> {
    Json(json!({
        "git_hash": GIT_HASH,
        "git_dirty": GIT_DIRTY == "true",
        "built_at_ms": built_at_ms(),
        "built_at_utc": iso_utc(built_at_ms()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stamp_describes_the_source_and_not_the_checkout() {
        // The point of the endpoint, asserted where someone changing it will
        // see it: these are compile-time constants. If this ever becomes a
        // runtime `git rev-parse`, it reports the directory the process is in
        // and checks nothing, because on the server that is the one fact
        // already known. See docs/decisions/2026-09-17-deploy-staleness.md.
        assert_eq!(GIT_HASH, env!("FD_GIT_HASH"));
        assert!(GIT_DIRTY == "true" || GIT_DIRTY == "false");
    }

    #[test]
    fn a_hash_is_either_a_commit_or_the_word_unknown() {
        // Never an empty string. The deploy compares for EQUALITY, so an empty
        // value would risk matching an empty expectation; `unknown` fails
        // loudly, which is the direction that is safe.
        assert!(!GIT_HASH.is_empty());
        if GIT_HASH != "unknown" {
            assert_eq!(GIT_HASH.len(), 40, "a full sha, not an abbreviation: {GIT_HASH}");
            assert!(GIT_HASH.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn the_build_time_is_a_real_instant_in_utc() {
        // Epoch ms, and the field says so. 1.7e12 is 2023; anything below it is
        // a seconds/milliseconds mix-up, which is the unit error this repo has
        // paid for in three other places.
        let at = built_at_ms();
        assert!(at > 1_700_000_000_000, "built_at_ms looks like seconds, not ms: {at}");
    }

    #[test]
    fn the_calendar_arithmetic_is_right_at_the_awkward_dates() {
        // Leap day, a century that is not a leap year, one that is, and the
        // epoch itself. Written out because this function replaces a date
        // crate and an off-by-one here would be invisible in the one place it
        // is used.
        assert_eq!(iso_utc(1), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(951_782_400_000), "2000-02-29T00:00:00Z");
        assert_eq!(iso_utc(4_107_542_400_000), "2100-03-01T00:00:00Z");
        // Verified against Python's utcfromtimestamp, not against my own
        // arithmetic: this line asserted 14:21:01Z on the first run and the
        // test failed. The code was right and the expectation was wrong, which
        // is the direction a test is for.
        assert_eq!(iso_utc(1_789_664_461_000), "2026-09-17T17:01:01Z");
        assert_eq!(iso_utc(0), "unknown");
        assert_eq!(iso_utc(-5), "unknown");
    }
}
