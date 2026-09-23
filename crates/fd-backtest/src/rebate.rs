//! The introducing-broker rebate, as a **credit beside the book**.
//!
//! The owner is the introducing broker on his own accounts, so part of the
//! spread his trading pays comes back to him. He was offered three ways of
//! recording that on 2026-09-21 and picked a separate credit line rather than
//! a change to the cost model (`docs/decisions/2026-09-21-rebate-credit-line.md`),
//! precisely so that a reader can see how much of a result is the strategy and
//! how much is the commercial arrangement.
//!
//! **Nothing in this module may change a gross figure.** It never mutates a
//! [`Trade`], never touches `TradingRules::spread`, and produces its net
//! numbers from a *copy*. Every receipt in `docs/decisions/` was measured
//! without a rebate; folding the credit into `spread` would silently restate
//! all of them, which is the thing the decision exists to prevent. The
//! invariant is pinned by `gross_is_untouched_by_the_credit` below and by the
//! whole existing golden suite, which this module does not touch.
//!
//! The arithmetic is the live desk's, to the rounding:
//!
//! ```text
//! credit = share_of_spread x spread x lots x contract_size      (USD, 4 dp)
//! ```
//!
//! ONE WHOLE ROUND-TURN SPREAD PER TRADE, and the word round-turn is
//! load-bearing. `engine::apply_costs` charges half the spread on entry and
//! half on exit, and the two halves are one spread; crediting a share of each
//! side would be twice the truth. `crates/fd-api/src/paper.rs`'s
//! `RebateTerms::on` is the live side of the same identity and
//! `crates/fd-api/tests/rebate_parity.rs` asserts the two agree, so they
//! cannot drift apart.

use std::path::Path;

use serde::Deserialize;

use crate::engine::{Metrics, Trade, TradingRules, metrics_of};

/// The rebate's terms, as `config/accounts.toml` records them.
///
/// `None` from [`Rebate::from_config_dir`] means the file declares no
/// arrangement at all, which is not the same fact as a rebate of zero: a zero
/// is a credit that was computed and came to nothing. Callers that cannot tell
/// the two apart should say "no rebate line" rather than print a nought.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Rebate {
    /// The share of the ROUND-TURN spread credited back; 0.45 today.
    pub share_of_spread: f64,
}

#[derive(Debug, Deserialize)]
struct RebateFile {
    rebate: Option<Rebate>,
}

impl Rebate {
    /// A rate stated in code, for a test or for `--rebate-share=`.
    ///
    /// Refuses anything that is not a finite share: a rate that is not a
    /// number would silently turn every credited figure into NaN, and a NaN
    /// profit factor reads as "no trades" rather than as "bad config".
    #[must_use]
    pub fn new(share_of_spread: f64) -> Option<Self> {
        (share_of_spread.is_finite() && share_of_spread >= 0.0).then_some(Self { share_of_spread })
    }

    /// The `[rebate]` table of `<dir>/accounts.toml`, or `None`.
    ///
    /// A missing or malformed registry costs the rebate line and nothing
    /// else, exactly as it does on the live side.
    #[must_use]
    pub fn from_config_dir(dir: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(dir.join("accounts.toml")).ok()?;
        let terms = toml::from_str::<RebateFile>(&text).ok()?.rebate?;
        Self::new(terms.share_of_spread)
    }

    /// The credit on one trade, in USD, or `None` when it cannot be priced.
    ///
    /// `lots x contract_size` is what one unit of price is worth in USD on
    /// this market — the engine's own P&L identity — so the spread the trade
    /// paid, in money, is `spread x lots x contract_size`, and this is the
    /// configured share of it.
    ///
    /// Refuses rather than returning zero when the spread or the size is not
    /// a positive number: a market configured with no spread charges nothing
    /// and therefore has nothing to rebate, but saying so with a `0.0` would
    /// make "not priced" and "priced at nothing" the same figure. This is
    /// `fd_api::paper::RebateTerms::on` verbatim, including the rounding, and
    /// a test asserts it stays that way.
    #[must_use]
    pub fn on(self, lots: f64, spread: f64, contract_size: f64) -> Option<f64> {
        let finite = lots.is_finite() && spread.is_finite() && contract_size.is_finite();
        (finite && lots > 0.0 && spread > 0.0 && contract_size > 0.0)
            .then(|| fd_core::js_round_to(self.share_of_spread * spread * lots * contract_size, 4))
    }

