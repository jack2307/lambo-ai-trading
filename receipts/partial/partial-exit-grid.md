# Receipt — thoat MOT PHAN: luoi 3x2 tren vang, hai cua so, hai arm

Ngay 2026-10-09. Nhanh `agent/partial-exit`, worktree `/e/rust/fd-partial-exit`.
Dang ky truoc: `docs/decisions/2026-10-09-partial-exit.md` (commit `e61c6cd`,
chi docs, truoc dong code dau tien). Ban va engine: commit `c3d746e`.

## Thiet lap, nguyen van tu header receipt

    binary:   /e/rust/fd-partial-exit/target-pe/release/search.exe
              (build tu agent/partial-exit = agent/stop-width + ban va partial;
               co lbar_line, drawdown, expectancy_net)
    market:   xauusd 15m
    data:     E:/rust/flowdesk/data (bars from E:/rust/flowdesk/data\bars\XAUUSD-15m.parquet)
    spread:   0.28 per round trip
    trail:    off
    partial:  ON bank <f> of the position at +<x>R, the rest runs to its own exit
    max hold: 4 h (14400000 ms) from [trading] max_hold_ms, no per-market override
    news:     747 events (2010-01-08 -> 2027-12-08) from E:/rust/flowdesk/data\news\events.parquet
    news scope: USD (news_currencies)
    null sides: coin - 50/50, carries no drift
    seeds:    200 matched null runs moi dong
    flags:    9 passed, every one of them read by --mode=hypotheses
    Cua so A: --from=2025-07-01 --to=2025-10-01 (6044 bars)
    Cua so B: --from=2025-04-01 --to=2025-07-01
    Batch:    docs/hypotheses/2026-10-09-partial-grid.toml (ban sao nguyen van
              cua docs/hypotheses/2026-10-06-trail-grid.toml tren agent/m4)

Dong `news:` in ra `E:/rust/flowdesk/data\news\events.parquet` — dung nguon ma
`--data=` khai. `data-sealed/` khong duoc tro toi, khong doc, khong dem.

## Doi chung TAI HIEN DUNG TUNG CHU SO con so cua agent/m4

Phu luc 8 muc II do rang chi 65,3% o chay lai khop so da cong bo. **Dong nay
khop:** `intraday/donchian-breakout` cua so A, `--fixed`, partial off:

    agent/m4 (06/10):  271 lenh  PF_usd 1.117  expect +0.054  TARGET 49  hold 128.4 min
    chay lai (09/10):  271 lenh  PF_usd 1.1175 expect +0.0536 TARGET 49  hold 128.4 min

So moi ma m4 khong co (bay gio co lbar_line + drawdown):

    PF_r 1.1259 (R) vs PF_usd 1.1175 (USD, gap +0.0084)
    Lbar 0.4254 R  => chan expectancy DU (Lbar >= 0.250R)
    sut 15.49 USD = 13.58% dinh von duong cong da dong lenh; net +13.52 USD

## Luat CO no — va no khong doi mot lenh nao

O moi o, `--exit-mix` in **dung cung mot dong** nhu doi chung: cua so A
khong-guards, ca 7 o (off + 6) deu la
`END_OF_DATA 1, STOP 65, TARGET 49, TIMEOUT 65, flat window 23, lost the
channel midline 30, reclaimed the channel midline 38; mean hold 128.4 min`,
va `n = 271` o ca 7. Trong khi do **178/271 vi the (65,7%) da chot mot phan**.

⇒ Day la mot phep co lap sach nhat co the: **so lenh, cach thoat, thoi gian
giu KHONG DOI; thu duy nhat doi la CO cua phan mang vao duoi.** Falsifier F2
(luat khong no) KHONG ban: so lan chot tu 10 den 258 tuy o, khong o nao bang 0.

## Toan bo luoi — ca 84 dong, khong chi o thang

### `intraday/donchian-breakout`

