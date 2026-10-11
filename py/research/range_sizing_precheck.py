"""Tien kiem cho `agent/range-sizing`: bien do DU BAO cua nen toi lam CO STOP.

Dang ky: docs/decisions/2026-10-11-range-sizing.md

`agent/intrabar-shape` do duoc rang hinh trong ruot nen 15m — dung THUAN tu
O/H/L/C cua 15 nen 1m — khong noi gi ve CHIEU cua nen sau (|t| < 1) nhung noi
rat to ve BIEN DO cua no (t phan du +28,303 / +38,060 tren `max_1m_share`).
Dung cu nay bien con so do thanh mot thu co the TIEU TIEN: mot chuoi `m(i)`,
co so nhan stop, ghi ra parquet companion de `search --companion=` nap vao
`Exits::Engine`.

Bon luat, moi cai la mot cho job nay co the chet:

1. KHONG DOC COT VOLUME. `XAUDUKA-1m.parquet` null tren ca 5.635.777 dong; ban
   15m mang mot so 0 che ra. Moi lan doc parquet o day truyen
   `columns=["time","open","high","low","close"]`.
2. NHAN QUA BANG CAU TRUC, khong bang loi. OLS khop DI BO TIEN o moc dau moi
   thang, va mot dong chi vao tap khop khi KET CUOC cua no da quan sat xong
   truoc moc do (`t15[i] + 2 x 900_000 <= moc`). `--selftest` cat chuoi o 5
   moc va doi moi gia tri truoc moc bang TUNG BIT, roi doi BA probe co tinh
   nhin truoc PHAI TRUOT dung test do — mot test luon pass la mot test vo
   gia tri.
3. NEN 15m KHONG DU 15 NEN 1m => NULL, khong dien, khong ve 0. Ca bon nhan
   chay tren DUNG CUNG TAP NEN vi `range-stop` tu choi nen `cmp.close` NaN o
   CA HAI nguon stop.
4. CO STOP TRUNG BINH PHAI KHOP truoc khi so PF — `agent/stop-width` da do lop
   "stop rong hon thi PF doi" tren 200 o, va mot dai luong do lai lop do thi
   khong phai mot phep do moi.

Hang so cua dung cu, khai truoc khi chay (xem ghi chu ngay 2026-10-11 o cuoi
file dang ky):

    REFIT       moc dau moi thang lich (UTC)
    MIN_TRAIN   20.000 dong truoc du bao dau tien
    NORM_WIN    2.000 du bao huu han gan nhat, TINH CA i
    NORM_MIN      200 du bao trong cua so, duoi do => NaN
    CLAMP       [0,50 ; 2,00]
    SEED        20261011 (xao tron trong thoi gian)
    DELTA_USD   0,138 = dung nua spread 0,28 vong (khuyet diem 16)
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
    SLOTS,
    WINDOWS,
    atr14_15m,
    control_features,
    grid_1m,
    intrabar_features,
    iso,
    ms_of,
    ols_fit,
    ols_pred,
    outcome,
    peak_mem_mb,
    read_bars,
)

MIN_TRAIN = 20_000
NORM_WIN = 2_000
NORM_MIN = 200
CLAMP_LO, CLAMP_HI = 0.50, 2.00
SEED = 20_261_011
DELTA_USD = 0.138  # nua spread 0,28 vong, khuyet diem 16
DONCHIAN = 20
ATR_PERIOD = 14
STOP_ATRS = (1.0, 2.0)

SOURCES = ("fc15", "fc1m", "shuf")
COMPANION = {"fc15": "XAURFC15", "fc1m": "XAURFC1M", "shuf": "XAURFCSH"}


# ------------------------------------------------------------------ dac trung


def design(b1: dict, b15: dict, t15: np.ndarray, probe_m16: bool = False) -> dict:
    """14 dac trung + y(bien do) + ATR, tren cung mot luoi nen 15m."""
    atr = atr14_15m(b15, ATR_PERIOD)
    g = grid_1m(b1, t15)
    f1 = intrabar_features(g, b15, probe=probe_m16, b1=b1, t15=t15)
    f15 = control_features(b15, atr)
    y = outcome(b15, t15, atr, kind="rng")
    out = {"atr": atr, "y": y}
    out.update({k: f15[k] for k in CONTROL})
    out.update({k: f1[k] for k in INTRABAR})
    if probe_m16:
        out["PROBE_peek_m16"] = f1["PROBE_peek_m16"]
    return out


def stack(d: dict, names: list[str]) -> np.ndarray:
    return np.column_stack([d[n] for n in names])


# ------------------------------------------------------- khop di bo tien, OLS


def month_starts(t15: np.ndarray) -> np.ndarray:
    """Moc dau moi thang lich UTC nam trong [t15[0], t15[-1]]."""
    days = t15.astype("datetime64[ms]").astype("datetime64[M]")
    uniq = np.unique(days)
    return uniq.astype("datetime64[ms]").astype("int64")


def walkforward(
    X: np.ndarray, y: np.ndarray, t15: np.ndarray, min_train: int = MIN_TRAIN, full_fit_probe: bool = False
) -> tuple[np.ndarray, int]:
    """Du bao NGOAI MAU. Tra ve (pred, so lan khop lai).

    Mot dong vao tap khop o moc `B` chi khi ket cuoc cua no da quan sat xong
    truoc `B`: `y(i)` doc nen `i+1`, dong luc `t15[i] + 2 x 15m`. Dieu khoan
    nay la cai lam C1 doc duoc — khong co no, mot dong ngay truoc `B` dong
    gop vao `beta` bang mot con so chua ton tai luc do, va cat chuoi o `B` se
    doi `beta`.

    `full_fit_probe=True` la PROBE: khop MOT lan tren CA chuoi, tuc tang khop
    co nhin truoc. Nó PHAI truot C1.
    """
    n = X.shape[0]
    pred = np.full(n, np.nan)
    rows_ok = np.isfinite(X).all(axis=1)
    fit_ok = rows_ok & np.isfinite(y)
    known_at = t15 + 2 * M15_MS

    if full_fit_probe:
        i = np.flatnonzero(fit_ok)
        if i.size < min_train:
            return pred, 0
        beta = ols_fit(X[i], y[i])
        j = np.flatnonzero(rows_ok)
        pred[j] = ols_pred(beta, X[j])
        return pred, 1

    bounds = month_starts(t15)
    refits = 0
    for k, b in enumerate(bounds):
        hi = bounds[k + 1] if k + 1 < bounds.size else t15[-1] + M15_MS
        tr = np.flatnonzero(fit_ok & (known_at <= b))
        if tr.size < min_train:
            continue
        beta = ols_fit(X[tr], y[tr])
        refits += 1
        te = np.flatnonzero(rows_ok & (t15 >= b) & (t15 < hi))
        if te.size:
            pred[te] = ols_pred(beta, X[te])
    return pred, refits


def multiplier(pred: np.ndarray, win: int = NORM_WIN, need: int = NORM_MIN) -> np.ndarray:
    """`m(i) = pred(i) / trung binh truot cua `win` du bao huu han gan nhat`.

    Tinh CA i, nen nhan qua. Duoi `need` du bao trong cua so => NaN (khong
    phai 1,0: "khong do duoc" khong bao gio in thanh mot gia tri trung tinh).
    Kep [CLAMP_LO, CLAMP_HI].
    """
    n = pred.shape[0]
    out = np.full(n, np.nan)
    idx = np.flatnonzero(np.isfinite(pred))
    if idx.size == 0:
        return out
    v = pred[idx]
    cs = np.concatenate([[0.0], np.cumsum(v)])
    k = np.arange(1, idx.size + 1)
    lo = np.maximum(0, k - win)
    cnt = k - lo
    mean = (cs[k] - cs[lo]) / cnt
    with np.errstate(invalid="ignore", divide="ignore"):
        r = np.where((mean > 0) & (cnt >= need), v / mean, np.nan)
    out[idx] = np.clip(r, CLAMP_LO, CLAMP_HI)
    return out


def shuffled(m: np.ndarray, seed: int = SEED) -> np.ndarray:
    """Hoan vi cac gia tri huu han TRONG THOI GIAN, giu nguyen vi tri NaN.

    => cung tap nen, cung phan phoi co stop, mat het thong tin thoi diem.
    """
    out = m.copy()
    i = np.flatnonzero(np.isfinite(m))
    if i.size:
        out[i] = np.random.default_rng(seed).permutation(m[i])
    return out


# --------------------------------------------------------------- luat vao lenh


def signal_bars(b15: dict, period: int = DONCHIAN) -> np.ndarray:
    """Dung lai luat vao lenh cua `range-stop` de do co stop tren DUNG nen do.

    `fd_indicators::donchian` lay cua so `bars[i-period..i]` — nen HIEN TAI bi
    LOAI. Khong co nen nao sau `i` duoc doc.
    """
    h, l, c = b15["high"], b15["low"], b15["close"]
    n = c.shape[0]
    sig = np.zeros(n, dtype=bool)
    if n <= period:
        return sig
    win_h = np.lib.stride_tricks.sliding_window_view(h, period)[:-1]
    win_l = np.lib.stride_tricks.sliding_window_view(l, period)[:-1]
    upper = win_h.max(axis=1)
    lower = win_l.min(axis=1)
    cc = c[period:]
    sig[period:] = (cc > upper) | (cc < lower)
    return sig


# ------------------------------------------------------------------- selftest


def selftest(b1: dict, b15: dict, t15: np.ndarray) -> bool:
    """C1 bit-equality · C2 BA probe phai truot · C3 khe · null != 0."""
    print("\n=== TEST NHAN QUA (dang ky muc 2) ===")
    ok_all = True
    n = 6000
    min_train = 600

    s15 = {k: v[:n].copy() for k, v in b15.items()}
    st15 = t15[:n].copy()
    m1 = b1["time"] < st15[-1] + M15_MS
    s1 = {k: v[m1].copy() for k, v in b1.items()}

    def chain(bb1, bb15, tt15, probe: str | None):
        d = design(bb1, bb15, tt15, probe_m16=(probe == "m16"))
        names = CONTROL + INTRABAR
        if probe == "m16":
            names = names + ["PROBE_peek_m16"]
        if probe == "y":
            names = names + ["y"]
        X = stack(d, names)
        pred, _ = walkforward(X, d["y"], tt15, min_train=min_train, full_fit_probe=(probe == "fullfit"))
        return d, X, pred, multiplier(pred), names

    full = {p: chain(s1, s15, st15, p) for p in (None, "m16", "y", "fullfit")}

    cuts = [int(n * q) for q in (0.35, 0.47, 0.6, 0.73, 0.88)]
    c1_bad = 0
    probe_moved = {p: 0 for p in ("m16", "y", "fullfit")}
    for cut in cuts:
        T = int(st15[cut])
        ct15 = st15[:cut]
        c15 = {k: v[:cut].copy() for k, v in s15.items()}
        c1 = {k: v[s1["time"] < T].copy() for k, v in s1.items()}
        cut_chain = {p: chain(c1, c15, ct15, p) for p in (None, "m16", "y", "fullfit")}

        d_f, X_f, p_f, m_f, names = full[None]
        d_c, X_c, p_c, m_c, _ = cut_chain[None]
        for j, name in enumerate(names):
            a, b = X_f[:cut, j], X_c[:, j]
            bad = int((~((np.isnan(a) & np.isnan(b)) | (a == b))).sum())
            if bad:
                c1_bad += 1
                print(f"  C1 TRUOT: dac trung {name} doi {bad} gia tri khi cat o {iso(T)}")
        for lab, a, b in (("pred", p_f[:cut], p_c), ("m", m_f[:cut], m_c)):
            bad = int((~((np.isnan(a) & np.isnan(b)) | (a == b))).sum())
            if bad:
                c1_bad += 1
                print(f"  C1 TRUOT: {lab} doi {bad} gia tri khi cat o {iso(T)}")

        for p in ("m16", "y", "fullfit"):
            a, b = full[p][2][:cut], cut_chain[p][2]
            moved = int((~((np.isnan(a) & np.isnan(b)) | (a == b))).sum())
            if moved:
                probe_moved[p] += 1

    if c1_bad == 0:
        print(f"  C1 PASS: {len(CONTROL + INTRABAR)} dac trung + pred + m, x {len(cuts)} moc cat,")
        print("           0 gia tri doi (so sanh TUNG BIT, NaN==NaN)")
    else:
        ok_all = False
    for p, why in (
        ("m16", "dac trung doc close PHUT 16 (phut dau cua nen i+1)"),
        ("y", "y(i) lam BIEN HOI QUY (tang du bao doc ket cuoc)"),
        ("fullfit", "khop MOT lan tren CA chuoi (tang khop nhin truoc)"),
    ):
        hit = probe_moved[p]
        tag = "PASS" if hit == len(cuts) else "TRUOT"
        print(f"  C2 {tag}: PROBE_{p} truot C1 o {hit}/{len(cuts)} moc — {why}")
        ok_all &= hit == len(cuts)

    d_f = full[None][0]
    gaps = np.flatnonzero((st15[1:] - st15[:-1]) != M15_MS)
    if gaps.size:
        bad = int(np.isfinite(d_f["y"][gaps]).sum())
        print(f"  C3 {'PASS' if bad == 0 else 'TRUOT'}: {gaps.size} khe tren lat nay, y co so o {bad}")
        ok_all &= bad == 0
    else:
        print("  C3 KHONG DO DUOC tren lat nay: 0 khe")

    cnt = np.isfinite(grid_1m(s1, st15)[3]).sum(axis=1)
    short = np.flatnonzero(cnt < SLOTS)
    if short.size:
        leak = int(np.isfinite(full[None][3][short]).sum())
        print(f"  NULL-KHONG-PHAI-0 {'PASS' if leak == 0 else 'TRUOT'}: {short.size} nen thieu nen 1m, {leak} mang mot so m")
        ok_all &= leak == 0
    else:
        print("  NULL-KHONG-PHAI-0 KHONG DO DUOC tren lat nay: 0 nen thieu")

    sh = shuffled(full[None][3])
    same_null = bool(np.array_equal(np.isnan(sh), np.isnan(full[None][3])))
    kept = np.allclose(np.sort(sh[np.isfinite(sh)]), np.sort(full[None][3][np.isfinite(full[None][3])]))
    print(f"  XAO-TRON {'PASS' if (same_null and kept) else 'TRUOT'}: vi tri NaN y het ({same_null}), phan phoi y het ({kept})")
    ok_all &= same_null and kept
    return ok_all


# ------------------------------------------------------------------ bao cao


def pct(x: np.ndarray, q: float) -> float:
    v = x[np.isfinite(x)]
    return float(np.percentile(v, q)) if v.size else float("nan")


def r2_corr(pred: np.ndarray, y: np.ndarray) -> tuple[float, float, int]:
    m = np.isfinite(pred) & np.isfinite(y)
    k = int(m.sum())
    if k < 100:
        return float("nan"), float("nan"), k
    p, t = pred[m], y[m]
    sse = float(np.sum((t - p) ** 2))
    sst = float(np.sum((t - np.mean(t)) ** 2))
    return (1.0 - sse / sst if sst > 0 else float("nan")), float(np.corrcoef(p, t)[0, 1]), k


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", default="E:/rust/flowdesk/data")
    ap.add_argument("--symbol", default="XAUDUKA")
    ap.add_argument("--out", default="data-rs/bars")
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--write", action="store_true")
    a = ap.parse_args()

    t0 = time.perf_counter()
    bars = Path(a.data) / "bars"
    p1, p15 = bars / f"{a.symbol}-1m.parquet", bars / f"{a.symbol}-15m.parquet"
    print("=== KHO ===")
    for p in (p1, p15):
        f = pq.ParquetFile(p)
        vol = f.metadata.num_rows
        print(f"  {p.name}: {vol} dong — doc 5 cot, `volume` KHONG trong `columns=`")
    b1 = read_bars(p1)
    b15 = read_bars(p15)
    t15 = b15["time"]
    print(f"  1m  {b1['time'].shape[0]} nen {iso(int(b1['time'][0]))} -> {iso(int(b1['time'][-1]))}")
    print(f"  15m {t15.shape[0]} nen {iso(int(t15[0]))} -> {iso(int(t15[-1]))}")
    print(f"  doc xong sau {time.perf_counter() - t0:.2f} s")

    if a.selftest and not selftest(b1, b15, t15):
        print("\nSELFTEST TRUOT => F4 BAN. Khong cong bo con so nao, tieu 0 o cong.")
        return 1

    print("\n=== DUNG DAC TRUNG + DU BAO (toan chuoi, 1 lan) ===")
    d = design(b1, b15, t15)
    X15 = stack(d, CONTROL)
    X1m = stack(d, CONTROL + INTRABAR)
    pred15, n15fit = walkforward(X15, d["y"], t15)
    pred1m, n1mfit = walkforward(X1m, d["y"], t15)
    print(f"  khop lai: fc15 {n15fit} lan, fc1m {n1mfit} lan (moc dau thang, tap khop EXPANDING)")
    m = {"fc15": multiplier(pred15), "fc1m": multiplier(pred1m)}
    m["shuf"] = shuffled(m["fc1m"])
    sig = signal_bars(b15)
    print(f"  nen tin hieu (donchian {DONCHIAN}, nen hien tai LOAI): {int(sig.sum())}")

    print("\n=== DOI CHUNG DUONG 1: du bao co doc duoc khong (ngoai mau) ===")
    print("  cua so            nguon   R2_oos      corr      n")
    best = {}
    for wl, (fr, to) in WINDOWS.items():
        w = (t15 >= ms_of(fr)) & (t15 < ms_of(to))
        for nm, pr in (("fc15", pred15), ("fc1m", pred1m)):
            r2, c, k = r2_corr(np.where(w, pr, np.nan), d["y"])
            best[(wl, nm)] = r2
            print(f"  {wl[:2]} {fr}  {nm:5s}  {r2:+.6f}  {c:+.4f}  {k}")
        dd = best[(wl, "fc1m")] - best[(wl, "fc15")]
        print(f"     => DELTA R2_oos (1m THEM vao) = {dd:+.6f}")

    print("\n=== PHAN PHOI CO SO NHAN STOP m ===")
    print("  cua so  nguon   p10     p50     p90    mean   p90/p10   n")
    for wl, (fr, to) in WINDOWS.items():
        w = (t15 >= ms_of(fr)) & (t15 < ms_of(to))
        for nm in SOURCES:
            v = np.where(w, m[nm], np.nan)
            vv = v[np.isfinite(v)]
            lo, hi = pct(v, 10), pct(v, 90)
            print(
                f"  {wl[:2]}      {nm:5s} {lo:6.4f} {pct(v, 50):6.4f} {hi:6.4f} "
                f"{float(np.mean(vv)) if vv.size else float('nan'):6.4f}  {hi / lo if lo > 0 else float('nan'):6.4f}  {vv.size}"
            )
        a1 = np.where(w, m["fc1m"], np.nan)
        a2 = np.where(w, m["fc15"], np.nan)
        k = np.isfinite(a1) & np.isfinite(a2)
        cc = float(np.corrcoef(a1[k], a2[k])[0, 1]) if int(k.sum()) > 100 else float("nan")
        print(f"     => corr(m_fc1m, m_fc15) = {cc:+.6f}   (F5b: >= 0,995 thi hai nguon la CUNG mot stop)")

    print("\n=== DOI CHUNG (a): CO STOP TRUNG BINH tren DUNG nen tin hieu ===")
    print("  d(i) = stopAtr x ATR14(i) x m(i), USD; `atr` la m = 1,000")
    for wl, (fr, to) in WINDOWS.items():
        w = (t15 >= ms_of(fr)) & (t15 < ms_of(to)) & sig
        base = np.where(w & np.isfinite(m["fc1m"]), d["atr"], np.nan)
        for f in STOP_ATRS:
            row = [f"  {wl[:2]} stopAtr {f:.1f}:"]
            ref = float(np.nanmean(f * base))
            row.append(f"atr {ref:7.4f}")
            for nm in SOURCES:
                v = f * base * np.where(np.isfinite(m[nm]), m[nm], np.nan)
                mu = float(np.nanmean(v))
                row.append(f"{nm} {mu:7.4f} ({100.0 * (mu / ref - 1.0):+.2f}%)")
            print("   ".join(row))
            dv = f * base * np.where(np.isfinite(m["fc1m"]), m["fc1m"], np.nan)
            print(
                f"       khuyet diem 16: delta {DELTA_USD} USD = {100.0 * DELTA_USD / pct(dv, 10):.2f}% cua R o p10(d), "
                f"{100.0 * DELTA_USD / pct(dv, 90):.2f}% o p90(d)"
            )
            print(
                f"       chi phi/R (spread 0,28 cau hinh, KHONG do): {100.0 * 0.28 / pct(dv, 50):.2f}% cua R o p50(d) "
                f"= {pct(dv, 50):.3f} USD"
            )

    if a.write:
        out = Path(a.out)
        out.mkdir(parents=True, exist_ok=True)
        print("\n=== GHI PARQUET COMPANION ===")
        for nm in SOURCES:
            v = m[nm]
            tb = pa.table(
                {
                    "time": pa.array(t15, type=pa.timestamp("ms", tz="UTC")),
                    "open": pa.array(v, type=pa.float64()),
                    "high": pa.array(v, type=pa.float64()),
                    "low": pa.array(v, type=pa.float64()),
                    "close": pa.array(v, type=pa.float64()),
                    "volume": pa.array(np.full(v.shape[0], None, dtype=object), type=pa.float64()),
                }
            )
            path = out / f"{COMPANION[nm]}-15m.parquet"
            pq.write_table(tb, path)
            print(f"  {path} — {v.shape[0]} dong, {int(np.isfinite(v).sum())} co so, volume NULL het")
        print(f"  (nen 15m co m: {int(np.isfinite(m['fc1m']).sum())} / {t15.shape[0]} = "
              f"{100.0 * np.isfinite(m['fc1m']).sum() / t15.shape[0]:.4f}%)")

    print(f"\ntong {time.perf_counter() - t0:.1f} s · dinh bo nho {peak_mem_mb():.0f} MB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
