# Staged AI entry: plan at the bar, trigger on ticks, funnel the cadence

Status: **APPROVED by the owner 2026-09-18 evening.** Stages 0, 3, 4 shipped
that evening (294d358, 4af81c3); stages 1 and 2 merged at 64b34d6 (API:
agent/pending-orders, agent/m1-ticks; Python: agent/plan-prompt) and run on
paper only. This is the plan as approved, with what was measured to write it. Each stage is its own registered campaign with a coin
control, under the three-file rule and the 30-trade / two-window bar that
every other prompt variant is held to.

## What runs today, measured

Per AI book (`py/live/ai_trader.py`):

- polls `/api/paper/run/{id}?bars=240` every 20 s; on a NEW 15m bar it makes
  ONE call: 40 bars printed one per line, the desk rulebook, EMA/RSI/session/
  day levels/profile/8 hourly bars, then the variant blocks (OTL, HTF).
  Answer: `{side, stop, target, reason}`; the intent fills at the NEXT bar's
  open; the executor mirrors it one bar later still (2a `--fill-on-open`
  removes that and is dormant).
- while a position is open the same cadence runs a `hold_check` instead
  (recorded, never acted on).
- 2026-09-18, 4 DeepSeek books: 416 calls, 492k in, **830k out**, of which
  the model's own reasoning is 5.7–7.0k tokens per decision and ~2k per hold
  check. Reasoning is billed as output at 4x the input rate, so it is 87% of
  the bill. Prefix-cache hits: 1%. Spend ≈ $1.1–1.5 a day at list.
- the model cannot say "wait for 4331". It can only take the next open or
  stand aside. That is the real limitation; the boundary-market cost itself
  measured 0.14 pt (`docs/hypotheses/…entry-lag`).

DeepSeek facts read on 2026-09-18 from api-docs.deepseek.com: thinking is on
by default at effort `high`; it is switched per request with
`thinking: {type: "disabled"}` or `reasoning_effort: low|high|max`; context
caching is on for everyone, keyed on an exact prefix, persisted at request
boundaries, and reported as `prompt_cache_hit_tokens`; cached input is 50x
cheaper than fresh ($0.006 vs $0.30 per million). Cache life is "hours to
days", no SLA.

## The idea, in the owner's words

The 15m bar is the expensive moment: give the model everything once. If it
declares an intention, the follow-up calls get more frequent and much smaller,
because everything already said is a cached prefix and the only new thing is
the last few minutes. Each stage narrows what goes in and what comes out.

## Stages

### Stage 0 — flip `--fill-on-open` (built, dormant)

Removes the one-bar mirror lag, the largest measured cost. New campaign
generation, so the mirrored books get new run ids. Half a day, mostly the
registration and the restart in a quiet window.

### Stage 1 — the model answers with a PLAN, not an order

Prompt v2 answer:

```
{"side": "LONG"|"SHORT"|"NONE",
 "entry": {"type": "market"|"limit"|"stop", "price": <price or null>},
 "zone": [lo, hi],          # where the entry is still valid
 "stop": <price>, "target": <price or null>,
 "valid_bars": <1..4>,       # cancelled unfilled after this many 15m bars
 "invalidate_above"/"invalidate_below": <price or null>,
 "reason": "<one sentence>"}
```

- **API**: `IntentRequest` gains `entry`, `zone`, `valid_bars`, invalidation.
  The paper book gets a pending-order slot beside the market one: filled from
  the tick feed the API already receives (`POST /api/paper/tick` carries
  bid/ask), a limit fills when the touched side crosses the price, a stop when
  it trades through, expiry cancels it and writes a `cancelled_unfilled` row so
  a missed trade is COUNTED and not silently absent. A tick gap over the
  window is "unfilled", logged as such.
- **Coin control**: the same entry type at mirrored distances (a limit 6 pt
  below on the book is a limit 6 pt above on the coin), so the control shares
  the fill mechanics and the comparison stays fair.
- **Executor**: `BUY_LIMIT/SELL_LIMIT/BUY_STOP/SELL_STOP` with
  `ORDER_TIME_SPECIFIED` expiry instead of `TRADE_ACTION_DEAL`; cancel on the
  book's expiry; three-key real-money lock unchanged. Not on the real account
  until the paper comparison below has 30 trades.
