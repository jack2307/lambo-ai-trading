# Thoát một phần: phần đuôi còn lại đáng GIÁ HƠN cái mức bán nó — nên không có mức nào, không có tỉ lệ nào, mua lại được gì

**Ngày:** 2026-10-09. Nhánh `agent/partial-exit`, worktree `/e/rust/fd-partial-exit`.
Đăng ký trước: `docs/decisions/2026-10-09-partial-exit.md` (commit `e61c6cd`,
**chỉ docs**, trước dòng code đầu tiên). Bản vá engine: `c3d746e`.
Receipt: `receipts/partial/` (28 file thô + `partial-exit-grid.md`).

**Câu hỏi:** giữ nguyên tới target đọc **+0,054R**; chốt hết sớm bằng trailing
đọc **−0,019R** (`agent/m4`). Khoảng giữa — **giữ một nửa phần đuôi, bảo hiểm
một nửa phần quay lại** — chưa ai đo. Nó mua lại được gì không?

**Trả lời:** Không, và lý do là **MỘT con số đo được, không phải một kết luận**:
ở cả ba mức chốt, trên cả hai cửa sổ và cả hai arm, **kết cuc trung bình của
một vị thế ĐÃ chạm mức luôn LỚN HƠN chính cái mức đó**. Vị thế `donchian`
chạm +0,33R đi tiếp tới **+0,515R**; chạm +0,67R tới **+0,743R**; chạm +1,00R
tới **+1,106R**. **12/12 ô** (3 mức × 2 cửa sổ × 2 arm) cùng dấu. Bán một phần
ở mức là **bán rẻ**, và `E` phải giảm — nó giảm, **tuyến tính theo liều**.

---

## 1. Việc, một câu

Thêm luật **thoát một phần** vào lớp RULES của engine (nó chưa từng có), rồi
quét lưới thô **3 mức chốt × 2 tỉ lệ = 6 ô** trên 3 cơ chế đã đăng ký × 2 cửa
sổ × 2 arm guards = **84 dòng**, với `PF_r` cạnh `PF_usd`, `Lbar`, drawdown USD
và số lần chốt in ở từng dòng.

## 2. Cổng

**0 / 84 dòng.** Không ô lưới nào (0/72) và không đối chứng nào (0/12) qua cổng
`PF_r >= 1,200` **và** `E >= +0,050R` **và** `>= 40 lệnh`, trên **một** cửa sổ
chứ đừng nói cả hai. Đếm tay.

Năm dòng qua **hai chân số** mà trượt sàn cỡ mẫu đều là `compression/bb-fade`
cửa sổ B arm guards với **n = 33 < 40** — và **dòng `off` của chính nó cũng ở
trong đó** (PF_r 1,2463 / E +0,0904 / n 33), nên bốn ô partial trong nhóm ấy
không phải thành tích của partial. Cùng bốn ô đó ở cửa sổ A: PF_r **0,63–0,65**,
E **−0,17 … −0,19**. Đó là lý do sàn 40 lệnh tồn tại.

`donchian-breakout` — dòng duy nhất có `n >= 40` **và** expectancy dương — cao
nhất đạt PF_r **1,1259** ở đối chứng, và **mọi** ô partial kéo nó xuống.

## 3. Falsifier — cái nào bắn, ở đâu

**F0 (tiền kiểm) BẮN MỘT NỬA, đúng như khai.** Đọc mã trước khi tiêu ô nào:
engine **không thoát một phần được** — `close_position` nhận `Live` theo giá
trị và luôn đóng hết `lots`, và `r = points/risk` **không đọc `lots`** nên hình
thức "hai Trade" sẽ khai R đầy đủ cho mỗi chân (một vị thế chốt nửa ở +0,33R
rồi dừng in **+0,33 và −1,00** thay vì **−0,335R** thật) và **phình `n`**. Hình
thức đó bị loại **bằng đọc mã, trước khi chạy**. Điều kiện thứ hai của F0 —
"tắt thì BIT-IDENTICAL hoặc tôi dừng" — **giữ được**: 27 test binary của
`fd-backtest` qua hết, golden parity trong đó, và đối chứng tái hiện
`agent/m4` đúng từng chữ số (mục 5).

