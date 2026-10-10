# Giờ trong ngày CÓ một đồng hồ đo được và nó KHỔNG LỒ — nhưng nó là đồng hồ của BIÊN ĐỘ, không phải của chiều; và `ny-morning` của batch đúng là cực đại của chính cái đồng hồ đó

**Ngày:** 2026-10-10. Nhánh `agent/hour-screen` (cắt từ `agent/stop-width`).
Đăng ký trước: `docs/decisions/2026-10-10-hour-screen.md` (commit `64f0624`, chỉ
docs) + ghi chú khai 32 ô cổng (`6aefbaf`, cũng chỉ docs, trước khi chạy engine).
Khai báo ô: `docs/hypotheses/2026-10-10-hour-screen.toml`.
Receipt: `receipts/hourscreen/`.

---

## 1. Việc, một câu

Sàng **từng giờ New York** trên `xauduka` 16 năm (378.749 nến, 2010-06-01 →
2026-05-31), hai cửa sổ chia theo thời gian, hai đại lượng (dịch chuyển có dấu
và biên độ, chuẩn hoá theo biên độ ngày), **hai thước đồng hồ** (NY có DST và
UTC cố định), **288 phép so khai trước**; rồi hỏi ba cửa sổ hand-picked của
batch `gold-intraday` nằm **ở đâu** trong bảng xếp hạng 24 giờ; rồi tiêu **32 ô
cổng** đã khai cho đúng những giờ tiền kiểm cấp phép.

## 2. Cổng

**0 / 32 ô qua cổng** — đọc bằng `PF_r` hay `PF_usd` đều **0 / 16 dòng** (mỗi
dòng = 1 nhãn × 1 arm, phải qua trên **cả hai** cửa sổ). Đếm tay, không dòng nào
dưới 40 lệnh (nhỏ nhất **1.551 lệnh**), nên chân cỡ mẫu không phải là thứ loại
chúng.

Bốn dòng tốt nhất, `PF_r` cạnh `PF_usd`, drawdown USD cạnh mọi số lợi nhuận:

| dòng | cửa sổ | n | `PF_usd` | `PF_r` | `E = total_r/n` | `Lbar` | sụt USD | sụt % |
|---|---|---|---|---|---|---|---|---|
| `ovn1900/long` (19:00→02:00) | W1 | 1644 | 1,081 | **1,0899** | +0,0078 R | 0,0868 | 936,71 | 7,66% |
| `ovn1900/long` | W2 | 1635 | 1,071 | **1,0703** | +0,0070 R | 0,0982 | 1.004,49 | 8,55% |
| `ovn1800/long` (18:00→02:00) | W1 | 1623 | 1,093 | **1,1006** | +0,0090 R | 0,0898 | 1.076,25 | 8,80% |
| `ovn1800/long` | W2 | 1635 | 1,149 | **1,1416** | +0,0140 R | 0,0985 | 806,10 | 6,22% |

Cổng đòi `PF_r >= 1,200`. Cao nhất đo được là **1,1416**, và nó ở dòng **đầu dò
hiện vật** (mục 4).

**Chân nào ràng buộc, và vì sao điều đó là kết luận:** output in `expectancy leg
BINDS (Lbar < 0,250R)` ở **cả 32 ô**. Nhưng `session-hold` khai
`Exits::Strategy` nên cái stop `riskDailyRanges` **không bao giờ được thi hành**
(kết quả `agent/stop-width`) — nó là **mẫu số của R**. Dưới một phép đổi mẫu số
`f`, `E` và `Lbar` tỉ lệ `1/f` còn **`PF_r` BẤT BIẾN** (`PF_r = Σ(+R)/|Σ(−R)|`,
mọi R co giãn cùng nhau). Theo hằng đẳng thức `E = Lbar(PF_r − 1)` — đúng
**32/32 ô, phần dư lớn nhất 0,00005 R** — chân expectancy **mua được** bằng cách
khai stop hẹp hơn (ví dụ `ovn1900/long` W1 cần `Lbar = 0,050/0,0899 = 0,556 R`,
tức `f = 0,156`), **nhưng `PF_r` thì không**.

⇒ **Với cả họ giữ-cửa-sổ tự quản, chân ràng buộc thật là `PF_r`, và nó miễn
nhiễm với lối thoát đổi-đơn-vị mà `agent/stop-width` tìm ra.** Dòng này trượt
`1,0899` vs `1,200` dưới **mọi** cỡ stop khai báo. Đó là một câu đóng, không phải
một con số cần quét thêm.

