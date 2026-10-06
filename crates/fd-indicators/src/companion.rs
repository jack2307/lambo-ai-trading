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

/// The primary's close divided by the companion's, and that ratio's
/// **z-score** against its own trailing `period` bars, as of each primary bar.
///
/// Returns `(z, ratio)`, each the length of `primary`. A value is `NaN` when
///
/// * the companion carries no bar stamped exactly this primary bar's time, or
///   either close is not strictly positive (a ratio of a non-price is not a
///   ratio);
/// * any of the `period` ratios ending at this bar is itself `NaN` — the
///   window is required to be COMPLETE rather than computed over whatever
///   survived, because a mean over a window with holes is not a mean over that
///   window; or
/// * the window's standard deviation is zero, so the z-score has no scale.
///
/// # Why a LEVEL may span a break when a CHANGE may not
///
/// [`aligned_change`] refuses a change measured across a weekend or a daily
/// break, because a thirty-minute move and a sixty-five-hour move are
/// different quantities. A z-score is not a change: it is where *this* bar's
/// level sits in the distribution of the last `period` levels, and that
/// distribution is no less valid for having a weekend inside it. So the window
/// here is `period` **consecutive bars of the primary series** and is not
/// required to be contiguous in wall-clock time. The stricter rule would make
/// a 480-bar lookback unreachable on any feed with a weekend, which is every
/// feed in this store.
///
/// # Logs, so that the two legs are one rule
///
/// The z-score of a ratio and the z-score of its reciprocal must be the same
/// number with the opposite sign, or reading the mechanism from the gold side
/// and from the silver side would be two different rules rather than one rule
/// read from either end. That identity holds in logs and fails in levels, so
/// the statistic is computed on `ln(primary / companion)`.
///
/// # Causality
///
/// `z[i]` reads `lr[i + 1 - period ..= i]` and nothing later, so truncating
/// either series at an instant leaves every value at or before it unchanged.
/// The tests assert both directions, because a two-series method has two ways
/// to read the future.
#[must_use]
pub fn ratio_zscore(primary: &[Bar], companion: &Companion, period: usize) -> (Vec<f64>, Vec<f64>) {
    let n = primary.len();
    let mut z = vec![f64::NAN; n];
    let mut ratio = vec![f64::NAN; n];
    // Fewer than two bars has no sample standard deviation, so there is no
    // z-score to report — not a zero one.
    if companion.bars.is_empty() || period < 2 || n == 0 {
        return (z, ratio);
    }
    let mut lr = vec![f64::NAN; n];
    let mut cursor = 0usize;
    for (i, bar) in primary.iter().enumerate() {
        while cursor < companion.bars.len() && companion.bars[cursor].time < bar.time {
            cursor += 1;
        }
        if cursor >= companion.bars.len() || companion.bars[cursor].time != bar.time {
            continue; // missing, not the previous bar and not one
        }
        let comp_close = companion.bars[cursor].close;
        if !(comp_close > 0.0) || !(bar.close > 0.0) {
            continue;
        }
        let r = bar.close / comp_close;
        ratio[i] = r;
        lr[i] = r.ln();
    }
    for i in (period - 1)..n {
        let window = &lr[i + 1 - period..=i];
        if window.iter().any(|v| !v.is_finite()) {
            continue;
        }
        let mean = window.iter().sum::<f64>() / period as f64;
        let var = window.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (period as f64 - 1.0);
        let sd = var.sqrt();
        if !(sd > 0.0) {
            continue;
        }
        z[i] = (lr[i] - mean) / sd;
    }
    (z, ratio)
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

    /* ------------------------------------------------- ratio_zscore */

    /// The identity the whole two-leg reading rests on: reading the ratio from
    /// the other side is the same statistic negated, so gold-leg and
    /// silver-leg runs measure one rule and not two.
    #[test]
    fn the_reciprocal_ratio_has_the_negated_z_score() {
        let n = 40;
        let au: Vec<Bar> = (0..n).map(|i| bar(i * M15, 3300.0 + (i as f64 / 3.0).sin() * 40.0)).collect();
        let ag: Vec<Bar> = (0..n).map(|i| bar(i * M15, 38.0 + (i as f64 / 5.0).cos() * 1.5)).collect();
        let comp_ag = Companion::new("XAG", "15m", ag.clone());
        let comp_au = Companion::new("XAU", "15m", au.clone());
        let (z_gold, r_gold) = ratio_zscore(&au, &comp_ag, 10);
        let (z_silver, r_silver) = ratio_zscore(&ag, &comp_au, 10);
        for i in 0..n as usize {
            if z_gold[i].is_finite() {
                assert!(
                    (z_gold[i] + z_silver[i]).abs() < 1e-9,
                    "z at {i}: gold leg {} vs silver leg {} are not negatives",
                    z_gold[i],
                    z_silver[i]
                );
                assert!((r_gold[i] * r_silver[i] - 1.0).abs() < 1e-9, "the ratios are not reciprocals at {i}");
            }
        }
        assert!(z_gold.iter().any(|v| v.is_finite()), "the fixture produced no z-score at all");
    }

    #[test]
    fn a_window_with_one_missing_companion_bar_is_refused_whole() {
        // Bar 5 is absent from the companion, so every window containing it —
        // bars 5 through 5 + period - 1 — has no z-score.
        let n = 30usize;
        let mut comp_bars: Vec<Bar> = (0..n as i64).map(|i| bar(i * M15, 38.0 + i as f64 * 0.1)).collect();
        comp_bars.remove(5);
        let comp = Companion::new("XAG", "15m", comp_bars);
        let primary: Vec<Bar> = (0..n as i64).map(|i| bar(i * M15, 3300.0 + (i as f64).sin() * 20.0)).collect();
        let period = 4usize;
        let (z, ratio) = ratio_zscore(&primary, &comp, period);
        assert!(ratio[5].is_nan(), "bar 5 has no companion bar, so it has no ratio");
        for i in 5..5 + period {
            assert!(z[i].is_nan(), "the window ending at {i} contains the hole at 5 and must be refused");
        }
        assert!(z[5 + period].is_finite(), "the first window clear of the hole is measurable again");
    }

    #[test]
    fn a_level_z_score_spans_a_break_that_a_change_would_refuse() {
        // Same shape as `a_change_across_a_break_is_refused_rather_than_measured`,
        // but a z-score of a LEVEL is defined over it: see the doc comment.
        let times: Vec<i64> = (0..12).map(|i| if i < 6 { i * M15 } else { i * M15 + 40 * M15 }).collect();
        let comp = Companion::new("XAG", "15m", times.iter().enumerate().map(|(i, t)| bar(*t, 38.0 + i as f64 * 0.2)).collect());
        let primary: Vec<Bar> =
            times.iter().enumerate().map(|(i, t)| bar(*t, 3300.0 + (i as f64 * 1.7).sin() * 15.0)).collect();
        let (z, _) = ratio_zscore(&primary, &comp, 5);
        assert!(z[7].is_finite(), "a window straddling the gap still has a mean and a spread");
    }

    #[test]
    fn a_flat_ratio_has_no_z_score_rather_than_a_zero_one() {
        // Both legs constant: the window's spread is zero, so there is no
        // scale to divide by and the honest answer is NaN.
        let primary: Vec<Bar> = (0..20).map(|i| bar(i * M15, 3300.0)).collect();
        let comp = Companion::new("XAG", "15m", (0..20).map(|i| bar(i * M15, 38.0)).collect());
        let (z, ratio) = ratio_zscore(&primary, &comp, 5);
        assert!(ratio.iter().all(|v| (*v - 3300.0 / 38.0).abs() < 1e-9));
        assert!(z.iter().all(|v| v.is_nan()), "a zero-spread window has no z-score");
    }

    #[test]
    fn nothing_installed_or_too_short_a_period_is_nan_throughout() {
        let primary: Vec<Bar> = (0..10).map(|i| bar(i * M15, 3300.0)).collect();
        let empty = Companion::new("XAG", "15m", Vec::new());
        let (z, ratio) = ratio_zscore(&primary, &empty, 5);
        assert!(z.iter().all(|v| v.is_nan()) && ratio.iter().all(|v| v.is_nan()));
        let comp = Companion::new("XAG", "15m", (0..10).map(|i| bar(i * M15, 38.0)).collect());
        let (z1, _) = ratio_zscore(&primary, &comp, 1);
        assert!(z1.iter().all(|v| v.is_nan()), "a one-bar window has no sample standard deviation");
    }

    /// The causality property, both ways round, for the z-score as well.
    #[test]
    fn truncating_either_series_leaves_every_earlier_z_score_unchanged() {
        let n = 80usize;
        let au: Vec<Bar> = (0..n as i64).map(|i| bar(i * M15, 3300.0 + (i as f64 / 7.0).sin() * 50.0)).collect();
        let ag: Vec<Bar> = (0..n as i64).map(|i| bar(i * M15, 38.0 + (i as f64 / 4.0).cos() * 2.0)).collect();
        let full = Companion::new("XAG", "15m", ag.clone());
        let (z, ratio) = ratio_zscore(&au, &full, 12);
        let cut = 50usize;

        let (z2, r2) = ratio_zscore(&au[..cut], &full, 12);
        for i in 0..cut {
            assert!(fd_core::parity_eq(z[i], z2[i]), "z at {i} moved when the primary was truncated");
            assert!(fd_core::parity_eq(ratio[i], r2[i]), "ratio at {i} moved when the primary was truncated");
        }

        let short = Companion::new("XAG", "15m", ag[..cut].to_vec());
        let (z3, r3) = ratio_zscore(&au[..cut], &short, 12);
        for i in 0..cut {
            assert!(fd_core::parity_eq(z[i], z3[i]), "z at {i} moved when later companion bars arrived");
            assert!(fd_core::parity_eq(ratio[i], r3[i]), "ratio at {i} moved when later companion bars arrived");
        }
        assert!(z.iter().take(cut).any(|v| v.is_finite()), "the fixture produced no z-score to compare");
    }
}