**F1 (chính) BẮN, và bắn ở mức mạnh nhất có thể.** Không ô nào trong 6 ô tốt
hơn đối chứng trên **cả hai** cửa sổ, ở bất kỳ cơ chế nào có `n >= 40`:

- `donchian-breakout`: **0/24 ô** tốt hơn đối chứng ở **một** cửa sổ nào, chứ
  chưa nói cả hai. Cửa sổ A không-guards: +0,0536R → +0,0285R (0,33/0,25) →
  **−0,0042R** (0,33/0,50). Cửa sổ B: +0,0168R → +0,0083R → **−0,0035R**.
- `intraday/bb-fade` (n 349–385): cả 6 ô **tốt hơn** ở cửa sổ A và cả 6 ô
  **xấu hơn** ở cửa sổ B. Không ô nào tốt ở cả hai.

**F3 (cửa sổ) BẮN, trên hai trong ba cơ chế.** Dấu của tác động lật giữa hai
quý liền nhau: `bb-fade` (cả hai nhãn) có "thoát một phần GIÚP" ở cửa sổ A và
"LÀM XẤU" ở cửa sổ B, trên **cùng** một cơ chế và **cùng** một ô. Đó là **hiện
vật cửa sổ thứ 12**, và nó nằm đúng ở đại lượng quyết định (mục 5).

**F2 (luật không nổ) KHÔNG bắn — và đây là phần đáng tin nhất của phép đo.**
Luật nổ **10 đến 258 lần** tuỳ ô, không ô nào bằng 0. Hơn thế: ở cửa sổ A
không-guards, cả 7 dòng (`off` + 6 ô) in **y nguyên** một dòng exit-mix
(`STOP 65, TARGET 49, TIMEOUT 65, flat window 23, …`) và **y nguyên** `n = 271`
và `mean hold 128,4 min`, trong khi **178/271 vị thế (65,7%) đã chốt một phần**.

⇒ **Số lệnh, cách thoát, thời gian giữ KHÔNG đổi; thứ duy nhất đổi là CỠ của
phần mang vào phần đuôi.** Không có phép cô lập nào sạch hơn thế trong hồ sơ
này: đây là trục thoát đầu tiên được đo mà **không** đồng thời đổi tập lệnh
(trail đổi 49 lần về đích thành 17).

## 4. Sổ đa phép thử — khai vs xem

| phần | khai | xem |
|---|---|---|
| run | 28 | **28** |
| row | 84 | **84** |
| ô lưới (mức × tỉ lệ) | 6 | **6** |

Không nâng lưới sau khi thấy kết quả. Toàn bộ 84 dòng công bố trong
`receipts/partial/partial-exit-grid.md`, không chỉ ô thắng — và vì không có ô
thắng, cái được công bố là cả cột đi xuống.

**Chi phí bản vá, khai vs thật** (đăng ký khai ~330–380 dòng):

    code (engine + config + search.rs + 5 cho khai Trade):  ~366 dong  <- trong khoang khai
    test (crates/fd-backtest/tests/partial_exit.rs):          350 dong  <- khai ~120, VUOT 230
    batch file toml:                                           32 dong
    tong insertions commit c3d746e:                            748 dong

Phần vượt nằm hết ở test, và tôi để nó vượt: 11 test, trong đó
`the_matched_null_takes_the_same_partial` là cái ghim quyết định thiết kế, và
`a_bar_that_reached_the_level_and_the_stop_books_the_whole_loss` là cái ghim
đọc bi quan. Một con số nữa đáng ghi: **1 trong 11 test của tôi bắt lỗi của
chính tôi** — tôi viết kỳ vọng `banked_r = 0,05` (số **lot**) cho một chỗ phải
là `0,25` (số **R**), và test bắt được trước khi một ô nào bị tiêu.

