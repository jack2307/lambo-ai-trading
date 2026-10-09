"""agent/month-clock: STRESS mot moc duy nhat qua sang (Amp / B-tdfe = 4).

Bon cau hoi, moi cau co the giet moc nay truoc khi no tieu mot o cong nao:
 1. Hoan vi TRONG THANG (khoi thang) — xu ly cum bien dong & tu tuong quan.
 2. No co phai hien vat THU TRONG TUAN?
 3. No co phai hien vat THANG TRONG NAM / ngay le?
 4. No con song khi doi CHUAN HOA (bien do ngay cua chinh ngay do, va ATR 15m)?
 5. Bien do thap co di kem DICH CHUYEN gi khong (edge huong = 0?).
"""
import numpy as np
import pandas as pd
from scipy import stats

obs = pd.read_pickle("month-clock-obs.pkl")
T = pd.read_pickle("month-clock-T.pkl")
rng = np.random.default_rng(20261009)

MARK = 4
print("=== MOC: Amp, thuoc B-tdfe, N = 4 (ngay giao dich thu 4 tu CUOI thang) ===")
row = T[(T.measure == "Amp") & (T.ruler == "B-tdfe") & (T["mark"] == MARK)].iloc[0]
for w in ("W1", "W2"):
    s = obs[obs.win == w]
    a = s.loc[s.tdfe == MARK, "Amp"]
    b = s.loc[s.tdfe != MARK, "Amp"]
    print(f"{w}: n={len(a)}  Amp(moc)={a.mean():.4f}  Amp(con lai)={b.mean():.4f}"
          f"  hieu={a.mean()-b.mean():+.4f} bien do ngay"
          f"  = {(a.mean()/b.mean()-1)*100:+.1f}% tuong doi"
          f"  Welch p={row['p_'+w]:.5f}")

# ---- 1. hoan vi TRONG THANG (giu nhan thang, xao vi tri ngay trong thang) ----
print("\n--- 1. HOAN VI TRONG THANG, 20.000 lan (khoi = mot thang) ---")
NPERM = 20000
for w in ("W1", "W2"):
    s = obs[obs.win == w]
    groups = [g["Amp"].to_numpy() for _, g in s.groupby("ym")]
    # vi tri thuc cua moc trong moi thang: chi so (len-MARK)
    real = np.mean([g[len(g) - MARK] for g in groups if len(g) >= MARK])
    draws = np.empty(NPERM)
    for i in range(NPERM):
        draws[i] = np.mean([g[rng.integers(len(g))] for g in groups if len(g) >= MARK])
    p = (np.sum(draws <= real) + 1) / (NPERM + 1)
    print(f"{w}: Amp thuc o moc = {real:.4f}; hoan vi trung vi = {np.median(draws):.4f};"
          f" phan vi cua moc = {100*np.mean(draws<=real):.2f};  p mot phia = {p:.4f}")

# ---- 2. thu trong tuan -------------------------------------------------------
print("\n--- 2. HIEN VAT THU TRONG TUAN? ---")
print("phan bo weekday (0=Mon) cua tdfe=4:",
      obs.loc[obs.tdfe == MARK, "weekday"].value_counts().sort_index().to_dict())
print("phan bo weekday cua moi ngay       :",
      obs["weekday"].value_counts().sort_index().to_dict())
print("\nAmp trung binh theo weekday (ca hai cua so):")
print(obs.groupby("weekday")["Amp"].agg(["mean", "count"]).to_string(
    float_format=lambda v: f"{v:.4f}"))
# hieu trong TUNG weekday (kiem soat weekday)
print("\nhieu Amp(moc) - Amp(cung weekday, khong moc), tung cua so x weekday:")
for w in ("W1", "W2"):
    s = obs[obs.win == w]
    out = []
    for wd in sorted(s.weekday.unique()):
        ss = s[s.weekday == wd]
        a = ss.loc[ss.tdfe == MARK, "Amp"]
        b = ss.loc[ss.tdfe != MARK, "Amp"]
        if len(a) < 3:
            continue
        t, p = stats.ttest_ind(a, b, equal_var=False)
        out.append(f"  {w} wd{wd}: n={len(a):3d} hieu={a.mean()-b.mean():+.4f} p={p:.3f}")
    print("\n".join(out))

