# The scouting team: three roles, and a gate that makes "new" checkable

The owner asked for a standing team hunting mechanisms that are not already on
the market, with everything they do visible on the screen. This is how it is
built and, more importantly, what stops it being theatre.

## The problem with asking for novelty

Thirty registrations are closed in `docs/decisions/` and none survived out of
sample. Every one was a known method. Asking a model for a *new* method has one
overwhelming failure mode: it returns a closed family wearing a new name. "Volume
thrust" becomes "participation impulse"; "range breakout" becomes "compression
release". The words are new, the mechanism is dead, and the desk re-tests it.

A second look at a dead idea is more dangerous than the first, because the
number is different and the difference reads as news.

So **novelty is not asserted by the proposer. It is checked by a role that
cannot propose.** That separation is the whole design.

And one honesty about the brief: nobody can verify that a mechanism is absent
from *the market*. What is verifiable is narrower and still useful — that it is
absent from **this repository's thirty decision records**, and that the scout
can name **why the information would survive being known**. A mechanism with no
answer to "so why has nobody arbitraged it" is not new, it is unexamined.

## The three roles

Each has one job, and each can kill a proposal on its own.

| role | asks | can answer |
|---|---|---|
| **scout** | what mechanism could carry information nobody is using? | a proposal |
| **historian** | have we already tried this, under any name? | `NOVEL` / `VARIANT` / `CLOSED` |
| **feasibility** | can it be tested with what is on this disk, and how? | `TESTABLE` / `BLOCKED` |

The **adversary** (`.claude/agents/adversary.md`) is the fourth and is not part
of the team: it sees only what survives all three, and it attacks the
registration rather than the idea.

A proposal is **shortlisted** only when the historian says `NOVEL` and
feasibility says `TESTABLE`. Anything else is `rejected` and stays on file with
the reason, because the list of ideas that were killed and why is the part that
stops the team going in circles.

**The scout may not overrule either gate.** It is the role with the incentive
to be excited, and it is the only one with no veto.

## What a proposal must contain

Five fields, and a proposal missing any of them is rejected without being read
further:

- **mechanism** — what economic or structural fact produces the information.
  Not a signal. "A 20-period channel break" is a signal; "dealers hedging a
  concentrated option strike must buy into a rally" is a mechanism.
- **why_unarbitraged** — why the information survives being known. Capacity
  limits, an institutional constraint, a cost nobody else pays, a data source
  that is awkward rather than secret. *"Nobody has thought of it" is not an
  answer* and is rejected on sight.
- **data_needed** — named against what is on this disk. Feasibility checks it.
- **falsifier_sketch** — what number, declared in advance, would kill it. An
  idea whose author cannot say how it dies is not a hypothesis.
- **closest_known** — the nearest thing this desk has already closed, named by
  its decision record, and what makes this different. Forcing the scout to find
  its own closest relative is what stops the historian doing all the work.

## Where it lives and how it reaches the screen

One JSON file per proposal in `docs/research/scouting/`, named
`<YYYY-MM-DD>-<slug>.json`. Append-only in spirit: a verdict is added to the
`verdicts` array, never by editing what a role already said.

```json
{
  "id": "2026-09-15-dealer-hedging-flow",
  "title": "…",
  "proposed_at": 1789450000000,
  "scout": {
    "mechanism": "…", "why_unarbitraged": "…",
    "data_needed": ["…"], "falsifier_sketch": "…", "closest_known": "…"
  },
  "verdicts": [
    {"role": "historian", "verdict": "NOVEL", "at": 1789451000000,
     "note": "…", "evidence": ["docs/decisions/2026-09-13-volcond-breakout.md"]}
  ],
  "status": "proposed",
  "registered_as": null
}
```

`status` is **derived, never written by an agent**: `rejected` if any verdict is
`CLOSED` or `BLOCKED`; `shortlisted` when both gates pass; `registered` once
`registered_as` names a file in `docs/hypotheses/`. A role that could set its
own proposal's status could promote it, and none of them can.

`GET /api/research` carries the array, and the **Research** screen renders it
with every verdict and every rejection visible. Nothing is hidden because it was
killed — the graveyard is the useful half.

## Running the team

```
/scout            propose, gate, and write the files
/scout review     re-gate everything still `proposed`
```

It is not a daemon. Each run costs model calls and produces a bounded number of
proposals, and a research team that runs unattended forever is a way to spend
money on plausible text. The gates are what make a run worth its cost.

## What this does not promise

It does not promise a tradeable mechanism. Thirty closed records say the base
rate for that is low, and a team that generated one on its first afternoon
would be a reason for suspicion rather than celebration.

What it promises is narrower: **every idea is written down before it is tested,
its closest dead relative is named, and the reason it was killed survives it.**
That is the thing this repository already does for hypotheses, extended one
step earlier — to the point where an idea is still just an idea.
