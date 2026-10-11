# KẾT QUẢ — `spread*` **không tồn tại** ở bất kỳ spread không âm nào, trên cả 11 dòng gần cổng nhất của hồ sơ

Đăng ký: `docs/decisions/2026-10-11-cost-breakeven.md`, commit **một mình nó** ở
`35e3283` trước file thiết kế và trước lần gọi `search.exe` đầu tiên của nhánh.
Thiết kế đóng băng: `docs/research/designs/2026-10-11-cost-breakeven.toml`
(`83c43eb`), **14 dòng**. Receipt: `{is,oos}-s{14 điểm spread}.txt`, **28 lần
chạy**, 392 ô, 1,3 MB.

Binary `/e/rust/fd-stop-width/target-sw/release/search.exe` (build 08/10/2026
20:57 từ `agent/stop-width` `29c8635`, **chính là HEAD worktree này** ⇒ **không
build gì**). Mọi receipt in lại
`flags: 13 passed, every one of them read by --mode=hypotheses` và
`news: 747 events (2010-01-08 → 2027-12-08) from E:/rust/flowdesk/data\news\events.parquet`
— thư mục `data/` **mặc định**, bất kể `--data=`. Header lặp lại ở mọi lần chạy:
`trail: off`, `max hold: 4 h (14400000 ms) from [trading] max_hold_ms`,
`swap: long 0.00 / short 0.00 USD per lot per night`,
`guards: on — max_open_loss_r 2.0, notional 300%, weekend flat 16:40 NY, news
flat 60/30 (impact>=3, USD), daily cap 20, loss limit $300, cooldown 30 min`.

---

## 0. Trả lời, một câu

**`spread* = null` cho cả 11 dòng.** Không một spread không âm nào — **kể cả
spread = 0, tức giao dịch MIỄN PHÍ** — làm bất kỳ dòng nào qua cả hai chân cổng
trên cả hai cửa sổ ở arm guards ON.

Dòng gần nhất là **`qs-lb3`**, và nó nói chính xác nó cần gì:

    cua so IS   : E cat vach +0,050 R o  s = +0,0047 USD/vong   (1,7% cua 0,28)
    cua so OOS  : E cat vach +0,050 R o  s = -0,1754 USD/vong   (mot KHOAN BU)
    => spread* tren CA HAI cua so        = -0,1754  ==  -62,6% cua 0,28

Tức để `qs-lb3` qua cổng, OOS đòi **execution miễn phí CỘNG một khoản bù 0,1754
USD mỗi vòng mỗi ounce**. Tính từ spread hiện hành, đó là **0,28 + 0,1754 =
0,4554 USD/vòng = 163% của spread hiện hành** phải được gỡ bỏ.

**10 dòng còn lại còn xa hơn thế**: `spread*` của chúng nằm **dưới −0,28** —
nghĩa là ngay cả một khoản bù bằng **100% spread hiện hành** cộng execution miễn
phí vẫn không đủ cho **một** trong hai cửa sổ.

---

## 1. Cổng: **0 ô**

Không ô nào qua cổng trên cả hai cửa sổ, ở bất kỳ điểm spread nào trong 14 điểm
— nên **không ô cổng nào bị tiêu**, đúng như đăng ký.

**Đếm tay ở 40, không ở `need 30` của tool:** `n` nhỏ nhất trong **toàn bộ 308 ô
ứng viên** là **209** (`qs-qp020` OOS). ⇒ **chân cỡ mẫu không quyết định gì ở
đây**, và khoảng 30–39 lệnh mà brief §4 cảnh báo **không chạm** tới job này.
Tool có in `SURVIVES` ở một số ô (ví dụ `ts-l10`, `qs-h15`, `qs-qp020`, `qs-h30`
ở IS) — đó là **verdict phân vị**, không phải cổng; **không một phân vị nào được
đọc hay công bố ở đây**.

**Chân nào ràng buộc, và đây là chỗ câu chuyện lật:**

| dòng | `Lbar` IS/OOS | chân ràng buộc | `spread*` chân E | `spread*` chân PF |
|---|---|---|---|---|
| `qs-lb3` | 0,208 / 0,209 | **expectancy** | +0,0047 / **−0,1754** | +0,2159 / +0,1102 |
| `ovn1900/long` | 0,087 / 0,098 | **expectancy** | **< −0,28 / < −0,28** | +0,1298 / +0,0251 |
| `qs-lb30` | 0,288 / 0,255 | PF ở IS | −0,2663 / ≥ +0,28 | **< −0,28** / ≥ +0,28 |
| `ts-l5` | 0,169 / 0,183 | expectancy | < −0,28 / < −0,28 | −0,0061 / < −0,28 |
| `ts-l20` | 0,195 / 0,208 | expectancy | < −0,28 / < −0,28 | −0,0371 / < −0,28 |
| `ts-l10` | 0,182 / 0,205 | expectancy ở OOS | ≥ +0,28 / < −0,28 | ≥ +0,28 / < −0,28 |
| `tsmom` | 0,218 / 0,216 | expectancy ở OOS | < −0,28 / **−0,1733** | < −0,28 / +0,1750 |
| `qs-h15` = `qs-h30` | 0,243 / 0,265 | expectancy ở OOS | ≥ +0,28 / < −0,28 | ≥ +0,28 / < −0,28 |
| `qs-qp020` | 0,240 / 0,264 | expectancy ở OOS | ≥ +0,28 / < −0,28 | ≥ +0,28 / < −0,28 |
| `qs-h2` | 0,191 / 0,216 | expectancy | −0,0165 / < −0,28 | +0,2651 / < −0,28 |

