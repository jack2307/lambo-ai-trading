# KET QUA — volume profile tren BTCUSDT: 0 o qua cong tren CA HAI cua so

Ngay: 2026-10-10. Nhanh `agent/vprofile-btc`. Dang ky:
`docs/decisions/2026-10-10-volume-profile-btc.md` (commit `176c8ca`, mot minh no,
truoc dong code dau tien). Binary: `target-vp/release/search.exe` build trong
worktree nay tu `agent/stop-width` — co `lbar_line`, drawdown, `--exit-mix`.
Binary dung chung cua phu luc 7 **khong biet** `vprofile` nen khong dung duoc.

Khong trich con so da cong bo nao. Moi so duoi day la tu run hom nay.

## 1. Mot cau

Volume profile — POC, VAH, VAL — **bi bac bo o cho thuan loi nhat ton tai**:
khoi luong **that** cua Binance, cot **dung yen**, instrument **re nhat** da do,
**0/24 o qua cong tren ca hai cua so 1 nam**, trong arm co guards (arm duy nhat
chu cho phep).

## 2. Tien kiem: PHAN BIET DUOC, nhung khong deu giua ba muc

730 cua so 1 ngay UTC, cung bars cung luoi (bucket = ATR14(15m)/4, trung vi
59,38 USD; 1R = 1,5xATR14 trung vi 356,26 USD; tick 0,01 USD):

    muc   cung-bucket   |d| trung vi            |d| p90       trung vi % cua 1R
    POC      60,7%     0,00 USD (0,00 bucket)   835,23 USD         0,00%
    VAH      30,1%    64,93 USD (1,09 bucket)   497,62 USD        18,23%
    VAL      32,3%    63,64 USD (1,07 bucket)   505,45 USD        17,86%

Cua so 5 ngay lech manh hon: POC cung-bucket 52,2%, VAH/VAL 19,4%/18,0%, trung
vi 36,30%/36,14% cua 1R.

F1a (>= 95% cung bucket **va** p90 <= 1 tick) **khong bat**. F1b (trung vi ca ba
< 10% cua 1R) **khong bat** — POC dat (0,00%) nhung VAH/VAL khong (18,23% /
17,86%). ⇒ tieu tiep 24 o nhu da khai.

**Con so dang giu nhat cua tien kiem:** khe khong deu. **POC co trung vi dung
0,00 USD** ⇒ mot co che neo **chi vao POC** gan nhu la **cung mot co che** duoi
hai measure; cot volume doi cau tra loi **o BIEN value area**, khong o POC. Dieu
nay khong duoc du doan trong dang ky.

F3 (bay `vwap_fade`) **khong bat**: `volume` doc duoc **70.080/70.080** nen
(100,000%), tong 15.948.687,245 don vi feed, **0 nen** roi ve 1,0.
`activity_profile_measured` **tra `None`** khi cot rong — co test giu.

## 3. Cong: 0 o, va day la con so chinh

Cong desk = `PF >= 1,200` VA `E >= +0,050R` VA `>= 40 lenh`, tren **ca hai** cua
so. W1 = 2024-09-12 -> 2025-09-12, W2 = 2025-09-12 -> 2026-09-12 (khong chong
nhau, dat bang CLI `--from/--to` vi `[run]` khong dat duoc cua so duoi
`--fixed`).

    arm              o   qua cong doc bang PF_r   doc bang PF_usd
    W1 co guards    24            0                     0
    W2 co guards    24            0                     0
    W1 khong guards 24            0                     0
    W2 khong guards 24            2                     1

**Arm co guards: 0/24 o ca hai cua so.** Moi dong co **156-352 lenh** — khong
dong nao duoi 40, nen khong co dong nao "khong ket luan".

Hai o qua o **W2 khong-guards** la o **arm chu da CAM** (`WEEKEND_FLAT` nen
khong bao gio giu qua tuan), **chi mot cua so**, va theo tieu chi da khai (cung
o qua **ca hai** cua so) chung dem la **0**. Them nua, mot trong hai la o
**TIME profile** (`vp-tpo-rej-5d-b10`), khong phai o volume — va no **lat dau**:
`PF_r 0,8749` o W1 khong-guards so voi `1,2488` o W2. Hien vat cua so, khong
phai co che.

