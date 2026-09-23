//! The built-in methods.
//!
//! Having several is the point: a method is only interesting if it beats the
//! plain technical baselines on the same bars, the same costs and the same stop
//! model. Three groups —
//!
//! * classic technical: `ema-cross`, `rsi-reversion`, `donchian-breakout`, `bb-fade`
//! * options-derived: `level-reversion`, `maxpain-magnet`, `flow-momentum`
//! * combined: `flow-at-level`
//!
//! plus `buy-and-hold`, the control everything else has to beat.

use std::collections::BTreeMap;

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Registry, Side, Strategy};

/// Indicator instance key, formatted exactly as the indicator crate builds it.
fn key(id: &str, values: &[f64]) -> String {
    if values.is_empty() {
        return id.to_string();
    }
    let joined = values.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join("_");
    format!("{id}_{joined}")
}

fn grid(entries: &[(&str, &[f64])]) -> BTreeMap<String, Vec<f64>> {
    entries.iter().map(|(k, v)| ((*k).to_string(), v.to_vec())).collect()
}

fn atr_spec(params: &Params) -> IndicatorSpec {
    IndicatorSpec::new("atr").with("period", params.get("atrPeriod"))
}

fn atr_key(params: &Params) -> String {
    key("atr", &[params.get("atrPeriod")])
}

/// Stop placed a fixed multiple of ATR from the close.
fn atr_stop(side: Side, close: f64, atr: f64, mult: f64) -> Option<f64> {
    Some(if side.is_long() { close - atr * mult } else { close + atr * mult })
}

pub fn register_all(registry: &mut Registry) {
    registry.register(Box::new(EmaCross));
    registry.register(Box::new(RsiReversion));
    registry.register(Box::new(DonchianBreakout));
    registry.register(Box::new(BollingerFade));
    registry.register(Box::new(LevelReversion));
    registry.register(Box::new(MaxPainMagnet));
    registry.register(Box::new(FlowMomentum));
    registry.register(Box::new(FlowAtLevel));
    registry.register(Box::new(BuyAndHold));
    // Structure-based, not indicator-based; lives in its own module.
    registry.register(Box::new(crate::ict::IctSweepMssFvg));
    registry.register(Box::new(crate::orb::OpeningRangeBreakout));
    registry.register(Box::new(crate::pdhl::PreviousDayLevels));
    registry.register(Box::new(crate::external::External));
    registry.register(Box::new(crate::session_hold::SessionHold));
    registry.register(Box::new(crate::intraday_momentum::IntradayMomentum));
    registry.register(Box::new(crate::vwap_fade::VwapFade));
    registry.register(Box::new(crate::tsmom::TimeSeriesMomentum));
    registry.register(Box::new(crate::doji::DojiReversal));
    registry.register(Box::new(crate::gap_fade::GapFade));
    registry.register(Box::new(crate::trend_pullback::TrendPullback));
    registry.register(Box::new(crate::volume_thrust::VolumeThrust));
    registry.register(Box::new(crate::screen::KeltnerBreak));
    registry.register(Box::new(crate::screen::MacdCross));
    registry.register(Box::new(crate::screen::Rsi2Pullback));
    registry.register(Box::new(crate::screen::SqueezeBreak));
    registry.register(Box::new(crate::screen::StochReversal));
    registry.register(Box::new(crate::volman_box::VolmanBox));
    // The cost-term instrument: one signal, two invalidation rules.
    registry.register(Box::new(crate::far_stop_break::FarStopBreak));
    // The only method here that reads TWO instruments. It takes no trades
    // unless a companion series is installed; see its module docs.
    registry.register(Box::new(crate::companion_unconfirmed::CompanionUnconfirmed));
}

/* ---------------- classic technical baselines ---------------- */

pub struct EmaCross;

