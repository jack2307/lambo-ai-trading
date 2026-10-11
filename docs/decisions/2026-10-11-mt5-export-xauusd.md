# 2026-10-11 — Mo rong nen XAUUSD bang `py/ingest/mt5_export.py`

Nhanh `agent/mt5-export`, cat tu `29c8635`.
Chu da CHO PHEP chay `py/ingest/mt5_export.py` cho XAUUSD
(`AGENT-BRIEF-ADDENDUM-11` §0.2).

## Gia thuyet

Nen `XAUUSD-15m` dong bang o `2026-09-17 13:00 UTC` con tape options chay toi
`2026-10-09 20:59 UTC`, nen **39,55% print (21.803/55.131) nam SAU nen cuoi** va
ho options vang khong bao gio co cua so thu hai. Keo nen toi hien tai mua duoc
cua so thu hai **neu va chi neu phan moi la CUNG MOT CHUOI** voi phan cu.

## Falsifier — cu the va ban duoc

Neu nen moi **khong cung hinh dang** voi nen cu o **mot** trong ba phep kiem A,
thi mo rong kho nay **khong mua duoc cua so thu hai — no mua mot DUONG MAY**, va
moi ket qua hai-cua-so chay tren no la hien vat cua duong may do. Khi do ho so
phai coi phan moi la mot **instrument KHAC**, khong phai cung mot chuoi dai hon.

Ba vach ban duoc, moi vach mot con so:

1. **A1 cot volume.** `XAUUSD` la tick volume (`mt5_export.py:33-34`). Vach:
   phan moi phai doc **cung ti le zero va cung ti le null** theo nam nhu phan cu
   (cu: **0,00% / 0,00% o ca 5 nam**). Mot nam moi doc zero > 0% nghia la kho
   tron hai cot, dung tien le `BTCUSD-15m` (zero 100% 2023-24 -> 42,1% 2025 ->
   0% 2026).
2. **A2 muc va hinh dang volume.** KS `D` giua nam cuoi cua phan cu va phan moi
   phai **khong lon hon** `D` lon nhat giua hai nam lien ke trong phan cu
   (do duoc: **0,8237** o `2024->2025`).
3. **A3 dau gia nha cung cap.** `open == low` o **dung nen 18:00 NY** phai giu
   lift so voi nen thuong. Nen cu: **6,74%** (74/1.098) so voi nen
   **2,73%** (2.623/96.194), **lift x2,47**.
4. **B duong may, doc THEO BAR.** Tren cac dau thoi gian TRUNG nhau giua truoc
   va sau, so bar co **OHLC khac nhau phai = 0**. Khac 0 nghia la merge da
   **ghi lai** nhung bar mot receipt da cong bo co the dang dung tren do.
5. **C voi toi duoc khong.** In so bar, % print nam trong vung co nen, va so
   block tach theo thoi gian cua tape. Tra loi duoc/khong duoc **bang so bar**.

## Da phep thu — dem O TRUOC

Job nay **khong tieu o cong nao**: 0 o backtest, 0 lan goi `search.exe`.
Khai truoc: **5 vach** (A1, A2, A3, B, C) tren **1 cap file** (truoc/sau) cua
**1 store** (`XAUUSD-15m`), cong **2 doi chung** (dong nhat, va probe co y nhin).
Tong **7 phep doc**, khong co phan vi, khong co verdict cong.

## Cach doc

- `null` != `0`. Khong do duoc thi in `null` (gini cua mot cot toan 0 in `null`).
- Moi con so co don vi: bar, print, %, USD, gio.
- **Doc theo BAR, khong theo frame** (`OptionsTimeline` 6.010/9.491 frame trung
  dau thoi gian = 63%).
- **`fd_core::clock::new_york_local` tra `(weekday, minute-of-day)`**, KHONG
  phai `(hour, minute)` — da xac minh `crates/fd-core/src/clock.rs:78-83`.
  Phep kiem A3 o day dung `zoneinfo` cua Python, khong dung ham do.
- **Be rong lop quyet dinh ket luan** o A3 ⇒ in CA HAI be rong, khong bao gio
  trich mot cai.

