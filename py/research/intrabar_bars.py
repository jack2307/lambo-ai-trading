"""Shared loader for the intrabar resolution map.

Reads XAUDUKA bars once and caches plain float64/int64 arrays, because the
parquet `time` column is a timestamp and converting 5.6 M of them through
Python objects takes minutes. Read-only against /e/rust/flowdesk/data.
"""
import os
import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

DATA = "E:/rust/flowdesk/data/bars"
CACHE = os.environ.get(
    "INTRABAR_CACHE",
    "C:/Users/Admin/AppData/Local/Temp/claude/e--nodejs-backcom-vantage/"
    "a5c7fd65-9276-457f-850d-25cb44e74c74/scratchpad",
)


def load(symbol_tf):
    """(time_ms int64, open, high, low, close) as numpy arrays."""
    os.makedirs(CACHE, exist_ok=True)
    npz = os.path.join(CACHE, f"intrabar-{symbol_tf}.npz")
    if os.path.exists(npz):
        d = np.load(npz)
        return d["t"], d["o"], d["h"], d["l"], d["c"]
    tbl = pq.read_table(f"{DATA}/{symbol_tf}.parquet",
                        columns=["time", "open", "high", "low", "close"])
    t = tbl.column("time").cast(pa.int64()).to_numpy(zero_copy_only=False).astype(np.int64)
    o = tbl.column("open").to_numpy(zero_copy_only=False).astype(np.float64)
    h = tbl.column("high").to_numpy(zero_copy_only=False).astype(np.float64)
    l = tbl.column("low").to_numpy(zero_copy_only=False).astype(np.float64)
    c = tbl.column("close").to_numpy(zero_copy_only=False).astype(np.float64)
    np.savez(npz, t=t, o=o, h=h, l=l, c=c)
    return t, o, h, l, c


def atr_wilder(h, l, c, period=14):
    """fd_indicators::atr = rma(true_range, period): Wilder, seeded on the
    mean of the first `period` true ranges, NaN before that."""
    n = len(c)
    tr = np.empty(n, dtype=np.float64)
    tr[0] = h[0] - l[0]
    pc = c[:-1]
    tr[1:] = np.maximum(h[1:] - l[1:], np.maximum(np.abs(h[1:] - pc), np.abs(l[1:] - pc)))
    out = np.full(n, np.nan)
    prev = tr[:period].mean()
    out[period - 1] = prev
    k = (period - 1.0) / period
    for i in range(period, n):
        prev = prev * k + tr[i] / period
        out[i] = prev
    return out
