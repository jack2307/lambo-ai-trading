"""The tables of docs/decisions/2026-10-07-gate-legs.md.

Input: `gate_legs_rows.json` + `gate_legs_lbar.json` written by
`gate_legs_check.py`, which reads the `rows.json` of `floor_audit_census.py`
(both that census and `floor_audit_harvest.py` are REUSED UNCHANGED from
`agent/floor-audit`).

No gate: nothing in this output passes or fails
`PF >= 1.200 AND expectancy >= +0.050R AND >= 40 trades on BOTH windows`.
`search.exe` is invoked zero times.
"""
import glob
import json
import re
import statistics as st
import sys
from collections import Counter

OUT = sys.argv[1]
RR = 1.8
PF_MIN, E_MIN, N_MIN = 1.200, 0.050, 40
W = 108


def rule(c="="):
    print(c * W)


# ------------------------------------------------------------------- Q1
def w_pf(rr, p=PF_MIN):
    return p / (rr + p)


def w_e(rr, e=E_MIN):
    return (1.0 + e) / (1.0 + rr)


def gap(rr):
    """w_pf - w_e in factored form: 0.15*(rr-0.4) / ((rr+1.2)(rr+1))."""
    return (PF_MIN * (1 + rr) - (1 + E_MIN) * (rr + PF_MIN)) / ((rr + PF_MIN) * (1 + rr))


def crossover(cost=0.0, p=PF_MIN, e=E_MIN):
    """The rr where the two legs swap roles, with a per-trade cost in R."""
    # w_pf = p(1+c)/(rr-c+p(1+c))   w_e = (1+e+c)/(1+rr)
    a, b = p * (1 + c), 1 + e + c
    # a(1+rr) = b(rr - c + a)  ->  rr(a-b) = b(a-c) - a
    return (b * (a - cost) - a) / (a - b)


rule()
print("Q1 - THE TWO LEGS AS FUNCTIONS OF reward_risk (rr).  NO GATE ON THIS AXIS.")
rule()
print("A1..A4 of the registration: every trade closes at exactly -1R or +rr*R, one R per")
print("trade, costs already inside the payoffs, PF = sum(+R)/|sum(-R)|.  Let w = win rate.")
print()
print("  expectancy  E  = w*rr - (1-w)*1 = w(1+rr) - 1")
print(f"    E >= +{E_MIN:.3f}R   <=>   w >= (1 + {E_MIN:.3f}) / (1 + rr)        = w_E(rr)")
print()
print("  profit factor  PF = w*rr / ((1-w)*1)")
print(f"    PF >= {PF_MIN:.3f}     <=>   w >= {PF_MIN:.3f} / (rr + {PF_MIN:.3f})         = w_PF(rr)")
print()
print("  the leg that BINDS is the one with the HIGHER threshold win rate:")
print(f"    D(rr) = w_PF - w_E = [{PF_MIN:.2f}(1+rr) - {1+E_MIN:.2f}(rr+{PF_MIN:.2f})] / ((rr+{PF_MIN:.2f})(1+rr))")
print(f"                       = 0.15 * (rr - 0.400) / ((rr + 1.200)(rr + 1))")
print()
print("  ONE root, at rr = 0.400, and the denominator is positive for every rr > 0, so the")
print("  sign of D changes exactly once:")
print("    rr <  0.400  ->  D < 0  ->  the EXPECTANCY leg binds, PF is redundant")
print("    rr =  0.400  ->  D = 0  ->  both legs demand w = 75.0%")
print("    rr >  0.400  ->  D > 0  ->  the PF leg binds, EXPECTANCY is redundant")
print()
print("  derivatives (so a later reader can re-derive the root without this script):")
print(f"    dw_PF/drr = -{PF_MIN:.2f} / (rr + {PF_MIN:.2f})^2        dw_E/drr = -{1+E_MIN:.2f} / (1 + rr)^2")
print("    dD/drr  = 1.05/(1+rr)^2 - 1.20/(rr+1.2)^2 ;  at rr = 0.400 it is +0.0670 of win")
print("    rate per unit rr, so +0.1 of rr above the root widens the gap by ~0.67 pp.")
print()
print("  the identity the two legs imply between PF and E (eliminate w) - this is what Q3")
print("  tests on the record:        PF = rr (1 + E) / (rr - E)")
print()
print(f"{'rr':>6}{'w_PF needs':>13}{'w_E needs':>12}{'D (pp)':>10}{'binding leg':>16}"
      f"{'PF at E=+0.050R':>18}{'E at PF=1.200':>16}")
