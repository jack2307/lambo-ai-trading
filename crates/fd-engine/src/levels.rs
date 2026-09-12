//! Derived levels and confluence clustering.
//!
//! The premise of the whole method: a price matters more when several
//! independently-derived option levels land on it, and more again when they come
//! from different expirations. Clustering turns a list of levels into zones with
//! a score.
//!
//! The scoring coefficients are **this project's own construction** and encode
//! untested hypotheses. They live in config so that when the hypotheses are
//! finally measured, nothing has to be hunted down in code.

use std::collections::BTreeMap;

use fd_core::premium::expiry_weight;
use serde::{Deserialize, Serialize};

/// Where a level came from.
// `Ord` is derived only so the type can live in a `BTreeSet` while counting how
// many distinct level types a cluster holds. The ordering itself is never
// meaningful and no result depends on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LevelType {
    MaxPain,
    Poc,
    AbovePoc,
    UnderPoc,
    CallBe,
    PutBe,
    Wsup,
    Wres,
    GammaWall,
}

impl LevelType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaxPain => "MAX_PAIN",
            Self::Poc => "POC",
            Self::AbovePoc => "ABOVE_POC",
            Self::UnderPoc => "UNDER_POC",
            Self::CallBe => "CALL_BE",
            Self::PutBe => "PUT_BE",
            Self::Wsup => "WSUP",
            Self::Wres => "WRES",
            Self::GammaWall => "GAMMA_WALL",
        }
    }

    /// True when the formula behind this level has never been reproduced.
    ///
    /// This flag travels all the way to the screen. A level nobody can derive
    /// twice should not look like one that is settled.
    #[must_use]
    pub const fn is_experimental(self) -> bool {
        matches!(self, Self::CallBe | Self::PutBe | Self::Wsup | Self::Wres | Self::GammaWall)
    }

    #[must_use]
    pub fn default_weight(self) -> f64 {
        match self {
            Self::MaxPain | Self::Poc => 1.0,
            Self::AbovePoc | Self::UnderPoc => 0.6,
            Self::CallBe | Self::PutBe => 0.7,
            Self::Wsup | Self::Wres => 0.9,
            Self::GammaWall => 0.8,
        }
    }
}

/// One level produced by one contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedLevel {
    pub price: f64,
    pub level_type: LevelType,
    /// ISO date of the contract it came from.
    pub expiration: String,
    pub symbol: String,
    pub dte: f64,
    pub weight: f64,
    pub experimental: bool,
}

/// A zone of levels that agree on a price.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelCluster {
    pub low: f64,
    pub high: f64,
    /// Weight-weighted centre.
    pub center: f64,
    pub score: f64,
    pub distinct_types: usize,
    pub distinct_expirations: usize,
    pub levels: Vec<DerivedLevel>,
}

/// How wide a cluster may be, in price units.
///
/// The floor matters as much as the ATR term: `$5` is meaningful on gold at
/// 4,300 and meaningless on BTC at 77,000, which is why it is per market.
#[must_use]
pub fn cluster_distance(floor: f64, atr_fraction: f64, atr: Option<f64>) -> f64 {
    let from_atr = match atr {
        Some(a) if a.is_finite() => atr_fraction * a,
        _ => 0.0,
    };
    floor.max(from_atr)
}

/// Single-linkage clustering along the price axis, strongest cluster first.
#[must_use]
pub fn cluster_levels(levels: &[DerivedLevel], distance: f64) -> Vec<LevelCluster> {
    let mut sorted: Vec<DerivedLevel> = levels.iter().filter(|l| l.price.is_finite()).cloned().collect();
    // Stable sort: levels at the same price keep the order they were derived in,
    // so a tie resolves identically on every run.
    sorted.sort_by(|a, b| a.price.partial_cmp(&b.price).expect("prices are finite"));

    let mut clusters: Vec<LevelCluster> = Vec::new();
    for level in sorted {
        match clusters.last_mut() {
            Some(current) if level.price - current.high <= distance => {
                current.high = level.price;
                current.levels.push(level);
            }
            _ => clusters.push(LevelCluster {
                low: level.price,
                high: level.price,
                center: level.price,
                score: 0.0,
                distinct_types: 0,
                distinct_expirations: 0,
                levels: vec![level],
            }),
        }
    }

    for cluster in &mut clusters {
        score_cluster(cluster);
    }
    clusters.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    clusters
}

