//! The GC → spot basis, as a ROLLING estimate instead of a constant.
//!
//! The gold option tape is COMEX GC; the only tradable gold bars on this disk
//! are Vantage spot, which trades tens of dollars below it. Every option level
//! therefore arrives on the wrong price axis and has to be moved before a rule
//! that triggers on a DISTANCE from price can be read at all.
//!
//! A constant offset was tried first (`--basis-offset=`, 2026-10-09) and the
//! measurement it produced is the reason this module exists: a **4.52 USD**
//! change in that constant — a change that sits inside the p10–p90 width of
//! the basis as already measured — moved `level-reversion` from `PF_r` 0.1735
//! to 1.3883, i.e. from a clear miss to clearing all three gate legs. The sign
//! of the result was a property of the constant, not of the market, so the
//! verdict was NOT DECIDED.
//!
//! The basis is not a constant. Measured on 6,718 overlapping minutes: mean
//! +43.70, sd 1.91, p10 41.26, p90 45.78, and quartile means drifting
//! monotonically 45.66 → 44.19 → 43.59 → 41.35. At `entryAtr = 0.35` the entry
//! tolerance is 1.58 USD against a residual sd of 1.91 USD — **the basis error
//! is larger than the tolerance the rule triggers on.**
//!
//! # The two properties this module exists to hold
//!
//! * **It may read the past only.** The window is `(t - W, t)` with the upper
//!   bound OPEN, and each observation is stamped with the instant it became
//!   knowable — a bar's close is knowable at `bar.time + interval`, not at
//!   `bar.time`. The arithmetic consequence, which is what makes it checkable:
//!   bar *i* reads the frame at `t <= bars[i].time`, and an observation *j*
//!   enters only if `bars[j].time + interval < t <= bars[i].time`, hence
//!   `j < i`. **A bar never contributes to the basis it is traded on.**
//!   [`tests::cutting_either_series_leaves_every_earlier_basis_untouched`] is
//!   the assertion, in the shape `companion`'s causality test uses: truncating
//!   either series must not move a value at or before the cut.
//! * **Missing is missing.** Where the tape and the bars do not overlap, or
//!   where the trailing window holds fewer than `min_obs` observations, the
//!   basis is `None` — never `0.0`, and never the last value carried forward.
//!   A refused frame keeps its time and its non-price fields and loses its
//!   levels, which is the engine-level expression of "do not enter at a level
//!   you cannot place". See [`apply`].

use fd_core::types::Bar;

use crate::context::{Frame, OptionsTimeline};

/// How the rolling estimate is taken. Declared before the run, never swept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RollingBasis {
    /// Trailing window, in milliseconds. The window is `(t - window_ms, t)`.
    pub window_ms: i64,
    /// Fewest observations that may produce an estimate. Below it the answer
    /// is `None`, which is a refusal and not a zero.
    pub min_obs: usize,
}

/// One basis observation, and the instant it became knowable.
///
/// `known_at` is deliberately not the bar's timestamp: `Bar::time` is the
/// bar's OPEN, so its close — the thing the observation is made of — is not
/// knowable until one interval later. Stamping it at `bar.time` would let a
/// bar's own close into the estimate used to trade that bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Observation {
    pub known_at: i64,
    pub value: f64,
}

/// The modal spacing of `bars`, in milliseconds.
///
/// Modal rather than first-difference so a session break does not redefine the
/// interval, the same reading `companion::interval_ms` takes.
#[must_use]
pub fn interval_ms(bars: &[Bar]) -> Option<i64> {
    if bars.len() < 2 {
        return None;
    }
    let mut gaps: Vec<i64> = bars.windows(2).map(|w| w[1].time - w[0].time).filter(|g| *g > 0).collect();
    if gaps.is_empty() {
        return None;
    }
    gaps.sort_unstable();
    let (mut best, mut best_run, mut i) = (gaps[0], 0usize, 0usize);
    while i < gaps.len() {
        let mut j = i;
        while j < gaps.len() && gaps[j] == gaps[i] {
            j += 1;
        }
        if j - i > best_run {
            best_run = j - i;
            best = gaps[i];
        }
        i = j;
    }
    Some(best)
}