## Ghi chu 2026-10-11, sau tien kiem — FALSIFIER BAN O TIEN KIEM, TIEU 0 O

Ba thu phai kiem truoc khi chay (brief yeu cau) **cho ra ba cho chan**, va hai
trong ba la quyet dinh cua chu:

### 1. Terminal MT5 CHUA DANG NHAP — chan cung

- Terminal dang chay: **`C:\MT5-demo\terminal64.exe /portable`**, pid **71572**,
  khoi dong **2026-10-11 20:28:24**. (Khong phai `C:\Program Files\MetaTrader 5`
  nhu brief ghi — do la ban CAI, khong phai ban DANG CHAY.)
- `mt5.initialize()` tra **`(-10005, 'IPC timeout')`**. Da kiem lai danh sach
  process sau do: **khong co terminal nao bi khoi dong them**, van dung pid 71572.
- Journal hom nay (`C:\MT5-demo\logs\20261011.log`, **856 byte**) chi co:
  `Terminal ... started`, `MCP started on 127.0.0.1:22346`,
  `MQL5.community authorization failed` x2.
  **KHONG co mot dong `Network '<login>': authorized on ...` nao.**
- Socket TCP duy nhat cua pid 71572 la **`Listen 127.0.0.1:22346`** (cong MCP
  cua chinh no). **Khong co mot ket noi ra ngoai nao** toi bat ky server broker.

⇒ Terminal **khong dang nhap tai khoan nao**, 0 symbol duoc dong bo. Mat khau la
viec cua chu (`ADDENDUM-11` §IX). **DUNG.**

Hau qua ro rang neu van chay: `symbol_info` tra `None`, `copy_rates_range` tra
`None`, script di vao nhanh `symbol_select failed` roi `return 1`
("nothing was exported"). Tuc la chay bay gio **chac chan goi cai bi cam**
(xem 2 duoi) va **chac chan khong duoc mot bar nao**. Gia mot vi pham, duoc 0 bar.

### 2. Script GOI `symbol_select` — va cai do bi cam, khong co co de tat

`py/ingest/mt5_export.py:311` goi **`mt5.symbol_select(symbol, True)` vo dieu
kien** cho moi symbol, va chinh docstring cua no (dong 3) khai dieu do. Nhung
`py/live/spread_log.py` khai luat nguoc lai, nguyen van:

> `symbol_select` is deliberately NOT called. Adding a symbol to Market Watch
> would make an unreadable symbol readable, and it is not on the approved
> read-only list for this account.

**Hai script trong cung cay mau thuan nhau ve cung mot loi goi MT5.** Khong co
co `--no-select`. Chay `mt5_export.py` nhu hien tai = goi cai ma luat kia cam.

Bang chung mem (khong du de tu go): `data/spreads/XAUUSD_sc.csv` co **8.968
dong** toi `2026-09-17T03:50:08Z`, do `spread_log.py` ghi — ma no **khong bao gio**
goi `symbol_select` ⇒ `XAUUSD.sc` **da** nam trong Market Watch cua terminal
**Live 21** luc do, nen o do loi goi se la no-op. Nhung dieu do **khong kiem
duoc** khi khong co gi dang nhap, va **khong noi gi** ve Market Watch cua
terminal demo.

⇒ **Quyet dinh cua chu, khong phai cua agent** (`ADDENDUM-11` §X): hoac (a)
xac nhan `symbol_info("XAUUSD.sc").visible == True` tren terminal da dang nhap
roi coi loi goi la no-op, hoac (b) them `--no-select` roi chay, hoac (c) cho
phep loi goi mot lan cho dung symbol nay.

### 3. DOI SERVER — cho chan dang nhat, va no la chinh FALSIFIER

Metadata cua `XAUUSD-15m.parquet` ghi `server = **VantageMarkets-Live 21**`.
Toan bo **100.586 bar** hien co den tu server do.

Ba terminal tren may nay, doc tu journal cua chung:

| Ban cai | Tai khoan | Server | Dang chay? |
|---|---|---|---|
| `C:\MT5-demo` | 26108386 | VantageMarkets-**Demo** | **CO** (pid 71572, chua dang nhap) |
| `C:\Program Files\MetaTrader 5` | 35911458 | VantageMarkets-**Live 21** | khong (log cuoi 2026-10-05) |
| `C:\MT5-cent` | 33705331 | VantageMarkets-**Live 21** | khong (log cuoi 2026-10-09) |