**Chi phí/R, kèm cả bốn thứ phụ lục 9 mục II đòi:** cửa sổ W1 / W2, thống kê
trung bình mỗi lệnh, **cỡ stop = 1,000 biên độ ngày NY trung bình 20 ngày** (KHÔNG
phải ATR 15m), spread **0,28 USD/vòng — giá trị `markets.xauduka.trading.spread`
trong config, và config tự ghi rằng nó ĐO từ terminal 2026-09-12**. Đo được
**1,19% – 1,80% của R**. Engine **không in** dòng `cost-matched null` cho dòng
tự quản (không có stop thi hành để khớp), nên con số này tôi tự tính từ
`spread paid` / `net` / `E` trong receipt.

**`gross/spread` — con số duy nhất quyết định, và nó là con số đáng giữ nhất:**

    ovn1900/long  gross/spread = 1,43x (W1)  |  1,58x (W2)
    ovn1800/long  gross/spread = 1,51x (W1)  |  2,24x (W2)
    khuon `rollover-flat` CAN         >= 8,7x
    cao nhat ca registry truoc day       3,94x

Một vòng spread ăn **44% – 71%** gross của dòng tốt nhất. Dòng này cách cái khuôn
duy nhất còn sống **một hệ số 4 đến 6**.

## 3. Falsifier — bắn chưa, ở đâu

**F1 KHÔNG bắn, và nó không bắn ở CẢ HAI điều khoản** — nên tôi tiêu 32 ô như đã
khai, không phải 0.

- **Điều khoản 1 ("không mốc nào cùng dấu + p<0,05 trên cả hai cửa sổ")**: SAI.
  **47 / 144 cặp** sống qua sàng, so với **0,18** kỳ vọng do may rủi.
- **Điều khoản 2 ("ba cửa sổ hand-picked không nằm đầu bảng")**: SAI cho
  `ny-morning` — nó là **hạng 1 / 24 theo biên độ trên CẢ HAI nửa**.

**F2 (sàn cỡ mẫu) bắn đúng một chỗ, và nó là một dữ kiện hạ tầng:** giờ **17:00
NY là mốc nghỉ CME**. Nó có **503 / 4.113 ngày** đủ 4 nến, **toàn bộ nằm trong
W1**; trong W2 nó có **0 ngày** ⇒ ở W2 giờ 17 là **`null`, không phải 0**, và nó
không có hàng trong bảng W2. Mọi mốc khác đạt **1.551 – 2.051 ngày**, trên sàn
200 rất xa.

**F3 (đồng hồ) BẮN trên đúng một mốc, và nó cho hiện vật thứ 13 một ví dụ cụ
thể:** `UTC 02:00` sống qua sàng ở đại lượng thô (W1 +0,0095 p 0,0000 / W2
+0,0048 p 0,0483) nhưng **chết trên thước NY** (các giờ NY tương ứng 21/22 cho
p 0,554 và 0,687). Nó là **hiện vật của thước đồng hồ**, không phải cơ chế.
Ngược lại, mốc nghỉ **không** là hiện vật đồng hồ: nó xuất hiện ở cả hai thước,
chỉ bị **DST xé làm hai** trên thước cố định (NY 18 +0,0309/+0,0272 → UTC 22
+0,0213/+0,0221 **cộng** UTC 23 +0,0151/+0,0175). ⇒ **Thước NY là thước đúng;
thước UTC cố định pha loãng cùng một hiệu ứng.**

## 4. Sổ đa phép thử — khai vs xem

| khối | khai trước | đạt p<0,05 (một cửa sổ) | sống qua sàng CẶP |
|---|---|---|---|
| CHÍNH: 24 giờ × 2 đại lượng × 2 cửa sổ, thước NY | 96 | 57 | sgn 3/24 · biên độ **17/24** |
| PHỤ 1: y hệt, thước UTC | 96 | 59 | sgn 3/24 · biên độ **18/24** |
| PHỤ 2: dấu thô so với 0, hai thước | 96 | 25 | NY 2/24 · UTC 4/24 |
| **tổng p-value** | **288** | **141** | **47 / 144 cặp** |
| ô cổng | 32 | — | **0 qua** |

