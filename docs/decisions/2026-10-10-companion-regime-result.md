# KẾT QUẢ — trạng thái đồng thời của companion PHÂN BIỆT được kết cuộc, nhưng cái nó phân biệt là chế độ biến động CỦA CHÍNH VÀNG, và trần của nó là −0,039 R

**Ngày:** 2026-10-10. Nhánh `agent/companion-regime`, cắt từ `agent/stop-width`
(`29c8635`). Đăng ký: `docs/decisions/2026-10-10-companion-regime.md` (`e739f8c`),
cộng một ghi chú có ngày ở cuối nó (`24cdee7`) khai thêm F6 (nền tự-trạng-thái
của vàng) và F7 (ổn định theo quý) **trước** khi chạy chúng.

**Câu hỏi:** một **trạng thái ĐỒNG THỜI** của companion (bạc/EUR, đọc trên nến
đã đóng **cùng nến tín hiệu**) có lọc được lệnh của một cơ chế vàng đã có không?
Không phải dự báo (`agent/lead-lag`, đã đóng), không phải tỉ số (`agent/n2`),
không phải cross bản địa (`agent/aud`). Vị thế luôn là vàng.

**Kết luận, ba câu:**
1. **Có, một phép so phân biệt được** — `XAGDUKA` × `vol` × `bb-fade`:
   ΔE **+0,07666 R** (cửa sổ A) / **+0,05334 R** (B), và **0/1000 lần rút** của
   **cả ba** đối chứng với tới được, trên **cả hai** arm guards. Nó **không**
   biến mất khi bỏ các nến quanh mốc nghỉ CME.
2. **Nhưng nó không phải thông tin của instrument thứ hai.** Cùng định nghĩa
   trạng thái tính trên **CHÍNH VÀNG** đọc **+0,06870 / +0,04821** — nền vàng
   tái tạo **89,6% / 90,4%** cú dịch; và chẻ làm bốn quý thì **nền vàng phân
   biệt MẠNH HƠN ở 2/4 quý** (Q1 và Q4). Phần tăng thêm của companion là **một
   đồng xu**.
3. **Và trần của nó ở dưới cổng.** `E` **cao nhất của BẤT KỲ nhóm nào của BẤT KỲ
   phép so nào** là **−0,03902 R** (arm guards ON) trong khi cổng đòi
   **≥ +0,050R** ⇒ **F3 bắn ở tiền kiểm** ⇒ **tiêu 0 ô cổng.**

## 1. Cổng

**0 ô cổng tiêu.** F3 bắn ở tiền kiểm, đúng điều kiện đã đăng ký (brief §5, tiền
lệ `agent/aud` khai 432 tiêu 0; `agent/lead-lag` khai ≤108 tiêu 0).

Nên **không có** `PF_r`/`PF_usd` của một ô cổng, không drawdown của một ô cổng,
không `matched_rate`, không phân vị null, không ba nhánh filter-on/off/không-filter
để báo — và đó là **thiết kế**, không phải thiếu sót. Những gì có là số của
**tiền kiểm**: xem mục 3–6.

**Chân nào ràng buộc:** không chân nào được chạm tới. Mọi sổ trong phép đo này
có `E` từ **−0,150 R** đến **−0,080 R** và `PF_r` **0,743–0,835** — cả hai chân
trượt cùng lúc, cách vạch rất xa. `Lbar` của mọi sổ là **0,487–0,585 R**, tức
**≥ 0,250 R** ⇒ theo hằng đẳng thức `E = Lbar × (PF_r − 1)` thì ở đây chân
expectancy là **dư**, chân `PF` ràng buộc. (Hằng đẳng thức được khẳng định bằng
test `the_gate_identity_holds_on_a_sub_book`, dư < 1e-12 R.)

## 2. Falsifier bắn ở đâu

