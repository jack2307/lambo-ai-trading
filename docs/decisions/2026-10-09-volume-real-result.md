# 2026-10-09 — volume-real, KET QUA: cot `volume` co 4 trang thai, khong 2; va
# ho volume DA tung duoc do tren feed co volume — no truot cong o ca hai

Dang ky: `docs/decisions/2026-10-09-volume-real.md`. Receipt:
`docs/research/runs/2026-10-09-volume-real/`. Binary: `fd-stop-width`
`search.exe` (co `lbar_line` + drawdown). O cong moi: **0**.

## 0. Mot cau

Gia thuyet "ho volume chua tung duoc do, 0-4 lenh la tieng cua mot cot so
khong" **sup o tien kiem o chan F2 va F3**: `volman-box` khong doc cot volume
**mot lan nao** (no la ho cua Bob *Volman*, khong phai *volume*), va
`volume-thrust` **da** duoc do tren hai feed co volume — `btc:15m` (677/362/179
lenh) va `xauusd:15m` 2022-06..2025-04 (241/57/43 lenh) — trong **chinh cua so
ma chu may de xuat do lai**. Nhung tien kiem lai tra ve mot thu khac va lon
hon: **cot tick volume cua feed sang la KHONG DUNG YEN**, va do la ly do that
lam ho nay khong do duoc tren vang.

## I. Tien kiem 1 — toan bo cot, 24 file, khong lay mau

Khong phai hai trang thai (0 va NULL) nhu phu luc 8 khai, ma **bon**:

    1. NULL tren 100% dong    Dukascopy 1m (AUDNZD/EURDUKA/XAGDUKA/XAUDUKA)
                              22.309.933 dong, 0 dong co so
                              + GC-1m: 18.383/18.383 dong NULL (chu may dem
                                16.903 — file dang lon, collector con chay)
    2. 0,0 THAT tren 100%     Dukascopy 5m va 15m, 4.699.007 dong
                              XAUDUKA-15m: 378.749/378.749 dong = 0,0
    3. 0,0 THEO GIAI DOAN     BTCUSD-15m: 2023 **100%** zero, 2024 **100%**
                              zero, 2025 **42,1%**, 2026 **0%**
                              => cot bat len giua nam 2025. 56.300/100.798 dong
    4. duong tren 100%        XAUUSD/EURUSD-15m (MT5 tick), BTCUSDT (Binance)

**Chu may dung o chan 15m, sai o chan 1m.** Cung mot vendor, cung mot
instrument, **hai cach ma hoa cua cung mot su vang mat**: 1m la NULL
("khong cong bo gi"), 5m/15m la 0,0 ("cong bo mot so khong"). Luat §8 bat
phan biet, va kho nay tu vi pham no o trong mot dong instrument.

Trang thai 3 la **moi, chua tung duoc dem**: BTCUSD-15m la mot feed co cot
volume **bat len giua cua so**. Bat ky dong nao doc no qua 2023-2026 dang doc
mot cot so khong o hai phan ba dau.

## II. Tien kiem 2 — ma: khong cho nao chia cho 0, va mot cho thay the im lang

Bon co che, doc truoc khi tieu o:

- `volume_thrust.rs:78` — `let Some(volume) = bar.volume else { return None }`
  roi `if mean <= 0.0 || volume < volMult*mean { return None }`.
  ⇒ cot 0 **hoac** NULL cho **0 lenh**, bang mot guard tuong minh.
  **Doan cua chu may dung chinh xac o day.** Nguong la `volMult x trung binh
  volume cua volBars=20 nen truoc`, nen trung binh 0 ⇒ nguong 0 — nhung
  `mean <= 0.0` chan truoc khi so sanh. Khong chia cho 0.
- `rsi_reversal_vol.rs:200 mean_volume()` — tra `None` neu **bat ky** nen trong
  cua so co `volume` NULL **hoac** `<= 0`. ⇒ 0 lenh. Cung dung.
- `vwap_fade.rs` -> `fd-indicators/src/lib.rs:800` —
  `let v = match bar.volume { Some(v) if v > 0.0 => v, _ => 1.0 }`.
  ⇒ **khong** 0 lenh: cot 0/NULL **im lang bien VWAP thanh TWAP** va co che
  chay binh thuong voi mot chi bao khac han cai no khai. Day la trang thai thu
  ba cua "doc mot cot so khong", va no **te hon** 0 lenh vi no in ra so.
  (`anchored_vwap()` thi khong he can nang volume — doc string tu khai dieu do.)
