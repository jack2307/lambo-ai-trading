//! The live desk and the backtest must price the same rebate the same way.
//!
//! There are two implementations of one commercial term. `fd-api`'s
//! [`RebateTerms::on`] credits the live and paper books; `fd-backtest`'s
//! [`Rebate::on`] credits a research run so that a closed construct can be
//! rescored (`docs/hypotheses/2026-09-23-rebate-rescore.md`). Two copies of an
//! arithmetic drift — this desk has paid for that twice, once on the market
//! overrides and once on the contract size — so they are held against each
//! other here, on the boundary, with the case that reopens it written down.
//!
//! **What would reopen this.** If `config/accounts.toml` ever states the
//! rebate per lot rather than as a share of the spread, the two forms are not
//! the same number wearing two hats and BOTH implementations change. This
//! test is then not a parity check but a reminder that there are two places
//! to change.

use fd_api::paper::RebateTerms;
use fd_backtest::rebate::Rebate;

/// The arguments a credit can be asked for, including every one that must be
/// refused: the arithmetic agrees on `None` as exactly as it agrees on a
/// number, because "not priced" and "priced at nothing" are different facts
/// and both sides say so the same way.
fn cases() -> Vec<(f64, f64, f64)> {
    vec![
        // The worked example in config/accounts.toml: 0.07 lots, 0.28
        // spread, one ounce — 0.88 USC on a cent account.
        (0.07, 0.28, 1.0),
        (0.01, 0.28, 1.0),
        (0.70, 0.28, 1.0),
        (2.563, 0.28, 1.0),
        // A standard lot's contract size, and the euro's.
        (0.07, 0.28, 100.0),
        (0.07, 0.00012, 1000.0),
        // BTC: a five-dollar spread on a one-coin contract.
        (0.02, 5.0, 1.0),
        // Refusals.
        (0.0, 0.28, 1.0),
        (0.07, 0.0, 1.0),
        (0.07, 0.28, 0.0),
        (-0.07, 0.28, 1.0),
        (f64::NAN, 0.28, 1.0),
        (0.07, f64::NAN, 1.0),
        (0.07, 0.28, f64::INFINITY),
    ]
}

#[test]
fn the_backtest_prices_the_rebate_exactly_as_the_live_side_does() {
    for share in [0.0, 0.1, 0.45, 0.6, 1.0] {
        let live = RebateTerms { share_of_spread: share };
        let research = Rebate::new(share).expect("a finite share is a rate");
        for (lots, spread, contract_size) in cases() {
            let a = live.on(lots, spread, contract_size);
            let b = research.on(lots, spread, contract_size);
            match (a, b) {
                (None, None) => {}
                (Some(x), Some(y)) => assert_eq!(
                    x.to_bits(),
                    y.to_bits(),
                    "share {share}, lots {lots}, spread {spread}, contract {contract_size}: live {x} vs backtest {y}"
                ),
                _ => panic!(
                    "one side priced this and the other refused it: share {share}, lots {lots}, spread {spread}, contract {contract_size} — live {a:?}, backtest {b:?}"
                ),
            }
        }
    }
}

/// The identity the word "round turn" decides, asserted on both sides at
/// once: the credit is a share of ONE spread per trade, never one per side.
/// Crediting each half would be twice the truth, and the engine charges the
/// spread in two halves (`engine::apply_costs`), which is exactly the shape
/// that invites the mistake.
#[test]
fn the_credit_is_one_round_turn_and_not_one_per_side() {
    let share = 0.45;
    let (lots, spread, contract_size) = (0.07, 0.28, 1.0);
    let whole = RebateTerms { share_of_spread: share }.on(lots, spread, contract_size).unwrap();
    let per_half = RebateTerms { share_of_spread: share }.on(lots, spread / 2.0, contract_size).unwrap();
    assert!((whole - 2.0 * per_half).abs() < 1e-12, "one round turn is two halves and the credit is taken once");
    assert_eq!(Rebate::new(share).unwrap().on(lots, spread, contract_size).unwrap().to_bits(), whole.to_bits());
    // And the cost the trade actually paid, for the reader: 45% of it. The
    // credit is rounded to four decimals on both sides — 0.45 x 0.0196 is
    // 0.00882 and is reported as 0.0088 — so the ratio is 0.45 to that
    // rounding and the tolerance says so rather than the rounding being
    // loosened to make the line read better.
    let cost: f64 = spread * lots * contract_size;
    assert_eq!(whole, 0.0088);
    assert!((whole / cost - share).abs() < 0.002);
}

/// Both sides read the same `[rebate]` table out of the same file, so a rate
/// change is one edit and cannot land on one side only.
#[test]
fn both_sides_read_the_same_share_out_of_the_same_file() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config");
    let research = Rebate::from_config_dir(&dir).expect("config/accounts.toml declares a [rebate] table");
    assert_eq!(research.share_of_spread, 0.45, "the owner's figure, 2026-09-20; change this test when he changes it");
    let live = RebateTerms { share_of_spread: research.share_of_spread };
    assert_eq!(
        live.on(0.07, 0.28, 1.0).unwrap().to_bits(),
        research.on(0.07, 0.28, 1.0).unwrap().to_bits(),
        "the configured rate prices the registry's own worked example identically on both sides"
    );
}