impl Strategy for EmaCross {
    fn id(&self) -> &'static str {
        "ema-cross"
    }
    fn name(&self) -> &'static str {
        "EMA cross"
    }
    fn description(&self) -> &'static str {
        "Long when the fast EMA crosses above the slow EMA, short on the reverse. The dumbest possible trend baseline."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("fast", 21.0), ("slow", 55.0), ("atrPeriod", 14.0), ("stopAtr", 1.5)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("fast", &[9.0, 21.0, 34.0]), ("slow", &[55.0, 89.0, 144.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("ema").with("period", p.get("fast")),
            IndicatorSpec::new("ema").with("period", p.get("slow")),
            atr_spec(p),
        ]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("slow") + 5
    }
    fn series(&self, p: &Params) -> Vec<String> {
        vec![key("ema", &[p.get("fast")]), key("ema", &[p.get("slow")]), atr_key(p)]
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const FAST: usize = 0;
        const SLOW: usize = 1;
        const ATR: usize = 2;
        let p = ctx.params;
        let (fast, slow) = (ctx.s(FAST), ctx.s(SLOW));
        let (fast_prev, slow_prev) = (ctx.s_back(FAST, 1), ctx.s_back(SLOW, 1));
        let atr = ctx.s(ATR);
        if ![fast, slow, fast_prev, slow_prev, atr].iter().all(|v| v.is_finite()) {
            return Intent::None;
        }

        let crossed_up = fast_prev <= slow_prev && fast > slow;
        let crossed_down = fast_prev >= slow_prev && fast < slow;

        if let Some(position) = ctx.position {
            let against = (position.side.is_long() && crossed_down) || (!position.side.is_long() && crossed_up);
            return if against { Intent::Exit { reason: "opposite cross".into() } } else { Intent::None };
        }

        if crossed_up {
            return Intent::Enter {
                side: Side::Long,
                stop: atr_stop(Side::Long, ctx.bar.close, atr, p.get("stopAtr")),
                target: None,
                reason: "fast EMA crossed above slow".into(),
            };
        }
        if crossed_down {
            return Intent::Enter {
                side: Side::Short,
                stop: atr_stop(Side::Short, ctx.bar.close, atr, p.get("stopAtr")),
                target: None,
                reason: "fast EMA crossed below slow".into(),
            };
        }
        Intent::None
    }
}

pub struct RsiReversion;

impl Strategy for RsiReversion {
    fn id(&self) -> &'static str {
        "rsi-reversion"
    }
    fn name(&self) -> &'static str {
        "RSI reversion"
    }
    fn description(&self) -> &'static str {
        "Fade an oversold or overbought RSI, exit back through the midline."
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("period", 14.0),
            ("oversold", 30.0),
            ("overbought", 70.0),
            ("exitLevel", 50.0),
            ("atrPeriod", 14.0),
            ("stopAtr", 1.5),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("period", &[7.0, 14.0, 21.0]), ("oversold", &[20.0, 25.0, 30.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("rsi").with("period", p.get("period")), atr_spec(p)]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("period") * 3
    }

    fn series(&self, p: &Params) -> Vec<String> {
        vec![key("rsi", &[p.get("period")]), atr_key(p)]
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const RSI: usize = 0;
        const ATR: usize = 1;
        let p = ctx.params;
        let (rsi, prev) = (ctx.s(RSI), ctx.s_back(RSI, 1));
        let atr = ctx.s(ATR);
        if ![rsi, prev, atr].iter().all(|v| v.is_finite()) {
            return Intent::None;
        }

        if let Some(position) = ctx.position {
            let done = if position.side.is_long() { rsi >= p.get("exitLevel") } else { rsi <= p.get("exitLevel") };
            return if done { Intent::Exit { reason: "RSI back to midline".into() } } else { Intent::None };
        }

        // Trigger on the turn out of the zone, not merely on being inside it.
        let oversold = p.get("oversold");
        if prev <= oversold && rsi > oversold {
            return Intent::Enter {
                side: Side::Long,
                stop: atr_stop(Side::Long, ctx.bar.close, atr, p.get("stopAtr")),
                target: None,
                reason: format!("RSI turned up out of {oversold}"),
            };
        }
        let overbought = p.get("overbought");
        if prev >= overbought && rsi < overbought {
            return Intent::Enter {
                side: Side::Short,
                stop: atr_stop(Side::Short, ctx.bar.close, atr, p.get("stopAtr")),
                target: None,
                reason: format!("RSI turned down out of {overbought}"),
            };
        }
        Intent::None
    }
}

