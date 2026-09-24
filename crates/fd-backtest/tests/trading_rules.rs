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

use fd_backtest::engine::{TradingRules, trading_rules_for};
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

    // Risk appetite is a decision about how this system trades, not a
    // convention of a venue, so it is the same everywhere.
    assert_eq!(btc.risk_per_trade_pct, gold.risk_per_trade_pct);
    assert_eq!(btc.reward_risk, gold.reward_risk);
    // The holding period and the starting equity CAN be stated per market
    // (`[markets.<id>.trading] max_hold_ms` / `starting_equity_usd`) and the
    // shipped `config/default.toml` states neither, so both still read the
    // shared table on every market. This assertion is therefore a check on the
    // shipped config, not on the plumbing: it fails the day someone adds an
    // override to `default.toml` without saying so in a record.
    assert_eq!(btc.max_hold_ms, gold.max_hold_ms);
    assert_eq!(btc.starting_equity_usd, gold.starting_equity_usd);
}

/// The default is four hours on every configured market, stated as a number
/// rather than as a comparison between two of them.
///
/// `max_hold_ms` is read by the paper loop through `trading_rules_for` and the
/// two funded books on account 33708517 were deployed under four hours. A
/// change that moved the default for a market naming no override would move
/// them, so the number is pinned here for every market the config declares.
#[test]
fn four_hours_is_the_default_on_every_market_the_config_declares() {
    let config = config();
    const FOUR_HOURS_MS: i64 = 14_400_000;
    assert_eq!(config.trading.max_hold_ms, FOUR_HOURS_MS, "the shared table must still say four hours");
    for id in config.markets.keys() {
        let rules = trading_rules_for(&config, id).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(rules.max_hold_ms, FOUR_HOURS_MS, "{id} must inherit four hours while it names no override");
        assert_eq!(
            config.market(id).expect("market").trading.max_hold_ms, None,
            "{id} declares a max_hold_ms override in config/default.toml — that is a live behaviour change and needs a record"
        );
    }
}

/// An override moves ONE market and nothing else.
///
/// The whole safety argument for the key is that it is additive, so the test
/// states both halves: the market that names it gets it, and a market that does
/// not keeps the shared four hours.
#[test]
fn a_per_market_max_hold_override_is_scoped_to_that_market() {
    let mut config = config();
    const ONE_WEEK_MS: i64 = 604_800_000;
    config.markets.get_mut("xauusd").expect("xauusd").trading.max_hold_ms = Some(ONE_WEEK_MS);

    let xauusd = trading_rules_for(&config, "xauusd").expect("xauusd rules");
    let gold = trading_rules_for(&config, "gold").expect("gold rules");
    let btc = trading_rules_for(&config, "btc").expect("btc rules");
    assert_eq!(xauusd.max_hold_ms, ONE_WEEK_MS, "the market that names the override takes it");
    assert_eq!(gold.max_hold_ms, 14_400_000, "a market that names none keeps the shared four hours");
    assert_eq!(btc.max_hold_ms, 14_400_000, "and so does BTC, whose every other figure differs");
    // Nothing else on the overridden market moved.
    let plain = {
        let mut c = config.clone();
        c.markets.get_mut("xauusd").expect("xauusd").trading.max_hold_ms = None;
        trading_rules_for(&c, "xauusd").expect("xauusd rules")
    };
    assert_eq!(TradingRules { max_hold_ms: plain.max_hold_ms, ..xauusd.clone() }, plain, "only the hold may differ");
}

/// A market table that names no `max_hold_ms` parses, and parses as `None` —
/// `null` is not `0`. A zero would mean "no timeout at all" to
/// `engine::check_exit`, which is the opposite of the default.
#[test]
fn an_absent_override_is_none_and_not_zero() {
    let config = config();
    assert_eq!(config.markets["xauusd"].trading.max_hold_ms, None);
    assert_eq!(config.markets["btc"].trading.max_hold_ms, None);
}

#[test]
fn an_unknown_market_is_refused() {
    assert!(trading_rules_for(&config(), "nonesuch").is_err());
}