    /// The credit on a closed trade, priced from the basis the trade CARRIES.
    ///
    /// A trade born since 2026-09-21 records the `spread` and `contract_size`
    /// it was booked under, and those are what the credit is computed from —
    /// a config correction must not restate a credit against a cost the
    /// trade never paid (`docs/decisions/2026-09-21-restated-pnl-contract-size.md`).
    /// Only a trade that carries neither falls back to the current rules, and
    /// that figure is an estimate.
    #[must_use]
    pub fn on_trade(self, trade: &Trade, rules: &TradingRules) -> Option<f64> {
        self.on(
            trade.lots,
            trade.spread.unwrap_or(rules.spread),
            trade.contract_size.unwrap_or(rules.contract_size),
        )
    }

    /// The same trades with the credit added, as a **new** vector.
    ///
    /// The input is borrowed and never modified. `pnl_usd` gains the credit
    /// and `r` gains the credit expressed in the trade's own risk unit; every
    /// other field is carried through untouched, because the credit changes
    /// what the trade earned and nothing about what it did.
    ///
    /// NEITHER FIGURE IS RE-ROUNDED HERE, and that is deliberate. The engine
    /// rounds `pnl_usd` to two decimals because that is a cent of the account
    /// currency, and re-rounding after the credit would DISCARD any credit
    /// smaller than a cent — which on this workspace's per-ounce gold is most
    /// of them: 0.07 lots at 0.28 credits 0.0088 USD, real money on a cent
    /// account and nought after a second rounding. The credit itself is
    /// rounded once, to four decimals, exactly where the live side rounds it,
    /// and [`metrics_of`] applies the presentation rounding to the aggregates
    /// as it always has.
    ///
    /// A trade whose risk unit cannot be recovered ([`usd_per_r`]) keeps its
    /// gross `r`. That is deliberate: inventing a risk unit to make a column
    /// line up is how a restatement becomes invisible. The profit factor,
    /// which is the falsifier's own statistic, reads `pnl_usd` and is
    /// unaffected either way.
    #[must_use]
    pub fn credited(self, trades: &[Trade], rules: &TradingRules) -> Vec<Trade> {
        trades
            .iter()
            .map(|t| {
                let Some(credit) = self.on_trade(t, rules) else {
                    return t.clone();
                };
                let mut out = t.clone();
                out.pnl_usd = t.pnl_usd + credit;
                if let Some(per_r) = usd_per_r(t, rules) {
                    out.r = t.r + credit / per_r;
                }
                out
            })
            .collect()
    }

    /// [`metrics_of`] over [`Rebate::credited`] — the book net of the rebate.
    #[must_use]
    pub fn credited_metrics(self, trades: &[Trade], rules: &TradingRules, starting_equity: f64) -> Metrics {
        metrics_of(&self.credited(trades, rules), starting_equity)
    }

    /// Total credit over a set of trades, in USD.
    #[must_use]
    pub fn total(self, trades: &[Trade], rules: &TradingRules) -> f64 {
        fd_core::js_round_to(trades.iter().filter_map(|t| self.on_trade(t, rules)).sum::<f64>(), 2)
    }

    /// The credit as a fraction of the risk taken, averaged over the trades
    /// whose risk unit is recoverable. `NaN` when none of them is.
    ///
    /// This is the number the registration's break-even arithmetic is in:
    /// "5.8% of R at a 2-point stop against 1.0% at a 12-point stop". A
    /// construct whose rebate is 1% of R cannot be moved across a gate by it.
    #[must_use]
    pub fn mean_fraction_of_r(self, trades: &[Trade], rules: &TradingRules) -> f64 {
        let mut sum = 0.0;
        let mut n = 0usize;
        for t in trades {
            if let (Some(credit), Some(per_r)) = (self.on_trade(t, rules), usd_per_r(t, rules)) {
                sum += credit / per_r;
                n += 1;
            }
        }
        if n == 0 { f64::NAN } else { sum / n as f64 }
    }
}

