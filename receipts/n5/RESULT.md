# n5 — long-horizon gold holds (>= 5 sessions) on a swap-free account: result

Registration: `docs/decisions/2026-10-06-long-horizon-swapfree.md` (committed
alone at `2902fa7`, before any gold number existed).
Rows: `docs/research/designs/2026-10-06-long-horizon-n5.toml`, 8, frozen.
Binary: `/e/rust/fd-wt-crt/target/release/search.exe` (2026-10-04 15:13). **No
build; `cargo` was never invoked on this branch.**
Every run: `--mode=hypotheses --fixed --exit-mix --null-sides=exposure
--seeds=200 --interval=15m --data=/e/rust/flowdesk/data`, spread 0.28, trail
off.
`news:` source, identical in all twelve receipts: **747 events (2010-01-08 ->
2027-12-08) from `E:/rust/flowdesk/data\news\events.parquet`**, scope USD.
`data-sealed/` was not opened, read, pointed at or counted.

## 1. The gate, counted

Gate as registered: **PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades**,
on **both** the IS and the OOS leg of the same market. Not moved.

| arm | guards | swap | cells clearing BOTH legs |
|---|---|---|---|
| **A** | off | 0.00 (the desk's measured account) | **4** |
| **B** | off | -0.83/-0.83 per lot-night (generic rate) | **0** |
| **C** | on | 0.00 | **0** |

The four, all in arm A:

| cell | market | IS leg | OOS leg | mean hold | own rule fired | percentile (null p50) |
|---|---|---|---|---|---|---|
| `qs-h15` | `xauduka` | 190 tr, PF 1.262, +0.130R | 199 tr, PF 1.201, +0.080R | 215 h / 189 h | 189/190, 198/199 | 91% (0.954), 62% (1.110) |
| `ts-l20` | `xauduka` | 183 tr, PF 1.315, +0.095R | 185 tr, PF 1.427, +0.122R | 334 h / 330 h | 182/183, 184/185 | 92% (0.982), 76% (1.188) |
| `ts-l60` | `xauduka` | 93 tr, PF 1.383, +0.151R | 90 tr, PF 2.183, +0.370R | 651 h / 670 h | 92/93, 89/90 | 81% (0.966), 82% (1.211) |
| `qs-h10` | `xauusd` | 49 tr, PF 1.295, +0.096R | 46 tr, PF 1.274, +0.098R | 163 h / 158 h | 48/49, 45/46 | 68% (1.032), 64% (1.149) |

All four are in-family (mean hold >= 120 h) and all four fire their own exit
rule on ~99-100% of exits, so none is the `tsmom/120d` trap.

**Not one of the four — and not one of the 96 cells — reached the 95th
percentile of its own exposure-matched drift null.** Highest anywhere in arm A:
94% (`ts-l20`, VAN-OOS). Every verdict string in arm A reads
`gate pass, inside the noise`; the engine printed `SURVIVES` **only** in arm C,
on four cells, and that is the arm whose own rule fires 22-24% of the time.

## 2. The number worth keeping: the drift null stops losing money

The desk's null median PF at <= 4 h horizons is **0.867** (brief section 5).
Same null machinery, same gate, same instrument, horizon moved to >= 5
sessions, rows with >= 40 trades:

| run | rows >= 40 tr | null p50 min / median / max | rows with null p50 > 1.000 |
|---|---|---|---|
| A-duka-is | 7 | 0.929 / 0.961 / 0.982 | 0/7 |
| A-duka-oos | 7 | 1.091 / 1.135 / 2.137 | **7/7** |
| A-van-is | 4 | 1.022 / 1.040 / 1.056 | **4/4** |
| A-van-oos | 5 | 1.149 / 1.195 / 1.302 | **5/5** |
| B-duka-is | 7 | 0.154 / 0.365 / 0.496 | 0/7 |
| B-duka-oos | 7 | 0.307 / 0.530 / 0.635 | 0/7 |
| B-van-is | 4 | 0.523 / 0.572 / 0.657 | 0/4 |
| B-van-oos | 5 | 0.918 / 0.960 / 0.998 | 0/5 |

**16 of 23** arm-A rows have a null median above 1.000. A cost-matched,
count-matched, side-ratio-matched, exposure-matched **random** hold MAKES MONEY
at this horizon on a swap-free account, which no null in this record has ever
done. The positive-drift band is real and it is measurable through the control.

**0 of 23** do once the generic financing is charged. The drift pays; the
carry eats it, and then some.

## 3. Falsifiers

**Falsifier 1 (registration section 4): did NOT fire.** Four cells clear the
gate on both legs at swap = 0. The >= 5-session band is therefore not empty the
way the <= 4 h band is.

**Falsifier 2 (the honest-naming one): FIRED, on all four.** Charged at the
generic rate, every one collapses:

| cell | PF at swap 0 | PF at the generic rate | nights charged per trade | financing as a multiple of the same trades' round-trip spread |
|---|---|---|---|---|
| `qs-h15` DUKA-IS | 1.262 | **0.594** | 11.80 | **35.0x** |
| `qs-h15` DUKA-OOS | 1.201 | **0.605** | 9.79 | 29.0x |
| `ts-l20` DUKA-IS | 1.315 | **0.309** | 18.62 | 55.2x |
| `ts-l20` DUKA-OOS | 1.427 | **0.494** | 18.03 | 53.4x |
| `ts-l60` DUKA-IS | 1.383 | **0.109** | 37.70 | 111.7x |
| `ts-l60` DUKA-OOS | 2.183 | **0.489** | 38.19 | 113.2x |
| `qs-h10` VAN-IS | 1.295 | **0.769** | 8.71 | 25.8x |
| `qs-h10` VAN-OOS | 1.274 | **1.109** | 7.78 | 23.1x |

Nights per trade are measured from the receipts, not assumed:
`nights = (swap$ / spread$) x (0.28 / 0.83)`, both figures printed by the run.
The last column is `swap$ / spread$` directly — receipt-internal, no outside
number. Translated to R at the stop sizes the record names for these methods
(`quiet-swing` median stop $41.57/oz on `xauusd`, $27.98/oz on `xauduka`,
`docs/research/designs/2026-09-23-designed-3-frozen.toml`; `tsmom` sizes at 2.0
daily ranges where `quiet-swing` uses 1.5, so its per-night R cost is 0.75x):

| cell | financing per trade, as a share of R | expectancy | cost / edge |
|---|---|---|---|
| `qs-h15` DUKA-IS | 35.0% R | +0.130R | **2.7x** |
| `qs-h15` DUKA-OOS | 29.0% R | +0.080R | **3.6x** |
| `ts-l20` DUKA-OOS | 40.1% R | +0.122R | **3.3x** |
| `ts-l60` DUKA-OOS | 84.9% R | +0.370R | **2.3x** |
| `qs-h10` VAN-IS | 17.3% R | +0.096R | **1.8x** |
| `qs-h10` VAN-OOS | 15.5% R | +0.098R | **1.6x** |

**So, in the words the registration committed to: this is an account perk, not
a strategy.** It is real money for *this* login — the exemption is measured,
340 closed positions and 87 overnight holds at swap 0.00 — and the whole of it
is a fee waiver worth 1.6x to 3.6x the edge. **Risk, one line: if Vantage
withdraws the swap exemption on this login, every cell in this family goes from
PF 1.20-2.18 to PF 0.11-1.11 overnight, and there is no parameter that
recovers it.**

## 4. What the guards arm does to this family (brief section 8)

Arm C is not a stricter reading of arm A. It is a different mechanism.

- **The horizon parameter stops existing.** On `C-van-is`, `qs-h5`, `qs-h10`,
  `qs-h15` and `qs-h20` return **identical** numbers — 83 trades, PF 0.885,
  -0.042R, 34% — because `flat_before_weekend_hhmm = 1640` closes every hold
  before `holdSessions` can matter. Same on `C-van-oos` (h10/h15/h20 all 80
  trades, PF 1.512) and `C-duka-is` (h15 and h20 both 393 trades, PF 1.255).
  A one-month horizon and a one-week horizon are the same run.
- **Sample inflation, measured here:** `qs-h20` 174 -> 393 trades (2.3x),
  `ts-l250` 28 -> 344 (**12.3x**), `ts-l120` 67 -> 464 (6.9x).
- **Hold collapse:** `qs-h15` 215 h -> 67 h; `ts-l250` 1,525 h -> 86 h.
- **The trap reproduced exactly.** `C-van-is / ts-l120`: PF 2.128, +0.157R, 47
  trades, `gate pass`, and its own rule fired **0 of 47** times. Void by the
  registration's rule 3, and it is the only arm-C `xauusd` IS cell that clears
  anything.
- **`SURVIVES` appears only here:** 4 cells on `C-duka-is`, percentiles 98-99,
  own rule firing 89-96 times out of 393-400. All four fail their OOS leg
  (PF 1.037-1.067). Zero arm-C cells clear both legs.

## 5. A tool defect this family exposes

**`expectancy` and `total_r` are blind to overnight financing; `profit_factor`
is not.** `engine.rs` close_position sets `r = points / position.risk` — a pure
price move — while `pnl_usd = points*lots*contract - commission + swap`.
`metrics_of` then takes `expectancy = mean(r)` and `profit_factor =
sum(wins.pnl_usd) / |sum(losses.pnl_usd)|`.

Measured consequence, visible in the receipts: `qs-h15` DUKA-IS prints
**expectancy +0.130R in both arms**, identical to three decimals, while PF goes
1.262 -> 0.594 and -$6,166 of swap is charged. Half the gate therefore cannot
see the dominant cost of a multi-night mechanism. (Spread *is* inside `r`,
because `apply_costs` moves the fill prices; commission and swap are not.)

Harmless at <= 4 h, where `swap_nights` returns 0. Not harmless here, and it
is why every number in section 3 is read off PF, not off expectancy.

## 6. Multiplicity ledger

| | declared | viewed |
|---|---|---|
| gold cells (run x row) | 96 | **96** (12 runs x 8 rows) |
| silver timing cell | 1 | 1 (`xagduka`, `holdSessions = 5`, `--seeds=10`, run before registration to time the binary; its number is not read) |

No threshold was moved, no window was changed after a result was seen, no row
was added, no sample floor was lowered. The only amendment is the dated note in
section 11 of the registration, which records an imprecision in the
registration's own wording and does not move anything.