`≥ +0,28` nghĩa là chân đó **đã qua ở spread hiện hành** — không có gì để mua.
`< −0,28` nghĩa là chân đó **không qua ngay cả ở đáy lưới đối chứng**.

⇒ **Năm dòng (`ts-l10`, `qs-lb30`, `qs-h15`, `qs-h30`, `qs-qp020`) qua CẢ HAI
chân ở ĐÚNG spread hiện hành 0,28 trên MỘT cửa sổ, và không qua ở cửa sổ kia dù
spread xuống tới −0,28.** Ở năm dòng đó chi phí **không phải** thứ chặn — **cửa
sổ** là. Đó là **hiện vật cửa sổ thứ 13**, và nó được đo ở đây bằng một thứ
không ai quét trước: một trục spread dài 0,56 USD không mua được cái mà việc đổi
nửa hồ sơ lấy đi.

---

## 2. Falsifier — bắn chưa, ở đâu

**F1 — BẮN, ở dạng mạnh nhất có thể.** Falsifier khai: "nếu `spread*` của mọi
dòng dưới 25% của 0,28 (= 0,0700) thì «chi vào chi phí» chết". Đo được: `spread*`
của **mọi** dòng là **null trên toàn bộ dải không âm** — tức dưới **0%**, không
chỉ dưới 25%. ⇒ **«Chi vào CHI PHÍ» chết trên vàng 15m ở mọi dòng gần cổng của
registry này.** Không cơ cấu hoàn phí nào bù được 163% spread, và với 10/11 dòng
thì con số đó **lớn hơn 200%**.

**F1b — KHÔNG bắn.** Không dòng nào hoà ở `>= 0,1680` (60% của 0,28). Hồ sơ
**vẫn chưa có** một con số chỉ vào một việc làm được bằng cách mua chi phí.

**F2 (parity, đối chứng DƯƠNG) — KHÔNG bắn, và nó khớp tới từng chữ số in ra.**
Chân `--spread=0.28` của tôi tái tạo **13/13 dòng** của `mould-target` và
`hour-screen` (arm guards ON): `n`, `PF_r`, `PF_usd`, `E`, `Lbar`, `sized down`,
drawdown USD, `avg_mae`, exit mix. Ví dụ khớp chính xác:

    px-1s        IS  n 2014  PF_usd 0,875  PF_r 0,8771  Lbar 0,1656  E -0,0204
    qs-lb3       IS  n  494  PF_usd 1,1900 PF_r 1,1865  Lbar 0,2077  E +0,0387  dd 880,24 USD
    qs-lb3       OOS n  452  PF_usd 1,1708 PF_r 1,1730  Lbar 0,2085  E +0,0361  dd 1036,10 USD
    ovn1900/long IS  n 1644  PF_usd 1,0814 PF_r 1,0899  Lbar 0,0868  E +0,0078  dd 936,71 USD
    ovn1900/short IS n 1644  PF_usd 0,6345 PF_r 0,6459  Lbar 0,1110  E -0,0393

Phụ lục 8 mục II cảnh báo hồ sơ này **thường không** chạy lại về số của chính
nó (295/452 ô). **Ở đây nó chạy lại đúng.** ⇒ dụng cụ **nhìn thấy được** trước
khi tôi báo một số âm.

**F3 (đối chứng ÂM) — KHÔNG bắn, hai nửa đều đúng.**
- **Đơn điệu:** `E` tăng đơn điệu khi spread giảm ở **28/28 dòng-cửa-sổ**, trên
  **cả 14 điểm**, không một chỗ lật. Và `E` gần như là một **đường thẳng** theo
  `s`: `R²(E ~ s)` **0,99992 – 0,99998** trên 11 ứng viên (thấp nhất cả bảng là
  `ema-cross` OOS 0,99253). ⇒ phép quét đang đọc đúng chi phí, và nội suy giữa
  hai điểm lưới là hợp lệ.
