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

---

## Ghi chu 2026-10-12 — chu da quyet ba cho chan; lam phan KHONG can dang nhap

Khong viet lai dong nao o tren (luat §5: them ghi chu co ngay vao CUOI).

### Quyet dinh cua chu (11/10/2026)

1. **Export tu `C:\MT5-cent` = TK 33705331 (Live 21)** — cung server voi 100.586
   bar dang co ⇒ **khong sinh duong may** — va khong phai TK cam tien that.
2. **`symbol_select`: them CO TAT, va chi goi khi symbol chua `visible`.** Doc
   `symbol_info().visible` truoc; da visible thi **bo qua hoan toan**. Kiem
   `visible` tai cho, dung dua vao suy luan tu `XAUUSD_sc.csv`.
3. **Ghi vao thu muc RIENG, kiem duong may truoc.** Kho song khong duoc cham mot
   byte. Hop nhat la mot quyet dinh RIENG cua chu, khong phai phan tiep.

### A) Da va `symbol_select` — `mt5_export.py`

- **`symbol_select_plan(info, allow_select)`** — ham thuan, quyet dinh
  `proceed` / `select` / `skip`. `visible == True` ⇒ **`proceed`, khong goi**,
  **ke ca khi co bat**: co la **giay phep, khong phai menh lenh**. `info is None`
  (trang thai cua terminal chua dang nhap) ⇒ cung nhanh voi not-visible, nhung
  **ly do in ra khac nhau** de doc log phan biet duoc.
- **`ensure_market_watch(symbol, allow_select, mt5_mod, log)`** — **cho goi
  `symbol_select` DUY NHAT trong ca file**, va `mt5_mod` duoc **tiem vao** chu
  khong lay tu global, de test cam vao mot stub co `symbol_select` **nem
  AssertionError neu bi goi**. Mot test chi kiem gia tri TRA VE cua `_plan` se
  pass y nhu the ca khi cho goi **bo qua** ke hoach — va ho so da tra gia cho
  dung khe giua **khai bao** va **hanh vi** cua mot co (`--null-sides=` in o moi
  mode, mot mode doc).
- Co: **`--allow-symbol-select`**, **TAT MAC DINH**. Thieu co + chua visible ⇒
  **bao va dung**, vao `skipped`, **ma exit code khoi 0** (1 neu khong export gi,
  2 neu mot phan) — dung co che exit code san co cua script.
- **Chong co bi nuot im lang** (luat §VI): `allow_abbrev=False` nen
  `--allow-sym` la **loi cung**, khong con la mot ket noi am tham; cac cach viet
  sai (`--no-select`, `--allow_symbol_select`, ...) **deu la loi cung**;
  `--days 0 / -1 / abc` la **loi cung** (truoc day `--days 0` dat `floor = now`
  nen vong lap vo ngay tu vong dau — mot no-op im lang deo ao mot pull co bien);
  va **bang co** in moi lan chay voi **gia tri hieu luc + no den tu dong lenh hay
  tu mac dinh**.

### B) Chot an toan cho bay `--days=None`

**`rewrite_plan(stored_count, since_stored, rewrite_existing)`**. Chot khoa vao
**FILE da ton tai**, khong khoa vao mot duong dan bam cung: danh sach duong dan
se bao ve mot thu muc va bo ho mot ban copy cua cung kho do, ma hiem hoa thuoc
ve **file**, khong thuoc ve **thu muc**. Ba duong qua, khong co duong thu tu:

- chua luu gi ⇒ backfill khong ghi de gi;
- `--since-stored` ⇒ pull bi chan vao phan duoi (mot bar trung lap co y);
- **`--rewrite-existing`** (co moi, tat mac dinh) ⇒ nguoi goi noi ra bang chu.