| o (muc / ti le) | arm | A n | A PF_r | A PF_usd | A E (R) | A Lbar | A sut USD | A chot | B n | B PF_r | B PF_usd | B E (R) | B Lbar | B sut USD | B chot |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **off (doi chung)** | noguards | 271 | 1.1259 | 1.1175 | +0.0536 | 0.4254 | 15.49 | - | 280 | 1.0376 | 1.0341 | +0.0168 | 0.4457 | 10.98 | - |
| 0.33 / 0.25 | noguards | 271 | 1.0759 | 1.0675 | +0.0285 | 0.3750 | 14.03 | 178/271 | 280 | 1.0213 | 1.0191 | +0.0083 | 0.3894 | 9.38 | 195/280 |
| 0.33 / 0.50 | noguards | 271 | 0.9867 | 0.9774 | -0.0042 | 0.3194 | 13.07 | 178/271 | 280 | 0.9890 | 0.9876 | -0.0035 | 0.3158 | 10.17 | 199/280 |
| 0.67 / 0.25 | noguards | 271 | 1.1178 | 1.1050 | +0.0455 | 0.3862 | 13.78 | 142/271 | 280 | 1.0223 | 1.0203 | +0.0092 | 0.4127 | 9.32 | 141/280 |
| 0.67 / 0.50 | noguards | 271 | 1.1034 | 1.1008 | +0.0359 | 0.3467 | 11.53 | 142/271 | 280 | 1.0040 | 1.0042 | +0.0015 | 0.3755 | 9.89 | 145/280 |
| 1.00 / 0.25 | noguards | 271 | 1.1092 | 1.0964 | +0.0449 | 0.4107 | 14.06 | 104/271 | 280 | 1.0349 | 1.0334 | +0.0148 | 0.4250 | 10.24 | 108/280 |
| 1.00 / 0.50 | noguards | 271 | 1.0879 | 1.0786 | +0.0352 | 0.4004 | 12.37 | 104/271 | 280 | 1.0346 | 1.0301 | +0.0140 | 0.4052 | 9.54 | 110/280 |
| **off (doi chung)** | guards | 268 | 1.0846 | 1.0681 | +0.0359 | 0.4244 | 11.55 | - | 276 | 1.0526 | 1.0302 | +0.0231 | 0.4395 | 9.97 | - |
| 0.33 / 0.25 | guards | 268 | 1.0428 | 1.0211 | +0.0159 | 0.3713 | 10.73 | 174/268 | 276 | 1.0389 | 1.0236 | +0.0148 | 0.3813 | 9.78 | 193/276 |
| 0.33 / 0.50 | guards | 268 | 0.9577 | 0.9546 | -0.0135 | 0.3191 | 10.05 | 174/268 | 276 | 1.0087 | 0.9937 | +0.0027 | 0.3102 | 9.76 | 197/276 |
| 0.67 / 0.25 | guards | 268 | 1.0803 | 1.0657 | +0.0305 | 0.3805 | 10.61 | 137/268 | 276 | 1.0379 | 1.0228 | +0.0154 | 0.4055 | 9.83 | 138/276 |
| 0.67 / 0.50 | guards | 268 | 1.0690 | 1.0636 | +0.0237 | 0.3438 | 9.70 | 137/268 | 276 | 1.0137 | 1.0076 | +0.0051 | 0.3736 | 9.88 | 142/276 |
| 1.00 / 0.25 | guards | 268 | 1.0706 | 1.0514 | +0.0287 | 0.4073 | 10.89 | 99/268 | 276 | 1.0494 | 1.0297 | +0.0207 | 0.4193 | 9.66 | 106/276 |
| 1.00 / 0.50 | guards | 268 | 1.0512 | 1.0427 | +0.0204 | 0.3988 | 9.95 | 99/268 | 276 | 1.0465 | 1.0354 | +0.0187 | 0.4031 | 8.45 | 108/276 |

### `intraday/bb-fade`