| falsifier | bắn? | ở đâu | con số |
| --- | --- | --- | --- |
| **F1** đối chứng cùng tỉ lệ cắt | **KHÔNG bắn sạch** | tiền kiểm | **1/24** phép so khai trước phân biệt được trên **cả hai** cửa sổ, cùng dấu, qua **cả ba** đối chứng (tung xu cho ~0,06/24). Giống nhau ở **cả hai** arm guards |
| **F6** nền tự-trạng-thái của vàng | **BẮN** (hiệu quả) | tiền kiểm | nền vàng tái tạo **89,6% / 90,4%** cú dịch của phép so sống sót; trên 24 phép so thì companion mạnh hơn nền ở **6/24** và nền mạnh hơn companion ở **6/24** — tung xu |
| **F7** ổn định theo quý | **BẮN một nửa** | tiền kiểm | dấu **KHÔNG lật** qua 4 quý (+0,0488 / +0,1089 / +0,0807 / +0,0262) — khác `agent/lead-lag`; nhưng **nền vàng thắng ở 2/4 quý**, nên **phần tăng thêm** của companion mới là thứ lật |
| **F3** trần | **BẮN, 24/24 phép so** | tiền kiểm | `E` tốt nhất của mọi nhóm = **−0,03902 R** (guards ON) / **−0,02715 R** (guards OFF); cổng đòi **+0,050 R** |
| **F2** nhân quả | **không bắn** (và không mù) | test Rust | cắt chuỗi companion ở **5 mốc** (120/200/300/400/500): trạng thái ở mọi nến ≤ mốc **bằng từng bit**. Probe **cố ý đọc nến sau** thì **TRƯỢT ở cả 5 mốc** ⇒ test bắn được |
| **F4** cỡ mẫu | không bắn | tiền kiểm | **0/24** phép so có nhóm < 40 lệnh ở cửa sổ đầy; nhóm nhỏ nhất của phép so sống sót là **5.958 lệnh** |
| **F5** tập lệnh ba nhánh | **không chạy** | — | 0 ô cổng. Tiền kiểm là **phân hoạch một lần chạy**, nên nó **không** đổi tập lệnh và **không** đổi phân bố cỡ lot — xem mục 7 |

## 3. Đã đo gì

- Dữ liệu: `/e/rust/flowdesk/data/bars/`, **chỉ đọc**. `XAUDUKA-15m` **378.749
  nến** 2010-06-01 → 2026-05-31; `XAGDUKA-15m` **374.188**; `EURDUKA-15m`
  **398.220**. Lịch tin: **747 sự kiện** từ
  `/e/rust/flowdesk/data/news/events.parquet` (đường dẫn được IN ra, vì engine
  mặc định đọc `data/news/` bất kể `--data=`).
- Khớp timestamp **CHÍNH XÁC** qua `fd_indicators::companion::aligned_change`
  (`period=1`, `atrPeriod=14`) — thiếu bar thì **`continue`**, **không** lấy bar
  trước. Đo được: `XAGDUKA` khớp **372.557/378.749 nến (98,4%)**, `EURDUKA`
  **378.027 (99,8%)**.
  *(Ghi chú: brief gọi hàm này là `companion::ratio_zscore`; **hàm đó không tồn
  tại** trên cây này — `grep -rn ratio_zscore --include=*.rs` → 0 kết quả. Hàm
  thật là `aligned_change`, và nó đúng là hàm có tính chất brief mô tả.)*
- Cửa sổ: **A 2010-06-01 → 2018-06-01**, **B 2018-06-01 → 2026-06-01**; thêm
  **4 quý 4 năm** cho F7. Cắt bằng `engine::Range`, **không** cắt mảng nến, nên
  chỉ báo vẫn warm-up trên dữ liệu trước cửa sổ.
- Cơ chế: `ema-cross`, `rsi-reversion`, `donchian-breakout`, `bb-fade`, **tham
  số mặc định**, bọc `weekdays` + `flat:1630-1815` (đúng arm `gold-intraday`
  của hồ sơ đặt lên mọi base method). **Cả bốn** không override `exits()` ⇒
  `Exits::Engine` ⇒ stop là **THI HÀNH**, không phải mẫu số (phụ lục 7B), nên
  `r` có nghĩa.
- Hai arm: **guards ON** (arm duy nhất chủ cho phép) và **guards OFF** (đọc
  thêm, **không** phải arm giao dịch được).