**`--days` CO Y khong phai mot duong qua, va no khong phai tham so cua ham** — co
mot test ghim dung dieu do (`inspect.signature`). Ly do: mot walk co bien **van
ghi de moi bar da luu nam trong bien do**; `--days 400` tren kho nay se ghi lai
400 ngay cua no. Chan "di bo nguoc bao xa" **khong phai** chan "de yen gia tri da
luu", va gop hai cai do lai chinh la cai lam cai bay re.

Loi tu choi **neu ro so bar dang bi de doa** va **ten hai co** de go.

### C1) `py/ingest/test_mt5_export_guards.py` — **33/33 PASS**, stdlib `unittest`

Khong co pytest trong cay nay, nen test khong duoc doi mot test runner moi.
Sau lop:

- `TestSymbolSelectPlan` (7) — bon trang thai x hai gia tri co, cong
  `getattr(info,"visible",False)`: mot object khong co `visible` **khong duoc**
  doc la visible.
- `TestCallSiteObeysThePlan` (5) — **hanh vi**: stub tripwire khang dinh duong
  "da visible" **khong goi** `symbol_select` (ke ca khi co bat), duong "co tat +
  chua visible" **bao va dung ma khong goi** va **loi nhac ten co**, va — **doi
  chung DUONG** — duong "co bat + chua visible" **CO goi**. Khong co doi chung
  duong nay thi moi khang dinh kia cung pass tren mot ban da **xoa han**
  `symbol_select`, va mot chot hoat dong bang cach bo nang luc di **khong phai**
  cai chot da duoc yeu cau.
- `TestRewritePlan` (5) + `TestFlagsCannotBeSwallowed` (7) — dung **parser THAT**
  (`build_parser` duoc tach ra khoi `main`), khong phai mot ban copy: khe giua
  hai cai do chinh la cho khuyet diem song.
- `TestLatchEndToEnd` (7) — **chay ca `main()`** voi mot stub `MetaTrader5` va
  mot kho parquet that trong temp. Day moi la cho **do co dat duoc tu DAU**
  (§VI: `[run] max_hold_ms` parse duoc, doc lai dung, va khong bao gio den duoc
  cai no goi ten). Do duoc:
  - lan chay thu hai tren cung kho **bi TU CHOI**, exit **1**,
    **`ranges_asked == 0`** (tu choi **khong ton mot lan hoi terminal**), va **so
    bar trong file khong doi**;
  - `--since-stored` qua chot va **noi them that** (so bar tang);
  - `--rewrite-existing` qua chot;
  - chua visible + khong co ⇒ exit **1**, **khong goi** select, **khong tao file**;
  - chua visible + co co ⇒ **co goi**, co file (doi chung duong);
  - bang co in ra co **ca `[default]` lan `[command line]`**.
- `TestSourceHoldsOneCallSite` (2) — doi ung Python cua test quet ma nguon ben
  Rust: **dung `ast`, khong dung `in`**. Ban quet bang chuoi **doc 2 cho** va chi
  mot la loi goi — cho kia la chu `symbol_select()` **trong help string** cua
  `--allow-symbol-select`. Mot test dem **lan nhac** se **truot khi tai lieu tot
  len** va **pass khi them mot loi goi** trong mot dong khong co comment: nguoc.
  Parser biet cai nao la cai nao. Nay: **dung MOT Call node, trong
  `ensure_market_watch`**.

**Mot khuyet diem nua, va no o dong duy nhat nguoi ta copy bang tay.** Docstring
module khong phai raw string, nen dong Usage
`--terminal C:\MT5-demo\terminal64.exe` **render ra
`C:\MT5-demo<TAB>erminal64.exe`** (`\t` la escape hop le) va `\M` nem
**DeprecationWarning moi lan import**. Va: `r"""`. Da kiem:
`python -W error::DeprecationWarning -c "import mt5_export"` sach, va
`chr(9) in __doc__` doc **False**.

### C2) Terminal: **VAN CHUA DANG NHAP. DUNG.**

