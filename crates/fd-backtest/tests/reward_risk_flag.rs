//! What `--reward-risk=` is allowed to move, and what it must not.
//!
//! The flag added on 2026-10-11 (`docs/decisions/2026-10-11-reward-risk.md`)
//! overwrites `TradingRules::reward_risk` for one run. Four claims were made
//! for it and none may be left as prose:
//!
//! 1. **Passing no flag changes nothing.** The ratio still resolves out of
//!    `[trading] reward_risk`, so every receipt in `docs/decisions/`
//!    reproduces. What is pinned here is the stronger statement — that the
//!    resolved default produces trades **bit-identical** to a `1.8` literal,
//!    field by field, not merely ones that round the same way.
//! 2. **It reaches a derived target and nothing else.** The entry builder
//!    (`engine.rs:737-744`) takes an **explicit** target first and refuses to
//!    derive one for a `self_managed` position at all, so neither a row that
//!    carries its own target nor a self-managed row can be moved at any value.
//!    Measured here rather than read off the source, because addendum 11 §IV.B
//!    attributes `agent/hold-cap`'s ceiling to this very number on three rows
//!    that all carry their own.
//! 3. **`0` is no target at all**, not "a target at zero distance" and not
//!    absent (`engine.rs:740`).
//! 4. **It moves the matched NULL**, on every row, including rows where it
//!    moves zero trades of the method — because `control.rs:150` enters with
//!    `target: None` under the default `Exits::Engine`. That is not a defect
//!    in the flag; it is the reason no percentile in that job is published
//!    across `reward_risk` values, and it is asserted here so the reason
//!    cannot quietly stop being true.
//!
//! Claims 2 and 3 are NEGATIVES, and a negative from a blind instrument is
//! worthless, so every assertion that nothing moved sits in the same test as a
//! derived-target control on the identical bars that the same two values move a
//! great deal. `a_probe_that_is_invariant_for_the_wrong_reason_is_not_evidence`
//! goes one step further: it builds a fixture that reads invariant for a reason
//! that is NOT causal, and asserts that it does — so that "it did not move" is
//! never read as evidence on its own.

use fd_backtest::engine::{ExitKind, Range, Trade, TradingRules, run_backtest};
use fd_backtest::control::RandomEntry;
use fd_core::types::Bar;
use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

const MINUTE: i64 = 60_000;
const FOUR_HOURS: i64 = 14_400_000;

/// The ratio the shipped `[trading]` table holds, stated so a change to it
/// breaks this file rather than quietly restating what "the default" means.
const SHIPPED_DEFAULT_RR: f64 = 1.8;

/// `Exits::Engine`, `target: None`: the row shape the flag exists for. The
/// engine derives its target at `risk x reward_risk`, so this is the control
/// that proves the instrument can see the flag at all.
struct DerivedTarget;

impl Strategy for DerivedTarget {
    fn id(&self) -> &'static str {
        "test-rr-derived-target"
    }
    fn name(&self) -> &'static str {
        "derived target"
    }
    fn description(&self) -> &'static str {
        ""
    }
    fn default_params(&self) -> Params {
        Params::new(&[("atrPeriod", 14.0)])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn warmup(&self, _: &Params) -> usize {
        16
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 10.0), target: None, reason: "derived".into() }
    }
}

/// `Exits::Engine` with its OWN target, the shape `orb`, `keltner-break` and
/// `far-stop-break` all have. `engine.rs:737` takes the explicit one, so
/// `reward_risk` is never read.
struct OwnTarget;

impl Strategy for OwnTarget {
    fn id(&self) -> &'static str {
        "test-rr-own-target"
    }
    fn name(&self) -> &'static str {
        "own target"
    }
    fn description(&self) -> &'static str {
        ""
    }
    fn default_params(&self) -> Params {
        Params::new(&[("atrPeriod", 14.0)])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn warmup(&self, _: &Params) -> usize {
        16
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        // 1.5 R of its own, deliberately NOT the shipped 1.8: if the flag ever
        // reached this row the trades would move toward whatever was passed.
        Intent::Enter {
            side: Side::Long,
            stop: Some(ctx.bar.close - 10.0),
            target: Some(ctx.bar.close + 15.0),
            reason: "own".into(),
        }
    }
}

/// `Exits::Strategy`: the stop is a sizing unit and the strategy owns every
/// exit, so the engine never places a target for it (`engine.rs:740`).
struct SelfManaged;