# ---- 3. thang trong nam -----------------------------------------------------
print("\n--- 3. HIEN VAT THANG TRONG NAM? (hieu theo tung thang, ca 16 nam) ---")
a_all = obs[obs.tdfe == MARK]
b_all = obs[obs.tdfe != MARK]
for mo in range(1, 13):
    a = a_all.loc[a_all.month == mo, "Amp"]
    b = b_all.loc[b_all.month == mo, "Amp"]
    print(f"  thang {mo:2d}: n={len(a):3d} hieu={a.mean()-b.mean():+.4f}")
pos = sum((a_all.loc[a_all.month == mo, "Amp"].mean()
           - b_all.loc[b_all.month == mo, "Amp"].mean()) < 0 for mo in range(1, 13))
print(f"  => {pos}/12 thang trong nam co hieu AM (cung dau voi ket qua tong)")

# ---- 4. doi CHUAN HOA -------------------------------------------------------
print("\n--- 4. DOI CHUAN HOA ---")
obs2 = obs.copy()
obs2["Amp_own"] = obs2["amp"] / obs2["day_range"]        # nen 15m / bien do NGAY DO
obs2["amp_raw"] = obs2["amp"]                            # diem gia, KHONG chuan hoa
obs2["Rng_base"] = obs2["day_range"] / obs2["base"]      # bien do NGAY / base
for col, lab in (("Amp", "amp15m / base20d  (dang ky)"),
                 ("Amp_own", "amp15m / bien do ngay do"),
                 ("amp_raw", "amp15m tho [USD]"),
                 ("Rng_base", "bien do NGAY / base20d")):
    line = [f"{lab:30s}"]
    for w in ("W1", "W2"):
        s = obs2[obs2.win == w]
        a = s.loc[s.tdfe == MARK, col]
        b = s.loc[s.tdfe != MARK, col]
        t, p = stats.ttest_ind(a, b, equal_var=False)
        line.append(f"{w}: hieu={a.mean()-b.mean():+.5f} ({(a.mean()/b.mean()-1)*100:+.1f}%) p={p:.4f}")
    print("  " + "  |  ".join(line))

# ---- 5. bien do thap co huong khong? ---------------------------------------
print("\n--- 5. DICH CHUYEN CO DAU o cung moc (edge HUONG) ---")
for w in ("W1", "W2"):
    s = obs[obs.win == w]
    a = s.loc[s.tdfe == MARK, "Disp"]
    b = s.loc[s.tdfe != MARK, "Disp"]
    t, p = stats.ttest_ind(a, b, equal_var=False)
    se = a.std(ddof=1) / np.sqrt(len(a))
    print(f"{w}: n={len(a)} Disp(moc)={a.mean():+.4f} +-{se:.4f}(se)"
          f"  Disp(con lai)={b.mean():+.4f}  hieu={a.mean()-b.mean():+.4f}  p={p:.4f}")
    print(f"     => bang khong trong +-{1.96*se:.3f} bien do ngay (95%);"
          f" dau: {int((a>0).sum())}/{len(a)} ngay duong")

# ---- 6. moc nay co gi KHAC mot gate VolAbs da co? --------------------------
print("\n--- 6. MOC NAY SO VOI MOT GATE BIEN DONG DA CO (VolAbs) ---")
for w in ("W1", "W2"):
    s = obs[obs.win == w]
    q = s["Amp"].quantile([0.1, 0.25, 0.5, 0.75, 0.9])
    a = s.loc[s.tdfe == MARK, "Amp"]
    print(f"{w}: phan vi Amp cua moi ngay: p10={q[0.1]:.4f} p25={q[0.25]:.4f}"
          f" p50={q[0.5]:.4f} p75={q[0.75]:.4f} p90={q[0.9]:.4f}")
    print(f"     Amp trung binh cua moc = {a.mean():.4f}"
          f" => nam o phan vi {100*(s['Amp'] < a.mean()).mean():.0f} cua phan phoi ngay")
    print(f"     be rong mot bang VolAbs p25-p75 = {q[0.75]-q[0.25]:+.4f};"
          f" hieu cua moc = {a.mean()-s.loc[s.tdfe!=MARK,'Amp'].mean():+.4f}"
          f" = {100*abs(a.mean()-s.loc[s.tdfe!=MARK,'Amp'].mean())/(q[0.75]-q[0.25]):.0f}% be rong do")
