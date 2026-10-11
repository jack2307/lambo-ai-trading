# Kết quả — chi phí/R của MỌI instrument trong kho (`agent/instr-cost-screen`)

**Ngày:** 2026-10-11. Đăng ký: `docs/decisions/2026-10-11-instr-cost-screen.md`.
Receipt duy nhất: `receipts/instr-screen/instr-screen-2026-10-11.txt` (490 dòng).
Binary: `target-is/release/instr_screen.exe`, nguồn
`crates/fd-backtest/src/bin/instr_screen.rs`.

**Cổng: 0 ô.** Job này không chạy cơ chế nào, không mở một lệnh nào, không tiêu
một ô cổng nào. Nó quyết định ô nào đáng tiêu về sau.

## 0. Một dòng

**F1 BẮN.** Không instrument nào trong kho — với spread có provenance **ĐO** —
rẻ hơn vàng 15m đủ hệ số **4,577×** để đóng khe `1,901× → 8,7×` của khuôn, trên
**bất kỳ** cửa sổ nào và trên **cả ba** thước chân trời. Hàng duy nhất vượt vạch
là `markets.btc` (Binance spot) ở **6,25×** cửa sổ W2 — và nó trượt **cả ba** phép
kiểm: spread **5,00 là GIẢ ĐỊNH** (chính config ghi thế), kho nến **không phủ W1**
(bắt đầu 2024-09-12), `swap` **là key VẮNG MẶT** nên 0,00 của nó là một `null`.
Và nó **không phải tài khoản desk giao dịch được** — BTC thật của Vantage là
`btcusd`, đọc **1,68×**, thiếu 2,72 lần.

⇒ **"Sang thị trường khác" là hướng KHÔNG đi được bằng dữ liệu đang có.** Nó đòi
**mua dữ liệu mới**, và đó là quyết định của chủ, không phải của agent.

## 1. Đối chứng dương — làm TRƯỚC, vì mọi thứ dưới đây là một số âm

| đối chứng | đọc được | phải đọc | kết |
|---|---|---|---|
| thước chi phí: nến vàng thật, spread = 1/10 spread vàng | 10,566% → **1,057%**, hệ số **×10,000** | đúng ×10 và **QUA** sàng 4,577× | **QUA** |
| dấu in: chuỗi cố ý `open = low` mọi nến | **100,00%** | 100% | **QUA** |
| dấu in: chuỗi giá liên tục | **0,00%** | ~0% | **QUA** |
| probe cố ý in CHỈ ở nến mở lại (phải bắn F5) | nến mở lại 100% vs nến thường 0%, lift **∞** | **BẮN** | **BẮN, như phải** |

⇒ Dụng cụ nhìn thấy được. Mọi "không bắn" ở dưới là một phép đo, không phải một
test mù.

**Và một kiểm chứng thứ năm, không khai trước mà đáng hơn cả bốn cái trên:**
thước P đọc vàng Duka **12,578% (W1) / 8,354% (W2)** — **khớp tới ba chữ số** hai
con số thật của hồ sơ (12,58% / 8,35%). Dụng cụ này tái tạo đúng số của chính hồ
sơ trước khi nó nói bất cứ điều gì mới.

## 2. Bước 1 — kiểm kê

### 2.1 Chín `markets.*`, và chỉ TÁM sinh ra được một hàng

`markets.gold` (tape OTL/COMEX, `bar_symbol = GC`) **bị loại**: trên đĩa chỉ có
`GC-1m.parquet`, **không có `GC-15m.parquet`**. Nó **không có** chi phí/R ở thước
P — `null`, không phải 0, và không phải con số của vàng Duka.

`markets.audnzd` **không tồn tại trên nhánh này** (`grep -rni audnzd` ra 0 kết
quả ở `29c8635`) trong khi nến của nó đã nằm trên đĩa từ 2026-10-06. Job này
**cherry-pick nguyên block config từ `agent/aud`** — thuần cộng thêm, không đổi
một con số nào của market cũ — để AUDNZD là một **hàng**, không phải một chú
thích.

| market | symbol | bar_sym | nguồn | spread | provenance | contract | min_lot | lot_step | equity USD | equity từ đâu |
|---|---|---|---|---|---|---|---|---|---|---|
| audnzd | AUDNZD.sc | AUDNZD | dukascopy | 0,00030 | **GIẢ ĐỊNH** (`# NOT MEASURED`) | 1000 | 0,01 | 0,01 | 10 000 | **thừa kế** |
| btc | BTCUSD | BTCUSDT | binance | 5,000 | **GIẢ ĐỊNH** | 1,00 | 0,001 | 0,001 | 10 000 | **thừa kế** |
| btcusd | BTCUSD.sc | BTCUSD | mt5 | 17,050 | **MỘT LẦN ĐỌC** | 0,01 | 0,01 | 0,01 | **100** | **KHAI** |
| eurduka | EURUSD.sc | EURDUKA | dukascopy | 0,00014 | ĐO (119k tick/3 ngày) | 1000 | 0,01 | 0,01 | 10 000 | thừa kế |
| eurusd | EURUSD.sc | EURUSD | mt5 | 0,00014 | ĐO (cùng phép đọc) | 1000 | 0,01 | 0,01 | **100** | **KHAI** |
| gold | XAUUSD | GC | reference | 0,300 | **KHÔNG KHAI** | 100,0 | 0,01 | 0,01 | 10 000 | thừa kế |
| xagduka | XAGUSD.sc | XAGDUKA | dukascopy | 0,02100 | ĐO (205k tick/3 ngày) | 50,0 | 0,01 | 0,01 | 10 000 | thừa kế |
| xauduka | XAUUSD.sc | XAUDUKA | dukascopy | 0,28000 | ĐO; **ĐO thật là 0,220** | 1,00 | 0,01 | 0,01 | 10 000 | thừa kế |
| xauusd | XAUUSD.sc | XAUUSD | mt5 | 0,28000 | ĐO; **ĐO thật là 0,220** | 1,00 | 0,01 | 0,01 | **100** | **KHAI** |

