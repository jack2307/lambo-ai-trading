# Dang ky truoc — volume profile tren BTCUSDT (khoi luong THAT)

Ngay: 2026-10-10. Nhanh: `agent/vprofile-btc` (cat tu `agent/stop-width`).
Binary: `/e/rust/fd-stop-width/target-sw/release/search.exe` (co `lbar_line` +
drawdown) cho moi dong qua cong; code moi build trong worktree nay.

## 0. Vi sao BTCUSDT va vi sao job nay rieng

Phu luc 8 muc I do duoc `volume > 0` o **0/50.000** mau tren moi feed Dukascopy,
va **moi cua so 16 nam cua ho so la DUKA** ⇒ khong dung duoc profile nao tren
do. `XAUUSD` co **tick** volume (`py/ingest/mt5_export.py:33-34` tu khai the, vi
"CFD real volume is always zero") va no **mat duoi**. `BTCUSDT` la khoi luong
**THAT** cua Binance, cot **dung yen** qua ba nam, va la instrument **re nhat**
da do. Day la cho duy nhat mot volume profile co co hoi.

Do lai ngay hom nay, khong trich: `BTCUSDT-15m` = **70.080 nen**,
2024-09-12 10:15 UTC den 2026-09-12 10:00 UTC, `volume > 0` o **70.080/70.080**,
`volume = NULL` o **0** dong. ATR14(15m) **trung vi 226,74 USD** (p10 105,89 /
p90 470,62). Spread btc = **5,0 USD vong**, lot_step 0,001, min_lot 0,001.

## 1. Gia thuyet (mot cau)

Tren BTCUSDT-15m, cac muc cua **volume profile** — POC, VAH, VAL cua ky truoc —
la nhung muc gia mot co che co the mua/ban quanh va qua duoc cong desk tren
**ca hai** nua 1 nam, **va** chung khac duoc voi cung cac muc dung tu **time
profile** (TPO, trong so deu).

## 2. VIEC DAU TIEN: cai da co