- `volman_box.rs` — **`grep volume` tren file: 4 ket qua, CA BON trong `mod
  tests`, ca bon la `volume: None`.** `on_bar` khong doc `volume`. Xac nhan
  bang so: no lay **6.070 va 255 lenh tren `xauduka:5m`**, feed co cot 0 tuyet
  doi. ⇒ **F2 BAN.** Va "volman-box <= 1 lenh" khong co trong ho so: cac dong
  da cong bo la 6.070 / 255 / 441 / 181.

## III. Tien kiem 3 — "0-4 lenh" den tu dau, va F3 ban

`grep` moi receipt cua bon co che do. Dong `--market=` tach lam hai nhom sach:

    --market=xauduka  (cot 0)    "volume-thrust took no trades; there is
                                  nothing to compare"   <- dung 0, nhu du doan
    --market=xauusd   (co vol)   6 lenh (ca gio, 1 nam) va 1 lenh (gio NY)
    --market=btc      (co vol)   677 / 362 / 179 lenh
    --market=xauusd   (co vol)   241 / 57 / 43 lenh  (cua so 2022-06..2025-04)

⇒ **"0-4 lenh" la hai thu bi tron lam mot.** Tren DUKA no la 0 that va vi cot
so khong. Tren `xauusd` no la **1 va 6** — va khong vi cot, ma vi cua so chi
1 nam (23.531 nen) va dieu kien vao rat hiem. **Tren cung instrument voi cua
so 2,8 nam, cung co che ra 241 lenh.**

⇒ **F3 BAN: ho volume DA duoc do tren feed co volume, hai lan, voi co mau du.**
Tien de "chua tung duoc do" sai. Toi tieu **0 o cong** va chi **chay lai** cac
dong da cong bo bang dung cu da va.

## IV. PHAT HIEN CHINH — tick volume cua feed sang khong dung yen

Day la thu dang giu nhat cua job, va no khong phai mot ket qua P&L.

`volume-thrust` doi `volume >= 2,5 x trung binh 20 nen`. Dem dieu kien do
**truc tiep tren cot**, XAUUSD-15m, toan bo 100.586 nen:

    nam   nen     vol>=2,5x  ty le%   lenh day du   tren 1k nen
    2022  12.865       149     1,16%          53         4,12
    2023  23.560       802     3,40%         234         9,93
    2024  23.716        79     0,33%          29         1,22
    2025  23.619        99     0,42%          39         1,65
    2026  16.826         6     0,04%           2         0,12

**85 lan it hon giua 2023 va 2026.** Va nguyen nhan doc duoc o hinh dang cot:

    nam   trung vi tick/nen   p99/p50   he so bien thien
    2022        950             4,31         0,698
    2023      1.177             4,80         0,790
    2024        512             3,13         0,513
    2025      2.300             3,00         0,621
    2026      5.130           **1,76**     **0,312**

Nam 2026 cot nay **phang den muc mot ngoai le 2,5x gan nhu khong xay ra duoc**:
p99 chi cao hon trung vi 1,76 lan. Co che khong ngung hoat dong — **cot ngung
co duoi.**

Doi chung, BTCUSDT-15m (**khoi luong giao dich THAT**, Binance):

    2024  5,55% | 2025  6,02% | 2026  6,46%      p99/p50  8,45 / 10,02 / 9,31
    lenh day du tren 1k nen: 19,03 / 19,29 / 16,50     cv 1,161 / 1,344 / 1,275

⇒ **Khoi luong giao dich that giu duoi va giu ty le qua 3 nam. So dem tick cua
sang thi mat duoi.** Do la mot phat bieu ve FEED, khong phai ve thi truong.

### Vet noi, dinh vi duoc den thang

Thang qua thang, XAUUSD-15m trung vi tick/nen:

    2023-11  1.108   p99/p50 4,57   cv 0,744
    2023-12    400   p99/p50 2,75   cv 0,450     <- x0,36 trong MOT thang

