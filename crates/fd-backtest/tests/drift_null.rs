//! The matched null has to carry the instrument's unconditional drift, and it
//! has to do so without giving up the count match or the cost match.
//!
//! Gold's unconditional drift over the window this desk measures is **+0.3946
//! ATR20 per 5 sessions, t = +6.74**, and the price on file went 1,200 to 3,700
//! USD per ounce. Until 2026-10-02 every control here drew its side from a coin
//! — `control.rs:RandomEntry::on_bar`, `control_hold.rs:RandomHold::on_bar` and
//! `direction.rs` all read `< 0.5` — so every control carried an expected long
//! share of 0.50 and an expected drift term of about zero whatever the method
//! it was a control for had collected. A method that merely tends to be long
//! beats such a null without predicting anything, which makes every percentile
//! in `docs/decisions/` a figure with an unaccounted long-share term in it.
//!
//! Registered in `docs/hypotheses/2026-10-02-drift-null.md`, which also names
//! the control chosen, the two alternatives rejected, and six falsifiers aimed
//! at the control rather than at any method.
//!
//! **Three properties, three tests, and they are different properties.**
//!
//! 1. [`the_drift_null_takes_the_methods_own_trade_count`] — count-matched.
//!    `RandomHold::default_params` sets `entryRate = 1.0`, which means
//!    "re-enter on the first bar the filters allow after every exit", so the
//!    hold control filled its gate and took 3.1-3.7x the method's trades
//!    (measured 2026-09-23; out of scope of that day's repair and of the cost
//!    match the day after). That is not cosmetic here: drift is collected in
//!    proportion to time in the market.
//! 2. [`the_drift_null_pays_the_methods_own_cost_share`] — cost-matched. On the
//!    entry branch the control is given the method's own stop, and the drift
//!    match must not disturb that. On the hold branch there is no stop, so the
//!    cost of a book is its trade count times the spread and the cost match is
//!    the count match; this test measures the spread bill in USD on both sides
//!    rather than inferring it. **The spread bill is confounded by equity
//!    growth** on a compounding book - see `HypothesisReport::cost_matched`,
//!    which records the 0.15 it printed on a real row for that reason - so this
//!    fixture is deliberately one where the method and its control come out at
//!    similar profit factors, and the equity-free half of the claim is the
//!    second assertion in that test: the control's `spread / stop` is
//!    bit-identical across the two settings.
//! 3. [`the_drift_null_carries_the_methods_own_signed_exposure`] —
//!    drift-controlled. The measured quantity is **signed minutes in the
//!    market**, long minus short, which is what the drift is paid on. A control
//!    can have the method's side ratio exactly and still be exposed three times
//!    over.
//!
//! And two that hold the instrument itself rather than a property of it:
//! [`an_even_method_is_read_against_the_same_null_it_always_was`], which is
//! falsifier 1 of the registration, and
//! [`a_fully_invested_one_sided_method_has_no_null_with_a_spread`], which is
//! falsifier 5 and is the collapse `agent/repair-a` reported before it stopped.
//!
//! **The bars here carry a deliberate, stated upward drift.** They are not a
//! market and nothing about a market is claimed from them: a control that
//! removes drift cannot be tested on a stream that has none, so the fixture has
//! some, in a known amount, and the test asserts about the control.
//!
//! **What would reopen this.** Any change to what the control's side ratio or
//! its hold-branch rate is measured from, or a path that builds a control
//! without going through `matched_control_for`.

use fd_backtest::engine::TradingRules;
use fd_backtest::hypotheses::{
    COUNT_MATCH_BAND, Hypothesis, NullSides, SIDE_MATCH_BAND, run_hypothesis_fixed_sides, side_ratio_is_one_sided,
};
use fd_backtest::sweep::PromisingGate;
use fd_core::types::Bar;
use fd_strategy::filter::Filter;
use fd_strategy::registry::Registry;

