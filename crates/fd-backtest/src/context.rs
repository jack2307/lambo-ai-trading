//! The options picture, sampled over time.
//!
//! Derived levels move far more slowly than a five-minute bar, so the tape is
//! replayed once at a fixed cadence and a compact frame stored at each step. A
//! strategy asking for "the options picture at bar time T" gets the most recent
//! frame **at or before** T.
//!
//! That lookup direction is the whole safety property: never the next frame,
//! even when it is only seconds away.

use fd_strategy::registry::{ClusterView, ContextView, OptionsView};
use serde::{Deserialize, Serialize};

/// One sample of the options board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Epoch milliseconds this frame was taken at.
    pub t: i64,
    pub spot: f64,
    pub bull_ratio: f64,
    pub bull_ratio_15m: f64,
    pub net_flow_velocity_norm: f64,
    pub big_trade_imbalance: f64,
    pub clusters: Vec<ClusterView>,
    pub contexts: Vec<ContextView>,
}

/// Frames in ascending time order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OptionsTimeline {
    frames: Vec<Frame>,
}

impl OptionsTimeline {
    #[must_use]
    pub fn new(mut frames: Vec<Frame>) -> Self {
        frames.sort_by_key(|f| f.t);
        Self { frames }
    }

    #[must_use]
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Most recent frame at or before `time_ms`.
    ///
    /// Returns `None` before the first frame: a strategy asking about a moment
    /// the tape had not reached yet must get nothing, not the earliest frame.
    #[must_use]
    pub fn at(&self, time_ms: i64) -> Option<&Frame> {
        if self.frames.is_empty() || time_ms < self.frames[0].t {
            return None;
        }
        let index = match self.frames.binary_search_by_key(&time_ms, |f| f.t) {
            Ok(exact) => exact,
            Err(insert) => insert - 1,
        };
        self.frames.get(index)
    }

    /// The strategy-facing view of the frame at `time_ms`.
    #[must_use]
    pub fn view_at(&self, time_ms: i64) -> Option<OptionsView<'_>> {
        self.at(time_ms).map(view_of)
    }

    /// The same view, for a caller that only ever moves forward.
    ///
    /// `at` binary-searches, which is right for random access and wrong for a
    /// replay: the search reruns for every bar of every parameter cell, and
    /// each probe is a cache miss on a frame carrying two vectors. A bar loop
    /// never goes back, so a cursor answers in amortised O(1). It falls back to
    /// the search if a caller does move backwards, so the answer is the same
    /// either way.
    #[must_use]
    pub fn view_from(&self, cursor: &mut usize, time_ms: i64) -> Option<OptionsView<'_>> {
        if self.frames.is_empty() || time_ms < self.frames[0].t {
            return None;
        }
        if self.frames[*cursor].t > time_ms {
            return self.view_at(time_ms);
        }
        while *cursor + 1 < self.frames.len() && self.frames[*cursor + 1].t <= time_ms {
            *cursor += 1;
        }
        Some(view_of(&self.frames[*cursor]))
    }
}

fn view_of(frame: &Frame) -> OptionsView<'_> {
    OptionsView {
        spot: frame.spot,
        bull_ratio: frame.bull_ratio,
        bull_ratio_15m: frame.bull_ratio_15m,
        net_flow_velocity_norm: frame.net_flow_velocity_norm,
        big_trade_imbalance: frame.big_trade_imbalance,
        clusters: &frame.clusters,
        contexts: &frame.contexts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(t: i64, spot: f64) -> Frame {
        Frame {
            t,
            spot,
            bull_ratio: 0.5,
            bull_ratio_15m: 0.5,
            net_flow_velocity_norm: 0.0,
            big_trade_imbalance: 0.0,
            clusters: Vec::new(),
            contexts: Vec::new(),
        }
    }

    #[test]
    fn the_timeline_never_looks_forward() {
        let timeline = OptionsTimeline::new(vec![frame(1_000, 1.0), frame(2_000, 2.0), frame(3_000, 3.0)]);

        assert!(timeline.at(999).is_none(), "before the first frame there is nothing to know");
        assert_eq!(timeline.at(1_000).map(|f| f.spot), Some(1.0));
        assert_eq!(timeline.at(2_999).map(|f| f.spot), Some(2.0), "must not jump to the next frame");
        assert_eq!(timeline.at(9_999).map(|f| f.spot), Some(3.0));
    }

    #[test]
    fn frames_are_sorted_however_they_arrive() {
        let timeline = OptionsTimeline::new(vec![frame(3_000, 3.0), frame(1_000, 1.0), frame(2_000, 2.0)]);
        assert_eq!(timeline.frames().iter().map(|f| f.t).collect::<Vec<_>>(), vec![1_000, 2_000, 3_000]);
    }

    #[test]
    fn an_empty_timeline_answers_nothing_rather_than_panicking() {
        let timeline = OptionsTimeline::default();
        assert!(timeline.at(1_000).is_none());
        assert!(timeline.view_at(1_000).is_none());
        assert!(timeline.is_empty());
    }
}