Chan rang buoc la **PF**, khong phai expectancy: moi dong in
`expectancy leg REDUNDANT (Lbar >= 0,250R)`, dung nhu hang dang thuc
`E = Lbar x (PF_r - 1)` cua phu luc 6 du doan. Phan du lon nhat tren 96 dong:
**0,00005 R**.

### Ba dong gan nhat (va vi sao chung khong phai ung vien)

    o                     arm        W1 PF_r   W2 PF_r   W1 E       W2 E
    vp-vol-ret-5d-b05   co guards    1,0734    1,1680   +0,0535   +0,1171 R
    vp-vol-ret-5d-b05   khong guards 1,1321    1,2275   +0,0938   +0,1548 R
    vp-vol-ret-5d-b025  co guards    1,1250    1,0784   +0,1015   +0,0626 R
    vp-tpo-rej-5d-b10   co guards    0,8831    1,1306   -0,0577   +0,0573 R

`vp-vol-ret-5d-b05` la o **on dinh nhat ca luoi** — duong o ca bon o (hai cua
so x hai arm) — nhung **khong dat 1,200 trong arm co guards o cua so nao**.
Drawdown cua no: **1.367 USD = 11,97%** (W1) va **3.596 USD = 23,33%** (W2) tren
duong vong da dong lenh; khong-guards W2 thi **6.678 USD = 33,19%**.

**Va luat rieng cua no gan nhu khong no:** TARGET (= cham POC) dong chi
**6,0-7,4%** so vi the. Exits W2 co guards: STOP 168 / TARGET 15 / TIMEOUT 65.
⇒ o nay khong song bang "gia quay ve POC"; no song bang **stop va tran giu 4h**.
Day la bay §6(a) o dang nhe: luat co no, nhung **khong phai luat lam ra ket qua**.

Tren toan luoi, TARGET dong **3,2%-43,8%** so vi the (trung binh 11,2% o cac o
`return-to-value`, 26,5% o cac o `edge-rejection`) ⇒ **khong o nao la bay
`tsmom/120d`** (0 lan), nhung o ho 5 ngay thi luat rieng gan nhu im.

## 4. KET QUA MANH NHAT: dau cua cot volume LA MOT HIEN VAT CUA SO

Ghep tung o volume voi o time **song sinh** (cung mode, cung co cua so, cung
buffer), doc bang `PF_r`, arm co guards:

    W1: volume thang 8/12 song sinh, delta PF_r trung binh +0,0767
        theo mode: return-to-value +0,0301, edge-rejection +0,1233
    W2: volume thang 4/12 song sinh, delta PF_r trung binh -0,0238
        theo mode: return-to-value +0,0463, edge-rejection -0,0940

    cung DAU o ca hai cua so: 4/12 song sinh  (ngau nhien ky vong 6/12)

Trong mode `edge-rejection` — **dung cho tien kiem noi volume CO THE anh huong**
(VAH/VAL lech ~1,1 bucket) — volume thang **6/6** o W1 va thua **6/6** o W2. Lat
hoan toan.

⇒ Khong chi volume profile truot cong; **chieu dong gop cua cot volume la thuoc
tinh cua CUA SO**, khong phai cua thi truong. Day la **hien vat cua so thu 12**
cua ho so, va no la hien vat dau tien **ve mot cot du lieu**, khong ve mot tham
so hay mot muc.

## 5. `cap_lots` cuoi tran nang, va **ca hai chan cong mu voi no**

`sized down` (= `guards.rs:237 cap_lots`, tran `max_notional_pct_equity = 300%`)
bit tren **9,6% den 85,0%** so lenh, co he thong theo **do hep cua stop**:

    bufferAtr 0,25  ->  cap_lots bit 49,1-85,0% so lenh
    bufferAtr 0,50  ->  28,3-71,6%
    bufferAtr 1,00  ->   9,6-43,6%

⇒ `PF_usd` cua cac o stop hep la PF cua mot so **bi cat co lenh o 4 trong 5
lenh**, khong phai so rui ro 1%. `PF_r` va `E` khong bi anh huong (`r =
points/risk` khong doc `lots`) — **do dung la ly do phai bao ca hai**. Xac nhan
phu luc 7 §C tren mot ho o khac.

