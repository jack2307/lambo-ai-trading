# KẾT QUẢ — không instrument nào trên đĩa này đi trước instrument nào ở 15m

**Ngày:** 2026-10-09. Nhánh `agent/lead-lag`, cắt từ `agent/stop-width`
(`29c8635`). Đăng ký: `docs/decisions/2026-10-09-lead-lag.md` (`983b808`), cộng
một ghi chú có ngày ở cuối nó (`21d7d1e`).

**Câu hỏi:** companion dùng làm **biến dự báo có hướng** — A dịch chuyển trước,
ta giao dịch **B** theo chiều của A — có mang thêm thông tin so với **chính quá
khứ của B** không? (Không phải tỉ số `agent/n2`, không phải cross bản địa
`agent/aud`, không phải pairs trade.)

**Kết luận:** **Không.** Companion **thua** nền tự tương quan của B ở **21/28**
cell đo được — companion thắng ở **7/28 (25,0%)** trong khi tung xu cho 50%. Ô
cổng tiêu: **0**.

## 1. Cổng

**0 ô cổng tiêu**, như đã đăng ký khi falsifier bắn ở tiền kiểm (brief §5, tiền
lệ `agent/aud` khai 432 tiêu 0). Khai trước, có điều kiện: 108 ô nếu tiền kiểm
qua. **Không ô nào được tiêu**, nên **không có `PF_r`, không `PF_usd`, không
drawdown, không `--exit-mix`, không phân vị** để báo — và điều đó là **thiết
kế**, không phải thiếu sót.

## 2. Falsifier bắn ở đâu

| falsifier | bắn? | ở đâu | con số |
| --- | --- | --- | --- |
| **F1** nền tự tương quan | **gần như** — 27/28 cell | tiền kiểm | companion vượt nền ở **7/28** cell, và vượt trên **cả hai** cửa sổ cùng dấu ở **1/12** tổ hợp cặp-trễ (tung xu cho 1,5/12) |
| **F1b** trần gross | **BẮN, 16/16 dòng** | tiền kiểm | `gross − drift` cao nhất **+0,0153 R/lệnh** so với `cost/R` **0,0819** ⇒ net **−0,0666 R/lệnh**; mọi dòng âm |
| **F1c** ruột của thước | không bắn (thước lành) | — | 11/11 phép kiểm qua |
| **F1d** ổn định | **BẮN** | tiền kiểm | cell duy nhất qua chữ F1 **lật dấu** giữa Q3 (+0,0503, t +4,89) và Q4 (−0,0259, t −2,50) |
| **F2** chi phí | **BẮN** | tiền kiểm | xem F1b; và cost/R vàng đo được **8,35–12,58%**, không phải 4,04% (mục 5) |
| **F4** drift | không bắn (và không cần) | — | `long_share` **49,5–51,3%** ⇒ phần drift **≤ 0,00014 R**, nhỏ hơn gross 100 lần. Edge **không phải** phơi nhiễm drift; nó chỉ đơn giản là **không có** |
| **F3** cổng | **không chạy** | — | 0 ô |
| **F5** nhân quả | không bắn | — | cắt chuỗi hai phía, phần dư **0 tới 12 chữ số** |

F1 như tôi khai có một khuyết điểm tôi tự bắt được sau khi thấy bảng và ghi lại
**trước** khi chạy tiếp (ghi chú có ngày, `21d7d1e`): nó không đòi companion
**phân biệt được với 0**, nên "vượt nền" ở một cell có thể là so hai con số đều
là 0. Đúng 1/32 cell qua chữ của nó, với `|t| = 1,34` và `0,68`. F1b và F1d
được khai thêm để trả lời đúng chỗ đó, và cả hai bắn.

## 3. Đã đo gì

- Dữ liệu: `/e/rust/flowdesk/data/bars/*-15m.parquet`, **chỉ đọc**, không ghi gì.
  `XAUDUKA` 378.749 nến · `XAGDUKA` 374.188 · `EURDUKA` 398.220 ·
  `BTCUSDT` 70.080 · `XAUUSD` 100.586.
- Hai cửa sổ của desk: **A 2010-06-01 → 2018-06-01**, **B 2018-06-01 → 2026-06-01**.
- Bốn cặp khai trước: `XAG→XAU`, `EUR→XAU`, `XAU→XAG`, `BTC→XAU`.
- Độ trễ `k ∈ {1, 2, 4, 8}` nến 15m. **Trễ 0 in để tham chiếu, KHÔNG tính là
  lead-lag.**