| o (muc / ti le) | arm | A n | A PF_r | A PF_usd | A E (R) | A Lbar | A sut USD | A chot | B n | B PF_r | B PF_usd | B E (R) | B Lbar | B sut USD | B chot |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **off (doi chung)** | noguards | 369 | 0.8016 | 0.8133 | -0.1062 | 0.5354 | 48.71 | - | 385 | 0.8892 | 0.8761 | -0.0588 | 0.5300 | 35.88 | - |
| 0.33 / 0.25 | noguards | 369 | 0.7834 | 0.7963 | -0.1004 | 0.4632 | 43.80 | 256/369 | 385 | 0.8546 | 0.8449 | -0.0687 | 0.4724 | 34.72 | 258/385 |
| 0.33 / 0.50 | noguards | 369 | 0.7564 | 0.7657 | -0.0928 | 0.3808 | 37.71 | 256/369 | 385 | 0.7913 | 0.7829 | -0.0834 | 0.3997 | 33.95 | 258/385 |
| 0.67 / 0.25 | noguards | 369 | 0.7875 | 0.7985 | -0.1065 | 0.5011 | 46.89 | 191/369 | 385 | 0.8590 | 0.8491 | -0.0709 | 0.5030 | 35.58 | 190/385 |
| 0.67 / 0.50 | noguards | 369 | 0.7756 | 0.7821 | -0.1034 | 0.4607 | 44.28 | 191/369 | 385 | 0.8167 | 0.8120 | -0.0859 | 0.4683 | 35.40 | 190/385 |
| 1.00 / 0.25 | noguards | 369 | 0.8043 | 0.8137 | -0.1012 | 0.5172 | 46.78 | 129/369 | 385 | 0.8795 | 0.8661 | -0.0622 | 0.5161 | 35.18 | 144/385 |
| 1.00 / 0.50 | noguards | 369 | 0.8101 | 0.8223 | -0.0943 | 0.4964 | 43.28 | 129/369 | 385 | 0.8702 | 0.8571 | -0.0648 | 0.4991 | 33.57 | 144/385 |
| **off (doi chung)** | guards | 349 | 0.8099 | 0.8295 | -0.0994 | 0.5232 | 21.03 | - | 367 | 0.9016 | 0.8905 | -0.0515 | 0.5231 | 25.06 | - |
| 0.33 / 0.25 | guards | 349 | 0.7923 | 0.8101 | -0.0942 | 0.4534 | 19.52 | 244/349 | 367 | 0.8721 | 0.8594 | -0.0607 | 0.4742 | 25.10 | 247/367 |
| 0.33 / 0.50 | guards | 349 | 0.7780 | 0.8009 | -0.0819 | 0.3689 | 16.52 | 244/349 | 367 | 0.8071 | 0.7980 | -0.0771 | 0.3994 | 25.98 | 247/367 |
| 0.67 / 0.25 | guards | 349 | 0.7973 | 0.8203 | -0.0994 | 0.4900 | 20.12 | 181/349 | 367 | 0.8760 | 0.8627 | -0.0621 | 0.5008 | 25.99 | 182/367 |
| 0.67 / 0.50 | guards | 349 | 0.7899 | 0.8165 | -0.0945 | 0.4498 | 18.65 | 181/349 | 367 | 0.8276 | 0.8132 | -0.0802 | 0.4655 | 28.19 | 182/367 |
| 1.00 / 0.25 | guards | 349 | 0.8098 | 0.8305 | -0.0969 | 0.5098 | 20.32 | 118/349 | 367 | 0.8921 | 0.8818 | -0.0552 | 0.5122 | 24.66 | 135/367 |
| 1.00 / 0.50 | guards | 349 | 0.8119 | 0.8422 | -0.0929 | 0.4936 | 18.72 | 118/349 | 367 | 0.8780 | 0.8706 | -0.0606 | 0.4965 | 24.09 | 135/367 |

### `compression/bb-fade`