Sau do trung vi bo monotone 749 -> 5.179 (x6,9 tu 2024-11 den 2026-09) trong
khi p99/p50 bi ket o 1,5-2,0 va cv o 0,28-0,33. Truoc vet: p99/p50 ~4-5,
cv ~0,6-0,8. **Khong phai mot bien co thi truong trong mot thang; day la mot
vet noi cua feed/export.** Hai che do:

    che do A  2022-06-16..2023-11-30   34.589 nen   286 lenh   8,27 / 1k nen
    che do B  2023-12-01..2026-09-17   65.997 nen    71 lenh   1,08 / 1k nen

**Ty le 7,7 lan qua vet noi.**

### Hau qua: khong co cach chia IS/OOS theo thoi gian nao doc duoc

357 tin hieu tren toan bo 4,25 nam. Chia thu:

    cut 2023-06-16:  151 / 206    cut 2024-07-01:  302 /  55
    cut 2024-01-01:  287 /  70    cut 2024-12-31:  316 /  41
    cut 2025-04-11:  321 /  36   <- cach chia HO SO DA KHAI: nua sau **36**

Moi cach chia dat du 40 tin hieu moi nua **deu phai cat BEN TRONG che do A**,
nen nua dau chua ca hai che do va nua sau chi co che do B. ⇒ **mot phep chia
theo thoi gian cua feed nay la mot phep chia GIUA HAI CACH DO "volume", khong
phai hai mau thoi gian cua mot thi truong.** Day la hien vat cua so **thu 12**:
khong phai tin hieu lat dau giua hai nua, ma **tien de cua tin hieu khong ton
tai o nua sau.**

Va 36 tin hieu tho thanh **14 lenh** sau hai filter da khai (`weekdays`,
`flat:1630-1815`) + warmup. Duoi ca vach `need 30` cua tool, chua noi cong 40.

## V. Chay lai — so cua toi, ca hai arm, ca hai cua so

### XAUUSD-15m (tick volume MT5), cach chia cua ho so

    arm        cua so  dong            n    PF_usd   PF_r      E (R)   Lbar(R)   DD USD    DD%
    no-guards  IS      vt-gold/fixed  241   0,9520  0,9607   -0,0190   0,4824    22,17   21,40%
    no-guards  IS      vt-gold/all     57   0,9651  0,9896   -0,0046   0,4407     8,05    7,70%
    no-guards  IS      vt-gold/ny      43   0,7968  0,8063   -0,0873   0,4504     7,18    7,03%
    guards     IS      vt-gold/fixed  235   1,0045  0,9669   -0,0160   0,4831     9,96    9,73%
    guards     IS      vt-gold/all     46   0,8821  0,8278   -0,0777   0,4515     4,06    4,02%
    guards     IS      vt-gold/ny      77   0,9474  0,8445   -0,0663   0,4263     3,84    3,76%
    no-guards  OOS     vt-gold/fixed   14   2,9122  2,9636   +0,3003   0,1529     1,05    1,01%
    no-guards  OOS     vt-gold/all     11   2,3702  2,4089   +0,2419   0,1717     1,05    1,01%
    no-guards  OOS     vt-gold/ny      18   1,1053  1,1072   +0,0427   0,3981     3,39    3,33%
    guards     OOS     vt-gold/fixed   11   2,4096  2,4869   +0,2894   0,1946     0,78    0,76%
    guards     OOS     vt-gold/all      8   1,8581  1,8685   +0,2050   0,2361     0,78    0,76%
    guards     OOS     vt-gold/ny      16   0,8878  0,9045   -0,0454   0,4752     2,49    2,45%

**Cua so duy nhat co >= 40 lenh la cua so no LO. Cua so co PF cao thi 8-18
lenh** — duoi `need 30`, duoi cong 40, §4 noi **khong ket luan**. Cong doi
qua **CA HAI** cua so ⇒ **0 dong qua cong, ca hai arm.**

⚠️ **Khuyet diem 17 ap dung nguyen**: `xauusd` la **so cent 100 USD voi lot bi
kep o min_lot**, nen moi `PF_usd` o bang nay la PF cua mot **so 0,01 lot**,
khong phai so rui ro 1%. `_pct` cao nhat 21,40% < 100% ⇒ khong dong nao chay.

⚠️ **Mot dong lat dau giua hai don vi**: `guards IS vt-gold/fixed` in
**PF_usd 1,0045 (tren 1) va PF_r 0,9669 (duoi 1)** tren **dung 235 lenh**, va
`net +0,30 USD` canh `E -0,0160 R`. Ai doc PF_usd thay mot so lai; so R noi
no lo. Khoang cach lon nhat toi do: **-0,1029 PF** (`guards IS vt-gold/ny`,
PF_usd 0,9474 vs PF_r 0,8445).