- Công cụ: `py/leadlag_precheck.py`, `py/leadlag_gross.py`,
  `py/leadlag_selftest.py`, `py/leadlag_stability.py` (commit `d8a42e1`). Chạy:
  `python py/leadlag_<x>.py /e/rust/flowdesk/data`.
- **Không dùng `search.exe`** — không có ô cổng nào để chạy.

Ba thứ làm phép đo trung thực, mỗi thứ là một dòng code:

1. **Khớp timestamp CHÍNH XÁC**, không nearest, không ffill — cùng luật
   `companion::aligned_change` (`crates/fd-indicators/src/companion.rs`) đang
   dùng: `continue` khi thiếu bar, **không** lấy bar trước.
2. **`k` nến liền mạch**: một lợi nhuận `k` nến chỉ được nhận khi
   `time[t] − time[t−k] == k × 900.000 ms` ở **cả hai** series. Vắt qua cuối
   tuần hay break ngày thì **từ chối**, không báo.
3. **Mẫu KHÔNG chồng lấn**: bước lấy mẫu bằng `k`, nên `t` không bị phồng. Nền
   tự tương quan đo bằng **cùng script, cùng lưới (giao của A và B), cùng bước,
   cùng luật liền mạch** — nếu không thì phép so không phải một phép so.

## 4. Bảng quyết định — companion so với chính quá khứ của B

Trễ 0, để thấy quy mô thứ **không** giao dịch được:

| cặp | cửa sổ A | cửa sổ B | n |
| --- | --- | --- | --- |
| `XAG↔XAU` | **+0,6900** (t +412,93) | **+0,7446** (t +474,76) | 187.628 / 181.108 |
| `EUR↔XAU` | +0,3185 (t +146,15) | +0,3442 (t +157,73) | 189.196 / 185.165 |
| `BTC↔XAU` | **null** (hai feed bắt đầu 2024-09-12 và 2022-06-16) | +0,1662 (t +33,66) | — / 39.910 |

Trễ ≥ 1, cạnh nền. `base` = `corr(rB[t−k], rB[t])` trên **cùng** mẫu:

| cặp | cửa sổ | k | corr | t | n | base | base t | |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| XAG→XAU | A | 1 | +0,0040 | +1,73 | 185.945 | −0,0048 | −2,06 | thua |
| XAG→XAU | A | 2 | +0,0068 | +2,05 | 91.301 | −0,0077 | −2,33 | thua |
| XAG→XAU | A | 4 | +0,0181 | **+3,81** | 43.993 | +0,0031 | +0,66 | vượt |
| XAG→XAU | A | 8 | +0,0094 | +1,34 | 20.358 | +0,0001 | +0,02 | vượt |
| XAG→XAU | B | 1 | +0,0029 | +1,22 | 179.013 | −0,0148 | −6,27 | thua |
| XAG→XAU | B | 2 | +0,0098 | +2,89 | 87.420 | −0,0062 | −1,84 | vượt |
| XAG→XAU | B | 4 | −0,0038 | −0,77 | 41.648 | −0,0098 | −1,99 | thua |
| XAG→XAU | B | 8 | +0,0049 | +0,68 | 18.809 | −0,0026 | −0,36 | vượt |
| EUR→XAU | A | 1 | +0,0057 | +2,46 | 187.614 | −0,0048 | −2,07 | vượt |
| EUR→XAU | A | 2 | +0,0054 | +1,65 | 92.230 | −0,0081 | −2,45 | thua |
| EUR→XAU | A | 4 | −0,0091 | −1,91 | 44.540 | +0,0028 | +0,59 | vượt |
| EUR→XAU | A | 8 | −0,0045 | −0,65 | 20.700 | +0,0047 | +0,68 | thua |
| EUR→XAU | B | 1 | +0,0038 | +1,63 | 183.101 | −0,0142 | −6,07 | thua |
| EUR→XAU | B | 2 | −0,0002 | −0,06 | 89.487 | −0,0144 | −4,32 | thua |
| EUR→XAU | B | 4 | +0,0041 | +0,85 | 42.687 | −0,0095 | −1,96 | thua |
| EUR→XAU | B | 8 | +0,0041 | +0,57 | 19.299 | −0,0083 | −1,15 | thua |
| XAU→XAG | A | 1 | +0,0002 | +0,10 | 185.945 | **−0,0829** | **−35,89** | thua |
| XAU→XAG | A | 2 | −0,0083 | −2,52 | 91.301 | −0,0567 | −17,17 | thua |
| XAU→XAG | A | 4 | −0,0148 | −3,11 | 43.993 | −0,0445 | −9,34 | thua |
| XAU→XAG | A | 8 | −0,0191 | −2,72 | 20.358 | −0,0349 | −4,98 | thua |
| XAU→XAG | B | 1 | −0,0077 | −3,27 | 179.013 | −0,0101 | −4,29 | thua |
| XAU→XAG | B | 2 | −0,0107 | −3,15 | 87.420 | −0,0157 | −4,64 | thua |
| XAU→XAG | B | 4 | −0,0095 | −1,95 | 41.648 | −0,0147 | −3,00 | thua |
| XAU→XAG | B | 8 | −0,0035 | −0,48 | 18.809 | −0,0016 | −0,22 | vượt |
| BTC→XAU | A | 1,2,4,8 | **null** | **null** | 0 | **null** | **null** | null |
| BTC→XAU | B | 1 | +0,0004 | +0,07 | 39.467 | −0,0010 | −0,20 | thua |
| BTC→XAU | B | 2 | −0,0023 | −0,31 | 19.291 | −0,0191 | −2,66 | thua |
| BTC→XAU | B | 4 | +0,0051 | +0,49 | 9.203 | −0,0220 | −2,11 | thua |
| BTC→XAU | B | 8 | −0,0021 | −0,14 | 4.160 | +0,0054 | +0,35 | thua |