const BAR: i64 = 15 * 60_000;
const SEEDS: usize = 24;

/// Gold as `config/default.toml` declares the Vantage market: one ounce a lot,
/// a 0.28 round-turn spread, no commission and no swap.
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

/// Fifteen-minute bars with a **stated** upward drift, starting at a real
/// epoch so that the New York clock the session filters read lands on real
/// sessions.
///
/// `drift_per_bar` is in price units per bar, so the drift a long collects over
/// a hold is `drift_per_bar * bars_held` — the arithmetic the tests below
/// reason with, rather than a figure read off a chart. The noise step is
/// uniform on +/- 3.0, standard deviation 1.73 per bar, so at the 0.03 these
/// tests use a 26-bar New York hold collects 0.78 of drift against a noise
/// standard deviation of 8.8: about 0.09 of a standard deviation, which is the
/// order gold's own +0.3946 ATR20 per 5 sessions sits at and is small enough
/// that no test here can pass on the size of the fixture's drift alone.
fn drifting_bars(n: usize, drift_per_bar: f64) -> Vec<Bar> {
    // 2022-06-16 00:00 UTC, the start of the window the registry's own
    // xauusd rows use.
    const T0: i64 = 1_655_337_600_000;
    let mut price = 1_800.0f64;
    (0..n)
        .map(|i| {
            let u = |stream: u64| (mix(0xD21F ^ mix((i as u64 * 0x1B3) ^ stream)) >> 11) as f64 / (1u64 << 53) as f64;
            let open = price;
            let close = open + (u(0) - 0.5) * 6.0 + drift_per_bar;
            let wick = 0.4 + u(1) * 2.4;
            price = close;
            Bar {
                time: T0 + i as i64 * BAR,
                open,
                high: open.max(close) + wick,
                low: open.min(close) - wick,
                close,
                volume: Some(1.0),
            }
        })
        .collect()
}

fn hypothesis(label: &str, base: &str, filters: Vec<Filter>, overrides: &[(&str, f64)]) -> Hypothesis {
    Hypothesis {
        label: label.to_string(),
        base: base.to_string(),
        filters,
        overrides: overrides.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
        why: "a fixture for the drift-controlled null, not a claim about the market".to_string(),
    }
}

/// Long the New York cash session, flat otherwise, gated to weekdays: the
/// `session-hold` configuration that gave the hold control a count match of
/// 3.39 on 2026-09-24, because the method holds 6.5 hours of a day the control
/// is free to fill.
fn long_ny() -> Hypothesis {
    hypothesis("long-ny", "session-hold", vec![Filter::weekdays()], &[
        ("from", 930.0),
        ("to", 1600.0),
        ("side", 1.0),
    ])
}

/* ------------------------------------------------------- property 1: count */

/// The hold control must take about as many trades as the method it is a
/// control for — and the defect, the control at `entryRate = 1.0` filling the
/// gate, must still be visible at the setting the record was read against.
///
/// Both halves are asserted. A test that only showed the repaired figure could
/// not tell a repair from a fixture that never had the problem.
#[test]
fn the_drift_null_takes_the_methods_own_trade_count() {
    let (rules, bars, registry, gate) = (rules(), drifting_bars(40_000, 0.03), Registry::with_builtins(), PromisingGate::default());
    let h = long_ny();

    let coin = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::CoinFlip)
        .expect("a well-formed hypothesis");
    let exposure = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::MatchedExposure)
        .expect("a well-formed hypothesis");

    assert!(coin.oos.trades > 100, "a fixture of {} trades is too thin to say anything", coin.oos.trades);

    // The defect, asserted as a defect. The control at entryRate 1.0 is in the
    // market for the whole gate while the method holds one session of it.
    assert!(
        coin.count_match() > 1.0 + COUNT_MATCH_BAND,
        "the recorded defect should be visible on this fixture: the hold control at entryRate 1.0 took a count match of \
         {:.2}, inside the band, so this test is no longer measuring what it was written for",
        coin.count_match(),
    );
    assert!(!coin.count_matched());

    // And the repair, reported as achieved rather than intended.
    assert!(
        exposure.count_matched(),
        "the exposure-matched control took a median of {:.0} trades against the method's {} — count match {:.2}, outside \
         the registered band of {COUNT_MATCH_BAND}",
        fd_backtest::hypotheses::median_count(&exposure.null_trades),
        exposure.oos.trades,
        exposure.count_match(),
    );

    // The gate figures belong to the method: falsifier 2 of the registration.
    assert_eq!(coin.oos.trades, exposure.oos.trades, "the method's trade count is not the control's business");
    assert_eq!(coin.oos.profit_factor.to_bits(), exposure.oos.profit_factor.to_bits(), "nor its profit factor");
    assert_eq!(coin.oos.expectancy.to_bits(), exposure.oos.expectancy.to_bits(), "nor its expectancy");
}