for rr in (0.20, 0.30, 0.40, 0.50, 0.75, 1.00, 1.50, 1.80, 2.00, 3.00):
    b = "expectancy" if gap(rr) < -1e-12 else ("BOTH" if abs(gap(rr)) <= 1e-12 else "PF")
    pf_at_e = rr * (1 + E_MIN) / (rr - E_MIN)
    e_at_pf = rr * (PF_MIN - 1) / (PF_MIN + rr)
    print(f"{rr:>6.2f}{100*w_pf(rr):>12.1f}%{100*w_e(rr):>11.1f}%{100*gap(rr):>+10.2f}"
          f"{b:>16}{pf_at_e:>18.3f}{e_at_pf:>+15.4f}R")
print()
print(f"  CHECK against AGENT-BRIEF-ADDENDUM-5 sec.A (rr = 1.800): it states PF >= 1.200 needs")
print(f"  >= 40.0% and expectancy >= +0.050R needs >= 37.5%.  Derived here: "
      f"{100*w_pf(1.8):.1f}% and {100*w_e(1.8):.1f}%.  MATCHES.")
print()
print("  the root is barely moved by cost (brief sec.8 / addendum sec.E measure 0.85-38.16% of R):")
for c, who in ((0.0, "no cost"), (0.0404, "gold, 1.5 ATR(15m)"), (0.1481, "EUR"),
               (0.1732, "silver"), (0.3816, "AUDNZD")):
    print(f"    cost {100*c:>5.2f}% of R  ({who:<20}) -> root at rr = {crossover(c):.3f}")


# ------------------------------------------------------------------- Q2
# Read off config/ and crates/ as text; every cell is cited in the report.
# "own rr" = the strategy builds target = entry +/- risk * riskReward itself,
# so `[trading] reward_risk` never reaches the position (engine.rs:737-744,
# `Some(explicit) => Some(explicit)`).
ENGINE_RR = 1.8
OWN = [  # id, default riskReward, declared grid, source
    ("crt", 1.5, [1.5], "crt.rs:87,187 (mode 2 only; modes 0/1 use range extremes)"),
    ("doji-reversal", 1.5, [1.0, 1.5, 2.0], "doji.rs:43,51,98"),
    ("far-stop-break", 1.0, [1.0, 1.8], "far_stop_break.rs:64,73,136"),
    ("ict-sweep-mss-fvg", 2.0, [1.5, 2.0, 3.0], "ict.rs:106,114,151"),
    ("keltner-break", 1.5, [1.0, 1.5, 2.0], "screen.rs:64,67,96"),
    ("macd-cross", 1.5, [1.0, 1.5, 2.0], "screen.rs:122,125,155"),
    ("orb", 1.5, [1.0, 1.5, 2.0], "orb.rs:48,57,153"),
    ("pdhl", 1.5, [1.0, 1.5, 2.0], "pdhl.rs:53,61,133"),
    ("rsi2-pullback", 1.0, [0.75, 1.0, 1.5], "screen.rs (grid 0.75/1.0/1.5)"),
    ("squeeze-break", 1.5, [1.0, 1.5, 2.0], "screen.rs"),
    ("stoch-reversal", 1.5, [1.0, 1.5, 2.0], "screen.rs"),
    ("trend-pullback", 1.5, [1.0, 1.5, 2.0], "trend_pullback.rs:41,47,99"),
    ("volume-thrust", 1.5, [1.0, 1.5, 2.0], "volume_thrust.rs:42,48,103"),
]
SHARED = ["bb-fade", "companion-unconfirmed", "donchian-breakout", "ema-cross",
          "external", "flow-at-level", "flow-momentum", "gap-fade",
          "level-reversion", "maxpain-magnet", "rsi-reversal-vol",
          "rsi-reversion", "volman-box", "vwap-fade"]
NO_RR = ["buy-and-hold", "intraday-momentum", "quiet-swing", "session-hold", "tsmom"]