- Công cụ: `crates/fd-backtest/src/bin/cmpregime.rs`, build
  `cargo +stable-x86_64-pc-windows-gnu --release --target-dir target-cr`.
  Receipt: `receipts/cmpregime/*.txt`.
  Chạy lại: `./target-cr/release/cmpregime.exe --data=/e/rust/flowdesk/data
  --config=config --market=xauduka --interval=15m --draws=1000 --seed=12345`
  (thêm `--drop-break=120` hoặc `--quarters`).

**Vì sao tiền kiểm là PHÂN HOẠCH chứ không phải bộ lọc.** Engine giữ **một** vị
thế, nên bỏ một entry sẽ cho một entry khác sau đó có chỗ (`month-clock` đo
**+17…+22 lệnh**, `m8` đo **+36**). Một bộ lọc do đó **đổi tập lệnh** và không
thể đọc như hai nửa của một sổ — và nó tốn ô cổng. Câu hỏi tiền kiểm hẹp hơn và
rẻ hơn: chạy cơ chế **một lần, không filter**, rồi gán nhãn **lệnh thật** của nó
theo trạng thái companion ở **nến tín hiệu** (nến **trước** nến khớp — `engine.rs`
đặt `pending` ở bước 3 và fill ở `bar.open` của nến sau, bước 1). Nếu nhãn không
mang thông tin về kết cuộc thì **không bộ lọc nào dựng từ nó giúp được**, và
không ô cổng nào cần tiêu.

**Ba đối chứng, phải qua cả ba** (`--draws=1000`, seed 12345):

1. **Xáo trộn thời gian** — giá trị companion mỗi nến bị **hoán vị trong cửa
   sổ** (Fisher–Yates **chỉ trên các nến có giá trị xác định**, nên nến
   companion không phủ vẫn không phủ). Tần số biên của mọi trạng thái **giữ
   nguyên chính xác** (test `the_time_shuffle_preserves_the_marginal_exactly`).
   Đây **chính là** "bộ lọc ngẫu nhiên cùng tỉ lệ cắt" mà đăng ký đòi: đo được
   **tỉ lệ cắt của null khớp tỉ lệ cắt thật** (ví dụ phép so sống sót: thật
   **50,2%**, null trung bình **51,1%**).
2. **Xáo trộn nhãn** — hoán vị nhãn **giữa các lệnh**, giữ **cỡ hai nhóm** y hệt.
3. **Xáo trộn theo khối** — xáo trộn thời gian theo **khối 96 nến = một ngày 15m**,
   giữ cấu trúc trong ngày, để một hiệu ứng giờ-trong-ngày không mượn được phân
   phối null hẹp của xáo trộn đầy.

## 4. Bảng quyết định — 24 phép so khai trước, arm guards ON

`ΔE = E(trong trạng thái) − E(ngoài)`, đơn vị **R**, `E = total_r/n`.
`p min%` = phần trăm lần rút nhỏ nhất trong ba đối chứng chạm tới `|ΔE|`.