pub struct DonchianBreakout;

impl Strategy for DonchianBreakout {
    fn id(&self) -> &'static str {
        "donchian-breakout"
    }
    fn name(&self) -> &'static str {
        "Donchian breakout"
    }
    fn description(&self) -> &'static str {
        "Buy a close above the prior N-bar high, sell a close below the prior N-bar low."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("period", 20.0), ("atrPeriod", 14.0), ("stopAtr", 2.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("period", &[10.0, 20.0, 40.0]), ("stopAtr", &[1.5, 2.0, 3.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![IndicatorSpec::new("donchian").with("period", p.get("period")), atr_spec(p)]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("period") + 5
    }

    fn series(&self, p: &Params) -> Vec<String> {
        let base = key("donchian", &[p.get("period")]);
        vec![format!("{base}.upper"), format!("{base}.lower"), format!("{base}.middle"), atr_key(p)]
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const UPPER: usize = 0;
        const LOWER: usize = 1;
        const MIDDLE: usize = 2;
        const ATR: usize = 3;
        let p = ctx.params;
        let upper = ctx.s(UPPER);
        let lower = ctx.s(LOWER);
        let mid = ctx.s(MIDDLE);
        let atr = ctx.s(ATR);
        if ![upper, lower, mid, atr].iter().all(|v| v.is_finite()) {
            return Intent::None;
        }

        if let Some(position) = ctx.position {
            let lost = if position.side.is_long() { ctx.bar.close < mid } else { ctx.bar.close > mid };
            return if lost {
                Intent::Exit {
                    reason: if position.side.is_long() {
                        "lost the channel midline".into()
                    } else {
                        "reclaimed the channel midline".into()
                    },
                }
            } else {
                Intent::None
            };
        }

        if ctx.bar.close > upper {
            return Intent::Enter {
                side: Side::Long,
                stop: atr_stop(Side::Long, ctx.bar.close, atr, p.get("stopAtr")),
                target: None,
                reason: format!("broke the {}-bar high", p.period("period")),
            };
        }
        if ctx.bar.close < lower {
            return Intent::Enter {
                side: Side::Short,
                stop: atr_stop(Side::Short, ctx.bar.close, atr, p.get("stopAtr")),
                target: None,
                reason: format!("broke the {}-bar low", p.period("period")),
            };
        }
        Intent::None
    }
}

pub struct BollingerFade;

impl Strategy for BollingerFade {
    fn id(&self) -> &'static str {
        "bb-fade"
    }
    fn name(&self) -> &'static str {
        "Bollinger fade"
    }
    fn description(&self) -> &'static str {
        "Fade a close outside the bands, exit at the middle band."
    }
    fn default_params(&self) -> Params {
        Params::new(&[("period", 20.0), ("mult", 2.0), ("atrPeriod", 14.0), ("stopAtr", 1.5)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("period", &[14.0, 20.0, 30.0]), ("mult", &[1.5, 2.0, 2.5])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![
            IndicatorSpec::new("bbands").with("period", p.get("period")).with("mult", p.get("mult")),
            atr_spec(p),
        ]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("period") + 5
    }

    fn series(&self, p: &Params) -> Vec<String> {
        let base = key("bbands", &[p.get("period"), p.get("mult")]);
        vec![format!("{base}.upper"), format!("{base}.lower"), format!("{base}.middle"), atr_key(p)]
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const UPPER: usize = 0;
        const LOWER: usize = 1;
        const MIDDLE: usize = 2;
        const ATR: usize = 3;
        let p = ctx.params;
        let upper = ctx.s(UPPER);
        let lower = ctx.s(LOWER);
        let middle = ctx.s(MIDDLE);
        let atr = ctx.s(ATR);
        if ![upper, lower, middle, atr].iter().all(|v| v.is_finite()) {
            return Intent::None;
        }

        if let Some(position) = ctx.position {
            let reached =
                if position.side.is_long() { ctx.bar.close >= middle } else { ctx.bar.close <= middle };
            return if reached { Intent::Exit { reason: "middle band reached".into() } } else { Intent::None };
        }

        if ctx.bar.close < lower {
            return Intent::Enter {
                side: Side::Long,
                stop: atr_stop(Side::Long, ctx.bar.close, atr, p.get("stopAtr")),
                target: Some(middle),
                reason: "closed below the lower band".into(),
            };
        }
        if ctx.bar.close > upper {
            return Intent::Enter {
                side: Side::Short,
                stop: atr_stop(Side::Short, ctx.bar.close, atr, p.get("stopAtr")),
                target: Some(middle),
                reason: "closed above the upper band".into(),
            };
        }
        Intent::None
    }
}

/* ---------------- options-derived ---------------- */

pub struct LevelReversion;

impl Strategy for LevelReversion {
    fn id(&self) -> &'static str {
        "level-reversion"
    }
    fn name(&self) -> &'static str {
        "Options level reversion"
    }
    fn description(&self) -> &'static str {
        "Fade price into a confluence cluster of options-derived levels, targeting the cluster centre and beyond."
    }
    fn needs_options(&self) -> bool {
        true
    }
    fn default_params(&self) -> Params {
        Params::new(&[("minClusterScore", 3.0), ("entryAtr", 0.35), ("atrPeriod", 14.0), ("stopAtr", 1.5)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("minClusterScore", &[2.0, 3.0, 5.0]), ("entryAtr", &[0.2, 0.35, 0.6])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![atr_spec(p)]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") * 3
    }

    fn series(&self, p: &Params) -> Vec<String> {
        vec![atr_key(p)]
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const ATR: usize = 0;
        let p = ctx.params;
        let atr = ctx.s(ATR);
        let Some(options) = ctx.options else { return Intent::None };
        if !atr.is_finite() || ctx.position.is_some() {
            return Intent::None;
        }

        // Read once: these are constant for the run, and the loop below runs
        // per cluster per bar per parameter cell.
        let min_score = p.get("minClusterScore");
        let entry_atr = p.get("entryAtr");
        let close = ctx.bar.close;
        for cluster in options.clusters {
            if cluster.score < min_score {
                continue;
            }
            let distance = cluster_distance(close, cluster.low, cluster.high);
            if distance > atr * entry_atr {
                continue;
            }
            // Support below price is bought; resistance above price is sold.
            return if cluster.center <= close {
                Intent::Enter {
                    side: Side::Long,
                    stop: Some(cluster.low - atr * 0.3),
                    target: None,
                    reason: format!("support cluster {:.1}-{:.1} score {:.1}", cluster.low, cluster.high, cluster.score),
                }
            } else {
                Intent::Enter {
                    side: Side::Short,
                    stop: Some(cluster.high + atr * 0.3),
                    target: None,
                    reason: format!(
                        "resistance cluster {:.1}-{:.1} score {:.1}",
                        cluster.low, cluster.high, cluster.score
                    ),
                }
            };
        }
        Intent::None
    }
}

fn cluster_distance(price: f64, low: f64, high: f64) -> f64 {
    if price < low {
        low - price
    } else if price > high {
        price - high
    } else {
        0.0
    }
}

pub struct MaxPainMagnet;

impl Strategy for MaxPainMagnet {
    fn id(&self) -> &'static str {
        "maxpain-magnet"
    }
    fn name(&self) -> &'static str {
        "Max pain magnet"
    }
    fn description(&self) -> &'static str {
        "Trade toward the front-expiry max pain when price sits far from it and expiry is near. Tests the magnet claim directly."
    }
    fn needs_options(&self) -> bool {
        true
    }
    fn default_params(&self) -> Params {
        Params::new(&[("minDistanceAtr", 1.5), ("maxDte", 3.0), ("atrPeriod", 14.0), ("stopAtr", 2.0)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("minDistanceAtr", &[1.0, 1.5, 2.5]), ("maxDte", &[1.0, 3.0, 7.0])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![atr_spec(p)]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") * 3
    }

    fn series(&self, p: &Params) -> Vec<String> {
        vec![atr_key(p)]
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const ATR: usize = 0;
        let p = ctx.params;
        let atr = ctx.s(ATR);
        let Some(options) = ctx.options else { return Intent::None };
        if !atr.is_finite() || ctx.position.is_some() {
            return Intent::None;
        }
        let Some(front) = options.contexts.first() else { return Intent::None };
        let Some(max_pain) = front.max_pain.filter(|v| v.is_finite()) else { return Intent::None };
        if !front.dte.is_finite() || front.dte > p.get("maxDte") {
            return Intent::None;
        }

        let gap = max_pain - ctx.bar.close;
        if gap.abs() < atr * p.get("minDistanceAtr") {
            return Intent::None;
        }
        let side = if gap > 0.0 { Side::Long } else { Side::Short };
        Intent::Enter {
            side,
            stop: atr_stop(side, ctx.bar.close, atr, p.get("stopAtr")),
            target: Some(max_pain),
            reason: format!(
                "max pain {:.1} is {:.1} ATR away, {:.1} DTE",
                max_pain,
                (gap.abs() / atr),
                front.dte
            ),
        }
    }
}

pub struct FlowMomentum;

impl Strategy for FlowMomentum {
    fn id(&self) -> &'static str {
        "flow-momentum"
    }
    fn name(&self) -> &'static str {
        "Options flow momentum"
    }
    fn description(&self) -> &'static str {
        "Follow the direction of accelerating bull or bear option premium, ignoring where price is."
    }
    fn needs_options(&self) -> bool {
        true
    }
    fn default_params(&self) -> Params {
        Params::new(&[("minBullRatio", 0.6), ("minVelocity", 0.05), ("atrPeriod", 14.0), ("stopAtr", 1.5)])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[("minBullRatio", &[0.55, 0.6, 0.65]), ("minVelocity", &[0.0, 0.05, 0.15])])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![atr_spec(p)]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") * 3
    }

    fn series(&self, p: &Params) -> Vec<String> {
        vec![atr_key(p)]
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const ATR: usize = 0;
        let p = ctx.params;
        let atr = ctx.s(ATR);
        let Some(options) = ctx.options else { return Intent::None };
        if !atr.is_finite() || ctx.position.is_some() {
            return Intent::None;
        }
        let (ratio, velocity) = (options.bull_ratio_15m, options.net_flow_velocity_norm);
        if !ratio.is_finite() || !velocity.is_finite() {
            return Intent::None;
        }

        let floor = p.get("minBullRatio");
        let min_velocity = p.get("minVelocity");
        if ratio >= floor && velocity >= min_velocity {
            return Intent::Enter {
                side: Side::Long,
                stop: atr_stop(Side::Long, ctx.bar.close, atr, p.get("stopAtr")),
                target: None,
                reason: format!("bull flow {ratio:.2}, velocity {velocity:.2}"),
            };
        }
        if 1.0 - ratio >= floor && velocity <= -min_velocity {
            return Intent::Enter {
                side: Side::Short,
                stop: atr_stop(Side::Short, ctx.bar.close, atr, p.get("stopAtr")),
                target: None,
                reason: format!("bear flow {:.2}, velocity {velocity:.2}", 1.0 - ratio),
            };
        }
        Intent::None
    }
}