Kỳ vọng do may rủi: **14,4** cú p<0,05 trên 288, và **0,18** mốc sống qua sàng
cặp trên 144. Đo được **141** và **47**. Hai con số này **không** nói cùng một
chuyện, và chỗ chúng rẽ ra là kết quả:

**Toàn bộ cái "sống" đó là biên độ.** 35 trong 47 mốc sống là biên độ
(17 + 18). Của **chiều** chỉ còn 12, và khi mở ra:

| mốc chiều sống (thước NY) | W1 | W2 | nó là gì |
|---|---|---|---|
| giờ 18 | +0,0309 (t +13,09) | +0,0272 (t +10,33) | **mốc nghỉ** — mục 5 |
| giờ 16 | −0,0137 (t −7,46) | −0,0094 (t −4,91) | **mốc nghỉ**, phía trước |
| giờ 19 | +0,0046 (p 0,046) | +0,0064 (p 0,003) | mốc sạch **duy nhất** |

Và mốc sạch duy nhất đó, quy ra tiền: **+0,25x** một vòng spread (W1) và
**+0,97x** (W2). **Nó không trả được tiền để vào nó.** Không mốc giờ nào trong
24 mốc, ở cửa sổ nào, đạt **một** vòng spread bằng dịch chuyển có dấu của nó —
**trừ hai mốc nghỉ** (17: −2,02x; 18: +2,04x / +3,41x).

**Thêm vào sổ sau khi chạy (khai ở đây, không có trong 288):** 2 p-value chéo
feed (mục 5) và các số mô tả của mục 6–7 (không có p-value).

## 5. Hai mốc chiều lớn nhất cả bảng là MỘT DẤU GIÁ CỦA NHÀ CUNG CẤP, không phải thị trường

Hiệu ứng giờ 18 nằm **trọn trong một nến 15 phút** — nến 18:00–18:15 — trong khi
ba nến còn lại của giờ đó về 0:

    XAUDUKA, nen dau gio 18, (close-open)/DR20:
      :00  +0,03099 (W1)  +0,02309 (W2)      <- toan bo hieu ung
      :15  +0,00033        +0,00551
      :30  +0,00222        -0,00026
      :45  -0,00283        +0,00098
    va GAP vao nen :00 chi +0,00215 / +0,00190 — gan nhu KHONG CO GAP
    qua mot muc nghi 75 PHUT (trung vi khoang cach 75,0 phut)

Một mốc nghỉ 75 phút của vàng mà **không** có gap là dấu hiệu của một dấu giá,
không phải một thị trường. Kiểm chéo ba feed (`receipts/hourscreen/halt-and-stretches.txt`):

| feed | `open` của nến mở lại **là đáy** nến | nến thường | `close` của nến trước nghỉ **là đáy** | nến thường |
|---|---|---|---|---|
| `XAUDUKA-15m` (vàng, Duka, 2.671 lần) | **19,02%** | 3,20% | **13,25%** | 1,88% |
| `XAGDUKA-15m` (**bạc**, Duka, 2.632 lần) | **24,16%** | 4,86% | **26,44%** | 3,29% |
| `XAUUSD-15m` (vàng, **feed của sàn**, 845 lần) | 6,75% | 2,77% | **1,42%** | 1,18% |

⇒ Cái dấu giá **có trên cả hai instrument của cùng nhà cung cấp** và **gần như
biến mất trên feed của sàn** (phía trước mốc nghỉ: 1,42% vs 1,18% — đúng bằng
nến thường). **Hai mốc chiều lớn nhất của cả bảng 24 giờ là một thuộc tính của
cách Dukascopy đóng nến quanh mốc nghỉ.** Thêm hiện vật phép đo thứ 13, và nó
không phải cửa sổ hay đồng hồ — nó là **ranh giới của tape**.

**Engine không ăn được nó, và điều đó đo được:** dòng `ovn1800/long` vào **sau**
nến mang dấu giá (tín hiệu ở nến 18:00, khớp một nến sau), và nó chỉ hơn
`ovn1900/long` **+0,0107 `PF_r` ở W1** trong khi hơn **+0,0713 ở W2** — tức phần
đóng góp của chính mốc nghỉ **không ổn định giữa hai cửa sổ**. Ở W2 ba giờ quanh
mốc nghỉ (16+17+18) cầm **+0,0215 DR20 = +0,71 USD = 2,53x spread**, tức **43%
toàn bộ drift của cả đồng hồ W2** (+0,0501 DR20). Bất kỳ kết quả 16 năm nào của
hồ sơ có vị thế mở qua 17:00–18:15 NY trên feed Duka đang mang phần đó.