- **Hai probe cố ý nhìn trước ĐỀU TRƯỢT như khai:** `ovn1900/short` (cùng giờ,
  cùng phơi nhiễm, cùng sizing với `ovn1900/long`, chỉ lật chiều) **không qua
  chân nào ở bất kỳ điểm nào trong 14 điểm**, kể cả −0,28 (`PF_r` 0,7744 /
  0,8280 ở `s = 0`). `ema-cross` cũng **không có `spread*`** trên cả hai cửa sổ
  ở mọi điểm.

**F4 (tập lệnh đổi) — KHÔNG bắn trên một ô ứng viên nào, và đó là một phát hiện.**
`mould-target` đo được quét spread đổi tập lệnh ở **151/298 (50,7%) ô
guards-ON**. Trên **308 ô ứng viên của tôi (11 dòng × 2 cửa sổ × 14 điểm)**,
`n` **bất biến tuyệt đối**: 494/452, 749/744, 612/618, 628/649, 398/344,
527/538, 393/374, 241/209, 393/374, 521/480, 1644/1635 — **không đổi một lệnh
nào** qua một trục spread dài 0,56 USD. Nơi `n` ĐỔI là đúng hai dòng đối chứng
không tự quản / cưỡi trần notional: `px-1s` OOS (1982 → 1979, chỉ ở phía âm) và
`ema-cross` (IS 3135 → 3095, OOS 3104 → 2967). ⇒ **F7 của `mould-target` là
thuộc tính của họ `Exits::Engine` + trần notional, không phải của arm guards nói
chung.** Hệ quả cho tôi: mọi `spread*` ở đây là **chính xác trên lưới**, không
phải xấp xỉ.

**F5 (khuyết điểm 16, `delta` nửa spread rò vào đơn vị rủi ro) — bắn ở ĐÚNG MỘT
ô, và ô đó là ô duy nhất từng cắt vạch trên dải không âm.** Mục 4.

**F6 (cháy) — KHÔNG bắn trên ứng viên.** `max_drawdown_pct` lớn nhất trong
**toàn bộ 308 ô ứng viên** là **15,10%** (`qs-h2` OOS ở 0,28). Không ô nào gần
100%. Nó **có** bắn trên đối chứng `ema-cross`: **94,23% của đỉnh = 10.467,19
USD** ở IS 0,28, và ở phía đối chứng âm nó in `_pct` **ÂM** (−8,28%, −0,07%,
−3,69%) — một con số không đọc được, dấu hiệu dòng đó đã đi qua mốc cháy
(khuyết điểm 14). ⇒ **không đọc `PF` của `ema-cross`** như một đại lượng; nó ở
đây chỉ làm probe.

**F7 (cửa sổ) — BẮN, và nó là kết quả thật của job.** Mục 1, bảng `≥ +0,28` vs
`< −0,28`. Năm dòng qua cả hai chân ở spread hiện hành trên một cửa sổ và không
qua ở cửa sổ kia ở mọi spread. **Hiện vật cửa sổ thứ 13.**

---

## 3. Con số của chi phí, đo bằng quét chứ không bằng phép chia

`cost/R` mỗi vòng, đọc là `E(s=0) − E(s=0,28)` trên **tập lệnh y hệt** (F4):
đây là **bốn thứ phụ lục 9 mục II đòi** — cửa sổ, thống kê (trung bình mỗi
lệnh), cỡ stop (giá trị riêng của từng dòng, **không** quét), và spread (0,28,
ĐO — mục 6).

| dòng | cost/R IS | cost/R OOS | R(0,28) IS | R(0,28) OOS | `dE/ds` IS (R mỗi USD) |
|---|---|---|---|---|---|
| `ovn1900/long` | **1,64%** | 1,24% | 15,57 điểm | 23,57 | −0,0594 |
| `qs-qp020` | 1,22% | 0,93% | 23,53 | 26,77 | −0,0438 |
| `qs-lb30` | 1,21% | 0,89% | 21,02 | 33,23 | −0,0434 |
| `qs-h15` / `qs-h30` | 1,20% | 0,89% / 0,88% | 22,91 | 29,58 / 29,10 | −0,0433 |
| `qs-h2` | 1,18% | 0,88% | 22,57 | 29,52 | −0,0425 |
| `qs-lb3` | 1,15% | **0,85%** | 25,30 | 34,07 | −0,0412 |
| `ts-l10` | 0,84% | 0,62% | 34,67 | 46,96 | −0,0303 |
| `ts-l5` / `ts-l20` | 0,83% | 0,62% / 0,60% | 33,60 / 33,46 | 46,62 / 44,51 | −0,0297 |
| `tsmom` | 0,83% | **0,56%** | 29,96 | 51,16 | −0,0298 |
| *(đối chứng)* `ema-cross` | **11,85%** | 9,37% | **3,21** | 3,24 | −0,4626 |