**Vượt 7/28 = 25,0%**, tung xu cho 50%. Và **1/12** tổ hợp cặp-trễ vượt trên
**cả hai** cửa sổ cùng dấu, trong khi tung xu cho **1,5/12** — companion không
chỉ không thắng nền, nó thua **thường xuyên hơn** một đồng xu.

**Dòng đáng giữ nhất của cả bảng:** `XAU→XAG` cửa sổ A trễ 1 — vàng ở nến trước
nói về bạc với `corr +0,0002`, còn **bạc ở nến trước nói về bạc với
`−0,0829` (t −35,89)**, trên cùng 185.945 mẫu. **Để giao dịch bạc, quá khứ của
chính bạc mang thông tin gấp ~400 lần quá khứ của vàng.** Đó là phép so quyết
định của đăng ký, và nó trả lời rõ hơn mọi `p`-value: companion không phải là
nguồn thông tin đi trước — nó là **nguồn tệ hơn thứ đã có sẵn và miễn phí**.

### Cell duy nhất qua chữ của F1, chẻ làm bốn (F1d)

`XAG→XAU` trễ 8:

| slice | corr | t | n | base | base t |
| --- | --- | --- | --- | --- | --- |
| A 2010-06..2018-06 | +0,0094 | +1,34 | 20.358 | +0,0001 | +0,02 |
| B 2018-06..2026-06 | +0,0049 | +0,68 | 18.809 | −0,0026 | −0,36 |
| Q1 2010-06..2014-06 | +0,0039 | +0,41 | 10.843 | −0,0086 | −0,90 |
| Q2 2014-06..2018-06 | +0,0127 | +1,24 | 9.521 | +0,0067 | +0,65 |
| Q3 2018-06..2022-06 | **+0,0503** | **+4,89** | 9.443 | +0,0245 | +2,38 |
| Q4 2022-06..2026-06 | **−0,0259** | **−2,50** | 9.366 | −0,0268 | −2,60 |

**Lật dấu giữa hai quý liền nhau**, đúng như đã dự đoán trong ghi chú trước khi
chạy. Và chú ý cột `base`: ở Q4 companion đọc **−0,0259** trong khi nền đọc
**−0,0268** — **giống nhau tới chữ số thứ ba**. Cái gọi là "thông tin của
companion" ở quý đó **chính là** tự tương quan của vàng, đi qua một đường khác.
Đây là **hiện vật cửa sổ thứ 12** của hồ sơ.

Cell duy nhất có `|t| > 2` trên một cửa sổ đầy (`XAG→XAU` trễ 4, A, t +3,81)
cũng vậy: Q1 +0,0257 (t +3,89) → Q4 **−0,0214 (t −3,09)**, và ở Q4 nền đọc
−0,0318, **mạnh hơn companion**.