impl Strategy for SelfManaged {
    fn id(&self) -> &'static str {
        "test-rr-self-managed"
    }
    fn name(&self) -> &'static str {
        "self managed"
    }
    fn description(&self) -> &'static str {
        ""
    }
    fn default_params(&self) -> Params {
        Params::new(&[("atrPeriod", 14.0)])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }
    fn warmup(&self, _: &Params) -> usize {
        16
    }
    fn exits(&self) -> Exits {
        Exits::Strategy
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        match ctx.position {
            Some(_) if ctx.i % 40 == 0 => Intent::Exit { reason: "bar 40".into() },
            Some(_) => Intent::None,
            None => {
                Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 10.0), target: None, reason: "sm".into() }
            }
        }
    }
}

/// No spread and no commission, so nothing but the rules under test can move a
/// number, and a four-hour clock so an unreached target still ends the trade.
fn rules(reward_risk: f64) -> TradingRules {
    TradingRules {
        contract_size: 1.0,
        spread: 0.0,
        commission_per_lot: 0.0,
        starting_equity_usd: 10_000.0,
        risk_per_trade_pct: 0.01,
        reward_risk,
        max_hold_ms: FOUR_HOURS,
        ..TradingRules::default()
    }
}

/// 1,600 fifteen-minute bars oscillating about $4,000 with an amplitude that
/// reaches a 0.9 R target often, a 1.8 R one less often and a 7.2 R one rarely.
/// A monotone drift would make every target reachable and hide the whole
/// question; a flat tape would make none reachable, which is a different test
/// (`a_probe_that_is_invariant_for_the_wrong_reason_is_not_evidence`).
fn bars() -> Vec<Bar> {
    (0..1600)
        .map(|i| {
            let px = 4000.0 + 40.0 * ((i as f64) * 0.37).sin() + 18.0 * ((i as f64) * 0.11).cos();
            Bar { time: i * 15 * MINUTE, open: px, high: px + 1.5, low: px - 1.5, close: px, volume: None }
        })
        .collect()
}

/// A flat tape: no stop and no target is EVER reachable, so every position is
/// cut on the clock whatever `reward_risk` says. Used only by the blind-probe
/// test.
fn flat_bars() -> Vec<Bar> {
    (0..1600)
        .map(|i| Bar { time: i * 15 * MINUTE, open: 4000.0, high: 4000.2, low: 3999.8, close: 4000.0, volume: None })
        .collect()
}

/// Every `f64` by its bits, every integer by its value: the comparison that
/// rounding cannot hide a difference behind.
fn fingerprint(trades: &[Trade]) -> Vec<String> {
    trades
        .iter()
        .map(|t| {
            format!(
                "{:?}|{}|{:016x}|{}|{:016x}|{:016x}|{:?}|{:016x}|{:016x}|{:016x}|{:016x}|{:016x}|{}|{}",
                t.direction,
                t.entry_time,
                t.entry_price.to_bits(),
                t.exit_time,
                t.exit_price.to_bits(),
                t.stop.to_bits(),
                t.target.map(f64::to_bits),
                t.lots.to_bits(),
                t.pnl_usd.to_bits(),
                t.swap_usd.to_bits(),
                t.r.to_bits(),
                t.mae.to_bits(),
                t.hold_ms,
                t.exit_reason,
            )
        })
        .collect()
}

/// The same thing WITHOUT the recorded `target` field.
///
/// Needed because of something this job measured rather than expected: on a
/// tape that never reaches a target, `--reward-risk=0` changes the `target`
/// field of every trade (`Some(..)` to `None`) while changing **no outcome** —
/// not an entry, not an exit, not a dollar. So a diff-based invariance claim
/// has two different strengths, and the blind probe below needs the weaker one
/// to say what it is blind about. A test that only ever compared the full
/// fingerprint would have read that bookkeeping change as a behaviour change.
fn outcome_fingerprint(trades: &[Trade]) -> Vec<String> {
    trades
        .iter()
        .map(|t| {
            format!(
                "{:?}|{}|{:016x}|{}|{:016x}|{:016x}|{:016x}|{:016x}|{:016x}|{:016x}|{:016x}|{}|{}|{:?}",
                t.direction,
                t.entry_time,
                t.entry_price.to_bits(),
                t.exit_time,
                t.exit_price.to_bits(),
                t.stop.to_bits(),
                t.lots.to_bits(),
                t.pnl_usd.to_bits(),
                t.swap_usd.to_bits(),
                t.r.to_bits(),
                t.mae.to_bits(),
                t.hold_ms,
                t.exit_reason,
                t.exit_kind,
            )
        })
        .collect()
}