`qs-lb3` đo **1,15% (IS) / 0,85% (OOS)** — khớp phụ lục 5 mục F
(**1,10% / 0,85%** ở 1,5 biên độ ngày NY) tới chữ số thứ hai, phép đo thứ ba độc
lập của cùng đại lượng. `ovn1900/long` đo **1,64% / 1,24%** so với
`hour-screen` công bố **1,19%–1,80%** — cùng dải, hai ruler khác nhau.

**Và đây là tỉ lệ quyết định mọi thứ** — khoảng cách tới cổng ở `s = 0` so với
**toàn bộ** hoá đơn spread:

| dòng | cửa sổ ràng buộc | thiếu `E` ở `s=0` | cả hoá đơn spread | thiếu / hoá đơn |
|---|---|---|---|---|
| `qs-lb3` | OOS | **+0,0054 R** | 0,0085 R | **0,64×** |
| `tsmom` | OOS | +0,0035 R | 0,0056 R | **0,63×** |
| `qs-lb30` | IS | +0,0116 R | 0,0121 R | 0,96× |
| `ts-l20` | OOS | +0,0231 R | 0,0060 R | 3,85× |
| `ts-l10` | OOS | +0,0249 R | 0,0062 R | 4,02× |
| `ts-l5` | OOS | +0,0247 R | 0,0062 R | 3,98× |
| `qs-h15` | OOS | +0,0215 R | 0,0089 R | 2,42× |
| `qs-h30` | OOS | +0,0234 R | 0,0088 R | 2,66× |
| `qs-qp020` | OOS | +0,0219 R | 0,0093 R | 2,35× |
| `qs-h2` | OOS | +0,0299 R | 0,0088 R | 3,40× |
| `ovn1900/long` | OOS | **+0,0303 R** | 0,0124 R | **2,44×** |

⇒ **Chín trên mười một dòng thiếu NHIỀU HƠN toàn bộ số chi phí chúng trả.** Hai
dòng còn lại (`qs-lb3`, `tsmom`) thiếu **0,63–0,64 lần** hoá đơn, nên trên giấy
chúng cần **xoá 63–64% spread**; nhưng xoá hết 100% vẫn không đủ vì **chúng
thiếu ở cửa sổ mà cửa sổ kia không thiếu** — `qs-lb3` IS qua ở `s ≤ 0,0047` còn
OOS đòi `−0,1754`; `tsmom` OOS qua ở `−0,1733` còn IS không qua ở `−0,28`.

**Mặt phẳng này cũng nói chỗ chi phí CÓ quyết định — và đó không phải chỗ gần
cổng.** `ema-cross` đổi **dấu** của `E` trên trục spread (−0,1534 R ở 0,28 →
+0,1072 R ở −0,28) vì `R` của nó là **3,2 điểm giá** và `cost/R` **11,85%**, tức
**10–21 lần** các ứng viên. Nhưng `PF_r` của nó ở 0,28 là **0,7418** — nó không
ở gần cổng chút nào. ⇒ **Chi phí có đòn bẩy lớn nhất ở đúng những dòng xa cổng
nhất, và gần như không có đòn bẩy nào ở những dòng gần cổng nhất.** Quan hệ
nghịch đó là lý do cấu trúc khiến trục "chi vào chi phí" chết.

---

## 4. Khuyết điểm 16: đo được, và nó bắn ở ĐÚNG ô duy nhất cắt vạch

**Code-exact, không phải một phép khớp.** `apply_costs` (`engine.rs:902-912`)
cho `entry = open ± s/2`; `sizing_stop` (`fd-strategy/src/tsmom.rs:122-127`) lấy
`bar.close` của nến **tín hiệu**; `risk = |entry − stop|`
(`engine.rs:717-719`). Thế vào:

    long :  risk(s) = (open - close) + f*range + s/2
    short:  risk(s) = (close - open) + f*range + s/2
    => d risk / d s = +1/2 CHINH XAC, ca hai chieu

Cả 14 dòng dùng `bar.close` làm gốc stop (`quiet-swing` gọi chính
`tsmom::sizing_stop` ở `quiet_swing.rs:306-307`; `session-hold` ở
`control_hold.rs:162`; `ema-cross` dùng `atr_stop(..., ctx.bar.close, ...)`),
nên **khuyết điểm 16 phủ toàn bộ bộ dòng này**, không chừa ai.

**Độ lớn, đo bằng `R` của chính từng dòng:** `delta(0,28) = 0,1400 điểm giá`
(khớp phụ lục 7 mục C: **0,138 USD**, lệch 1,4%):

    qs-lb3        0,553% (IS) / 0,411% (OOS) cua mot R
    ovn1900/long  0,899% / 0,594%
    cac ung vien  0,274% - 0,899%
    ema-cross     4,362% / 4,327%   <- stop hep, va day la vi sao

