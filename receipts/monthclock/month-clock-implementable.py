"""agent/month-clock: moc duy nhat qua sang co THI HANH DUOC khong?

Thuoc B o tien kiem dem "ngay CO NEN tu cuoi thang" — tuc da loai ngay le SAU
KHI biet ngay nao co nen. Mot `Filter` trong engine chi biet LICH: no co the
dem nguoc tu ngay cuoi thang qua Thu Bay/Chu Nhat, nhung KHONG biet ngay le.
Neu moc chet duoi thuoc chi-dung-lich thi no khong thi hanh duoc.
"""
import numpy as np
import pandas as pd
from scipy import stats

obs = pd.read_pickle("month-clock-obs.pkl")

# --- thuoc B' : dem nguoc tu ngay cuoi thang, CHI bo Thu Bay/Chu Nhat -------
# (dung duoc tu lich, khong can biet ngay nao co nen)
rows = []
for (y, m), g in obs.groupby(["year", "month"]):
    last_dom = pd.Timestamp(year=int(y), month=int(m), day=1).days_in_month
    cal = pd.date_range(f"{int(y)}-{int(m):02d}-01", periods=last_dom, freq="D")
    wd = cal[cal.dayofweek < 5]          # Mon..Fri cua thang, theo LICH
    idx = {d.day: len(wd) - i for i, d in enumerate(wd)}   # 1 = ngay lam viec cuoi
    for j, dom in zip(g.index, g["dom"]):
        rows.append((j, idx.get(int(dom), np.nan)))
cal_idx = pd.Series({j: v for j, v in rows})
obs["tdfe_cal"] = cal_idx.reindex(obs.index)

agree = (obs["tdfe"] == obs["tdfe_cal"]).mean()
print(f"thuoc B (ngay CO NEN) vs B' (chi lich): khop {100*agree:.1f}% cua"
      f" {len(obs)} ngay")
print("  lech o cac ngay le: phan bo lech =",
      (obs['tdfe_cal'] - obs['tdfe']).value_counts().sort_index().to_dict())

print("\n=== MOC Amp @ N=4, do bang CA HAI thuoc, hai cua so ===")
for col, lab in (("tdfe", "B  ngay CO NEN (tien kiem)"),
                 ("tdfe_cal", "B' CHI LICH (thi hanh duoc)")):
    out = [f"{lab:30s}"]
    for w in ("W1", "W2"):
        s = obs[obs.win == w]
        a = s.loc[s[col] == 4, "Amp"]
        b = s.loc[s[col] != 4, "Amp"]
        t, p = stats.ttest_ind(a, b, equal_var=False)
        out.append(f"{w}: n={len(a)} hieu={a.mean()-b.mean():+.5f}"
                   f" ({(a.mean()/b.mean()-1)*100:+.1f}%) p={p:.4f}")
    print("  " + "  |  ".join(out))

# --- quet LAI toan bo thuoc B' (23 moc x 2 dai luong x 2 cua so) ------------
print("\n=== QUET LAI thuoc B' CHI LICH: 23 moc x 2 dai luong x 2 cua so ===")
res = []
for measure in ("Amp", "Disp"):
    for mk in range(1, 24):
        rec = {"measure": measure, "mark": mk}
        for w in ("W1", "W2"):
            s = obs[obs.win == w]
            a = s.loc[s["tdfe_cal"] == mk, measure]
            b = s.loc[s["tdfe_cal"] != mk, measure]
            if len(a) < 3:
                rec[f"n_{w}"], rec[f"d_{w}"], rec[f"p_{w}"] = len(a), np.nan, np.nan
                continue
            t, p = stats.ttest_ind(a, b, equal_var=False)
            rec[f"n_{w}"], rec[f"d_{w}"], rec[f"p_{w}"] = len(a), a.mean() - b.mean(), p
        res.append(rec)
R = pd.DataFrame(res)
R["same_sign"] = np.sign(R.d_W1) == np.sign(R.d_W2)
R["both_p05"] = (R.p_W1 < 0.05) & (R.p_W2 < 0.05)
R["pass"] = R.same_sign & R.both_p05
print(R.to_string(index=False, float_format=lambda v: f"{v:.4f}"))
print(f"\nthuoc B' : moc qua sang = {int(R['pass'].sum())}"
      f" ; p<0,05 o W1 chi = {int((R.p_W1<0.05).sum())}"
      f" ; o W2 chi = {int((R.p_W2<0.05).sum())}"
      f" ; ky vong may rui moi cua so = {len(R)*0.05:.1f}")

# --- va mot cau hoi nua: moc do co lap lai neu doi CHO CAT cua so? ---------
print("\n=== MOC Amp@4 (thuoc B goc) duoi BON cho cat cua so khac ===")
for cut in (2014 * 12 + 6, 2016 * 12 + 6, 2018 * 12 + 6, 2020 * 12 + 6, 2022 * 12 + 6):
    lab = f"{cut//12}-{cut%12:02d}"
    halves = [obs[obs.ym < cut], obs[obs.ym >= cut]]
    bits = []
    for k, h in enumerate(halves, 1):
        a = h.loc[h.tdfe == 4, "Amp"]
        b = h.loc[h.tdfe != 4, "Amp"]
        if len(a) < 10:
            bits.append(f"nua{k}: n={len(a)} qua it")
            continue
        t, p = stats.ttest_ind(a, b, equal_var=False)
        bits.append(f"nua{k}: n={len(a):3d} hieu={a.mean()-b.mean():+.5f} p={p:.4f}")
    print(f"  cat {lab}: " + " | ".join(bits))
obs.to_pickle("month-clock-obs.pkl")