| companion | trạng thái | cơ chế | ΔE A (R) | ΔE B (R) | pA min% | pB min% | |
| --- | --- | --- | --- | --- | --- | --- | --- |
| XAGDUKA | dir | ema-cross | +0,07782 | −0,02196 | 5,9 | 61,0 | lật dấu |
| XAGDUKA | dir | rsi-reversion | +0,03086 | +0,06284 | 36,2 | 5,3 | trong tiếng ồn |
| XAGDUKA | dir | donchian-breakout | +0,04184 | −0,05175 | 9,8 | 3,3 | lật dấu |
| XAGDUKA | dir | bb-fade | +0,02871 | +0,05505 | 15,8 | 0,8 | trong tiếng ồn |
| XAGDUKA | move | ema-cross | +0,01781 | −0,02630 | 70,0 | 58,6 | lật dấu |
| XAGDUKA | move | rsi-reversion | +0,04045 | +0,06606 | 24,8 | 5,8 | trong tiếng ồn |
| XAGDUKA | move | donchian-breakout | +0,06256 | +0,00363 | 0,5 | 86,4 | trong tiếng ồn |
| XAGDUKA | move | bb-fade | −0,01897 | −0,03966 | 29,6 | 5,0 | trong tiếng ồn |
| XAGDUKA | vol | ema-cross | +0,03118 | −0,02154 | 43,1 | 61,0 | lật dấu |
| XAGDUKA | vol | rsi-reversion | +0,10207 | +0,04630 | 0,0 | 14,8 | trong tiếng ồn ở B |
| XAGDUKA | vol | donchian-breakout | +0,01161 | +0,01425 | 60,4 | 50,5 | trong tiếng ồn |
| **XAGDUKA** | **vol** | **bb-fade** | **+0,07666** | **+0,05334** | **0,0** | **0,4** | **PHÂN BIỆT cả hai cửa sổ** |
| EURDUKA | dir | ema-cross | −0,01630 | +0,01868 | 69,9 | 64,4 | lật dấu |
| EURDUKA | dir | rsi-reversion | +0,00887 | +0,01622 | 78,4 | 60,6 | trong tiếng ồn |
| EURDUKA | dir | donchian-breakout | +0,01973 | +0,01551 | 38,9 | 48,0 | trong tiếng ồn |
| EURDUKA | dir | bb-fade | −0,00080 | −0,01564 | 96,3 | 43,7 | trong tiếng ồn |
| EURDUKA | move | ema-cross | −0,05234 | −0,02319 | 25,7 | 63,9 | trong tiếng ồn |
| EURDUKA | move | rsi-reversion | +0,05793 | +0,02104 | 10,3 | 52,8 | trong tiếng ồn |
| EURDUKA | move | donchian-breakout | +0,04862 | +0,04784 | 3,4 | 4,7 | **gần** — xem mục 6 |
| EURDUKA | move | bb-fade | −0,01183 | −0,01550 | 55,4 | 42,5 | trong tiếng ồn |
| EURDUKA | vol | ema-cross | −0,06066 | +0,04600 | 13,3 | 26,0 | lật dấu |
| EURDUKA | vol | rsi-reversion | +0,08458 | +0,05136 | 0,2 | 9,3 | trong tiếng ồn ở B |
| EURDUKA | vol | donchian-breakout | +0,04669 | +0,04020 | 3,0 | 5,4 | **gần** — xem mục 6 |
| EURDUKA | vol | bb-fade | +0,02827 | +0,03468 | 11,2 | 6,6 | trong tiếng ồn |

**1/24** phân biệt được trên cả hai cửa sổ cùng dấu (tung xu cho **~0,06/24**;
xác suất họ-phép-thử có ≥1 do may ≈ **5,8%** — đúng ở vạch đã khai).
**18/24** chỉ chung dấu (tung xu cho 12/24). **0/24** không đo được.
Arm **guards OFF** cho **cùng một** phép so sống sót, **cùng dấu**:
ΔE **+0,06714 / +0,05356**, p **0,0% / 0,2%**.

### Phép so sống sót, chi tiết (cửa sổ A, guards ON)

| nhóm | n | E (R) | PF_r | Lbar (R) | win | exits |
| --- | --- | --- | --- | --- | --- | --- |
| **IN** (bạc ồn) | 5.958 | **−0,07292** | 0,8602 | 0,5217 | 45,8% | STOP 2.844 / TARGET 1.261 / SIGNAL 1.477 / TIMEOUT 252 / NEWS_FLAT 43 / WEEKEND_FLAT 81 |
| **OUT** (bạc êm) | 5.994 | **−0,14957** | 0,7265 | 0,5469 | 47,0% | STOP 3.001 / TARGET 1.751 / SIGNAL 1.086 / TIMEOUT 103 / NEWS_FLAT 25 / WEEKEND_FLAT 28 |
| ΔE | — | **+0,07666** | — | — | — | tỉ lệ cắt **50,2%** (null **51,1%**) |

Đối chứng: xáo trộn thời gian **0,0%** của 1.000 lần rút chạm tới, tái tạo
**19,5%** cú dịch; xáo trộn nhãn **0,0% / 19,9%**; khối-ngày **0,0% / 19,1%**.
(Để so: `vprofile-gold` đo đối chứng của nó tái tạo **45%**. Ở đây đối chứng tái
tạo **19%** — hiệu ứng thật hơn hẳn mức đó.)