`PF_r` **miễn nhiễm** với nó (`PF_r = Σ(+R)/|Σ(−R)|`, mọi `R` co giãn cùng
nhau), nên nó **chỉ** bóp chân `E` — và chân `E` là chân ràng buộc ở **10/11**
ứng viên. Tôi in `spread*` hai lần, và chênh lệch là:

**Ô duy nhất trong toàn bộ 224 ô không âm từng cắt cả hai vạch là `qs-lb3` IS ở
`s = 0`:**

    E(0)      = +0,0502 R   so voi vach +0,0500   => bien +0,0002 R  => QUA
    E_pin(0)  = +0,0499 R   (don vi R ghim o gia tri cua s=0,28)     => TRUOT

⇒ **Khuyết điểm 16 đáng 0,0003 R ở ô đó, còn biên của ô là 0,0002 R.** Cái cắt
vạch duy nhất của toàn bộ phép quét **nhỏ hơn chính khuyết điểm.** Nên câu trả
lời cho "`spread*` của bạn có bị nó bóp hay không": **không ở 223/224 ô, và ở ô
thứ 224 thì nó quyết định** — nhưng ô đó cũng không đổi kết luận, vì cửa sổ OOS
của cùng dòng trượt ở `s = 0` với biên **27 lần lớn hơn** (+0,0054 R).

**Phép khớp thực nghiệm thì KHÔNG phân giải được `delta`, và tôi nói ra vì sao.**
Tôi khớp `risk(s) = A + k·s` trên 13 điểm, với `risk(s)` suy từ
`(net_usd/total_r) × n × s / spread_paid` ở từng điểm. Ruler đó **calibrate**
được: nó đọc `px-1s` IS **27,05 điểm** so với `agent/stop-width` công bố
**25,69** (lệch 5,3%), và đọc `ovn1900/long` cost/R **1,80%** đúng đầu dải
`hour-screen` công bố. Nhưng:

    k tu do        : -0,25 den -5,14 (sai DAU), R2 0,004 - 0,97
    k co dinh 0,5  : SSR *xau hon* k = 0 o 28/28 dong-cua-so
    du doan cua delta tren toan luoi: risk doi 0,28 diem = ~1% cua mot R
    nhieu + lech cua chinh ruler    : 0,1 - 0,5 diem tren cung dai

⇒ **Ruler của tôi thô hơn đại lượng cần đo khoảng 2–5 lần**, và cái lệch có
hướng: `net_usd/total_r` tự đi lên khi spread giảm (sổ lãi hơn ⇒ `equity` kép
lớn hơn ⇒ `risk_usd` lớn hơn), đúng chiều ngược `delta`. **Đó là một giới hạn
của phép đo của tôi, KHÔNG phải bằng chứng chống khuyết điểm 16** — và nó là lý
do tôi dẫn số bằng đọc mã (slope = 1/2 chính xác) thay vì bằng `R²`. Số `R²` in
ra ở trên, đủ cả ba biến thể, trong receipt.

---

## 5. `sized_down_by_guard` — đọc THẲNG field, và tôi kiểm cả chỗ `0` đến từ đâu

Không grep chuỗi. `hypotheses.rs:752` in **chính field**
`sized_down_by_guard`; `hypotheses.rs:747` in `guards: nothing acted` **thay
cho** dòng ba phần khi cả `skipped`, `closed` **và** `sized_down_by_guard == 0`
— nên `ovn1900/long` không in `sized down` là một **0 đọc từ nhánh code đó**,
không phải một chuỗi thiếu. (`mould-target` đã bắt được chính mình in `0` từ một
chuỗi engine không in; tôi đọc mã trước khi tin con số.)

**Đo được, ở MỌI điểm spread:**

    11 ung vien, 2 cua so, 14 diem = 308 o:  sized_down = 0 o TAT CA
    px-1s, ovn1900/short:                    sized_down = 0 o moi diem
    ema-cross (doi chung):   IS  2868 -> 2930 (91,5% -> 93,5% so lenh)
                             OOS 2718 -> 2800 (87,6% -> 90,2%)

⇒ **Cơ chế brief mô tả là thật và tôi đo được nó — nhưng chỉ trên dòng
`Exits::Engine` stop hẹp.** Spread hẹp hơn ⇒ `risk` nhỏ hơn ⇒
`lots = risk_usd/(risk × contract_size)` lớn hơn ⇒ trần `notional 300%` cắn
nhiều hơn: `ema-cross` tăng **62 entries bị thu nhỏ** khi spread đi từ 0,28 về
−0,28. Trên 11 ứng viên (stop 1,0–1,5 biên độ ngày NY, `R` 15–51 điểm giá) `lots`
không bao giờ chạm trần, nên **cả hai chân cổng mù với nó, nhưng ở đây không có
gì để mù cả**. Mọi con số quyết định của tôi vẫn là `PF_r`/`E`.