**Khuyết điểm 17 KHÔNG phổ quát, đã kiểm từng market:** ba market khai
`starting_equity_usd = 100,0` (xauusd, eurusd, btcusd); **sáu** market không khai
và thừa kế `[trading] 10 000,0` (`engine.rs:354`) — gold, xauduka, xagduka,
eurduka, audnzd, btc.

**`swap` — một 0,00 KHAI và một key VẮNG MẶT là cùng một thứ với serde, và chúng
KHÔNG phải cùng một thứ** (`config.rs:347-350`, `#[serde(default)] f64`):

- **key VẮNG MẶT ⇒ 0,0 là một `null` đóng vai 0**: `markets.gold`, `markets.btc`.
  Chi phí qua đêm của hai market này **không đo được**.
- **khai 0,0 VÀ có phép đo trên đúng symbol đó**: chỉ `xauusd` (340 lệnh đóng, 87
  qua đêm, swap 0,00, 2026-04-21…09-08).
- **khai 0,0, phép đo BORROW từ symbol khác**: `xauduka` (cùng symbol
  `XAUUSD.sc`, nhưng phép đo 5 tháng đem áp cho 16 năm ⇒ một **SÀN**).
- **khai 0,0 trong khi chính config nói phép đo KHÔNG trên symbol đó**: xagduka,
  eurduka, eurusd, audnzd, btcusd. Với audnzd config còn nói rõ cross mang
  chênh lệch lãi suất AU-NZ, **tức toàn bộ lợi nhuận của bản cổ điển của trade
  đó**.

⇒ **Mọi con số chi phí/R trong báo cáo này là `spread/stop` VÀ CHỈ THẾ.** Không
swap, không commission, không slippage. Ở mọi hàng F4 bắn, nó là một **SÀN**.

### 2.2 Kho nến thật sự có gì (24 file `.parquet`, 0 file nào không đọc được)

| market | bar_sym | 1m | 5m | 15m | khoảng |
|---|---|---|---|---|---|
| audnzd | AUDNZD | 5 507 472 | 1 111 927 | 371 148 | 2010-06-02 … 2026-05-29 |
| btc | BTCUSDT | 10 683 | 218 880 | 70 080 | 1m: 2026-09-05…09-12 · 5m: 2024-08-13… · 15m: **2024-09-12**… |
| btcusd | BTCUSD | 99 823 | 100 860 | 100 798 | 1m: 2026-07-05… · 5m: 2025-09-26… · 15m: **2023-10-05**…2026-09-15 |
| eurduka | EURDUKA | 5 881 397 | 1 193 417 | 398 220 | 2010-06-01 … 2026-05-31 |
| eurusd | EURUSD | **VẮNG** | 61 054 | 61 247 | 15m: 2024-03-29 … 2026-09-15 |
| gold | GC | 16 903 | **VẮNG** | **VẮNG** | 1m only |
| xagduka | XAGDUKA | 5 285 287 | 1 117 119 | 374 188 | 2010-06-01 … 2026-05-31 |
| xauduka | XAUDUKA | 5 635 777 | 1 135 389 | 378 749 | 2010-06-01 … 2026-05-31 |
| xauusd | XAUUSD | 100 000 | 101 242 | 100 586 | 15m: 2022-06-16 … **2026-09-17** |

`flat%` (nến `high == low`) là **0,0%** trên mọi file trừ `EURUSD-5m` (0,1%).

**Kho bar trên đĩa mà config KHÔNG khai — cộng hết:**

1. **AUDNZD, ba file, 6 990 547 nến, 90,3 MB, 2010-06-02 → 2026-05-29.** Không
   `[markets.*]` nào trên `29c8635` khai `bar_symbol = AUDNZD`. Đây là **toàn bộ**
   kho không khai, và nó là một kho **16 năm** — lớn hơn kho của bất kỳ feed MT5
   nào trong config. (Job này đã khai nó bằng cherry-pick; con số trên là tình
   trạng **trước** cherry-pick.)
2. **Bốn file `.bak`, 63,7 MB**: `XAUDUKA-{1m,5m,15m}.parquet.bak` và
   `XAUUSD-15m.parquet.bak`. Không đường ống nào đọc chúng (`read_bars` chỉ nhận
   đúng đuôi `.parquet`). Chúng là **bản tiền-rebuild**: `XAUDUKA-1m.bak` 46,6 MB
   so với bản sống 95,8 MB.
3. Và ngược lại, **hai store CONFIG KHAI mà đĩa không có**: `GC-15m` (nên `gold`
   bị loại) và `EURUSD-1m`.

### 2.3 Cột volume — khai từng instrument

| market | 1m | 5m | 15m |
|---|---|---|---|
| audnzd | **null 5 507 472/5 507 472** | **CHẾ RA 0** (1 giá trị, 0 cái > 0) | **CHẾ RA 0** |
| eurduka | **null 5 881 397/5 881 397** | **CHẾ RA 0** | **CHẾ RA 0** |
| xagduka | **null 5 285 287/5 285 287** | **CHẾ RA 0** | **CHẾ RA 0** |
| xauduka | **null 5 635 777/5 635 777** | **CHẾ RA 0** | **CHẾ RA 0** |
| xauusd | tick | tick | tick |
| eurusd | (không file) | tick | tick |
| btcusd | tick | tick | **tick, NHƯNG 55,9% dòng đúng bằng 0** |
| btc | thật | thật | thật |
| gold (GC) | (chỉ 1m; xem ghi chú) | — | — |

