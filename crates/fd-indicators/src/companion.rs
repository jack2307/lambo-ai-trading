//! A **second instrument's** bars, so an indicator can be a function of two
//! series instead of one.
//!
//! # Why this is a process-wide global, and not a `BarContext` field
//!
//! Every one of the registry's mechanisms reads one price series, and the
//! plumbing says so in three places at once: [`crate::compute_indicators`]
//! takes `(&[Bar], &[IndicatorSpec])` and nothing else,
//! `fd_strategy::registry::BarContext` carries one `bars` slice, and
//! `Strategy::series` names keys that are looked up in the set those bars
//! produced. A second instrument has to enter through one of them.
//!
//! It enters here, for the same reason the scheduled-news calendar enters
//! through `fd_strategy::news`: an [`crate::IndicatorSpec`] is a value parsed
//! from a name and a few floats and carries no IO, and every reader of a set
//! — a sweep cell, a matched null, the Workbench, the paper loop — must see
//! the same second series or the comparison between them is not one. The list
//! is installed **once**, by the binary that owns the data directory
//! (`search`), and read by [`crate::compute_indicators`] through
//! [`bars`]. Nothing here touches a file.
//!
//! Sweeps run cells in parallel, so the series is immutable after
//! [`install`]: a second install of the same series is accepted, a different
//! one is refused rather than silently swapped under a running search. When
//! nothing is installed every companion series is `NaN` at every bar — not
//! zero, not the primary's own value — so a strategy that needs one takes no
//! trades rather than trading on a fabricated number, and [`summary`] says
//! which case holds so a receipt can quote it.
//!
//! # Alignment, which is the whole risk
//!
//! Two bars with the same timestamp are only the same interval if both feeds
//! stamp bars the same way. [`aligned_change`] therefore matches on an
//! **exact timestamp** and never on a nearest or previous bar: a primary bar
//! whose timestamp the companion does not carry is missing, and reads `NaN`.
//! A gap in one series is never filled from the other.
//!
//! The look-ahead this creates if the two clocks differ is the one that is
//! easy to miss. The companion bar stamped `T` closes at `T + interval`, the
//! same instant the primary bar stamped `T` closes, and the engine fills the
//! decision at `T + interval`. So reading it is causal **exactly when the two
//! feeds are on the same grid**, and that is a measurement about the data, not
//! an assumption: `docs/research/designs/2026-09-23-designed-4-two-series.md`
//! records the shift scan that established it for the pairs used there.
//!
//! The lookback is checked as strictly: the change over `period` bars is
//! `NaN` unless the companion's own bar `period` places earlier is exactly
//! `period` intervals older, where the interval is the companion series' modal
//! spacing ([`Companion::interval_ms`]). A change measured across a weekend or
//! a daily break is refused rather than reported.

use std::sync::OnceLock;

use fd_core::types::Bar;

/// One instrument's bars, with the name they came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Companion {
    /// Store symbol, e.g. `XAGDUKA`.
    pub symbol: String,
    /// Bar interval as the store spells it, e.g. `15m`.
    pub interval: String,
    /// Ascending by time, no duplicates.
    pub bars: Vec<Bar>,
}

impl Companion {
    /// Sorts by time and drops duplicate timestamps (keeping the first).
    #[must_use]
    pub fn new(symbol: &str, interval: &str, mut bars: Vec<Bar>) -> Self {
        bars.sort_by_key(|b| b.time);
        bars.dedup_by_key(|b| b.time);
        Self { symbol: symbol.to_string(), interval: interval.to_string(), bars }
    }

    /// The series' modal positive spacing in milliseconds, or `None` when
    /// there are fewer than two bars.
    ///
    /// Modal rather than first-difference or median: a feed with a daily break
    /// and a weekend has three or four distinct spacings, and the one that
    /// defines "one bar later" is the commonest.
    #[must_use]
    pub fn interval_ms(&self) -> Option<i64> {
        if self.bars.len() < 2 {
            return None;
        }
        let mut counts: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
        for pair in self.bars.windows(2) {
            let d = pair[1].time - pair[0].time;
            if d > 0 {
                *counts.entry(d).or_default() += 1;
            }
        }
        // Commonest spacing; the SMALLEST of them when two are equally common,
        // because "one bar later" is the shortest step the feed takes and a tie
        // must not depend on map order.
        counts.into_iter().max_by_key(|(d, n)| (*n, -*d)).map(|(d, _)| d)
    }

    /// Index of the bar stamped exactly `t`, or `None`.
    #[must_use]
    pub fn index_at(&self, t: i64) -> Option<usize> {
        self.bars.binary_search_by_key(&t, |b| b.time).ok()
    }
}

static COMPANION: OnceLock<Companion> = OnceLock::new();