/// Pair every tradable bar with the GC reference series, giving
/// `GC_close - spot_close` stamped at the instant that close was knowable.
///
/// # Why the reference is a BAR SERIES and not `Frame::spot`
///
/// `Frame::spot` is the `underlying_price` of the tape's **last print**
/// (`fd-engine/engine.rs:191-195`), whichever expiry that print belonged to.
/// Measured on the 55,131 prints in the store: **26 distinct expiry symbols**,
/// whose mean `underlying_price` runs from **4,146.00** to **4,385.32** —
/// 239 USD apart, because they are options on DIFFERENT GC contract months
/// (U6 = 2026-09, V6 = 10, X6 = 11, Z6 = 12). Within a single minute the
/// dispersion is mean **+10.66** USD, p90 **+35.10**, max **+58.10**, and it is
/// non-zero in **81.1%** of the 8,316 minutes carrying more than one print.
///
/// So `Frame::spot` is not a price series; it is the contract-month dispersion
/// sampled by whichever print landed last. Using it as the GC axis produced a
/// raw basis of mean +16.48 **sd 16.40**, range −26.51 to +89.01 — a 115 USD
/// spread where the real basis moves 10 USD.
///
/// `data/bars/GC-1m.parquet` is ONE series: 18,707 of 18,709 bars have
/// `open == high == low == close`, so its close is the per-minute reference
/// price and nothing else. Paired with `XAUUSD-15m` it gives mean **+42.16**
/// sd **2.31** over the whole 785-bar overlap — which is the quantity this
/// module is for.
///
/// A bar with no reference bar inside it produces **no observation** — there is
/// nothing to subtract, and a zero there would be a fabricated basis of zero
/// dollars. Ascending in `known_at`, because bars are.
#[must_use]
pub fn observations(reference: &[Bar], bars: &[Bar], interval_ms: i64) -> Vec<Observation> {
    let mut out = Vec::new();
    if reference.is_empty() || interval_ms <= 0 {
        return out;
    }
    // Both series ascend, so one forward cursor answers every lookup. The
    // reference bar taken is the last one that STARTED inside this bar, so the
    // two closes are as near in time as the two grids allow and neither reads
    // past the bar's own close.
    let mut cursor: usize = 0;
    for bar in bars {
        let deadline = bar.time + interval_ms;
        while cursor + 1 < reference.len() && reference[cursor + 1].time < deadline {
            cursor += 1;
        }
        let r = &reference[cursor];
        if r.time < bar.time || r.time >= deadline {
            continue; // no reference price inside this bar: missing, not zero
        }
        if !r.close.is_finite() || !bar.close.is_finite() {
            continue;
        }
        out.push(Observation { known_at: deadline, value: r.close - bar.close });
    }
    out
}

/// The rolling estimate at `t`, from observations knowable strictly before it.
///
/// **Median**, not mean, and that is a declared choice rather than a tuned one:
/// the degenerate rows the constant-offset run produced (`Lbar` 3.50 R and
/// 11.02 R against 0.47–0.72 R at the neighbouring offsets) came from a handful
/// of entries where `risk = entry - (cluster.low - 0.3 x ATR)` lands near zero,
/// and a median is not dragged by a few such points. For an even count it is
/// the mean of the two middle values.
#[must_use]
pub fn estimate(obs: &[Observation], t: i64, cfg: &RollingBasis) -> Option<f64> {
    if cfg.window_ms <= 0 || cfg.min_obs == 0 {
        return None;
    }
    let lo = t - cfg.window_ms;
    // `(lo, t)`: lower bound open, upper bound OPEN. An observation knowable
    // exactly at `t` is not in the past of `t`.
    let start = obs.partition_point(|o| o.known_at <= lo);
    let end = obs.partition_point(|o| o.known_at < t);
    if end <= start || end - start < cfg.min_obs {
        return None;
    }
    let mut window: Vec<f64> = obs[start..end].iter().map(|o| o.value).collect();
    window.sort_unstable_by(f64::total_cmp);
    let n = window.len();
    Some(if n % 2 == 1 { window[n / 2] } else { (window[n / 2 - 1] + window[n / 2]) / 2.0 })
}

/// The estimate at every frame, `None` where there is not enough past.
#[must_use]
pub fn series(frames: &[Frame], reference: &[Bar], bars: &[Bar], cfg: &RollingBasis) -> Vec<Option<f64>> {
    let Some(step) = interval_ms(bars) else {
        return vec![None; frames.len()];
    };
    let obs = observations(reference, bars, step);
    frames.iter().map(|f| estimate(&obs, f.t, cfg)).collect()
}

/// Move every price in a frame DOWN by `offset`, onto the bars' axis.
///
/// The single place the field list lives, so the constant path and the rolling
/// path shift exactly the same things and a comparison between them is a
/// comparison of the estimate rather than of two field lists.
///
/// Everything price-valued in a frame comes off the option tape's strike grid:
/// the cluster bounds and centre, and `max_pain` / `poc` / `w_sup` / `w_res` /
/// `call_be` / `put_be` / `spot` in every expiration context. `score`, `dte`,
/// the flow ratios and the velocity are **not prices** and are left alone —
/// which is also the check on this function: `flow-momentum` reads only those,
/// so its trade set and its metrics must be identical at every offset.
///
/// Levels are shifted down rather than bars up because the bars are the
/// account: moving them would move every notional, every margin check and
/// every price a receipt quotes, while the distances the rules trigger on are
/// identical either way.
pub fn shift_frame_down(f: &mut Frame, offset: f64) {
    f.spot -= offset;
    for c in &mut f.clusters {
        c.low -= offset;
        c.high -= offset;
        c.center -= offset;
    }
    for c in &mut f.contexts {
        for slot in [&mut c.max_pain, &mut c.poc, &mut c.w_sup, &mut c.w_res, &mut c.call_be, &mut c.put_be] {
            *slot = slot.map(|v| v - offset);
        }
    }
}