## 5. Trần gross — F1b, và đây là chỗ trục chết dứt điểm

Luật đo: `sign(lợi nhuận k nến của A kết thúc ở i)` ⇒ giữ **B** `k` nến tiếp.
A kết thúc ở `i`, B di chuyển trên `[i, i+k]` ⇒ **không đọc gì trước khi nó
đóng**. `R = 1,5 × ATR(15m)` của instrument **được giao dịch** (ATR = `rma` của
true range, khớp `fd_indicators::atr`). Spread đọc từ `config/default.toml`:
vàng **0,28**, bạc **0,021**, EUR **0,00014**.

| cặp | cửa sổ | k | n | gross R | t | long% | drift R | gross−drift | cost/R | **net R** |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| XAG→XAU | A | 1 | 180.939 | +0,00211 | +1,80 | 49,9% | +0,00000 | +0,00211 | 0,1248 | **−0,12271** |
| XAG→XAU | A | 2 | 89.665 | +0,00408 | +1,71 | 50,1% | −0,00000 | +0,00408 | 0,1249 | **−0,12082** |
| XAG→XAU | A | 4 | 43.488 | −0,00349 | −0,70 | 50,0% | +0,00000 | −0,00349 | 0,1247 | **−0,12816** |
| XAG→XAU | A | 8 | 20.219 | +0,00615 | +0,57 | 50,0% | +0,00000 | +0,00615 | 0,1242 | **−0,11802** |
| XAG→XAU | B | 1 | 175.246 | −0,00211 | −1,83 | 50,7% | +0,00001 | −0,00212 | 0,0824 | **−0,08456** |
| XAG→XAU | B | 2 | 86.271 | −0,00491 | −2,10 | 50,8% | +0,00003 | −0,00494 | 0,0825 | **−0,08746** |
| XAG→XAU | B | 4 | 41.290 | −0,00274 | −0,56 | 51,3% | +0,00009 | −0,00283 | 0,0823 | **−0,08515** |
| XAG→XAU | B | 8 | 18.710 | **+0,01544** | +1,44 | 51,0% | +0,00014 | **+0,01530** | 0,0819 | **−0,06663** |
| EUR→XAU | A | 1 | 185.494 | +0,00311 | +2,70 | 50,1% | −0,00000 | +0,00311 | 0,1254 | **−0,12227** |
| EUR→XAU | A | 2 | 91.551 | +0,00260 | +1,10 | 50,1% | −0,00000 | +0,00261 | 0,1252 | **−0,12262** |
| EUR→XAU | A | 4 | 44.294 | −0,00390 | −0,79 | 49,9% | +0,00000 | −0,00390 | 0,1250 | **−0,12888** |
| EUR→XAU | A | 8 | 20.624 | −0,01162 | −1,09 | 50,0% | −0,00000 | −0,01162 | 0,1246 | **−0,13625** |
| EUR→XAU | B | 1 | 180.478 | +0,00156 | +1,38 | 50,0% | +0,00000 | +0,00156 | 0,0833 | **−0,08174** |
| EUR→XAU | B | 2 | 88.588 | +0,00008 | +0,03 | 50,0% | +0,00000 | +0,00008 | 0,0833 | **−0,08319** |
| EUR→XAU | B | 4 | 42.409 | +0,00249 | +0,51 | 49,7% | −0,00002 | +0,00251 | 0,0830 | **−0,08049** |
| EUR→XAU | B | 8 | 19.231 | +0,01228 | +1,14 | 49,5% | −0,00007 | +0,01235 | 0,0824 | **−0,07007** |

**16/16 dòng net âm.** Dòng tốt nhất của cả bảng — `XAG→XAU` cửa sổ B trễ 8 —
mang gross **+0,0153 R/lệnh** và phải trả **0,0819 R** spread: nó thiếu
**5,4 lần**. Và `|t|` của nó là **1,44**: ngay cả cái gross đó cũng không phân
biệt được với 0 trên 18.710 mẫu không chồng lấn.

**Drift không liên quan (F4).** `long_share` nằm trong **49,5%–51,3%** ở cả 16
dòng, nên phần drift giải thích được là **≤ 0,00014 R**, nhỏ hơn gross hơn 100
lần. Edge này **không phải** phơi nhiễm drift — nó **không tồn tại**.

### Con số khác hồ sơ — số đo thắng (brief §8)