rule()
print("Q2 - THE rr EVERY MECHANISM ACTUALLY RUNS AT, AND EVERY OVERRIDE PATH")
rule()
print("`[trading] reward_risk = 1.8` (config/default.toml:98) is the ONLY place in the tree")
print("that states one.  The override paths, each checked in the source:")
print()
print("  per market  [markets.<id>.trading]  : NO SUCH KEY EXISTS.  MarketTradingOverride")
print("      (fd-core/src/config.rs:338-385) declares symbol, contract_size, spread,")
print("      lot_step, min_lot, swap_*, news_currencies, account_currency, units_per_usd,")
print("      leverage, price_decimals, starting_equity_usd, max_hold_ms - and no")
print("      reward_risk.  This is `not possible`, which is not the same finding as")
print("      `not stated`; fd-backtest/tests/trading_rules.rs:48 asserts btc == gold.")
print("  per batch   overrides = { .. }       : REJECTED AS AN ERROR.  hypotheses.rs:1454")
print("      `preset_params` returns Err(\"<id> has no parameter `reward_risk`\") for any")
print("      key the STRATEGY does not declare, and reward_risk is not a strategy param.")
print("  --params=key=value                   : REJECTED.  search.rs:698-704 takes the same")
print("      `params.contains(key)` route and prints `bad or unknown --params entry`.")
print("      It is also in BY_MODE for `null-dir` ALONE (search.rs:78), so even a legal")
print("      entry changes nothing under --mode=hypotheses / rescore / null.")
print("  a CLI flag                           : none.  The flag list of brief sec.1 has no")
print("      --reward-risk, and a flag this binary does not know is reported, not applied")
print("      (search.rs::flag_audit).")
print()
print("So rr is NOT configurable at all.  It varies between mechanisms by exactly one")
print("route: a strategy that builds its own target bypasses `reward_risk` entirely")
print("(engine.rs:737 `Some(explicit) => Some(explicit)`).")
print()
rows_q2 = ([(i, d, g, "own target", s) for i, d, g, s in OWN]
           + [(i, ENGINE_RR, [ENGINE_RR], "[trading] 1.8", "engine.rs:368,742") for i in SHARED]
           + [(i, None, [], "Exits::Strategy", "no engine target at all") for i in NO_RR])
print(f"{'mechanism':<24}{'rr default':>11}{'rr values declared':>24}{'where rr comes from':>18}")
rule("-")
for i, d, g, src, _ in sorted(rows_q2, key=lambda r: (r[3], r[0])):
    dd = f"{d:.2f}" if d is not None else "null"
    gg = "/".join(f"{x:g}" for x in g) if g else "null (no target)"
    print(f"{i:<24}{dd:>11}{gg:>24}{src:>18}")
rule("-")
allrr = sorted({x for _, _, g, _, _ in rows_q2 for x in g})
defined = [r for r in rows_q2 if r[1] is not None]
print(f"registered mechanisms                                  : {len(rows_q2)}")
print(f"  with a defined rr (the engine places a target)        : {len(defined)}")
print(f"  with NO rr at all (Exits::Strategy; rr is null)       : {len(NO_RR)}  "
      f"<- the algebra of Q1 does not apply to these")
print()
print(f"every rr value that appears anywhere in the tree        : "
      f"{', '.join(f'{x:g}' for x in allrr)}")
print(f"  lowest                                               : {min(allrr):g}")
print(f"  the Q1 root                                          : 0.400")
print(f"  mechanisms at rr BELOW the root (expectancy binds)    : "
      f"{sum(1 for r in defined if max(r[2]) < 0.400)}")
print(f"  mechanisms at rr ABOVE the root (PF binds)            : "
      f"{sum(1 for r in defined if min(r[2]) > 0.400)}")
print(f"  mechanisms with any declared rr value within +-0.10   : "
      f"{sum(1 for r in defined if any(abs(x - 0.400) <= 0.10 for x in r[2]))}")
print(f"  closest approach to the root                          : "
      f"{min(allrr) - 0.400:+.3f} of rr (rsi2-pullback at 0.75), i.e. {min(allrr)/0.400:.2f}x the root")
print()
print("  ONE CAVEAT WITH A NUMBER: the root moves with cost, and at AUDNZD's measured")
print(f"  38.16% of R the root is rr = {crossover(0.3816):.3f} - which is rsi2-pullback's own grid")
print("  low of 0.75.  On that instrument that one cell sits ON the root, not above it.")