/* -------------------------------------------------------- property 2: cost */

/// The control must pay what the method paid, measured in the spread bill
/// rather than inferred — and the drift match must not undo the cost match the
/// entry branch already has.
#[test]
fn the_drift_null_pays_the_methods_own_cost_share() {
    let (rules, bars, registry, gate) = (rules(), drifting_bars(40_000, 0.03), Registry::with_builtins(), PromisingGate::default());

    // The hold branch, which had no cost figure at all: its cost is its trade
    // count times the spread, so a control taking 3.4x the trades paid 3.4x.
    let h = long_ny();
    let coin = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::CoinFlip)
        .expect("a well-formed hypothesis");
    let exposure = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::MatchedExposure)
        .expect("a well-formed hypothesis");
    assert!(coin.cost_usd > 0.0, "a fixture that paid no spread cannot show a cost match");
    assert!(
        !coin.cost_matched(),
        "the defect should be visible: the hold control at entryRate 1.0 paid a cost match of {:.2}",
        coin.cost_match(),
    );
    assert!(
        exposure.cost_matched(),
        "the exposure-matched control paid {:.2} USD of spread against the method's {:.2} — cost match {:.2}, outside \
         the band",
        fd_backtest::hypotheses::median_f64(&exposure.null_cost_usd),
        exposure.cost_usd,
        exposure.cost_match(),
    );

    // The entry branch, where the cost match is the stop and was repaired on
    // 2026-09-24. The drift match must leave it exactly where it is.
    let crossing = hypothesis("at-2.0", "donchian-breakout", Vec::new(), &[("stopAtr", 2.0)]);
    let before = run_hypothesis_fixed_sides(&registry, &crossing, &bars, &rules, &gate, SEEDS, None, NullSides::CoinFlip)
        .expect("a well-formed hypothesis");
    let after =
        run_hypothesis_fixed_sides(&registry, &crossing, &bars, &rules, &gate, SEEDS, None, NullSides::MatchedExposure)
            .expect("a well-formed hypothesis");
    let (b, a) = (
        before.control_stop.expect("an engine-exit method's control has a stop"),
        after.control_stop.expect("an engine-exit method's control has a stop"),
    );
    assert_eq!(b.atr.to_bits(), a.atr.to_bits(), "the drift match must not move the control's stop: {} vs {}", b.atr, a.atr);
    assert_eq!(b.atr, 2.0, "and it is still the method's own declared stop");
    assert_eq!(
        b.cost_fraction_of_r(&rules).to_bits(),
        a.cost_fraction_of_r(&rules).to_bits(),
        "so the control's cost as a fraction of risk is the same number, not a close one",
    );
}

/* ------------------------------------------------------- property 3: drift */