### BTCUSDT-15m (khoi luong giao dich THAT) — feed duy nhat dung yen

    arm        cua so  dong            n    PF_usd   PF_r      E (R)   Lbar(R)      DD USD    DD%
    no-guards  IS      vt-btc/fixed   677   0,9924  1,0040   +0,0020   0,5037    3.542,62  29,78%
    no-guards  IS      vt-btc/all     362   1,1193  1,1302   +0,0641   0,4921    1.373,72  10,43%
    no-guards  IS      vt-btc/us      179   1,0359  1,0407   +0,0195   0,4789    1.592,15  14,14%
    guards     IS      vt-btc/fixed   557   1,0165  1,0074   +0,0037   0,4996    2.898,53  23,50%
    guards     IS      vt-btc/all     403   1,0307  1,0510   +0,0265   0,5190    1.854,79  15,75%
    guards     IS      vt-btc/us      205   1,1430  1,1597   +0,0757   0,4743      841,31   7,23%
    no-guards  OOS     vt-btc/fixed   313   0,9828  0,9913   -0,0043   0,4891    1.836,44  17,31%
    no-guards  OOS     vt-btc/all     397   0,9734  0,9769   -0,0119   0,5177    2.439,39  22,53%
    no-guards  OOS     vt-btc/us       40   1,7853  1,7955   +0,3121   0,3923      466,45   4,14%  SURVIVES
    guards     OOS     vt-btc/fixed   272   1,0221  1,0129   +0,0064   0,4925    1.538,52  14,71%
    guards     OOS     vt-btc/all     290   1,0487  1,0544   +0,0277   0,5098    2.029,08  18,79%
    guards     OOS     vt-btc/us       39   1,8751  1,8809   +0,3384   0,3841      397,49   3,51%  SURVIVES

**Dong `SURVIVES` duy nhat ca job — va no khong phai ung vien, vi bon ly do
doc duoc tu chinh receipt cua no:**

1. **n = 40** (no-guards) va **n = 39** (guards). Arm chu cho phep la arm
   co guards ⇒ o do no **truot chan so lenh cua cong desk (40)**, va tool in
   `SURVIVES` vi nguong cua tool la 30 (bay §4).
2. **count match 2,24 / 2,28 — NGOAI BANG.** Theo §4 va phu luc 6 VI:
   **khong cong bo phan vi.** Cai `100%` khong dung duoc.
3. **Cung dong, cua so IS: PF_r 1,0407 / 1,1597** — duoi 1,200. Cong doi **ca
   hai** cua so ⇒ dong nay **TRUOT cong**. `SURVIVES` la per-window.
4. 179 lenh IS (15,6 thang) -> 40 lenh OOS (8,4 thang): 11,5/thang -> 4,8/thang.

**Luat cua co che CO no** (bay `tsmom/120d` khong ap): `exits:` cua moi dong
deu co STOP va TARGET that — vd `no-guards IS vt-gold/fixed` STOP 103 /
TARGET 67 / TIMEOUT 70 / flat 1, va dong `SURVIVES` STOP 14 / TARGET 12 /
TIMEOUT 14. STOP+TARGET = 65-71% so lan thoat. Day khong phai mot dong ma
luat cua no no 0 lan.

## VI. Hang dang thuc va don vi — xac nhan va mot cho SUA phu luc truoc

`E = Lbar x (PF_r - 1)` dung **24/24 dong** toi chay, phan du lon nhat
**0,00005 R**. Xac nhan phu luc 7 muc A.

`PF_usd` doc **CAO hon** `PF_r` o **19/24 dong (79%)**. Phu luc 8 muc III.1 do
320/452 = 71% va noi "doc bang USD lech LEN, khong mang dau cua so" —
**so cua toi xac nhan phu luc 8 va bac phu luc 7A**. Them mot chi tiet: ca
**5** dong lech xuong nam o **arm co guards** (3/3 dong vang guards IS va
2/6 dong BTC guards), khong dong nao o arm khong-guards. ⇒ **huong lech co
lien quan den guards, khong den dau cua so.**

