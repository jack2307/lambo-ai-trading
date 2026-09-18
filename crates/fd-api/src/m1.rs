//! One-minute bars folded from the tick feed, served while they are still
//! warm.
//!
//! Stage 2 of `docs/plans/2026-09-18-staged-ai-entry.md` asks the model,
//! while a plan sits unfilled, "trigger, wait or cancel" every sixty seconds
//! on the last few minutes of price. The minute series the store holds comes
//! from the export task, which lags up to five minutes — long enough that a
//! limit two points away has filled or gone before the bar that showed it
//! arrives. The only thing this process already has that is fresher is the
//! tick feed: `POST /api/paper/tick` lands about once a second per stream
//! with the broker's bid and ask. So the minutes are folded here, from the
//! quotes, and served with the bar still forming.
//!
//! Three things are deliberate and worth stating once:
//!
//! - **The price is the mid.** `(bid + ask) / 2`, in the price the poller
//!   posts — the broker's own quote for the symbol, `XAUUSD.sc` in dollars per
//!   ounce for gold. Not the forming bar's `close`: that is the broker's own
//!   M15 or M5 candle, which every poller of that stream repeats verbatim,
//!   and its `close` is the last bid, which a trigger decision would read
//!   half a spread wrong on the ask side. The spread itself is kept as a mean
//!   so the trigger can see when it widened.
//! - **Every stream of one market feeds one aggregator.** The 15m and 5m
//!   pollers of `xauusd` read the same `symbol_info_tick` and post the same
//!   quote a fraction of a second apart. A minute is a fact about the market,
//!   not about the poller, so both fold into one series — and the second
//!   copy of a quote is a duplicate, not a tick (see [`Aggregator::fold`]).
//! - **Nothing here is persisted.** The ring is twelve hours in memory; a
//!   restart starts empty and the route says so in `unavailable` rather than
//!   serving a gap as history. The stored minute series is the record; this
//!   is the last twelve hours of it, arriving first.
//!
//! This module reads `AppState` and the one [`LiveBar`] the tick handler
//! already builds. It does not touch a run, a book or a file, and it takes no
//! lock any book takes.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::paper::LiveBar;
use crate::state::AppState;

/// One minute, in epoch milliseconds.
pub const MINUTE_MS: i64 = 60_000;

/// Complete minutes kept per market: twelve hours.
///
/// The trigger question wants the last few minutes; the desk wants to see a
/// session. Twelve hours is a session and a half of gold at 720 bars of seven
/// numbers each — about 40 kB a market — and nothing this process holds in
/// memory for a display needs to outlive the working day. Older minutes are
/// in the store, five minutes late, which is fine for anything that is not a
/// trigger.
pub const RING_MINUTES: usize = 720;

/// The tick this endpoint is described as: the source, so a reader of the
/// JSON knows these are not the stored, exported bars.
pub const SOURCE: &str = "ticks";

/// What the numbers are, spelled out beside them, because the response
/// carries a price, a spread and two clocks and the name `time` cannot say
/// which clock (`docs/decisions/2026-09-17-unit-carrying.md`).
pub const PRICE_UNIT: &str = "mid of the posted bid/ask, in the market's own quoted price";
pub const CLOCK: &str = "UTC epoch ms of the API's receipt of each tick, minute-aligned; not the broker's tick stamp";

/// One minute bar, complete or forming.
///
/// `time` is the minute's **open** in epoch milliseconds UTC, aligned to the
/// minute — the bucket, not the first tick's stamp. Prices are the mid of the
/// posted quote in the market's own price units (dollars per ounce on
/// `XAUUSD.sc`); `spread_mean` is `ask - bid` averaged over the minute's
/// ticks, in the same units.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct M1Bar {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// Distinct quotes folded into this minute. Not the broker's tick volume:
    /// the poller reads the quote once a second, so this is at most sixty
    /// per poller-second, and a duplicate posted by a second poller is not
    /// in it.
    pub ticks: u32,
    pub spread_mean: f64,
}

/// A minute under construction: the bar plus the running sum the mean is
/// taken from. Kept apart from [`M1Bar`] so the wire never carries a sum
/// somebody would have to know to divide.
#[derive(Debug, Clone)]
struct Minute {
    bar: M1Bar,
    spread_sum: f64,
}

impl Minute {
    fn start(time: i64, mid: f64, spread: f64) -> Self {
        Self { bar: M1Bar { time, open: mid, high: mid, low: mid, close: mid, ticks: 1, spread_mean: spread }, spread_sum: spread }
    }

    fn fold(&mut self, mid: f64, spread: f64) {
        let bar = &mut self.bar;
        bar.high = bar.high.max(mid);
        bar.low = bar.low.min(mid);
        bar.close = mid;
        bar.ticks += 1;
        self.spread_sum += spread;
        bar.spread_mean = self.spread_sum / f64::from(bar.ticks);
    }
}

/// The last quote the aggregator accepted: what the next one is judged against.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Last {
    at: i64,
    bid: f64,
    ask: f64,
}

