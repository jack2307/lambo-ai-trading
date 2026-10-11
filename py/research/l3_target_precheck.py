"""Tien kiem cho `agent/l3-target`: lop L3 — target la mot MUC GIA.

Dang ky: docs/decisions/2026-10-11-l3-target.md

`agent/range-sizing` dong lop L2 va noi ro vi sao: o L2 target la
`reward_risk x risk`, nen NOI STOP CUNG NOI TARGET, va target xa hon phai vao
duoc trong tran `max_hold_ms = 4h = 16 nen`. Do duoc, W1 guards-off, f=2,0:

    atr/f2.0 : STOP 3615  TARGET 1530  TIMEOUT 2613
    fc/f2.0  : STOP 3616  TARGET 1432  TIMEOUT 2686
    => STOP KHONG DOI; TARGET roi 98 lenh, TIMEOUT tang 73

Dung cu nay dung chuoi `d(i)` — KHOANG CACH tu `close(i)` den MUC GIA gan nhat
nam ngoai no — va ghi ba ban cua dung mot cot do vao ba file companion, de ba
nguon target khac nhau o MOT chuoi so va khong gi nua:

    XAUL3TRU : d(i) thuc    (muc time-profile gan nhat ngoai close(i))
    XAUL3SHF : d(i) XAO TRON TRONG THOI GIAN (doi chung b)
    XAUL3CON : d(i) = HANG SO moi cua so, bang trung binh cua ban thuc (doi chung c)

Ca ba mang `close = m(i)` — co so nhan stop, DA KHOP CO, lay nguyen duong ong
cua `range_sizing_precheck` — nen stop y het o ca ba file va thu duy nhat doi la
target.

Nam luat, moi cai la mot cho job nay co the chet:

1. KHONG DOC COT VOLUME. Muc la TIME profile (`price_levels.rs:300` tu khai
   `measure = "TIME_AT_PRICE"`). `XAUDUKA-1m.volume` null tren ca 5.635.777
   dong; ban 15m mang mot so 0 CHE RA. Moi lan doc parquet truyen
   `columns=["time","open","high","low","close"]`.
2. HANG SO CUA PROFILE LAY TU MA, khong quet: `buckets_per_atr = 4.0`
   (`fd-api/src/levels.rs:154`), `value_area_pct = 0.70` (`:158`),
   `profile_days = 5` (`:126`) => cua so 5 x 96 = 480 nen 15m NGAY TRUOC nen i.
3. NHAN QUA BANG CAU TRUC. Cua so profile ket thuc o `i` (nen i LOAI). Selftest
   cat chuoi o 5 moc va doi tung bit, roi doi BON probe nhin truoc PHAI TRUOT
   dung test do — ba probe muon nguyen tu `range_sizing_precheck` (tang dac
   trung va tang khop cua du bao) cong PROBE_profile_i1 moi cho tang MUC.
4. KHONG CO MUC NAO THOA => NULL, khong dien, khong ve 0. Va khoang cach phai
   > 0,28 USD = spread MOT VONG: mot target gan hon ca hoa don mot vong khong
   the la mot exit co lai.
5. CO STOP TRUNG BINH PHAI KHOP tren DUNG tap nen duoc giao dich (nen tin hieu
   co ca m va d) truoc khi so PF — `agent/stop-width` da do lop "stop rong hon
   thi PF doi" tren 200 o.

Hang so cua dung cu, khai truoc khi chay:

    PROFILE_BARS   480 nen 15m (= 5 ngay x 96), cua so ket thuc o i (i LOAI)
    BUCKETS_PER_ATR 4.0        tu fd-api/src/levels.rs:154
    VALUE_AREA_PCT  0.70       tu fd-api/src/levels.rs:158
    MIN_TARGET_USD  0.28       = spread MOT VONG, config markets.xauduka.trading
    SEED_LEVEL      20261011   xao tron muc trong thoi gian (cung seed ho so)
    DELTA_USD       0.1400     = spread/2 CHINH XAC, doc tu MA
    PARITY_BUCKET   0.625      = bucket_size_price(2.5, 4.0), bo fixture parity
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

sys.path.insert(0, str(Path(__file__).resolve().parent))

from intrabar_shape_precheck import (  # noqa: E402
    CONTROL,
    INTRABAR,
    M15_MS,
    WINDOWS,
    atr14_15m,
    iso,
    ms_of,
    peak_mem_mb,
    read_bars,
)
from range_sizing_precheck import (  # noqa: E402
    ATR_PERIOD,
    DELTA_USD,
    DONCHIAN,
    STOP_ATRS,
    design,
    multiplier,
    signal_bars,
    stack,
    walkforward,
)
from range_sizing_precheck import selftest as forecast_selftest  # noqa: E402

PROFILE_BARS = 480
BUCKETS_PER_ATR = 4.0
VALUE_AREA_PCT = 0.70
MIN_TARGET_USD = 0.28
MAX_BUCKETS = 5_000  # price_levels.rs:429
SEED_LEVEL = 20_261_011
PARITY_BUCKET = 0.625

TARGETS = ("tru", "shuf", "con")
COMPANION = {"tru": "XAUL3TRU", "shuf": "XAUL3SHF", "con": "XAUL3CON"}


# ------------------------------------------------------- time profile (POC/VAH/VAL)


def activity_profile(
    high: np.ndarray, low: np.ndarray, bucket: float, pct: float = VALUE_AREA_PCT
) -> tuple[float, float, float]:
    """POC / VAH / VAL cua mot `activity_profile` TIME-AT-PRICE tren mot cua so.

    Viet lai `fd_engine::price_levels::activity_profile`
    (`crates/fd-engine/src/price_levels.rs:449`) TUNG BUOC: moi nen cong 1,0 vao
    MOI bucket ma dai low->high cua no cham (dinh nghia market profile — mot nen
    rong gop nhieu hon vi no o nhieu gia hon), POC la bucket nang nhat voi hoa
    di ve bucket THAP hon, va value area la buoc di co dien "lay neo nang hon
    trong hai lang gieng" cho den khi phu `pct` cua tong.

    Phai viet lai (khong goi ban Rust) vi doi chung (b) — xao tron muc TRONG
    THOI GIAN — khong bieu dat duoc bang mot indicator tren chuoi chinh. Vi the
    dang ky doi mot test Rust goi dung ham do tren mot bo nen tat dinh ma ham
    nay dung lai duoc tung bit: `crates/fd-engine/tests/profile_parity.rs`.

    Tra `(nan, nan, nan)` khi khong do duoc — khong bao gio tra 0.
    """
    nan3 = (float("nan"), float("nan"), float("nan"))
    if not (np.isfinite(bucket) and bucket > 0.0):
        return nan3
    ok = np.isfinite(low) & np.isfinite(high) & (high >= low)
    if not ok.any():
        return nan3
    h, l = high[ok], low[ok]
    mn = float(l.min())
    mx = float(h.max())
    if not (np.isfinite(mn) and np.isfinite(mx)) or mx < mn:
        return nan3
    span = np.floor((mx - mn) / bucket)
    if not np.isfinite(span) or span < 0.0 or span > MAX_BUCKETS:
        return nan3
    count = int(span) + 1

    il = np.clip(np.floor((l - mn) / bucket), 0.0, count - 1).astype(np.int64)
    ih = np.clip(np.floor((h - mn) / bucket), 0.0, count - 1).astype(np.int64)
    diff = np.bincount(il, minlength=count + 1).astype(np.float64) - np.bincount(
        ih + 1, minlength=count + 1
    ).astype(np.float64)
    act = np.cumsum(diff)[:count]
    total = float(act.sum())
    if total <= 0.0:
        return nan3

    a = act.tolist()
    poc = int(np.argmax(act))  # hoa ve bucket THAP hon, dung nhu ban Rust
    target = total * pct
    lo = hi = poc
    acc = a[poc]
    neg = float("-inf")
    while acc < target and (lo > 0 or hi < count - 1):
        below = a[lo - 1] if lo > 0 else neg
        above = a[hi + 1] if hi < count - 1 else neg
        if above >= below:
            hi += 1
            acc += a[hi]
        else:
            lo -= 1
            acc += a[lo]

    def edge(k: int) -> float:
        return mn + k * bucket

    return (edge(poc) + bucket / 2.0, edge(hi + 1), edge(lo))


def parity_fixture(n: int = 600) -> dict[str, np.ndarray]:
    """Bo nen TAT DINH ma ban Rust dung lai duoc tung bit (LCG so nguyen).

    Chi so nguyen va phep chia cho 100.0 — hai thu IEEE-754 cho ket qua y het
    o moi ngon ngu — nen hai ban cai dat nhan DUNG CUNG dau vao, va mot khac
    biet o dau ra la mot khac biet ve SO HOC chu khong phai ve du lieu.
    """
    x = 20_261_011
    o = np.empty(n)
    h = np.empty(n)
    l = np.empty(n)
    c = np.empty(n)
    for k in range(n):
        x = (1_103_515_245 * x + 12_345) % (1 << 31)
        base = 1000.0 + (x % 4000) / 100.0
        x = (1_103_515_245 * x + 12_345) % (1 << 31)
        up = (x % 150) / 100.0
        x = (1_103_515_245 * x + 12_345) % (1 << 31)
        dn = (x % 150) / 100.0
        o[k] = base
        h[k] = base + up
        l[k] = base - dn
        c[k] = (base + up + base - dn) / 2.0
    return {"open": o, "high": h, "low": l, "close": c}


# --------------------------------------------------------------- muc -> khoang cach


def level_distance(
    b15: dict,
    atr: np.ndarray,
    idx: np.ndarray,
    up: np.ndarray,
    end_offset: int = 0,
) -> tuple[np.ndarray, np.ndarray]:
    """`d(i)` = khoang cach tu `close(i)` den MUC gan nhat nam ngoai no.

    Mot nen `i`:
      * cua so profile = `bars[i - PROFILE_BARS + end_offset : i + end_offset]`
        `end_offset = 0` => nen `i` LOAI, khong doc gi sau `i` (NHAN QUA).
        `end_offset = 2` => cua so GOM nen `i+1`  <- PROBE_profile_i1, phai truot C1.
      * bucket = ATR14(i) / 4.0   (ATR warm toi het nen i)
      * long  (`up[i]`): muc THAP NHAT trong {POC, VAH, VAL} nam TREN close(i)
        short          : muc CAO NHAT  trong {POC, VAH, VAL} nam DUOI close(i)
      * khoang cach phai > MIN_TARGET_USD, khong thi NaN (khong dien, khong ve 0)

    `idx` la cac nen can tinh (nen tin hieu). Moi nen ngoai `idx` la NaN: cac nen
    do khong bao gio vao lenh, nen mot muc o do la mot con so khong ai doc.

    Tra `(dist, kind)` — `kind` la 0 khong co muc / 1 POC / 2 VAH / 3 VAL, chi
    de BAO CAO, va NaN o nen khong tinh.
    """
    n = b15["close"].shape[0]
    dist = np.full(n, np.nan)
    kind = np.full(n, np.nan)
    high, low, close = b15["high"], b15["low"], b15["close"]
    for raw in idx:
        i = int(raw)
        end = i + end_offset
        start = end - PROFILE_BARS
        if start < 0 or end > n:
            continue
        a = atr[i]
        if not (np.isfinite(a) and a > 0.0):
            continue
        poc, vah, val = activity_profile(high[start:end], low[start:end], a / BUCKETS_PER_ATR)
        if not np.isfinite(poc):
            continue
        c = close[i]
        named = ((1.0, poc), (2.0, vah), (3.0, val))
        if up[i]:
            cand = [(v - c, k) for k, v in named if np.isfinite(v) and v - c > MIN_TARGET_USD]
        else:
            cand = [(c - v, k) for k, v in named if np.isfinite(v) and c - v > MIN_TARGET_USD]
        if not cand:
            kind[i] = 0.0
            continue
        best = min(cand)
        dist[i] = best[0]
        kind[i] = best[1]
    return dist, kind


def entry_side_up(b15: dict, period: int = DONCHIAN) -> np.ndarray:
    """`True` o nen tin hieu LONG (close > kenh tren), `False` o moi nen khac.

    Cung cua so kenh `range-stop` dung: `bars[i-period..i]`, nen `i` LOAI.
    """
    h, c = b15["high"], b15["close"]
    n = c.shape[0]
    up = np.zeros(n, dtype=bool)
    if n <= period:
        return up
    win_h = np.lib.stride_tricks.sliding_window_view(h, period)[:-1]
    up[period:] = c[period:] > win_h.max(axis=1)
    return up


def shuffled_in_time(d: np.ndarray, seed: int = SEED_LEVEL) -> np.ndarray:
    """Hoan vi cac gia tri huu han TRONG THOI GIAN, giu nguyen vi tri NaN.

    => cung tap nen, cung PHAN PHOI KHOANG CACH TARGET, mat het thong tin
    thoi diem. Dung dieu khoan doi chung (b) cua dang ky.
    """
    out = d.copy()
    i = np.flatnonzero(np.isfinite(d))
    if i.size:
        out[i] = np.random.default_rng(seed).permutation(d[i])
    return out


# ------------------------------------------------------------------- selftest


def selftest_levels(b15: dict, t15: np.ndarray) -> bool:
    """C1 tung bit tren chuoi MUC + PROBE_profile_i1 phai truot no."""
    print("\n=== TEST NHAN QUA — TANG MUC (dang ky muc 6) ===")
    ok_all = True
    n = 4000
    s15 = {k: v[:n].copy() for k, v in b15.items()}
    st15 = t15[:n].copy()
    atr = atr14_15m(s15, ATR_PERIOD)
    sig = np.flatnonzero(signal_bars(s15, DONCHIAN))
    sig = sig[sig >= PROFILE_BARS]
    if sig.size < 50:
        print(f"  KHONG DO DUOC: chi {sig.size} nen tin hieu tren lat nay")
        return False
    up = entry_side_up(s15, DONCHIAN)

    def chain(bb15, end_offset: int):
        aa = atr14_15m(bb15, ATR_PERIOD)
        ss = np.flatnonzero(signal_bars(bb15, DONCHIAN))
        ss = ss[ss >= PROFILE_BARS]
        uu = entry_side_up(bb15, DONCHIAN)
        return level_distance(bb15, aa, ss, uu, end_offset=end_offset)[0]

    full_ok = chain(s15, 0)
    full_probe = chain(s15, 2)

    cuts = [int(n * q) for q in (0.55, 0.65, 0.75, 0.85, 0.93)]
    c1_bad = 0
    probe_moved = 0
    for cut in cuts:
        T = int(st15[cut])
        c15 = {k: v[:cut].copy() for k, v in s15.items()}
        ct15 = st15[:cut]
        aa, bb = full_ok[:cut], chain(c15, 0)
        bad = int((~((np.isnan(aa) & np.isnan(bb)) | (aa == bb))).sum())
        if bad:
            c1_bad += 1
            print(f"  C1 TRUOT: d doi {bad} gia tri khi cat o {iso(T)}")
        pa_, pb = full_probe[:cut], chain(c15, 2)
        if int((~((np.isnan(pa_) & np.isnan(pb)) | (pa_ == pb))).sum()):
            probe_moved += 1

    if c1_bad == 0:
        print(f"  C1 PASS: chuoi d (muc time-profile), x {len(cuts)} moc cat, 0 gia tri doi")
        print("           (so sanh TUNG BIT, NaN==NaN)")
    else:
        ok_all = False
    tag = "PASS" if probe_moved == len(cuts) else "TRUOT"
    print(f"  C2 {tag}: PROBE_profile_i1 truot C1 o {probe_moved}/{len(cuts)} moc")
    print("           — cua so profile GOM nen i+1 (end_offset = 2)")
    ok_all &= probe_moved == len(cuts)

    d = full_ok
    fin = np.isfinite(d)
    print(f"  NULL-KHONG-PHAI-0 {'PASS' if (d[~fin] != d[~fin]).all() else 'TRUOT'}: "
          f"{int((~fin).sum())} nen khong co muc thoa => NaN, 0 nen mang mot so 0")
    sh = shuffled_in_time(d)
    same_null = bool(np.array_equal(np.isnan(sh), np.isnan(d)))
    kept = bool(np.allclose(np.sort(sh[np.isfinite(sh)]), np.sort(d[np.isfinite(d)])))
    print(f"  XAO-TRON {'PASS' if (same_null and kept) else 'TRUOT'}: vi tri NaN y het ({same_null}), "
          f"phan phoi khoang cach y het ({kept})")
    ok_all &= same_null and kept
    return ok_all


def parity_report() -> None:
    """In POC/VAH/VAL cua bo fixture, de test Rust assert DUNG ba so nay."""
    f = parity_fixture()
    poc, vah, val = activity_profile(f["high"], f["low"], PARITY_BUCKET, VALUE_AREA_PCT)
    print("\n=== DOI CHUNG (e): PARITY SO HOC VOI BAN RUST ===")
    print(f"  fixture: 600 nen LCG tat dinh, bucket {PARITY_BUCKET} (= bucket_size_price(2.5, 4.0))")
    print(f"  high[0] {f['high'][0]!r}  low[0] {f['low'][0]!r}  high[599] {f['high'][599]!r}  low[599] {f['low'][599]!r}")
    print(f"  Python: POC {poc!r}  VAH {vah!r}  VAL {val!r}")
    print("  (crates/fd-engine/tests/profile_parity.rs assert DUNG ba so nay tu ham Rust)")


# ------------------------------------------------------------------ bao cao


def pct_of(x: np.ndarray, q: float) -> float:
    v = x[np.isfinite(x)]
    return float(np.percentile(v, q)) if v.size else float("nan")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", default="E:/rust/flowdesk/data")
    ap.add_argument("--symbol", default="XAUDUKA")
    ap.add_argument("--out", default="data-l3/bars")
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--write", action="store_true")
    a = ap.parse_args()

    t0 = time.perf_counter()
    bars = Path(a.data) / "bars"
    p1, p15 = bars / f"{a.symbol}-1m.parquet", bars / f"{a.symbol}-15m.parquet"
    print("=== KHO ===")
    for p in (p1, p15):
        print(f"  {p.name}: {pq.ParquetFile(p).metadata.num_rows} dong — doc 5 cot, `volume` KHONG trong `columns=`")
    b1 = read_bars(p1)
    b15 = read_bars(p15)
    t15 = b15["time"]
    print(f"  1m  {b1['time'].shape[0]} nen {iso(int(b1['time'][0]))} -> {iso(int(b1['time'][-1]))}")
    print(f"  15m {t15.shape[0]} nen {iso(int(t15[0]))} -> {iso(int(t15[-1]))}")
    print(f"  doc xong sau {time.perf_counter() - t0:.2f} s")

    parity_report()

    if a.selftest:
        if not forecast_selftest(b1, b15, t15):
            print("\nSELFTEST (tang du bao) TRUOT => F8 BAN. Khong cong bo so nao, tieu 0 o cong.")
            return 1
        if not selftest_levels(b15, t15):
            print("\nSELFTEST (tang muc) TRUOT => F8 BAN. Khong cong bo so nao, tieu 0 o cong.")
            return 1

    print("\n=== DUNG DAC TRUNG + DU BAO (toan chuoi, 1 lan) ===")
    d = design(b1, b15, t15)
    pred15, n15 = walkforward(stack(d, CONTROL), d["y"], t15)
    pred1m, n1m = walkforward(stack(d, CONTROL + INTRABAR), d["y"], t15)
    print(f"  khop lai: fc15 {n15} lan, fc1m {n1m} lan (moc dau thang, tap khop EXPANDING)")
    m_fc15 = multiplier(pred15)
    m_raw = multiplier(pred1m)
    # MAT NA cua `agent/range-sizing`, giu nguyen: m hop le khi CA HAI nguon co
    # so, de tap nen cua job nay LONG trong tap nen cua ket qua da cong bo.
    both = np.isfinite(m_raw) & np.isfinite(m_fc15)
    m_raw = np.where(both, m_raw, np.nan)
    print(f"  mat na cua range-sizing (fc1m & fc15 co so): {int(both.sum())} / {t15.shape[0]} "
          f"= {100.0 * both.sum() / t15.shape[0]:.4f}%")

    atr = d["atr"]
    sig_mask = signal_bars(b15, DONCHIAN)
    up = entry_side_up(b15, DONCHIAN)
    sig = np.flatnonzero(sig_mask)
    sig = sig[sig >= PROFILE_BARS]
    print(f"  nen tin hieu (donchian {DONCHIAN}, nen hien tai LOAI, >= {PROFILE_BARS} nen warm): {sig.size}")

    print(f"\n=== MUC TIME-PROFILE tren {sig.size} nen tin hieu ===")
    print(f"  cua so {PROFILE_BARS} nen truoc i · bucket ATR14(i)/{BUCKETS_PER_ATR} · "
          f"value area {VALUE_AREA_PCT:.0%} · min {MIN_TARGET_USD} USD")
    tl = time.perf_counter()
    dist, kind = level_distance(b15, atr, sig, up, end_offset=0)
    dist = np.where(sig_mask, dist, np.nan)
    print(f"  xong sau {time.perf_counter() - tl:.1f} s")
    names = {0.0: "khong co muc", 1.0: "POC", 2.0: "VAH", 3.0: "VAL"}
    tot = int(np.isfinite(kind).sum())
    print("  muc nao duoc chon: " + "  ".join(
        f"{nm} {int((kind == code).sum())} ({100.0 * int((kind == code).sum()) / max(tot, 1):.1f}%)"
        for code, nm in names.items()))

    # Mat na chung CUOI CUNG: m co so (mat na range-sizing) VA d co so.
    mask = np.isfinite(m_raw) & np.isfinite(dist)
    print(f"  mat na CHUNG (m co so & d co so) tren nen tin hieu: "
          f"{int((mask & sig_mask).sum())} / {int(sig_mask.sum())} nen tin hieu "
          f"= {100.0 * (mask & sig_mask).sum() / max(int(sig_mask.sum()), 1):.4f}%")
    dist = np.where(mask, dist, np.nan)

    print("\n=== PHAN PHOI KHOANG CACH TARGET d(i) tren nen GIAO DICH DUOC ===")
    print("  cua so  p10     p50     p90    mean    n      d/ATR(p50)")
    for wl, (fr, to) in WINDOWS.items():
        w = (t15 >= ms_of(fr)) & (t15 < ms_of(to)) & mask & sig_mask
        v = np.where(w, dist, np.nan)
        vv = v[np.isfinite(v)]
        ratio = np.where(w, dist / atr, np.nan)
        print(f"  {wl[:2]}    {pct_of(v, 10):6.3f} {pct_of(v, 50):6.3f} {pct_of(v, 90):6.3f} "
              f"{float(np.mean(vv)) if vv.size else float('nan'):6.3f} {vv.size:7d}   {pct_of(ratio, 50):6.3f}")

    print("\n=== TARGET CO NAM SAU ENTRY KHONG (phien ban 'wrong_side' cua target) ===")
    print("  entry = open(i+1) + spread/2; target = close(i) + d(i). Neu nen i+1 GAP")
    print("  qua target thi `check_exit` dong ngay o bar.open va in TARGET tren mot")
    print("  lenh chua di dau ca. Day la mot so DEM DUOC, khong phai mot gia dinh.")
    nxt_open = np.full(t15.shape[0], np.nan)
    nxt_open[:-1] = b15["open"][1:]
    tgt = np.where(up, b15["close"] + dist, b15["close"] - dist)
    entry = np.where(up, nxt_open + DELTA_USD, nxt_open - DELTA_USD)
    behind = np.where(up, tgt <= entry, tgt >= entry)
    for wl, (fr, to) in WINDOWS.items():
        w = (t15 >= ms_of(fr)) & (t15 < ms_of(to)) & mask & sig_mask & np.isfinite(nxt_open)
        k = int(w.sum())
        bad = int((w & behind).sum())
        print(f"  {wl[:2]}: {bad} / {k} = {100.0 * bad / max(k, 1):.4f}% nen tin hieu co target SAU entry")

    print("\n=== HANG SO KHOP CO STOP: mot so moi cua so, tren DUNG tap giao dich duoc ===")
    print("  m_khop(i) = m(i) / [mean(ATR x m) / mean(ATR)] tren nen tin hieu co ca m va d,")
    print("  nen cung `stopAtr` nghia la cung co stop trung binh o ca hai nhanh stop.")
    m = np.full(t15.shape[0], np.nan)
    for wl, (fr, to) in WINDOWS.items():
        inw = (t15 >= ms_of(fr)) & (t15 < ms_of(to))
        w = inw & mask & sig_mask
        base = np.where(w, atr, np.nan)
        ref = float(np.nanmean(base))
        mu = float(np.nanmean(base * np.where(w, m_raw, np.nan)))
        r = mu / ref
        m[inw] = m_raw[inw] / r
        print(f"  {wl[:2]} ti so co = {r:.6f}  => m chia cho no   (ATR trung binh {ref:.4f} USD)")
    print("  kiem lai sau khi chia (phai doc 0,0000%):")
    for wl, (fr, to) in WINDOWS.items():
        w = (t15 >= ms_of(fr)) & (t15 < ms_of(to)) & mask & sig_mask
        base = np.where(w, atr, np.nan)
        ref = float(np.nanmean(base))
        mu = float(np.nanmean(base * np.where(w, m, np.nan)))
        mk = np.where(w, m, np.nan)
        print(f"  {wl[:2]}: {100.0 * (mu / ref - 1.0):+.4f}%   m_khop: p10 {pct_of(mk, 10):.4f} "
              f"p50 {pct_of(mk, 50):.4f} p90 {pct_of(mk, 90):.4f}")

    print("\n=== CO STOP, CHI PHI/R VA KHUYET DIEM 16 (stop BIEN THIEN) ===")
    for wl, (fr, to) in WINDOWS.items():
        w = (t15 >= ms_of(fr)) & (t15 < ms_of(to)) & mask & sig_mask
        base = np.where(w, atr, np.nan)
        for f in STOP_ATRS:
            dv = f * base * np.where(w, m, np.nan)
            ref = float(np.nanmean(f * base))
            print(f"  {wl[:2]} stopAtr {f:.1f}: atr {ref:7.4f} USD   fc {float(np.nanmean(dv)):7.4f} USD "
                  f"({100.0 * (float(np.nanmean(dv)) / ref - 1.0):+.2f}%)")
            print(f"       khuyet diem 16: delta = spread/2 = {DELTA_USD:.4f} diem gia (TU MA: d risk/d s = +1/2"
                  f" chinh xac, apply_costs engine.rs:902-912 + risk engine.rs:717-719)"
                  f" = {100.0 * DELTA_USD / pct_of(dv, 10):.2f}% cua R o p10(d), "
                  f"{100.0 * DELTA_USD / pct_of(dv, 90):.2f}% o p90(d)")
            print(f"       chi phi/R (spread 0,28 CAU HINH, khong do): {100.0 * 0.28 / pct_of(dv, 50):.2f}% "
                  f"cua R o p50(d) = {pct_of(dv, 50):.3f} USD")
            tr = np.where(w, dist, np.nan) / dv
            print(f"       target/stop (= reward_risk THUC TE cua L3, KHONG phai hang so): "
                  f"p10 {pct_of(tr, 10):.3f} p50 {pct_of(tr, 50):.3f} p90 {pct_of(tr, 90):.3f}  "
                  f"(L2 dung 1,800 o moi lenh)")

    print("\n=== BA NGUON TARGET ===")
    out = {"tru": dist, "shuf": shuffled_in_time(dist)}
    con = np.full(t15.shape[0], np.nan)
    for wl, (fr, to) in WINDOWS.items():
        inw = (t15 >= ms_of(fr)) & (t15 < ms_of(to))
        w = inw & mask & sig_mask
        k = float(np.nanmean(np.where(w, dist, np.nan)))
        con[inw] = np.where(np.isfinite(dist[inw]), k, np.nan)
        print(f"  {wl[:2]} doi chung (c) hang so khoang cach = {k:.6f} USD (trung binh d tren nen giao dich duoc)")
    out["con"] = con
    for nm in TARGETS:
        v = out[nm]
        for wl, (fr, to) in WINDOWS.items():
            w = (t15 >= ms_of(fr)) & (t15 < ms_of(to)) & mask & sig_mask
            vv = np.where(w, v, np.nan)
            print(f"  {nm:4s} {wl[:2]}: p10 {pct_of(vv, 10):6.3f} p50 {pct_of(vv, 50):6.3f} "
                  f"p90 {pct_of(vv, 90):6.3f} mean {float(np.nanmean(vv)):6.3f} USD  "
                  f"n {int(np.isfinite(vv).sum())}")

    if a.write:
        od = Path(a.out)
        od.mkdir(parents=True, exist_ok=True)
        print("\n=== GHI PARQUET COMPANION (close = m DA KHOP CO, high = d) ===")
        for nm in TARGETS:
            dd = np.where(mask, out[nm], np.nan)
            mm = np.where(mask, m, np.nan)
            tb = pa.table(
                {
                    "time": pa.array(t15, type=pa.timestamp("ms", tz="UTC")),
                    # `open`/`low` mang CUNG `d` de khong co mot slot nao mang
                    # mot so khong ai khai; `change`/`atr` cua `cmp` tren file
                    # nay la so hoc tren nhung slot KHONG phai mot chuoi gia va
                    # KHONG duoc doc (companion.rs::aligned_ohl noi dung the).
                    "open": pa.array(dd, type=pa.float64()),
                    "high": pa.array(dd, type=pa.float64()),
                    "low": pa.array(dd, type=pa.float64()),
                    "close": pa.array(mm, type=pa.float64()),
                    "volume": pa.array(np.full(t15.shape[0], None, dtype=object), type=pa.float64()),
                }
            )
            path = od / f"{COMPANION[nm]}-15m.parquet"
            pq.write_table(tb, path, compression="zstd")
            print(f"  {path} — {t15.shape[0]} dong, m co so {int(np.isfinite(mm).sum())}, "
                  f"d co so {int(np.isfinite(dd).sum())}, volume NULL het")

    print(f"\ntong {time.perf_counter() - t0:.1f} s · dinh bo nho {peak_mem_mb():.0f} MB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
