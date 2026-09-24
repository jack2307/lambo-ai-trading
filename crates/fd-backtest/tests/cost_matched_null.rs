//! The matched null has to be matched on COST as well as on count, and
//! repairing that must leave every row whose stop is already the control's
//! exactly where it was.
//!
//! Cost as a fraction of risk is `spread / stop`. Until 2026-09-24
//! `hypotheses::control_for` built the random-entry control from
//! `RandomEntry::default_params()` and overrode only `seed` (and `entryRate`,
//! which `matched_rate` calibrates), so **the control stopped at 1.5 ATR
//! whatever the method under test stopped at** and its cost share was pinned
//! near 7.5% of R. A method that merely widened its stop paid less per trade
//! than its own control and cleared it without predicting anything:
//! `far-stop-break` at an 80-bar channel's far edge measured **profit factor
//! 0.973 — losing money — at the 98th percentile**, count match 0.95 and
//! inside the band (`docs/research/runs/2026-09-23-designed-1-cost-term/`).
//!
//! Four things are held here and they are different things.
//!
//! 1. **A method already at the control's stop does not move.** Not "moves
//!    little": the corrected control's parameters and its whole null
//!    distribution are bit-identical to the pre-repair reading. Both funded
//!    books run at `stopAtr = 1.5`, so this is what keeps the repair from
//!    silently restating a number the owner was given.
//! 2. **A method that names its stop gets that stop, exactly.**
//! 3. **A method whose stop is structural gets its own REALISED median stop**,
//!    measured from the trades it took — and a declared `stopAtr` that the
//!    method does not actually use does not get copied. `far-stop-break`
//!    declares 1.2 and, in `stopMode = 0`, stops at the opposite edge of the
//!    channel instead. A repair that asked only "does the method name
//!    `stopAtr`?" would have copied 1.2 onto the control of the very row that
//!    exposed the defect.
//! 4. **The count match survives the cost match.** A wider control stop makes
//!    the control's trades last longer and so changes how many it takes, which
//!    is why `matched_rate` is calibrated AFTER the stop is set. Trading one
//!    defect for the other is not a repair.
//!
//! And the gate figures belong to the method: they are bit-identical between
//! the two readings, because the only thing that differs is the control.
//!
//! **What would reopen this.** Any change to what the control's stop is
//! matched from, or a path that builds a random-entry control without going
//! through `matched_control_for`.

use fd_backtest::engine::TradingRules;
use fd_backtest::hypotheses::{
    CostMatch, Hypothesis, STOP_GOVERNS_BAND, StopSource, run_hypothesis_fixed_as, run_hypothesis_fixed_guarded,
};
use fd_backtest::sweep::PromisingGate;
use fd_core::types::Bar;
use fd_strategy::registry::Registry;

const BAR: i64 = 15 * 60_000;
const SEEDS: usize = 24;

