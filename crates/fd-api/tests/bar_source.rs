//! Which stored series answers a timeframe request.
//!
//! This has one rule and the rule is easy to get backwards. Preferring the
//! *finest* stored series sounds right and is not: on this store the minute
//! series covers a week while the 15-minute series covers two years, so
//! "finest wins" answers a two-year request with seven days of bars and says
//! nothing about it.

use std::path::Path;

use fd_api::AppState;
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::write_bars;

/// Midnight UTC, so that every bucket size divides the span evenly.
///
/// Starting anywhere else leaves a partial bucket at each end and turns a clean
/// assertion into an off-by-one argument about boundaries rather than about the
/// rule under test.
const START: i64 = 1_788_000_000_000 / 86_400_000 * 86_400_000;

/// `count` bars of `step`, starting at [`START`].
fn series(step: i64, count: usize) -> Vec<Bar> {
    (0..count).map(|i| Bar::flat(START + i as i64 * step, 100.0 + i as f64)).collect()
}

fn config() -> Config {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config");
    Config::load(dir).expect("the workspace config")
}

#[test]
fn a_long_coarse_series_beats_a_short_fine_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    let bars = dir.path().join("bars");

    // A week of minutes and two years of quarter-hours, which is the shape the
    // real store ended up in after a migration plus a backfill.
    write_bars(&bars.join("BTCUSDT-1m.parquet"), &series(60_000, 10_080)).expect("write");
    write_bars(&bars.join("BTCUSDT-15m.parquet"), &series(900_000, 70_080)).expect("write");

    let state = AppState::new(config(), dir.path().to_path_buf());
    let fifteen = state.bars("btc", "15m").expect("15m");
    assert_eq!(fifteen.bars.len(), 70_080, "an exact-timeframe file must be used as it is");

    let hourly = state.bars("btc", "1h").expect("1h");
    assert_eq!(hourly.bars.len(), 17_520, "an hour must be built from the series that covers the span");
}

#[test]
fn a_timeframe_finer_than_anything_stored_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    write_bars(&dir.path().join("bars").join("BTCUSDT-15m.parquet"), &series(900_000, 100))
        .expect("write");

    let state = AppState::new(config(), dir.path().to_path_buf());
    // Inventing minutes out of quarter-hours would be fabrication, so this is
    // an error rather than an interpolation.
    assert!(state.bars("btc", "1m").is_err());
}

#[test]
fn an_empty_store_says_so_rather_than_returning_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let state = AppState::new(config(), dir.path().to_path_buf());
    assert!(state.bars("btc", "15m").is_err());
}
