//! With no calendar installed, `news:` filters are no-ops.
//!
//! Lives in its own integration-test binary because the calendar is a
//! process-wide `OnceLock`: the crate's unit tests install one, and a test in
//! that process could never observe the "nothing installed" state.

use fd_core::types::Bar;
use fd_indicators::{IndicatorSet, IndicatorSpec};
use fd_strategy::filter::{Filter, Filtered};
use fd_strategy::news;
use fd_strategy::registry::{BarContext, Intent, Params, Side, Strategy};

struct Always;
impl Strategy for Always {
    fn id(&self) -> &'static str {
        "always"
    }
    fn name(&self) -> &'static str {
        "always"
    }
    fn description(&self) -> &'static str {
        ""
    }
    fn default_params(&self) -> Params {
        Params::default()
    }
    fn indicators(&self, _: &Params) -> Vec<IndicatorSpec> {
        Vec::new()
    }
    fn warmup(&self, _: &Params) -> usize {
        0
    }
    fn on_bar(&self, ctx: &BarContext) -> Intent {
        Intent::Enter { side: Side::Long, stop: Some(ctx.bar.close - 1.0), target: None, reason: "always".into() }
    }
}

#[test]
fn nothing_installed_means_never_in_blackout_and_a_no_op_filter() {
    assert_eq!(news::installed(), None);
    assert!(news::events().is_empty());
    assert!(!news::in_blackout(0, 60 * 60_000, 30 * 60_000, 3, None));
    assert!(!news::in_blackout(1_800_000_000_000, i64::MAX / 4, i64::MAX / 4, 1, Some(&["USD".to_string()])));
    assert_eq!(news::summary("data/news/events.parquet"), "news: none loaded — news: filters are no-ops");

    let f = Filtered { inner: &Always, filters: vec![Filter::parse("news:60-30").unwrap()] };
    let bar = Bar::flat(1_800_000_000_000, 100.0);
    let bars = [bar];
    let ind = IndicatorSet::new();
    let params = Params::default();
    let ctx = BarContext { bar: &bars[0], i: 0, bars: &bars, ind: &ind, series: &[], options: None, position: None, params: &params };
    assert!(matches!(f.on_bar(&ctx), Intent::Enter { .. }));
}