/// What became of one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fold {
    /// Folded into the forming minute (or opened a new one).
    Folded,
    /// The same quote again — the same bid and ask as the last accepted
    /// quote, under whatever stamp. Not counted; not an error.
    Duplicate,
    /// Refused: older than the last accepted tick, or posted without a quote.
    /// Counted in `dropped` so a feed that goes wrong is visible in the
    /// response rather than in a bar that quietly lost its ticks.
    Dropped,
}

/// The minute series for one market.
#[derive(Debug, Default)]
pub struct Aggregator {
    /// Complete minutes, oldest first, at most [`RING_MINUTES`].
    done: VecDeque<Minute>,
    forming: Option<Minute>,
    last: Option<Last>,
    dropped: u64,
    duplicates: u64,
}

impl Aggregator {
    /// Fold one tick, stamped `at` (epoch ms), into the series.
    ///
    /// The dedupe rule is on the QUOTE, and the stamp only orders. Why not
    /// the stamp: the poller does not post the broker's `time_msc`, so `at`
    /// is the API's receipt clock, and two pollers posting the same quote
    /// arrive under two different stamps a few hundred milliseconds apart —
    /// the stamp never matches. The other way round fails too: the receipt
    /// clock has millisecond resolution and two POSTs carrying two different
    /// quotes landed in one millisecond the first time the test below posted
    /// them back to back (2026-09-18), so an equal stamp does not mean the
    /// same tick either. What does: MT5's `symbol_info_tick` returns the
    /// LAST tick, and a tick exists only when bid, ask or last moved, so a
    /// quote whose bid and ask both equal the last accepted one is the same
    /// tick read again, whether by the other poller or by the same poller a
    /// second later in a quiet market. It changes no price and is not a
    /// second tick. A quote that moved away and back to the exact pip within
    /// one poll is lost to this rule, and to the one-second poll before it.
    ///
    /// Out of order (`at` older than the last accepted) is dropped rather than
    /// folded into a minute that has already closed: the clocks here are one
    /// process's own, so an older stamp means the clock stepped back, and a
    /// bar that grows after it has been served is the one thing a trigger
    /// cannot be given. An equal stamp is in order.
    pub fn fold(&mut self, at: i64, bid: Option<f64>, ask: Option<f64>) -> Fold {
        let (Some(bid), Some(ask)) = (bid, ask) else {
            self.dropped += 1;
            return Fold::Dropped;
        };
        if let Some(last) = self.last {
            if at < last.at {
                self.dropped += 1;
                return Fold::Dropped;
            }
            if bid == last.bid && ask == last.ask {
                self.duplicates += 1;
                return Fold::Duplicate;
            }
        }
        self.last = Some(Last { at, bid, ask });

        let mid = (bid + ask) / 2.0;
        let spread = ask - bid;
        let minute = at.div_euclid(MINUTE_MS) * MINUTE_MS;
        match &mut self.forming {
            Some(current) if current.bar.time == minute => current.fold(mid, spread),
            Some(current) => {
                // `at` is monotone (checked above), so the forming minute is
                // older than this one: it is complete. A minute nobody ticked
                // in between is simply absent from `done`, never filled flat —
                // a flat bar would read as a market that stood still when the
                // truth is that nothing was heard.
                let closed = std::mem::replace(current, Minute::start(minute, mid, spread));
                if self.done.len() == RING_MINUTES {
                    self.done.pop_front();
                }
                self.done.push_back(closed);
            }
            None => self.forming = Some(Minute::start(minute, mid, spread)),
        }
        Fold::Folded
    }

    /// The last `n` complete minutes, oldest first.
    #[must_use]
    pub fn bars(&self, n: usize) -> Vec<M1Bar> {
        let skip = self.done.len().saturating_sub(n);
        self.done.iter().skip(skip).map(|m| m.bar.clone()).collect()
    }

    #[must_use]
    pub fn forming(&self) -> Option<M1Bar> {
        self.forming.as_ref().map(|m| m.bar.clone())
    }

    /// The oldest minute still held, complete or forming.
    #[must_use]
    pub fn first_minute_ms(&self) -> Option<i64> {
        self.done.front().or(self.forming.as_ref()).map(|m| m.bar.time)
    }

    #[must_use]
    pub fn last_tick_ms(&self) -> Option<i64> {
        self.last.map(|l| l.at)
    }

    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    #[must_use]
    pub fn duplicates(&self) -> u64 {
        self.duplicates
    }

    #[must_use]
    pub fn complete(&self) -> usize {
        self.done.len()
    }
}

/// One aggregator per market, behind one lock.
///
/// Its own mutex, beside `paper` and `live_bars` and never held with either:
/// a tick folds under it for a few hundred nanoseconds, and a book's step
/// must not wait on it or be waited on by it.
#[derive(Debug, Default)]
pub struct M1Hub {
    markets: Mutex<BTreeMap<String, Aggregator>>,
}