**Quyết định thiết kế, và nó giữ:** luật nằm trong `TradingRules::partial`
cạnh `trail`, **không** trong strategy. `control::RandomEntry` không khai
`fn exits()` nên nhận `Exits::Engine` và engine đặt target `reward_risk × risk`
cho nó ⇒ **null đi qua đúng luật đó, không một dòng nào viết riêng cho null**,
và test ghim điều đó. Không có đường riêng cho null ở bất kỳ đâu trong
`engine.rs`.

## 5. Con số đáng giữ nhất

### (a) GIÁ TRỊ TIẾP TỤC — một con số, và nó đóng cả trục

Mô hình một dòng: bán phần `f` của một vị thế tại `+xR` thay cho để nó chạy tới
kết cuộc của nó đổi `E` đi **−(f/n) × Σ(r_cuối − x)** trên tập vị thế có chạm
mức. Nên **hai liều đọc ra một con số**: kết cuộc trung bình của một vị thế
**đã** chạm `+xR`. Hai liều là hai phép đo độc lập của cùng số đó, và chúng
**khớp nhau tới ≤ 0,027R ở mọi ô, phần lớn < 0,01R** — tức đáp ứng **tuyến
tính** theo `f`, đúng như mô hình đòi. Đó là kiểm đúng trước khi tin con số.

`intraday/donchian-breakout`, arm không-guards:

| mức | A: % chạm | A: tiếp tục | B: % chạm | B: tiếp tục |
|---|---|---|---|---|
| +0,33R | 65,7% | **+0,515R** | 71,1% | **+0,394R** |
| +0,67R | 52,4% | **+0,743R** | 51,8% | **+0,747R** |
| +1,00R | 38,4% | **+1,106R** | 39,3% | **+1,026R** |

**Tiếp tục > mức ở 12/12 ô** (3 mức × 2 cửa sổ × 2 arm). ⇒ **Không có mức nào
mà bán một phần là miễn phí.** Và chú ý nó không phải "phần đuôi to": phần
vượt chỉ **+0,03 đến +0,19R**. Trục này không chết vì phần đuôi béo; nó chết vì
phần đuôi **hơi** béo và cổng tính theo lệnh không có chỗ cho một thứ hơi âm.

Hai dòng `bb-fade` **lật dấu giữa hai cửa sổ** ở đúng đại lượng này: A có
`tiếp tục < mức` (nên chốt một phần giúp), B có `tiếp tục > mức`. Cùng cơ chế,
hai quý liền nhau. ⇒ **giá trị tiếp tục là một đại lượng của CỬA SỔ ở cơ chế
lỗ, và của CƠ CHẾ ở cơ chế lãi** — chỉ `donchian` giữ một dấu trên cả hai.

### (b) Dấu phụ thuộc sổ đang lãi hay lỗ, và nó tái hiện kết luận 16/09

    E tot hon doi chung:  28 / 72 o      E xau hon:  43 / 72 o   (1 y nguyen)

28 ô tốt hơn **không phải tin tốt**: chúng nằm gần hết ở sổ **đang lỗ**
(`intraday/bb-fade` PF_r 0,78–0,89 và `compression/bb-fade` cửa sổ A PF_r
0,63–0,69), nơi chốt một phần làm **lỗ nhỏ đi** vì phần đuôi của chúng là âm.
Đó đúng là câu của quyết định 2026-09-16 về trailing stop — *"nó giúp bên lỗ lỗ
ít hơn, và làm bên lãi duy nhất xấu đi"* — **đo lại được trên một trục khác,
bằng một cơ chế khác**. Hai trục, cùng một hình dạng:

    can thiep vao phan duoi = CHUYEN TU PHAN DUOI.
    so nao co phan duoi DUONG thi mat; so nao co phan duoi AM thi duoc.
    Va mot so khong co phan duoi duong thi khong phai ung vien.

### (c) Drawdown THẬT nhỏ đi — và nó vẫn không đủ

    sut USD nho hon doi chung: 68 / 72 o   (lon hon: 4, deu la bb-fade B guards)

Dự đoán thứ hai của đăng ký **đúng**. `donchian` cửa sổ A không-guards:
sụt **15,49 → 11,53 USD** ở ô 0,67/0,50 (**−25,6%**). Nhưng tỉ giá trao đổi
xấu ở mọi ô có `n >= 40` và `E > 0`:

| ô | ΔE | Δsụt USD |
|---|---|---|
| 0,33 / 0,25 | **−46,8%** | −9,4% |
| 0,33 / 0,50 | **−107,8%** (lật dấu) | −15,6% |
| 0,67 / 0,25 | **−15,1%** | −11,0% |
| 0,67 / 0,50 | **−33,0%** | −25,6% |
| 1,00 / 0,25 | **−16,2%** | −9,2% |
| 1,00 / 0,50 | **−34,3%** | −20,1% |

**Không ô nào mua được một đơn vị drawdown rẻ hơn một đơn vị expectancy.** Ô
gần nhất (0,67/0,50) trả 33% expectancy cho 26% drawdown. Và cổng không có chữ
nào về drawdown, nên nếu chủ máy muốn đổi, **đây là bảng tỉ giá** — nhưng nó là
một cuộc đổi lỗ ở mọi ô, không phải một lựa chọn.

### (d) Dự đoán ÂM của tôi SAI về cơ chế, và cái đúng rẻ hơn

Tôi khai trước: `Lbar` sẽ tụt về **khoảng một phần ba**, xuống dưới 0,250R, và
**chân expectancy sẽ thành chân ràng buộc** — đó là cách trục này chết.

Đo được: `Lbar` tụt **−24,9%** (0,4254 → 0,3194 ở ô nặng nhất), **không phải
−67%**. Và **0/84 dòng có `Lbar < 0,250R`** (thấp nhất 0,2803) ⇒ **chân
expectancy vẫn DƯ ở mọi dòng.** Cơ chế tôi dự đoán **không nổ**.

Trục chết đơn giản hơn: **`PF_r` cũng tụt.** 1,1259 → 0,9867. Cả hai chân cùng
xấu đi, nên không có đánh đổi nào để cân — và vì thế không cần tới hằng đẳng
thức để kết luận. **Con số của tôi thua số đo (brief §8); tôi báo cả hai.**

Lý do `Lbar` không tụt như tôi tính: tôi lấy `−1,00R` làm cỡ lỗ. Cỡ lỗ thật là
`|avg_loss_r| = 0,7343` (nhiều lỗ là TIMEOUT và exit tín hiệu, nhỏ hơn một R),
và chốt nửa ở +0,33R chỉ bù lại `0,5 × 0,33 = 0,165R`. Prior "81% chạm +0,33R"
của sổ `xau-stoch` cũng cao: dòng này đo **65,7%** (A) và **71,1%** (B).

### (e) Chiều của khe `PF_usd` vs `PF_r` KHÔNG phải hằng số của engine

    tren 84 dong cua job nay:  PF_usd doc THAP hon PF_r o 62/84 (73,8%)
    phu luc 8 muc III, 452 o:  PF_usd doc CAO  hon PF_r o 320/452 (71%)

Hai phép đo **ngược dấu nhau**, nên chiều ấy phải được **đo ở từng dòng**, đừng
dự đoán. Và **5/84 dòng có hai chân PF ở HAI phía vạch 1,200** (tất cả là
`compression/bb-fade` B guards, n 33) — ở đó **đọc bằng USD sẽ LOẠI một dòng mà
đọc bằng R nhận**, ngược chiều với ví dụ của phụ lục 7A.

Và hằng đẳng thức vẫn đúng khi cỡ lệnh đổi giữa đường: `E = Lbar × (PF_r − 1)`
có phần dư lớn nhất **0,00007R** trên cả 84 dòng, kể cả 72 dòng có vị thế thay
đổi cỡ. Đó là kiểm đúng kế toán của bản vá, do engine tự in.

## 6. Thứ KHÔNG đo được, và vì sao

- **Họ tự quản (`Exits::Strategy`) — không đo, cố ý.** Luật không chạm vị thế
  `self_managed`, đúng luật `trail_stop`: ở đó "stop" là mẫu số của R chứ không
  phải một lệnh (phụ lục 7B), nên không có vị thế do engine quản để lấy một
  phần. ⇒ Mọi dòng họ `close/*` / `session_hold` là **KHÔNG ĐO ĐƯỢC**, không
  phải bị bác bỏ.