`spread / (1,5 × ATR(15m))` **không** ra 4,04% cho vàng ở **bất kỳ** cửa sổ nào
của desk. Đo (`spread` đọc từ config, ATR = `rma(true_range, 14)`):

| sym | cửa sổ | trung vị 1,5×ATR | cost/R trung vị | cost/R trung bình | hồ sơ ghi |
| --- | --- | --- | --- | --- | --- |
| XAUDUKA | A 2010-06..2018-06 | 2,22612 | **12,58%** | 10,79% | 4,04% |
| XAUDUKA | B 2018-06..2026-06 | 3,35178 | **8,35%** | 5,79% | 4,04% |
| XAUDUKA | full 2010-06..2026-06 | 2,65001 | 10,57% | 7,55% | 4,04% |
| XAUDUKA | **recent 2024-06..2026-06** | 7,05568 | **3,97%** | 2,84% | 4,04% |
| XAUDUKA | 3m 2026-03..2026-06 | 16,11959 | 1,74% | 1,48% | 4,04% |
| XAGDUKA | A | 0,06936 | **30,28%** | 20,47% | 17,32% |
| XAGDUKA | B | 0,08642 | 24,30% | 15,27% | 17,32% |
| XAGDUKA | **full** | 0,07809 | 26,89% | **17,53%** | 17,32% |
| EURDUKA | A | 0,00127 | 11,04% | 9,83% | 14,81% |
| EURDUKA | **B** | 0,00090 | **15,51%** | 13,92% | 14,81% |

⇒ **Ba con số cost/R của hồ sơ không nằm trên cùng một cửa sổ và cùng một thống
kê.** Vàng 4,04% khớp **trung vị của cửa sổ 2 năm gần nhất** (3,97%); bạc
17,32% khớp **trung bình của cả 16 năm** (17,53%); EUR 14,81% khớp **trung vị
cửa sổ B** (15,51%). Trên hai cửa sổ desk thật dùng, cost/R của vàng là
**8,35%–12,58%**, tức **2–3 lần** con số đang được trích. Nguyên nhân cơ học:
ATR tính bằng **USD** nên nó lớn lên cùng giá vàng (1,5×ATR: **2,23 USD** ở cửa
sổ A → **16,12 USD** ở ba tháng gần nhất), còn spread thì cố định 0,28. **Cùng
một spread, cùng một hệ số 1,5, chi phí/R chênh 7 lần giữa hai đầu hồ sơ.**

Tôi **không chỉnh** gì theo con số này (cấm chỉnh cổng, và mọi receipt cũ đo ở
0,28). Tôi báo nó: **mọi lần ai trích "vàng chỉ tốn 4,04% của R" cho một kết
quả đo trên cửa sổ 2010-2018 hay 2018-2026, con số đúng là 12,58% hay 8,35%** —
và đó là một hệ số 2-3 trên đại lượng quyết định dấu của net.

## 6. Thước có bắn được chiều kia không — F1c

`py/leadlag_selftest.py`, **11/11 qua**. Đây là phần làm cho "không tìm thấy
lead-lag" thành một kết luận thay vì một dụng cụ hỏng:

1. **Chứng thực dương** — chuỗi tổng hợp trong đó A **thật sự** đi trước B 1
   nến (hệ số 0,8): thước đọc **corr +0,6211, t +194,1** trên 59.998 mẫu, nền
   **−0,0022 (t −0,54)**, và trễ 2 yếu hơn trễ 1 (+0,3141). ⇒ **Thước phát hiện
   được một lead-lag thật, to và rõ.** Nên con số **+0,0094** đo trên đĩa là
   một phát biểu về thị trường, không phải về dụng cụ.
2. **Chứng thực nhìn trước** — nạp A = tương lai của B: thước đọc
   **corr +1,000000** ⇒ nó **sẽ** bắt được một look-ahead chứ không im lặng báo
   nó như một edge.
3. **Nhân quả (F5)** — cắt chuỗi **companion** ở `T`: `+0,163490013428` so với
   `+0,163490013428`, `n` 4.998 so với 4.998 — **0 tới 12 chữ số**. Cắt chuỗi
   **primary** cũng vậy. Đây là đúng tính chất
   `companion.rs::a_truncated_primary_is_a_prefix_and_so_is_a_truncated_companion`
   khẳng định, ở phía phép đo.