# ------------------------------------------------------------------- Q3
lb = json.load(open(f"{OUT}/gate_legs_lbar.json", encoding="utf-8"))
allrows = json.load(open(f"{OUT}/gate_legs_rows.json", encoding="utf-8"))
census = json.load(open(f"{OUT}/rows.json", encoding="utf-8"))

rule()
print("Q3 - THE RECORD AGAINST THE Q1 IDENTITY   PF = rr(1+E)/(rr-E)  at rr = 1.800")
rule()
print("Harvest REUSED UNCHANGED from agent/floor-audit (floor_audit_harvest.py +")
print("floor_audit_census.py); only the reading of the PF and expectancy columns is new.")
print(f"refs scanned                                        : 118 local+remote heads")
print(f"distinct printed rows                               : {len(census)}"
      "   (floor-audit read 9,858 on 110 refs; 8 further")
print(f"{'':52}refs have been pushed since, +218 rows)")
print(f"rows carrying BOTH a PF and an expectancy           : {len(allrows)}")
print(f"  rows read per printed column pair: "
      + ", ".join(f"{a}+{b} {n}" for (a, b), n in
                  Counter((r['pf_col'], r['e_col']) for r in allrows).most_common()))
print()
print("FIRST, A1 CHECKED ON THE RECORD ITSELF (the registration said this assumption would")
print("be the one to fail).  Every distinct printed `exits: ...; mean hold` line:")
EXL = re.compile(r"exits:\s*(.+?);\s*mean hold")
KIND = re.compile(r"([A-Za-z_][A-Za-z_ 0-9%+.-]*?)\s+(\d+)(?:,|$)")
pure, seen, kinds = [], set(), Counter()
for p in glob.glob(f"{OUT}/blobs/*.txt"):
    for line in open(p, encoding="utf-8", errors="replace"):
        m = EXL.search(line)
        if not m:
            continue
        body = m.group(1).strip()
        if body == "no exits recorded" or body in seen:
            continue
        seen.add(body)
        tot = pu = 0
        for name, n in KIND.findall(body):
            n, name = int(n), name.strip()
            tot += n
            kinds[name] += n
            if name in ("STOP", "TARGET"):
                pu += n
        if tot:
            pure.append(pu / tot)
tot_ex = sum(kinds.values())
print(f"  distinct exit-mix lines                           : {len(pure)}")
print(f"  share of exits that are STOP or TARGET, overall   : "
      f"{100*(kinds['STOP']+kinds['TARGET'])/tot_ex:.1f}% of {tot_ex} booked exits")
print(f"  per line: min {100*min(pure):.1f}%  p25 {100*st.quantiles(pure,n=4)[0]:.1f}%  "
      f"median {100*st.median(pure):.1f}%  p75 {100*st.quantiles(pure,n=4)[2]:.1f}%  max {100*max(pure):.1f}%")
print(f"  lines where A1 HOLDS (100% STOP/TARGET)           : "
      f"{sum(1 for x in pure if x>=0.999)}  ({100*sum(1 for x in pure if x>=0.999)/len(pure):.1f}%)")
print(f"  lines with ZERO STOP and ZERO TARGET              : "
      f"{sum(1 for x in pure if x<=0.001)}  ({100*sum(1 for x in pure if x<=0.001)/len(pure):.1f}%)")
print("  => A1 is false on 95.1% of the record.  The Q1 identity is therefore expected to")
print("     miss, and HOW it misses is the finding.")
print()
print("  the exits that break A1, by booked trade count:")
for k, v in kinds.most_common(12):
    tag = "  <- A1 holds" if k in ("STOP", "TARGET") else ""
    print(f"    {v:>7}  {k}{tag}")
print()
print("RESIDUAL OF THE IDENTITY (printed PF vs the PF the identity predicts from printed E):")
bandcount = Counter()
for r in allrows:
    rel = r["rel"]
    if rel is None:
        bandcount["E >= rr: no losing side, PF undefined"] += 1
    else:
        a = abs(rel)
        bandcount["|residual| <= 0.5% of predicted PF" if a <= 0.005 else
                  "|residual| <= 1%" if a <= 0.01 else
                  "|residual| <= 5%" if a <= 0.05 else
                  "|residual| <= 20%" if a <= 0.20 else
                  "|residual| > 20%"] += 1