/// Strip a frame of everything that is a price, keeping everything that is not.
///
/// This is the REFUSAL, and it is deliberately not a deletion. Dropping the
/// frame would make `OptionsTimeline::view_from` hand the next bar an EARLIER
/// surviving frame — stale levels carrying a stale basis, which is worse than
/// no correction at all. Keeping it with no levels makes the three
/// level-reading mechanisms return `Intent::None` at that bar (read off
/// `builtin.rs`: two of them loop over `options.clusters`, the third takes
/// `options.contexts.first()`), while `flow-momentum`, which reads no price,
/// is untouched — exactly right, since it needs no basis.
///
/// `spot` becomes `NaN` rather than keeping its GC-axis value or becoming
/// `0.0`: there is no basis to move it with, so the honest encoding of "not
/// measurable here" is the one every caller already tests for with
/// `is_finite()`.
pub fn refuse_frame(f: &mut Frame) {
    f.spot = f64::NAN;
    f.clusters.clear();
    f.contexts.clear();
}

/// What a basis run did, for the receipt. Counts are separate on purpose:
/// `null` is not `0`, so "no basis here" is never folded into a total.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub frames: usize,
    /// Frames that got an estimate and were shifted.
    pub shifted: usize,
    /// Frames with no estimate, whose levels were therefore refused.
    pub refused: usize,
    /// Frames sharing a timestamp with the frame before them.
    ///
    /// **Not cosmetic, and measured at 6,010 of 9,491 on the gold tape.**
    /// `build_timeline` steps a sampling clock but stamps each frame with
    /// `snapshot.as_of`, which is the LAST PRINT's time — so across the tape's
    /// 17-day gap every five-minute step emits another frame carrying the same
    /// timestamp as the one before. The frames are identical, so no lookup is
    /// wrong, but any count or share taken over FRAMES is meaningless: 63% of
    /// this timeline is one September instant repeated. Reported so nobody
    /// reads `shifted / frames` as a coverage figure.
    pub duplicate_frames: usize,
    /// Bars the engine can actually ask about: bars with any frame at or
    /// before them, inside the bar series it is run on. **This is the
    /// denominator that means something** — the engine iterates bars, never
    /// frames, so a frame outside the bar series is never read at all.
    pub bars_covered: usize,
    /// Of those, the bars whose frame still carries levels after the basis was
    /// applied — i.e. the bars at which a level-reading rule can fire.
    pub bars_with_levels: usize,
    /// Raw observations available at all, over the whole bar series.
    pub observations: usize,
    /// Distribution of the APPLIED estimates, one per frame, `None` when
    /// nothing was applied.
    ///
    /// ⚠️ **Frame-weighted, and therefore distorted** on this tape: 63% of the
    /// frames are duplicate timestamps (see `duplicate_frames`), all stamped
    /// at the last print before the tape's gap, which is where the basis is at
    /// its lowest. Read [`Report::at_bars`] for the number the engine actually
    /// traded on.
    pub applied: Option<Spread>,
    /// Distribution of the basis **as each covered BAR saw it** — the estimate
    /// carried by the frame that bar's own lookup returns.
    ///
    /// This is the honest description of the correction: one value per bar the
    /// engine can trade, in bar order, BAR-weighted rather than frame-weighted.
    /// It is the series to compare a constant offset against, because a
    /// constant offset is also one number per bar. (Two bars can read the same
    /// frame, so a frame may appear twice here — that is correct: what is being
    /// described is what the bars traded on, not what the timeline holds.)
    pub at_bars: Option<Spread>,
    /// Distribution of the RAW observations, `None` when there are none.
    pub raw: Option<Spread>,
}

/// A distribution, reported rather than summarised to one number — the whole
/// finding this module answers to is that one number was not enough.
#[derive(Debug, Clone, PartialEq)]
pub struct Spread {
    pub n: usize,
    pub mean: f64,
    pub sd: f64,
    pub min: f64,
    pub p10: f64,
    pub p50: f64,
    pub p90: f64,
    pub max: f64,
    /// Means of the four quartiles in TIME order, which is how the monotone
    /// drift in this series was found in the first place.
    pub quartile_means: [f64; 4],
}

