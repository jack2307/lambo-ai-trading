//! A strategy that decides nothing, so that something outside the process can.
//!
//! Every other strategy in this registry is a rule: it reads bars and returns
//! an intent, synchronously, in microseconds. A model behind a network call
//! cannot live in that signature and must not try — `on_bar` runs inside the
//! backtest loop and a strategy that blocks there would make a sweep
//! unrunnable and a paper book unreliable.
//!
//! So this one is a **mailbox**. It always returns `Intent::None`, and a run
//! configured with it is driven entirely by `POST /api/paper/intent`, which
//! sets the book's pending intent directly. The external process reads the
//! bars it wants over the API, takes as long as it likes to think, and posts
//! an entry naming the bar it decided on.
//!
//! Three properties fall out of that, and all three are the point:
//!
//! * **No look-ahead is possible.** A posted intent fills at the *next* bar's
//!   open, exactly like every rule in this registry, because it goes through
//!   the same `pending` slot. There is no path by which an outside decider
//!   can act on the bar it was shown.
//! * **A late decision is dropped, not applied.** The intent names the bar it
//!   was made on; if that bar is no longer the last one, the run has moved on
//!   and the intent is refused. Slowness costs a trade, never a bad fill.
//! * **The engine still owns the exit.** `Exits::Engine`, so the stop, the
//!   target and the maximum hold are the desk's, not the decider's. An
//!   outside process may propose a trade; it may not propose an unbounded one.
//!
//! It is deliberately useless in a backtest — a sweep over it produces
//! nothing, because there is no mailbox to read. That is correct: the thing
//! it exists for cannot be backtested, only run forward and measured against
//! a control, which is what `docs/paper/AI-TRADER.md` sets up.

use fd_indicators::IndicatorSpec;

use crate::registry::{BarContext, Exits, Intent, Params, Strategy};

pub struct External;

impl Strategy for External {
    fn id(&self) -> &'static str {
        "external"
    }
    fn name(&self) -> &'static str {
        "External decider"
    }
    fn description(&self) -> &'static str {
        "Decides nothing itself; a run using it is driven by POST /api/paper/intent. The engine still owns the stop, the target and the maximum hold."
    }
    fn default_params(&self) -> Params {
        // `atrPeriod` only, because the engine needs a sizing series even when
        // the decider supplies its own stop — and it must size the trade the
        // same way it sizes every other book, or the comparison is not one.
        Params::new(&[("atrPeriod", 14.0)])
    }

    fn indicators(&self, p: &Params) -> Vec<IndicatorSpec> {
        // The ATR the ENGINE sizes with, not one this strategy reads. A book
        // driven from outside must be sized by exactly the same rule as every
        // other book on the desk, or the comparison between them is not one.
        vec![IndicatorSpec::new("atr").with("period", p.get("atrPeriod"))]
    }

    fn warmup(&self, p: &Params) -> usize {
        p.get("atrPeriod").max(1.0) as usize
    }

    fn exits(&self) -> Exits {
        Exits::Engine
    }

    fn on_bar(&self, _ctx: &BarContext) -> Intent {
        // Never anything. The mailbox is filled from outside the process.
        Intent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Params;

    #[test]
    fn it_never_decides_anything_on_its_own() {
        // The guarantee the whole design rests on: whatever the bars do, this
        // strategy proposes nothing. Every trade on such a run came from
        // outside and is attributable to whoever posted it.
        let s = External;
        assert_eq!(s.id(), "external");
        assert!(matches!(s.exits(), Exits::Engine), "the desk keeps the stop and the clock");
        assert!(s.series(&Params::new(&[])).is_empty(), "it reads no series, so it can read no signal");
    }
}