fn run(s: &dyn Strategy, bars: &[Bar], rr: f64) -> fd_backtest::engine::BacktestResult {
    run_backtest(bars, s, &s.default_params(), &rules(rr), None, Range::default())
}

/// **CLAIM 1.** The shipped default and a `1.8` literal are the same run, to
/// the bit.
///
/// This is the test that makes the flag safe to add. The whole-output proof is
/// the five captured fixtures in `receipts/2026-10-11-reward-risk/`; this is
/// the trade-level half of it, and if it ever fails, falsifier F4 of the record
/// says to publish no measurement at all.
#[test]
fn the_default_ratio_is_bit_identical_to_a_literal() {
    // Read from the shipped config rather than from a constant, so an edit to
    // `config/default.toml` breaks this test instead of sliding past it.
    let config = fd_core::config::Config::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config"),
    )
    .expect("the workspace config");
    assert_eq!(
        config.trading.reward_risk, SHIPPED_DEFAULT_RR,
        "[trading] reward_risk must still be 1.8 — if it is not, every receipt in docs/decisions/ has moved",
    );
    assert_eq!(
        TradingRules::default().reward_risk, SHIPPED_DEFAULT_RR,
        "and the registry default must agree with it, or the two disagree about what 'default' means",
    );
    let bars = bars();
    let from_default = run(&DerivedTarget, &bars, config.trading.reward_risk);
    let from_literal = run(&DerivedTarget, &bars, SHIPPED_DEFAULT_RR);

    assert!(from_default.trades.len() > 40, "the fixture must carry a real book: {}", from_default.trades.len());
    assert_eq!(fingerprint(&from_default.trades), fingerprint(&from_literal.trades));
    assert_eq!(from_default.metrics.expectancy.to_bits(), from_literal.metrics.expectancy.to_bits());
    assert_eq!(from_default.metrics.profit_factor.to_bits(), from_literal.metrics.profit_factor.to_bits());
    assert_eq!(from_default.metrics.total_r.to_bits(), from_literal.metrics.total_r.to_bits());
    assert_eq!(from_default.metrics.max_drawdown_usd.to_bits(), from_literal.metrics.max_drawdown_usd.to_bits());
}

/// **CLAIM 2, with its positive control asserted FIRST in the same test.**
///
/// A derived-target row is a different book at every value. On the identical
/// bars, a row carrying its own target and a self-managed row are ONE run
/// repeated: 0 trades change, bit for bit. The control goes first so a day when
/// the flag stops working cannot read as a day when the two guarantees held.
#[test]
fn the_flag_moves_a_derived_target_and_leaves_an_explicit_one_and_a_self_managed_one_alone() {
    let bars = bars();

    // THE CONTROL FIRST. Derived target: four ratios, four books.
    let d0 = run(&DerivedTarget, &bars, 0.0);
    let d09 = run(&DerivedTarget, &bars, 0.9);
    let d18 = run(&DerivedTarget, &bars, 1.8);
    let d72 = run(&DerivedTarget, &bars, 7.2);
    assert_ne!(fingerprint(&d09.trades), fingerprint(&d18.trades), "the instrument must see the flag at all");
    assert_ne!(fingerprint(&d18.trades), fingerprint(&d72.trades), "a wider target must be a different book");
    assert_ne!(fingerprint(&d18.trades), fingerprint(&d0.trades), "removing the target must be a different book");
    // And it moves in the direction the arithmetic demands: a nearer target is
    // reached more often. This is the half that catches a flag which is read
    // but applied to the wrong field.
    let targets = |r: &fd_backtest::engine::BacktestResult| {
        r.trades.iter().filter(|t| t.exit_kind == ExitKind::Target).count()
    };
    assert!(
        targets(&d09) > 0 && targets(&d09) > targets(&d18) && targets(&d18) >= targets(&d72),
        "TARGET count must fall as the ratio widens: 0.9 -> {}, 1.8 -> {}, 7.2 -> {}",
        targets(&d09),
        targets(&d18),
        targets(&d72),
    );
    assert_eq!(targets(&d0), 0, "and at 0 there is no target to reach");

    // NOW THE NEGATIVES, on the identical bars. These use the FULL
    // fingerprint, which is the stronger of the two invariance senses: not
    // even the recorded target field moves. The blind probe below is invariant
    // only in the weaker, outcome sense, and that is the difference between
    // these rows and a row the flag simply cannot reach on a given tape.
    let o18 = run(&OwnTarget, &bars, 1.8);
    assert!(o18.trades.len() >= 40, "the explicit-target fixture must take a real book: {}", o18.trades.len());
    for rr in [0.0, 0.9, 3.6, 7.2] {
        assert_eq!(
            fingerprint(&run(&OwnTarget, &bars, rr).trades),
            fingerprint(&o18.trades),
            "a row carrying its own target may not move at reward_risk {rr}",
        );
    }

    let s18 = run(&SelfManaged, &bars, 1.8);
    assert!(s18.trades.len() >= 40, "the self-managed fixture must take a real book: {}", s18.trades.len());
    for rr in [0.0, 0.9, 3.6, 7.2] {
        assert_eq!(
            fingerprint(&run(&SelfManaged, &bars, rr).trades),
            fingerprint(&s18.trades),
            "a self-managed row may not move at reward_risk {rr}",
        );
    }
    assert!(
        s18.trades.iter().all(|t| t.target.is_none()),
        "the engine must never place a target on a self-managed position",
    );
    assert!(
        s18.trades.iter().all(|t| t.exit_kind != ExitKind::Target),
        "a self-managed position is never closed on a target, so TARGET must not appear at all",
    );
}

