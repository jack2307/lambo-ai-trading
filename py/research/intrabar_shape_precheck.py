"""Tien kiem: hinh trong ruot nen 15m, dung THUAN tu O/H/L/C cua nen 1m.

Dang ky: docs/decisions/2026-10-10-intrabar-shape.md

Cau hoi: hinh thoi-gian-tai-gia trong mot nen 15m (dung tu 15 nen 1m) co phan
biet duoc ket cuoc cua nen 15m TIEP THEO hon chinh O/H/L/C cua nen 15m do khong?

Ba luat cua dung cu nay, moi cai la mot cho job nay co the chet:

1. KHONG DOC COT VOLUME. `XAUDUKA-1m` null tren ca 5.635.777 dong; 15m mang
   mot so 0 che ra tu buoc `null -> 0`. Cot volume khong duoc mo ra o day,
   `columns=` cua moi lan doc parquet noi dung the.
2. NHAN QUA. Tin hieu o nen `i` khop o OPEN cua nen `i+1`, nen hinh doc o nen
   `i` chi duoc gom 15 phut cua CHINH nen `i`. Bat chuoc tinh chat cua
   `fd_indicators::companion::aligned_change`: khop `bar.time` CHINH XAC, bar
   thieu thi bo qua (khong lay bar truoc, khong dien 0). `--selftest` chay ba
   dieu khoan C1/C2/C3 cua dang ky, trong do C2 doi mot dac trung co tinh nhin
   truoc PHAI truot C1 — mot test luon pass la mot test vo gia tri.
3. NEN 15m KHONG DU 15 NEN 1m => DAC TRUNG NULL, khong phai 0. Va doi chung
   chay tren DUNG CUNG TAP DONG do.
"""

from __future__ import annotations

import argparse
import ctypes
import ctypes.wintypes as wt
import sys
import time
from pathlib import Path

import numpy as np
import pyarrow.parquet as pq

M15_MS = 900_000
M1_MS = 60_000
SLOTS = 15
POC_BINS = 10

# Hai cua so cua ho so (phu luc 5 muc B). Nua mo: [tu, den).
WINDOWS = {
    "W1 2010-06 -> 2018-06": ("2010-06-01", "2018-06-01"),
    "W2 2018-06 -> 2026-06": ("2018-06-01", "2026-06-01"),
}

INTRABAR = [
    "tpo_skew",
    "t_high",
    "t_low",
    "extreme_order",
    "path_eff",
    "late_push",
    "max_1m_share",
    "tpo_poc_pos",
]
CONTROL = ["ret_15", "body", "uw", "lw", "rng_atr", "close_pos"]

# Vach Bonferroni khai truoc: 16 phep so chinh, alpha 0,05 => alpha/16 = 0,003125
T_GATE = 2.95


# ---------------------------------------------------------------- bo nho / gio