/// Score a cluster in place.
///
/// Two different level types at one price say more than the same level type
/// twice, and two different expirations say more again — hypotheses H1 and H2,
/// written down so they can eventually be measured.
pub fn score_cluster(cluster: &mut LevelCluster) {
    let mut weight_sum = 0.0;
    let mut price_sum = 0.0;
    let mut types = std::collections::BTreeSet::new();
    let mut expirations = std::collections::BTreeSet::new();

    for level in &cluster.levels {
        weight_sum += level.weight;
        price_sum += level.price * level.weight;
        types.insert(level.level_type);
        expirations.insert(level.expiration.clone());
    }

    cluster.center = if weight_sum > 0.0 { price_sum / weight_sum } else { (cluster.low + cluster.high) / 2.0 };
    cluster.distinct_types = types.len();
    cluster.distinct_expirations = expirations.len();
    cluster.score = weight_sum
        * (1.0 + 0.25 * (types.len() as f64 - 1.0))
        * (1.0 + 0.35 * (expirations.len() as f64 - 1.0));
}

/// The levels one expiration contributes, in a fixed order.
#[derive(Debug, Clone, Default)]
pub struct ContextLevels {
    pub symbol: String,
    pub expiration: String,
    pub dte: f64,
    pub max_pain: Option<f64>,
    pub poc: Option<f64>,
    pub above_poc: Option<f64>,
    pub under_poc: Option<f64>,
    pub call_be: Option<f64>,
    pub put_be: Option<f64>,
    pub w_sup: Option<f64>,
    pub w_res: Option<f64>,
    pub gamma_wall: Option<f64>,
}

/// Turn one expiration's levels into weighted rows.
#[must_use]
pub fn levels_from_context(ctx: &ContextLevels, type_weights: &BTreeMap<String, f64>) -> Vec<DerivedLevel> {
    let expiry = expiry_weight(ctx.dte);
    let pairs: [(LevelType, Option<f64>); 9] = [
        (LevelType::MaxPain, ctx.max_pain),
        (LevelType::Poc, ctx.poc),
        (LevelType::AbovePoc, ctx.above_poc),
        (LevelType::UnderPoc, ctx.under_poc),
        (LevelType::CallBe, ctx.call_be),
        (LevelType::PutBe, ctx.put_be),
        (LevelType::Wsup, ctx.w_sup),
        (LevelType::Wres, ctx.w_res),
        (LevelType::GammaWall, ctx.gamma_wall),
    ];

    pairs
        .into_iter()
        .filter_map(|(level_type, price)| {
            let price = price.filter(|p| p.is_finite())?;
            let base = type_weights.get(level_type.as_str()).copied().unwrap_or_else(|| level_type.default_weight());
            Some(DerivedLevel {
                price,
                level_type,
                expiration: ctx.expiration.clone(),
                symbol: ctx.symbol.clone(),
                dte: ctx.dte,
                weight: base * expiry,
                experimental: level_type.is_experimental(),
            })
        })
        .collect()
}

/// Nearest cluster to a price, and whether the price is inside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NearestCluster {
    pub cluster: LevelCluster,
    pub distance: f64,
    pub inside: bool,
}