| o (muc / ti le) | arm | A n | A PF_r | A PF_usd | A E (R) | A Lbar | A sut USD | A chot | B n | B PF_r | B PF_usd | B E (R) | B Lbar | B sut USD | B chot |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **off (doi chung)** | noguards | 39 | 0.6572 | 0.6537 | -0.1929 | 0.5629 | 8.13 | - | 35 | 1.0671 | 1.0489 | +0.0271 | 0.4043 | 7.27 | - |
| 0.33 / 0.25 | noguards | 39 | 0.6653 | 0.6609 | -0.1568 | 0.4684 | 6.82 | 28/39 | 35 | 1.0506 | 1.0380 | +0.0180 | 0.3552 | 6.24 | 25/35 |
| 0.33 / 0.50 | noguards | 39 | 0.6906 | 0.6888 | -0.1108 | 0.3582 | 5.68 | 28/39 | 35 | 1.0097 | 0.9970 | +0.0029 | 0.3020 | 5.16 | 25/35 |
| 0.67 / 0.25 | noguards | 39 | 0.6579 | 0.6554 | -0.1785 | 0.5218 | 7.43 | 19/39 | 35 | 1.0997 | 1.0877 | +0.0372 | 0.3729 | 6.45 | 21/35 |
| 0.67 / 0.50 | noguards | 39 | 0.6531 | 0.6465 | -0.1662 | 0.4792 | 7.08 | 19/39 | 35 | 1.1502 | 1.1418 | +0.0503 | 0.3349 | 5.51 | 21/35 |
| 1.00 / 0.25 | noguards | 39 | 0.6627 | 0.6592 | -0.1855 | 0.5500 | 7.84 | 10/39 | 35 | 1.0806 | 1.0674 | +0.0326 | 0.4043 | 7.05 | 14/35 |
| 1.00 / 0.50 | noguards | 39 | 0.6656 | 0.6608 | -0.1796 | 0.5372 | 7.64 | 10/39 | 35 | 1.1010 | 1.0851 | +0.0408 | 0.4043 | 6.76 | 14/35 |
| **off (doi chung)** | guards | 39 | 0.6335 | 0.6671 | -0.2063 | 0.5629 | 2.84 | - | 33 | 1.2463 | 1.1141 | +0.0904 | 0.3672 | 3.64 | - |
| 0.33 / 0.25 | guards | 39 | 0.6413 | 0.6832 | -0.1668 | 0.4651 | 2.24 | 28/39 | 33 | 1.2013 | 1.0862 | +0.0654 | 0.3248 | 3.08 | 24/33 |
| 0.33 / 0.50 | guards | 39 | 0.6620 | 0.7216 | -0.1225 | 0.3625 | 1.82 | 28/39 | 33 | 1.1404 | 1.0527 | +0.0393 | 0.2803 | 2.52 | 24/33 |
| 0.67 / 0.25 | guards | 39 | 0.6355 | 0.6779 | -0.1899 | 0.5212 | 2.62 | 19/39 | 33 | 1.2214 | 1.0855 | +0.0785 | 0.3545 | 3.53 | 19/33 |
| 0.67 / 0.50 | guards | 39 | 0.6379 | 0.6778 | -0.1736 | 0.4796 | 2.48 | 19/39 | 33 | 1.1897 | 1.0437 | +0.0648 | 0.3419 | 3.41 | 19/33 |
| 1.00 / 0.25 | guards | 39 | 0.6413 | 0.6889 | -0.1978 | 0.5514 | 2.59 | 10/39 | 33 | 1.2642 | 1.1383 | +0.0970 | 0.3672 | 3.45 | 14/33 |
| 1.00 / 0.50 | guards | 39 | 0.6495 | 0.7033 | -0.1893 | 0.5400 | 2.41 | 10/39 | 33 | 1.2831 | 1.1691 | +0.1040 | 0.3672 | 3.25 | 14/33 |
`A chot` / `B chot` = so vi the da chot mot phan / so vi the. `sut USD` =
`max_drawdown_usd` tren duong von DA DONG LENH (excursion trong lenh khong
nam trong do; `avg_mae` la truong duy nhat thay no). Khong dong nao in
`_pct > 100%`: cao nhat ca 84 dong la **42,05%**, nen khong dong nao chay tai
khoan va PF cua moi dong doc duoc.

## Lieu luong THAT khac lieu luong KHAI — va toi doc dap ung theo lieu THAT

`lots` phai di theo `lot_step = 0.01` va ca hai phan phai >= `min_lot = 0.01`,
tren mot so 100 USD (`starting_equity_usd = 100.0`, TK cent) voi
`contract_size = 1.0`: `lots` cua dong nay ~0,13. Nen:

    ti le KHAI 0,25  ->  ti le THAT do duoc 0,170 - 0,237  (trung binh ~0,21)
    ti le KHAI 0,50  ->  ti le THAT do duoc 0,433 - 0,480  (trung binh ~0,46)

(do bang `banked_r / (so_lan_chot x muc)`, gia dinh khop dung tai muc.)

Va o cua so B, **so lan chot khac nhau giua hai ti le**: 195 o f=0,25 so voi
199 o f=0,50 (donchian, khong-guards). Ly do la cung mot thu: mot vai vi the
nho den muc `0,25 x lots < min_lot`, nen chung **khong chot duoc mot phan tu
ma chot duoc mot nua**. Day la khuyet diem 17 cua phu luc 7 (TK cent, lot toi
thieu) hien ra o truc nay, va no la mot **sai lieu luong**, khong phai mot sai
dau. Moi so dap ung duoi day chia cho lieu THAT.