Engine tu in `expectancy leg REDUNDANT (Lbar >= 0,250R)` tren 19/24 dong va
`BINDS (Lbar < 0,250R)` tren dung 5 dong mong (`Lbar` 0,153-0,236) — xac nhan
hang dang thuc cong cua phu luc 6 muc I, truc tiep tu receipt.

## VII. Hai cho phai goi dung ten

1. **Toi do TICK VOLUME, khong phai dong tien.** Va day khong phai suy dien:
   `py/ingest/mt5_export.py:33-34` tu khai — *"`volume` holds MT5 **tick**
   volume — the number of price changes in the bar — because **CFD real volume
   is always zero**"*. `fd-core/src/market.rs` khai `BarSource::Mt5` la
   "the broker's own OHLC with **tick volume**". ⇒ `volume-thrust` tren
   `xauusd`/`eurusd`/`btcusd` la mot co che ve **tan so bao gia**, khong phai
   ve dong tien. Chi `BarSource::Binance` (BTCUSDT) la khoi luong giao dich
   that. Mot co che tan-so-bao-gia khong vo gia tri, nhung goi no la
   "volume thrust" la goi sai ten, va bang muc IV cho thay **gia dat cua viec
   goi sai ten**: dai luong do bi nha mang doi ma khong ai thong bao.
2. **Ho volume bi bac bo lan dau MOT CACH HOP LE o day — nhung khong phai
   tren vang.** Tren `btc:15m`, khoi luong giao dich that, cot dung yen 3 nam,
   co mau 677+362+179 IS va 313+397+40 OOS, ca hai arm: **0 dong qua cong
   tren CA HAI cua so.** Do la ket luan sach, va no la cua BTCUSDT. Tren
   `xauusd` ket luan sach **khong lay duoc** va se khong lay duoc, vi ly do o
   muc IV: nua sau cua so khong con tin hieu de do.

## VIII. Thu KHONG do duoc, va vi sao

- **`volume-thrust` tren DUKA, bat ky cua so 16 nam nao.** 0 lenh tren
  378.749 nen (15m) va 0 kha nang tren 5.635.777 nen (1m, NULL). Khong phai
  "khong co tin hieu" — **khong co du lieu**. In ra la `null`, khong phai 0.
- **`rsi-reversal-vol` chua duoc do o dau ca.** `grep` khong ra mot receipt
  nao. Tren DUKA no se cho 0 lenh (muc II). Tren `xauusd` no chua tung chay.
  Day la cho con lai duy nhat cua ho nay, va no nam tren cung cai cot khong
  dung yen ⇒ **toi khong de xuat no.**
- **Nguyen nhan cua vet noi 2023-12.** Toi dinh vi duoc vet (x0,36 trung vi,
  cv 0,744 -> 0,450, trong mot thang) nhung khong truy duoc no den mot lan
  chay `mt5_export.py` cu the; file parquet khong giu nhat ky nguon tung
  tranche. Doc string noi "the export is re-run to extend it" ⇒ gia thuyet la
  hai tranche export khac nhau, **nhung toi khong chay ra bang chung.**
- **`vwap/allday` == `vwap/wide` trong receipt 2026-09-13 (ca hai 5.828 lenh,
  PF 0,951, E -0,029) khong tai lap duoc.** Toi do `stretchAtr` **song** tren
  binary da va, ca qua `--params=` (8.366 lenh @1,5 vs 6.382 @2,5) **va** qua
  `overrides` trong batch file walk-forward (5.700 @1,5 vs 4.966 @2,5). ⇒ hoac
  da duoc va, hoac la mot hien vat cua binary cu. Dat vao so "ho so khong chay
  lai ve dung so cua chinh no" (phu luc 8 muc II), khong phai mot khuyet diem
  moi toi chung minh duoc.
- **Drawdown trong lenh.** `max_drawdown_*` la duong von **da dong lenh**;
  `avg_mae` (-0,44 den -0,72 R tren cac dong) la field duy nhat thay
  excursion. Khong doi.

## IX. So dang giu nhat, ke ca khi am

    XAUUSD-15m, dieu kien vao cua volume-thrust, dem truc tiep tren cot:
      2023:  3,40% so nen        2026:  0,04% so nen        => 85 lan
    BTCUSDT-15m, cung dieu kien, khoi luong giao dich that:
      2024:  5,55%   2025: 6,02%   2026: 6,46%              => dung yen

Mot cot dung yen va mot cot khong, tren cung mot phep thu, cung mot co nen.