#[must_use]
pub fn nearest_cluster(clusters: &[LevelCluster], price: f64, tolerance: f64) -> Option<NearestCluster> {
    let mut best: Option<(&LevelCluster, f64)> = None;
    for cluster in clusters {
        let distance = if price < cluster.low {
            cluster.low - price
        } else if price > cluster.high {
            price - cluster.high
        } else {
            0.0
        };
        if best.is_none_or(|(_, d)| distance < d) {
            best = Some((cluster, distance));
        }
    }
    best.map(|(cluster, distance)| NearestCluster {
        cluster: cluster.clone(),
        distance,
        inside: distance <= tolerance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(price: f64, level_type: LevelType, expiration: &str, symbol: &str) -> DerivedLevel {
        DerivedLevel {
            price,
            level_type,
            expiration: expiration.into(),
            symbol: symbol.into(),
            dte: 5.0,
            weight: 1.0,
            experimental: level_type.is_experimental(),
        }
    }

    #[test]
    fn the_cluster_width_never_falls_below_its_floor() {
        assert_eq!(cluster_distance(5.0, 0.15, Some(10.0)), 5.0);
        assert_eq!(cluster_distance(5.0, 0.15, Some(100.0)), 15.0);
        assert_eq!(cluster_distance(5.0, 0.15, None), 5.0);
        // BTC's wider floor is what keeps a $5 window from splitting every level.
        assert_eq!(cluster_distance(100.0, 0.15, Some(200.0)), 100.0);
    }

    #[test]
    fn nearby_levels_merge_and_distant_ones_do_not() {
        let levels = vec![
            level(4400.0, LevelType::MaxPain, "2026-09-25", "OGV6"),
            level(4402.0, LevelType::Poc, "2026-09-19", "OG3U6"),
            level(4600.0, LevelType::Wres, "2026-09-25", "OGV6"),
        ];
        let clusters = cluster_levels(&levels, 10.0);
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].levels.len(), 2, "strongest cluster comes first");
        assert_eq!(clusters[0].distinct_types, 2);
        assert_eq!(clusters[0].distinct_expirations, 2);
    }

    #[test]
    fn diversity_raises_the_score_above_a_lone_level() {
        let diverse = vec![
            level(4400.0, LevelType::MaxPain, "2026-09-25", "OGV6"),
            level(4402.0, LevelType::Poc, "2026-09-19", "OG3U6"),
        ];
        let repeated = vec![
            level(4400.0, LevelType::MaxPain, "2026-09-25", "OGV6"),
            level(4402.0, LevelType::MaxPain, "2026-09-25", "OGV6"),
        ];
        let diverse_score = cluster_levels(&diverse, 10.0)[0].score;
        let repeated_score = cluster_levels(&repeated, 10.0)[0].score;
        assert!(diverse_score > repeated_score, "{diverse_score} should beat {repeated_score}");
    }

    #[test]
    fn the_centre_is_weighted_not_the_midpoint() {
        let mut heavy = level(4400.0, LevelType::MaxPain, "2026-09-25", "OGV6");
        heavy.weight = 9.0;
        let light = level(4410.0, LevelType::Poc, "2026-09-25", "OGV6");
        let clusters = cluster_levels(&[heavy, light], 20.0);
        let center = clusters[0].center;
        assert!(center < 4405.0, "the heavier level must pull the centre: {center}");
    }

    #[test]
    fn experimental_levels_are_marked_as_such() {
        assert!(LevelType::Wsup.is_experimental());
        assert!(LevelType::CallBe.is_experimental());
        assert!(!LevelType::MaxPain.is_experimental());
        assert!(!LevelType::Poc.is_experimental());
    }

    #[test]
    fn a_context_only_contributes_the_levels_it_has() {
        let ctx = ContextLevels {
            symbol: "OGV6".into(),
            expiration: "2026-09-25".into(),
            dte: 12.0,
            max_pain: Some(4400.0),
            poc: Some(4380.0),
            call_be: None,
            w_sup: Some(f64::NAN), // a NaN must be dropped, not carried through
            ..Default::default()
        };
        let levels = levels_from_context(&ctx, &BTreeMap::new());
        let types: Vec<LevelType> = levels.iter().map(|l| l.level_type).collect();
        assert_eq!(types, vec![LevelType::MaxPain, LevelType::Poc]);
        // Beyond seven days the expiry weight discounts the level.
        assert!(levels[0].weight < 1.0);
    }

    #[test]
    fn configured_type_weights_override_the_defaults() {
        let ctx = ContextLevels {
            symbol: "OGV6".into(),
            expiration: "2026-09-25".into(),
            dte: 5.0,
            max_pain: Some(4400.0),
            ..Default::default()
        };
        let mut weights = BTreeMap::new();
        weights.insert("MAX_PAIN".to_string(), 4.0);
        let levels = levels_from_context(&ctx, &weights);
        assert_eq!(levels[0].weight, 4.0, "expiry weight is 1.0 at five days");
    }

    #[test]
    fn nearest_cluster_reports_zero_distance_when_inside() {
        let clusters = cluster_levels(
            &[
                level(4400.0, LevelType::MaxPain, "2026-09-25", "OGV6"),
                level(4405.0, LevelType::Poc, "2026-09-25", "OGV6"),
            ],
            10.0,
        );
        let inside = nearest_cluster(&clusters, 4402.0, 1.0).unwrap();
        assert_eq!(inside.distance, 0.0);
        assert!(inside.inside);

        let outside = nearest_cluster(&clusters, 4450.0, 1.0).unwrap();
        assert!(outside.distance > 0.0);
        assert!(!outside.inside);
        assert!(nearest_cluster(&[], 4400.0, 1.0).is_none());
    }
}