## GIA TRI TIEP TUC: dai luong quyet dinh ca truc nay

Mo hinh, mot dong: ban mot phan `f` cua mot vi the tai `+xR` thay cho de no
chay den ket cuc cua no doi `E` di **−(f/n) x sum(r_cuoi − x)** tren tap vi
the co cham muc. Nen tu hai lieu luong doc ra duoc **mot so**: ket cuc trung
binh cua mot vi the DA cham `+xR`.

Hai lieu luong la hai phep do doc lap cua cung so do. Chung **khop nhau toi
<= 0,027 R o moi o, phan lon < 0,01 R** — nghia la dap ung TUYEN TINH theo
`f`, dung nhu mo hinh doi, va day la kiem dung truoc khi tin con so.
| co che | arm | muc +xR | A cham | A tiep tuc (R) | A dau | B cham | B tiep tuc (R) | B dau |
|---|---|---|---|---|---|---|---|---|
| `intraday/donchian-breakout` | noguards | +0.33 | 65.7% | +0.515 / +0.526 | **> muc** | 71.1% | +0.394 / +0.394 | **> muc** |
| `intraday/donchian-breakout` | noguards | +0.67 | 52.4% | +0.743 / +0.742 | **> muc** | 51.8% | +0.747 / +0.735 | **> muc** |
| `intraday/donchian-breakout` | noguards | +1.00 | 38.4% | +1.106 / +1.102 | **> muc** | 39.3% | +1.026 / +1.016 | **> muc** |
| `intraday/donchian-breakout` | guards | +0.33 | 64.9% | +0.475 / +0.501 | **> muc** | 71.4% | +0.390 / +0.394 | **> muc** |
| `intraday/donchian-breakout` | guards | +0.67 | 51.1% | +0.716 / +0.720 | **> muc** | 51.4% | +0.745 / +0.746 | **> muc** |
| `intraday/donchian-breakout` | guards | +1.00 | 36.9% | +1.083 / +1.087 | **> muc** | 39.1% | +1.031 / +1.025 | **> muc** |
| `intraday/bb-fade` | noguards | +0.33 | 69.4% | +0.289 / +0.287 | < muc | 67.0% | +0.406 / +0.412 | **> muc** |
| `intraday/bb-fade` | noguards | +0.67 | 51.8% | +0.673 / +0.658 | lan muc | 49.4% | +0.795 / +0.789 | **> muc** |
| `intraday/bb-fade` | noguards | +1.00 | 35.0% | +0.934 / +0.927 | < muc | 37.4% | +1.046 / +1.034 | **> muc** |
| `intraday/bb-fade` | guards | +0.33 | 69.9% | +0.293 / +0.273 | < muc | 67.3% | +0.410 / +0.418 | **> muc** |
| `intraday/bb-fade` | guards | +0.67 | 51.9% | +0.670 / +0.649 | lan muc | 49.6% | +0.795 / +0.798 | **> muc** |
| `intraday/bb-fade` | guards | +1.00 | 33.8% | +0.966 / +0.958 | < muc | 36.8% | +1.059 / +1.056 | **> muc** |
| `compression/bb-fade` | noguards | +0.33 | 71.8% | +0.091 / +0.074 | < muc | 71.4% | +0.392 / +0.406 | **> muc** |
| `compression/bb-fade` | noguards | +0.67 | 48.7% | +0.539 / +0.553 | < muc | 60.0% | +0.590 / +0.587 | < muc |
| `compression/bb-fade` | noguards | +1.00 | 25.6% | +0.874 / +0.891 | < muc | 40.0% | +0.935 / +0.927 | < muc |
| `compression/bb-fade` | guards | +0.33 | 71.8% | +0.080 / +0.068 | < muc | 72.7% | +0.485 / +0.486 | **> muc** |
| `compression/bb-fade` | guards | +0.67 | 48.7% | +0.527 / +0.527 | < muc | 57.6% | +0.759 / +0.764 | **> muc** |
| `compression/bb-fade` | guards | +1.00 | 25.6% | +0.860 / +0.860 | < muc | 42.4% | +0.933 / +0.932 | < muc |
`cham` = ti le vi the cham muc (o o f=0,50). `tiep tuc` = ket cuc trung binh
cua mot vi the da cham muc, doc tu lieu 0,25 / lieu 0,50.