/// Install the companion series for this process. Returns its bar count.
///
/// Installing the same series twice is fine; a different one once one is
/// installed is an error, because sets that already computed read the first.
pub fn install(companion: Companion) -> Result<usize, String> {
    let n = companion.bars.len();
    match COMPANION.set(companion) {
        Ok(()) => Ok(n),
        Err(rejected) => {
            let current = COMPANION.get().expect("set failed, so one is installed");
            if *current == rejected {
                Ok(n)
            } else {
                Err(format!(
                    "companion: {}-{} is already installed ({} bars); refused {}-{} ({} bars)",
                    current.symbol,
                    current.interval,
                    current.bars.len(),
                    rejected.symbol,
                    rejected.interval,
                    rejected.bars.len()
                ))
            }
        }
    }
}

/// The installed companion, or `None` when nothing has been installed.
#[must_use]
pub fn installed() -> Option<&'static Companion> {
    COMPANION.get()
}

/// The installed companion's bars, or an empty slice.
#[must_use]
pub fn bars() -> &'static [Bar] {
    COMPANION.get().map_or(&[], |c| c.bars.as_slice())
}

/// The one-line receipt a binary prints after trying to load one:
/// `companion: XAGDUKA-15m 358,065 bars 2010-06-01 → 2025-09-22 (interval 900s)`
/// or `companion: none loaded — every cmp series is NaN`.
#[must_use]
pub fn summary() -> String {
    match COMPANION.get() {
        None => "companion: none loaded — every cmp series is NaN".to_string(),
        Some(c) if c.bars.is_empty() => {
            format!("companion: {}-{} loaded with NO bars — every cmp series is NaN", c.symbol, c.interval)
        }
        Some(c) => format!(
            "companion: {}-{} {} bars {} → {} (modal interval {}s)",
            c.symbol,
            c.interval,
            c.bars.len(),
            iso_day(c.bars[0].time),
            iso_day(c.bars[c.bars.len() - 1].time),
            c.interval_ms().map_or(0, |ms| ms / 1000),
        ),
    }
}

fn iso_day(ms: i64) -> String {
    let (y, m, d) = fd_core::clock::civil_from_days(ms.div_euclid(86_400_000));
    format!("{y:04}-{m:02}-{d:02}")
}