4. **Phồng `t` do chồng lấn** — cùng một tương quan, bước 1 thay vì bước `k`:

   | k | `t` bước k (n) | `t` bước 1 (n) | tỉ số | `sqrt(k)` |
   | --- | --- | --- | --- | --- |
   | 2 | +14,01 (29.998) | +18,72 (59.996) | 1,34 | 1,41 |
   | 4 | +4,06 (14.998) | +9,30 (59.992) | 2,29 | 2,00 |
   | 8 | +1,33 (7.498) | +3,78 (59.984) | **2,85** | **2,83** |

   ⇒ **Luật "cửa sổ chồng lấn phồng `t` lên ~`sqrt(overlap)`" của hồ sơ là
   ĐÚNG, và giờ đã được đo chứ không chỉ được nhắc.** Nó cũng cho thấy chuyện
   gì sẽ xảy ra nếu tôi lấy mẫu chồng lấn: trễ 8 sẽ in `t +3,78` thay vì
   `+1,33` — **một dương nhầm `|t| > 2` làm ra từ hư không**, và đó đúng là
   hình dạng của con số drift `+6,74` mà hồ sơ đang nghi.

## 7. Sổ đa phép thử — khai vs xem

| | khai trước | thật sự xem | tiêu ô cổng |
| --- | --- | --- | --- |
| tương quan trễ | 32 (4 cặp × 4 trễ × 2 cửa sổ) | **32** (4 null) | 0 |
| nền tự tương quan | 16 | **32** (đo nền cho **mọi** cell gồm trễ 0, không chỉ 8 — tốn thêm, không đổi kết luận) | 0 |
| trễ 0 tham chiếu | không khai riêng | 8 (6 đo được) | 0 |
| trần gross (F1b) | khai trong ghi chú | **16** (2 cặp × 4 trễ × 2 cửa sổ) | 0 |
| ổn định (F1d) | khai trong ghi chú | **12** (2 cell × 6 slice) | 0 |
| calibrate thước | khai trong ghi chú | **30** (3 sym × 5 cửa sổ × 2 thống kê) | 0 |
| ruột thước (F1c) | khai trong ghi chú | **11** phép kiểm tổng hợp | 0 |
| **cơ chế** | **≤ 108 ô, có điều kiện** | **0** | **0** |

Vượt khai: nền đo 32 thay vì 16 (tôi đo nền cho cả trễ 0 để thấy nó bằng 1,0
đúng như phải vậy — một phép tự kiểm, không phải một phép thử thêm). Và tôi
xem **3 cặp** chứ không 4 vì `BTC→XAU` null ở cửa sổ A.

## 8. Thứ KHÔNG đo được, và vì sao

- **`BTC→XAU` ở cửa sổ A: null, không phải 0.** `BTCUSDT-15m` trên đĩa bắt đầu
  **2024-09-12** (70.080 nến, tới 2026-09-12) và `XAUUSD-15m` bắt đầu
  **2022-06-16**, còn cửa sổ A kết thúc 2018-06-01. Giao của hai feed trong cửa
  sổ B chỉ là **2024-09 → 2026-06** (39.910 nến chung). Cặp này **chưa
  được đo**, không phải **đã bị bác bỏ**. Ở cửa sổ B nó đo được và thua nền ở
  cả 4 độ trễ, nhưng một cửa sổ không kết luận được (11 hiện vật cửa sổ).
- **Trễ dưới 1 nến.** Nếu lead-lag tồn tại ở **1 phút** hay dưới nó, thước 15m
  không thấy. `XAUDUKA-1m` / `XAGDUKA-1m` **có** trên đĩa. Đây là phép đo tiếp
  theo đúng nghĩa, và hồ sơ đã có tiền lệ cảnh báo: **dấu của edge là thuộc
  tính của thước 15m** (+0,114R → −0,065R → −0,043R khi chỉ đổi cỡ nến).
  **Không** suy ra gì từ kết quả này về khung 1m.
- **Lead-lag có điều kiện.** Tôi đo tương quan **không điều kiện**. Có thể A chỉ
  đi trước B **quanh tin**, hay **chỉ khi A dịch chuyển lớn**. Tương quan không
  điều kiện ~0 **không** loại bỏ chuyện đó — nhưng mỗi điều kiện là một phép thử
  nữa trên 28 phép thử đã trắng, và trần gross ở mục 5 nói rằng nó phải tìm được
  **hơn 5,4 lần** gross để chỉ hoà spread.