/// Gold as `config/default.toml` declares the Vantage market: one ounce a
/// lot, a 0.28 round-turn spread, no commission and no swap.
fn rules() -> TradingRules {
    TradingRules {
        contract_size: 1.0,
        spread: 0.28,
        commission_per_lot: 0.0,
        swap_long_per_lot: 0.0,
        swap_short_per_lot: 0.0,
        starting_equity_usd: 100.0,
        lot_step: 0.01,
        min_lot: 0.01,
        ..TradingRules::default()
    }
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The same seeded random walk `matched_null.rs` uses: not a market and not a
/// claim about one, a bar stream that wanders far enough for a real method to
/// find signals in it, reproducible from the seed alone.
fn bars(n: usize) -> Vec<Bar> {
    let mut price = 3600.0f64;
    (0..n)
        .map(|i| {
            let u = |stream: u64| (mix(0xF10D ^ mix((i as u64 * 0x1B3) ^ stream)) >> 11) as f64 / (1u64 << 53) as f64;
            let open = price;
            let step = (u(0) - 0.5) * 6.0;
            let close = open + step;
            let wick = 0.4 + u(1) * 2.4;
            price = close;
            Bar { time: i as i64 * BAR, open, high: open.max(close) + wick, low: open.min(close) - wick, close, volume: Some(1.0) }
        })
        .collect()
}

fn hypothesis(label: &str, base: &str, overrides: &[(&str, f64)]) -> Hypothesis {
    Hypothesis {
        label: label.to_string(),
        base: base.to_string(),
        filters: Vec::new(),
        overrides: overrides.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
        why: "a fixture for the cost-matched null, not a claim about the market".to_string(),
    }
}

/// `ema-cross` stops at 1.5 ATR, which is where the control has always
/// stopped. Both funded books are in this case.
#[test]
fn a_method_already_at_the_controls_stop_is_read_against_a_bit_identical_null() {
    let (rules, bars, registry, gate) = (rules(), bars(4000), Registry::with_builtins(), PromisingGate::default());
    let h = hypothesis("at-1.5", "ema-cross", &[]);

    let corrected = run_hypothesis_fixed_as(&registry, &h, &bars, &rules, &gate, SEEDS, None, CostMatch::Method)
        .expect("a well-formed hypothesis");
    let record = run_hypothesis_fixed_as(&registry, &h, &bars, &rules, &gate, SEEDS, None, CostMatch::RegisteredStop)
        .expect("a well-formed hypothesis");

    let stop = corrected.control_stop.expect("an engine-exit method's control has a stop");
    assert_eq!(stop.atr, 1.5, "the method's own stop, copied");
    assert_eq!(stop.source, StopSource::Named, "it declares stopAtr and uses it");

    assert!(corrected.oos.trades > 0, "a fixture that takes no trade cannot show a null holding still");
    assert_eq!(corrected.null_pf.len(), record.null_pf.len(), "the same number of control runs");
    for (i, (a, b)) in corrected.null_pf.iter().zip(&record.null_pf).enumerate() {
        assert_eq!(a.to_bits(), b.to_bits(), "null run {i}: {a} vs {b} — a row at 1.5 ATR must not move at all");
    }
    assert_eq!(corrected.null_trades, record.null_trades, "and the control takes the same trades");
    assert_eq!(
        corrected.percentile.to_bits(),
        record.percentile.to_bits(),
        "so the percentile is the same number, not a close one: {} vs {}",
        corrected.percentile,
        record.percentile,
    );
}

/// `donchian-breakout` stops at 2.0 ATR — a third further out than the
/// control did, so it was paying a third less of its risk in spread than the
/// distribution it was scored against.
#[test]
fn a_wider_named_stop_is_copied_onto_the_control_and_changes_its_distribution() {
    let (rules, bars, registry, gate) = (rules(), bars(4000), Registry::with_builtins(), PromisingGate::default());
    let h = hypothesis("at-2.0", "donchian-breakout", &[]);

    let corrected = run_hypothesis_fixed_as(&registry, &h, &bars, &rules, &gate, SEEDS, None, CostMatch::Method)
        .expect("a well-formed hypothesis");
    let record = run_hypothesis_fixed_as(&registry, &h, &bars, &rules, &gate, SEEDS, None, CostMatch::RegisteredStop)
        .expect("a well-formed hypothesis");

    let stop = corrected.control_stop.expect("an engine-exit method's control has a stop");
    assert_eq!(stop.atr, 2.0, "the method's own stop, copied exactly");
    assert_eq!(stop.source, StopSource::Named);
    assert_eq!(record.control_stop.expect("also a stop").atr, 1.5, "the record's control kept its own");

    assert!(corrected.oos.trades > 0, "a fixture that takes no trade cannot show a null moving");
    assert_ne!(
        corrected.null_pf, record.null_pf,
        "the control's stop changed by a third and its profit factors did not: nothing was matched"
    );
    // The cost the control pays, which is the quantity the whole repair is
    // about, is lower at the wider stop.
    assert!(
        stop.cost_fraction_of_r(&rules) < record.control_stop.unwrap().cost_fraction_of_r(&rules),
        "a wider stop pays a smaller share of its risk in spread",
    );
}

/// The case the registration calls case 2, and the case a naive fix gets
/// wrong: `far-stop-break` in `stopMode = 0` DECLARES `stopAtr = 1.2` and
/// stops at the opposite edge of the channel instead, several times further
/// out.
#[test]
fn a_structural_stop_is_matched_from_the_methods_realised_distance_not_its_declared_one() {
    let (rules, bars, registry, gate) = (rules(), bars(4000), Registry::with_builtins(), PromisingGate::default());
    let h = hypothesis("struct", "far-stop-break", &[("period", 80.0), ("stopMode", 0.0), ("riskReward", 1.0)]);

    let report = run_hypothesis_fixed_guarded(&registry, &h, &bars, &rules, &gate, SEEDS, None)
        .expect("a well-formed hypothesis");
    let stop = report.control_stop.expect("an engine-exit method's control has a stop");

    assert!(report.oos.trades > 0, "a fixture that takes no trade has no realised stop to measure");
    assert_eq!(stop.source, StopSource::Realised, "the declared 1.2 does not govern this mode");
    assert_eq!(stop.named, Some(1.2), "and the declared value is still reported, so a reader sees the contradiction");
    assert!(
        (stop.realised_atr / 1.2 - 1.0).abs() > STOP_GOVERNS_BAND,
        "the fixture is wrong, not the code: a structural stop at {:.2} ATR is within a quarter of the declared 1.2",
        stop.realised_atr,
    );
    assert_eq!(stop.atr.to_bits(), stop.realised_atr.to_bits(), "the control takes the realised median, exactly");
    assert_eq!(stop.measured_trades, report.oos.trades, "measured from every trade the method took on this window");
    assert!(stop.points() > 0.0 && stop.points().is_finite(), "and the same distance in points, for the cost line");
}

/// Changing the control's stop changes how long its trades last and so how
/// many it takes. The rate is recalibrated after the stop is set, and this is
/// the measured proof that it was: a structural row, where the stop moves
/// furthest, still comes out the method's size.
#[test]
fn the_count_match_survives_the_cost_match() {
    let (rules, bars, registry, gate) = (rules(), bars(4000), Registry::with_builtins(), PromisingGate::default());
    for h in [
        hypothesis("struct", "far-stop-break", &[("period", 80.0), ("stopMode", 0.0), ("riskReward", 1.0)]),
        hypothesis("at-2.0", "donchian-breakout", &[]),
        hypothesis("at-1.5", "ema-cross", &[]),
    ] {
        let report = run_hypothesis_fixed_guarded(&registry, &h, &bars, &rules, &gate, SEEDS, None)
            .expect("a well-formed hypothesis");
        assert!(report.oos.trades > 0, "{}: no trades, nothing to match", h.label);
        assert!(
            report.count_matched(),
            "{}: control at {:.2} ATR took a median of {:.0} trades against the method's {} — ratio {:.2}. \
             The cost match was bought with the count match.",
            h.label,
            report.control_stop.map_or(f64::NAN, |s| s.atr),
            fd_backtest::hypotheses::median_count(&report.null_trades),
            report.oos.trades,
            report.count_match(),
        );
    }
}

/// The gate belongs to the method. If repairing the control could move a
/// profit factor, every receipt in `docs/decisions/` would be restated by it.
#[test]
fn the_cost_match_cannot_move_a_gate_figure() {
    let (rules, bars, registry, gate) = (rules(), bars(4000), Registry::with_builtins(), PromisingGate::default());
    for h in [
        hypothesis("struct", "far-stop-break", &[("period", 80.0), ("stopMode", 0.0), ("riskReward", 1.0)]),
        hypothesis("at-2.0", "donchian-breakout", &[]),
    ] {
        let corrected = run_hypothesis_fixed_as(&registry, &h, &bars, &rules, &gate, 8, None, CostMatch::Method)
            .expect("a well-formed hypothesis");
        let record = run_hypothesis_fixed_as(&registry, &h, &bars, &rules, &gate, 8, None, CostMatch::RegisteredStop)
            .expect("a well-formed hypothesis");
        assert_eq!(corrected.oos.trades, record.oos.trades, "{}: the trade count is the method's", h.label);
        for (name, a, b) in [
            ("profit factor", corrected.oos.profit_factor, record.oos.profit_factor),
            ("expectancy", corrected.oos.expectancy, record.oos.expectancy),
            ("total R", corrected.oos.total_r, record.oos.total_r),
            ("net P&L", corrected.oos.net_pnl_usd, record.oos.net_pnl_usd),
        ] {
            assert_eq!(a.to_bits(), b.to_bits(), "{}: {name} must be bit-identical: {a} vs {b}", h.label);
        }
        assert_eq!(corrected.verdict.promising, record.verdict.promising, "{}: and the verdict with them", h.label);
    }
}