- **Liều thật ≠ liều khai.** `lots` phải đi theo `lot_step = 0,01` và cả hai
  phần phải `>= min_lot = 0,01` trên một sổ 100 USD (TK cent, khuyết điểm 17
  của phụ lục 7). Liều khai 0,25 cho liều thật **0,170–0,237**; liều khai 0,50
  cho **0,433–0,480**. Và ở cửa sổ B **số lần chốt khác nhau giữa hai liều**
  (195 so với 199) vì vài vị thế nhỏ tới mức không chốt được một phần tư mà
  chốt được một nửa. Mọi số đáp ứng ở mục 5 **chia cho liều THẬT**, nên nó là
  một sai liều, không phải một sai dấu — nhưng **một lưới liều mịn hơn không
  đo được trên sổ này**, và đó là một giới hạn của dụng cụ, không của trục.
- **Phần đuôi ở mức TỪNG LỆNH.** `avg_mfe` là trường duy nhất thấy excursion và
  nó đo tới giá THOÁT, nên trên một vị thế đã chốt một phần nó vẫn là excursion
  của **cả** đường đi. Giá trị tiếp tục ở mục 5(a) là một **số trung bình suy
  ra từ hai liều**, không phải một phân bố đọc từng lệnh. Một phân bố cần một
  trường mới trên `Trade`, và tôi không thêm nó.
- **Phân vị — không công bố.** Mọi dòng in `count match 0,62–0,73
  ** outside the band **` và `cost match 0,60–0,72 ** outside the band **`,
  đúng như đối chứng `agent/m4` đã in. Brief §4: ngoài băng thì không công bố
  phân vị. (`null p50` của mọi dòng nằm trong **0,824–0,987** — dưới 1, tức
  null trung vị **lỗ tiền**.)
- **Sổ phẳng live (`paper.rs`) KHÔNG được nối luật này**, cố ý: nó là vòng lặp
  của sổ tiền thật và một luật mới ở đó là rủi ro không cần cho một phép đo.
  ⇒ Nếu ai bật `[trading.partial]` trong config thì backtest chốt một phần mà
  sổ phẳng **không**. Ghi ở đây vì nó là một cái bẫy, không phải một tính năng.

## 7. Kết luận cho desk

**Trục thoát một phần CHẾT**, và nó chết cùng một kiểu với trail nhưng bằng một
phép đo sạch hơn (tập lệnh không đổi). Cộng hai kết quả:

> **Mọi can thiệp vào phần đuôi trên hồ sơ này đều làm sổ lãi xấu đi và làm sổ
> lỗ lỗ ít hơn, vì cả hai trục chỉ CHUYỂN TỪ phần đuôi.** Họ "quản lý vị thế" —
> trailing stop, chốt một phần — **đóng được** trên vàng 15m ở hai cửa sổ này,
> không phải vì nó không hoạt động mà vì **đại lượng nó khai thác (giá trị tiếp
> tục nhỏ hơn mức) có dấu NGƯỢC trên mọi dòng đáng giao dịch.**

Cái **không** đóng, và là việc đáng đi tiếp: giá trị tiếp tục là một đại lượng
đo được rẻ (hai run cho một con số, 1,6 giây). Nếu một ngày có một cơ chế mà
`tiếp tục < mức` trên **cả hai** cửa sổ, thì thoát một phần có chỗ ở đó — và
luật đã có trong engine, tắt mặc định, chờ sẵn.

---

## Ghi chú về việc tái hiện

Đối chứng tái hiện `agent/m4` **đúng từng chữ số**, trái với tỉ lệ 65,3% của
phụ lục 8 mục II:

    agent/m4 (06/10):  271 lenh  PF_usd 1.117  expect +0.054  TARGET 49  hold 128.4 min
    chay lai (09/10):  271 lenh  PF_usd 1.1175 expect +0.0536 TARGET 49  hold 128.4 min

Nên bản vá engine **không** dịch một con số nào của hồ sơ khi luật tắt, và đó
là điều kiện F0 đã khai trước.