`crates/fd-engine/src/price_levels.rs:449 activity_profile()` **da la** mot
profile gia theo nen voi POC/VAH/VAL bang dung "classic walk". Nhung no **tu
khai la TIME-AT-PRICE, khong phai volume** (`BarProfile::measure =
"TIME_AT_PRICE"`, va doc-comment cua chinh no noi: mot profile dung tren cot
volume cua cac feed nay "would be a time profile wearing a volume profile's
name, so this one is a time profile wearing its own").
`crates/fd-engine/src/profile.rs` la profile **STRIKE cua options** (trong so
premium/contracts), khong phai gia. Va `activity_profile` **chi** duoc goi boi
route bao cao `/api/paper/levels` (`crates/fd-api/src/levels.rs:751`) —
**khong co chien luoc backtest nao doc no**. Nen: **nhanh TIME cua tien kiem DA
TON TAI va trung thuc; nhanh VOLUME chua ton tai o dau.** Viec cua toi la
**them dung cot trong so** vao cung mot thuat toan, khong viet lai POC/VA.

Khac biet so voi dinh nghia chuan, noi ro: `activity_profile` cong **1,0 vao
MOI bucket** ma bien do low..high cua nen cham (dung dinh nghia TPO market
profile), POC la **bucket**, gia POC la **trung diem** bucket, VAH la **bien
NGOAI** cua bucket ngoai cung, VAL la **bien duoi**. Hoa giai tie ve bucket
**THAP hon**.

## 3. TIEN KIEM QUYET DINH (0 o cong)

Dung **ba** profile tren **cung** bars, **cung** luoi bucket, chi khac trong so:

    TPO   : w_k += 1.0                            cho moi bucket nen cham (da co)
    VOLd  : w_k += volume / (so bucket nen cham)  bao toan tong volume, dinh
                                                  nghia volume profile chuan
    VOLt  : w_k += volume                         tuong tu hinh TPO, nen rong
                                                  duoc dem nhieu lan

Luoi: `bucket = ATR14(15m)/4` (hang so `BUCKETS_PER_ATR = 4.0` cua route da co),
`value_area_pct = 0,70`. Cua so profile: **1 ngay UTC** va **5 ngay UTC** (hai
co, khai truoc), moi cua so chi dung **nen da dong** cua ky TRUOC.

Do, bang **khoang cach gia**, khong bang mat, tren moi cua so:
`|POC_vol - POC_tpo|`, `|VAH_vol - VAH_tpo|`, `|VAL_vol - VAL_tpo|` — bao
**USD**, **so bucket**, **so tick** (tick = 0,01 USD), va **% cua 1,5xATR14(15m)
cua cua so do** (tuc cua mot R dien hinh).

**F1a (dong nhat):** neu >= 95% cua so cho POC_vol va POC_tpo **cung mot
bucket** VA p90 cua `|POC_vol - POC_tpo|` <= **1 tick** (va the voi VAH/VAL) thi
"volume profile" o day **chinh la** time profile, cot volume khong them gi ⇒
**dong o tien kiem, tieu 0 o cong.**

**F1b (khong phan biet duoc ve VAN HANH):** neu **trung vi** cua ca ba khoang
cach < **10% cua 1,5xATR14** thi hai bo muc la cung mot bo muc doi voi bat ky
co che nao, vi khe nho hon sai so mot mau 40 lenh phan giai duoc ⇒ **dong o
tien kiem, tieu 0 o cong**, va noi ro day la "ve van hanh", khong phai
"dong nhat".

**F3 (bay `vwap_fade`):** `fd-indicators/src/lib.rs:800` lam
`match bar.volume { Some(v) if v > 0.0 => v, _ => 1.0 }` nen **cot 0/NULL bien
VWAP thanh TWAP va van in so binh thuong.** Vi the toi in **bo dem**: so nen co
`volume` doc duoc, so nen roi ve 1,0, va `sum(w)` cua ca ba profile. Neu
`sum(w_VOLd) == window_bars` thi nhanh volume **chua chay** — do la khuyet diem,
khong phai ket qua. **Khong co dong nao duoc doc truoc khi bo dem nay in ra.**

## 4. Co che (khai TRUOC khi do), neu tien kiem qua

`vprofile`, `Exits::Engine`, hai doc:

* `mode = 0` **quay ve POC**: nen dong **ngoai** value area cua ky truoc thi vao
  nguoc huong, **target = gia POC** (mot MUC GIA), stop = bien VA phia ngoai
  cong `bufferAtr x ATR14`.
* `mode = 1` **tu choi o bien**: nen cham VAH (hoac VAL) roi **dong lai ben
  trong** VA thi vao nguoc huong, **target = gia POC**, stop = bien do nen cong
  `bufferAtr x ATR14`.

**Phan lop stop theo muc 5 cua brief, doc bang `Exits::` trong ma, khong bang
ten tham so:** `Exits::Engine` (khong `self_managed`) cong `Intent::Enter` mang
**ca `stop` va `target` la MUC GIA tuyet doi** nen day la **L3** — lop duy nhat
"noi stop" co cau tra loi sach. Noi ro truoc: `bufferAtr` o day **doi tap
lenh**, khong phai doi don vi.

Mot lenh moi muc moi ky (POC/VAH/VAL), nhu `pdhl` lam voi PDH/PDL.

## 5. Da phep thu — DEM TRUOC

    measure     : VOLd, TPO                 = 2   (tuong phan nay CHINH LA phep thu)
    mode        : 0, 1                      = 2
    profileDays : 1, 5                      = 2
    bufferAtr   : 0,25 / 0,5 / 1,0          = 3
    ------------------------------------------------
    o tham so                               = 24
    x 2 cua so (hai nua 1 nam)              = 48 dong
    x 2 arm (co guards / khong)             = 96 dong in ra

**Arm co guards la arm duy nhat chu cho phep** (phu luc 5 muc D, phu luc 7 muc
D). Arm khong-guards chi in de so sanh; neu ket qua chi song o do thi **no
khong giao dich duoc** va toi noi thang the.

Voi 24 o, so o qua ngau nhien o mot vach p95 ky vong khoang 1,2, nen **mot o
qua MOT cua so la nhieu**. Tieu chi da khai: **cung mot o qua cong tren CA HAI
cua so**, va toi bao dem o nhu the **tren 24**.

Hai cua so chia theo thoi gian, khong chong nhau:

    W1 = 2024-09-12 -> 2025-09-12
    W2 = 2025-09-12 -> 2026-09-12

## 6. Cach doc — cac bay da biet, khai truoc

* **`PF_r` canh `PF_usd`, khai don vi.** `E = total_r/n`, **khong** doc
  `expectancy` 3 chu so (phu luc 8 muc IV: hieu hai so gan nhau, lam tron co the
  lon hon dai luong). `PF_r = 1 + E/Lbar` tu `lbar_line`. Chieu khe USD-vs-R do
  **tung dong**, khong gia dinh.
* **Drawdown USD canh moi so loi nhuan.** `_pct` la SAN. `_pct > 100%` nghia la
  dong **da chay**, khong doc PF cua no. Kiem `cap_lots` (phu luc 7 muc C: mot o
  cuoi tran o 98,1% so lenh va **ca hai chan cong mu voi no**).
* **`--exit-mix` va kiem luat cua co che CO NO** (bay `tsmom/120d`: PF 2,236
  cong `SURVIVES` voi luat rieng no **0 lan**). Neu exit cua toi toan
  `NEWS_FLAT/WEEKEND_FLAT/END_OF_DATA` thi co che **chua duoc do**.
* **Cong desk la 40 lenh**, tool kiem 30 (`need 30`), nen **dem bang tay**. Duoi
  khoang 40 lenh **khong ket luan**.
* **Khong cong bo phan vi** tru khi `count match` trong bang va co ly do manh:
  `SURVIVES` phu thuoc **cho ngoi trong file TOML** (null p95 1,297 vs 2,036).
  `null p50 = 0,000` nghia la **khong calibrate duoc**.
* **Chi phi/R lay tu dong `cost-matched null: ... cost X% of R` ma output tu
  in**, va **luon noi ca ba**: cua so cong thong ke cong co stop. (Uoc luong doc
  lap cua toi hom nay, de doi chieu: spread 5,0 chia cho 1,5 x ATR14(15m)
  **trung vi** 226,74 = **1,470% cua R**, cua so 2 nam day.)
* Khuyet diem 16/17 (nua spread ro vao don vi rui ro; moi dong `xauusd` chay o
  lot toi thieu): BTC co `min_lot = 0,001` nen phai **kiem lai** tren feed nay,
  khong gia dinh no khong xay ra.

## 7. FALSIFIER — tong

1. **F1a hoac F1b bat o tien kiem** nghia la volume profile **chinh la** time
   profile o day; dong, **tieu 0 o cong**, bao khoang cach da do.
2. **F3 bat** nghia la nhanh volume chua chay; bao khuyet diem, **khong** bao
   ket qua.
3. **Phan biet duoc nhung 0/24 o qua cong tren CA HAI cua so** (arm co guards)
   nghia la volume profile **bi bac bo o cho thuan loi nhat ton tai**: khoi
   luong that, cot dung yen, chi phi re nhat da do. Ket luan do manh hon moi
   phep do tren vang, va toi bao no nhu mot ket qua, khong nhu mot that bai.
4. Neu co o qua: no phai qua **ca hai** cua so, trong **arm co guards**, voi
   **luat rieng co no** trong `--exit-mix`, `_pct < 100%`, va `PF_r` (khong chi
   `PF_usd`) tren vach 1,200. Thieu bat ky dieu nao thi khong phai ung vien.

## 8. Khong lam

Khong mo `data-sealed/`. Khong cham `main`, `config/local.toml`,
`config/accounts.toml` ngoai worktree, hai `collect.exe`, hay VPS. Khong hua chi
tieu drawdown nao ngoai viec **in** no. Khong sua khuyet diem 14/16/17 — dem,
khong sua.
