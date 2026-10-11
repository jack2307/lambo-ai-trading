//! Control (e) of `docs/decisions/2026-10-11-l3-target.md`: the Python
//! re-implementation of [`activity_profile`] must be the SAME ARITHMETIC.
//!
//! `agent/l3-target` needs the time-at-price profile's POC / VAH / VAL as a
//! series over 378,749 bars, with a version **shuffled in time** and a
//! constant-distance version beside it. A shuffle in time is not expressible
//! as an indicator over the primary series, so the level had to be built
//! outside the engine, in `py/research/l3_target_precheck.py`. That makes the
//! sentence *"my level is the desk's `activity_profile`"* a **claim** unless
//! something checks it.
//!
//! This is the check. Both implementations are fed a fixture built only from
//! integer arithmetic and division by `100.0` — operations IEEE-754 gives
//! identical results for in either language — so the inputs are bit-identical
//! by construction and any difference in the output is a difference in the
//! ARITHMETIC and not in the data.
//!
//! The expected numbers below were printed by the Python side
//! (`l3_target_precheck.parity_report`). If this test fails, the two sides
//! disagree and no level number from that script may be published.

use fd_core::types::Bar;
use fd_engine::price_levels::{activity_profile, bucket_size_price};

/// The same linear congruential generator the Python fixture uses:
/// `x <- (1103515245 x + 12345) mod 2^31`, seeded with the date.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = (1_103_515_245u64.wrapping_mul(self.0).wrapping_add(12_345)) % (1u64 << 31);
        self.0
    }
}

fn fixture(n: usize) -> Vec<Bar> {
    let mut lcg = Lcg(20_261_011);
    (0..n)
        .map(|k| {
            let base = 1000.0 + (lcg.next() % 4000) as f64 / 100.0;
            let up = (lcg.next() % 150) as f64 / 100.0;
            let dn = (lcg.next() % 150) as f64 / 100.0;
            Bar {
                time: k as i64 * 900_000,
                open: base,
                high: base + up,
                low: base - dn,
                // Written the same way round as the Python expression, because
                // f64 addition is not associative and the order is part of the
                // fixture: ((base + up) + base) - dn, then halved.
                close: (base + up + base - dn) / 2.0,
                // The feed this axis runs on carries NO volume (XAUDUKA-1m is
                // null on all 5,635,777 rows) and the profile is time-at-price,
                // so a volume here would be a number nothing reads.
                volume: None,
            }
        })
        .collect()
}

#[test]
fn the_fixture_is_the_same_bars_on_both_sides() {
    let bars = fixture(600);
    assert_eq!(bars.len(), 600);
    // Printed by the Python side; bit equality, not a tolerance.
    assert_eq!(bars[0].high, 1014.51, "high[0]");
    assert_eq!(bars[0].low, 1012.96, "low[0]");
    assert_eq!(bars[599].high, 1033.91, "high[599]");
    assert_eq!(bars[599].low, 1033.3600000000001, "low[599]");
}

#[test]
fn the_bucket_height_is_derived_the_same_way() {
    // The fixture's bucket is `bucket_size_price(2.5, 4.0)`, so the parity run
    // exercises the same derivation the live route uses
    // (fd-api/src/levels.rs:750) and not a round number chosen here.
    assert_eq!(bucket_size_price(2.5, 4.0), Some(0.625));
}

#[test]
fn poc_vah_and_val_agree_with_the_python_implementation_bit_for_bit() {
    let bars = fixture(600);
    let profile = activity_profile(&bars, 0.625, 0.70, 4.0).expect("the fixture has a profile");
    assert_eq!(profile.measure, "TIME_AT_PRICE", "it is a TIME profile and the field says so");

    let price = |level: &Option<fd_engine::price_levels::PriceLevel>| {
        level.as_ref().and_then(|l| l.price).expect("level price")
    };
    // Printed by `l3_target_precheck.parity_report`. Bit equality: the two
    // implementations do the same adds in the same order or this fails.
    assert_eq!(price(&profile.poc), 1030.8474999999999, "POC");
    assert_eq!(price(&profile.vah), 1035.5349999999999, "VAH");
    assert_eq!(price(&profile.val), 1008.66, "VAL");
}