⇒ Terminal duy nhat dang chay la terminal **DEMO**. Neu dang nhap **no** roi
export, phan moi se duoc bao gia boi **mot server khac** moi bar cu.
**Do la mot duong may theo dinh nghia, truoc khi so mot con so nao.** Ban cai
tai tao duoc feed cua kho la `C:\Program Files\MetaTrader 5` (Live 21, 35911458)
— **nhung do cung la tai khoan cam tien that cua desk**
(`flowdesk-vps-desk`: TK LIVE 35911458), va `mt5_export.py:276-288` tu khai luat
"attaching to the account that trades in order to copy some candles is a risk
taken for no gain".

⇒ **Quyet dinh cua chu**: export tu Live 21 nao (`Program Files` hay `MT5-cent`),
hay chap nhan doi server.

### Nhung gi DA do duoc, khong can terminal

Dung `py/ingest/bar_seam_check.py` (them trong commit nay). Nen cu ngay truoc
khi chay — day la trang thai TRUOC, in ra de lan sau so duoc:

    XAUUSD-15m   100.586 bar   2022-06-16 10:30 .. 2026-09-17 13:00 UTC   2,20 MB
       exported_at 2026-09-17T13:23:48Z   spread_points_at_export 21
    XAUUSD-5m    101.242 bar   2025-04-11 01:05 .. 2026-09-15 16:00 UTC   2,25 MB
    XAUUSD-1m    100.000 bar   2026-06-02 20:15 .. 2026-09-11 20:56 UTC   1,95 MB
    XAUDUKA-15m  378.749 bar   2010-06-01 00:00 .. 2026-05-31 23:45 UTC   7,98 MB
    XAUDUKA-5m 1.135.389 bar   2010-06-01 00:00 .. 2026-05-31 23:55 UTC  22,25 MB

**Cả 5 con so khop brief tới tung bar.** Khong store nao bi chạm: 0 byte ghi
vao `data/bars`.

**A1 — kho co tron hai cot volume khong? KHONG.** Theo nam, `XAUUSD-15m` doc
**null 0,00% va zero 0,00% o ca 5 nam** (2022..2026). Tien le `BTCUSD-15m`
khong tai dien o day. (Doi chung: `XAUDUKA-15m` doc **zero 100,00% tren ca
378.749 bar**, va gini cua no in **`null`** chu khong in 0.)

**A2 — muc volume, va con so «x15,4» KHONG tai tao duoc.** Do lai:

    nam   bar      median   mean    gini
    2022  12.865     950    1.102   0,3457
    2023  23.560   1.177    1.494   0,4022
    2024  23.716     512      547   0,2594
    2025  23.619   2.300    2.683   0,3471
    2026  16.826   5.130    5.230   0,1768

    KS D lien ke:  2022->23 0,1770 (x1,355) | 2023->24 0,5864 (x0,366)
                   2024->25 0,8237 (x4,906) | 2025->26 0,5547 (x1,949)

- **«x15,4» khong co o dau.** Day-len-dinh doc **x10,02** theo median
  (512 -> 5.130) va **x9,56** theo mean (547 -> 5.230); buoc lien ke lon nhat la
  **x4,906**. ⇒ con so **x10,0** cua `instr-cost-screen` la **day-len-dinh theo
  median**, va **x4,4 / x1,78** la nhung buoc lien ke khac. Dung trich «x15,4».
- **Brief dat sai NGAY va GOP hai cu doi khac nhau.** Brief ghi "doi hinh dang o
  2023-12 (gini 0,3372->0,1797, KS D 0,82-0,85)". Do lai: cap gini gan nhat la
  **2025 0,3471 -> 2026 0,1768**, va `D` **0,8237** nam o **2024->2025**. Do la
  **hai** cu doi o **hai** nam khac, khong phai mot cu o 2023-12.
  (Luat §8: so do thang.)