`XAUDUKA-1m` **null trên đúng 5 635 777/5 635 777 dòng** — khớp con số của phụ
lục 9 tới từng dòng. Và nó đúng với **cả bốn** feed Dukascopy, không riêng vàng:
nhà cung cấp **không công bố gì** ở 1m, và 5m/15m mang **đúng một giá trị 0,0**
do bước chuyển `null` → `0`.

**MỚI, và nó là một hiện vật cửa sổ bên trong MỘT file:** `BTCUSD-15m` đổi trạng
thái cột volume giữa file.

| năm | dòng | zero% | trung vị của >0 |
|---|---|---|---|
| 2023 | 7 848 | **100,0%** | null |
| 2024 | 33 984 | **100,0%** | null |
| 2025 | 34 335 | **42,1%** | 1 560 |
| 2026 | 24 631 | 0,0% | 1 308 |

⇒ **41 832 nến đầu (2023-10 → 2024-12) của BTC CFD mang một số 0 CHẾ RA**, rồi
cột trở thành tick giữa 2025. Một phép đo volume trên `btcusd-15m` phủ cả ba năm
đang trộn **hai cột khác nhau**. Chưa ai báo cái này.

**`XAUUSD` là tick volume** (`py/ingest/mt5_export.py:33-34`: *"CFD real volume is
always zero"*) và **mức của nó dịch rất lớn** — trung vị của >0 theo năm:
**950 → 1 177 → 512 → 2 300 → 5 130** (2022…2026), tức **×10,0** từ 2024 sang
2026. ⚠️ **Con số ×15,4 của phụ lục 9 KHÔNG tái tạo được** trên ba thống kê tôi
thử: trung vị 2024→2026 **×10,0**, trung vị 2023→2026 **×4,4**, trung bình toàn
bộ trước/sau 2023-12 **×1,78**. Mức CÓ dịch rất lớn; hệ số 15,4 không phải một
con số tôi đo lại được (brief §8: số đo thắng — báo nó).

## 3. Bước 2 — chi phí/R, BỐN thứ trên mỗi con số

**MỘT định nghĩa chân trời, chốt trước và in trên mọi bảng:**

    thuoc P (CHINH):  stop = 1,5 x ATR14 tren nen 15m, don vi gia cua
                      chinh instrument do

Chọn P vì nó là thước **duy nhất** mà các con số đã công bố của hồ sơ đứng trên.
Hai thước thứ hai, in cạnh, vì **một thước duy nhất là một lựa chọn chưa khai**:

    thuoc S1:  stop = 1,5 x ATR20 tren nen NGAY PHIEN (ranh gioi 21:00Z = 17:00 NY)
    thuoc S2:  stop = 1,5 x ATR14 tren nen 4h (= `[trading] max_hold_ms`)

`k = 1,5` **không đổi thứ tự của bất kỳ hàng nào** — tỉ số là `spread/(k × ATR)`
nên `k` là một phép co giãn thuần. Thứ đổi thứ tự là **cỡ nến và chu kỳ ATR**.

### 3.1 Thước P — trung vị chi phí/R, hai cửa sổ thật

| market | **W1** 2010-06→2018-06 | cỡ stop W1 | **W2** 2018-06→2026-06 | cỡ stop W2 | spread? |
|---|---|---|---|---|---|
| btc | **null** (kho bắt đầu 2024-09) | — | **1,337%** (n 60 138) | 374,086 USD | **GIẢ ĐỊNH** |
| btcusd | **null** (kho bắt đầu 2023-10) | — | **4,958%** (n 90 546) | 343,904 USD | **MỘT LẦN ĐỌC** |
| xauusd | **null** (kho bắt đầu 2022-06) | — | **6,581%** (n 93 386) | 4,255 USD | ĐO |
| **xauduka (VÀNG, mốc)** | **12,578%** (n 190 876) | 2,226 USD | **8,354%** (n 187 860) | 3,352 USD | ĐO |
| eurduka | **11,037%** (n 199 603) | 0,00127 | **15,512%** (n 198 604) | 0,00090 | ĐO |
| eurusd | **null** (kho bắt đầu 2024-03) | — | **15,888%** (n 53 874) | 0,00088 | ĐO |
| xagduka | **30,277%** (n 189 953) | 0,06936 USD | **24,300%** (n 184 222) | 0,08642 USD | ĐO |
| audnzd | **24,210%** (n 178 758) | 0,00124 | **37,222%** (n 192 377) | 0,00081 | **GIẢ ĐỊNH** |
| gold (GC) | **null** — không có `GC-15m.parquet` | — | **null** | — | KHÔNG KHAI |

p10/p90 đầy đủ ở receipt. Ví dụ bề rộng: vàng W2 p10 **3,001%** / p90 **18,258%**
quanh trung vị 8,354% — **một trung vị không mô tả được phân phối này**.

**Cơ chế chênh lệch, đã đo lại và nó đúng:** cỡ stop 1,5 ATR(15m) của vàng đi từ
**2,226 USD** (W1) lên **3,352 USD** (W2) — và pooled là 2,650 USD trong khi cửa
sổ chung (2024-09→2026-05) là **8,021 USD**, tức **×3,6** chỉ do mức giá. Spread
cố định 0,28 ⇒ **cùng một spread, chi phí/R của CÙNG một instrument chạy từ
12,578% xuống 3,491%**, chênh **3,6 lần**, chỉ vì cửa sổ. **Mọi con số phải gắn
cửa sổ.**

### 3.2 Thước S1 và S2 — kết luận KHÔNG phụ thuộc thước

| market | P/W2 | S1/W2 | S2/W2 | hệ số rẻ hơn vàng: P / S1 / S2 |
|---|---|---|---|---|
| btc | 1,337% | 0,109% | 0,296% | **6,25× / 7,13× / 6,77×** |
| btcusd | 4,958% | 0,402% | 1,110% | 1,68× / 1,93× / 1,81× |
| xauusd | 6,581% | 0,601% | 1,649% | 1,27× / 1,29× / 1,22× |
| xauduka | 8,354% | 0,777% | 2,005% | 1,00× (mốc) |
| eurduka | 15,512% | 1,381% | 3,581% | 0,54× / 0,56× / 0,56× |
| eurusd | 15,888% | 1,403% | 3,687% | 0,53× / 0,55× / 0,54× |
| xagduka | 24,300% | 2,433% | 6,090% | 0,34× / 0,32× / 0,33× |
| audnzd | 37,222% | 3,866% | 9,331% | 0,22× / 0,20× / 0,21× |

**Thứ tự giống hệt trên cả ba thước, và hệ số lệch nhau dưới 15%.** Lựa chọn
thước **không** quyết định câu trả lời ở bộ dữ liệu này — một điều đáng nói rõ,
vì ba đêm trước thước đã là hiện vật nhiều lần.

Và nhớ phụ lục 5 mục F: `1,5 ATR(15m)` vs `1,5 biên độ ngày` **không mâu thuẫn,
hai thước khác nhau** — ở đây là 8,354% so với 0,777% trên **cùng** vàng, **cùng**
cửa sổ W2. Hệ số giữa hai thước là **10,8×**, và nó không đổi thứ tự một hàng nào.

### 3.3 Và nếu định giá lại vàng theo spread ĐO ĐƯỢC thì mọi thứ KHÓ HƠN

Spread ĐO của `XAUUSD.sc` là **0,220** (p50 của 2 315 mẫu/12,8h), còn config
**tính 0,28** — tức tính quá ~27%. Ở spread đo, vàng đọc **9,883% (W1) /
6,564% (W2)** thay vì 12,578% / 8,354%.

⇒ **Định giá lại vàng về số đo thật làm "sang thị trường khác" KHÓ hơn 27%, không
dễ hơn.** Và không instrument nào khác có một spread đo khác số đang tính, nên
không hàng nào được lợi từ phép định giá lại đó.

## 4. Bước 3 — hai mốc so sánh, cạnh mỗi hàng

**Mốc 1:** chi phí/R của vàng 15m (`xauduka`, thước P) cùng cửa sổ: **12,578% (W1)
/ 8,354% (W2) / 3,491% (CM) / 10,566% (PL)**.

**Mốc 2 — ngưỡng khuôn:** khuôn đòi `gross/spread ≥ 8,7×` mỗi phiên phơi nhiễm;
trần ĐO ĐƯỢC của vàng là `1,901×`. ⇒ hệ số một thị trường khác phải rẻ hơn vàng

    8,7 / 1,901 = 4,577x

⚠️ **Phép quy đổi này khai một giả định CHƯA ĐO**: nó giả định **cùng một tín
hiệu** bê sang — cùng gross tính bằng **R** — nên `gross/spread` co giãn đúng bằng
`cost_vàng / cost_instrument`. Chưa ai đo gross của một tín hiệu vàng trên
instrument khác. ⇒ cột hệ số là một điều kiện **CẦN**, không phải **ĐỦ**.

### 4.1 Bảng cuối — cửa sổ W2 (cửa sổ duy nhất mọi instrument có hàng)

| market | chi phí/R | vàng | rẻ hơn | spread? | spread CẦN để đạt khuôn | sàng ≥ 4,577× |
|---|---|---|---|---|---|---|
| btc | 1,337% | 8,354% | **6,25×** | **GIẢ ĐỊNH** | 6,828 | **vượt vạch NHƯNG spread là GIẢ ĐỊNH** |
| btcusd | 4,958% | 8,354% | 1,68× | **MỘT LẦN ĐỌC** | 6,277 | KHÔNG (thiếu 2,72 lần) |
| xauusd | 6,581% | 8,354% | 1,27× | ĐO | 0,07766 | KHÔNG (thiếu 3,61 lần) |
| xauduka | 8,354% | — | 1,00× | ĐO | 0,06118 | mốc |
| eurduka | 15,512% | 8,354% | 0,54× | ĐO | 0,0000165 | KHÔNG (thiếu 8,50 lần) |
| eurusd | 15,888% | 8,354% | 0,53× | ĐO | 0,0000161 | KHÔNG (thiếu 8,70 lần) |
| xagduka | 24,300% | 8,354% | 0,34× | ĐO | 0,00158 | KHÔNG (thiếu 13,31 lần) |
| audnzd | 37,222% | 8,354% | 0,22× | **GIẢ ĐỊNH** | 0,0000147 | KHÔNG (thiếu 20,39 lần) |

Cửa sổ **W1**: chỉ bốn instrument có hàng, và cái gần nhất là `eurduka` ở
**1,14×** — thiếu **4,02 lần**. Cửa sổ **CM** (2024-09-12 … 2026-05-29, nơi vàng
rẻ nhất, 3,491%): hàng tốt nhất là `btc` ở **2,62×**, thiếu 1,75 lần. Cửa sổ
**PL** (trộn thập kỷ, **không dùng để so instrument**): `btc` **7,48×**.

### 4.2 Hàng rẻ bất thường duy nhất, và ba phép kiểm nó trượt

`markets.btc` đọc **1,337%** ở W2 — rẻ nhất cả bảng, và nó tái tạo đúng con số
«BTC 1,33%» đang lưu hành. Ba phép kiểm:

1. **spread là ĐO hay GIẢ ĐỊNH?** `markets.btc.spread = 5.0`, và chính comment
   của `markets.btcusd` ghi `spread = 17.05 # MEASURED: 3.4x the 5.00 assumed`.
   **GIẢ ĐỊNH.** ⇒ **F2 BẮN.** Ở spread 17,05 (cái đã-đọc-một-lần của BTC thật)
   con số thành **4,958%** và hệ số thành **1,68×**.
2. **kho nến phủ cả hai cửa sổ?** `BTCUSDT-15m` bắt đầu **2024-09-12**, 70 080
   nến. **W1 = null.** ⇒ **F3 BẮN.** Nó là **một cửa sổ, không phải một
   instrument**. Và `BTCUSD-15m` cũng chỉ bắt đầu 2023-10-05 ⇒ F3 bắn cho cả hai
   hàng BTC, cả `eurusd` (2024-03) và cả `xauusd` (2022-06).
3. **`swap` có phải `null` đóng vai 0?** `[markets.btc.trading]` **không khai**
   `swap_long_per_lot` ⇒ 0,0 là serde default. ⇒ **F4 BẮN.** Chi phí qua đêm của
   hàng rẻ nhất bảng **không đo được**.

Và một điều thứ tư, từ chính config: `markets.btc` là **Binance spot**, không phải
một tài khoản desk giao dịch được; tài khoản thật là `BTCUSD.sc`, tức hàng
`btcusd` ở **1,68×**.

⇒ **Hàng rẻ nhất của bảng không phải một instrument rẻ. Nó là một giả định rẻ
trên một cửa sổ.** Đúng như brief dự đoán, và tôi không chạy một ô nào trên nó.

## 5. Dấu in nhà cung cấp — chạy cho MỌI instrument

Hai định nghĩa "nến mở lại", vì **một định nghĩa là một lựa chọn**:

- `gap` = nến đầu sau một **lỗ trong chuỗi** dài hơn một nến và ngắn hơn một
  ngày. Nó tìm mốc nghỉ **bất kể đồng hồ nói gì**, nhưng nó chỉ tồn tại ở feed
  nào **thực sự để trống** chỗ nghỉ.
- `ny18` = nến đóng dấu **đúng 18:00 giờ New York** (mốc mở lại CME). Nó tồn tại
  **kể cả khi feed lấp kín** chỗ nghỉ. ⚠️ **Đúng 18:00, không phải GIỜ 18:00** —
  giờ đó chứa **bốn** nến 15m và chỉ nến đầu là nến mở lại, nên một lớp rộng cả
  giờ **pha loãng lift ~4 lần** về phía nền. (Lần chạy đầu của tôi dùng cả giờ và
  nó **đổi kết luận**; xem §5.3.)

**Histogram giờ NY của các `gap` xác nhận mốc bằng dữ liệu, không bằng giả định:**
XAUDUKA **2 725/2 799** lỗ nằm ở **18h NY**, XAGDUKA **2 715/2 901**, XAUUSD
**872/874**. Mốc nghỉ là 17:00 NY và nến mở lại đóng dấu 18:00 NY — **đo được**.

### 5.1 Bảng — `open == low`, nến mở lại ĐÚNG 18:00 NY so với nến thường

| market | feed | `ny18` o==l | n | nền o==l | **lift** | F5 |
|---|---|---|---|---|---|---|
| xauduka | duka (vàng) | **20,53%** | 4 076 | 3,14% | **×6,54** | **BẮN** |
| xagduka | duka (bạc) | **20,77%** | 4 016 | 4,84% | **×4,29** | **BẮN** |
| **eurusd** | **mt5 (SÀN)** | **19,44%** | 638 | 4,58% | **×4,24** | **BẮN** |
| eurduka | duka (FX) | **11,69%** | 4 157 | 4,39% | **×2,66** | **BẮN** |
| xauusd | mt5 (sàn) | **6,74%** | 1 098 | 2,77% | **×2,43** | **BẮN** |
| audnzd | duka (FX) | **7,32%** | 3 879 | 3,23% | **×2,26** | **BẮN** |
| btcusd | mt5 (sàn) | 6,57% | 1 004 | 3,36% | ×1,96 | không bắn |
| btc | binance | 8,22% | 730 | 4,50% | ×1,82 | không bắn |

Và bằng định nghĩa `gap`, ở ba feed thực sự để trống chỗ nghỉ:
**xauduka 18,79% vs 3,14% (×5,98, n 2 799)** · **xagduka 23,92% vs 4,84%
(×4,94, n 2 901)** · **xauusd 6,75% vs 2,77% (×2,44, n 874)**. Hai định nghĩa
**đồng ý** ở ba feed này (×5,98 vs ×6,54 · ×4,94 vs ×4,29 · ×2,44 vs ×2,43) — đó
là chỗ kết quả đáng tin nhất.

**Tái tạo hai phép đo độc lập tới hai chữ số:** XAUDUKA 18,79% vs nền 3,14%
(họ báo 19,02% / 3,20%), XAGDUKA 23,92% vs 4,84% (họ: 24,16% / 4,86%), XAUUSD
**6,75%** vs 2,77% (họ: **6,75% / 2,77%** — khớp hoàn toàn). Dụng cụ đồng ý với
hai agent trước.

### 5.2 Bốn điều mới, và ba cái trong số đó đổi cách đọc

1. **Dấu in KHÔNG riêng của Dukascopy, và KHÔNG riêng của kim loại.** `eurusd`
   của **feed sàn MT5** đọc **×4,24** — lift **cao thứ ba cả bảng**, cao hơn cả
   vàng Duka đo bằng định nghĩa `gap`. Và cả hai FX Dukascopy cũng bắn (×2,66,
   ×2,26). ⇒ **6/8 instrument trong kho mang dấu in ở nến 18:00 NY.**
2. **Hai hàng duy nhất KHÔNG bắn đều là BTC** (×1,96 và ×1,82). Đó là nghĩa lý:
   BTC chạy **24/7** nên 17:00 NY **không phải một mốc đóng cửa thị trường** —
   `markets.btc` có **0 lỗ trong 70 080 nến** và `eurusd` cũng **0 lỗ**, nên với
   chúng định nghĩa `gap` là **`null`, không phải "sạch"**.
3. **`close == low` KHÔNG đi cùng `open == low`, và đây là chỗ dễ đo sai nhất.**
   Ở nến 18:00 NY: xauduka **1,40%** so với nền 1,88% = **×0,74**; xagduka ×0,64;
   audnzd ×0,57; btc ×0,74. **Sáu trong tám hàng có `close==low` THẤP HƠN nền.**
   ⇒ Dấu in nằm ở **`open`** và **chỉ** ở `open`. Ai thử bằng `close` sẽ kết luận
   "không có dấu in" trên đúng những instrument mang nó nặng nhất.
4. **`open == high` không bắn ở đâu cả** (xauduka ×0,89 ở nến 18:00; cao nhất là
   btcusd ×1,77). ⇒ dấu in có **CHIỀU**: nó đặt `open` ở **đáy** nến, không ở
   đỉnh. Một cơ chế long đọc `open` nến đó đang mua ở mức thấp nhất của nến theo
   một hiện vật đường ống, không theo một giao dịch.

### 5.3 Và một chỗ TÔI ĐÃ SAI, bắt được bằng chính phép đo này

Lần chạy đầu tôi định nghĩa `ny18` là **cả giờ 18:00 NY** (bốn nến 15m). Lớp đó
pha loãng lift ~4 lần và nó dẫn tôi tới một kết luận **sai**: rằng *"dấu in là
hiện vật của mốc nghỉ CME trên KIM LOẠI, feed FX của cùng nhà cung cấp không
mang nó"* (eurduka đọc ×1,98 và audnzd ×1,74, cả hai **dưới** vạch 2×).

Thu lớp về **đúng nến 18:00** thì eurduka thành **×2,66**, audnzd **×2,26**, và
eurusd của sàn **×4,24** — kết luận lật: **FX cũng mang dấu in, và feed sàn mang
nó mạnh nhất trong nhóm FX.**

⇒ **Bề rộng của lớp đối chứng là một lựa chọn chưa khai, và ở đây nó quyết định
dấu của kết luận.** Nếu bản đầu được công bố thì nó sẽ nói sai về ba instrument.
Cả hai bản đều ở lịch sử git; receipt công bố là bản đã sửa.

### 5.4 Việc phải làm trước khi tiêu một ô nào

**Trước khi tiêu một ô trên BẤT KỲ instrument nào trừ hai hàng BTC, kiểm xem cơ
chế có đọc `open` của nến 18:00 NY không.** Và nhắc lại phụ lục 10 §IV:
**0/153 dòng của `mould-target` có filter `flat 16:30-18:15`** (`filters = []`)
⇒ **đừng cho rằng dòng của bạn miễn nhiễm.**

## 6. Khuyết điểm 17, đo lại đúng cách: nó là thuộc tính của CỠ STOP

`lots = max(floor(raw_lots/lot_step)×lot_step, min_lot)`, `raw_lots =
(equity×0,01)/(stop×contract_size)` (`engine.rs:729-730`). ⇒ **"clamped" không
phải một thuộc tính của market; nó là một NGƯỠNG CỠ STOP:**

    stop toi han = (equity x 0,01) / (min_lot x contract_size)

| market | equity | **stop tới hạn** | raw_lots ở stop P | ở stop S1 | ở stop S2 |
|---|---|---|---|---|---|
| eurusd | 100 | **0,10000** | 1,183 | **0,103** | 0,277 |
| audnzd | 10 000 | 10,000 | 101,589 | 10,888 | 25,991 |
| eurduka | 10 000 | 10,000 | 94,230 | 8,353 | 22,055 |
| xauusd | 100 | **100,000** | 0,220 | **0,020** | 0,056 |
| xagduka | 10 000 | 200,000 | 25,611 | 2,466 | 6,262 |
| xauduka | 10 000 | 10 000,000 | 37,736 | 3,343 | 8,864 |
| btcusd | 100 | 10 000,000 | 0,301 | **0,024** | 0,067 |
| btc | 10 000 | 100 000,000 | 0,283 | 0,023 | 0,062 |

**0/8 market bị kẹp `min_lot`** ở trung vị của bất kỳ thước nào trong ba thước.
⇒ Câu «mọi dòng trên `xauusd` chạy ở lot tối thiểu» (phụ lục 7, khuyết 17) **đúng
ở một cỡ stop RỘNG hơn**: nó cần stop > **100 USD/oz**, tức hơn **hai** biên độ
ngày NY. Ở `f = 1,0` biên độ ngày (stop 49,055) `raw_lots = 0,020` — **không
kẹp**.

**Nhưng chỗ méo thật trên các sổ cent không phải `min_lot`, nó là `lot_step`:**
ở `raw_lots = 0,020` với `lot_step = 0,01`, cỡ lệnh chỉ có **HAI bậc**, nên một
bậc làm tròn là một sai số cỡ lệnh tới **50%**. `btcusd` ở 0,024 và `eurusd` ở
0,103 cùng nằm trong vùng đó. ⇒ **trên ba market cent, `PF_usd` mang một lượng
tử hoá cỡ lệnh cỡ chục phần trăm mà cả hai chân cổng mù với nó** — và đó là một
lý do nữa để đọc `PF_r`/`E`, không `PF_usd`.

## 7. Falsifier — bắn chưa, ở đâu

| | kết | ở đâu |
|---|---|---|
| **F1** (không instrument nào đóng được khe 4,577×) | **BẮN** | Bảng 4.1 + 3.2. 0/8 hàng đạt 4,577× trên W1+W2 với spread ĐO. Trên cả ba thước. |
| **F2** (hàng rẻ nhất có provenance GIẢ ĐỊNH) | **BẮN** | `markets.btc`, spread 5,00, config tự ghi là giả định. |
| **F3** (hàng chỉ phủ một cửa sổ) | **BẮN, 4 hàng** | btc (2024-09), btcusd (2023-10), eurusd (2024-03), xauusd (2022-06). |
| **F4** (`swap` là `null` đóng vai 0) | **BẮN, 7/9 hàng** | Vắng key: gold, btc. Khai 0,0 mà phép đo không trên symbol đó: xagduka, eurduka, eurusd, audnzd, btcusd. Chỉ `xauusd` có phép đo thật; `xauduka` borrow. |
| **F5** (dấu in nhà cung cấp) | **BẮN, 6/8 hàng** | §5.1. Ở nến **đúng 18:00 NY**: xauduka ×6,54 · xagduka ×4,29 · **eurusd (sàn) ×4,24** · eurduka ×2,66 · xauusd ×2,43 · audnzd ×2,26. Không bắn: **btcusd ×1,96 và btc ×1,82 — cả hai là BTC, thị trường 24/7 không có mốc đóng**. Không đo được bằng `gap` (0 lỗ): btc, eurusd. |
| **F6** (`raw_lots < min_lot`) | **KHÔNG BẮN, 0/8** | §6. Nhưng thay vào đó đo được ngưỡng và tìm ra chỗ méo là `lot_step`. |

## 8. Sổ đa phép thử — khai vs xem

| | khai trước | thật sự xem | ghi chú |
|---|---|---|---|
| chi phí/R | 9 × 3 thước × 4 cửa sổ = **108** | **108** (36 là `null` vì không phủ cửa sổ / không có file 15m) | đúng khai |
| kiểm kho nến | 24 file | **24** đọc hết + 4 `.bak` liệt kê + 2 store khai-mà-vắng | đúng khai |
| cột volume | 24 file × 4 trạng thái | **24** + **một trục KHÔNG khai trước: per-year** (8 market × 2 cỡ nến × số năm = 118 dòng) | **thêm một trục**, xem dưới |
| dấu in | 9 × 4 phép thử × 2 lớp = **72** | **64** (8 hàng × 4 phép thử × 2 lớp) + lớp nền `ord` đã khai | ít hơn khai, vì `gold` không có nến 15m. **Lớp `ny18` chạy HAI bề rộng** (cả giờ, rồi đúng nến) ⇒ **thêm 32 tỉ lệ không khai trước**; bản đầu bị bỏ, lý do ở §5.3 |
| lot kẹp | 9 × 1 | **8 × 3 thước = 24** + 8 ngưỡng tới hạn | **thêm 2 thước**, xem dưới |
| đối chứng dương | 4 | **4** | đúng khai |

**Hai trục KHÔNG có trong đăng ký, khai thẳng ra:**

1. **Volume per-year** (118 dòng). Thêm vào *sau khi* thấy `BTCUSD-15m` có 55,9%
   dòng bằng 0 mà 1m/5m không có — tức **sau khi thấy số**, điều đăng ký cấm.
   Nó là một **phép đo mô tả cột dữ liệu**, không có cổng, không có vạch, không
   có verdict, nên nó không mua được một kết quả dương nào; nhưng **nó vẫn là một
   trục chọn sau khi thấy số** và phải đọc như thế.
2. **Lot kẹp ở cả ba thước thay vì một** (24 thay vì 9). Lý do: ở một thước duy
   nhất, 0/8 kẹp đọc như "khuyết điểm 17 không tồn tại", mà sự thật là ngưỡng
   nằm giữa các thước. Cùng loại: mô tả, không có vạch.

**Và chỗ bề rộng lớp đối chứng đổi dấu kết luận** — xem §5.3. Nó là trục thứ ba
không khai trước, và là trục duy nhất trong ba cái **đổi một kết luận**.

**Và một sai của chính dụng cụ, tự bắt được, báo ra:** lần chạy đầu
`fd_core::clock::new_york_local` bị đọc như `(hour, minute)` trong khi nó trả
**`(weekday, minute-of-day)`**. Hậu quả: lớp `ny18` **rỗng về mặt cấu trúc trên
mọi instrument**, và histogram tôi in nhãn là "giờ" thật ra là **thứ trong tuần**
(nó in `01h:705 02h:699 03h:696 04h:687` = Mon/Tue/Wed/Thu — một lỗ mỗi ngày
làm việc). **Chính sự vô nghĩa của histogram đó là cách phát hiện ra.** Đã sửa,
chạy lại, và receipt hiện tại là bản đã sửa. Nếu lần chạy đầu được công bố thì
nó sẽ khai "`ny18`: 0 nến" trên tám instrument và đọc như một dữ kiện.

## 9. Con số đáng giữ nhất

**4,577×** là khe phải đóng, và **hàng tốt nhất với một spread ĐO là `xauusd` ở
1,27×** — tức chính vàng, trên feed của sàn, trong cửa sổ gần. Khoảng cách còn
lại là **3,61 lần**, và nó phải đến từ một spread **không tồn tại trên bất kỳ
instrument nào trong kho này**.

Cách nói thứ hai, bằng cột "spread cần": để đạt khuôn với tín hiệu vàng hiện có,
`xauusd` cần spread **0,0777** thay vì **0,28** đang tính (hay 0,220 đo được) —
**rẻ hơn 2,8 lần so với số đo thật**, trên một instrument mà 2 315 mẫu nói spread
chưa bao giờ xuống dưới **0,210**.

## 10. Thứ KHÔNG đo được, và vì sao

1. **`markets.gold` (tape OTL/COMEX)**: không có `GC-15m.parquet`, chỉ `GC-1m`
   (16 903 nến). Không có chi phí/R ở bất kỳ thước nào — **null**.
2. **Chi phí qua đêm của 7/9 hàng** (F4). Mọi chi phí/R ở đây là **SÀN**.
3. **`spread` theo thời gian** của btcusd (17,05 chưa bao giờ kiểm qua thời gian,
   config tự nói thế) và của audnzd (không có log tick AUDNZD trên máy này) và
   của btc (giả định). ⇒ **ba trong chín** hàng có một spread không ai đo.
4. **Slippage, commission, phí tài trợ**: `commission_per_lot = 0,0` toàn cục và
   không có mô hình slippage. Không đo được, không ước.
5. **Giả định "cùng tín hiệu bê sang được"** của phép quy đổi 4,577× — chưa ai
   đo gross của một tín hiệu vàng trên instrument khác. Cột hệ số là điều kiện
   **CẦN**.
6. **Hệ số ×15,4 của volume XAUUSD** không tái tạo trên ba thống kê (§2.3).
7. **`gross/spread` thật của mỗi instrument.** Job này đo **mẫu số** (chi phí/R).
   **Tử số** — gross mỗi phiên trên instrument đó — đòi chạy một cơ chế, và brief
   cấm. ⇒ bảng này **loại** instrument, nó **không chọn** instrument.

## 11. Chủ phải quyết gì

F1 bắn ⇒ **đóng** hướng "sang thị trường khác bằng dữ liệu đang có". Ba việc
**chỉ chủ quyết được**, và agent không tự làm:

1. **Có mua dữ liệu/kết nối mới không**, và cho instrument nào. Điều kiện số là
   rõ: cần một instrument có chi phí/R **≤ 1,83%** ở thước P cửa sổ W2
   (= 8,354%/4,577) **với spread ĐO** và **kho phủ cả hai cửa sổ**. Không
   instrument nào trong kho đạt; `btc` đạt con số nhưng không đạt ba điều kiện.
2. **Có đo spread BTCUSD.sc qua thời gian không** (`py/live/spread_log.py` đã
   sample BTCUSD.sc nhưng 17,05 chưa bao giờ được kiểm). Nếu spread thật của BTC
   CFD thấp hơn 17,05 đáng kể thì hàng `btcusd` **dịch**, và nó là hàng duy nhất
   trong kho có đường để dịch. Ở 1,68× nó cần rẻ hơn **2,72 lần**, tức spread
   **≈ 6,28** — và đó là một phép đo, không phải một backtest.
3. **Có định giá lại vàng về 0,220 không** (`docs/decisions/2026-09-16-spread-
   repricing-scale.txt` là tiền lệ). Nó làm mọi receipt cũ không tái lập được, và
   nó làm khe khuôn **khó hơn 27%**.

**Và một việc KHÔNG nên làm, nói thẳng:** đừng chạy một chương trình trên `btc`
(Binance spot) vì nó đọc 1,337%. Con số đó là một **spread giả định** trên **một
cửa sổ** của một **tài khoản desk không giao dịch được**.

---

## Phụ — ba chỗ brief/phụ lục không khớp số tôi đo (brief §8: số đo thắng)

1. **«BTC đo lại 2,35% → 7,79%»**: trung vị của tôi cho `btcusd` là **4,958%**
   (W2) / 4,447% (CM). Cặp 2,35–7,79 **khớp như một băng p10–p90**, không như
   một cặp trung vị: W2 p10/p90 = **2,604% / 10,937%**, CM = **2,421% / 8,430%**.
   ⇒ nói "chi phí BTC là 2,35–7,79%" là nói p10–p90, và trung vị là **4,96%**.
2. **«AUDNZD 38,16% của R»**: tôi đọc **37,222%** ở W2 và **30,477%** pooled,
   thước P. Khớp ở W2 trong 2,5%; pooled thì không.
3. **«volume XAUUSD đổi mức ×15,4»**: không tái tạo. Xem §2.3 — ×10,0 / ×4,4 /
   ×1,78 tuỳ thống kê.

Và một chỗ **khớp tới ba chữ số**, đáng nói vì nó là chứng chỉ của dụng cụ:
vàng **12,578% / 8,354%** so với 12,58% / 8,35% trong phụ lục 9.
