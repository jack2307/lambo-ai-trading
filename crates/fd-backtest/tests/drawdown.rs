//! `max_drawdown_usd` / `max_drawdown_pct`: the risk figure the gate never
//! printed, checked against sequences whose answer is known by arithmetic.
//!
//! These two fields have been in `engine.rs::Metrics` the whole time and
//! `--mode=hypotheses` never printed either of them, so across ~9,858
//! published rows **nobody has ever read them** — and a quantity nobody has
//! read is a quantity nobody has checked. This file checks them before the
//! printer puts them beside every profit figure in the record.
//! `docs/decisions/2026-10-07-drawdown-printed.md`.

use fd_backtest::engine::{ExitKind, Metrics, Trade, metrics_of};
use fd_strategy::registry::Side;

/// A closed trade whose only interesting property is its USD result.
///
/// `metrics_of` reads `pnl_usd` and nothing else for its equity walk, so a
/// hand-built list of results is exactly the input the drawdown arithmetic
/// sees. `mae` is deliberately a large negative on every one of them: it is
/// what separates a CLOSED-EQUITY drawdown from an intrabar one, and the test
/// below uses it to pin which of the two this is.
fn result_of(pnl_usd: f64) -> Trade {
    Trade {
        direction: Side::Long,
        entry_time: 0,
        entry_price: 2000.0,
        exit_time: 60_000,
        exit_price: 2000.0,
        exit_reason: "TARGET".into(),
        exit_kind: ExitKind::Signal,
        stop: 1990.0,
        target: None,
        lots: 1.0,
        pnl_usd,
        swap_usd: 0.0,
        r: pnl_usd / 100.0,
        mae: -3.0,
        mfe: 3.0,
        hold_ms: 60_000,
        reason: "t".into(),
        contract_size: Some(1.0),
        spread: Some(0.0),
        risk_usd: Some(100.0),
        r_net: Some(pnl_usd / 100.0),
        banked_r: None,
    }
}

/// The same 10,000 USD book every published receipt is sized against
/// (`TradingRules::starting_equity_usd`).
fn curve(results: &[f64]) -> Metrics {
    metrics_of(&results.iter().copied().map(result_of).collect::<Vec<_>>(), 10_000.0)
}

/// A SEQUENCE WHOSE ANSWER IS KNOWN BY HAND.
///
/// ```text
/// start   10,000
/// +500    10,500   <- peak
/// -300    10,200
/// -200    10,000
/// +100    10,100
/// -400     9,700   <- trough
/// ```
///
/// The worst peak-to-trough fall is 10,500 - 9,700 = **800.00 USD**. No other
/// pair in the curve is further apart, because every later fall starts from a
/// lower peak. Note that the curve's NET result is -300 USD: the drawdown and
/// the net are different numbers and neither implies the other.
#[test]
fn the_known_drawdown_is_the_worst_peak_to_trough_fall() {
    let m = curve(&[500.0, -300.0, -200.0, 100.0, -400.0]);
    assert_eq!(m.max_drawdown_usd, 800.0, "10,500 peak to 9,700 trough is 800 USD, got {}", m.max_drawdown_usd);
    assert_eq!(m.net_pnl_usd, -300.0, "and the curve ends 300 USD down, which is a different number");
}

/// AND IT IS THE *WORST* FALL, NOT THE LAST ONE.
#[test]
fn a_later_smaller_fall_does_not_replace_the_worst_one() {
    let m = curve(&[-2_000.0, 2_500.0, -100.0]);
    assert_eq!(m.max_drawdown_usd, 2_000.0, "the first fall is the deepest, got {}", m.max_drawdown_usd);
}

/// ALL WINS: the drawdown is **0.00 USD** and the percent is **0**, not `NaN`.
///
/// This is the one place in this record where 0 is the honest answer rather
/// than a stand-in for "not measured": the fall WAS measured, over a curve
/// that never fell, and it was zero.
#[test]
fn an_all_winning_curve_has_a_zero_drawdown_and_not_a_nan() {
    let m = curve(&[100.0, 200.0, 300.0]);
    assert_eq!(m.max_drawdown_usd, 0.0, "nothing ever fell, got {}", m.max_drawdown_usd);
    assert!(m.max_drawdown_pct.is_finite(), "max_drawdown_pct must be a number, got {}", m.max_drawdown_pct);
    assert_eq!(m.max_drawdown_pct, 0.0, "and that number is 0, got {}", m.max_drawdown_pct);
}