Khuyet diem 17 (moi dong `xauusd` chay o lot toi thieu) **khong xay ra o day**:
`markets.btc` khong ghi de `starting_equity_usd` nen no thua **10.000 USD** cua
`[trading]` (khong phai so cent 100 USD cua `markets.btcusd`), va `min_lot =
0,001` ⇒ khong dong nao bi kep san. Kiem lai thay vi gia dinh, dung luat.

## 6. `PF_usd` vs `PF_r`: khe khong phai hang so, va mot o lat vach

96 dong. `PF_usd - PF_r` di tu **-0,159** den **+0,135**; `PF_usd` doc **cao
hon** o **29/48** dong arm co guards (60,4%) — **khong** phai hang so, va khong
khop ty le 71% cua phu luc 8 §III.1. Do tung dong, dung luat.

Ca lat vach nang nhat: **`vp-tpo-rej-5d-b025` W2 co guards in `PF_usd 1,153`
nhung `PF_r 0,9945`** — khe **-0,159**, hai phia vach **1,000**. Doc bang USD
thi do la dong tot nhat W2; doc bang R thi no **lo**. Khong o nao lat dung vach
1,200, nhung co che lech la cai phu luc 7 §A dem.

## 7. Chi phi/R — ba con so, luon di cung nhau

Luon lay tu dong `cost-matched null: ... cost X% of R` ma output tu in, kem
**cua so + thong ke + co stop**:

    cua so W1+W2, trung vi da thuc hien, stop 0,492-1,881 ATR14(15m)
      ->  cost/R  3,96%  (stop 0,492 ATR)  ...  0,89%  (stop 1,881 ATR)

Nen con so «BTC 1,33% cua R» chi dung o **stop ~1,5 ATR14(15m)**; o stop ma co
che nay thuc su dung thi no **cao gap 1,1-3,0 lan**.

**Va mot canh bao ve chinh con so «instrument re nhat»:** ca run nay chay o
`markets.btc.trading.spread = 5.0`, ma `config/default.toml` tu ghi la **gia
dinh** cho thi truong Binance; `markets.btcusd.trading` ghi
`spread = 17.05  # MEASURED: 3.4x the 5.00 assumed`. Do lai tren o tot nhat
(`vp-vol-ret-5d-b05`, W2, co guards):

    spread  5,00 (gia dinh)  ->  PF_r 1,1680  E +0,1171 R  cost/R 2,35%
    spread 17,05 (DA DO)     ->  PF_r 1,0726  E +0,0523 R  cost/R 7,79%

⇒ o spread **da do** cua symbol tai khoan that giao dich duoc, chi phi la
**7,79% cua R** o stop 0,891 ATR — **cung tam voi vang**, khong phai «re nhat».
Nen tien de «chi phi re nhat» cua brief dung voi mot spread **gia dinh**; con so
da do thi khong ung ho no. (Luat §8: so do thang.)

## 8. Drawdown — khong dong nao chay

96 dong, `max_drawdown_pct` cao nhat **46,64%** (`vp-vol-ret-5d-b025`, W2
khong-guards, **11.038 USD**). **0 dong in `_pct > 100%`** ⇒ khong dong nao
giao dich qua moc chay tai khoan, nen `PF` cua moi dong **doc duoc** (khuyet
diem 14 khong xuat hien o day). Arm co guards: dd 930-4.169 USD = 7,96-36,32%.

## 9. Lop stop: L3, doc tu ma

`VolumeProfileLevels::exits() == Exits::Engine` (khong `self_managed`, nen
`engine.rs` thi hanh stop that) va `Intent::Enter` mang **`stop: Some(<gia>)`
cong `target: Some(POC)` — ca hai la muc gia tuyet doi** ⇒ **L3** theo phan lop
cua brief §5. Khang dinh bang so: `bufferAtr` **doi tap lenh** (184-352 lenh qua
ba gia tri), doi `PF`, doi `cost/R` (0,89-3,96%) va doi co stop da thuc hien
(0,492-1,881 ATR) — **khong** chi doi don vi. Day khong phai mot quet don vi.

**`wrong_side_stop`:** bo dem **khong ton tai tren cay nay** (`grep -rn
wrong_side_stop crates/` = 0 ket qua), dung nhu phu luc 8 §III.2 noi, nen toi
**khong** bao mot so 0 doc tu no. Thay vao do co che **tu choi vao lenh** khi
stop khong o phia lo hoac khi POC o phia lo cua entry — mot tinh chat cua ma, co
test giu, khong phai mot phep do.

