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

## This rule is weak, and it is worth knowing why

**Everything above survives only if whoever quotes a number remembers to say
which machine made the bars.** That is the same kind of guarantee as a
comment that is true when written, a launcher that carries its own copy of
the registry, a help string listing the timeframes someone typed out once, or
a warning that fires on every run. This desk spent 2026-09-18 finding five of
those in five different files, and each one had held for a while before it
stopped.

So read this document as buying time, not as a fix. It is written down
because that is better than nothing, and it is not a check.

## The check that does not need remembering

`mt5_export.py` stamps provenance into each parquet's own metadata, so a file
can be asked what it is instead of inferred from its path:

```
python -c "import glob,pyarrow.parquet as pq; [print(p, (pq.read_schema(p).metadata or {}).get(b'exported_at', b'-').decode()) for p in sorted(glob.glob('data/bars/*.parquet'))]"
```

`exported_at` is the honest discriminator available today. The desktop corpus
is frozen at whenever it was last exported by hand; the VPS copy advances
every time the task runs, so a recent stamp on a machine you did not just
export from means you are looking at the live mirror. The metadata also
carries `broker_symbol`, `server`, `contract_size` and `clock_rule`, which
answer the next three questions a study usually has.

Two limits, both real:

- **It distinguishes "recently exported" from "frozen", not one machine from
  another.** Two machines both exporting hourly would look alike. One extra
  key in `mt5_export.py`'s metadata dict — the hostname — would make it
  definitive, and it is one line in a dict that already has ten.
- **Only the MT5 exports carry it at all.** Measured 2026-09-18 on the
  desktop: `XAUUSD`, `EURUSD` and `BTCUSD` are stamped; `XAUDUKA`, `XAGDUKA`,
  `EURDUKA`, `BTCUSDT` and `GC` answer `-`, because they came through other
  importers. That absence is informative rather than a gap - no
  `exported_at` means it is not an MT5 export, which on this desk narrows it
  to the Dukascopy corpus, the Binance series or the GC futures file - but it
  means the command classifies by IMPORTER first and by machine second.

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
