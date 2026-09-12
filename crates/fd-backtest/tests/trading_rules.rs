//! Trading rules come from the market, not from the top-level table.
//!
//! The top-level `[trading]` table describes gold. Reading it for BTC prices a
//! 1-BTC contract as 100 ounces and charges a $0.30 spread where the real one
//! is $5.00 — understating the cost of every BTC trade by about seventeen
//! times. A backtest built that way is not slightly optimistic, it is wrong,
//! and nothing about the result looks unusual.
//!
//! The gold parity gate cannot catch this: for gold the market values *are* the
//! top-level ones, so both readings agree.

use std::path::Path;

use fd_backtest::engine::trading_rules_for;
use fd_core::config::Config;

fn config() -> Config {
    Config::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config"))
        .expect("the workspace config")
}

#[test]
fn btc_uses_its_own_contract_size_and_spread() {
    let config = config();
    let rules = trading_rules_for(&config, "btc").expect("btc rules");
    let market = config.market("btc").expect("btc market");

    assert_eq!(rules.contract_size, market.trading.contract_size);
    assert_eq!(rules.spread, market.trading.spread);
    assert_eq!(rules.lot_step, market.trading.lot_step);
    assert_eq!(rules.min_lot, market.trading.min_lot);

    // The point of the test: these differ from the shared table, so reading the
    // shared table would have gone unnoticed.
    assert_ne!(rules.spread, config.trading.spread, "btc's spread must not be gold's");
    assert_ne!(rules.contract_size, config.trading.contract_size);
}

#[test]
fn policy_stays_shared_across_markets() {
    let config = config();
    let btc = trading_rules_for(&config, "btc").expect("btc rules");
    let gold = trading_rules_for(&config, "gold").expect("gold rules");

    // Risk appetite and holding period are decisions about how this system
    // trades, not conventions of a venue, so they are the same everywhere.
    assert_eq!(btc.risk_per_trade_pct, gold.risk_per_trade_pct);
    assert_eq!(btc.reward_risk, gold.reward_risk);
    assert_eq!(btc.max_hold_ms, gold.max_hold_ms);
    assert_eq!(btc.starting_equity_usd, gold.starting_equity_usd);
}

#[test]
fn an_unknown_market_is_refused() {
    assert!(trading_rules_for(&config(), "nonesuch").is_err());
}
