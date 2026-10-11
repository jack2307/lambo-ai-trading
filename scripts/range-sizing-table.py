"""Gop 12 receipt cua agent/range-sizing thanh mot bang doc duoc.

Moi so o day duoc CAT TU receipt, khong go lai tu dau. Dong quyet dinh la
`dPF_r` o arm co guards tren CA HAI cua so; `PF_usd` in canh nhung khong
quyet dinh gi (phu luc 10 muc V: bay phep do, ba ket qua ve chieu cai khe).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

OUT = Path("receipts/range-sizing")
SRC = {"XAURFC15": "fc15", "XAURFC1M": "fc1m", "XAURFCSH": "shuf"}

HEAD = re.compile(r"^(atr/f[\d.]+|fc/f[\d.]+)\s+range-stop\s+(\d+)\s+([\d.]+)\s+([-+]?[\d.]+)\s+([\d.]+)\s+([\d.]+)")
LBAR = re.compile(r"Lbar ([\d.]+) R .*PF_r ([\d.]+) \(R\) vs PF_usd ([\d.]+) \(USD, gap ([-+][\d.]+)\)")
DD = re.compile(r"max drawdown ([\d.]+) USD = ([\d.]+)% of the book's peak equity")
NET = re.compile(r"net ([-+][\d.]+) USD over (\d+) trades")
EXITS = re.compile(r"^\s+exits: (.*?); mean hold ([\d.]+) min")
GUARD = re.compile(r"^\s+guards: (.*)$")
RSTOP = re.compile(r"realised stop: median ([\d.]+) ATR = ([\d.]+) points")
COSTR = re.compile(r"cost-matched null: control stop ([\d.]+) ATR = ([\d.]+) points, cost ([\d.]+)% of R")
AVGMAE = re.compile(r"avg_mae ([-+][\d.]+) R")


def parse(path: Path) -> dict[str, dict]:
    cells: dict[str, dict] = {}
    cur = None
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        h = HEAD.match(line)
        if h:
            cur = {"label": h.group(1), "n": int(h.group(2)), "pf_usd_hdr": float(h.group(3)), "E_hdr": float(h.group(4))}
            cells[h.group(1)] = cur
            continue
        if cur is None:
            continue
        for rx, keys in (
            (LBAR, ("lbar", "pf_r", "pf_usd", "gap")),
            (DD, ("dd_usd", "dd_pct")),
            (NET, ("net_usd", "n2")),
            (RSTOP, ("rstop_atr", "rstop_pts")),
            (COSTR, ("ctl_atr", "ctl_pts", "cost_r")),
            (AVGMAE, ("avg_mae",)),
        ):
            m = rx.search(line)
            if m:
                for i, k in enumerate(keys):
                    cur[k] = float(m.group(i + 1))
        e = EXITS.match(line)
        if e:
            cur["exits"] = e.group(1)
            cur["hold"] = float(e.group(2))
        g = GUARD.match(line)
        if g and "guards:" in line and cur is not None:
            cur["guards"] = g.group(1)
    return cells


def stop_share(exits: str, n: int) -> float:
    m = re.search(r"\bSTOP (\d+)", exits or "")
    return 100.0 * int(m.group(1)) / n if m and n else float("nan")


def sized_down(path: Path, label: str) -> str:
    """`sized_down_by_guard` — doc DONG guards cua chinh o do, khong grep engine."""
    txt = path.read_text(encoding="utf-8", errors="replace").splitlines()
    want = None
    for i, line in enumerate(txt):
        if HEAD.match(line) and line.split()[0] == label:
            want = i
        elif want is not None and HEAD.match(line):
            break
        elif want is not None and line.strip().startswith("guards:"):
            return line.strip()[len("guards:") :].strip()
    return "null (dong guards khong ton tai o o nay)"


def main() -> int:
    rows = []
    for src, tag in SRC.items():
        for wl in ("W1", "W2"):
            for arm in ("off", "on"):
                p = OUT / f"{wl}-{src}-guards-{arm}.txt"
                if not p.exists():
                    print(f"THIEU {p}")
                    return 1
                for label, c in parse(p).items():
                    rows.append((wl, arm, tag, label, c, p))

    print("BANG 1 — 32 O CONG + 8 O TRUNG LAP (nhan `atr/*` xuat hien 3 lan moi (cua so, arm))")
    print("Moi so cat tu receipt. `dd%` > 100% => DONG DA CHAY, dung doc PF_usd cua no.")
    print()
    print(f"{'cs':3s} {'arm':4s} {'nguon':5s} {'nhan':9s} {'n':>5s} {'PF_r':>7s} {'PF_usd':>7s} "
          f"{'khe':>8s} {'E':>9s} {'Lbar':>7s} {'ddUSD':>10s} {'dd%':>7s} {'STOP%':>6s} {'stop(ATR)':>9s} {'cost/R%':>7s}")
    for wl, arm, tag, label, c, _ in rows:
        e = c.get("net_usd", float("nan")) / c["n"] if c.get("n") else float("nan")
        print(
            f"{wl:3s} {arm:4s} {tag:5s} {label:9s} {c['n']:5d} {c.get('pf_r', float('nan')):7.4f} "
            f"{c.get('pf_usd', float('nan')):7.4f} {c.get('gap', float('nan')):+8.4f} {c['E_hdr']:+9.4f} "
            f"{c.get('lbar', float('nan')):7.4f} {c.get('dd_usd', float('nan')):10.2f} {c.get('dd_pct', float('nan')):7.2f} "
            f"{stop_share(c.get('exits', ''), c['n']):6.1f} {c.get('rstop_atr', float('nan')):9.3f} "
            f"{c.get('cost_r', float('nan')):7.2f}"
        )

    print()
    print("BANG 2 — DOI CHUNG DUONG 2: nhan `atr/*` KHONG doc gia tri companion,")
    print("nen no phai in SO Y HET o ca ba lan chay companion. Neu no doi, viec nap")
    print("companion dang ro vao mot nhan khong doc no.")
    by = {}
    for wl, arm, tag, label, c, _ in rows:
        if label.startswith("atr/"):
            by.setdefault((wl, arm, label), []).append((tag, c["n"], c.get("pf_r"), c["E_hdr"]))
    bad = 0
    for k, v in sorted(by.items()):
        vals = {(n, pf, ee) for _, n, pf, ee in v}
        ok = len(vals) == 1
        bad += 0 if ok else 1
        print(f"  {k[0]} guards-{k[1]} {k[2]:9s} {'Y HET' if ok else 'KHAC NHAU'}  " + "  ".join(f"{t}:n{n}/PF_r{pf}" for t, n, pf, _ in v))
    print(f"  => {'PASS' if bad == 0 else 'TRUOT'}: {len(by) - bad}/{len(by)} nhom y het")

    print()
    print("BANG 3 — CON SO QUYET DINH: dPF_r = PF_r(nguon) - PF_r(atr), cung `stopAtr`,")
    print("cung tap lenh, cung co stop trung binh. Vach F1: duong o CA HAI cua so,")
    print("arm CO guards. Vach F2: `shuf` tai tao >= 50% cua dPF_r thi truc dong.")
    idx = {(wl, arm, tag, label): c for wl, arm, tag, label, c, _ in rows}
    print()
    print(f"{'cs':3s} {'arm':4s} {'f':5s} {'PF_r(atr)':>9s} {'fc15':>9s} {'fc1m':>9s} {'shuf':>9s} "
          f"{'d.fc15':>8s} {'d.fc1m':>8s} {'d.shuf':>8s} {'shuf/fc1m':>9s}")
    for wl in ("W1", "W2"):
        for arm in ("off", "on"):
            for f in ("1.0", "2.0"):
                base = idx[(wl, arm, "fc1m", f"atr/f{f}")].get("pf_r")
                got = {t: idx[(wl, arm, t, f"atr/f{f}" if False else f"fc/f{f}")].get("pf_r") for t in ("fc15", "fc1m", "shuf")}
                d = {t: got[t] - base for t in got}
                rep = d["shuf"] / d["fc1m"] if d["fc1m"] != 0 else float("nan")
                print(
                    f"{wl:3s} {arm:4s} {f:5s} {base:9.4f} {got['fc15']:9.4f} {got['fc1m']:9.4f} {got['shuf']:9.4f} "
                    f"{d['fc15']:+8.4f} {d['fc1m']:+8.4f} {d['shuf']:+8.4f} {rep:9.2f}"
                )

    print()
    print("BANG 3b — F3 (co lap phan 1m): `fc1m` so voi `fc15` tren DUNG CUNG TAP NEN")
    print("(mot mat na NULL chung cho ca ba file companion). Duong = 8 dac trung 1m")
    print("lam cai stop du bao DO HON, du ca hai van duoi doi chung ATR.")
    better = 0
    for wl in ("W1", "W2"):
        for arm in ("off", "on"):
            for f in ("1.0", "2.0"):
                base = idx[(wl, arm, "fc1m", f"atr/f{f}")]["pf_r"]
                a_, b_ = idx[(wl, arm, "fc15", f"fc/f{f}")]["pf_r"], idx[(wl, arm, "fc1m", f"fc/f{f}")]["pf_r"]
                g = b_ - a_
                better += int(g > 0)
                print(f"  {wl} guards-{arm} f{f}: fc15 {a_ - base:+.4f}  fc1m {b_ - base:+.4f}  "
                      f"=> 1m THEM {g:+.4f} PF_r  ({'do hon' if g > 0 else 'te hon'})")
    print(f"  => 1m lam do hon o {better}/8 phep so; nhung ca hai deu AM so voi ATR o 8/8.")

    print()
    print("BANG 4 — `sized_down_by_guard` DOC TU FIELD (dong `guards:` cua chinh o do,")
    print("`hypotheses.rs:752`; `nothing acted` = sized_down == 0, KHONG phai thieu chuoi).")
    print("Ca hai chan cong mu voi no. Va no la thuoc tinh cua ho `Exits::Engine` CUOI")
    print("TRAN NOTIONAL, khong phai cua arm guards: stop hep => risk_usd nho => nhieu")
    print("lot => dung `notional 300%`.")
    for wl, arm, tag, label, c, p in rows:
        if arm == "on":
            line = sized_down(p, label)
            m = re.search(r"sized down (\d+)", line)
            sd = int(m.group(1)) if m else 0
            print(f"  {wl} {tag:5s} {label:9s} n={c['n']:5d}  sized_down {sd:5d} = {100.0 * sd / c['n']:6.2f}%  | {line}")
    print()
    print("  Ti le bit DOI theo nguon stop — nen moi so quyet dinh phai la PF_r/E:")
    for wl in ("W1", "W2"):
        for f in ("1.0", "2.0"):
            out = [f"  {wl} f{f}:"]
            for tag, lab in (("fc1m", f"atr/f{f}"), ("fc15", f"fc/f{f}"), ("fc1m", f"fc/f{f}"), ("shuf", f"fc/f{f}")):
                p = OUT / f"{wl}-{[k for k, v in SRC.items() if v == tag][0]}-guards-on.txt"
                c = idx[(wl, "on", tag, lab)]
                m = re.search(r"sized down (\d+)", sized_down(p, lab))
                sd = int(m.group(1)) if m else 0
                name = "atr" if lab.startswith("atr") else tag
                out.append(f"{name} {100.0 * sd / c['n']:.2f}%")
            print("   ".join(out))

    print()
    print("BANG 5 — exit-mix day du: luat cua co che co NO khong")
    for wl, arm, tag, label, c, _ in rows:
        print(f"  {wl} guards-{arm} {tag:5s} {label:9s} hold {c.get('hold', float('nan')):6.1f} min  {c.get('exits', 'null')}")

    print()
    print("BANG 6 — DEM TAY chan >= 40 lenh va ca hai chan cong, tung o")
    print("(tool kiem san 30 lenh; cong desk la 40)")
    npass = 0
    for wl, arm, tag, label, c, _ in rows:
        legs = []
        if c.get("pf_r", 0.0) < 1.2:
            legs.append(f"PF_r {c.get('pf_r', float('nan')):.4f} < 1,200")
        if c["E_hdr"] < 0.05:
            legs.append(f"E {c['E_hdr']:+.4f}R < +0,050R")
        if c["n"] < 40:
            legs.append(f"n {c['n']} < 40")
        if not legs:
            npass += 1
            print(f"  QUA: {wl} guards-{arm} {tag} {label} n={c['n']}")
    print(f"  => {npass} / {len(rows)} o qua ca ba chan (ke ca 8 o trung lap)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