**Luật của cơ chế CÓ nổ:** `STOP` + `TARGET` là **4.105/5.958 = 68,9%** lệnh
nhóm IN. Đây **không** phải ca `tsmom/120d` (PF 2,236, `SURVIVES`, luật nổ 0 lần).

## 5. F6 — và đây là chỗ trục này chết

Tính **cùng một định nghĩa trạng thái trên CHÍNH VÀNG** (`XAUDUKA` làm companion
của chính nó, khớp timestamp 100% theo cấu trúc, **cùng đường code**). Đây là cái
tương đương nền tự tương quan đã đóng `agent/lead-lag`.

| đọc | companion `XAGDUKA vol` | nền `XAUDUKA vol` | nền tái tạo |
| --- | --- | --- | --- |
| cửa sổ A | +0,07666 | **+0,06870** | **89,6%** |
| cửa sổ B | +0,05334 | **+0,04821** | **90,4%** |
| A, bỏ nến mốc nghỉ | +0,08085 | **+0,07206** | **89,1%** |
| B, bỏ nến mốc nghỉ | +0,06007 | **+0,05440** | **90,6%** |
| Q1 2010-06..2014-06 | +0,04875 | **+0,05443** | **111,7% — nền MẠNH HƠN** |
| Q2 2014-06..2018-06 | +0,10885 | +0,08272 | 76,0% |
| Q3 2018-06..2022-06 | +0,08074 | +0,06076 | 75,3% |
| Q4 2022-06..2026-06 | +0,02617 | **+0,03607** | **137,8% — nền MẠNH HƠN** |

⇒ Trên hai cửa sổ đầy, companion trông mạnh hơn nền **~10–11%**. Chẻ làm bốn quý
thì **nền vàng thắng 2/4** — **một đồng xu**. Và trên **cả 24** phép so:
companion mạnh hơn nền trên **cả hai** cửa sổ ở **6/24**, nền mạnh hơn companion
trên **cả hai** ở **6/24**, còn lại **12/24** lẫn lộn.

**Con số đáng giữ nhất của cả job:** phép so **duy nhất** sống sót qua ba đối
chứng cùng tỉ lệ cắt trên hai cửa sổ được **chính biến động của vàng tái tạo
89,6% / 90,4%** — và phần **10%** còn lại **lật** xem bên nào thắng ở **2 trong
4 quý**. Đây **cùng một cái bẫy** `agent/lead-lag` bắt được ở Q4 của nó
(companion −0,0259 vs nền −0,0268, trùng tới chữ số thứ ba): **cái gọi là "thông
tin của companion" là chế độ của chính vàng đi qua một đường khác.** Đó là
**hiện vật cửa sổ thứ 12** của hồ sơ, xuất hiện lần thứ hai, trên một trục khác.

## 6. F3 — trần, và vì sao 0 ô cổng là câu trả lời đúng

| arm | `E` cao nhất của bất kỳ nhóm nào | ở đâu | cổng đòi |
| --- | --- | --- | --- |
| guards ON | **−0,03902 R** | XAGDUKA dir donchian-breakout, cửa sổ B | **≥ +0,050 R** |
| guards OFF | **−0,02715 R** | XAGDUKA dir bb-fade, cửa sổ A | **≥ +0,050 R** |

Không nhóm nào của không phép so nào, trên không cửa sổ nào, có `E` dương. Nhóm
tốt nhất vẫn **thiếu 0,089 R mỗi lệnh** so với vạch. Bộ lọc **chia một sổ lỗ
thành "ít lỗ" và "lỗ hơn"**, nó không làm ra một sổ lãi. Hai phép so "gần" ở mục
4 (`EURDUKA move/vol × donchian-breakout`, p 3,0–5,4%) cũng vậy: `E` nhóm tốt
nhất của chúng là **−0,04904 R** và **−0,05950 R** (cả hai ở cửa sổ B, nhóm IN).

⇒ **Tiêu 0 ô cổng.** Và tôi **không** cài một `Filter` companion vào engine —
làm thế là tiêu ô cổng cho một thứ đã biết trần của nó.