class _PMC(ctypes.Structure):
    _fields_ = [
        ("cb", wt.DWORD),
        ("PageFaultCount", wt.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
    ]


def peak_mem_mb() -> float:
    """Dinh working set cua process nay, MB. Khong do duoc thi `nan`, khong 0."""
    pmc = _PMC()
    pmc.cb = ctypes.sizeof(_PMC)
    # Thu pham cua `nan` o lan chay dau: khong dat `restype` thi ctypes coi
    # `GetCurrentProcess()` la `c_int`, va handle gia -1 bi CAT con 32 bit khi
    # truyen tiep. Phai khai ca `restype` lan `argtypes`.
    k32 = ctypes.windll.kernel32
    k32.GetCurrentProcess.restype = wt.HANDLE
    h = k32.GetCurrentProcess()
    for fn in (
        getattr(k32, "K32GetProcessMemoryInfo", None),
        getattr(ctypes.windll.psapi, "GetProcessMemoryInfo", None),
    ):
        if fn is None:
            continue
        fn.argtypes = [wt.HANDLE, ctypes.POINTER(_PMC), wt.DWORD]
        fn.restype = wt.BOOL
        if fn(h, ctypes.byref(pmc), pmc.cb) and pmc.PeakWorkingSetSize > 0:
            return pmc.PeakWorkingSetSize / 1048576.0
    return float("nan")


def ms_of(day: str) -> int:
    return int(np.datetime64(day + "T00:00:00", "ms").astype("int64"))


def iso(ms: int) -> str:
    return str(np.datetime64(int(ms), "ms"))


# ------------------------------------------------------------------- doc kho


def read_bars(path: Path) -> dict[str, np.ndarray]:
    """O/H/L/C + time. `volume` KHONG nam trong `columns` — co y."""
    tb = pq.read_table(path, columns=["time", "open", "high", "low", "close"])
    return {
        "time": tb["time"].cast("int64").to_numpy(),
        "open": tb["open"].to_numpy(),
        "high": tb["high"].to_numpy(),
        "low": tb["low"].to_numpy(),
        "close": tb["close"].to_numpy(),
    }


# ------------------------------------------------------- xep 1m vao o 15 phut


def grid_1m(b1: dict, t15: np.ndarray) -> tuple[np.ndarray, ...]:
    """Tra ve (o,h,l,c) hinh (n15, 15), NaN o cho khong co nen 1m.

    Khop CHINH XAC: nen 1m chi vao o cua nen 15m mang dung `time // 900000`, va
    chi vao slot `(time % 900000) // 60000`. Mot nen 1m co timestamp le (khong
    chia het 60s) hoac thuoc mot bucket 15m khong co trong `t15` thi bi BO, chu
    khong bi gan vao o gan nhat.
    """
    n15 = t15.shape[0]
    t = b1["time"]
    bucket = (t // M15_MS) * M15_MS
    slot = (t - bucket) // M1_MS
    pos = np.searchsorted(t15, bucket)
    pos_c = np.clip(pos, 0, max(n15 - 1, 0))
    keep = (pos < n15) & (t15[pos_c] == bucket) & (slot >= 0) & (slot < SLOTS)
    keep &= (t - bucket) % M1_MS == 0
    row = pos_c[keep]
    col = slot[keep].astype(np.int64)
    out = []
    for k in ("open", "high", "low", "close"):
        g = np.full((n15, SLOTS), np.nan)
        g[row, col] = b1[k][keep]
        out.append(g)
    return tuple(out)


def atr14_15m(b15: dict, period: int = 14) -> np.ndarray:
    """ATR Wilder tren nen 15m, tinh TOI HET nen i (biet duoc luc quyet dinh).

    NaN truoc khi warm. True range cua nen i dung close(i-1), nen no khong doc
    nen nao sau i.
    """
    h, l, c = b15["high"], b15["low"], b15["close"]
    n = h.shape[0]
    pc = np.empty(n)
    pc[0] = np.nan
    pc[1:] = c[:-1]
    tr = np.maximum(h - l, np.maximum(np.abs(h - pc), np.abs(l - pc)))
    atr = np.full(n, np.nan)
    if n <= period:
        return atr
    first = np.nanmean(tr[1 : period + 1])
    atr[period] = first
    prev = first
    for i in range(period + 1, n):
        prev = (prev * (period - 1) + tr[i]) / period
        atr[i] = prev
    return atr


# ----------------------------------------------------------------- dac trung


def intrabar_features(
    g: tuple[np.ndarray, ...], b15: dict, probe: bool = False, b1: dict | None = None, t15: np.ndarray | None = None
) -> dict[str, np.ndarray]:
    """8 dac trung hinh-trong-ruot. NULL o moi nen khong du 15 nen 1m."""
    go, gh, gl, gc = g
    n15 = go.shape[0]
    full = np.isfinite(go).sum(axis=1) == SLOTS
    full &= np.isfinite(gh).sum(axis=1) == SLOTS
    full &= np.isfinite(gl).sum(axis=1) == SLOTS
    full &= np.isfinite(gc).sum(axis=1) == SLOTS

    O, H, L, C = b15["open"], b15["high"], b15["low"], b15["close"]
    RNG = H - L
    ok = full & (RNG > 0)

    f = {k: np.full(n15, np.nan) for k in INTRABAR}
    if not ok.any():
        return f

    i = np.flatnonzero(ok)
    oo, hh, ll, cc = go[i], gh[i], gl[i], gc[i]
    Oi, Li, Ri = O[i], L[i], RNG[i]

    mid = (hh + ll) / 2.0
    half = (Li + Ri / 2.0)[:, None]
    f["tpo_skew"][i] = ((mid > half).sum(axis=1) - (mid < half).sum(axis=1)) / float(SLOTS)

    f["t_high"][i] = np.argmax(hh, axis=1) / float(SLOTS - 1)
    f["t_low"][i] = np.argmin(ll, axis=1) / float(SLOTS - 1)
    f["extreme_order"][i] = f["t_high"][i] - f["t_low"][i]

    prev_c = np.concatenate([Oi[:, None], cc[:, :-1]], axis=1)
    path = np.abs(cc - prev_c).sum(axis=1)
    eff = np.full(i.shape[0], np.nan)
    pos = path > 0
    eff[pos] = (cc[pos, -1] - Oi[pos]) / path[pos]
    f["path_eff"][i] = eff

    # phut 12..15 = 4 phut cuoi => moc la close cua phut 11 (chi so 10)
    f["late_push"][i] = (cc[:, -1] - cc[:, 10]) / Ri
    f["max_1m_share"][i] = (hh - ll).max(axis=1) / Ri

    # time profile: moi phut MOT trong so bang nhau, rai deu tren cac o gia ma
    # [l_j, h_j] phu. Khong doc volume.
    edges = Li[:, None] + (Ri[:, None] / POC_BINS) * np.arange(POC_BINS + 1)[None, :]
    lo = edges[:, None, :-1]
    hi = edges[:, None, 1:]
    lj = ll[:, :, None]
    hj = hh[:, :, None]
    span = np.maximum(hj - lj, 0.0)
    ov = np.clip(np.minimum(hj, hi) - np.maximum(lj, lo), 0.0, None)
    with np.errstate(invalid="ignore", divide="ignore"):
        w = np.where(span > 0, ov / np.where(span > 0, span, 1.0), 0.0)
    flat = span[:, :, 0] <= 0  # nen 1m phang: dat tron mot phut vao o chua no
    if flat.any():
        kflat = np.clip(((lj[:, :, 0] - Li[:, None]) / (Ri[:, None] / POC_BINS)).astype(np.int64), 0, POC_BINS - 1)
        rr, mm = np.nonzero(flat)
        w[rr, mm, :] = 0.0
        w[rr, mm, kflat[rr, mm]] = 1.0
    prof = w.sum(axis=1) / float(SLOTS)
    f["tpo_poc_pos"][i] = (np.argmax(prof, axis=1) + 0.5) / POC_BINS

    if probe:
        # C2: dac trung CO TINH nhin truoc — doc close cua PHUT 16, tuc phut dau
        # cua nen i+1. C1 PHAI truot tren no.
        assert b1 is not None and t15 is not None
        pk = np.full(n15, np.nan)
        want = t15 + M15_MS
        j = np.searchsorted(b1["time"], want)
        jc = np.clip(j, 0, b1["time"].shape[0] - 1)
        hit = (j < b1["time"].shape[0]) & (b1["time"][jc] == want)
        pk[hit] = b1["close"][jc[hit]]
        f["PROBE_peek_m16"] = pk
    return f


def control_features(b15: dict, atr: np.ndarray) -> dict[str, np.ndarray]:
    O, H, L, C = b15["open"], b15["high"], b15["low"], b15["close"]
    RNG = H - L
    n = O.shape[0]
    g = {k: np.full(n, np.nan) for k in CONTROL}
    ok = RNG > 0
    i = np.flatnonzero(ok)
    g["ret_15"][i] = (C[i] - O[i]) / RNG[i]
    g["body"][i] = np.abs(C[i] - O[i]) / RNG[i]
    g["uw"][i] = (H[i] - np.maximum(O[i], C[i])) / RNG[i]
    g["lw"][i] = (np.minimum(O[i], C[i]) - L[i]) / RNG[i]
    g["close_pos"][i] = (C[i] - L[i]) / RNG[i]
    with np.errstate(invalid="ignore", divide="ignore"):
        g["rng_atr"] = np.where(np.isfinite(atr) & (atr > 0), RNG / atr, np.nan)
    return g


def outcome(b15: dict, t15: np.ndarray, atr: np.ndarray, kind: str = "dir") -> np.ndarray:
    """`dir`: y(i) = (close(i+1) - open(i+1)) / ATR14(i)  — chieu, cai cong do.
    `rng`: y(i) = (high(i+1) - low(i+1)) / ATR14(i)      — bien do, truc G.

    C3: chi do khi nen i+1 ton tai VA cach dung mot nen (900_000 ms). Qua cuoi
    tuan / qua nghi phien thi TU CHOI, khai null — khong do.
    """
    n = t15.shape[0]
    y = np.full(n, np.nan)
    if n < 2:
        return y
    adj = (t15[1:] - t15[:-1]) == M15_MS
    a = atr[:-1]
    good = adj & np.isfinite(a) & (a > 0)
    i = np.flatnonzero(good)
    if kind == "rng":
        y[i] = (b15["high"][i + 1] - b15["low"][i + 1]) / a[i]
    else:
        y[i] = (b15["close"][i + 1] - b15["open"][i + 1]) / a[i]
    return y


# ----------------------------------------------------------------- thong ke


def tstat(x: np.ndarray, y: np.ndarray) -> tuple[float, float, int]:
    m = np.isfinite(x) & np.isfinite(y)
    n = int(m.sum())
    if n < 30:
        return float("nan"), float("nan"), n
    xx, yy = x[m], y[m]
    if np.std(xx) == 0 or np.std(yy) == 0:
        return float("nan"), float("nan"), n
    r = float(np.corrcoef(xx, yy)[0, 1])
    if abs(r) >= 1.0:
        return float("nan"), r, n
    return r * np.sqrt((n - 2) / (1 - r * r)), r, n


def ols_fit(X: np.ndarray, y: np.ndarray) -> np.ndarray:
    A = np.column_stack([np.ones(X.shape[0]), X])
    beta, *_ = np.linalg.lstsq(A, y, rcond=None)
    return beta


def ols_pred(beta: np.ndarray, X: np.ndarray) -> np.ndarray:
    return np.column_stack([np.ones(X.shape[0]), X]) @ beta


def oos(Xtr, ytr, Xte, yte) -> tuple[float, float]:
    beta = ols_fit(Xtr, ytr)
    p = ols_pred(beta, Xte)
    sse = float(np.sum((yte - p) ** 2))
    sst = float(np.sum((yte - np.mean(yte)) ** 2))
    r2 = 1.0 - sse / sst if sst > 0 else float("nan")
    nz = p != 0
    acc = float(np.mean(np.sign(p[nz]) == np.sign(yte[nz]))) if nz.any() else float("nan")
    return r2, acc


# ------------------------------------------------------------------ selftest


def selftest(b1: dict, b15: dict, t15: np.ndarray) -> bool:
    """C1 tien to · C2 bay tu ban · C3 luoi chinh xac."""
    print("\n=== TEST NHAN QUA (dang ky muc 4) ===")
    ok_all = True

    # Mot lat that, du dai de co nghi phien va cuoi tuan trong do.
    n = 4000
    s15 = {k: v[:n].copy() for k, v in b15.items()}
    st15 = t15[:n].copy()
    hi = st15[-1] + M15_MS
    m1 = b1["time"] < hi
    s1 = {k: v[m1].copy() for k, v in b1.items()}

    atr_full = atr14_15m(s15)
    g_full = grid_1m(s1, st15)
    f_full = intrabar_features(g_full, s15, probe=True, b1=s1, t15=st15)
    y_full = outcome(s15, st15, atr_full)

    # --- C1 + C2 tren 5 moc cat
    cuts = [int(n * q) for q in (0.2, 0.37, 0.5, 0.73, 0.9)]
    c1_fail, probe_moved = 0, 0
    for cut in cuts:
        T = int(st15[cut])  # cat o dau nen `cut` => moi nen `i` co time+900k <= T
        ct15 = st15[:cut]
        c15 = {k: v[:cut].copy() for k, v in s15.items()}
        mm = s1["time"] < T
        c1 = {k: v[mm].copy() for k, v in s1.items()}
        gc_ = grid_1m(c1, ct15)
        fc = intrabar_features(gc_, c15, probe=True, b1=c1, t15=ct15)
        for name in INTRABAR:
            a, b = f_full[name][:cut], fc[name]
            same = (np.isnan(a) & np.isnan(b)) | (a == b)
            bad = int((~same).sum())
            if bad:
                c1_fail += 1
                print(f"  C1 TRUOT: {name} doi {bad} gia tri khi cat o {iso(T)}")
        pa, pb = f_full["PROBE_peek_m16"][:cut], fc["PROBE_peek_m16"]
        psame = (np.isnan(pa) & np.isnan(pb)) | (pa == pb)
        if int((~psame).sum()) > 0:
            probe_moved += 1

    if c1_fail == 0:
        print(f"  C1 PASS: 8 dac trung x {len(cuts)} moc cat, 0 gia tri doi (so sanh bit, NaN==NaN)")
    else:
        ok_all = False
    if probe_moved == len(cuts):
        print(f"  C2 PASS: PROBE_peek_m16 TRUOT C1 o {probe_moved}/{len(cuts)} moc => test bat duoc nhin truoc")
    else:
        print(f"  C2 TRUOT: probe chi doi o {probe_moved}/{len(cuts)} moc => C1 khong chung minh gi")
        ok_all = False

    # --- C3: khong do qua khe
    gaps = np.flatnonzero((st15[1:] - st15[:-1]) != M15_MS)
    if gaps.size:
        bad = int(np.isfinite(y_full[gaps]).sum())
        print(f"  C3 {'PASS' if bad == 0 else 'TRUOT'}: {gaps.size} khe tren lat nay, y co so o {bad} trong so do")
        ok_all &= bad == 0
    else:
        print("  C3 KHONG DO DUOC tren lat nay: 0 khe")

    # --- nen thieu nen 1m => null, khong phai 0
    cnt = np.isfinite(g_full[3]).sum(axis=1)
    short = np.flatnonzero(cnt < SLOTS)
    if short.size:
        leak = int(np.isfinite(f_full["tpo_skew"][short]).sum())
        print(f"  NULL-KHONG-PHAI-0 {'PASS' if leak == 0 else 'TRUOT'}: {short.size} nen thieu, {leak} mang so")
        ok_all &= leak == 0
    else:
        print("  NULL-KHONG-PHAI-0 KHONG DO DUOC tren lat nay: 0 nen thieu")

    # --- doi chung phai doc duoc tren dung tap dong (khong tu dong rong)
    print(f"  mau lat: {int(np.isfinite(y_full).sum())}/{n} nen co y, "
          f"{int(np.isfinite(f_full['tpo_skew']).sum())}/{n} nen co hinh")
    return ok_all


# ---------------------------------------------------------------------- main


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", default="E:/rust/flowdesk/data")
    ap.add_argument("--symbol", default="XAUDUKA")
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--cache", default="", help="thu muc ghi lop dac trung ra dia")
    a = ap.parse_args()

    t_start = time.time()
    bars = Path(a.data) / "bars"
    p1, p15 = bars / f"{a.symbol}-1m.parquet", bars / f"{a.symbol}-15m.parquet"

    print("=" * 78)
    print(f"TIEN KIEM hinh-trong-ruot — {a.symbol}, nguon tin hieu 1m cho co che 15m")
    print("dang ky: docs/decisions/2026-10-10-intrabar-shape.md")
    print("=" * 78)

    t0 = time.time()
    b1 = read_bars(p1)
    t_read1 = time.time() - t0
    t0 = time.time()
    b15 = read_bars(p15)
    t_read15 = time.time() - t0
    t15 = b15["time"]
    print(f"\n1m : {b1['time'].shape[0]:,} nen  {iso(b1['time'][0])} -> {iso(b1['time'][-1])}  doc {t_read1:.2f}s")
    print(f"15m: {t15.shape[0]:,} nen  {iso(t15[0])} -> {iso(t15[-1])}  doc {t_read15:.2f}s")
    print("cot volume: KHONG MO (null 100% tren 1m; 15m mang mot 0.0 che ra)")

    if a.selftest:
        ok = selftest(b1, b15, t15)
        print(f"\nTEST NHAN QUA: {'PASS' if ok else 'TRUOT — F3 ban, moi so bo'}")
        if not ok:
            return 2

    t0 = time.time()
    g = grid_1m(b1, t15)
    t_grid = time.time() - t0
    cnt = np.isfinite(g[3]).sum(axis=1)
    print(f"\n=== PHU DU LIEU ===  (xep 1m vao o 15 phut: {t_grid:.2f}s)")
    for k in range(SLOTS + 1):
        c = int((cnt == k).sum())
        if c:
            print(f"  {k:2d}/15 nen 1m: {c:>7,} nen 15m  ({100*c/t15.shape[0]:.4f}%)")
    print(f"  du 15/15 : {int((cnt == SLOTS).sum()):,} ({100*(cnt==SLOTS).mean():.4f}%)")
    print(f"  < 15     : {int((cnt < SLOTS).sum()):,} ({100*(cnt<SLOTS).mean():.4f}%)  => DAC TRUNG NULL")

    t0 = time.time()
    atr = atr14_15m(b15)
    feat = intrabar_features(g, b15)
    ctrl = control_features(b15, atr)
    y = outcome(b15, t15, atr)
    t_feat = time.time() - t0
    print(f"\ndac trung + ket cuoc: {t_feat:.2f}s")

    allf = {**ctrl, **feat}
    if a.cache:
        d = Path(a.cache)
        d.mkdir(parents=True, exist_ok=True)
        np.savez_compressed(d / f"{a.symbol}-intrabar-shape.npz", time=t15, y=y, count_1m=cnt, **allf)
        sz = (d / f"{a.symbol}-intrabar-shape.npz").stat().st_size / 1048576.0
        print(f"lop dac trung ghi ra dia: {d / f'{a.symbol}-intrabar-shape.npz'}  {sz:.1f} MB")

    # ---------------- A/B: don bien, 3 buoc mau
    print("\n=== A + B. DON BIEN: t tren mau KHONG CHONG LAP, 3 buoc ===")
    print("ket cuoc cua tin hieu i la nen i+1, cua i+1 la nen i+2 => da roi nhau o buoc 1.")
    print(f"vach Bonferroni khai truoc cho 16 phep so chinh: |t| >= {T_GATE}")
    res: dict[tuple[str, str, int], tuple[float, float, int]] = {}
    for wname, (d0, d1) in WINDOWS.items():
        lo, hi = ms_of(d0), ms_of(d1)
        sel = (t15 >= lo) & (t15 < hi)
        ysel = y[sel]
        print(f"\n--- {wname}   {int(sel.sum()):,} nen 15m, {int(np.isfinite(ysel).sum()):,} co y")
        print(f"{'dac trung':<16}{'buoc1 t':>10}{'r':>10}{'n':>9}{'buoc2 t':>10}{'buoc4 t':>10}{'|t|1/|t|4':>11}")
        for name in INTRABAR:
            x = allf[name][sel]
            row = []
            for k in (1, 2, 4):
                t_, r_, n_ = tstat(x[::k], ysel[::k])
                res[(wname, name, k)] = (t_, r_, n_)
                row.append((t_, r_, n_))
            ratio = abs(row[0][0]) / abs(row[2][0]) if np.isfinite(row[2][0]) and row[2][0] != 0 else float("nan")
            print(f"{name:<16}{row[0][0]:>10.3f}{row[0][1]:>10.4f}{row[0][2]:>9,}"
                  f"{row[1][0]:>10.3f}{row[2][0]:>10.3f}{ratio:>11.2f}")
        print("  (doi chung don bien, cung mau:)")
        for name in CONTROL:
            x = allf[name][sel]
            t_, r_, n_ = tstat(x, ysel)
            res[(wname, name, 1)] = (t_, r_, n_)
            print(f"  {name:<14}{t_:>10.3f}{r_:>10.4f}{n_:>9,}")

    # ---------------- F1
    print("\n=== F1: dong dau hai cua so, |t| >= 2,95 ===")
    w1, w2 = list(WINDOWS.keys())
    f1_survivors = []
    for k in (1, 2, 4):
        hits = []
        for name in INTRABAR:
            t1 = res[(w1, name, k)][0]
            t2 = res[(w2, name, k)][0]
            if np.isfinite(t1) and np.isfinite(t2) and abs(t1) >= T_GATE and abs(t2) >= T_GATE and np.sign(t1) == np.sign(t2):
                hits.append((name, t1, t2))
        print(f"  buoc {k}: {len(hits)}/8 dac trung qua vach VA cung dau")
        for name, t1, t2 in hits:
            print(f"     {name}: t(W1)={t1:+.3f}  t(W2)={t2:+.3f}")
        if k == 1:
            f1_survivors = hits
    # lat dau = hien vat cua so thu 13 (F4)
    print("\n=== F4: dac trung lat dau giua hai cua so (hien vat cua so) ===")
    flipped = []
    for name in INTRABAR:
        t1, t2 = res[(w1, name, 1)][0], res[(w2, name, 1)][0]
        if np.isfinite(t1) and np.isfinite(t2) and np.sign(t1) != np.sign(t2):
            flipped.append((name, t1, t2))
            print(f"  {name}: t(W1)={t1:+.3f} -> t(W2)={t2:+.3f}  LAT DAU")
    if not flipped:
        print("  0/8 lat dau")

    # ---------------- F2: doi chung long nhau, NGOAI MAU
    print("\n=== F2 (DOI CHUNG BAT BUOC): cung mo hinh, cung mau, chi bo 8 dac trung 1m ===")
    print("doc NGOAI MAU — mo hinh nhieu dac trung hon luon khop tot hon TRONG mau.")
    f2_fired = False
    for wname, (d0, d1) in WINDOWS.items():
        lo, hi = ms_of(d0), ms_of(d1)
        sel = (t15 >= lo) & (t15 < hi)
        names = CONTROL + INTRABAR
        M = np.column_stack([allf[n][sel] for n in names])
        yy = y[sel]
        tt = t15[sel]
        rows = np.isfinite(M).all(axis=1) & np.isfinite(yy)
        M, yy, tt = M[rows], yy[rows], tt[rows]
        nc = len(CONTROL)
        half = M.shape[0] // 2
        print(f"\n--- {wname}: {M.shape[0]:,} dong co DU ca 14 dac trung + y (cung tap cho ca hai mo hinh)")
        for label, (tr, te) in (
            ("fit nua 1 -> doc nua 2", (slice(0, half), slice(half, None))),
            ("fit nua 2 -> doc nua 1", (slice(half, None), slice(0, half))),
        ):
            r2c, acc_c = oos(M[tr, :nc], yy[tr], M[te, :nc], yy[te])
            r2f, acc_f = oos(M[tr], yy[tr], M[te], yy[te])
            d_r2, d_acc = r2f - r2c, acc_f - acc_c
            fired = (d_r2 <= 0) or (d_acc <= 0)
            f2_fired |= fired
            print(f"  {label}  n_fit={M[tr].shape[0]:,} n_doc={M[te].shape[0]:,}")
            print(f"    doi chung (6 dac trung 15m) : R2_oos {r2c:+.6f}  dung dau {100*acc_c:.3f}%")
            print(f"    + 8 dac trung 1m            : R2_oos {r2f:+.6f}  dung dau {100*acc_f:.3f}%")
            print(f"    DELTA                       : R2 {d_r2:+.6f}      dau {100*d_acc:+.3f} diem%"
                  f"   => F2 {'BAN' if fired else 'khong ban'}")

    # ---------------- D/E/F: ghi chu them 2026-10-10
    print("\n=== D. BAO NHIEU PHAN CUA 'HINH TRONG RUOT' DA NAM TRONG O/H/L/C 15m ===")
    print("R2 cua moi dac trung 1m hoi quy len 6 dac trung doi chung. 0 phep so, mot ti le.")
    resid: dict[tuple[str, str], np.ndarray] = {}
    resid_y: dict[str, np.ndarray] = {}
    for wname, (d0, d1) in WINDOWS.items():
        lo, hi = ms_of(d0), ms_of(d1)
        sel = (t15 >= lo) & (t15 < hi)
        names = CONTROL + INTRABAR
        M = np.column_stack([allf[n][sel] for n in names])
        yy = y[sel]
        rows = np.isfinite(M).all(axis=1) & np.isfinite(yy)
        M, yy = M[rows], yy[rows]
        nc = len(CONTROL)
        Xc = M[:, :nc]
        resid_y[wname] = yy
        print(f"\n--- {wname}  n = {M.shape[0]:,}")
        for k, name in enumerate(INTRABAR):
            xf = M[:, nc + k]
            b = ols_fit(Xc, xf)
            pr = ols_pred(b, Xc)
            e = xf - pr
            sst = float(np.sum((xf - xf.mean()) ** 2))
            r2 = 1.0 - float(np.sum(e**2)) / sst if sst > 0 else float("nan")
            resid[(wname, name)] = e
            print(f"  {name:<16} R2(1m ~ 15m) = {r2:.4f}   => phan MOI = {100*(1-r2):.2f}% phuong sai")

    print("\n=== E. t CUA PHAN DU (dang don bien cua FALSIFIER) ===")
    print(f"da tru phan 6 dac trung 15m giai thich duoc. vach {T_GATE}.")
    e_surv = {1: [], 4: []}
    for kstep in (1, 4):
        print(f"\n  buoc mau {kstep}:")
        print(f"  {'dac trung':<16}{'t(W1)':>10}{'t(W2)':>10}{'n':>10}  cung dau & qua vach?")
        for name in INTRABAR:
            ts = []
            for wname in WINDOWS:
                t_, _, n_ = tstat(resid[(wname, name)][::kstep], resid_y[wname][::kstep])
                ts.append((t_, n_))
            pas = (
                np.isfinite(ts[0][0])
                and np.isfinite(ts[1][0])
                and abs(ts[0][0]) >= T_GATE
                and abs(ts[1][0]) >= T_GATE
                and np.sign(ts[0][0]) == np.sign(ts[1][0])
            )
            if pas:
                e_surv[kstep].append(name)
            print(f"  {name:<16}{ts[0][0]:>10.3f}{ts[1][0]:>10.3f}{ts[1][1]:>10,}  {'CO' if pas else 'khong'}")
        print(f"  => {len(e_surv[kstep])}/8 qua truc E o buoc {kstep}")

    print("\n=== F. tuong quan tung cap: 4 dac trung song sot F1 vs 2 doi chung manh nhat ===")
    for wname, (d0, d1) in WINDOWS.items():
        lo, hi = ms_of(d0), ms_of(d1)
        sel = (t15 >= lo) & (t15 < hi)
        print(f"  {wname}")
        for name in ["t_high", "extreme_order", "path_eff", "late_push"]:
            line = []
            for c in ("ret_15", "close_pos"):
                xa, xb = allf[name][sel], allf[c][sel]
                m = np.isfinite(xa) & np.isfinite(xb)
                line.append(f"{c}={np.corrcoef(xa[m], xb[m])[0,1]:+.4f}")
            print(f"    {name:<16}" + "  ".join(line))

    # ---------------- G: doi ket cuoc sang BIEN DO (ghi chu them thu hai)
    print("\n=== G. CUNG PHEP SO, KET CUOC = BIEN DO nen sau / ATR14(i) ===")
    print("KHONG phai chan cong (cong do CHIEU). Chi de chon cau ket luan dung.")
    y_rng = outcome(b15, t15, atr, kind="rng")
    g_surv = []
    for wname, (d0, d1) in WINDOWS.items():
        lo, hi = ms_of(d0), ms_of(d1)
        sel = (t15 >= lo) & (t15 < hi)
        names = CONTROL + INTRABAR
        M = np.column_stack([allf[n][sel] for n in names])
        yy = y_rng[sel]
        rows = np.isfinite(M).all(axis=1) & np.isfinite(yy)
        M, yy = M[rows], yy[rows]
        nc = len(CONTROL)
        half = M.shape[0] // 2
        print(f"\n--- {wname}: {M.shape[0]:,} dong")
        for label, (tr, te) in (
            ("fit nua 1 -> doc nua 2", (slice(0, half), slice(half, None))),
            ("fit nua 2 -> doc nua 1", (slice(half, None), slice(0, half))),
        ):
            r2c, _ = oos(M[tr, :nc], yy[tr], M[te, :nc], yy[te])
            r2f, _ = oos(M[tr], yy[tr], M[te], yy[te])
            print(f"  {label}: doi chung R2_oos {r2c:+.6f} -> +1m {r2f:+.6f}"
                  f"   DELTA {r2f-r2c:+.6f}  {'1m THEM DUOC' if r2f-r2c > 0 else 'khong them'}")
        Xc = M[:, :nc]
        print("  t cua phan du (da tru 6 dac trung 15m):")
        for k, name in enumerate(INTRABAR):
            xf = M[:, nc + k]
            e = xf - ols_pred(ols_fit(Xc, xf), Xc)
            t_, _, _ = tstat(e, yy)
            flag = ""
            if abs(t_) >= T_GATE:
                g_surv.append((wname, name, t_))
                flag = "  <- qua vach"
            print(f"    {name:<16}{t_:>10.3f}{flag}")

    print("\n" + "=" * 78)
    print(f"F1 {'KHONG BAN' if f1_survivors else 'BAN'}: {len(f1_survivors)}/8 dac trung dong dau qua vach o buoc 1")
    print(f"   (buoc 4: 0/8 — xem bang tren)  truc E: {len(e_surv[1])}/8 o buoc 1, {len(e_surv[4])}/8 o buoc 4")
    print(f"   truc G (bien do): {len(g_surv)}/16 phan du qua vach")
    print(f"F2 {'BAN' if f2_fired else 'KHONG BAN'}")
    print(f"=> O CONG TIEU: {0 if (not f1_survivors or f2_fired) else 'do cong'}")
    print("=" * 78)
    print(f"\nCHI PHI I/O: tong {time.time()-t_start:.1f}s tuong, dinh bo nho {peak_mem_mb():.0f} MB")
    print(f"  doc 1m {t_read1:.2f}s · doc 15m {t_read15:.2f}s · xep luoi {t_grid:.2f}s · dac trung {t_feat:.2f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