impl M1Hub {
    /// Fold a tick for `market`. See [`Aggregator::fold`].
    pub fn fold(&self, market: &str, at: i64, bid: Option<f64>, ask: Option<f64>) -> Fold {
        self.markets.lock().expect("m1 aggregators").entry(market.to_string()).or_default().fold(at, bid, ask)
    }

    /// The series for `market` as the route answers it.
    #[must_use]
    pub fn snapshot(&self, market: &str, n: usize) -> M1Bars {
        let markets = self.markets.lock().expect("m1 aggregators");
        let agg = markets.get(market);
        let last_tick_ms = agg.and_then(Aggregator::last_tick_ms);
        M1Bars {
            market: market.to_string(),
            tf: "1m",
            source: SOURCE,
            symbol: None,
            price_unit: PRICE_UNIT,
            clock: CLOCK,
            ring_minutes: RING_MINUTES,
            bars: agg.map(|a| a.bars(n)).unwrap_or_default(),
            forming: agg.and_then(Aggregator::forming),
            first_minute_ms: agg.and_then(Aggregator::first_minute_ms),
            last_tick_ms,
            dropped: agg.map_or(0, Aggregator::dropped),
            duplicates: agg.map_or(0, Aggregator::duplicates),
            unavailable: last_tick_ms.is_none().then(|| NO_TICKS.to_string()),
        }
    }
}

/// The sentence a market with no ticks answers with. It names the restart
/// because that is the common way to arrive here: the desk was just
/// redeployed and the pollers have not posted yet, or nobody polls this
/// market at all.
pub const NO_TICKS: &str = "no ticks yet: M1 bars are folded from the tick feed in memory and start empty at every restart";

/// The hook the tick handler calls, once, after it has built the [`LiveBar`]
/// it stores. Takes the same `at` the live bar carries so the minute a tick
/// lands in and the age the desk shows are measured on one clock.
pub fn on_tick(state: &AppState, market: &str, live: &LiveBar) -> Fold {
    state.m1.fold(market, live.at, live.bid, live.ask)
}

/// `GET /api/paper/m1?market=xauusd&n=30`
#[derive(Debug, Deserialize)]
pub struct M1Query {
    pub market: String,
    /// Complete minutes wanted, newest `n`; at most [`RING_MINUTES`].
    #[serde(default)]
    pub n: Option<usize>,
}

/// Complete minutes sent when the query does not say.
pub const DEFAULT_N: usize = 60;

/// The response of `GET /api/paper/m1`.
///
/// Units, spelled out because the wire carries them under short names:
/// `time` on every bar and `first_minute_ms` / `last_tick_ms` are epoch
/// milliseconds UTC on the API's own clock (`clock` says so); prices and
/// `spread_mean` are in the market's own quoted price (`price_unit`), and
/// `symbol` names whose quote that is when the market is in the config.
#[derive(Debug, Clone, Serialize)]
pub struct M1Bars {
    pub market: String,
    pub tf: &'static str,
    pub source: &'static str,
    /// The bar symbol from the config, when the market has one — the quote
    /// these prices are the mid of. `null` for a market the config does not
    /// know, which the tick route accepts and this one therefore answers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub price_unit: &'static str,
    pub clock: &'static str,
    pub ring_minutes: usize,
    /// Complete minutes only, oldest first, at most `n`.
    pub bars: Vec<M1Bar>,
    /// The current minute, or `null` before the first tick.
    pub forming: Option<M1Bar>,
    /// The oldest minute held, complete or forming.
    pub first_minute_ms: Option<i64>,
    /// The last accepted quote CHANGE, not the last POST: a duplicate does
    /// not advance it, so in a market that has not moved for three minutes
    /// this is three minutes old while the feed is alive — and `forming` is
    /// the last minute a quote changed in, not the wall-clock minute. Feed
    /// liveness is `/api/paper/status`'s live age, measured from every POST.
    pub last_tick_ms: Option<i64>,
    /// Ticks refused: out of order, or posted without a quote.
    pub dropped: u64,
    /// Ticks seen twice: the same quote again, from a second poller or from
    /// the same poller in a quiet second. Expected to run at about one per
    /// tick while two pollers post one market; a zero here with two pollers
    /// up would mean they disagree about the quote, which is worth knowing.
    pub duplicates: u64,
    /// `null` when there is data; a sentence when there is none yet.
    pub unavailable: Option<String>,
}

pub async fn m1(State(state): State<Arc<AppState>>, Query(query): Query<M1Query>) -> Result<Json<M1Bars>, ApiError> {
    let n = query.n.unwrap_or(DEFAULT_N);
    if n > RING_MINUTES {
        return Err(ApiError::BadRequest(format!(
            "n={n}: at most {RING_MINUTES} complete minutes are kept (twelve hours, in memory); ask for {RING_MINUTES} or fewer"
        )));
    }
    let mut answer = state.m1.snapshot(&query.market, n);
    answer.symbol = state.config.market(&query.market).ok().map(|m| m.bar_symbol);
    Ok(Json(answer))
}
