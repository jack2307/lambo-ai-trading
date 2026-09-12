//! Premium arithmetic.
//!
//! Two conventions exist in this system and confusing them silently corrupts
//! every downstream number:
//!
//! | market | quote | premium in USD |
//! |---|---|---|
//! | COMEX gold | USD per ounce, 100 oz per contract | `price × size × 100` |
//! | Deribit BTC | **BTC** per contract, 1 BTC per contract | `price × index × size` |
//!
//! Which one applies is a property of the market, never a constant in a call
//! site — see [`crate::market::Market::premium_usd`].

use crate::MS_PER_DAY;

/// Premium for a fixed-multiplier market (gold).
///
/// Verified print-by-print against the reference tape: `21.9 × 5 × 100 = 10,950`.
#[must_use]
pub fn premium_usd(trade_price: f64, contracts: f64, multiplier: f64) -> f64 {
    trade_price * contracts * multiplier
}

/// Premium where the option is quoted in the underlying (Deribit).
///
/// `0.002 BTC × 77,000 USD/BTC × 3 = 462 USD`.
#[must_use]
pub fn premium_usd_in_underlying(trade_price: f64, contracts: f64, index_price: f64) -> f64 {
    trade_price * index_price * contracts
}

/// Notional value of the underlying a print controls.
#[must_use]
pub fn notional_usd(contracts: f64, underlying_price: f64, multiplier: f64) -> f64 {
    contracts * underlying_price * multiplier
}

/// Delta-adjusted notional. Only meaningful once Greeks exist; premium alone
/// exaggerates far out-of-the-money trades.
#[must_use]
pub fn delta_adjusted_notional(contracts: f64, underlying_price: f64, delta: f64, multiplier: f64) -> f64 {
    notional_usd(contracts, underlying_price, multiplier) * delta.abs()
}

/// Days to expiration between two epoch-millisecond instants.
#[must_use]
pub fn dte_from(now_ms: i64, expiration_ms: i64) -> f64 {
    (expiration_ms - now_ms) as f64 / MS_PER_DAY
}

/// Weight a level gains as its expiry approaches.
///
/// This encodes hypothesis H5 ("options levels matter more near expiry") and is
/// **not** a measured curve. It stays here as a single function so that when the
/// hypothesis is finally tested, one edit changes every consumer.
#[must_use]
pub fn expiry_weight(dte: f64) -> f64 {
    if dte <= 0.25 {
        1.5
    } else if dte <= 1.0 {
        1.3
    } else if dte <= 3.0 {
        1.15
    } else if dte <= 7.0 {
        1.0
    } else {
        0.8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gold_premium_matches_the_verified_tape_arithmetic() {
        assert!((premium_usd(21.9, 5.0, 100.0) - 10_950.0).abs() < 1e-9);
        assert!((premium_usd(1.0, 1.0, 100.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn deribit_premium_needs_the_index_not_a_multiplier() {
        assert!((premium_usd_in_underlying(0.002, 3.0, 77_000.0) - 462.0).abs() < 1e-9);
        // Fractional contract sizes are normal on Deribit.
        assert!((premium_usd_in_underlying(0.0017, 0.4, 77_208.49) - 52.50_f64).abs() < 0.01);
    }

    #[test]
    fn dte_counts_down_and_goes_negative_after_expiry() {
        let expiry = 1_000_000_000_000_i64;
        assert!((dte_from(expiry - 86_400_000, expiry) - 1.0).abs() < 1e-9);
        assert!(dte_from(expiry + 86_400_000, expiry) < 0.0);
    }

    #[test]
    fn expiry_weight_is_monotone_toward_expiry() {
        let weights = [0.1, 0.5, 2.0, 5.0, 30.0].map(expiry_weight);
        for pair in weights.windows(2) {
            assert!(pair[0] >= pair[1], "weight must not rise with DTE: {weights:?}");
        }
    }
}