Guard **khác** thì có cắn, và đáng nói vì nó là lý do các dòng này tồn tại trong
arm chủ cho phép (`s = 0,28`, IS):

    qs-lb3   refused none; closed NEWS_FLAT 137, OPEN_LOSS_CAP 1, WEEKEND_FLAT 151
    ts-l20   refused none; closed NEWS_FLAT 220, OPEN_LOSS_CAP 2, WEEKEND_FLAT 243
    ovn1900/long  "nothing acted"   <- khong guard nao can thiep, 1644 lenh

---

## 6. `0,28` là **ĐO** — nhưng một mẫu, và mẫu lớn hơn nói nó **quá cao**, trừ một giờ

Brief đòi kiểm. `config/default.toml:306-333` **tự khai**: 0,28 được **ĐỌC** từ
terminal VantageMarkets-Live 21 ngày **12/09/2026** (`XAUUSD.sc` 28 điểm, hợp
đồng 1 oz) — **không phải giả định**, khác `markets.btc` (`spread = 5.0`, config
ghi là giả định) và giống `markets.btcusd`
(`spread = 17.05 # MEASURED: 3.4x the 5.00 assumed`). Nhưng cùng khối chú thích
ghi tiếp rằng đó là **một lần đọc duy nhất**, và `py/live/spread_log.py` đã lấy
2.315 mẫu / 12,8 giờ cho p50 **0,220**.

**Tôi đọc chính cái log đó trên đĩa (chỉ đọc):**
`E:/rust/flowdesk/data/spreads/XAUUSD_sc.csv` — **8.967 mẫu**, từ
`2026-09-15T02:01:39Z` đến `2026-09-17T03:50:08Z`, **49,8 giờ đồng hồ** (gấp
3,9 lần cái config ghi):

    min 0,210  p10 0,210  p25 0,210  p50 0,220  p75 0,220
    p90 0,220  p95 0,260  p99 0,290  max 0,290  trung binh 0,2204
    chi 7 gia tri roi rac: 0,21 (4068) 0,22 (4022) 0,26 (376) 0,29 (251)
                           0,27 (106) 0,25 (80)   0,28 (64)

**Và một chỗ config SAI, đo được (brief §8: số đo thắng):** config viết
*"the configured 0.28 is above every spread ever observed"*. **Không còn đúng.**
**2,80% mẫu (251 mẫu ở 0,290) VƯỢT 0,28**, và chúng **tập trung vào đúng giờ
rollover** mà config nói mẫu 12,8 giờ chưa phủ:

    gio UTC   p50    p90    max
    20:00    0,260  0,290  0,290
    21:00    0,290  0,290  0,290   <- gio rollover, p50 NAM TREN 0,28
    22:00    0,260  0,280  0,290
    23:00    0,210  0,220  0,220
    00-19    0,210-0,220 (p90 0,220 - 0,250)

⇒ **Ba mốc để chủ tự so với `spread*`**, và không mốc nào là một lời khuyên:

    0,280  gia tri config dang tinh (DO, 1 mau, 12/09/2026)
    0,220  p50 cua 8.967 mau (49,8 gio, 15-17/09/2026)
    0,290  p99 = max da quan sat, va la p50 cua gio rollover 21:00 UTC

Vì `spread*` của mọi dòng là **null trên toàn dải không âm**, **không mốc nào
trong ba mốc này đổi kết luận** — khoảng cách tới cổng lớn hơn toàn bộ dải giữa
0,210 và 0,290 **hàng chục lần**. Nhưng hai mốc kia đáng ghi vì chúng là
**amendment chưa được viết** mà config tự nói là còn thiếu, và một trong hai điều
kiện config đặt ra cho amendment đó (giờ rollover 21:00 UTC) **giờ đã có dữ
liệu**; điều kiện còn lại (một ngày cuối tuần) thì **vẫn chưa**.

Một ghi chú đặt đúng chỗ: `ovn1900/long` vào lệnh **19:00 NY = 23:00 UTC** và
giữ tới 02:00 NY — tức nó sống ở **giờ RẺ NHẤT** của phân phối (p50 0,210), chứ
không ở giờ rollover đắt. Nên với dòng đó, 0,28 phẳng là **bi quan**, và nó vẫn
thiếu **+0,0303 R = 2,44 lần cả hoá đơn spread** ở `s = 0`.

---

## 7. `spread*` là một con số **KỸ THUẬT**. Nói thẳng, như brief đòi