/* ---------------- combined ---------------- */

pub struct FlowAtLevel;

impl Strategy for FlowAtLevel {
    fn id(&self) -> &'static str {
        "flow-at-level"
    }
    fn name(&self) -> &'static str {
        "Flow at level"
    }
    fn description(&self) -> &'static str {
        "The framework the reference methodology implies: options structure says WHERE, options flow says WHICH WAY, and a candle close through the level says WHEN."
    }
    fn needs_options(&self) -> bool {
        true
    }
    fn default_params(&self) -> Params {
        Params::new(&[
            ("minClusterScore", 3.0),
            ("entryAtr", 0.4),
            ("minBullRatio", 0.55),
            ("requireReclaim", 1.0),
            ("atrPeriod", 14.0),
            ("stopAtr", 1.5),
        ])
    }
    fn grid(&self) -> BTreeMap<String, Vec<f64>> {
        grid(&[
            ("minClusterScore", &[2.0, 3.0, 5.0]),
            ("minBullRatio", &[0.5, 0.55, 0.6]),
            ("requireReclaim", &[0.0, 1.0]),
        ])
    }
    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        vec![atr_spec(p)]
    }
    fn warmup(&self, p: &Params) -> usize {
        p.period("atrPeriod") * 3
    }

    fn series(&self, p: &Params) -> Vec<String> {
        vec![atr_key(p)]
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        const ATR: usize = 0;
        let p = ctx.params;
        let atr = ctx.s(ATR);
        let Some(options) = ctx.options else { return Intent::None };
        if !atr.is_finite() || ctx.position.is_some() {
            return Intent::None;
        }
        let ratio = options.bull_ratio_15m;
        if !ratio.is_finite() {
            return Intent::None;
        }

        let min_score = p.get("minClusterScore");
        let require_reclaim = p.get("requireReclaim") != 0.0;
        let entry_atr = p.get("entryAtr");
        let min_bull_ratio = p.get("minBullRatio");
        let close = ctx.bar.close;
        let prev_close = ctx.prev().map(|b| b.close);

        for cluster in options.clusters.iter().filter(|c| c.score >= min_score) {
            if cluster_distance(close, cluster.low, cluster.high) > atr * entry_atr {
                continue;
            }
            let at_support = cluster.center <= close;

            // Layer C: the bar must close back *through* the level, not merely
            // touch it. Without this the method buys every tag of a level.
            let reclaimed_up = !require_reclaim || prev_close.is_some_and(|prev| prev <= cluster.low && close > cluster.low);
            let rejected_down =
                !require_reclaim || prev_close.is_some_and(|prev| prev >= cluster.high && close < cluster.high);

            if at_support && ratio >= min_bull_ratio && reclaimed_up {
                return Intent::Enter {
                    side: Side::Long,
                    stop: Some(cluster.low - atr * 0.3),
                    target: None,
                    reason: format!("bull flow {ratio:.2} at support {:.1}-{:.1}", cluster.low, cluster.high),
                };
            }
            if !at_support && 1.0 - ratio >= min_bull_ratio && rejected_down {
                return Intent::Enter {
                    side: Side::Short,
                    stop: Some(cluster.high + atr * 0.3),
                    target: None,
                    reason: format!(
                        "bear flow {:.2} at resistance {:.1}-{:.1}",
                        1.0 - ratio,
                        cluster.low,
                        cluster.high
                    ),
                };
            }
        }
        Intent::None
    }
}