- **Mọi thứ của cổng**: `PF_r`, `PF_usd`, `Lbar`, `max_drawdown_usd`,
  `--exit-mix`, `matched_rate`, phân vị. **Không đo được vì 0 ô cổng được
  tiêu**, và đó là tuân luật, không phải thiếu sót.
- **Vì sao gross âm ở `EUR→XAU` A trễ 8 (−0,0116) mà corr chỉ −0,0045**: hai đại
  lượng khác nhau (một là corr của lợi nhuận log, một là mean của gross chuẩn
  hoá theo ATR có đuôi dày). **Không** ép chúng khớp nhau; cả hai đều trong
  tiếng ồn.

## 9. Câu kết — đúng cái falsifier của brief

> *"Nếu không cặp nào có tương quan trễ vượt nền tự tương quan trên cả hai cửa
> sổ, thì không instrument nào trên đĩa này đi trước instrument nào ở khung
> 15m."*

Đo được: **1/12** tổ hợp cặp-trễ vượt nền trên cả hai cửa sổ cùng dấu (tung xu
cho 1,5/12), cái duy nhất đó có `|t| = 1,34` và `0,68`, **lật dấu** giữa Q3 và
Q4, và ở Q4 nó **trùng với nền tới chữ số thứ ba**. Trần gross của nó là
**+0,0153 R/lệnh** so với spread **0,0819 R** — thiếu **5,4 lần**.

⇒ **Không instrument nào trên đĩa này đi trước instrument nào ở khung 15m.**
Họ lead-lag **đóng bằng tiền kiểm**, **0 ô cổng tiêu**. Và chiều ngược lại của
cùng một hook thì **không đóng**: `corr` trễ 0 của vàng và bạc là **+0,69 /
+0,74** — companion mang **rất nhiều** thông tin **đồng thời**, đúng thứ
`companion_unconfirmed` đã dùng. Thứ nó không mang là **thông tin ĐI TRƯỚC**.

## 10. Thứ gì mở lại được

- **Khung 1m** trên `XAUDUKA-1m` / `XAGDUKA-1m`, cùng ba tính chất (khớp chính
  xác, liền mạch, mẫu không chồng lấn) và cùng nền tự tương quan. Nếu một cặp
  vượt nền ở trễ 1-8 **nến 1m** trên cả hai cửa sổ, trục này mở lại — nhưng khi
  đó trần gross phải trả spread trên một chân trời **ngắn hơn 15 lần**, nên
  `cost/R` là cửa ải, không phải tương quan.
- **Một feed có BTC trùng cửa sổ A.** `BTCUSDT` bắt đầu 2024-09; `BTCUSD-15m`
  có trên đĩa nhưng chưa kiểm span.
- **Một con số đo lại khác bảng ở mục 4.** Hồ sơ này chỉ chạy lại về đúng số của
  chính nó ở **65,3%** ô (phụ lục 8 mục II), nên hãy chạy lại bằng
  `py/leadlag_precheck.py` chứ đừng trích bảng này.

## 11. Thứ kết quả này KHÔNG nói

- **Không** nói vàng và bạc không liên quan: trễ 0 là **+0,69 / +0,74**.
- **Không** nói `companion_unconfirmed` hay `far_stop_break` sai: chúng đọc
  companion ở trễ 0, đúng chỗ thông tin **có**.
- **Không** nói lead-lag không tồn tại ở khung khác, hay có điều kiện (mục 8).
- **Không** nói con số `+0,701` trong doc comment của `companion_unconfirmed.rs`
  sai: tôi chạy lại và được **+0,6900** (A) / **+0,7446** (B), và "dưới +0,01 ở
  mọi trễ −4..+4" gần đúng — tôi đo cao nhất **+0,0181** ở trễ 4 cửa sổ A. Đó là
  **xác nhận độc lập**, không phải một trích dẫn.
- **Không** nói cost/R 4,04% là một con số bịa: nó **đúng** trên cửa sổ 2 năm
  gần nhất (3,97%). Nó **sai cửa sổ** khi trích cho 2010-2018 / 2018-2026.
- **Không** đề xuất một phép thử có điều kiện nào ngay bây giờ: 28 phép thử
  trắng cộng trần gross thiếu 5,4 lần là một lý do mạnh để **không** đi tìm cái
  thứ 29.