/// The control must be exposed to the instrument's drift the way the method
/// was: the same side ratio AND the same signed time in the market.
///
/// The second half is the one the ratio alone does not give. A control with a
/// long share of 1.000 and 3.4x the method's trades is 3.4x as long, and
/// collects 3.4x the drift.
#[test]
fn the_drift_null_carries_the_methods_own_signed_exposure() {
    let (rules, bars, registry, gate) = (rules(), drifting_bars(40_000, 0.03), Registry::with_builtins(), PromisingGate::default());
    let h = long_ny();

    let coin = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::CoinFlip)
        .expect("a well-formed hypothesis");
    let ratio = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::MatchedRatio)
        .expect("a well-formed hypothesis");
    let exposure = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::MatchedExposure)
        .expect("a well-formed hypothesis");

    // The method is long-only, which is what makes it the case the control was
    // written for.
    assert_eq!(coin.long_share, 1.0, "the fixture is long-only");
    assert!(side_ratio_is_one_sided(coin.long_share));

    // The defect: the coin's control is about half long whatever the method
    // did, so its expected drift term is about zero where the method's was not.
    assert!(
        (coin.null_long_share_median() - 0.5).abs() < 0.1,
        "the coin-flip control should sit near 0.50 long: {:.3}",
        coin.null_long_share_median(),
    );
    assert!(!coin.side_matched(), "a 50%-long control against a 100%-long method is not a side match");
    assert!(
        coin.signed_minutes > 0.0,
        "a long-only method on these bars holds positive signed time: {:.0} long-minutes",
        coin.signed_minutes,
    );

    // The ratio match fixes the ratio and NOT the exposure — stated here so
    // that the reason the exposure setting exists is on the record as a
    // measurement rather than as an argument.
    assert!(
        ratio.side_matched(),
        "the ratio-matched control's long share is {:.3} against the method's {:.3}, further than {SIDE_MATCH_BAND}",
        ratio.null_long_share_median(),
        ratio.long_share,
    );
    assert!(
        !ratio.exposure_matched(),
        "the ratio match alone should NOT match the exposure on this fixture: signed share {:+.3} vs the null's {:+.3}, time in market ratio {:.2}",
        ratio.signed_share(),
        ratio.null_signed_share_median(),
        ratio.time_in_market_match(),
    );

    // And the exposure match fixes both.
    assert!(exposure.side_matched(), "the side ratio: {:.3} vs {:.3}", exposure.null_long_share_median(), exposure.long_share);
    assert!(
        exposure.exposure_matched(),
        "the control's signed share of time is {:+.3} against the method's {:+.3} and its time in the market is {:.2}x — outside the band",
        exposure.null_signed_share_median(),
        exposure.signed_share(),
        exposure.time_in_market_match(),
    );
}

/* ------------------------------------------- the instrument, not a property */

/// Falsifier 1 of the registration: a method whose own long share is inside
/// 40-60% must be read against the same null it always was.
///
/// Not "about the same": the control's parameters are built from a measured
/// share that is not 0.5, so the runs differ bar for bar — what must hold is
/// that the null's own distribution does not move materially and the verdict
/// does not change. The percentile is asserted within the seed-to-seed noise of
/// the null it is read against rather than bit-for-bit, which is the honest
/// claim and is what the registration committed to.
#[test]
fn an_even_method_is_read_against_the_same_null_it_always_was() {
    let (rules, bars, registry, gate) = (rules(), drifting_bars(40_000, 0.03), Registry::with_builtins(), PromisingGate::default());
    // `ema-cross` on a drifting stream takes both sides and takes plenty.
    let h = hypothesis("even", "ema-cross", Vec::new(), &[]);

    let coin = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::CoinFlip)
        .expect("a well-formed hypothesis");
    let exposure = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::MatchedExposure)
        .expect("a well-formed hypothesis");

    assert!(coin.oos.trades > 100, "too thin a fixture: {} trades", coin.oos.trades);
    assert!(
        !side_ratio_is_one_sided(coin.long_share),
        "this test needs an even method and {} is {:.3} long — pick another base rather than widening the band",
        h.base,
        coin.long_share,
    );

    let (p50_coin, p50_exp) = (coin.null_quantile(0.5), exposure.null_quantile(0.5));
    assert!(
        (p50_coin - p50_exp).abs() < 0.05,
        "an even row's null median moved {p50_coin:.3} -> {p50_exp:.3}; the drift control reached something other than \
         the sides",
    );
    assert_eq!(
        coin.verdict.promising, exposure.verdict.promising,
        "and the gate verdict is the method's, not the control's",
    );
    assert_eq!(coin.oos.profit_factor.to_bits(), exposure.oos.profit_factor.to_bits(), "no gate figure may move");
}