**Toi KHONG goi `mt5.initialize(path=r"C:\MT5-cent\terminal64.exe")`**, va day la
ly do — do duoc, khong phai doan:

- `C:\MT5-cent\terminal64.exe` **KHONG chay**. Danh sach process chi co mot
  `terminal64.exe`: pid **71572** = `C:\MT5-demo\terminal64.exe /portable`.
- ⇒ `initialize(path=...)` se **KHOI DONG** no. Va `C:\MT5-cent\config\start.ini`
  doc (chi doc TEN KHOA va ba gia tri khong phai bi mat):

      Account=33705331    AllowLiveTrading=1    Enabled=1

  cong `config/accounts.dat` **8.042 byte** (kho thong tin dang nhap da luu) va
  `common.ini` co khoa `Login` + `Server` (**khong doc gia tri**).

⇒ Khoi dong no = **tu dong dang nhap mot TK Live 21 voi `AllowLiveTrading=1` va
mot expert `Enabled=1`**. Do la **THUC HIEN** viec dang nhap, khong phai **do**
no — va la dung cai bi cam. Loi goi do khong them mot thong tin nao: danh sach
process **da** tra loi cau hoi. **Mat khau va viec khoi dong la viec cua chu.**

### Cai do duoc them, va no ung ho quyet dinh 1 cua chu

Do do sau cache lich su, chi doc:

    C:\MT5-cent   \bases\VantageMarkets-Live 21\history\XAUUSD.sc -> 2026 chi, **15 KB**
    C:\Program Files\MetaTrader 5
                  \bases\VantageMarkets-Live 21\history\XAUUSD.sc -> **2022,2023,2024,2025,2026, 91.305 KB**
    C:\MT5-cent + C:\MT5-demo
                  \bases\VantageMarkets-Demo\history\**XAUUSD**    -> 2026, 4.419 KB

Ba dieu doc ra:

1. **Terminal da dung nen kho 100.586 bar la `C:\Program Files\MetaTrader 5`** —
   do sau cache **2022..2026** trung **dung** span cua kho
   (2022-06-16 .. 2026-09-17). Nhung do la terminal cua TK 35911458 (cam tien
   that) ma chu co y tranh.
2. **`C:\MT5-cent` gan nhu khong co cache** cho `XAUUSD.sc` (15 KB = co bap cua
   mot muc rong; nhieu symbol khac doc dung 15 KB). Nhung **ca hai deu la
   `VantageMarkets-Live 21`** ⇒ **cung server, cung feed, khong duong may**:
   quyet dinh 1 cua chu dung ve danh tinh. Va vi lenh se chay voi
   **`--since-stored`** / `--days` nho, phan can keo la **duoi 2026**, dung phan
   ma MT5-cent **CO** ⇒ cache mong **khong phai cho chan**. (Mot backfill day
   thi se phai tai 2022-2025 tu server.)
3. **Tren server Demo, vang ten la `XAUUSD`, khong phai `XAUUSD.sc`.** Nen neu lo
   chay vao terminal demo voi `--symbols XAUUSD.sc` thi `symbol_info` tra `None`
   ⇒ **chot moi se bao va dung**, khong goi `symbol_select`. Bay do **da duoc
   bit**, khong con dua vao viec nho.

Them: **`MaxBars=100000`** trong `common.ini` cua **ca** MT5-cent **va** MT5-demo
⇒ tran "Max bars in chart" la **100.000**, dung con so docstring khai, nen mot
pull M15 "ca lich su" chi duoc ~100k bar (~2022-07 tro lai). Kho hien co
**100.586** bar la vi no **gop qua nhieu lan chay**, khong phai mot lan.

### DUNG MOT LENH, chay sau khi chu dang nhap `C:\MT5-cent` (TK 33705331)