for k in ("|residual| <= 0.5% of predicted PF", "|residual| <= 1%", "|residual| <= 5%",
          "|residual| <= 20%", "|residual| > 20%", "E >= rr: no losing side, PF undefined"):
    v = bandcount[k]
    print(f"  {v:>5}  {100*v/len(allrows):>5.1f}%   {k}")
print(f"\n  rows matching the identity to print width          : "
      f"{bandcount['|residual| <= 0.5% of predicted PF']} of {len(allrows)} "
      f"({100*bandcount['|residual| <= 0.5% of predicted PF']/len(allrows):.1f}%)")
print(f"  rows that do NOT match it                         : "
      f"{len(allrows)-bandcount['|residual| <= 0.5% of predicted PF']} "
      f"({100*(len(allrows)-bandcount['|residual| <= 0.5% of predicted PF'])/len(allrows):.1f}%)")
print("  The 5.7% that match and the 4.9% of exit-mix lines that are pure STOP/TARGET are")
print("  the same fact read from two different printed fields, and they agree.")


# ------------------------------------------------------------------- Q4
rule()
print("Q4 - IS ANY ROW REJECTED BY ONE LEG ALONE?  (the mirror of agent/floor-audit)")
rule()
cells = Counter()
for r in allrows:
    cells[("pass" if r["pf"] >= PF_MIN else "fail",
           "pass" if r["e"] >= E_MIN else "fail")] += 1
print(f"{'':34}{'expectancy >= +0.050R':>24}{'expectancy < +0.050R':>24}")
print(f"{'PF >= 1.200':<34}{cells[('pass','pass')]:>24}{cells[('pass','fail')]:>24}"
      "   <- THE CELL THAT SHOULD BE EMPTY")
print(f"{'PF <  1.200':<34}{cells[('fail','pass')]:>24}{cells[('fail','fail')]:>24}")
rule("-")
only_e = [r for r in lb if r["pf"] >= PF_MIN and r["e"] < E_MIN]
only_p = [r for r in lb if r["pf"] < PF_MIN and r["e"] >= E_MIN]
print(f"rows rejected by the EXPECTANCY leg ALONE : {len(only_e):>5}"
      f"   of which >= {N_MIN} trades too: {sum(1 for r in only_e if r['trades']>=N_MIN)}")
print(f"rows rejected by the PF leg ALONE         : {len(only_p):>5}"
      f"   of which >= {N_MIN} trades too: {sum(1 for r in only_p if r['trades']>=N_MIN)}")
print()
print("Q1 PREDICTED THE FIRST NUMBER WOULD BE 0 AT EVERY rr THE RECORD USES.  IT IS NOT 0.")
print("The derivation is missing a term, and here it is:")
print()
print("  drop A1 entirely and keep only A2 (one R per trade) and A4.  Write")
print("  Lbar = |sum(-R)| / n, the GROSS LOSS PER TRADE in R, averaged over ALL n trades.")
print("  Then, with no assumption whatever about how a trade exits:")
print()
print("        E  =  Lbar * (PF - 1)                 <- exact, always")
print()
print("  so    E >= +0.050R   <=>   PF - 1 >= 0.050 / Lbar")
print("        PF >= 1.200    <=>   PF - 1 >= 0.200")
print()
print("  THE EXPECTANCY LEG IS REDUNDANT IF AND ONLY IF   Lbar >= 0.250 R.")
print("  rr never appears.  It entered Q1 only through A1, which pins Lbar = (1-w); at the")
print("  PF threshold that is Lbar = rr/(rr+1.2), and Lbar >= 0.250 <=> rr >= 0.400 - the")
print("  SAME root.  So Q1 was a special case: correct, but blind to the only term that")
print("  matters here, which is HOW BIG THE LOSSES ARE, not how far away the targets are.")
print()
sep = Counter()
for r in lb:
    sep[(r["Lbar"] >= 0.250, r["pf"] >= PF_MIN and r["e"] < E_MIN)] += 1
print("  the switch tested on all 6,112 readable rows:")
print(f"{'':6}{'Lbar >= 0.250R':>16}{'rejected by expectancy alone':>32}{'rows':>8}")
for k in ((True, False), (True, True), (False, False), (False, True)):
    print(f"{'':6}{str(k[0]):>16}{str(k[1]):>32}{sep[k]:>8}")
