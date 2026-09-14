//! Domain core for flowdesk.
//!
//! Everything downstream — indicators, the level engine, the backtester, the
//! live loop — speaks the types defined here.
//!
//! # Why `f64` and not a decimal type
//!
//! Money code usually reaches for fixed-point decimals, and for settlement or
//! ledger arithmetic that is right. This crate does statistics on market data:
//! ATR-normalised distances, premium-weighted means, payout curves. Two reasons
//! `f64` is the correct choice here:
//!
//! 1. Nothing in this system moves money. It produces levels and paper fills.
//! 2. The JavaScript prototype this is ported from computes in `f64` (every
//!    JavaScript number is one), and the port is gated on reproducing its
//!    numbers. A decimal type would introduce differences that look like port
//!    bugs but are not.
//!
//! If an execution path is ever added, prices crossing the broker boundary get
//! a decimal type at that boundary — not here.

pub mod classify;
pub mod clock;
pub mod config;
pub mod market;
pub mod premium;
pub mod types;

pub use classify::{FlowClass, classify_flow, infer_aggressor, is_bear, is_bull, position_sign};
pub use config::{Config, ConfigError};
pub use market::{Market, MarketId};
pub use premium::{dte_from, expiry_weight, notional_usd, premium_usd};
pub use types::{
    AggressorSide, Bar, ExpirationType, NewsEvent, OptionTrade, OptionType, TradeFlags, assign_content_ids,
};

/// Milliseconds in a day, used wherever a DTE is derived.
pub const MS_PER_DAY: f64 = 86_400_000.0;

/// Absolute tolerance used by parity tests against the JavaScript oracle.
///
/// The golden files round to nine decimals, so a true value and its serialised
/// form can legitimately differ by half a unit there.
pub const PARITY_EPSILON: f64 = 1e-9;

/// Relative tolerance, which the absolute one cannot replace.
///
/// Premium sums reach the billions, where one unit in the last place of an
/// `f64` is already larger than [`PARITY_EPSILON`]: an absolute-only comparison
/// would call bit-identical numbers different. Conversely a relative-only
/// comparison is useless near zero. The gate needs both.
pub const PARITY_RELATIVE: f64 = 1e-12;

/// Compare two floats the way the parity gate does.
///
/// NaN equals NaN here, deliberately: a warm-up gap is meaningful data in this
/// system, so an indicator that is not ready yet must be *equally* not ready on
/// both sides rather than being skipped.
#[must_use]
pub fn parity_eq(a: f64, b: f64) -> bool {
    if a.is_nan() && b.is_nan() {
        return true;
    }
    if a.is_nan() || b.is_nan() {
        return false;
    }
    if a.is_infinite() || b.is_infinite() {
        return a == b;
    }
    let diff = (a - b).abs();
    diff <= PARITY_EPSILON || diff <= PARITY_RELATIVE * a.abs().max(b.abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_treats_nan_as_equal_and_rejects_real_drift() {
        assert!(parity_eq(f64::NAN, f64::NAN));
        assert!(parity_eq(1.0, 1.0 + 1e-12));
        assert!(!parity_eq(1.0, 1.000_001));
        assert!(parity_eq(f64::INFINITY, f64::INFINITY));
        assert!(!parity_eq(f64::INFINITY, f64::NEG_INFINITY));
        assert!(!parity_eq(f64::NAN, 1.0));
        assert!(!parity_eq(1.0, f64::NAN));
    }

    #[test]
    fn half_a_unit_in_the_ninth_decimal_is_serialisation_noise() {
        // The oracle writes nine decimals, so this is the worst case a correct
        // value can look like after a round trip through the golden file.
        let truth = 79_801.107_341_647_5_f64;
        let written = 79_801.107_341_647_f64;
        assert!(parity_eq(truth, written));
    }

    #[test]
    fn large_sums_need_the_relative_tolerance() {
        // Bull premium reaches the billions; one ULP there already exceeds the
        // absolute epsilon, so an absolute-only gate would reject correct data.
        let a = 4_669_226_283.0_f64;
        let b = a + f64::EPSILON * a;
        assert!(parity_eq(a, b), "one ULP apart at 4.7e9 must still be equal");
        assert!(!parity_eq(a, a + 1.0), "a whole dollar apart must not be");
    }
}

/// JavaScript's `Math.round(value * 10^decimals) / 10^decimals`.
///
/// Not the same as Rust's [`f64::round`], and the difference is not academic:
/// `Math.round` rounds a half **up** (towards positive infinity), so
/// `Math.round(-0.5)` is `-0`, while `f64::round` rounds a half away from zero
/// and gives `-1`. Everything this codebase rounds with it — PnL, R multiples,
/// net premium — is routinely negative.
#[must_use]
pub fn js_round_to(value: f64, decimals: u32) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let scale = 10f64.powi(decimals as i32);
    (value * scale + 0.5).floor() / scale
}

/// JavaScript's `Number(value.toFixed(decimals))`.
///
/// Distinct from [`js_round_to`], which is why both exist: `toFixed` rounds the
/// double's own exact value, while `Math.round(v * scale)` multiplies first and
/// rounds whatever error that introduced. They disagree on values near a
/// boundary — `dte` of 9.85625 came out 9.8562 from one and 9.8563 from the
/// other, which is what the timeline parity gate caught.
///
/// One residual difference, documented rather than hidden: on an *exact* tie
/// Rust's formatter rounds to even and `toFixed` rounds away from zero. A tie
/// needs the double to be exactly a multiple of `10^-decimals / 2`, which the
/// quantities here (quotients by 86_400_000, premium sums) do not reach. If a
/// future caller rounds dyadic fractions like `0.0625`, this is where it breaks.
#[must_use]
pub fn js_to_fixed(value: f64, decimals: usize) -> f64 {
    if !value.is_finite() {
        return value;
    }
    format!("{value:.decimals$}").parse().unwrap_or(value)
}

#[cfg(test)]
mod rounding_tests {
    use super::*;

    #[test]
    fn a_negative_half_rounds_the_way_javascript_rounds_it() {
        // `Math.round(-0.5)` is -0, not -1. Rust's own `round` disagrees, and
        // PnL is negative half the time.
        assert_eq!(js_round_to(-0.005, 2), 0.0);
        assert_eq!((-0.005_f64 * 100.0).round() / 100.0, -0.01, "this is what we must NOT do");
        assert_eq!(js_round_to(0.005, 2), 0.01);
    }

    #[test]
    fn to_fixed_rounds_the_value_not_the_scaled_product() {
        // The case the timeline gate caught.
        let dte = 9.856_25_f64;
        assert_eq!(js_to_fixed(dte, 4), 9.8562);
        assert_eq!(js_round_to(dte, 4), 9.8563);
    }

    #[test]
    fn non_finite_values_pass_through_untouched() {
        assert!(js_round_to(f64::NAN, 2).is_nan());
        assert!(js_to_fixed(f64::NAN, 4).is_nan());
        assert_eq!(js_round_to(f64::INFINITY, 2), f64::INFINITY);
    }
}
