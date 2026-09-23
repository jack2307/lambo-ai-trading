//! The direction control: the same trades with the side decided by a coin.
//!
//! Lifted out of `fd-backtest --bin search` unchanged on 2026-09-23 so that
//! the rescore in [`crate::hypotheses`] reads the *same* control the receipts
//! were written against rather than a second copy of it. Nothing about the
//! arithmetic moved: [`DirectionFlipped`] is the wrapper `--mode=null-dir` has
//! always used, and [`permuted_sides_pnls`] is the body of its
//! `permuted_sides_pf`, split so that a caller can credit the rebate onto the
//! null's own P&L before taking a profit factor.
//!
//! **A control that is not given the rebate is not a control.** A credit
//! applied to the strategy and withheld from the thing it is measured against
//! lifts every row equally and produces a percentile that means nothing; see
//! `docs/hypotheses/2026-09-23-rebate-rescore.md`, which fails on exactly that
//! point.

use fd_indicators::IndicatorSpec;
use fd_strategy::registry::{BarContext, Exits, Intent, Params, Side, Strategy};

use crate::engine::{Trade, TradingRules};
use crate::rebate::Rebate;

/// A strategy with its entry direction replaced by a coin flip.
///
/// The stop and target are mirrored around the entry price rather than kept,
/// so a flipped long risks and targets the same distance a long did. Keeping
/// them would put a short's stop below the market, which is not a trade
/// anybody would take and would make the null meaninglessly bad.
pub struct DirectionFlipped<'a> {
    pub inner: &'a dyn Strategy,
    pub seed: u64,
}

impl Strategy for DirectionFlipped<'_> {
    fn id(&self) -> &'static str {
        self.inner.id()
    }
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn description(&self) -> &'static str {
        self.inner.description()
    }
    fn default_params(&self) -> Params {
        self.inner.default_params()
    }
    fn grid(&self) -> std::collections::BTreeMap<String, Vec<f64>> {
        self.inner.grid()
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        self.inner.indicators(p)
    }
    fn warmup(&self, p: &Params) -> usize {
        self.inner.warmup(p)
    }
    fn series(&self, p: &Params) -> Vec<String> {
        self.inner.series(p)
    }
    fn needs_options(&self) -> bool {
        self.inner.needs_options()
    }
    fn exits(&self) -> Exits {
        self.inner.exits()
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        let intent = self.inner.on_bar(ctx);
        let Intent::Enter { side, stop, target, reason } = intent else {
            return intent;
        };
        let mut hash = self.seed ^ (ctx.bar.time as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        hash ^= hash >> 29;
        hash = hash.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        if hash >> 63 == 0 {
            return Intent::Enter { side, stop, target, reason };
        }
        let close = ctx.bar.close;
        let mirror = |price: f64| 2.0 * close - price;
        Intent::Enter {
            side: if side.is_long() { Side::Short } else { Side::Long },
            stop: stop.map(mirror),
            target: target.map(mirror),
            reason,
        }
    }
}

