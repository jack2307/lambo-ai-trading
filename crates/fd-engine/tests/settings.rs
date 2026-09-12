//! Engine settings come from the market, not from the shared table.
//!
//! Four values in `[levels]`, `[big_trades]` and the multiplier are per market,
//! and the top-level table holds gold's. Reading gold's `$5` cluster floor for
//! BTC does not fail — it builds narrow clusters out of levels that have
//! nothing to do with each other, and every cluster feature downstream is then
//! confidently wrong.
//!
//! Gold cannot catch this: for gold the market values *are* the shared ones.

use std::path::Path;

use fd_engine::engine::EngineSettings;
use fd_core::config::Config;

fn config() -> Config {
    Config::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config"))
        .expect("the workspace config")
}

#[test]
fn btc_gets_its_own_cluster_width_and_big_print_floor() {
    let config = config();
    let settings = EngineSettings::from_config(&config, "btc").expect("btc settings");
    let market = config.market("btc").expect("btc market");

    assert_eq!(settings.cluster_floor, market.cluster_floor);
    assert_eq!(settings.cluster_atr_fraction, market.cluster_atr_fraction);
    assert_eq!(settings.big_trades.min_premium_usd, market.big_trade_min_premium_usd);
    assert_eq!(settings.multiplier, market.multiplier);

    // The point of the test: these differ from the shared table, so reading the
    // shared table would have gone unnoticed.
    assert_ne!(settings.cluster_floor, config.levels.cluster.floor, "btc's cluster floor must not be gold's");
    assert_ne!(settings.big_trades.min_premium_usd, config.big_trades.min_premium_usd);
}

#[test]
fn what_is_shared_stays_shared() {
    let config = config();
    let btc = EngineSettings::from_config(&config, "btc").expect("btc");
    let gold = EngineSettings::from_config(&config, "gold").expect("gold");

    // How a profile is built and what counts as bullish are decisions about the
    // method, not conventions of a venue.
    assert_eq!(btc.profile_mode, gold.profile_mode);
    assert_eq!(btc.value_area_pct, gold.value_area_pct);
    assert_eq!(btc.max_clusters, gold.max_clusters);
    assert_eq!(btc.type_weights, gold.type_weights);
    assert_eq!(btc.bias, gold.bias);
}