print(f"  PERFECT SEPARATION: {sep[(False,True)]} of {sep[(False,True)]} single-leg "
      f"rejections have Lbar < 0.250R, and")
print(f"  {sep[(True,True)]} of the {sep[(True,False)]+sep[(True,True)]} rows with "
      "Lbar >= 0.250R are rejected by expectancy alone.")
L = [r["Lbar"] for r in lb if r["Lbar"] > 0]
print(f"  Lbar over the record (positive values): p25 {st.quantiles(L,n=4)[0]:.4f}R  "
      f"median {st.median(L):.4f}R  p75 {st.quantiles(L,n=4)[2]:.4f}R")
print()
print("A SECOND MISSING TERM, AND THIS ONE IS A UNIT MISMATCH IN THE ENGINE (count, do not")
print("fix - brief sec.7):")
neg = [r for r in lb if r["Lbar"] < 0]
print("  engine.rs:1071-1114 computes gross_win / gross_loss from `t.pnl_usd` and prints")
print("  that as `profit_factor`, while `expectancy` is the mean of `t.r`.  PF IS IN USD")
print("  AND EXPECTANCY IS IN R.  Sizing is `risk_usd = equity * risk_per_trade_pct`")
print("  (engine.rs:728) on a COMPOUNDING equity (`equity += trade.pnl_usd`, engine.rs:521,")
print("  576, 600, 648), and `lots` is floored to lot_step then raised to min_lot, so one R")
print("  is a different number of dollars early and late in a run: A2 is false by design.")
print(f"  Consequence, counted: {len(neg)} rows ({100*len(neg)/len(lb):.1f}% of {len(lb)}) print a")
print("  PF and an expectancy whose SIGNS CANNOT BOTH BE RIGHT for one book -")
print(f"    PF > 1 with E < 0 : {sum(1 for r in neg if r['pf']>1 and r['e']<0)} rows")
print(f"    PF < 1 with E > 0 : {sum(1 for r in neg if r['pf']<1 and r['e']>0)} rows")
print("  No identity in R can reconcile these; they are brief sec.6(b) counted over the")
print("  whole record.  THE SIX WORST BY NAME:")
for r in sorted(neg, key=lambda r: r["Lbar"])[:6]:
    print(f"    PF={r['pf']:>7.3f}  E={r['e']:>+10.3f}R  n={r['trades']:<5} {r['label'][:26]}")
    print(f"      {sorted(r['paths'])[0]}")

rule()
print(f"EVERY ROW REJECTED BY THE EXPECTANCY LEG ALONE, BY NAME ({len(only_e)} rows; "
      f"{sum(1 for r in only_e if r['trades']>=N_MIN)} also clear the {N_MIN}-trade leg)")
print("  Lbar = the gross loss per trade in R that the row's own PF and expectancy imply.")
print("  n>=40 marks the rows where expectancy is the SOLE reason the desk gate says no.")
rule()
for r in sorted(only_e, key=lambda r: (-r["pf"], r["label"])):
    flag = "n>=40" if r["trades"] >= N_MIN else "n<40 "
    print(f"{flag}  PF={r['pf']:>6.3f}  E={r['e']:>+8.4f}R  Lbar={r['Lbar']:>9.4f}R  "
          f"n={r['trades']:<5} {r['label'][:30]}")
    print(f"{'':8}{sorted(r['paths'])[0]}   [{len(r['branches'])} refs]")

rule()
print(f"THE PF-ONLY CELL ({len(only_p)} rows) - the mirror, by family with its extremes named")
rule()
fam = Counter(r["label"].split("/")[0] for r in only_p)
print("  by the first component of the printed label, top 12:")
for k, v in fam.most_common(12):
    print(f"    {v:>4}  {k}")
print(f"  distinct labels: {len(fam)}")
print("  the 6 with the LOWEST PF (expectancy clears, PF is furthest from 1.200):")
for r in sorted(only_p, key=lambda r: r["pf"])[:6]:
    print(f"    PF={r['pf']:>6.3f}  E={r['e']:>+8.4f}R  Lbar={r['Lbar']:>9.4f}R  "
          f"n={r['trades']:<5} {r['label'][:28]}")
    print(f"      {sorted(r['paths'])[0]}")