Buoc 1 — keo vao **thu muc RIENG**, kho song khong cham mot byte. Thu muc rong
nen `read_existing` khong thay gi va chot `rewrite_plan` doc "nothing stored yet":

    cd /e/rust/fd-mt5-export && python py/ingest/mt5_export.py \
      --terminal "C:\MT5-cent\terminal64.exe" \
      --symbols XAUUSD.sc --timeframes M15 \
      --out /e/rust/fd-mt5-export/data/bars-mt5export-20261012 \
      --days 40

`--days 40` phu tu ~2026-09-02, tuc **trum ca bar cuoi cua kho song
(2026-09-17 13:00)** nen co phan trung lap de phep kiem B co cai ma so; va no
giu pull nho. **Khong** truyen `--allow-symbol-select`: neu `XAUUSD.sc` da
visible — nhu mong doi tren Live 21 — thi no **khong can**, va neu chua visible
thi lenh **phai** bao va dung de chu quyet.

Buoc 2 — phep kiem duong may, **truoc khi ban den chuyen hop nhat**:

    cd /e/rust/fd-mt5-export && python py/ingest/bar_seam_check.py \
      --old /e/rust/flowdesk/data/bars/XAUUSD-15m.parquet \
      --new /e/rust/fd-mt5-export/data/bars-mt5export-20261012/XAUUSD-15m.parquet \
      --step-min 15 \
      --tape "E:/rust/flowdesk/data/gold/tape/date=*/*.parquet"

Doc ket qua theo **nam vach da ghim o tren**, va bon so **do duoc o day**, dung
trich lai cua ai:

- cu doi hinh dang volume la **HAI** cu, o **2024->2025** (`D` 0,8237, x4,906) va
  **2025->2026** (`D` 0,5547, x1,949) — **khong phai mot cu 2023-12**;
- **x10,02** la day-len-dinh theo **median** (512 -> 5.130), **x9,56** theo mean;
  **«x15,4» khong tai tao duoc**;
- `open == low` o **dung nen 18:00 NY** **6,74%** vs nen **2,73%**, lift
  **x2,47**; lay ca gio lam loang **1,42x** (**khong phai ~4x**);
- tape het o **2026-10-09 20:59 UTC** (khong phai 10-10), **55.131 print**,
  **21.803 (39,55%)** nam sau nen cuoi, va tape **tu no** co **ba block** tach
  theo thoi gian: 15.307 / 18.478 / **21.346** print, block 3 (10-04..10-09) dang
  **0,00%** trong nen.

Chi khi buoc 2 doc **0 bar OHLC khac nhau** tren phan trung lap thi moi ban den
hop nhat — va **hop nhat la quyet dinh rieng cua chu**, khong phai phan tiep cua
job nay. Neu hop nhat: lenh la **cung buoc 1** nhung `--out` tro vao kho song
**va** bat buoc co **`--since-stored`** (khong co no, chot se tu choi — do la
viec cua chot).

### Ket luan cua ghi chu nay

**Van 0 bar da keo. 0 o cong tieu. 0 byte ghi vao bat ky kho nao.** Hai chot da
vao cay voi **33/33 test**, va cho chan con lai la **mot viec cua chu**: khoi
dong + dang nhap `C:\MT5-cent`.

### Ghi chu 2026-10-12, muon hon — MOT LAN DANG NHAP DA THU VA DA TRUOT, va no xac nhan phan tich server

Doc tu journal cua terminal (chi doc). **Khong phai toi**: toi khong co mat khau,
khong goi `initialize` kem thong tin dang nhap, va khong khoi dong terminal nao.

`C:\MT5-demo\logs\20261012.log`:

    08:46:19.958  Network  '33705331': authorization on VantageMarkets-Demo failed (Invalid account)
    08:46:24.743  Terminal exit with code 0
    08:46:25.687  Terminal shutdown with 0

Ba dieu doc ra, va dieu thu hai la cai dang nhat:

1. **Hien gio KHONG CO terminal MT5 nao chay.** Terminal demo pid **71572** (len
   11/10 20:28) da **thoat 08:46:24 hom nay**. Ket luan cua muc C **khong doi**,
   chi ro hon: truoc la "dang chay nhung chua dang nhap", nay la "khong chay".
2. **Da co nguoi thu dang nhap TK 33705331 — TK chu chon — vao server
   `VantageMarkets-Demo`, va server TRA VE "Invalid account".** Day la **doi
   chung doc lap** cho phan tich danh tinh feed o tren: TK 33705331 thuoc
   **`VantageMarkets-Live 21`**, va server Demo **khong he biet** no. Tuc la
   terminal demo **khong bao gio** export duoc du lieu nay — khong phai "feed
   khac" ma la **khong xac thuc duoc**. (Trung voi phat hien ten symbol: tren
   Demo vang la `XAUUSD`, tren Live 21 la `XAUUSD.sc`.)
3. ⚠️ **Dang nhap phai vao `C:\MT5-cent`, khong phai `C:\MT5-demo`.** TK
   33705331 song o `C:\MT5-cent` (`start.ini: Account=33705331`, va
   `bases\VantageMarkets-Live 21\` ton tai o do). Mot lan thu nua vao terminal
   demo se truot y nhu lan 08:46.

### Mot khuyet diem TOI tu gay ra trong lan va nay, va test bat duoc

Khi viet lai khoi chan doan `if defined MISSING ( ... )` trong
`deploy/run-bars-export.cmd`, toi dat mot cap **ngoac don** trong mot dong
`echo`. Trong cmd.exe mot `)` tran trong khoi `if (...)` **dong khoi ngay tai
cho**, nen `set "RC=10"` thanh **VO DIEU KIEN** va wrapper tra **10 tren mot lan
chay sach**. `deploy/bars-export-selftest.py` bat dung no o ca duy nhat ma
khong file nao thieu:

    FAIL exporter rc=2, 6/6 present -> 10 (want 2)

Da va (bo ngoac, them `rem` canh bao tai cho). Dang ghi lai vi no la mot vi du
dung cho luat cua ho so: **mot test "pass" truoc khi sua khong noi len dieu gi
neu khong co ca ca bat duoc** — va o day cai bat duoc la mot dong tu choi *gia*
tren mot lan chay dung, tuc dung loai loi lam nguoi van hanh di sai huong luc
3 gio sang.

### Mot bien phai biet truoc: CHAY LAI BUOC 1 SE BI TU CHOI

Lenh buoc 1 chay vao mot thu muc rong nen chot doc "nothing stored yet" va cho
qua. **Lan chay thu hai vao CUNG thu muc do se bi TU CHOI** (gio da co bar), va
do la dung y do. Hai cach di tiep, chon theo viec:

- **noi them phan duoi** (binh thuong): them `--since-stored`;
- **lam lai tu dau**: xoa thu muc `data/bars-mt5export-20261012` roi chay lai —
  **dung** `--rewrite-existing` cho viec nay, vi co do cho phep ghi lai gia tri
  cua bar cu ma khong de lai dau vet nao rang da ghi lai.

Va nhac lai cho khong duoc lam: `--out` **khong bao gio** tro vao
`/e/rust/flowdesk/data/bars` trong job nay. Neu sau nay chu quyet hop nhat thi
lenh do **bat buoc** co `--since-stored`; thieu no, chot se tu choi — va tu choi
**khong ton mot lan hoi terminal** (test `ranges_asked == 0` ghim dieu do).

### `deploy/install-tasks.ps1` — da kiem, khong bi anh huong

No tu choi cai mot checkout ma `mt5_export.py --help` khong co `--since-stored`.
Ban va giu nguyen co do trong help, nen check do **van pass** (selftest cung doc
`install-tasks.ps1 emits cmd.exe /c ...` la `ok`). Doan trich `mt5_export.py` o
dong ~220 cua no ("The terminal cannot be launched from SSH at all") **van dung**.