**Một chỗ luật §7 của brief trúng:** `flat 16:30–18:15` của batch giữ sổ **ra
ngoài đúng chỗ này**. Phụ lục 5 mục C khai nó là luật **carry**; đo được nó cũng
là luật che **dấu giá**. Nó được khai vì lý do A và tình cờ làm đúng việc B.

## 6. CÂU HỎI RIÊNG: ba cửa sổ hand-picked nằm ở đâu — trả lời bằng thứ hạng

So **cái được chọn** với **mọi cái có thể chọn cùng hình dạng**: 24 cửa sổ liền
kề 4 giờ (và 24 cửa sổ 7 giờ), trên **cả hai** cửa sổ thời gian.

| cửa sổ của batch | lý do batch **tự khai** trong mã | hạng theo **biên độ** | hạng theo **\|dịch chuyển\|** | sống qua sàng cặp? |
|---|---|---|---|---|
| `ny-morning` 08:00–12:00 | *"trend into the session with the volume"* | **1/24 và 1/24** | 19/24 và 21/24 | **không** |
| `london-open` 02:00–06:00 | *"London sets the range"* | 9/24 và 8/24 | 3/24 và 6/24 | **không** (lật dấu: −0,0272 → +0,0129) |
| `asia` 19:00–02:00 | *"thin hours are said to mean-revert"* | 19/24 và 17/24 | 5/24 và 8/24 | **có** (+0,0290 p 0,0000 / +0,0177 p 0,0104) |

**Trả lời, bốn phần:**

1. **`ny-morning` MANG một phép chọn — trên biên độ, và nó là cực đại chính xác.**
   Hạng 1/24 trên cả hai nửa. Bốn giờ thành phần xếp **1,2,3,4** (W1) và
   **3,1,2,4** (W2) trong bảng 24 giờ. Giờ 08 rộng **0,363 DR20 = 6,87 USD** so
   với giờ 16 rộng **0,115 DR20 = 2,18 USD** — **gấp 3,15 lần**. ⇒ **Mọi kết quả
   mang nhãn `ny-morning` đang nằm ở đầu một thước 24 bậc**, và phải đọc như thế.
   *Nhưng* biên lợi thế mỏng: 0,673 so với 0,650 của `07:00–11:00` (+3,5%) ở W1
   và 0,690 so với 0,665 (+3,8%) ở W2 — **một cao nguyên, không phải một đỉnh**.
2. **`ny-morning` KHÔNG mang phép chọn nào trên chiều.** Hạng 19/24 và 21/24,
   p 0,629 và 0,873, không sống qua sàng. Và đo trong engine: cửa sổ rộng nhất
   của ngày, giữ **long** cho `PF_r` **0,8143 / 0,9061**, giữ **short** cho
   **0,9952 / 0,9487** — **cả hai chiều đều lỗ trên cả hai cửa sổ**.
   ⇒ **Biên độ không phải edge.** Đây là câu đáng giữ nhất của mục này: thứ duy
   nhất giờ-trong-ngày chứa một cách áp đảo (đồng hồ biên độ, |t| tới 57,8 trên
   thước NY và 61,8 trên thước UTC) **không trả một xu nào theo chiều nào.**
3. **`london-open` KHÔNG mang phép chọn, và lý do nó tự khai là SAI ĐO ĐƯỢC.**
   *"London sets the range"* — 02:00–06:00 NY là hạng **9/24 và 8/24** theo biên
   độ, giữa bảng. Nếu ai chọn cửa sổ này bằng cách nhìn biên độ trên dữ liệu thì
   họ đã chọn sai. Chiều của nó **lật dấu** giữa hai nửa.
4. **`asia` đúng là "thin" (hạng 19/24 và 17/24 theo biên độ — nhất quán với lời
   khai), nhưng lời khai gắn SAI CƠ CHẾ.** Batch gắn nó với `rsi-reversion` và
   `bb-fade` vì *"mean-revert"*. Đo được: bảy giờ đó cầm một **drift một chiều,
   LÊN**, cùng dấu p<0,05 trên cả hai nửa. ⇒ **Batch ghép đúng cửa sổ với sai
   phát biểu.** Và `asia` **không** phải cực đại ngay trong nhóm sống: cửa sổ 7
   giờ bắt đầu **18:00** mạnh hơn (+0,0522 / +0,0466 so với +0,0290 / +0,0177),
   chỉ là nó ôm mốc nghỉ.