impl Spread {
    /// `values` in time order; the quantiles sort a copy, the quartile means do
    /// not, because their whole point is that the order matters.
    #[must_use]
    pub fn of(values: &[f64]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let n = values.len();
        let mean = values.iter().sum::<f64>() / n as f64;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
        let mut sorted = values.to_vec();
        sorted.sort_unstable_by(f64::total_cmp);
        let q = |p: f64| {
            let idx = ((n as f64 - 1.0) * p).round() as usize;
            sorted[idx.min(n - 1)]
        };
        let mut quartile_means = [f64::NAN; 4];
        for (k, slot) in quartile_means.iter_mut().enumerate() {
            let a = n * k / 4;
            let b = n * (k + 1) / 4;
            if b > a {
                *slot = values[a..b].iter().sum::<f64>() / (b - a) as f64;
            }
        }
        Some(Self {
            n,
            mean,
            sd: var.sqrt(),
            min: sorted[0],
            p10: q(0.10),
            p50: q(0.50),
            p90: q(0.90),
            max: sorted[n - 1],
            quartile_means,
        })
    }
}

/// Apply the rolling basis to a whole timeline, and say what it did.
///
/// `reference` is the GC-axis bar series (see [`observations`] for why it is a
/// bar series). An EMPTY reference yields no observations, hence no estimate at
/// any frame, hence every frame refused — which is the correct answer to "the
/// reference series is missing" and is visible in the report rather than
/// silently reading GC levels as spot levels 42 dollars out.
#[must_use]
pub fn apply(
    timeline: OptionsTimeline,
    reference: &[Bar],
    bars: &[Bar],
    cfg: &RollingBasis,
) -> (OptionsTimeline, Report) {
    let frames_in = timeline.frames();
    let step = interval_ms(bars);
    let obs = step.map(|s| observations(reference, bars, s)).unwrap_or_default();
    let raw = Spread::of(&obs.iter().map(|o| o.value).collect::<Vec<_>>());

    let (mut shifted, mut refused, mut applied_values) = (0usize, 0usize, Vec::new());
    let out: Vec<Frame> = frames_in
        .iter()
        .map(|f| {
            let mut f = f.clone();
            match estimate(&obs, f.t, cfg) {
                Some(b) => {
                    shifted += 1;
                    applied_values.push(b);
                    shift_frame_down(&mut f, b);
                }
                None => {
                    refused += 1;
                    refuse_frame(&mut f);
                }
            }
            f
        })
        .collect();

    let duplicate_frames = out.windows(2).filter(|w| w[0].t == w[1].t).count();
    let shifted_timeline = OptionsTimeline::new(out);
    // Counted over BARS, because that is what the engine iterates. A frame the
    // bar series never reaches is not coverage, and the frame counts above are
    // inflated by `duplicate_frames` besides.
    let mut bars_covered = 0usize;
    let mut bars_with_levels = 0usize;
    // The basis per BAR, re-derived at the frame time that bar's own lookup
    // lands on, so it is the same number the rules were run with.
    let mut at_bars = Vec::new();
    for bar in bars {
        if let Some(f) = shifted_timeline.at(bar.time) {
            bars_covered += 1;
            if !f.clusters.is_empty() {
                bars_with_levels += 1;
                if let Some(b) = estimate(&obs, f.t, cfg) {
                    at_bars.push(b);
                }
            }
        }
    }
    let report = Report {
        frames: shifted_timeline.len(),
        shifted,
        refused,
        duplicate_frames,
        bars_covered,
        bars_with_levels,
        observations: obs.len(),
        applied: Spread::of(&applied_values),
        at_bars: Spread::of(&at_bars),
        raw,
    };
    (shifted_timeline, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_strategy::registry::{ClusterView, ContextView};

    const M15: i64 = 900_000;
    const M5: i64 = 300_000;
    const M1: i64 = 60_000;

    fn bar(t: i64, close: f64) -> Bar {
        Bar { time: t, open: close, high: close + 0.5, low: close - 0.5, close, volume: None }
    }

    fn frame(t: i64, spot: f64) -> Frame {
        Frame {
            t,
            spot,
            bull_ratio: 0.5,
            bull_ratio_15m: 0.5,
            net_flow_velocity_norm: 0.0,
            big_trade_imbalance: 0.0,
            clusters: vec![ClusterView { low: spot - 2.0, high: spot + 2.0, center: spot, score: 4.0 }],
            contexts: vec![ContextView {
                symbol: "X".to_string(),
                dte: 1.0,
                max_pain: Some(spot + 1.0),
                poc: Some(spot),
                w_sup: Some(spot - 5.0),
                w_res: Some(spot + 5.0),
                call_be: Some(spot + 7.0),
                put_be: Some(spot - 7.0),
                bull_ratio: 0.5,
            }],
        }
    }

    /// The observation is stamped at the bar's CLOSE, not at its open — the
    /// one-term difference between a causal basis and a look-ahead one.
    #[test]
    fn an_observation_is_stamped_when_the_close_became_knowable() {
        // GC minutes inside each 15m bar; the last one that starts inside is
        // the one taken.
        let gc: Vec<Bar> = (0..60).map(|i| bar(i * M1, 2000.0 + (i / 15) as f64)).collect();
        let bars: Vec<Bar> = (0..4).map(|i| bar(i * M15, 1950.0)).collect();
        let obs = observations(&gc, &bars, M15);
        assert_eq!(obs.len(), 4);
        assert_eq!(obs[0].known_at, M15, "bar 0 opens at 0 and is knowable at its close");
        assert!((obs[0].value - 50.0).abs() < 1e-9);
        assert!((obs[3].value - 53.0).abs() < 1e-9);
    }

    /// A bar with no reference price inside it has nothing to subtract, so
    /// there is no observation — not an observation of zero.
    #[test]
    fn a_bar_with_no_reference_price_makes_no_observation_rather_than_a_zero() {
        // The reference series starts only at bar 10.
        let gc: Vec<Bar> = (150..210).map(|i| bar(i * M1, 2000.0)).collect();
        let bars: Vec<Bar> = (0..14).map(|i| bar(i * M15, 1950.0)).collect();
        let obs = observations(&gc, &bars, M15);
        assert_eq!(obs.len(), 4, "only bars 10..13 contain a reference minute");
        assert!(obs.iter().all(|o| o.value != 0.0));
    }

    /// An EMPTY reference series refuses everything rather than reading GC
    /// levels as spot levels 42 dollars out.
    #[test]
    fn an_absent_reference_series_refuses_every_frame() {
        let frames: Vec<Frame> = (0..40).map(|i| frame(i * M5, 2000.0)).collect();
        let bars: Vec<Bar> = (0..40).map(|i| bar(i * M15, 1950.0)).collect();
        let cfg = RollingBasis { window_ms: 8 * 3_600_000, min_obs: 5 };
        let got = series(&frames, &[], &bars, &cfg);
        assert!(got.iter().all(Option::is_none), "no reference means no estimate anywhere");
        let (_, report) = apply(OptionsTimeline::new(frames.clone()), &[], &bars, &cfg);
        assert_eq!(report.observations, 0);
        assert_eq!(report.shifted, 0);
        assert_eq!(report.refused, frames.len(), "every frame refused, and counted as refused");
        assert!(report.applied.is_none(), "nothing applied is `null`, not a spread of zeros");
        assert!(report.at_bars.is_none(), "and no bar traded on a basis, which is `null` too");
        assert_eq!(report.bars_with_levels, 0, "no basis means no bar can read a level");
        assert!(report.bars_covered > 0, "the bars are still covered by frames; it is the LEVELS that went");
    }

    /// Too little past is `None`, which the caller turns into a refusal. If
    /// this ever returned `0.0` the engine would silently read GC levels as if
    /// they were spot levels, 42 dollars out.
    #[test]
    fn too_few_observations_is_none_and_never_zero() {
        let frames: Vec<Frame> = (0..40).map(|i| frame(i * M5, 2000.0)).collect();
        let gc: Vec<Bar> = (0..600).map(|i| bar(i * M1, 2000.0)).collect();
        let bars: Vec<Bar> = (0..40).map(|i| bar(i * M15, 1956.3)).collect();
        let cfg = RollingBasis { window_ms: 2 * 3_600_000, min_obs: 5 };
        let got = series(&frames, &gc, &bars, &cfg);
        assert!(got[0].is_none(), "nothing is knowable before the first close");
        assert!(got.iter().any(Option::is_some), "and it does warm up");
        let first_some = got.iter().position(Option::is_some).unwrap();
        assert!(
            got[..first_some].iter().all(Option::is_none),
            "the warm-up prefix is all None, with no zero in it"
        );
        assert!((got[first_some].unwrap() - 43.7).abs() < 1e-9);
    }

    /// The window's upper bound is OPEN: an observation knowable exactly at
    /// `t` is not in the past of `t`.
    #[test]
    fn an_observation_knowable_exactly_at_t_is_excluded() {
        let obs: Vec<Observation> = (1..=6).map(|i| Observation { known_at: i * M15, value: i as f64 }).collect();
        let cfg = RollingBasis { window_ms: 100 * M15, min_obs: 1 };
        // At t = 3*M15 the knowable set is {1,2} -> median 1.5. If the bound
        // were closed it would be {1,2,3} -> 2.0.
        assert_eq!(estimate(&obs, 3 * M15, &cfg), Some(1.5));
        assert_eq!(estimate(&obs, 3 * M15 + 1, &cfg), Some(2.0));
    }

    /// THE CAUSALITY TEST, in the shape `companion`'s uses -- but cut by
    /// KNOWABILITY rather than by index, which is what makes it bite.
    ///
    /// The property asserted is the information one: *at wall-clock instant
    /// `tau`, the basis for every frame at `t <= tau` must be the same number
    /// it is with the whole series present.* So the truncation is: frames at
    /// `t <= tau`, and bars whose CLOSE was knowable by `tau`
    /// (`bar.time + interval <= tau`) -- not bars whose open was before it.
    ///
    /// An earlier version of this test cut both series at a bar INDEX and
    /// compared frames before that bar's open. It passed -- and it also passed
    /// for a probe that stamps observations at the bar's open instead of its
    /// close, because a stamp error of exactly one bar interval is invisible to
    /// a cut that lands on a bar interval. Written down because that near miss
    /// is the whole reason this test is worth having: it was blind in the one
    /// direction it existed to watch.
    ///
    /// Five cut instants, deliberately including frame times that are NOT bar
    /// boundaries, which is where a sub-interval leak shows up.
    #[test]
    fn cutting_either_series_by_knowability_leaves_every_earlier_basis_untouched() {
        let (frames, gc, bars, cfg) = drifting_case();
        let full = series(&frames, &gc, &bars, &cfg);
        assert!(full.iter().any(Option::is_some), "the series is not vacuously equal");
        let mut checked = 0usize;
        for tau in cut_instants(&frames) {
            let (short, visible) = series_as_of(&frames, &gc, &bars, &cfg, tau, series);
            for i in 0..visible {
                assert!(
                    parity_opt(full[i], short[i]),
                    "frame {i} basis moved when data not knowable at tau={tau} was removed: {:?} vs {:?}",
                    full[i],
                    short[i]
                );
            }
            checked += 1;
        }
        assert_eq!(checked, 5, "all five cut instants were exercised");
    }

    /// DELIBERATELY look-ahead baselines, asserted to FAIL the same test.
    ///
    /// A cut test only proves something if a method that genuinely reads the
    /// future is caught by it. Two probes, each a mistake someone would make:
    ///
    /// * `probe_stamped_at_open` stamps each observation at `bar.time` instead
    ///   of `bar.time + interval`, so a bar's own close feeds a basis dated
    ///   before that close existed. **ONE changed term, invisible in a
    ///   receipt** -- and invisible to the index-cut version of the test above.
    /// * `probe_whole_sample_median` is the median of **every** observation in
    ///   the series: the same quantity every value of `--basis-offset=` is. So
    ///   this probe is not hypothetical -- it is what the three constant control
    ///   offsets ARE, and this assertion is the statement that **p10 / mean /
    ///   p90 of the whole overlap are IN-SAMPLE numbers.**
    ///
    /// Each must be caught at every one of the five cut instants. If either
    /// stops failing, nothing is checking causality any more.
    #[test]
    fn a_deliberately_look_ahead_basis_fails_the_same_cut_test() {
        let (frames, gc, bars, cfg) = drifting_case();
        type Probe = fn(&[Frame], &[Bar], &[Bar], &RollingBasis) -> Vec<Option<f64>>;
        for (name, probe) in [
            ("stamped at the bar's own open", probe_stamped_at_open as Probe),
            ("whole-sample median, i.e. what a constant offset IS", probe_whole_sample_median as Probe),
        ] {
            let full = probe(&frames, &gc, &bars, &cfg);
            let (mut caught_unaligned, mut unaligned) = (0usize, 0usize);
            let mut caught_total = 0usize;
            for tau in cut_instants(&frames) {
                let (short, visible) = series_as_of(&frames, &gc, &bars, &cfg, tau, probe);
                let caught = (0..visible).any(|i| !parity_opt(full[i], short[i]));
                caught_total += usize::from(caught);
                if tau % M15 != 0 {
                    unaligned += 1;
                    caught_unaligned += usize::from(caught);
                }
            }
            assert_eq!(unaligned, 4, "the fixture must offer four cut instants off the bar grid");
            assert_eq!(
                caught_unaligned, unaligned,
                "the `{name}` probe reads the future and the cut test caught it at only {caught_unaligned}/{unaligned} instants OFF the bar grid, so it is blind"
            );
            // MEASURED, and the reason `cut_instants` is not allowed to be all
            // bar-aligned: at a cut instant that lands exactly on the bar grid
            // the `stamped at the bar's own open` probe is NOT caught, because
            // its error is exactly one bar interval and the cut removes exactly
            // whole bars. 4 of 5 here, not 5 of 5 -- a one-interval leak is
            // invisible to an interval-aligned cut, which is how the first
            // version of the test above passed while being blind.
            assert!(
                caught_total >= 4,
                "the `{name}` probe was caught at {caught_total}/5 instants overall"
            );
        }
    }

    /// A drifting basis, so a value that moved would actually differ, on the
    /// real cadences: a 1m GC reference, a 15m bar grid, a 5m frame grid.
    fn drifting_case() -> (Vec<Frame>, Vec<Bar>, Vec<Bar>, RollingBasis) {
        let frames: Vec<Frame> = (0..240).map(|i| frame(i * M5, 2000.0)).collect();
        let bars: Vec<Bar> = (0..80).map(|i| bar(i * M15, 2000.0 + (i as f64 / 5.0).cos())).collect();
        // GC drifts away from spot, so the basis itself drifts.
        let gc: Vec<Bar> = (0..80 * 15)
            .map(|i| {
                let t = i as f64 / 15.0;
                bar(i * M1, 2000.0 + (t / 5.0).cos() + 45.0 - t * 0.05 + (t / 7.0).sin())
            })
            .collect();
        (frames, gc, bars, RollingBasis { window_ms: 8 * 3_600_000, min_obs: 5 })
    }

    /// Five instants to cut at. Frame times, chosen so that most are NOT on
    /// the 15m bar grid: a sub-interval leak is only visible there.
    fn cut_instants(frames: &[Frame]) -> Vec<i64> {
        let picks = [61usize, 100, 121, 150, 181];
        let out: Vec<i64> = picks.iter().map(|i| frames[*i].t).collect();
        assert!(out.iter().filter(|t| *t % M15 != 0).count() >= 3, "the cut instants must not all be bar-aligned");
        out
    }

    /// All THREE series as a reader at `tau` would hold them, and how many
    /// frames that reader can see. A bar is visible only once its CLOSE is
    /// knowable; a frame only once it has been taken. The reference series is
    /// truncated the same way — it is the third place a two-series method can
    /// reach into the future, and the one `Frame::spot` used to hide in.
    fn series_as_of(
        frames: &[Frame],
        reference: &[Bar],
        bars: &[Bar],
        cfg: &RollingBasis,
        tau: i64,
        f: impl Fn(&[Frame], &[Bar], &[Bar], &RollingBasis) -> Vec<Option<f64>>,
    ) -> (Vec<Option<f64>>, usize) {
        let step = interval_ms(bars).expect("the fixture has a modal interval");
        let rstep = interval_ms(reference).expect("the fixture's reference has one too");
        let frames_vis: Vec<Frame> = frames.iter().filter(|fr| fr.t <= tau).cloned().collect();
        let bars_vis: Vec<Bar> = bars.iter().filter(|b| b.time + step <= tau).copied().collect();
        let ref_vis: Vec<Bar> = reference.iter().filter(|b| b.time + rstep <= tau).copied().collect();
        let n = frames_vis.len();
        (f(&frames_vis, &ref_vis, &bars_vis, cfg), n)
    }

    /// The probe: every observation stamped at the bar's OPEN, so a bar's own
    /// close feeds a basis dated before that close existed. One term.
    fn probe_stamped_at_open(
        frames: &[Frame],
        reference: &[Bar],
        bars: &[Bar],
        cfg: &RollingBasis,
    ) -> Vec<Option<f64>> {
        let Some(step) = interval_ms(bars) else { return vec![None; frames.len()] };
        let obs: Vec<Observation> = observations(reference, bars, step)
            .into_iter()
            .map(|o| Observation { known_at: o.known_at - step, value: o.value }) // <- the bug
            .collect();
        frames.iter().map(|f| estimate(&obs, f.t, cfg)).collect()
    }

    /// The probe: one number for the whole run, taken over the whole series --
    /// which is exactly the quantity `--basis-offset=` carries.
    fn probe_whole_sample_median(
        frames: &[Frame],
        reference: &[Bar],
        bars: &[Bar],
        cfg: &RollingBasis,
    ) -> Vec<Option<f64>> {
        let Some(step) = interval_ms(bars) else { return vec![None; frames.len()] };
        let obs = observations(reference, bars, step);
        let one = median_of(&obs, cfg.min_obs);
        vec![one; frames.len()]
    }

    fn median_of(window: &[Observation], min_obs: usize) -> Option<f64> {
        if window.len() < min_obs {
            return None;
        }
        let mut v: Vec<f64> = window.iter().map(|o| o.value).collect();
        v.sort_unstable_by(f64::total_cmp);
        let n = v.len();
        Some(if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 })
    }

    fn parity_opt(a: Option<f64>, b: Option<f64>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(x), Some(y)) => fd_core::parity_eq(x, y),
            _ => false,
        }
    }

    /// A refused frame loses every price and keeps every non-price, which is
    /// what makes it a refusal for the three level readers and a no-op for
    /// `flow-momentum`.
    #[test]
    fn a_refused_frame_keeps_the_flow_fields_and_loses_the_levels() {
        let mut f = frame(0, 2000.0);
        f.bull_ratio_15m = 0.73;
        f.net_flow_velocity_norm = 0.21;
        refuse_frame(&mut f);
        assert!(f.clusters.is_empty() && f.contexts.is_empty());
        assert!(f.spot.is_nan(), "no basis means no spot on the bars' axis — NaN, not 0.0");
        assert!((f.bull_ratio_15m - 0.73).abs() < 1e-12);
        assert!((f.net_flow_velocity_norm - 0.21).abs() < 1e-12);
    }

    /// The shift touches prices and only prices — the property that makes
    /// `flow-momentum` a usable control on the whole correction.
    #[test]
    fn the_shift_moves_prices_and_leaves_the_flow_fields_alone() {
        let mut f = frame(0, 2000.0);
        f.bull_ratio_15m = 0.73;
        f.net_flow_velocity_norm = 0.21;
        let before = f.clone();
        shift_frame_down(&mut f, 43.7);
        assert!((f.spot - (before.spot - 43.7)).abs() < 1e-9);
        assert!((f.clusters[0].low - (before.clusters[0].low - 43.7)).abs() < 1e-9);
        assert!((f.clusters[0].center - (before.clusters[0].center - 43.7)).abs() < 1e-9);
        assert!((f.contexts[0].max_pain.unwrap() - (before.contexts[0].max_pain.unwrap() - 43.7)).abs() < 1e-9);
        assert!((f.contexts[0].put_be.unwrap() - (before.contexts[0].put_be.unwrap() - 43.7)).abs() < 1e-9);
        assert_eq!(f.clusters[0].score, before.clusters[0].score, "score is not a price");
        assert_eq!(f.contexts[0].dte, before.contexts[0].dte, "dte is not a price");
        assert_eq!(f.bull_ratio_15m, before.bull_ratio_15m);
        assert_eq!(f.net_flow_velocity_norm, before.net_flow_velocity_norm);
    }

    /// Duplicate-timestamped frames are COUNTED, because the real timeline has
    /// 6,010 of them in 9,491 and a share taken over frames is therefore not a
    /// coverage figure.
    #[test]
    fn duplicate_frames_are_counted_and_bars_are_the_denominator() {
        // Three distinct instants, with the middle one repeated four times —
        // the shape `build_timeline` produces across a tape gap, since it
        // stamps every step with the last print's time.
        let times = [0, M5, M5, M5, M5, 2 * M5];
        let frames: Vec<Frame> = times.iter().map(|t| frame(*t, 2000.0)).collect();
        let gc: Vec<Bar> = (0..600).map(|i| bar(i * M1, 2000.0)).collect();
        let bars: Vec<Bar> = (0..40).map(|i| bar(i * M15, 1956.3)).collect();
        let cfg = RollingBasis { window_ms: 8 * 3_600_000, min_obs: 5 };
        let (_, r) = apply(OptionsTimeline::new(frames), &gc, &bars, &cfg);
        assert_eq!(r.frames, 6);
        assert_eq!(r.duplicate_frames, 3, "four frames at one instant are three duplicates");
        assert_eq!(r.bars_covered, 40, "every bar has a frame at or before it");
    }

    /// The modal interval survives a session break, so a gap in the bars does
    /// not silently redefine when a close became knowable.
    #[test]
    fn the_modal_interval_ignores_a_session_break() {
        let times = [0, M15, 2 * M15, 3 * M15, 3 * M15 + 20 * M15, 3 * M15 + 21 * M15];
        let bars: Vec<Bar> = times.iter().map(|t| bar(*t, 2000.0)).collect();
        assert_eq!(interval_ms(&bars), Some(M15));
    }

    /// Quartile means stay in TIME order: the drift is the thing being looked
    /// for, so sorting them away would hide it.
    #[test]
    fn the_spread_reports_drift_in_time_order() {
        let values: Vec<f64> = (0..100).map(|i| 46.0 - i as f64 * 0.05).collect();
        let s = Spread::of(&values).unwrap();
        assert!(s.quartile_means[0] > s.quartile_means[3], "a falling series must read as falling");
        assert!(s.quartile_means.windows(2).all(|w| w[0] > w[1]), "and monotonically");
        assert!(s.p10 < s.p90);
        assert_eq!(s.n, 100);
    }
}