**A3 — dau gia nha cung cap, va be rong lop lam loang 1,42x KHONG phai ~4x.**

    open == low, dong ho NEW YORK (dung `zoneinfo`, khong dung `new_york_local`)
      dung nen 18:00 NY (mot nen 15m):   74/1.098  = 6,74%   lift x2,47
      ca gio 18:00 NY               :  208/4.392  = 4,74%   lift x1,74
      moi nen khac (nen)            : 2.623/96.194 = 2,73%

**6,74% so voi nen 2,73% tai tao DUNG con so hai agent khac da cong bo.**
Nhung **do loang do la 1,42x, khong phai ~4x** nhu brief ghi — chieu dung, do
lon **noi qua 2,8 lan**. (Luat §8.)

**B — nen cu KHONG co dau thoi gian trung va KHONG lech luoi.**

    XAUUSD-15m : 100.586 bar | dup 0 | off-grid 0 | 1.100 lo (222 hinh cuoi tuan)
                 48.609 bar thieu trong lo | chiem luoi 67,42% span cua chinh no
    XAUDUKA-15m: 378.749 bar | dup 0 | off-grid 0 | chiem luoi 67,51%

Hai feed doc lap cung doc **~67,4-67,5%** ⇒ **67,4% la lich cua chinh thi truong**
(nghi 1h/ngay + cuoi tuan), **khong phai mat du lieu**. Vay **75,8 bar moi ngay
giao dich** cua kho la dung, khong phai lo.

**C — cua so thu hai co toi duoc khong? CAU TRUC thi CO; bang so bar:**

    tape: 55.131 print  2026-09-06 22:09 .. 2026-10-09 20:59 UTC
          (brief ghi tape chay toi 10-10; do duoc la **10-09 20:59**)
    nen ket thuc 2026-09-17 13:15 UTC (bar cuoi + mot bar)
      print TRONG vung co nen: 33.328 (60,45%)
      print SAU nen cuoi     : 21.803 (39,55%)   <- khop brief tung chu so

    tape tu no da tach thanh BA block (cat o lo print >= 2 ngay):
      block 1  15.307 print  09-06 22:09 .. 09-11 20:59  | trong nen 100,00%
      block 2  18.478 print  09-13 22:00 .. 09-17 13:46  | trong nen  97,53%
      block 3  21.346 print  10-04 22:02 .. 10-09 20:59  | trong nen   0,00%

Khoang trong block 2 -> block 3 la **16 ngay** (tape mat 09-18..10-03).

Keo nen toi hien tai can phu `2026-09-17 13:00 -> 2026-10-11`:

    = 24,02 ngay lich
    nhip cua chinh kho (60 ngay cuoi): 65,0 bar/ngay lich, 75,8 bar/ngay giao dich
    => **~1.562 bar M15 moi**  (= +1,55% cua 100.586 bar)
    rieng phan phu block 3 (10-04 .. 10-09 21:00, 5,88 ngay): **~382 bar M15**

⇒ **Cua so thu hai toi duoc bang ~382 bar M15, va tong keo them ~1.562 bar
(+1,55%).** Hai block tach theo thoi gian **da ton tai san trong tape**
(block 1+2 vs block 3, cach nhau 16 ngay) nen chan "tach theo thoi gian" cua
cong la **dat duoc**. Chan `n >= 40 lenh moi cua so` thi **KHONG tra loi duoc tu
day**: print khong phai lenh, va day la viec cua job chay ho options. Khong in
mot so 0 cho no.

### Kho MOC bao nhieu, va no ghi vao DAU

Doc het `mt5_export.py` (407 dong):

- `--out` mac dinh = `py/ingest/../../data/bars` ⇒ **trong worktree nay la
  `/e/rust/fd-mt5-export/data/bars`, THU MUC CHUA TON TAI** (`data/` o day chi co
  `news/`). Mot lan chay mac dinh **khong cham** kho song. Kho song
  `/e/rust/flowdesk/data/bars` **chi** cham duoc neu truyen `--out` tuong minh.
