# Which machine's bars a result was measured on

**Short version: every closed registration in `docs/decisions/` was measured
on the DESKTOP's `data/bars`, and the VPS copy of `data/bars` is a live
mirror that is not a research input.**

This file exists because from 2026-09-18 those two directories stopped being
copies of each other, and nothing in a parquet's name says which one it came
from.

## What changed

The VPS runs `flowdesk-bars-export` (`deploy/run-bars-export.cmd`), which
pulls M1 through D1 from the price terminal into `C:\flowdesk\data\bars` and
**merges** into whatever is already there. It exists because
`GET /api/paper/htf` and the chart read those files and `data/` is gitignored,
so no commit can ship them — the bars have to be produced on the machine that
reads them.

So on the VPS those files move. `XAUUSD-15m.parquet` there gains bars on
every run, and it is the same filename the desktop's research corpus uses.

## The rule

- **Research reads the desktop.** `E:\rust\flowdesk\data\bars` is the corpus
  the closed registrations were measured on: Dukascopy gold, silver and
  EURUSD 2010–2026, plus the Vantage MT5 exports. It is not written by any
  schedule. A study that wants to be reproducible reads that.
- **The VPS copy serves the live desk.** It is written every hour by a
  scheduled task, it holds only what the desk needs, and its history begins
  whenever that symbol and timeframe were first exported — not in 2010.
  Nothing has been lost there, because the export merges rather than
  truncates; it is simply a different and shorter series.
- **A result measured on one is not automatically true of the other.** The
  two will diverge further with every VPS export, and the divergence is
  silent: same filename, same schema, same symbol.

## What would make this unnecessary

A registration that named the file **and its content hash** could say what it
actually read rather than which path it read from, and the question would
stop depending on which machine somebody was sitting at. That is d1's
suggestion and it is the only version that survives two machines. Nobody owns
it yet; it is in `docs/research/BACKLOG.md` terms a small, real task.

Until then, a study that quotes a number should say which machine produced
the bars — and the answer, for everything closed before 2026-09-18, is the
desktop.

---

Written 2026-09-18 when the VPS export went from two timeframes to six and
the corpus question stopped being hypothetical. Related:
`docs/hypotheses/2026-09-18-htf-context.md`, which is the first registration
whose *live* inputs come from the VPS copy while its *prior* was measured on
the desktop's.