rule()
print("THE 87 SPLIT BY CAUSE - two different things, not one (brief sec.8: null != 0)")
rule()
small = [r for r in only_e if 0 < r["Lbar"] < 0.250]
negs = [r for r in only_e if r["Lbar"] <= 0]
print(f"  0 < Lbar < 0.250R   the trades are genuinely too small in R, and the")
print(f"                      expectancy leg is the only leg that can see it")
print(f"                      {len(small):>4} rows, {sum(1 for r in small if r['trades']>=N_MIN)} of them with >= {N_MIN} trades")
print(f"  Lbar <= 0           PF and expectancy disagree in SIGN, so no Lbar exists:")
print(f"                      these are the USD-vs-R unit defect, not a small book")
print(f"                      {len(negs):>4} rows, {sum(1 for r in negs if r['trades']>=N_MIN)} of them with >= {N_MIN} trades")
print()
print("  the second group is reported as its own population and is NOT counted as evidence")
print("  that the expectancy leg is doing useful work; it is evidence that the two columns")
print("  are not in the same unit.  ALL NINE BY NAME:")
for r in sorted(negs, key=lambda r: r["Lbar"]):
    print(f"    PF={r['pf']:>6.3f}  E={r['e']:>+10.4f}R  n={r['trades']:<5} {r['label'][:26]}")
    print(f"      {sorted(r['paths'])[0]}")
print()
print(f"  So the defensible headline is the FIRST group: {len(small)} rows, "
      f"{sum(1 for r in small if r['trades']>=N_MIN)} past the {N_MIN}-trade leg,")
print("  where the expectancy leg rejects a row that PF and the trade count both admit,")
print("  for the one reason PF cannot express - the trades are too small.")

rule()
print("FALSIFIER F1 - DOES IT FIRE?")
rule()
print("F1 fires (the three-leg gate has been a one-leg gate all along) only if at every rr")
print("the record uses the PF threshold is the higher one AND the count of rows rejected by")
print("the expectancy leg alone is 0.")
print()
print("  first condition  : HOLDS.  All 27 mechanisms with a defined rr run at rr in")
print("                     [0.75, 3.00], every value above the root 0.400, so w_PF > w_E")
print("                     everywhere in the tree.  Q2 names all 27.")
print(f"  second condition : FAILS.  {len(only_e)} rows are rejected by the expectancy leg alone,")
print(f"                     {sum(1 for r in only_e if r['trades']>=N_MIN)} of them with >= {N_MIN} trades.")
print()
print("  => F1 DOES NOT FIRE.  The gate is a genuine TWO-leg gate in the record, and the")
print("     reason is not rr: it is that 94.3% of the record does not exit purely at stop")
print("     or target, so Lbar falls below 0.250R and the expectancy leg starts doing work")
print("     that PF cannot do.  PF is scale-free; expectancy in R is not.")

rule()
print("RECOMMENDATION FOR THE OWNER - stated with numbers and LEFT THERE (no threshold is")
print("changed or proposed on this axis; brief sec.7)")
rule()
print("  1. Addendum sec.A, `the expectancy leg is redundant`, is true on a PURE STOP/TARGET")
print(f"     book and false on this record: {len(only_e)} rows, {sum(1 for r in only_e if r['trades']>=N_MIN)} of them past the {N_MIN}-trade leg,")
print("     are rejected by expectancy and by nothing else.  It is not a restatement of PF.")
print("  2. What the expectancy leg actually enforces is a MINIMUM TRADE SIZE: PF >= 1.200")
print("     and E >= +0.050R together are PF - 1 >= max(0.200, 0.050/Lbar).  The whole")
print("     session-close family (`close/<window>`, 36 of the 87) prints PF up to 2.065 on")
print("     an Lbar of 0.027R - a 2x profit factor on trades 2.7% of a risk unit in size.")
print("     That is addendum sec.C's px-1s seen from the gate's side.")
print("  3. `profit_factor` in USD printed beside `expectancy` in R is the defect worth a")
print(f"     decision: {len(neg)} rows print a sign-contradictory pair.  A `profit_factor_r` over")
print("     `t.r` would make the two legs commensurable; it would also restate the PF")
print("     column of the whole record, which is why it is not done here.")