/// **CLAIM 3.** `0` is no target at all — not a target at zero distance, and
/// not the absent value.
#[test]
fn a_zero_ratio_removes_the_target_rather_than_placing_one_at_the_entry() {
    let bars = bars();
    let zero = run(&DerivedTarget, &bars, 0.0);
    assert!(zero.trades.len() >= 40, "{} trades", zero.trades.len());
    assert!(zero.trades.iter().all(|t| t.target.is_none()), "reward_risk 0 must leave target unset");
    assert_eq!(
        zero.trades.iter().filter(|t| t.exit_kind == ExitKind::Target).count(),
        0,
        "nothing may close on a target that was never placed",
    );
    // And the trades still happen and still end — on the stop or on the clock.
    assert!(
        zero.trades.iter().any(|t| t.exit_kind == ExitKind::Stop)
            && zero.trades.iter().any(|t| t.exit_kind == ExitKind::Timeout),
        "with no target the two remaining engine exits must both still fire",
    );
    // A target at zero distance would be reached by every bar, so it would read
    // as a book of near-zero-R TARGET exits. Pinned as the thing 0 is NOT.
    let tiny = run(&DerivedTarget, &bars, 0.001);
    assert!(
        tiny.trades.iter().filter(|t| t.exit_kind == ExitKind::Target).count() > zero.trades.len() / 2,
        "a ratio just above zero is a target at the entry, which is what 0 must NOT mean",
    );
}

/// **CLAIM 4. The flag re-specifies the matched NULL.**
///
/// `control.rs:150` enters with `target: None` and `RandomEntry` takes the
/// default `Exits::Engine` (`registry.rs:256`), so the control the percentiles
/// are read against is a different control at every `reward_risk`. This is
/// asserted — not hoped — because the conclusion that rests on it is
/// "publish no percentile across these values", and that conclusion would
/// silently become wrong if the null ever stopped deriving a target.
#[test]
fn the_null_is_a_different_control_at_every_ratio() {
    let bars = bars();
    let p = Params::new(&[("seed", 7.0), ("entryRate", 0.05), ("longShare", 0.5), ("stopAtr", 1.5), ("atrPeriod", 14.0)]);
    let null = |rr: f64| run_backtest(&bars, &RandomEntry, &p, &rules(rr), None, Range::default());

    let n18 = null(1.8);
    assert!(n18.trades.len() >= 40, "the null fixture must take a real book: {}", n18.trades.len());
    assert!(n18.trades.iter().any(|t| t.target.is_some()), "the null derives a target, which is the whole point");
    assert!(
        n18.trades.iter().any(|t| t.exit_kind == ExitKind::Target),
        "and it closes on that target, so moving the ratio moves the control",
    );
    for rr in [0.0, 0.9, 3.6, 7.2] {
        assert_ne!(
            fingerprint(&null(rr).trades),
            fingerprint(&n18.trades),
            "the matched null must be acknowledged as moving at reward_risk {rr}",
        );
    }
    assert!(
        null(0.0).trades.iter().all(|t| t.target.is_none()),
        "at 0 the control loses its target too, so a method with no target is compared against a control with none",
    );
}