/// The P&L of `trades` with each side re-drawn by coin flip: the price move
/// over the same interval reversed, the spread paid again.
///
/// One entry per trade, in the trades' own order, so a caller can add the
/// rebate to each before taking a profit factor. The arithmetic is the one
/// `search.rs` has used since the tsmom-2 pass — including its reading of
/// `rules.spread` and `rules.contract_size` rather than the trade's own
/// carried basis, which is what every receipt measured against.
#[must_use]
pub fn permuted_sides_pnls(trades: &[Trade], rules: &TradingRules, seed: u64) -> Vec<f64> {
    trades
        .iter()
        .map(|t| {
            let mut hash = seed ^ (t.entry_time as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            hash ^= hash >> 29;
            hash = hash.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            let spread = rules.spread * t.lots * rules.contract_size;
            // pnl = move − spread (+ swap, which does not flip with the side
            // here: this account charges none). The flipped trade earns
            // −move − spread.
            if hash >> 63 == 0 { t.pnl_usd } else { -(t.pnl_usd + spread) - spread }
        })
        .collect()
}

/// The profit factor of a set of P&Ls, classified exactly as
/// [`crate::engine::metrics_of`] classifies them: a trade at zero is a loss.
#[must_use]
pub fn profit_factor_of(pnls: &[f64]) -> f64 {
    let (mut wins, mut losses) = (0.0f64, 0.0f64);
    for pnl in pnls {
        if *pnl > 0.0 {
            wins += pnl;
        } else {
            losses -= pnl;
        }
    }
    if losses > 0.0 { wins / losses } else { f64::INFINITY }
}

/// The permuted-sides control's profit factor, gross and net of the rebate.
///
/// The credit does not depend on the side — it is a share of the spread the
/// trade paid, and a flipped trade pays the same spread over the same lots —
/// so the null carries exactly the credit the method carries, which is the
/// whole point of giving it one.
#[must_use]
pub fn permuted_sides_profit_factors(
    trades: &[Trade],
    rules: &TradingRules,
    seed: u64,
    rebate: Rebate,
) -> (f64, f64) {
    let pnls = permuted_sides_pnls(trades, rules, seed);
    let gross = profit_factor_of(&pnls);
    let credited: Vec<f64> = pnls
        .iter()
        .zip(trades)
        .map(|(pnl, t)| pnl + rebate.on_trade(t, rules).unwrap_or(0.0))
        .collect();
    (gross, profit_factor_of(&credited))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ExitKind;

    fn rules() -> TradingRules {
        TradingRules { contract_size: 1.0, spread: 0.28, ..TradingRules::default() }
    }

    fn trade(entry_time: i64, pnl: f64) -> Trade {
        Trade {
            direction: Side::Long,
            entry_time,
            entry_price: 2000.0,
            exit_time: entry_time + 900_000,
            exit_price: 2001.0,
            exit_reason: "TARGET".to_string(),
            exit_kind: ExitKind::Target,
            stop: 1990.0,
            target: None,
            lots: 0.10,
            pnl_usd: pnl,
            swap_usd: 0.0,
            r: pnl / 10.0,
            mae: 0.0,
            mfe: 0.0,
            hold_ms: 900_000,
            reason: "test".to_string(),
            contract_size: Some(1.0),
            spread: Some(0.28),
        }
    }

    /// The split must not have changed what the control produces: the gross
    /// figure is still the profit factor of the permuted P&Ls, seed for seed.
    #[test]
    fn the_gross_permuted_control_is_what_it_always_was() {
        let rules = rules();
        let trades: Vec<Trade> = (0..40).map(|i| trade(1_600_000_000_000 + i * 900_000, if i % 3 == 0 { 20.0 } else { -7.0 })).collect();
        for seed in 1..25u64 {
            let pnls = permuted_sides_pnls(&trades, &rules, seed);
            let (gross, _) = permuted_sides_profit_factors(&trades, &rules, seed, Rebate::new(0.45).unwrap());
            assert_eq!(profit_factor_of(&pnls).to_bits(), gross.to_bits(), "seed {seed}");
        }
    }

    /// And the rebate must reach the control, or the percentile it produces
    /// is an accounting error rather than a test.
    #[test]
    fn the_control_carries_the_rebate() {
        let rules = rules();
        let trades: Vec<Trade> = (0..40).map(|i| trade(1_600_000_000_000 + i * 900_000, if i % 3 == 0 { 20.0 } else { -7.0 })).collect();
        let zero = Rebate::new(0.0).unwrap();
        let real = Rebate::new(0.45).unwrap();
        let (gross, net_at_zero) = permuted_sides_profit_factors(&trades, &rules, 7, zero);
        assert_eq!(gross.to_bits(), net_at_zero.to_bits(), "a zero rate credits nothing");
        let (_, net) = permuted_sides_profit_factors(&trades, &rules, 7, real);
        assert!(net > gross, "the control's own profit factor rises with the credit: {net} vs {gross}");
    }

    #[test]
    fn a_book_that_never_lost_has_an_infinite_profit_factor_not_a_nan() {
        assert!(profit_factor_of(&[1.0, 2.0]).is_infinite());
        assert_eq!(profit_factor_of(&[1.0, -1.0]), 1.0);
        assert_eq!(profit_factor_of(&[1.0, 0.0]), f64::INFINITY, "a flat trade is a loss of nothing");
    }
}