**Dấu vân tay của một bộ nhãn khai theo TIÊN NGHIỆM, không phải fit dữ liệu:**
một cửa sổ ở cực đại, một ở giữa bảng, một ở phần ba cuối **với sai cơ chế**.
Một batch chọn bằng cách nhìn dữ liệu thì cả ba đã ở đầu bảng. ⇒ **Các nhãn phiên
của hồ sơ KHÔNG mang một phép chọn ẩn có hệ thống** — nhưng `ny-morning` **mang
một phép chọn công khai trên biên độ**, và hai nhãn còn lại mang **những phát
biểu dữ liệu bác bỏ**. Ba kết luận khác nhau cho ba nhãn, không phải một.

⚠️ Và lý do của `ny-morning` — *"the volume"* — **không đo được trên feed này**:
`volume` của `XAUDUKA-15m` là **0 ở 378.749/378.749 dòng** (phụ lục 8 mục I, phụ
lục 9 mục III). Nhãn đứng trên một cột không tồn tại; cái đúng về nó (biên độ)
không phải cái nó tự khai.

## 7. Con số đáng giữ nhất, kể cả khi âm

**Drift 16 năm của vàng được kiếm TRONG ĐÊM, và ban ngày trả lại.** Tổng dịch
chuyển có dấu theo từng đoạn đồng hồ NY (`receipts/hourscreen/halt-and-stretches.txt`):

| đoạn | W1 (DR20 tb 18,93 USD) | W2 (DR20 tb 32,88 USD) |
|---|---|---|
| đêm 19–01 (giờ `asia`) | **+0,0311 DR20 = +0,59 USD = +2,10x spread** | **+0,0165 = +0,54 USD = +1,94x** |
| ngày 02–16 (London + NY) | **−0,0472 = −0,89 USD = −3,19x** | +0,0046 = +0,15 USD = +0,53x |
| `ny-morning` 08–11 | −0,0053 = −0,10 USD = −0,36x | +0,0015 = +0,05 USD = +0,18x |
| ranh giới nghỉ 16+17+18 | −0,0133 = −0,25 USD = −0,90x | **+0,0215 = +0,71 USD = +2,53x** |
| cả đồng hồ | **−0,0157 = −0,30 USD** | **+0,0501 = +1,65 USD** |

Đọc theo hàng: **cú lật dấu drift mà phụ lục 5 mục B ghi (−0,0040R → +0,0205R
mỗi phiên) nằm Ở ĐOẠN BAN NGÀY** (−0,0472 → +0,0046 DR20). **Đoạn ban đêm giữ
dấu** (+0,0311 → +0,0165). ⇒ **Đoạn 19:00–02:00 NY là đoạn duy nhất của đồng hồ
dương trên CẢ HAI nửa**, và ở W1 nó dương **trong khi cả đồng hồ âm**. Nó không
phải một phần của drift; nó **là** drift, và phần còn lại của ngày là cái phủ
định nó.

Kiểm chéo trên feed thứ ba, độc lập (`XAUUSD-15m`, feed MT5 của sàn,
2022-06 → 2026-09): 19:00→02:00 NY cho **+0,0238 DR20, t +2,32, p 0,0205,
n 856 đêm** — **cùng dấu, cửa sổ thứ ba**. Trên **bạc cùng nhà cung cấp, 16
năm**: **+0,0007 (p 0,90)** và **+0,0075 (p 0,31)** — **không có**. ⇒ hiệu ứng là
của **vàng**, không của nhà cung cấp.

**Và đây là chỗ nó chết, bằng số, không bằng ý kiến:**

    0,59 USD mot dem / 0,28 USD mot vong spread = 2,10x        (tien kiem, W1)
    do trong engine, ovn1900/long: gross/spread = 1,43x / 1,58x
    khuon `rollover-flat` CAN:                                  >= 8,7x

Dòng này **giữ hoàn toàn giữa hai mốc 17:00 NY** nên `swap$ = 0` trên **cả 32
ô** — carry tránh được **thật**, đúng như phụ lục 5 mục C mô tả. **Cái khuôn vẫn
đóng vì tín hiệu, không vì carry.** Và nó đóng ở hệ số 4–6, không phải ở lề.

## 8. Thứ KHÔNG đo được, và vì sao

