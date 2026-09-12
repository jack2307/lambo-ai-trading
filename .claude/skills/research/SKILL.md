---
name: research
description: Run one trading hypothesis through the whole discipline — pre-register, implement if needed, in-sample walk-forward against matched and direction nulls, then the pre-chosen out-of-sample market, then the review team and a decision record. `/research <idea>` for a new idea, `/research next` for the top of docs/research/BACKLOG.md, `/research <id>` to resume one.
---

# /research — one hypothesis, start to finish

You are the manager (the arbiter). You run the process; the gates and the
veto roles decide. The one failure this project has actually suffered is a
number that looked right, so every step below exists to make that harder.
Do them in order. Do not skip a step because the result already looks
obvious — the ICT chain looked obvious in both directions on the same day.

## 0. Pick the hypothesis

- `/research next`: take the first unchecked item in `docs/research/BACKLOG.md`.
- `/research <idea text>`: the user's idea; add it to the backlog first.
- `/research <id>`: resume from the `Status:` line of `docs/hypotheses/<id>.md`.

Ask `historian` (subagent) whether it or a near relative has been tried:
`docs/decisions/` and the backlog's Closed list. If it has, stop and say so
with the record's path. A re-test of a closed idea needs a *new reason*, not
a new name.

## 1. Pre-register — before any code, before any run

Ask `researcher` (subagent) for the spec. Write two files from it:

- `docs/hypotheses/<id>.md` from `docs/hypotheses/TEMPLATE.md`: claim,
  falsifier, base method (existing id or `NEW:` spec), **in-sample and
  out-of-sample markets both named now**, sample needed, what each outcome
  means.
- `docs/hypotheses/<id>.toml`: the `[run]` table and the `[[hypothesis]]`
  batch — at most a handful of entries, each with a `why`. Filters are the
  one-line spellings `Filter::parse` accepts (`weekdays`, `hours:0800-1200`,
  `sessions:0100-0500|0600-1000`, `flat:1630-1815`, `vol:14/100:1.2-99`).

Commit both with the message `Pre-register <id>`. The commit hash is the
proof the file predates the result.

Available data (check `data/bars/`): `xauusd` 1m (three months), 5m (1.4y),
15m (4.2y); `xauduka` 1m (four years, Dukascopy, out-of-sample structure);
`btcusd` 5m/15m; `btc` (Binance) 1m/15m. Default pairing for gold minutes:
in-sample `xauusd:1m`, out-of-sample `xauduka:1m`.

## 2. Implement, if the base is new

Spawn `strategy-implementer` with the spec. It must return test output and
clippy clean. Read its "exact entry rule as implemented" against the spec
yourself; if they differ, send it back — do not patch the rule to make a run
happen. Commit: `Implement <id>`.

`cargo build --release -p fd-backtest --bin search` afterwards; the runner
uses the release binary.

## 3. In-sample

```
python scripts/research-run.py docs/hypotheses/<id>.toml --stage in
```

Outputs land in `docs/research/runs/<id>/`. Read `in-sample.txt` and every
`direction-*.txt`. Update the hypothesis file's `Status:` and paste the
verdict table into it. Commit: `<id>: in-sample`.

If **nothing** survives (gate + ≥ 95th percentile of the matched null), go to
step 5 — there is no out-of-sample question to ask. Do not widen the grid, do
not add a filter, do not try the other timeframe "just to see". That is the
search the nulls exist to catch.

## 4. Out-of-sample — only for survivors, only the pre-registered market

```
python scripts/research-run.py docs/hypotheses/<id>.toml --stage oos
```

The runner refuses to run this before step 3 has a file. Read
`out-of-sample.txt`. A survivor that fails here is closed; it does not get a
second out-of-sample market.

## 5. Review

Spawn the three veto roles in parallel, each pointed at a *different*
artefact, and require a citation from each:

- `data-integrity` → the hypothesis file + the data files named in it
  (clock, coverage, whether the OOS feed is really out of sample).
- `adversary` → `docs/research/runs/<id>/*.txt` (nulls, percentiles, trade
  counts, multiplicity across the batch).
- `risk` → the trade lists / drawdowns in the in-sample output and the
  strategy's exits.

Advisory roles (`execution-realist`, `portfolio`, `historian`) get the same
files and one question each. Any veto with evidence stands; you do not break
ties by preference.

## 6. Record and close

Write `docs/decisions/<date>-<id>.md` from `docs/decisions/TEMPLATE.md`: the
tables verbatim from the run files, what each role said (quoting), what would
reopen it, what it does not say. Set the hypothesis file's `Status: decided`,
move the backlog item to Closed with the record's path, commit:
`Decision: <id>`.

Then tell the user in a few lines: the claim, the in-sample and out-of-sample
numbers, the verdict, and the one thing that would reopen it. A survivor at
this point earns a **paper** run proposal, nothing more — no broker order is
ever placed by anything in this repository.

## Standing rules while in this loop

- A failing gate is the result. Never re-tune to pass it.
- The out-of-sample market is opened once, after the in-sample record exists.
- Every number quoted in a record comes from a file under
  `docs/research/runs/`, never from memory.
- An LLM (you included) may veto or narrow; it never chooses a direction, sets
  a price, or raises size.
- The MT5 terminal on this machine is a live account: read-only, always.