- Nó **không phải** một mức giá đi mua được. Tôi **không biết** sàn nào cho
  spread nào, và **cơ cấu hoàn phí / commission của desk là thứ chủ biết, không
  phải thứ tôi đo**. `commission_per_lot = 0.0` trong config, nên `spread*` ở
  đây **chỉ là spread**, không có một đồng commission hay hoàn phí nào trong đó.
- Tôi **không** viết "đạt được" hay "không đạt được" ở bất kỳ đâu. Tôi in
  `spread*` cạnh ba mốc của mục 6 và để chủ so.
- **Phía âm của lưới là ĐỐI CHỨNG, không phải một khoản hoàn phí.** Một
  `--spread=` âm làm `apply_costs` chạy ngược dấu: nó cải thiện **giá khớp** và
  đồng thời **co `risk`** qua `delta = s/2`. Một khoản hoàn phí tiền mặt thì
  **cộng USD** mà không chạm giá khớp lẫn đơn vị R. Hai thứ **khác nhau về bản
  chất**; con số âm ở đây là một **yêu cầu về ĐỘ LỚN của việc giảm chi phí**, đo
  thay vì ngoại suy (brief mục 1: quét thật, không phải một phép chia), và
  **không** phải một mức hoàn phí có thể đi hỏi sàn.

---

## 8. Sổ đa phép thử: khai vs xem

| | khai (đăng ký `35e3283`) | xem |
|---|---|---|
| dòng | 14 (11 ứng viên + 3 đối chứng) | **14** |
| cửa sổ | 2 | **2** |
| arm guards | 1 (chỉ ON) | **1** |
| điểm spread >= 0 | 8 | **8** |
| điểm spread đối chứng < 0 | 6 | **6** |
| ô chính | 224 | **224** |
| ô đối chứng | 168 | **168** |
| tổng ô / lần chạy | 392 / 28 | **392 / 28, 0 ô không đọc được** |
| **ô cổng** | 0 bây giờ, chỉ dòng qua mới tiêu | **0** |

Không thêm dòng, không đổi cửa sổ, không thêm arm, không chỉnh tham số nào,
không dịch ngưỡng nào, không hạ sàn cỡ mẫu nào. **Không quét cỡ stop** (bẫy 2
phụ lục 10) — cỡ stop cố định ở giá trị riêng của từng dòng ở **cả 392 ô**.

**Hai thứ được đưa vào SAU khi thấy số, và được dán nhãn tại chỗ:**
1. `E_pin` (ghim đơn vị R ở giá trị `s = 0,28`) — một phép đổi đơn vị của một cột
   đã khai ở F5 của đăng ký, không phải một dòng mới.
2. Phép đo phân phối spread sống ở mục 6 — nó **không** là một ô backtest, nó là
   một lần đọc một file CSV có sẵn, và nó đi vào báo cáo vì brief yêu cầu kiểm
   "0,28 là ĐO hay GIẢ ĐỊNH".

**Hai ứng viên là CÙNG MỘT SỔ ở IS:** `qs-h15` và `qs-h30` in **y hệt** trên IS
(393 lệnh, `PF_r` 1,2713, `E` +0,0659, `Lbar` 0,2428, exit mix giống từng con
số) vì `NEWS_FLAT`/`WEEKEND_FLAT` đóng lệnh trước khi hold 15 hay 30 phiên hết
hạn; OOS chúng mới tách (1,0741 vs 1,0672). ⇒ **ứng viên phân biệt được là 10,
không phải 11**, và tôi khai điều đó ở đây chứ không giấu nó trong bảng.

---

## 9. Thứ KHÔNG đo được, và vì sao

1. **Spread thật theo giờ, trong engine.** Mọi điểm trên lưới là spread **phẳng
   ở mọi giờ**. Mục 6 đo được rằng spread thật đi từ 0,210 (giờ Á) tới 0,290
   (giờ rollover 21:00 UTC) — **một dải 38%** mà engine không biểu đạt được, vì
   `--spread=` là một số. Với `ovn1900/long` thì phẳng-0,28 là **bi quan**; với
   một dòng sống ở mốc nghỉ thì nó là **lạc quan**. Không đo được ở đây.
2. **Commission và hoàn phí.** `commission_per_lot = 0.0`. Không có trong engine,
   không có trong `spread*`.
3. **Excursion trong lệnh.** Mọi drawdown là đường vốn **đã đóng lệnh**.
   `avg_mae` ở `s = 0,28` (IS / OOS): `qs-lb3` **−0,397 / −0,385 R**,
   `ovn1900/long` **−0,194 / −0,212**, `qs-lb30` **−0,534 / −0,473**,
   `qs-h15` **−0,474 / −0,484**.
4. **`wrong_side_stop`** — `grep -rn wrong_side_stop crates/` trả **0** trên cây
   này (phụ lục 8 mục III.2). Không có bộ đếm ⇒ **không đo được**, và không một
   số 0 nào được đọc từ nó.