1. **`volume` trên mọi feed Duka** — 0 ở 378.749/378.749 dòng `XAUDUKA-15m`. Nên
   lý do *"the volume"* của `ny-morning` **không kiểm được trên bộ này**, và tôi
   không thay nó bằng biên độ rồi gọi là xác nhận.
2. **Giờ 17:00 NY ở W2** — `null`. 0/2.036 ngày đủ 4 nến. Ở W1 có 503 ngày,
   nhưng là **503 ngày không ngẫu nhiên** (những ngày Duka vẫn in trong mốc
   nghỉ), nên con số của nó không so được với 23 mốc kia.
3. **Chi phí/R từ dòng `cost-matched null`** — engine **không in** nó cho dòng
   tự quản (`check_exit` trả `None` ngay ở `self_managed` nên không có stop thi
   hành để khớp). Con số 1,19–1,80% của R tôi tự tính từ `spread paid`/`net`/`E`;
   nó **không** phải cùng một phép đo với 4,04% / 12,58% / 8,35% đang lưu hành.
4. **`wrong_side_stop`** — `grep -rn wrong_side_stop crates/` trên cây này cho
   **0 kết quả** (xác nhận phụ lục 8 mục III.2). **Không có bộ đếm ⇒ không đo
   được**, và tôi không báo một số 0 đọc từ nó.
5. **Excursion trong lệnh** — drawdown của engine là đường vốn **đã đóng lệnh**;
   `avg_mae` là trường duy nhất thấy excursion (−0,194 … −0,241 R ở các dòng đêm).
   Không ô nào in `_pct > 100%` ⇒ **không dòng nào cháy**; cao nhất là **50,21%**
   (`ovn1800/short` W2 guards on).
6. **Phân vị** — tôi **không công bố** (phụ lục 8/9: `SURVIVES` phụ thuộc chỗ
   ngồi trong file TOML). Để ghi nhận: ở `--seeds=100`, W1 guards-off, dòng
   `ovn1900/long` in `null p50` **0,826** và `null p95` **0,922** — **null trung
   vị LỖ**, nên một phân vị cao ở đây chỉ nghĩa "lỗ ít hơn vào ngẫu nhiên", đúng
   như chính output tự cảnh báo. Tôi chạy `--seeds=5` thay vì 100 **chính
   vì** không công bố phân vị, và kiểm được rằng điều đó không đổi số của phương
   pháp: ba dòng đầu của W1 guards-off khớp **tới từng chữ số** giữa `--seeds=100`
   (`gate-W1-guardsoff-seeds100-partial.txt`) và `--seeds=5` — n 1644/1644/1623,
   `PF_usd` 1,081/0,630/1,093, `Lbar` 0,0868/0,1121/0,0898, sụt
   936,71/5.008,86/1.076,25 USD. (Một điểm **dương** nhỏ so với phụ lục 8 mục II:
   ở đây chạy lại khớp 3/3.)

## 9. Năm bẫy dụng cụ đã kiểm, với số

1. **Trần giữ 4h KHÔNG cắt dòng nào.** Receipt in `max hold: 4 h`, nhưng
   `engine.rs:772-777` mở đầu `if position.self_managed { return None }`
   **trước** phép kiểm `max_hold_ms` (`engine.rs:800`). Khẳng định bằng số: thời
   gian giữ trung bình **360,0 – 495,0 phút** (6,0 – 8,25 giờ) và **TIMEOUT nổ 0
   lần trên cả 32 ô**. ⇒ hiện vật "đồng hồ = trần giữ" (số 2 trong danh sách)
   **không** chạm job này.
2. **Luật của cơ chế CÓ nổ, 99,9%.** `--exit-mix`: `window closed` **1.545 –
   1.650** trên **1.551 – 1.650** lệnh. Phần còn lại là `END_OF_DATA 1`,
   `OPEN_LOSS_CAP 1–5`, `NEWS_FLAT 1`. Không có ca `tsmom/120d` ở đây.
3. **`swap$ = 0` trên cả 32 ô** — mọi dòng giữ giữa hai mốc 17:00 NY.
4. **`cap_lots` KHÔNG bịt gì: `sized down 0` trên cả 32 ô.** (So với 57,0% của
   dòng `close/1615-2200` mà `agent/stop-width` đo.) Và khuyết điểm 17 **không
   áp dụng**: `xauduka` thừa kế `starting_equity_usd = 10_000.0` của
   `config/default.toml:95`, không phải 100 USD của `xauusd` — nên `PF_usd` ở đây
   **không** phải PF của một sổ 0,01 lot.