/// The control every other result must beat.
pub struct BuyAndHold;

impl Strategy for BuyAndHold {
    fn id(&self) -> &'static str {
        "buy-and-hold"
    }
    fn name(&self) -> &'static str {
        "Buy and hold"
    }
    fn description(&self) -> &'static str {
        "Enter long on the first tradable bar and never exit. The control every other result must beat."
    }
    fn exits(&self) -> Exits {
        // Without this the engine imposes an ATR stop and the control turns into
        // dozens of stopped-out trades — which is exactly what happened the
        // first time, and it stopped being a control.
        Exits::Strategy
    }
    fn default_params(&self) -> Params {
        Params::default()
    }
    fn indicators(&self, _p: &Params) -> Vec<IndicatorSpec> {
        Vec::new()
    }
    fn warmup(&self, _p: &Params) -> usize {
        1
    }

    fn on_bar(&self, ctx: &BarContext) -> Intent {
        if ctx.position.is_some() {
            return Intent::None;
        }
        Intent::Enter { side: Side::Long, stop: None, target: None, reason: "baseline".into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicator_keys_match_the_indicator_crate() {
        assert_eq!(key("ema", &[21.0]), "ema_21");
        assert_eq!(key("bbands", &[20.0, 2.0]), "bbands_20_2");
        assert_eq!(key("bbands", &[20.0, 2.5]), "bbands_20_2.5");
        assert_eq!(key("atr", &[14.0]), "atr_14");
    }

    #[test]
    fn every_builtin_declares_the_parameters_it_reads() {
        let registry = Registry::with_builtins();
        for strategy in registry.all() {
            let params = strategy.default_params();
            // Each indicator spec must resolve against a declared parameter.
            for spec in strategy.indicators(&params) {
                for name in spec.params.keys() {
                    assert!(
                        spec.params[name].is_finite(),
                        "{} passed a non-finite {name} to {}",
                        strategy.id(),
                        spec.id
                    );
                }
            }
            assert!(strategy.warmup(&params) > 0 || strategy.id() == "buy-and-hold");
        }
    }

    #[test]
    fn grids_only_name_declared_parameters() {
        let registry = Registry::with_builtins();
        for strategy in registry.all() {
            let defaults = strategy.default_params();
            for name in strategy.grid().keys() {
                assert!(defaults.contains(name), "{} sweeps an undeclared parameter {name}", strategy.id());
            }
        }
    }

    #[test]
    fn options_strategies_are_flagged_and_technical_ones_are_not() {
        let registry = Registry::with_builtins();
        let needs: Vec<&str> = registry.all().iter().filter(|s| s.needs_options()).map(|s| s.id()).collect();
        assert!(needs.contains(&"flow-at-level") && needs.contains(&"maxpain-magnet"));
        assert!(!needs.contains(&"ema-cross"));
    }

    #[test]
    fn buy_and_hold_manages_its_own_exit() {
        let registry = Registry::with_builtins();
        assert_eq!(registry.get("buy-and-hold").unwrap().exits(), Exits::Strategy);
        assert_eq!(registry.get("ema-cross").unwrap().exits(), Exits::Engine);
    }

    #[test]
    fn cluster_distance_is_zero_inside_and_positive_outside() {
        assert_eq!(cluster_distance(4400.0, 4390.0, 4410.0), 0.0);
        assert_eq!(cluster_distance(4380.0, 4390.0, 4410.0), 10.0);
        assert_eq!(cluster_distance(4420.0, 4390.0, 4410.0), 10.0);
    }
}

#[cfg(test)]
mod known_defects {
    //! Defects carried over from the oracle on purpose.
    //!
    //! The parity gate's job is to prove the port did not change the
    //! arithmetic, so a bug in the original stays a bug here until the oracle
    //! is retired. These tests exist so that "faithful" never quietly becomes
    //! "forgotten": each one asserts the *wrong* behaviour and says what the
    //! right one is.

    use crate::registry::Params;

    #[test]
    fn maxpain_magnet_accepts_an_expired_contract() {
        // `on_bar` filters `dte > maxDte` with no lower bound, so a contract
        // that settled hours ago — negative DTE — passes the guard. The live
        // engine has served `dte: -0.1711` for a settled BTC expiry, which
        // would put a trade on the max pain of a contract that no longer
        // exists.
        //
        // The oracle does the same (`src/strategy/builtin.js`: `if (front.dte >
        // params.maxDte) return NONE;`), so fixing it here would break parity
        // and cost the only proof the port is faithful.
        //
        // CORRECT BEHAVIOUR: reject `dte < 0`. Apply it when the oracle is
        // retired in phase 6, or sooner behind an explicit opt-in — but never
        // silently, and never without regenerating the golden files.
        let params = Params::new(&[("minDistanceAtr", 1.5), ("maxDte", 3.0)]);
        let max_dte = params.get("maxDte");
        let expired_dte = -0.1711_f64;

        // Negated on purpose: this mirrors the guard's own shape, which is the
        // whole point of the test.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        let passes_the_guard = !(expired_dte > max_dte);
        assert!(passes_the_guard, "an expired contract still passes the only DTE guard the strategy has");
        assert!(expired_dte < 0.0, "and it is expired: {expired_dte} days to expiry");
    }
}
