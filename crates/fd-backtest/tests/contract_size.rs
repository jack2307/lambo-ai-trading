//! A trade carries the contract size it was sized under, and no later
//! correction to the config can restate what it earned.
//!
//! This gate exists because the desk shipped the opposite and published the
//! result. `eur-hours` sized a short of 256.3 lots at 07:00 UTC on
//! 2026-09-16, when `config/default.toml` declared the euro's contract size
//! 1.0; commit cd92998 corrected it to the measured 1000.0 at 08:53 UTC; the
//! position closed at 15:00 UTC and was booked at $176.85 on a $100 book for
//! a move worth $0.18. Neither contract size was wrong. The defect was that
//! the trade did not carry the one it was born with.
//!
//! `docs/decisions/2026-09-21-restated-pnl-contract-size.md`, and
//! `docs/decisions/2026-09-17-unit-carrying.md` rule 4: a conversion is
//! pinned by a test at the boundary, and the test says what would reopen it.

use fd_backtest::engine::{ExitKind, TradingRules, close_position, open_position};
use fd_strategy::registry::Side;

/// The euro as `config/default.toml` declared it BEFORE cd92998, with every
/// cost off so the arithmetic below is only the one term under test.
fn euro(contract_size: f64) -> TradingRules {
    TradingRules {
        contract_size,
        spread: 0.0,
        commission_per_lot: 0.0,
        swap_long_per_lot: 0.0,
        swap_short_per_lot: 0.0,
        lot_step: 0.01,
        min_lot: 0.01,
        price_decimals: 5,
        risk_per_trade_pct: 0.01,
        max_hold_ms: 0,
        ..TradingRules::default()
    }
}

/// The eur-hours position, reproduced: $100 of equity, a 39-pip stop, a
/// short at 1.1546 closed at 1.15391.
fn eur_hours_short(rules: &TradingRules) -> fd_backtest::engine::Live {
    let (position, sized_down) = open_position(
        Side::Short,
        Some(1.1585),
        None,
        "window 0245-1045 New York".into(),
        1_789_542_000_000,
        1.1546,
        None,
        100.0,
        rules,
        true,
        None,
    )
    .expect("the position opens");
    assert!(!sized_down, "no guards here, so nothing sizes it down");
    position
}

#[test]
fn a_trade_is_booked_at_the_contract_size_it_was_sized_under() {
    // Sized under 1.0, exactly as the book was on the morning of 2026-09-16.
    let sizing = euro(1.0);
    let position = eur_hours_short(&sizing);
    // The book recorded 256.3; reconstructing it from the record's own
    // five-decimal prices gives 256.41, and the tenth of a lot is that
    // rounding. The factor under test is a thousand, not a tenth.
    assert!(
        (position.lots - 256.3).abs() < 0.2,
        "$1 of risk over a 0.0039 stop at contract 1.0 is about 256.3 lots, got {}",
        position.lots
    );
    assert_eq!(position.contract_size, Some(1.0), "the position carries its sizing basis");
    assert_eq!(position.spread, Some(0.0), "and the spread it was charged");

    // Now close it under the CORRECTED config, which is what happened: the
    // fix landed at 08:53 UTC and the position closed at 15:00 UTC.
    let corrected = euro(1000.0);
    let trade = close_position(
        position,
        1.15391,
        1_789_570_800_000,
        ExitKind::Signal,
        "window closed",
        &corrected,
        "window 0245-1045 New York",
    );

    // 0.00069 x 256.3 x 1.0. The number the book actually earned.
    assert!(
        (trade.pnl_usd - 0.18).abs() < 0.005,
        "a 0.00069 move on 256.3 lots of a 1.0 contract is $0.18, not ${}",
        trade.pnl_usd
    );
    // The figure this gate exists to refuse. It was published.
    assert!(
        (trade.pnl_usd - 176.85).abs() > 1.0,
        "the trade was restated at the corrected contract size: ${}",
        trade.pnl_usd
    );
    assert_eq!(trade.contract_size, Some(1.0), "and the basis travels onto the closed trade");
    assert_eq!(trade.spread, Some(0.0));

    // R was always right, which is why nothing caught this: it is
    // points/risk and the contract size cancels out of it entirely. A book
    // reporting 0.18 R and $176.85 on $100 of equity was saying two
    // contradictory things and only the dollar figure was read.
    assert!((trade.r - 0.1769).abs() < 0.001, "R is points over risk and does not move: {}", trade.r);
}

#[test]
fn a_position_with_no_recorded_basis_falls_back_and_says_it_did() {
    // A position reloaded from a state file written before the field
    // existed. The current rules stand in — there is nothing else to use —
    // but the trade records `None`, so nothing downstream can mistake the
    // fallback for a measurement.
    let rules = euro(1000.0);
    let mut position = eur_hours_short(&euro(1.0));
    position.contract_size = None;
    position.spread = None;

    let trade =
        close_position(position, 1.15391, 1_789_570_800_000, ExitKind::Signal, "window closed", &rules, "reloaded");

    assert!((trade.pnl_usd - 176.85).abs() < 0.2, "priced from the current rules: ${}", trade.pnl_usd);
    assert_eq!(trade.contract_size, None, "absent, not 1000.0 — the basis is unknown and stays unknown");
    assert_eq!(trade.spread, None);
}

#[test]
fn the_euro_and_silver_contract_sizes_stay_as_measured() {
    // MEASURED on the live account 2026-09-16 (`symbol_info.trade_contract_size`)
    // and written into the config by cd92998. These values must stay until
    // they are re-measured, not until a result looks better without them.
    // Changing one of these numbers restates nothing any more — that is what
    // the rest of this file is for — but it still charges every future trade
    // the wrong size.
    let config = fd_core::config::Config::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config"),
    )
    .expect("the workspace config loads");
    // `xagduka` is the silver block; it carries `symbol = "XAGUSD.sc"` and
    // is the one the commit's table calls XAGUSD.
    for (market, want) in [("eurusd", 1000.0), ("eurduka", 1000.0), ("xagduka", 50.0)] {
        let rules = fd_backtest::engine::trading_rules_for(&config, market).expect("the market is configured");
        assert!(
            (rules.contract_size - want).abs() < f64::EPSILON,
            "{market}: contract size is {} and must stay {want} until re-measured on the account",
            rules.contract_size
        );
    }
}