5. **Phân vị.** Không công bố, không đọc. `--seeds=2` truyền chỉ vì mode đòi một
   con số. `count match` của `qs-lb3` là **2,96** (ngoài băng) và `null p50` của
   nó **0,929 < 1** ⇒ theo brief §4 **đừng công bố phân vị**, và tôi không.
6. **`level-reversion`** — một cửa sổ, cần timeline options (`timeline: none`
   trong mọi header). Không chạy. Nó trượt tiêu chí chọn vì **cửa sổ**, không vì
   kết quả.
7. **`data-sealed/`** — không mở, không trỏ tới, không đếm.
8. **Instrument khác, cỡ nến khác.** Mọi số là `xauduka` 15m. Và phụ lục 10 mục
   IV là lý do để nghi chính feed đó: `open == low` ở **19,02%** nến mở lại sau
   mốc nghỉ. **Không một dòng nào của tôi có filter `flat 16:30-18:15`**
   (`filters = []` trên 12 dòng; 2 dòng `ovn1900` lọc `hours:1900-1905` nên
   **không** chạm mốc nghỉ) ⇒ 9 dòng `quiet-swing`/`tsmom` và `px-1s` **không
   miễn nhiễm**, tôi nói ra chứ không giả định.
9. **`max_hold_ms`** — không đổi, không có cờ CLI. Nó là lý do mọi dòng
   `Exits::Engine` không thể vào bộ ứng viên (`mould-target` F1b).
10. **Vì sao OOS tệ hơn IS ở 9/11 dòng.** Đo được là nó tệ hơn; **không** đo
    được nguyên nhân. Đó là hiện vật cửa sổ, và nó là chỗ đáng đi tiếp, không
    phải chi phí.

---

## 10. Nếu chủ chỉ muốn một đoạn

**Chi phí không phải thứ chặn các dòng gần cổng nhất của hồ sơ, và giờ có con số
nói điều đó theo cả hai chiều.** Trên 11 dòng gần cổng nhất — chọn bằng một bộ
lọc số khai trước (`min PF_r ∈ [1,05; 1,20)`, `n ≥ 40`, hai cửa sổ, arm guards
ON) — **`spread*` là `null` ở mọi dòng**: kể cả **giao dịch miễn phí** cũng
không dòng nào qua cổng trên cả hai cửa sổ. Dòng gần nhất, `qs-lb3`, cần
**execution miễn phí cộng một khoản bù 0,1754 USD mỗi vòng mỗi ounce** — tức
**163% của spread hiện hành** phải biến mất — và 10 dòng kia cần **hơn 200%**.
Chín trên mười một dòng **thiếu nhiều R hơn toàn bộ số chi phí chúng trả**
(cost/R chỉ **0,56%–1,64%** của R ở các dòng này, đo bằng quét 14 điểm trên tập
lệnh bất biến). Và năm dòng **đã qua cả hai chân ở đúng spread 0,28 trên MỘT cửa
sổ** rồi trượt ở cửa sổ kia dù spread xuống −0,28: ở đó **không có gì để mua**,
vì thứ chặn là **cửa sổ**, không phải giá. Chỗ chi phí thật sự quyết định dấu là
họ **stop hẹp** (`ema-cross`: `R` 3,2 điểm giá, cost/R **11,85%**, `E` lật
−0,153 → +0,107 R trên cùng trục spread) — nhưng dòng đó có `PF_r` **0,7418**,
xa cổng nhất bảng. **Chi phí có đòn bẩy lớn nhất ở đúng những dòng xa cổng nhất
và gần như không có đòn bẩy nào ở những dòng gần cổng nhất.** ⇒ **Desk thôi hỏi
hướng "chi vào chi phí" trên vàng 15m.** Tiền nên đi vào chỗ khác — và phép đo
duy nhất ở đây chỉ vào một chỗ: **tại sao năm dòng qua cổng ở một nửa hồ sơ và
chết ở nửa kia.**

Hai ghi chú phụ, nhỏ hơn nhưng dùng được: (a) `0,28` trong config là **ĐO** từ
một lần đọc terminal, còn **8.967 mẫu / 49,8 giờ** nói p50 là **0,220** — và
câu config tự viết *"0,28 trên mọi spread từng quan sát"* **không còn đúng**:
**2,80%** mẫu ở **0,290**, tập trung ở **giờ rollover 21:00 UTC** (p50 giờ đó
**0,290**), đúng điều kiện mà config nói amendment còn thiếu. (b) `F7` của
`mould-target` (50,7% ô guards-ON đổi tập lệnh khi quét spread) là thuộc tính
của họ **`Exits::Engine` cưỡi trần notional**, không phải của arm guards: trên
**308 ô** ứng viên của tôi, `n` **không đổi một lệnh nào** qua một trục spread
dài 0,56 USD.