- **Measured against** the market variant on the same model, same bars:
  fill rate, winners missed (what the market entry would have returned on the
  same bar, computable from the bars), points saved at entry, net R. The
  falsifier is registered before the first call: if net R of the plan variant
  is not above the market variant by the second window, limit entries are
  dropped and only the funnel (stages 2–4) survives.
- Effort: 2–3 days (Rust pending-order fill + tests, prompt v2 + parser +
  `sane()` rules, executor pending path + selftest, registration).

### Stage 2 — the cadence funnel

While a plan is pending and unfilled:

- the trader switches from "once per 15m bar" to **every 60 s, or on any
  tick inside the zone**;
- the prompt is `[static rulebook] + [the 15m block, frozen verbatim for the
  window] + [NEW: last k M1 bars and the quote]`. The first two parts are an
  exact repeat of the stage-1 request, so DeepSeek serves them from cache;
  only the M1 tail is fresh (~200 tokens);
- the question is narrow: `{"action": "TRIGGER"|"WAIT"|"CANCEL", "reason"}`,
  sent with `thinking: disabled` (or `reasoning_effort: low` as a second
  arm), output capped at 60 tokens;
- M1 bars come from the API aggregating its own tick feed (the export task
  lags up to 5 min and cannot time an entry). One new aggregation in
  `paper.rs`, served on `/api/paper/m1` with the forming minute (built
  2026-09-18 as `crates/fd-api/src/m1.rs`; the chart route was not reused so
  the exported 1m series and the tick-built one never share a name).
- **Compared with** the rule-only trigger from stage 1 (a limit that fills
  itself): does a model at the trigger add anything, or only cost? Two arms,
  same plan source, registered.
- Cost of the fast calls, per book per day: ~10 windows × ~10 calls ×
  (3k cached + 200 fresh in + 60 out) ≈ **$0.01**. The 15m call is unchanged.
- Effort: 1–2 days after stage 1.

### Stage 3 — the hold funnel (can go first; it is advisory)

Today: a hold check every bar while open, ~2k reasoning tokens each, 117 of
them on 2026-09-18 for $0.26. Replace with an event-triggered check:
only when price has moved more than 0.5R against, or an invalidation level
is crossed, or a new HTF structure event lands; `thinking: disabled`, output
≤ 60 tokens. The verdict is recorded and not acted on, exactly as now, so
this changes no book's result and needs no coin. Saves ~30% of the current
spend. Half a day.

### Stage 4 — order the prompt for the cache

Static rulebook as the system message; then the slow blocks (D1/H4/H1 facts,
which change hourly at most); then the desk state; then the 15m bars; keep
number formatting stable so a repeated block is byte-identical. Expected
prefix hits on the 15m call: from 1% to well over half. No behaviour change;
a prompt-order change is still a new variant id for the record. Half a day.

## Expected token shape per book per day (4 books today)

| | calls | input | output | list cost |
|---|---|---|---|---|
| today | ~55 decision + ~30 hold | 123k (1% cached) | 208k, mostly reasoning | ~$0.30 |
| after 3+4 | ~55 decision + ~5 hold | 123k (>50% cached) | ~170k | ~$0.20 |
| after 1+2 | + ~100 trigger calls | +20k, ~90% cached | +6k | +$0.01 |

The decision call's reasoning stays the main cost by design: that is the call
worth paying for. Everything else becomes nearly free.

## Order of work, if approved

1. Stage 0 today (owner's flip, registration, quiet-window restart).
2. Stage 3 and stage 4 this week: cheap, no science risk, immediate bill cut.
3. Stage 1 as a registered variant beside the market one, paper only.
4. Stage 2 on top of 1, two arms.
5. Real money only for a stage that wins its comparison at 30 trades over
   two windows; the funded account keeps mirroring the market books until
   then.

## Risks named

- Limit entries miss runners; the paper comparison is the only honest judge,
  and the falsifier above says what happens if they lose.
- Pending orders on the funded account add partial-fill and expiry states the
  executor has never handled; the selftest gains them before the flag exists.
- Paper fills from ticks depend on the tick feed being up; a gap is an
  unfilled order, never an assumed fill.
- `thinking: disabled` on the trigger is a quality change; that is why it is
  an arm and not a default.
- DeepSeek's cache has no SLA; the hit rate is logged per call and the prompt
  order is worth having even at zero hits.
