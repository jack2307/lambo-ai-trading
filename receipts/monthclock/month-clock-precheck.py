"""agent/month-clock TIEN KIEM: bien do + dich chuyen co dau cua nen 15m
theo NGAY TRONG THANG, tren XAUDUKA-15m, hai cua so, chuan hoa bien do ngay NY.

Khong doc volume (feed duka: volume = 0 tren 0/50.000 mau, phu luc 8 muc I).
Cai lai luat `average_day_range` cua engine (tsmom.rs:137) ke ca luat ngay-ngan.
"""
import numpy as np
import pandas as pd
from scipy import stats

DAY_MS = 86_400_000
HOUR_MS = 3_600_000
PARQUET = "E:/rust/flowdesk/data/bars/XAUDUKA-15m.parquet"
RANGE_DAYS = 20


# ---- clock.rs: luat DST My tu 2007 ----------------------------------------
def days_from_civil(y, m, d):
    y = np.where(m <= 2, y - 1, y)
    era = np.floor_divide(y, 400)
    yoe = y - era * 400
    mp = np.where(m > 2, m - 3, m + 9)
    doy = (153 * mp + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    return era * 146_097 + doe - 719_468


def nth_sunday(year, month, n):
    first = days_from_civil(np.asarray(year), np.asarray(month), np.asarray(1))
    to_sunday = (7 - ((first + 4) % 7)) % 7
    return first + to_sunday + 7 * (n - 1)


def ny_offset_ms(utc_ms):
    days = np.floor_divide(utc_ms, DAY_MS)
    # civil year of the UTC day
    yr = pd.to_datetime(days * DAY_MS, unit="ms", utc=True).year.to_numpy()
    start = nth_sunday(yr, 3, 2) * DAY_MS + 7 * HOUR_MS
    end = nth_sunday(yr, 11, 1) * DAY_MS + 6 * HOUR_MS
    dst = (utc_ms >= start) & (utc_ms < end)
    return np.where(dst, -4 * HOUR_MS, -5 * HOUR_MS)


# ---- ngay NY -> tong hop ngay ----------------------------------------------
df = pd.read_parquet(PARQUET)
utc = df["time"].astype("datetime64[ms, UTC]").astype("int64").to_numpy()  # ms
off = ny_offset_ms(utc)
local = utc + off
ny_day = np.floor_divide(local, DAY_MS)
ny_min = np.floor_divide(np.mod(local, DAY_MS), 60_000)

df = df.assign(ny_day=ny_day, ny_min=ny_min, rng=df["high"] - df["low"])
g = df.groupby("ny_day", sort=True)
day = pd.DataFrame({
    "amp": g["rng"].mean(),            # bien do trung binh moi nen 15m, diem gia
    "hi": g["high"].max(),
    "lo": g["low"].min(),
    "first_open": g["open"].first(),
    "last_close": g["close"].last(),
    "n_bars": g["close"].size(),
    "span_min": g["ny_min"].max() - g["ny_min"].min(),
})
day["disp"] = day["last_close"] - day["first_open"]   # dich chuyen CO DAU
day["day_range"] = day["hi"] - day["lo"]
day = day.reset_index()
cal = pd.to_datetime(day["ny_day"] * DAY_MS, unit="ms", utc=True)
day["year"] = cal.dt.year
day["month"] = cal.dt.month
day["dom"] = cal.dt.day
day["weekday"] = cal.dt.dayofweek  # 0=Mon
day["ym"] = day["year"] * 12 + day["month"]

print(f"ngay NY co nen: {len(day)}  tu {cal.min().date()} den {cal.max().date()}")

# ---- luat ngay-ngan cua engine: span*2 >= span dai nhat --------------------
longest = int(day["span_min"].max())
day["full"] = day["span_min"] * 2 >= longest
print(f"span dai nhat = {longest} phut; ngay DAT luat full: {int(day['full'].sum())}"
      f" / {len(day)}  (loai {int((~day['full']).sum())})")
print("  phan bo ngay bi loai theo weekday (0=Mon):",
      day.loc[~day["full"], "weekday"].value_counts().sort_index().to_dict())

# ---- base(d): trung binh bien do 20 ngay FULL da hoan tat TRUOC d ----------
# engine: lay want = 26 ngay moi nhat, loc theo span, take(20)
want = RANGE_DAYS + RANGE_DAYS // 5 + 2
rngs = day["day_range"].to_numpy()
spans = day["span_min"].to_numpy()
base = np.full(len(day), np.nan)
for i in range(len(day)):
    lo = max(0, i - want)
    w_r = rngs[lo:i][::-1]
    w_s = spans[lo:i][::-1]
    if len(w_r) == 0:
        continue
    lg = w_s.max()
    keep = w_r[w_s * 2 >= lg][:RANGE_DAYS]
    if len(keep) < min(RANGE_DAYS, 5):
        continue
    m = keep.mean()
    if m > 0:
        base[i] = m
day["base"] = base

# ---- chuan hoa -------------------------------------------------------------
day["Amp"] = day["amp"] / day["base"]
day["Disp"] = day["disp"] / day["base"]

# ---- thuoc B: ngay giao dich thu N tu CUOI thang (chi ngay FULL) -----------
day["tdfe"] = np.nan
obs = day[day["full"] & day["base"].notna()].copy()
for ym, idx in obs.groupby("ym").groups.items():
    pos = list(idx)
    for k, j in enumerate(reversed(pos), start=1):
        obs.loc[j, "tdfe"] = k
obs["tdfe"] = obs["tdfe"].astype(int)

# ---- hai cua so ------------------------------------------------------------
W1 = (2010 * 12 + 6, 2018 * 12 + 6)   # [W1a, W1b)
W2 = (2018 * 12 + 6, 2026 * 12 + 6)
obs["win"] = np.where((obs["ym"] >= W1[0]) & (obs["ym"] < W1[1]), "W1",
                      np.where((obs["ym"] >= W2[0]) & (obs["ym"] < W2[1]), "W2", "-"))
obs = obs[obs["win"] != "-"].copy()
print(f"\nquan sat dung duoc: {len(obs)}  "
      f"(W1 {int((obs['win']=='W1').sum())}, W2 {int((obs['win']=='W2').sum())})")
for w in ("W1", "W2"):
    s = obs[obs["win"] == w]
    print(f"  {w}: {s['ym'].nunique()} thang, "
          f"Amp mean {s['Amp'].mean():.4f} sd {s['Amp'].std():.4f}; "
          f"Disp mean {s['Disp'].mean():+.4f} sd {s['Disp'].std():.4f}")

# ---- bang phep so ----------------------------------------------------------
rows = []
for measure in ("Amp", "Disp"):
    for ruler, col, marks in (("A-dom", "dom", range(1, 32)),
                              ("B-tdfe", "tdfe", range(1, 24))):
        for mk in marks:
            rec = {"measure": measure, "ruler": ruler, "mark": mk}
            for w in ("W1", "W2"):
                s = obs[obs["win"] == w]
                a = s.loc[s[col] == mk, measure].to_numpy()
                b = s.loc[s[col] != mk, measure].to_numpy()
                if len(a) < 3:
                    rec[f"n_{w}"], rec[f"d_{w}"], rec[f"p_{w}"] = len(a), np.nan, np.nan
                    continue
                t, p = stats.ttest_ind(a, b, equal_var=False)
                rec[f"n_{w}"] = len(a)
                rec[f"d_{w}"] = a.mean() - b.mean()
                rec[f"p_{w}"] = p
                rec[f"se_{w}"] = a.std(ddof=1) / np.sqrt(len(a))
            rows.append(rec)
T = pd.DataFrame(rows)
n_tests = int(T[["p_W1", "p_W2"]].notna().to_numpy().sum())
print(f"\n=== PHEP SO THUC TE CHAY: {n_tests} "
      f"(khai 216; {len(T)} to hop x 2 cua so) ===")

T["same_sign"] = np.sign(T["d_W1"]) == np.sign(T["d_W2"])
T["both_p05"] = (T["p_W1"] < 0.05) & (T["p_W2"] < 0.05)
T["pass"] = T["same_sign"] & T["both_p05"]

print(f"\nmoc p<0,05 o W1 chi: {int((T['p_W1']<0.05).sum())}"
      f" | o W2 chi: {int((T['p_W2']<0.05).sum())}"
      f" | ky vong may rui moi cua so: {len(T)*0.05:.1f}")
print(f"moc CUNG DAU (mot minh, vo dung): {int(T['same_sign'].sum())} / {len(T)}"
      f"  (ky vong may rui {len(T)*0.5:.0f})")
print(f"moc QUA SANG (cung dau VA p<0,05 ca hai cua so): {int(T['pass'].sum())}"
      f"  (ky vong duong-gia {len(T)*0.00125:.3f})")

# Benjamini-Hochberg DE DOC, khong lam cong
pv = np.sort(T[["p_W1", "p_W2"]].to_numpy().ravel()[
    ~np.isnan(T[["p_W1", "p_W2"]].to_numpy().ravel())])
m = len(pv)
bh = pv <= (np.arange(1, m + 1) / m) * 0.05
print(f"BH q=0,05 tren {m} p-value: {int(bh.sum())} phep so song"
      f" (p nho nhat {pv[0]:.4g}, nguong BH dau tien {0.05/m:.3g})")

pd.set_option("display.width", 200)
cols = ["measure", "ruler", "mark", "n_W1", "d_W1", "p_W1", "n_W2", "d_W2", "p_W2",
        "same_sign", "both_p05", "pass"]
T.to_csv("month-clock-precheck.csv", index=False)
print("\n--- MOC CO p<0,05 O IT NHAT MOT CUA SO ---")
sub = T[(T["p_W1"] < 0.05) | (T["p_W2"] < 0.05)].sort_values(["measure", "ruler", "mark"])
print(sub[cols].to_string(index=False, float_format=lambda v: f"{v:.4f}"))

print("\n--- TOP 12 theo |d| trung binh hai cua so, trong so moc CUNG DAU ---")
T["d_abs"] = (T["d_W1"].abs() + T["d_W2"].abs()) / 2
print(T[T["same_sign"]].nlargest(12, "d_abs")[cols].to_string(
    index=False, float_format=lambda v: f"{v:.4f}"))

print("\n--- RIENG CAC MOC CUOI/DAU THANG (prior tai can bang) ---")
pri = T[((T["ruler"] == "B-tdfe") & (T["mark"].isin([1, 2, 3, 20, 21, 22, 23])))
        | ((T["ruler"] == "A-dom") & (T["mark"].isin([1, 2, 3, 28, 29, 30, 31])))]
print(pri[cols].to_string(index=False, float_format=lambda v: f"{v:.4f}"))

T.to_pickle("month-clock-T.pkl")
obs.to_pickle("month-clock-obs.pkl")
print("\nluu: month-clock-precheck.csv / month-clock-T.pkl / month-clock-obs.pkl")
