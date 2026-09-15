---
name: scout
description: Run the scouting team — three roles hunting mechanisms this desk has not already closed, with every proposal and every rejection written to docs/research/scouting/ and shown on the Research screen. `/scout` for a fresh round, `/scout review` to re-gate everything still awaiting a verdict, `/scout register <id>` to promote a shortlisted idea into a real hypothesis.
---

# /scout — ideas, before they are hypotheses

Read `docs/research/SCOUTING.md` first. It is short and it is the design.

You are the manager here, not a proposer. **Do not add ideas of your own.** The
whole value of this pipeline is that the role which proposes has no veto and
the roles which can veto do not propose; a manager who quietly slips in a
favourite has collapsed both halves of that.

## Why this exists at all

Thirty-one registrations are closed in `docs/decisions/` and none survived out
of sample. Every one was a known method. The failure mode of asking a model for
*new* methods is that it returns a dead family under a new name — "volume
thrust" comes back as "participation impulse" — and the re-test produces a
different number which then gets believed, because it is new.

So novelty is checked, never asserted. That is the only thing here that matters.

## `/scout` — a fresh round

**1. Scout.** One `scout` subagent (`.claude/agents/scout.md`). Tell it to read
the whole graveyard — `docs/decisions/` and `docs/research/BACKLOG.md` — before
it proposes anything, and to write **three to five** proposals as JSON into
`docs/research/scouting/`. Not more. If it can only defend two, two is the
honest answer and a result about the search space.

**2. The two gates, in parallel.** For each new proposal:

- a `historian` subagent → `NOVEL`, `VARIANT` or `CLOSED`, with the decision
  record that says so. `VARIANT` is not fatal but must name what is different
  and why that difference is a *mechanism* rather than a parameter.
- a `feasibility` subagent → `TESTABLE` or `BLOCKED`, naming the instrument,
  the sample it would get, and the cost hurdle. It must open the data files
  rather than assume them.

Append each verdict to the proposal's `verdicts` array. **Append — never edit
what a role already said**, and never touch `status`: the server derives it, so
that nothing can promote itself.

**3. Report.** The shortlist, the rejections with their reasons, and — say this
plainly — if nothing was shortlisted, that nothing was. A round that kills
everything is a working round, not a failed one.

## `/scout review`

Re-run only the gates, over every proposal whose `status` is still `proposed`.
Use it after a scout run was interrupted, or when new data arrives that could
unblock something: a `BLOCKED` verdict is about the disk on the day it was
written, and `data/spreads/` did not exist last week.

Do **not** re-gate a `rejected` proposal without a stated reason. Re-gating
until something passes is the search this repository spent thirty-one
registrations learning not to do.

## `/scout register <id>`

Promote one **shortlisted** proposal into a real hypothesis:

1. Refuse if its status is not `shortlisted`. Say which verdict is missing.
2. Hand it to `/research <idea>`, which takes over with the full discipline —
   pre-registration committed before anything runs, declared falsifier, nulls,
   the review team, a decision record.
3. Set `registered_as` on the proposal to the hypothesis file. That is the one
   field this skill may write, and it is what closes the loop between the board
   and `docs/hypotheses/`.

## The rules that are not negotiable

- **Three to five proposals a round.** Twenty proposals is one idea and
  nineteen rewordings, and the gates will spend the run saying so.
- **A proposal missing any of the five fields is rejected unread.** Mechanism,
  why_unarbitraged, data_needed, falsifier_sketch, closest_known.
- **"Nobody has thought of it" is not a `why_unarbitraged`.** Reject it.
- **Nothing here is a strategy, a paper run, or a reason to change a book.** A
  shortlisted proposal is an idea that has earned the right to be
  pre-registered, and nothing more.
- Everything written lands in `docs/research/scouting/` and appears on the
  **Research** screen, rejections included. The graveyard is the useful half.