## 10. Da phep thu: khai vs xem

    khai truoc (dang ky §5):  24 o tham so x 2 cua so x 2 arm = 96 dong
    da xem:                   24 o tham so x 2 cua so x 2 arm = 96 dong
    luoi co test giu dung 24 o  (the_grid_is_the_twenty_four_cells_the_registration_declared)

Khong them o nao, khong bo o nao. **Khong cong bo phan vi**: `SURVIVES` da do
duoc la phu thuoc **cho ngoi trong file TOML**, nen file batch co cho ngoi co
dinh (alphabet theo label) va bao cao nay doc **cong**, khong doc phan vi. Ghi
cho du lieu: `seeds = 10` (arm co guards) va `4` (khong-guards), **khong du de
cong bo p95** va khong duoc dung lam vay.

## 11. Thu KHONG do duoc, va vi sao

* **Phan vi / `SURVIVES`** — co y khong do: xem §10.
* **`wrong_side_stop`** — bo dem khong ton tai tren cay nay.
* **Excursion trong lenh** — `max_drawdown_*` chi doc duong vong **da dong
  lenh**; `avg_mae` (−0,82 den −0,90 R tren cac o do) la truong duy nhat thay
  excursion.
* **Volume profile tren vang / 16 nam cua ho so** — **khong do duoc het**, vi
  moi feed Dukascopy co `volume > 0` o 0/50.000 mau. Ket luan cua bao cao nay
  **khong** mo rong sang «volume profile tren vang bi bac bo»; no noi rang o cho
  co khoi luong that, no khong qua.
* **Tran giu 4h** — TIMEOUT dong **8,5-41,7%** so vi the. Phu luc goc do duoc
  rang chieu cua «dong ho» chi vuot phan vi 95 o dung hai muc tran 4h/8h, nen
  mot phan ket qua o day **nam trong mot tran** toi khong go. Khong do.
* **Volume cua cac co nen khac** (5m co 218.880 nen) — khong chay; mot «thuoc
  nen» khac la hien vat so 3 cua ho so va mot cua so ngan khong du de tach no.

## 12. Falsifier: bat o dau

* **F1a / F1b (tien kiem): KHONG bat** — hai profile phan biet duoc. Tieu 24 o
  nhu da khai.
* **F3 (bay volume rong): KHONG bat** — 70.080/70.080 nen co volume.
* **Falsifier 3 cua dang ky §7: BAT.** Phan biet duoc nhung **0/24 o qua cong
  tren ca hai cua so trong arm co guards** ⇒ volume profile **bi bac bo o cho
  thuan loi nhat ton tai**. Va manh hon the: §4 do duoc rang **chieu dong gop
  cua cot volume lat dau giua hai cua so o 8/12 song sinh** — nen ngay ca cai
  «volume tot hon time» cua W1 cung khong phai mot phat hien, no la hien vat
  cua so thu 12.

## 13. Ma da them (va khong sua gi dang chay)

* `crates/fd-engine/src/price_levels.rs`: `ProfileMeasure` (3 measure) +
  `ProfileAudit` + `activity_profile_measured`. `activity_profile` cu gio la
  wrapper; test `the_time_measure_reproduces_the_profile_published_before_the_volume_arms`
  giu cho no **y nguyen** (so sanh `==` ca `BarProfile`). **Khong** thay 1,0 cho
  volume thieu — test
  `a_volume_measure_on_an_empty_column_returns_nothing_rather_than_a_time_profile`
  giu dieu do.
* `crates/fd-strategy/src/vprofile.rs`: co che `vprofile` (L3), 8 test.
* `crates/fd-backtest/src/bin/vprofile_precheck.rs`: dung cu tien kiem.
* `docs/research/designs/2026-10-10-vprofile-btc.toml`: 24 o.

`/api/paper/levels` **khong doi hanh vi**: no van goi `activity_profile` va van
in `measure = "TIME_AT_PRICE"`. Khong cham `main`, `config/local.toml`,
`config/accounts.toml`, `data-sealed/`, hai `collect.exe`, hay VPS.
`--data=` chi doc.