/// What one R was worth in USD on this trade.
///
/// The engine sizes `lots = risk_usd / (risk x contract_size)` and books
/// `pnl = points x lots x contract_size`, with `r = points / risk`. So one R
/// is `risk x lots x contract_size`, and `risk` is the entry-to-stop distance
/// the position was born with.
///
/// A self-managed position carries no engine stop — the risk unit was an ATR
/// multiple and the recorded `stop` is not a number — so the identity
/// `pnl = r x (one R) - commission + swap` is inverted instead. Returns
/// `None` when neither route gives a positive, finite figure, rather than
/// guessing one.
#[must_use]
pub fn usd_per_r(trade: &Trade, rules: &TradingRules) -> Option<f64> {
    let contract_size = trade.contract_size.unwrap_or(rules.contract_size);
    let from_stop = (trade.entry_price - trade.stop).abs() * trade.lots * contract_size;
    if from_stop.is_finite() && from_stop > 0.0 {
        return Some(from_stop);
    }
    let gross = trade.pnl_usd + rules.commission_per_lot * trade.lots * 2.0 - trade.swap_usd;
    let per_r = (gross / trade.r).abs();
    (trade.r.is_finite() && trade.r.abs() > 1e-12 && per_r.is_finite() && per_r > 0.0).then_some(per_r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ExitKind;
    use fd_strategy::registry::Side;

    fn rules() -> TradingRules {
        TradingRules {
            contract_size: 1.0,
            spread: 0.28,
            commission_per_lot: 0.0,
            swap_long_per_lot: 0.0,
            swap_short_per_lot: 0.0,
            ..TradingRules::default()
        }
    }

    /// A trade that risked `entry - stop` and earned `pnl`.
    fn trade(entry: f64, stop: f64, lots: f64, pnl: f64, r: f64) -> Trade {
        Trade {
            direction: Side::Long,
            entry_time: 1_600_000_000_000,
            entry_price: entry,
            exit_time: 1_600_000_900_000,
            exit_price: entry + pnl,
            exit_reason: "TARGET".to_string(),
            exit_kind: ExitKind::Target,
            stop,
            target: None,
            lots,
            pnl_usd: pnl,
            swap_usd: 0.0,
            r,
            mae: 0.0,
            mfe: r.max(0.0),
            hold_ms: 900_000,
            reason: "test".to_string(),
            contract_size: Some(1.0),
            spread: Some(0.28),
        }
    }

    /// The worked example `config/accounts.toml` states in full, so that an
    /// implementation cannot pass its own arithmetic back to itself: "On
    /// XAUUSD.sc at the configured 0.28 spread, 0.07 lots, contract 1 oz …
    /// the spread costs 1.96 USC and this credits 0.88 USC".
    #[test]
    fn the_worked_example_from_the_account_registry() {
        let rebate = Rebate::new(0.45).unwrap();
        // The registry quotes USC on a cent account; the engine's own unit is
        // USD per ounce and the conversion happens where a human reads it, so
        // the identity to check is the same number x100.
        let credit = rebate.on(0.07, 0.28, 1.0).unwrap();
        assert!((credit * 100.0 - 0.88).abs() < 0.005, "0.88 USC on 0.07 lots at 0.28, got {}", credit * 100.0);
        let spread_cost: f64 = 0.28 * 0.07 * 1.0;
        assert!((spread_cost * 100.0 - 1.96).abs() < 0.005, "the round turn costs 1.96 USC");
        // 0.45 x 0.0196 = 0.00882, which the shared 4-dp rounding reports as
        // 0.0088 — so the ratio is 0.4490 and not 0.45 to the last place. The
        // rounding is the live side's and is deliberately not loosened here.
        assert_eq!(credit, 0.0088, "the credit as the live side rounds it");
        assert!((credit / spread_cost - 0.45).abs() < 0.002, "the credit is 45% of the round turn, once, to the rounding");
    }

    #[test]
    fn a_rate_that_is_not_a_number_is_refused_rather_than_propagated() {
        assert!(Rebate::new(f64::NAN).is_none());
        assert!(Rebate::new(-0.1).is_none());
        assert_eq!(Rebate::new(0.0).unwrap().share_of_spread, 0.0);
    }

    #[test]
    fn nothing_is_priced_where_there_is_no_spread_or_no_size() {
        let rebate = Rebate::new(0.45).unwrap();
        assert_eq!(rebate.on(0.07, 0.0, 1.0), None, "no spread charged, nothing to rebate — and not a zero");
        assert_eq!(rebate.on(0.0, 0.28, 1.0), None);
        assert_eq!(rebate.on(0.07, 0.28, 0.0), None);
        assert_eq!(rebate.on(f64::NAN, 0.28, 1.0), None);
    }

    /// THE INVARIANT THE DECISION OF 2026-09-21 TURNS ON. Every receipt in
    /// `docs/decisions/` was measured without a rebate; if crediting one
    /// could move a gross figure, all of them would be silently restated.
    #[test]
    fn gross_is_untouched_by_the_credit() {
        let rules = rules();
        let rebate = Rebate::new(0.45).unwrap();
        // Two winners and two losers with hand-written figures, so the gross
        // numbers below are a pin and not this module quoting itself.
        let trades = vec![
            trade(2000.0, 1990.0, 0.10, 20.0, 2.0),
            trade(2000.0, 1990.0, 0.10, -10.0, -1.0),
            trade(2000.0, 1995.0, 0.20, 10.0, 1.0),
            trade(2000.0, 1995.0, 0.20, -10.0, -1.0),
        ];
        let before = metrics_of(&trades, 100.0);
        assert_eq!(before.profit_factor, 1.5, "gross PF = (20+10)/(10+10)");
        assert_eq!(before.net_pnl_usd, 10.0);
        assert_eq!(before.total_r, 1.0);

        let snapshot = trades.clone();
        let credited = rebate.credited(&trades, &rules);

        assert_eq!(trades, snapshot, "credited() borrowed the book and must not have written to it");
        let after = metrics_of(&trades, 100.0);
        assert_eq!(
            before.profit_factor.to_bits(),
            after.profit_factor.to_bits(),
            "the gross profit factor is bit-identical after the credit was taken"
        );
        assert_eq!(before.net_pnl_usd.to_bits(), after.net_pnl_usd.to_bits());
        assert_eq!(before.total_r.to_bits(), after.total_r.to_bits());
        assert_eq!(before.expectancy.to_bits(), after.expectancy.to_bits());

        // And the credited copy is a different book, by exactly the credit.
        let credit_small = rebate.on(0.10, 0.28, 1.0).unwrap(); // 0.0126
        let credit_large = rebate.on(0.20, 0.28, 1.0).unwrap(); // 0.0252
        assert_eq!(credited[0].pnl_usd, 20.0 + credit_small);
        assert_eq!(credited[2].pnl_usd, 10.0 + credit_large);
        assert_eq!(rebate.total(&trades, &rules), fd_core::js_round_to(2.0 * credit_small + 2.0 * credit_large, 2));
    }

    /// A credit added to every trade lifts the winners and shrinks the
    /// losers, so the net profit factor can never be below the gross one. A
    /// net figure that came out lower would mean the credit had been applied
    /// as a cost somewhere.
    #[test]
    fn the_net_profit_factor_is_never_below_the_gross_one() {
        let rules = rules();
        let rebate = Rebate::new(0.45).unwrap();
        let trades = vec![
            trade(2000.0, 1990.0, 0.10, 20.0, 2.0),
            trade(2000.0, 1990.0, 0.10, -10.0, -1.0),
            trade(2000.0, 1995.0, 0.20, 0.30, 0.03),
            trade(2000.0, 1995.0, 0.20, -0.01, -0.001),
        ];
        let gross = metrics_of(&trades, 100.0);
        let net = rebate.credited_metrics(&trades, &rules, 100.0);
        assert!(net.profit_factor >= gross.profit_factor, "{} < {}", net.profit_factor, gross.profit_factor);
        assert!(net.net_pnl_usd > gross.net_pnl_usd);
    }

    /// The registration's own arithmetic, in the unit it is stated in: the
    /// credit is a fixed fraction of the spread, so as a fraction of R it
    /// grows as the stop tightens. A 12-point stop is a tenth of the burden a
    /// 1.2-point stop is.
    #[test]
    fn the_credit_as_a_fraction_of_r_grows_as_the_stop_tightens() {
        let rules = rules();
        let rebate = Rebate::new(0.45).unwrap();
        let wide = vec![trade(2000.0, 1988.0, 0.10, 1.0, 0.083)];
        let tight = vec![trade(2000.0, 1998.8, 0.10, 1.0, 0.83)];
        let wide_frac = rebate.mean_fraction_of_r(&wide, &rules);
        let tight_frac = rebate.mean_fraction_of_r(&tight, &rules);
        // 0.45 x 0.28 / 12 = 0.0105 of R; 0.45 x 0.28 / 1.2 = 0.105 of R.
        assert!((wide_frac - 0.0105).abs() < 1e-4, "wide stop: {wide_frac}");
        assert!((tight_frac - 0.105).abs() < 1e-3, "tight stop: {tight_frac}");
        assert!(tight_frac > wide_frac * 9.0);
    }

    /// A self-managed trade records no engine stop, and the risk unit has to
    /// come back out of the P&L identity rather than out of a guess.
    #[test]
    fn the_risk_unit_of_a_stopless_trade_comes_from_the_pnl_identity() {
        let rules = rules();
        let mut t = trade(2000.0, f64::NAN, 0.10, 5.0, 2.5);
        t.stop = f64::NAN;
        assert_eq!(usd_per_r(&t, &rules), Some(2.0), "5.0 USD over 2.5 R is 2.0 USD per R");
        t.r = 0.0;
        assert_eq!(usd_per_r(&t, &rules), None, "nothing is inferred from a trade that went nowhere");
    }
}