**Một ca `null != 0` cần ghi, trong arm guards OFF:** nền vàng `dir × bb-fade`
cửa sổ B in **IN n 52, E +0,16119 R, PF_r 1,3458, win 51,9%** — nhóm **duy
nhất** dương của toàn bộ phép đo. Nó **cắt 99,6%** lệnh (52 giữ / 12.451 bỏ),
nó **trượt** đối chứng xáo trộn nhãn (**8,2%**), cửa sổ A của nó đọc **+0,03748
(p 4,5%)**, và nó ở **arm chủ đã cấm**. Đây đúng hình dạng hồ sơ cảnh báo
(`PF 1,753 / phân vị 96 / 14 lệnh`) — **không phải** một ứng viên, và nó **không
phải companion** mà là trạng thái của chính vàng.

## 7. Bẫy dụng cụ đo được trong lần chạy này

- **`cap_lots` bịt phần lớn số lệnh, và CẢ HAI chân cổng mù với nó.** Đọc trực
  tiếp từ `BacktestResult::sized_down_by_guard` (**không** grep): arm guards ON
  `bb-fade` **11.101/12.061 entries = 92,0%** (cửa sổ A), **10.399/11.803 =
  88,1%** (B); `ema-cross` **2.697/2.955 = 91,3%**; `donchian-breakout`
  **6.554/8.396 = 78,1%**. Arm guards OFF: **0/…** trên mọi dòng. ⇒ `PF_usd`
  của arm guards ON là PF của một sổ **bị trần notional bóp 78–92% số lệnh**;
  `r = points/risk` không đọc `lots` nên **`PF_r` và `E` không bị ảnh hưởng** —
  đó là lý do mọi con số quyết định ở trên là `PF_r`/`E`.
- **Khe `PF_usd` − `PF_r` LẬT DẤU giữa hai cửa sổ trên CÙNG MỘT dòng** (bằng
  chứng thêm cho phụ lục 9 §I, và nó bác luôn mọi tỉ lệ cố định):

  | cơ chế (guards ON) | A: PF_usd / PF_r | khe A | B: PF_usd / PF_r | khe B |
  | --- | --- | --- | --- | --- |
  | **bb-fade** | 0,8909 / 0,7892 | **+0,1017** | 0,7364 / 0,8135 | **−0,0771** |
  | **rsi-reversion** | 0,8389 / 0,7641 | **+0,0748** | 0,8161 / 0,8310 | **−0,0149** |
  | ema-cross | 0,7253 / 0,7430 | −0,0177 | 0,7621 / 0,7731 | −0,0110 |
  | donchian-breakout | 0,7719 / 0,8024 | −0,0305 | 0,7987 / 0,8350 | −0,0363 |

  **2/4 cơ chế lật dấu khe giữa hai cửa sổ.** ⇒ đọc cổng bằng `PF_usd` ở đây sẽ
  đổi **thứ tự** của bb-fade và rsi-reversion giữa hai cửa sổ.
- **Mọi sổ sụt 89,3–99,85% của đỉnh vốn.** `max_drawdown_usd` 9.030–11.100 USD
  trên `starting_equity_usd` **10.000** (thừa kế — `markets.xauduka` **không**
  đặt riêng, nên **khuyết điểm 17 không áp dụng**, đã kiểm chứ không giả định).
  **Không dòng nào in `_pct > 100%`** nên không dòng nào chính thức "đã cháy",
  nhưng `bb-fade` A ở **99,85%** là cách vạch cháy **0,15%**. **Đừng đọc `PF`
  của những dòng này như PF của một phương pháp**; chúng ở đây làm **nền** cho
  một phép phân hoạch, không phải làm ứng viên.
- **Mốc nghỉ CME KHÔNG giải thích hiệu ứng.** `agent/hour-screen` đo dấu giá của
  nhà cung cấp ở 17:00 NY trên **cả hai** feed Duka (`open==low` 19,02% vàng /
  24,16% bạc vs 3,20% / 4,86% nến thường). Bỏ **mọi** lệnh có nến tín hiệu trong
  **±120 phút** quanh 17:00 NY (**730** lệnh cửa sổ A, **604** cửa sổ B của
  `bb-fade`) thì hiệu ứng **mạnh hơn một chút**: ΔE **+0,07666 → +0,08085** (A),
  **+0,05334 → +0,06007** (B). ⇒ hiệu ứng **không** là hiện vật bút của nhà cung
  cấp. Nhưng nền vàng cũng mạnh lên cùng nhịp (+0,06870 → +0,07206), nên **F6
  không đổi**.