- `write_bars` la **GHI DE CA FILE** (`pq.write_table(tmp)` + `os.replace`),
  nhung **noi dung la doc-sua-ghi**: `read_existing(path)` nap moi bar da luu vao
  `merged`, bar moi thay the theo khoa dau thoi gian. **Khong bar nao bi mat** —
  nhung **toan bo byte cua file va ca khoi metadata bi thay** (`exported_at`,
  `spread_points_at_export` moi) ⇒ moi receipt dung tren file cu **khong con tai
  lap duoc tung bit**, dung dang §VII da lam 25/34 dong khong tai lap.
- **`--days` mac dinh `None` ⇒ di bo CA LICH SU** terminal co, va vi
  `max bars in chart = 100.000` thi M15 duoc ~100k bar. Chay nhu the tren kho
  song se **ghi lai gia tri cua ~100k bar nguoc ve 2022-06**, khong phai noi them
  1.562 bar. **`--since-stored` moi la co gioi han vao phan duoi — va no TAT
  MAC DINH.**

⇒ Neu chu cho chay, dang chay **dung** la: `--out` vao mot thu muc RIENG
(khong phai kho song), roi `bar_seam_check.py --old <kho song> --new <file moi>`;
chi khi B doc **0 bar OHLC khac nhau** tren phan trung lap thi moi ban den
chuyen hop nhat. Hoac neu ghi thang vao kho song thi **phai co
`--since-stored`** va **phai sao luu truoc**.

### Doi chung — dung cu co nhin thay khong?

Yeu cau cua `ADDENDUM-11` §VIII. Hai doi chung, va mot probe co y nhin:

1. **Doi chung DUONG, tai tao so cua nguoi khac.** `bar_seam_check.py` doc doc
   lap ra **21.803 print / 39,55%** va **6,74% vs 2,73%** — dung bang con so hai
   agent khac da cong bo, **toi tung chu so**. Dung cu nhin thay duoc.
2. **Doi chung dong nhat.** `--old X --new X` doc **0 bar OHLC khac nhau,
   0 bar volume khac nhau, 0 bar duoi** ⇒ khong bao dong gia.
3. **Probe CO Y NHIN, phai TRUOT.** Noi `XAUUSD-15m` (Live 21) voi
   `XAUDUKA-15m` (venue khac, cung kim loai) — hai file **that su** la hai
   instrument. Checker doc:
   **92.644/92.644 dau thoi gian trung co OHLC KHAC NHAU (100,00%)**,
   **100,00% volume khac nhau**, va cot volume doi **0,00% zero -> 100,00% zero**.
   ⇒ Probe **TRUOT** phep kiem B va A1 nhu phai truot. Mot lan pass cua checker
   **khong phai** mot lan pass mu.

### Ket luan

**0 bar da duoc keo. 0 o cong da tieu. 0 byte ghi vao bat ky kho nao.**
Falsifier **ban o tien kiem** — nhung khong phai o chan hinh dang nen: no ban o
chan **DANH TINH FEED**, truoc do mot buoc.

> Mo rong kho nay **bang terminal dang chay** (DEMO 26108386) se mua mot
> **DUONG MAY**, khong phai mot cua so thu hai, vi toan bo 100.586 bar cu den tu
> **VantageMarkets-Live 21**. Doi server la doi instrument; ho so se phai coi
> phan moi la mot instrument KHAC, khong phai cung mot chuoi dai hon.

Ba viec cho chu quyet, khong agent nao duoc tu chon:

1. **Dang nhap terminal nao** (mat khau la viec cua chu). Terminal dang chay
   khong dang nhap: 0 ket noi ra ngoai, 0 dong `authorized` trong journal.
2. **Export tu Live 21 nao** — `C:\Program Files\MetaTrader 5` (35911458, cung la
   TK cam tien that) hay `C:\MT5-cent` (33705331) — **hay chap nhan doi server
   sang Demo va chap nhan duong may.**
3. **`symbol_select`**: `mt5_export.py:311` goi vo dieu kien, `spread_log.py`
   khai no bi cam. Can (a) xac nhan `visible == True` roi coi la no-op, (b) them
   `--no-select`, hoac (c) cho phep mot lan.

Khi ba cho do duoc go, phep do la **mot lenh**: chay export vao thu muc rieng,
roi `bar_seam_check.py --old --new`. Ca nam vach da ghim o tren, va trang thai
TRUOC da in.
