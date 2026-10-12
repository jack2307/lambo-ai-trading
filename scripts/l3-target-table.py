"""Doc 112 dong receipt cua `agent/l3-target` thanh mot bang doc duoc.

Luat doc (dang ky muc 9, va ADDENDUM-11 muc V-VI):

* IN CA `PF_r` LAN `PF_usd` o moi dong, khai don vi. Khe giua hai cai co CHIN
  phep do khong ai khop ai ⇒ do tren bo du lieu cua minh, khong trich ti le
  cua ai.
* `_pct > 100%` ⇒ dong DA CHAY, `PF_usd` va drawdown cua no KHONG doc duoc.
* `sized_down` doc tu FIELD; `guards: nothing acted` NGHIA LA 0, khong phai
  thieu chuoi.
* Regex neo vao dong `exits:` va KHONG doc prose: ADDENDUM-11 muc VI ghi mot
  parser doc "mean hold 247,7 min" trong mot chuoi `why =` nhu mot phep do.
* `Lbar` + dang thuc `E = Lbar x (PF_r - 1)` de noi CHAN NAO rang buoc.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

OUT = Path("receipts/l3-target")
LABEL = re.compile(
    r"^(?P<label>(?:L2|L3|atr|fc)/\S+)\s+(?P<base>range-stop(?:-l3)?)\s+(?P<n>\d+)\s+"
    r"(?P<pf>[\d.]+)\s+(?P<exp>[-+][\d.]+)\s+(?P<p50>[\d.]+)\s+(?P<p95>[\d.]+)\s+"
    r"(?P<swap>-?\d+)\s+(?P<pct>-?\d+)%"
)
PFR = re.compile(r"Lbar (?P<lbar>[\d.]+) R .*?PF_r (?P<pfr>[\d.]+) \(R\) vs PF_usd (?P<pfusd>[\d.]+)")
RESID = re.compile(r"residual (?P<res>[-+][\d.]+) R")
DD = re.compile(r"max drawdown (?P<usd>[\d.]+) USD = (?P<pct>[\d.]+)% of the book's peak")
SIZED = re.compile(r"^\s+guards: (?P<body>.*)$")
EXITS = re.compile(r"^\s+exits: (?P<body>[^;]*); mean hold (?P<hold>[\d.]+) min")
STOP = re.compile(r"realised stop: median (?P<atr>[\d.]+) ATR")
COST = re.compile(r"cost-matched null: control stop [\d.]+ ATR = [\d.]+ points, cost (?P<c>[\d.]+)% of R")


def rows(path: Path) -> list[dict]:
    out: list[dict] = []
    cur: dict | None = None
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        m = LABEL.match(line)
        if m:
            cur = dict(m.groupdict())
            cur["file"] = path.name
            cur["sized_down"] = None
            out.append(cur)
            continue
        if cur is None:
            continue
        for rx, keys in ((PFR, ("lbar", "pfr", "pfusd")), (RESID, ("res",)), (DD, ("usd", "pct")),
                         (STOP, ("atr",)), (COST, ("c",))):
            g = rx.search(line)
            if g:
                for k in keys:
                    cur[{"usd": "ddusd", "pct": "ddpct", "atr": "stopatr", "c": "costr"}.get(k, k)] = g.group(k)
        g = SIZED.match(line)
        if g:
            body = g.group("body")
            if "nothing acted" in body:
                cur["sized_down"] = 0
            else:
                s = re.search(r"sized down (\d+)", body)
                cur["sized_down"] = int(s.group(1)) if s else 0
        g = EXITS.match(line)
        if g:
            cur["hold"] = g.group("hold")
            counts = dict(
                (k, int(v)) for k, v in re.findall(r"([A-Z_]+) (\d+)", g.group("body"))
            )
            cur["exits"] = counts
    return out


def main() -> int:
    files = sorted(OUT.glob("W*-*-guards-*.txt"))
    if not files:
        print("khong co receipt nao")
        return 1
    all_rows: list[dict] = []
    for f in files:
        win, src, _, arm = f.stem.split("-")
        for r in rows(f):
            r.update(win=win, src=src, arm=arm)
            all_rows.append(r)

    print(f"== {len(all_rows)} dong tu {len(files)} receipt ==")
    print()
    hdr = (f"{'cs':3s} {'arm':4s} {'companion':9s} {'nhan':12s} {'n':>6s} {'PF_r':>7s} {'PF_usd':>7s} "
           f"{'E':>7s} {'Lbar':>6s} {'du':>8s} {'ddUSD':>8s} {'dd%':>7s} {'sized':>6s} "
           f"{'STOP%':>6s} {'TGT%':>6s} {'TMO%':>6s} {'hold':>6s} {'stopATR':>7s} {'cost/R':>6s} {'swap':>5s}")
    print(hdr)
    print("-" * len(hdr))
    for r in all_rows:
        ex = r.get("exits", {})
        n = int(r["n"])
        tot = sum(ex.values()) or 1
        def sh(k: str) -> str:
            return f"{100.0 * ex.get(k, 0) / tot:6.2f}"
        sd = r["sized_down"]
        print(f"{r['win']:3s} {r['arm']:4s} {r['src']:9s} {r['label']:12s} {n:6d} "
              f"{float(r.get('pfr', 'nan')):7.4f} {float(r.get('pfusd', 'nan')):7.4f} "
              f"{float(r['exp']):+7.3f} {float(r.get('lbar', 'nan')):6.4f} {r.get('res', '?'):>8s} "
              f"{float(r.get('ddusd', 'nan')):8.2f} {float(r.get('ddpct', 'nan')):7.2f} "
              f"{(f'{100.0 * sd / n:5.1f}%' if sd is not None else ' null'):>6s} "
              f"{sh('STOP')} {sh('TARGET')} {sh('TIMEOUT')} {float(r.get('hold', 'nan')):6.1f} "
              f"{float(r.get('stopatr', 'nan')):7.3f} {float(r.get('costr', 'nan')):6.2f} {r['swap']:>5s}")

    # ---- doi chung (d): nhan L2/* khong doc cot khoang cach => phai y het
    print()
    print("== DOI CHUNG (d): nhan L2/* khong doc `cmp.high`, phai in SO Y HET o ba file ==")
    groups: dict[tuple, list[dict]] = {}
    for r in all_rows:
        if r["label"].startswith("L2/"):
            groups.setdefault((r["win"], r["arm"], r["label"]), []).append(r)
    bad = 0
    for key, g in sorted(groups.items()):
        ns = {x["n"] for x in g}
        pf = {x.get("pfr") for x in g}
        ok = len(ns) == 1 and len(pf) == 1
        bad += 0 if ok else 1
        if not ok:
            print(f"  TRUOT {key}: n={sorted(ns)} PF_r={sorted(pf)}")
    print(f"  {len(groups)} nhom x 3 file: {len(groups) - bad} PASS, {bad} TRUOT")

    # ---- bon phep so quyet dinh, cung stopAtr / cung cua so / cung arm
    print()
    print("== BON PHEP SO QUYET DINH (dang ky muc 2) — dPF_r, don vi R ==")
    print("   P1 L3(fc)-L2(fc)   P2 L3(fc)-L3(atr)   P3 L3tru(fc)-L3con(fc)   P4 L3tru(fc)-L3shf(fc)")
    print()
    idx = {(r["win"], r["arm"], r["src"], r["label"]): r for r in all_rows}

    def pfr(win, arm, src, label):
        r = idx.get((win, arm, src, label))
        return float(r["pfr"]) if r and r.get("pfr") else float("nan")

    print(f"{'cs':3s} {'arm':4s} {'f':4s} {'L2(fc)':>8s} {'L3tru(fc)':>9s} {'L3tru(atr)':>10s} "
          f"{'L3con(fc)':>9s} {'L3shf(fc)':>9s} | {'P1':>8s} {'P2':>8s} {'P3':>8s} {'P4':>8s}")
    for win in ("W1", "W2"):
        for arm in ("off", "on"):
            for f in ("1.0", "2.0"):
                l2 = pfr(win, arm, "XAUL3TRU", f"L2/fc/f{f}")
                tru = pfr(win, arm, "XAUL3TRU", f"L3/fc/f{f}")
                atr = pfr(win, arm, "XAUL3TRU", f"L3/atr/f{f}")
                con = pfr(win, arm, "XAUL3CON", f"L3/fc/f{f}")
                shf = pfr(win, arm, "XAUL3SHF", f"L3/fc/f{f}")
                print(f"{win:3s} {arm:4s} {f:4s} {l2:8.4f} {tru:9.4f} {atr:10.4f} {con:9.4f} {shf:9.4f} | "
                      f"{tru - l2:+8.4f} {tru - atr:+8.4f} {tru - con:+8.4f} {tru - shf:+8.4f}")
    print()
    print("  CHU: hai o khac `stopAtr` KHONG so sanh duoc o PF_r theo cung cach o L3")
    print("  (reward_risk thoi la hang so) — moi dong o tren la MOT `stopAtr`, khong tron.")

    # ---- F3: xao tron tai tao duoc bao nhieu phan cu dich?
    print()
    print("== F3: doi chung (b) tai tao duoc bao nhieu phan cu dich L3tru-L2? ==")
    print("   vach khai truoc: >= 50% thi F3 ban (thoi diem cua muc vo nghia)")
    print(f"{'cs':3s} {'arm':4s} {'f':4s} {'L3tru-L2':>9s} {'L3shf-L2':>9s} {'ti le':>8s}")
    for win in ("W1", "W2"):
        for arm in ("off", "on"):
            for f in ("1.0", "2.0"):
                l2 = pfr(win, arm, "XAUL3TRU", f"L2/fc/f{f}")
                tru = pfr(win, arm, "XAUL3TRU", f"L3/fc/f{f}")
                shf = pfr(win, arm, "XAUL3SHF", f"L3/fc/f{f}")
                a_, b_ = tru - l2, shf - l2
                print(f"{win:3s} {arm:4s} {f:4s} {a_:+9.4f} {b_:+9.4f} {(b_ / a_ if a_ else float('nan')):+8.1%}")

    # ---- cong: dem tay, moi dong
    print()
    print("== CONG, DEM TAY (PF_r >= 1,200 VA E >= +0,050R VA n >= 40, CA HAI cua so) ==")
    passed = [r for r in all_rows if float(r.get("pfr", "nan")) >= 1.200 and float(r["exp"]) >= 0.050 and int(r["n"]) >= 40]
    nmin = min(int(r["n"]) for r in all_rows)
    pfmax = max(float(r.get("pfr", "nan")) for r in all_rows)
    emax = max(float(r["exp"]) for r in all_rows)
    lbmin = min(float(r.get("lbar", "nan")) for r in all_rows)
    burned = [r for r in all_rows if float(r.get("ddpct", "0")) > 100.0]
    print(f"  qua cong: {len(passed)} / {len(all_rows)} dong")
    print(f"  n nho nhat ca bang: {nmin} (cong doi 40; tool kiem san 30)")
    print(f"  PF_r cao nhat: {pfmax:.4f} (vach 1,200)   E cao nhat: {emax:+.3f} R (vach +0,050)")
    print(f"  Lbar nho nhat: {lbmin:.4f} R => chan expectancy DU o moi dong (vach 0,250R)")
    print(f"  dong CHAY (_pct > 100%): {len(burned)} / {len(all_rows)} => PF_usd + drawdown cua chung KHONG doc duoc")
    swaps = {r["swap"] for r in all_rows}
    print(f"  cot swap$ doc: {sorted(swaps)} => carry = 0 theo cau hinh, expectancy_net == expectancy")
    return 0


if __name__ == "__main__":
    sys.exit(main())