- **Tiền kiểm KHÔNG đổi cỡ stop và KHÔNG đổi phân bố cỡ lot** — nó phân hoạch
  **một** lần chạy, nên khuyết điểm `agent/mould-target` vừa đo (ở arm guards ON,
  `max_open_loss_r = 2.0` tính bằng R và trần notional 300% làm "stop tự-quản chỉ
  là mẫu số" **sai tới 81%**) **không** chạm vào phép đo này. Mọi cơ chế ở đây
  cũng là `Exits::Engine`, không phải tự-quản.

## 8. Sổ đa phép thử — khai vs xem

| | khai trước | thật sự xem | tiêu ô cổng |
| --- | --- | --- | --- |
| phép so companion (2 companion × 3 trạng thái × 4 cơ chế) | **24** | **24** | 0 |
| × 2 cửa sổ × 2 arm guards (khai là 96 con số, không phải 96 phép thử) | 96 số | **96 số** | 0 |
| nền tự-trạng-thái vàng (F6, khai ở ghi chú có ngày) | 12 | **12** (+12 ở arm OFF) | 0 |
| ổn định theo quý (F7, khai ở ghi chú có ngày) | chẻ 2 cửa sổ thành 4 quý, cùng 36 cặp | **288 số** (36 cặp × 4 quý × 2 arm) | 0 |
| bỏ nến mốc nghỉ (thêm theo yêu cầu điều phối, giữa job) | không khai trước | **144 số** (36 cặp × 2 cửa sổ × 2 arm) | 0 |
| test nhân quả + probe nhìn trước (F2) | khai | **6 test Rust**, qua 6/6 | 0 |
| **ô cổng** | **có điều kiện, 0 nếu tiền kiểm trượt** | **0** | **0** |

**Vượt khai:** F7 đo trên **cả ba** series (hai companion + nền) chứ không chỉ
trên phép so sống sót — tốn thêm, và nó **đổi kết luận** (chính nó cho con số
"nền thắng 2/4 quý"). Phần "bỏ nến mốc nghỉ" được điều phối yêu cầu **sau** khi
đăng ký; nó khai ở đây, và nó **không** tạo ra phép so mới mà chỉ đọc lại 36 cặp
đã có trên một tập lệnh nhỏ hơn.

## 9. Thứ KHÔNG đo được, và vì sao

- **Ba nhánh filter-on / filter-off / không-filter**, và cùng với nó mọi số của
  một ô cổng (`PF_r` + `PF_usd` của ô, `max_drawdown_usd` của ô, `--exit-mix`
  của ô, `matched_rate`, phân vị null, `null p50`). **Không đo được vì 0 ô cổng
  được tiêu**, và đó là tuân đăng ký (F3 bắn), không phải thiếu sót. Số lệnh ba
  nhánh (`month-clock` +17…+22, `m8` +36) **vẫn là số của người khác** ở báo cáo
  này — tôi **không** trích nó như số của mình.
- **Ngưỡng khác của trạng thái.** `0,25` / `0,75` / `trung vị 500 nến` **chốt ở
  đăng ký, không quét**. Một ngưỡng khác có thể cho ΔE khác; mỗi ngưỡng là một
  phép thử nữa trên 24 đã khai, và F3 nói trần ở **−0,039 R** nên quét ngưỡng
  không đổi được kết luận về cổng.
- **Trạng thái companion phi tuyến hay nhiều biến.** Tôi đo **ba** trạng thái
  một-biến. "Bạc ồn VÀ cùng chiều" chẳng hạn **chưa được đo**.
- **Cơ chế khác.** Bốn baseline. `donchian-breakout` và `bb-fade` ở đây là sổ lỗ
  sâu; một cơ chế **đã dương** có thể phản ứng khác với cùng bộ lọc. Nhưng hồ sơ
  **không có** cơ chế vàng dương nào ở arm chủ cho phép để lọc — đó là lý do
  trần F3 nói nhiều hơn mọi `p`-value ở đây.
- **Khung khác 15m.** Dấu của edge là thuộc tính của thước 15m (+0,114R →
  −0,065R → −0,043R khi chỉ đổi cỡ nến). **Không** suy ra gì cho 1m/5m.
- **`BTCUSDT` làm companion của vàng**: feed bắt đầu **2024-09-12**, cửa sổ A
  **null**, không phải 0.

## 10. Thứ kết quả này KHÔNG nói

- **Không** nói hook companion đóng hoàn toàn. Falsifier của brief đòi "**không**
  phân biệt được hơn một bộ lọc ngẫu nhiên cùng tỉ lệ cắt, trên cả hai cửa sổ" —
  **1/24 phép so phân biệt được**, bền qua hai arm, bền qua việc bỏ nến mốc nghỉ,
  không lật dấu qua 4 quý. **Chiều đồng thời KHÔNG trắng như chiều đi trước.**
  Cái đóng nó là **F6** (nền vàng tái tạo 90%) và **F3** (trần −0,039 R), **không
  phải** F1.
- **Không** nói biến động của bạc vô nghĩa với vàng: trễ 0 vàng/bạc là
  **+0,69 / +0,74** (`agent/lead-lag`), và trạng thái `vol` của bạc phân biệt
  **+0,077 R** trên 11.952 lệnh. Nó nói rằng **gần như toàn bộ** thông tin đó đã
  có trong **chính vàng**, miễn phí, không cần feed thứ hai.
- **Không** nói `companion_unconfirmed` hay `far_stop_break` sai: chúng đọc
  companion ở trễ 0, đúng chỗ thông tin **có**.
- **Không** nói `bb-fade` nên giao dịch. Nó lỗ **−0,10 đến −0,11 R mỗi lệnh**
  trên cả hai cửa sổ và sụt **99,7–99,85%** đỉnh vốn.
- **Không** đề xuất một phép thử companion nào tiếp. Xem mục 11.

## 11. Thứ mở lại được, và thứ không

**Mở lại được, nhưng phải trả một giá rõ:** một trạng thái companion **nhiều
biến** hoặc trên **khung khác**. Nhưng nó phải vượt **F6**, tức phải chứng minh
nó mang thông tin **vàng không tự có**, và nó phải vượt **F3**, tức nhóm nó giữ
phải có `E ≥ +0,050R` — trong khi nhóm tốt nhất của 24 phép so này là
**−0,039 R**. Hai cửa ải đó là lý do tôi **không** đề xuất phép thử thứ 25.

**Không mở lại được:** companion làm **nguồn tín hiệu đi trước** (`agent/lead-lag`
đã đóng: 1/12 tổ hợp vượt nền trên hai cửa sổ, trần gross thiếu 5,4 lần).

**Câu trả lời cho câu hỏi của desk về instrument thứ hai, bằng một dòng:**
trên đĩa này, ở khung 15m, **instrument thứ hai không đi trước** (`lead-lag`) và
**trạng thái đồng thời của nó gần như chỉ là trạng thái của chính instrument thứ
nhất** (job này: nền tái tạo **89,6% / 90,4%**, và thắng ở **2/4 quý**). Thứ còn
thiếu để một bộ lọc chế độ có giá trị **không phải** một companion tốt hơn — mà
là **một cơ chế vàng có `E` dương để lọc**. Hồ sơ chưa có cái đó ở arm chủ cho phép.

## 12. Nhánh, commit, đĩa

- Nhánh `agent/companion-regime`, cắt từ `agent/stop-width` (`29c8635`).
- `e739f8c` đăng ký trước (chỉ docs) · `24cdee7` ghi chú có ngày (chỉ docs) ·
  commit code + receipts + file này sau đó.
- `df -h /e`: **16 GB trước**, **14 GB sau** (trên sàn 3 GB; thủ phạm các cú tụt
  là `pagefile.sys`, **không** phải `target/` — không xoá gì).
- `collect.exe` pid **5044** và **38720** **vẫn chạy** (kiểm bằng `tasklist` sau
  khi xong). Không chạm `data/gold/`, `data/btc/`,
  `/e/rust/flowdesk/target/release/`, `main`, `config/accounts.toml`,
  `config/local.toml`, `data-sealed/`, VPS.