**Doc bang mot cau:** `tiep tuc > muc` nghia la phan duoi con lai dang gia
HON cai muc, nen ban mot phan o do la **ban re** va `E` phai giam. Do la dau
cua moi o cua `donchian-breakout`: **12/12 (3 muc x 2 cua so x 2 arm) co
`tiep tuc > muc`.** Hai dong `bb-fade` **lat dau giua hai cua so**: cua so A
`tiep tuc < muc` (nen chot mot phan GIUP), cua so B `tiep tuc > muc` (nen no
LAM XAU). Cung mot co che, hai quy lien nhau.

## Tong hop 72 o luoi so voi doi chung CUA CHINH CHUNG

    E tot hon doi chung:       28 / 72 o
    E xau hon:                 43 / 72 o
    E y nguyen:                 1 / 72 o
    sut USD NHO hon:           68 / 72 o
    sut USD lon hon:            4 / 72 o   (deu la intraday/bb-fade cua so B, arm guards)

    qua CONG that (PF_r >= 1,200 VA E >= +0,050R VA >= 40 lenh, dem tay):
       0 / 72 o luoi   va   0 / 12 dong doi chung   =>  0 / 84

Nam dong qua HAI chan so ma truot san co mau — tat ca la
`compression/bb-fade` cua so B arm guards voi **n = 33 < 40**, va dong `off`
cua chinh no cung o trong do (PF_r 1,2463 / E +0,0904 / n 33). Cung bon o day
o cua so A cho PF_r 0,63-0,65 va E −0,17 den −0,19. Day la ly do san 40 lenh
ton tai, va la ly do khong cong bo o nao trong so chung nhu mot ung vien.

## Hai chan PF doc ra HAI phia vach 1,200 o 5/84 dong

    compression/bb-fade B guards off        PF_r 1.2463 | PF_usd 1.1141
    compression/bb-fade B guards 0.33/0.25  PF_r 1.2013 | PF_usd 1.0862
    compression/bb-fade B guards 0.67/0.25  PF_r 1.2214 | PF_usd 1.0855
    compression/bb-fade B guards 1.00/0.25  PF_r 1.2642 | PF_usd 1.1383
    compression/bb-fade B guards 1.00/0.50  PF_r 1.2831 | PF_usd 1.1691

Tai day **doc bang USD se LOAI mot dong ma doc bang R nhan** — nguoc chieu
voi phu luc 8 muc III. Dem tren 84 dong cua job nay:

    PF_usd doc THAP hon PF_r:  62 / 84 dong (73,8%)
    PF_usd doc CAO hon:        22 / 84 dong

Phu luc 8 do 320/452 o (71%) `PF_usd` doc **cao hon**. Hai phep do nguoc dau
nhau ⇒ **chieu cua khe USD-vs-R khong phai mot hang so cua engine**; no phai
duoc DO o tung dong, khong duoc du doan. (Dong ho so khac nhau: 452 o kia la
ho `close/*` tu quan, 84 dong nay la ba co che `Exits::Engine`.)

Va **hang dang thuc van dung khi co le lenh doi giua duong**: `E = Lbar x
(PF_r − 1)` co phan du lon nhat **0,00007 R** tren ca 84 dong, ke ca 72 dong
co vi the thay doi co giua duong. Do la kiem dung ke toan cua ban va.

## Khong cong bo phan vi

Moi dong in `count match 0.62 - 0.73 ** outside the band: this percentile is
unmatched **` va `cost match 0.60 - 0.72 ** outside the band **`, dung nhu
doi chung cua `agent/m4` da in. Theo brief §4, **o ngoai bang thi khong cong
bo phan vi**, va job nay khong dua ket luan nao vao phan vi. (`null p50` cua
moi dong nam trong 0,824 - 0,987 — duoi 1, tuc null trung vi LO tien.)

## File receipt tho

28 file, mot cho moi (cua so x arm x o), ten
`receipts/partial/<cua so>-<muc>-<ti le>-<arm>.txt`, moi file chua ca ba dong
hypothesis va header day du.