/// Falsifier 5, and the thing `agent/repair-a` found and called "a defect in my
/// own control" before it was cut off: **the ratio-matched null collapses to a
/// single point** on a long-only hold row, every seed replaying the same run, so
/// its "percentile" is one comparison dressed as a quantile.
///
/// Measured here, the collapse is reproduced and then explained, and the
/// explanation is not the one that branch guessed. It is not a property of the
/// row. It is a property of a control that matches the side ratio and leaves
/// `entryRate` at 1.0: at that rate the control re-enters on the first bar its
/// filters allow after every exit, so its entry draw decides nothing and its
/// side draw decides nothing either once the share is 0 or 1 — there is no
/// random input left. The count match is what repairs it, because a rate below
/// 1.0 is a timing lottery again, and this is the second reason the exposure
/// setting exists rather than a lucky side effect of the first.
///
/// Both halves are asserted, and so is the reporting rule: a null with no
/// spread reports `null`, not 0 and not 100.
#[test]
fn a_fully_invested_one_sided_method_has_no_null_with_a_spread() {
    let (rules, bars, registry, gate) = (rules(), drifting_bars(40_000, 0.03), Registry::with_builtins(), PromisingGate::default());
    // Long the whole day, every day: buy-and-hold with an overnight flat, the
    // simplest long-only hold row there is.
    let h = hypothesis("long-day", "session-hold", Vec::new(), &[("from", 0.0), ("to", 2345.0), ("side", 1.0)]);

    let coin = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::CoinFlip)
        .expect("a well-formed hypothesis");
    let ratio = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::MatchedRatio)
        .expect("a well-formed hypothesis");
    let exposure = run_hypothesis_fixed_sides(&registry, &h, &bars, &rules, &gate, SEEDS, None, NullSides::MatchedExposure)
        .expect("a well-formed hypothesis");

    assert_eq!(coin.long_share, 1.0, "the fixture is long-only");

    // Against a coin the null looks like a distribution, and the whole of its
    // spread is the side lottery the method never faced.
    assert!(coin.null_has_spread(), "the coin-flip null's spread is the side lottery, and it is there");
    assert!(coin.percentile_or_null().is_some(), "so the record printed a percentile for this row");

    // Ratio-matched and nothing else: the collapse, reproduced.
    assert!(
        !ratio.null_has_spread(),
        "the ratio-matched null should collapse to a single point on this row; p50 {:.4} p95 {:.4}",
        ratio.null_quantile(0.5),
        ratio.null_quantile(0.95),
    );
    assert_eq!(
        ratio.percentile_or_null(),
        None,
        "a null with no spread reports `null`, not a percentile — and the raw field was {:.0}",
        ratio.percentile,
    );
    assert!(!ratio.survives(), "and a row with no percentile cannot survive on one");

    // Count-matched as well: a rate below 1.0 is a timing lottery again, so the
    // null is a distribution and the comparison means something.
    assert!(
        exposure.null_has_spread(),
        "the count match should restore the spread the ratio match destroyed; p50 {:.4} p95 {:.4}",
        exposure.null_quantile(0.5),
        exposure.null_quantile(0.95),
    );
    assert!(exposure.side_matched(), "while still taking the method's side ratio: {:.3}", exposure.null_long_share_median());
    assert!(exposure.percentile_or_null().is_some(), "so this row has a percentile to read");
}