/// **THE BLIND PROBE.** A fixture that is invariant FOR THE WRONG REASON, and
/// the assertion that it is.
///
/// On a flat tape no target is ever reached, so a derived-target row produces
/// the same BOOK at every `reward_risk` — the same trades, the same dollars,
/// the same exits — exactly like the explicit-target and self-managed rows in
/// the test above, and for a reason that has nothing to do with the causal
/// path. Without this test, "it did not move" would read as evidence of a
/// mechanism when it is sometimes only evidence of a tape.
///
/// **And the probe caught a second thing, which is why it is written down.**
/// The first version of this test compared the full fingerprint and FAILED at
/// `reward_risk = 0` — because at 0 the engine records `target: None` where at
/// 1.8 it records `Some(..)`, on trades whose entry, exit, R and dollars are
/// bit-identical. So "invariant" has two strengths that do not coincide:
///
/// * **outcome-invariant** — not a trade, not a dollar, not an exit moved.
///   The explicit-target and self-managed rows are invariant in BOTH senses.
///   This probe is invariant only in this one.
/// * **byte-invariant** — the recorded fields are identical too.
///
/// A diff of receipts cannot tell those apart, and this probe is the fixture
/// where they come apart. That is reported in the decision record rather than
/// smoothed over.
#[test]
fn a_probe_that_is_invariant_for_the_wrong_reason_is_not_evidence() {
    let flat = flat_bars();
    let base = run(&DerivedTarget, &flat, 1.8);
    assert!(base.trades.len() >= 40, "the flat fixture must still take a book: {}", base.trades.len());

    // OUTCOME-invariant at every value — and NOT because this row is outside
    // the causal path.
    for rr in [0.0, 0.9, 3.6, 7.2] {
        assert_eq!(
            outcome_fingerprint(&run(&DerivedTarget, &flat, rr).trades),
            outcome_fingerprint(&base.trades),
            "the blind probe is expected to produce the same book at reward_risk {rr}; that is what makes it blind",
        );
    }
    // The non-causal reason, stated as a number: nothing is ever reached, so
    // every trade is cut on the clock.
    // Every exit is the clock except the last, which the tape running out
    // closes — so the assertion is "no price-based exit ever fired", which is
    // the statement that makes the invariance a property of the tape.
    assert!(
        base.trades.iter().all(|t| t.exit_kind == ExitKind::Timeout || t.exit_kind == ExitKind::EndOfData),
        "the probe's invariance is a property of the tape: no stop and no target may ever be reached, but the \
         exits were {:?}",
        base.trades.iter().map(|t| format!("{:?}", t.exit_kind)).collect::<std::collections::BTreeSet<_>>(),
    );
    assert_eq!(
        base.trades.iter().filter(|t| t.exit_kind == ExitKind::EndOfData).count(),
        1,
        "exactly one trade is closed by the tape ending; the rest are the clock",
    );
    // THE SECOND FINDING: at 0 the row is outcome-invariant and NOT
    // byte-invariant. The engine stops recording a target it was never going to
    // reach. Measured, and kept as an assertion so it cannot silently change.
    let zero = run(&DerivedTarget, &flat, 0.0);
    assert_ne!(
        fingerprint(&zero.trades),
        fingerprint(&base.trades),
        "at 0 the recorded target field must differ even though the book does not",
    );
    assert!(base.trades.iter().all(|t| t.target.is_some()), "at 1.8 a target is recorded on every trade");
    assert!(zero.trades.iter().all(|t| t.target.is_none()), "at 0 none is");
    // And the non-zero values ARE byte-invariant here, which is what makes the
    // line above a statement about 0 and not about the probe being noisy.
    for rr in [0.9, 3.6, 7.2] {
        assert_ne!(
            fingerprint(&run(&DerivedTarget, &flat, rr).trades),
            fingerprint(&base.trades),
            "a different positive ratio records a different target price, even unreached: {rr}",
        );
    }

    // And the proof that invariance alone cannot be read as evidence: this row
    // IS in the family by the source rule, while the explicit-target row of the
    // test above is NOT — yet both produce the same book across every value.
    assert_eq!(DerivedTarget.exits(), Exits::Engine, "the probe is an engine-exit row by the family rule");

    // The paired POSITIVE control: the identical row on a tape that reaches
    // targets does move, in both senses. Without this the test above could pass
    // on an engine that ignores reward_risk everywhere.
    let real = bars();
    assert_ne!(
        outcome_fingerprint(&run(&DerivedTarget, &real, 0.9).trades),
        outcome_fingerprint(&run(&DerivedTarget, &real, 7.2).trades),
        "the same row on a tape that reaches targets must produce a different book",
    );
}
