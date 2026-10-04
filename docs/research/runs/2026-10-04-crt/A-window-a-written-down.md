# Window A, written down and committed before Window B was opened

Receipts: `A-in-sample-cells.txt` (the nine declared cells),
`A-in-sample-flipped.txt` (the nine flipped-sides diagnostic rows).
`xauduka:15m`, `--from=2018-06-16 --to=2022-06-16`, 94,125 bars from
2018-06-17 22:00 to 2022-06-14 23:45 UTC, `--fixed --guards --seeds=300
--null-sides=exposure`, spread 0.28 price units per round trip, swap 0.00.

## The nine cells

| cell | trades | PF | expectancy R | null p50 PF | null p95 PF | raw pct | F3 bands | verdict |
|---|---:|---:|---:|---:|---:|---:|---|---|
| `crt/1h-opposite` | 5,332 | 0.735 | −0.137 | 0.783 | 0.841 | 10% | count 0.77, cost 0.86, time 1.18 — in band | **refuted, F1** |
| `crt/1h-mid` | 5,332 | 0.541 | −0.179 | 0.768 | 0.827 | 0% | time 2.60 — **unmatched** | **refuted, F1** |
| `crt/1h-rr15` | 4,937 | 0.760 | −0.155 | 0.786 | 0.841 | 26% | count 0.79, cost 0.93, time 0.98 — in band | **refuted, F1** |
| `crt/4h-opposite` | 1,159 | 0.728 | −0.155 | 0.835 | 0.937 | 3% | time 1.27 — **unmatched** | **refuted, F1** |
| `crt/4h-mid` | 1,135 | 0.727 | −0.128 | 0.824 | 0.919 | 4% | time 2.11 — **unmatched** | **refuted, F1** |
| `crt/4h-rr15` | 1,158 | 0.775 | −0.116 | 0.835 | 0.933 | 18% | time 1.29 — **unmatched** | **refuted, F1** |
| `crt/1d-opposite` | 15 | 0.689 | −0.149 | 0.838 | 1.723 | 28% | count 3.07, cost 3.03, time 3.49 — **unmatched** | **refuted, F1 (trades and gate)** |
| `crt/1d-mid` | 15 | 1.218 | +0.017 | 0.845 | 1.730 | 82% | count 3.07, cost 2.98, time 4.14 — **unmatched** | **refuted, F1 (trades and expectancy)** |
| `crt/1d-rr15` | 15 | 0.756 | −0.105 | 0.830 | 1.652 | 37% | count 3.07, cost 3.04, time 3.67 — **unmatched** | **refuted, F1 (trades and gate)** |

**Survivors: 0 of 9.** The binary's own closing line: "Nothing survived: no
hypothesis is both past the gate and outside its own null."

Not one cell needed the null to be refuted: every one fails F1 on the gate
alone, and no cell reached even the uncorrected 95th, let alone the corrected
99.44th.

## The nine flipped-sides rows (F4)

| cell | pct | mirror | mirror pct | sum | F4 |
|---|---:|---|---:|---:|---|
| `crt/1h-opposite` | 10% | `crt-flip/1h-opposite` PF 0.674 | 0% | 10 | passes |
| `crt/1h-mid` | 0% | PF 0.515 | 0% | 0 | passes |
| `crt/1h-rr15` | 26% | PF 0.755 | 19% | 45 | passes |
| `crt/4h-opposite` | 3% | PF 0.794 | 27% | 30 | passes |
| `crt/4h-mid` | 4% | PF 0.705 | 1% | 5 | passes |
| `crt/4h-rr15` | 18% | PF 0.844 | 57% | 75 | passes |
| `crt/1d-opposite` | 28% | PF 0.905 | 56% | 84 | passes |
| `crt/1d-mid` | 82% | PF 1.055 | 70% | 152 | passes |
| `crt/1d-rr15` | 37% | PF 0.914 | 56% | 93 | passes |

No pair sums to within 10 points of 100, so none of these percentiles is a
restatement of gold's direction. **F4 passes on all nine** — and the reading
that matters is simpler than the percentiles: **all nine mirrors also lose
money**, profit factor 0.515 to 1.055. A rule that loses in both directions is
not pointing the wrong way; it is paying a cost and receiving nothing.

## F5, the falsifier the registration expected to fire

The opposite-extreme cell must beat its own 1.5R sibling on both profit factor
and expectancy. On Window A:

| range candle | opposite PF / expectancy | rr15 PF / expectancy | F5 |
|---|---|---|---|
| 1H | 0.735 / −0.137 R | **0.760 / −0.155 R** | split — rr15 better on PF, opposite better on expectancy |
| 4H | 0.728 / −0.155 R | **0.775 / −0.116 R** | **FIRED** — rr15 better on both |
| 1D | 0.689 / −0.149 R | **0.756 / −0.105 R** | **FIRED** — rr15 better on both |

F5 fires on two of three range candles outright and is not cleared on the
third. CRT's variable target does not beat `pdhl` mode 0's constant 1.5R exit
on the same structure anywhere in this window.

## The one thing in this table I do not believe, stated before Window B

**Fifteen trades over four years on the 1D cells is not a thin result, it is a
defect in my own measurement, and I expect to be able to name it.** Four years
holds about 1,460 completed daily candles; a one-sided sweep that closes back
inside should happen on a sizeable fraction of consecutive pairs, so the
expected order of magnitude is hundreds, not fifteen. The receipt points
straight at the cause: the 1D cells' **realised median stop is 2.738 ATR(14)
= 4.38 price units** against a declared ceiling of `maxRiskAtr = 3.0`. The ATR
is measured on **15-minute** bars while the stop spans a **daily** sweep, so a
fixed multiple of the 15m ATR is a tiny absolute distance at daily scale and
refuses almost every 1D setup. That makes the registration's `rangeTf` axis
partly confounded: it is not only comparing timeframes, it is comparing how
many of each timeframe's setups survive one absolute risk ceiling.

This is written down here, before Window B is opened, and will be quantified
after Window B is recorded — as a declared diagnostic that cannot produce a
survivor, not as a tenth cell.