/// ONE TRADE, both ways.
#[test]
fn a_single_trade_reports_its_own_loss_and_nothing_for_a_win() {
    let loser = curve(&[-250.0]);
    assert_eq!(loser.trades, 1);
    assert_eq!(loser.max_drawdown_usd, 250.0, "the only trade's loss IS the drawdown, got {}", loser.max_drawdown_usd);
    assert_eq!(loser.max_drawdown_pct, 2.5, "250 of a 10,000 peak is 2.5%, got {}", loser.max_drawdown_pct);

    let winner = curve(&[250.0]);
    assert_eq!(winner.max_drawdown_usd, 0.0, "a single winning trade never fell, got {}", winner.max_drawdown_usd);
    assert_eq!(winner.max_drawdown_pct, 0.0);
}

/// WHICH OF THE TWO DRAWDOWNS THIS IS — the question a reader of the new
/// column has to be able to answer.
///
/// Every trade here carries `mae = -3.0`, three R against the book while
/// open, and a curve of winners still reports a drawdown of 0.00 USD. So the
/// field is the fall of the **CLOSED-TRADE equity curve**, measured only at
/// closes, and it is **blind to intrabar excursion**. A position that went 3 R
/// against the book and came back to close green contributes nothing to it.
/// `avg_mae` is the only field that sees the open position.
#[test]
fn it_is_a_closed_equity_drawdown_and_not_an_intrabar_one() {
    let m = curve(&[100.0, 100.0, 100.0]);
    assert_eq!(m.avg_mae, -3.0, "each of these trades went 3 R against the book while open");
    assert_eq!(
        m.max_drawdown_usd, 0.0,
        "and the drawdown is still 0.00 USD: it is measured at closes only, got {}",
        m.max_drawdown_usd,
    );
}

/// THE DENOMINATOR, PINNED — AND IT IS NOT THE ONE A READER EXPECTS.
///
/// `max_drawdown_pct` is `max_dd / peak * 100` where `peak` is the **highest
/// equity the whole curve ever reached**, not the peak the drawdown actually
/// fell from. An early fall on a curve that later doubles is divided by the
/// later, larger number and so reads SMALLER than the account lived through:
///
/// ```text
/// start 10,000  ->  -1,000  ->   9,000   fell 1,000 of the 10,000 it had = 10.0%
///               -> +11,000  ->  20,000   new peak
/// printed:      1,000 / 20,000 = 5.0%    <- half of what was lived through
/// ```
///
/// The behaviour is asserted as it IS, because the number is about to be
/// printed beside every profit figure in the record and must be printed with
/// its true definition. It is a FLOOR on the conventional
/// max-drawdown-percent and can never overstate it. The USD field is exact
/// either way, which is why the USD field is the one the receipt quotes.
#[test]
fn the_percent_is_against_the_curves_highest_peak_not_the_peak_it_fell_from() {
    let m = curve(&[-1_000.0, 11_000.0]);
    assert_eq!(m.max_drawdown_usd, 1_000.0, "the USD figure is unambiguous and correct");
    assert_eq!(
        m.max_drawdown_pct, 5.0,
        "divided by the FINAL peak of 20,000, not the 10,000 it fell from, got {}",
        m.max_drawdown_pct,
    );
    assert_eq!(1_000.0 / 10_000.0 * 100.0, 10.0, "the conventional reading of the same curve is 10.0%");
}

/// AND THE USD FIGURE NEVER DEPENDS ON THE DENOMINATOR: the same 1,000 USD
/// fall reads 1,000 USD on three curves whose percents all differ.
#[test]
fn the_usd_figure_never_depends_on_the_denominator() {
    for results in [vec![-1_000.0, 11_000.0], vec![-1_000.0, 1_000.0], vec![-1_000.0]] {
        let m = curve(&results);
        assert_eq!(m.max_drawdown_usd, 1_000.0, "the same 1,000 USD fall on {results:?}");
    }
}

/// NO TRADES IS NOT A ZERO DRAWDOWN. `Metrics::empty` is what a cell that took
/// no position reports: its percent is `NaN` (correct — nothing was measured)
/// while its USD field reads 0.0 (not a measurement). A printer must therefore
/// gate the drawdown line on `trades > 0` rather than trust the USD field, and
/// the printer added on this branch does. Pinned so a later change cannot make
/// an empty cell look like a book that never fell.
#[test]
fn an_empty_cell_reports_a_nan_percent_and_must_be_gated_on_the_trade_count() {
    let m = Metrics::empty();
    assert_eq!(m.trades, 0);
    assert!(m.max_drawdown_pct.is_nan(), "an unmeasured percent is NaN, got {}", m.max_drawdown_pct);
    assert_eq!(m.max_drawdown_usd, 0.0, "the USD field, however, reads 0.0 and is NOT a measurement here");
}