/// The companion's close-to-close change over `period` of its own bars, and
/// its ATR, **as of each primary bar's timestamp**.
///
/// Returns `(change, atr, close)`, each the length of `primary`. A value is
/// `NaN` when
///
/// * the companion carries no bar stamped exactly this primary bar's time;
/// * the bar `period` places earlier is not exactly `period` intervals older
///   (a change across a break is refused, not reported); or
/// * the ATR is not warm yet at that companion bar.
///
/// This is the arithmetic behind the `cmp` indicator, over an explicit series,
/// so it can be tested without installing anything process-wide.
#[must_use]
pub fn aligned_change(
    primary: &[Bar],
    companion: &Companion,
    period: usize,
    atr_period: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = primary.len();
    let mut change = vec![f64::NAN; n];
    let mut atr_out = vec![f64::NAN; n];
    let mut close_out = vec![f64::NAN; n];
    if companion.bars.is_empty() || period == 0 {
        return (change, atr_out, close_out);
    }
    let Some(step) = companion.interval_ms() else {
        return (change, atr_out, close_out);
    };
    let comp_atr = crate::atr(&companion.bars, atr_period);
    // The companion is ascending and so is the primary, so one forward cursor
    // answers every lookup; a binary search per bar would re-walk the tree for
    // every bar of every parameter cell of every sweep.
    let mut cursor = 0usize;
    let span = (period as i64) * step;
    for (i, bar) in primary.iter().enumerate() {
        while cursor < companion.bars.len() && companion.bars[cursor].time < bar.time {
            cursor += 1;
        }
        if cursor >= companion.bars.len() || companion.bars[cursor].time != bar.time {
            continue; // missing, not zero and not the previous value
        }
        close_out[i] = companion.bars[cursor].close;
        atr_out[i] = comp_atr.get(cursor).copied().unwrap_or(f64::NAN);
        if let Some(back) = cursor.checked_sub(period)
            && companion.bars[cursor].time - companion.bars[back].time == span
        {
            change[i] = companion.bars[cursor].close - companion.bars[back].close;
        }
    }
    (change, atr_out, close_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(t: i64, c: f64) -> Bar {
        Bar { time: t, open: c, high: c + 0.5, low: c - 0.5, close: c, volume: None }
    }

    const M15: i64 = 900_000;

    #[test]
    fn a_timestamp_the_companion_lacks_is_nan_and_never_the_previous_bar() {
        // Bar 2 is missing from the companion; everything else is on the grid,
        // so the modal spacing is unambiguously one interval.
        let mut comp_bars: Vec<Bar> = (0..8).filter(|i| *i != 2).map(|i| bar(i * M15, 10.0 + i as f64)).collect();
        comp_bars.sort_by_key(|b| b.time);
        let comp = Companion::new("X", "15m", comp_bars);
        assert_eq!(comp.interval_ms(), Some(M15));
        let primary: Vec<Bar> = (0..8).map(|i| bar(i * M15, 100.0)).collect();
        let (change, _, close) = aligned_change(&primary, &comp, 1, 2);
        assert_eq!(close[0], 10.0);
        assert_eq!(close[1], 11.0);
        assert!(close[2].is_nan(), "bar 2 is absent from the companion: missing, not 11.0");
        assert_eq!(close[3], 13.0);
        assert_eq!(change[1], 1.0);
        assert!(change[2].is_nan());
        assert!(change[3].is_nan(), "bar 3's predecessor is two intervals back, so the change is refused");
        assert_eq!(change[4], 1.0, "back on the grid, the change is measurable again");
    }

    #[test]
    fn a_change_across_a_break_is_refused_rather_than_measured() {
        // Two sessions an hour apart; the modal spacing is still 15m.
        let times = [0, M15, 2 * M15, 3 * M15, 3 * M15 + 4 * M15, 3 * M15 + 5 * M15];
        let comp = Companion::new("X", "15m", times.iter().enumerate().map(|(i, t)| bar(*t, 10.0 + i as f64)).collect());
        assert_eq!(comp.interval_ms(), Some(M15));
        let primary: Vec<Bar> = times.iter().map(|t| bar(*t, 100.0)).collect();
        let (change, _, _) = aligned_change(&primary, &comp, 1, 2);
        assert_eq!(change[3], 1.0, "inside the first session");
        assert!(change[4].is_nan(), "the bar before it is four intervals back");
        assert_eq!(change[5], 1.0, "inside the second session");
    }

    #[test]
    fn nothing_installed_means_nan_rather_than_zero() {
        // The global is untouched in this test binary unless a test installs
        // one, so this asserts the shape of the empty case.
        let empty = Companion::new("X", "15m", Vec::new());
        let primary: Vec<Bar> = (0..3).map(|i| bar(i * M15, 100.0)).collect();
        let (change, atr, close) = aligned_change(&primary, &empty, 1, 2);
        assert!(change.iter().all(|v| v.is_nan()));
        assert!(atr.iter().all(|v| v.is_nan()));
        assert!(close.iter().all(|v| v.is_nan()));
        assert!(summary().contains("companion:"));
    }

    /// The causality property, in the second series as well as the first.
    #[test]
    fn a_truncated_primary_is_a_prefix_and_so_is_a_truncated_companion() {
        let comp_bars: Vec<Bar> = (0..60).map(|i| bar(i * M15, 20.0 + (i as f64 / 5.0).sin() * 2.0)).collect();
        let primary: Vec<Bar> = (0..60).map(|i| bar(i * M15, 1000.0 + i as f64)).collect();
        let full = Companion::new("X", "15m", comp_bars.clone());
        let (change, atr, close) = aligned_change(&primary, &full, 3, 14);

        // Truncating the PRIMARY must not change any earlier value.
        let cut = 40;
        let (c2, a2, k2) = aligned_change(&primary[..cut], &full, 3, 14);
        for i in 0..cut {
            assert!(fd_core::parity_eq(change[i], c2[i]), "change at {i}");
            assert!(fd_core::parity_eq(atr[i], a2[i]), "atr at {i}");
            assert!(fd_core::parity_eq(close[i], k2[i]), "close at {i}");
        }

        // Truncating the COMPANION at the same wall-clock instant must not
        // change any value at or before it either: that is the second way a
        // two-series method can read the future.
        let short = Companion::new("X", "15m", comp_bars[..cut].to_vec());
        let (c3, a3, k3) = aligned_change(&primary[..cut], &short, 3, 14);
        for i in 0..cut {
            assert!(fd_core::parity_eq(change[i], c3[i]), "change at {i} moved when later companion bars arrived");
            assert!(fd_core::parity_eq(atr[i], a3[i]), "atr at {i} moved when later companion bars arrived");
            assert!(fd_core::parity_eq(close[i], k3[i]), "close at {i} moved when later companion bars arrived");
        }
    }

    #[test]
    fn the_modal_interval_ignores_breaks() {
        let mut times: Vec<i64> = (0..40).map(|i| i * M15).collect();
        times.push(40 * M15 + 3 * M15); // one long gap
        let comp = Companion::new("X", "15m", times.iter().map(|t| bar(*t, 10.0)).collect());
        assert_eq!(comp.interval_ms(), Some(M15));
    }
}