5. **Chiều khe `PF_usd − PF_r`: ở bộ này `PF_r` CAO hơn ở 23/32 ô (72%)** —
   **ngược** tỉ lệ 320/452 (71% USD cao hơn) của phụ lục 8 và **cùng chiều** với
   `partial-exit` (73,8%). Phép đo độc lập thứ **năm**, kết quả thứ **ba**. Phụ
   lục 9 mục I đúng: **đo từng bộ, đừng trích tỉ lệ.** Khe lớn nhất ở đây chỉ
   **0,0119**, và **không ô nào** có hai chân ở hai phía vạch 1,200.

**Guards:** `ovn*` gần như không bị guards chạm (`OPEN_LOSS_CAP` 1–3 lệnh,
`refused: none`), nên arm **có guards** — arm duy nhất chủ cho phép — cho gần như
cùng số: `ovn1900/long` W1 `PF_r` **1,0899** ở cả hai arm. `day0800` thì bị
`NEWS_FLAT` từ chối **71 (W1) / 92 (W2)** lệnh, đúng như một cửa sổ chứa 08:30 NY
phải bị. ⇒ **kết quả của job này không sống nhờ arm bị cấm.**

## 10. Kết luận — hai câu, và cả hai đều đóng một thứ

1. **Giờ trong ngày CHỨA một thứ đo được, rất lớn, và ổn định hơn mọi thứ trong
   hồ sơ: một đồng hồ BIÊN ĐỘ** (17/24 và 18/24 mốc sống qua hai cửa sổ, |t| tới
   57,8 / 61,8, giờ rộng nhất gấp **3,15 lần** giờ hẹp nhất). **Nó không chứa một
   đồng hồ của CHIỀU:** hai mốc chiều lớn nhất là một **dấu giá của nhà cung cấp
   quanh mốc nghỉ CME** (có trên cả vàng và bạc Duka, biến mất trên feed của
   sàn), và mốc sạch duy nhất còn lại trả **0,25x – 0,97x** một vòng spread.
2. **Nhãn phiên của hồ sơ: ba nhãn, ba câu trả lời khác nhau.** `ny-morning`
   **mang** một phép chọn — nó là **cực đại chính xác** của bảng biên độ 24 bậc
   trên cả hai nửa — nhưng là phép chọn trên **đại lượng không trả tiền** (giữ
   cửa sổ đó, long hay short, lỗ trên cả hai cửa sổ). `london-open` và `asia`
   **không** mang phép chọn; lời khai của `london-open` **sai** (giữa bảng biên
   độ) và lời khai của `asia` **gắn sai cơ chế** (fade một đoạn thực ra mang drift
   lên).

Và thứ ba, cái đáng nhất để mang sang: **đoạn 19:00–02:00 NY là đoạn duy nhất
của đồng hồ dương trên cả hai nửa, kiểm được trên feed thứ ba, vắng mặt trên
bạc — và nó đáng 1,43x–1,58x một vòng spread khi cái khuôn duy nhất còn sống
đòi 8,7x.** Không phải "trong tiếng ồn". Là **một phần sáu cái cần**, đo xong.

---

## Phụ: nơi đọc từng con số

| file | chứa |
|---|---|
| `receipts/hourscreen/hour-screen.txt` | 24 mốc × 2 đại lượng × 2 cửa sổ × 2 thước, t và p; sàng cặp; 8 bảng xếp hạng; 4 cửa sổ hand-picked đọc theo hạng; cả 24 cửa sổ 4h và 7h |
| `receipts/hourscreen/hours-{ny,utc}.csv` | bảng mốc giờ dạng máy đọc |
| `receipts/hourscreen/windows-{ny,utc}.csv` | bảng cửa sổ liền kề dạng máy đọc |
| `receipts/hourscreen/halt-and-stretches.txt` | dấu giá mốc nghỉ trên ba feed; đêm 19:00→02:00 trên ba feed; phân rã đồng hồ theo đoạn |
| `receipts/hourscreen/gate-W{1,2}-guards{off,on}.txt` | 32 ô cổng, `--exit-mix`, `Lbar`/`PF_r`, drawdown USD |
| `receipts/hourscreen/gate-W1-guardsoff-seeds100-partial.txt` | ba dòng ở `--seeds=100` để kiểm seeds không đổi số của phương pháp |
