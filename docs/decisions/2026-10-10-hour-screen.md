# Đăng ký trước — sàng GIỜ TRONG NGÀY, và hỏi chính hồ sơ của desk

**Ngày:** 2026-10-10. Nhánh `agent/hour-screen` (cắt từ `agent/stop-width`).
Commit này **chỉ docs**, trước dòng code đầu tiên.

## 1. Giả thuyết (một câu)

Giờ trong ngày New York chứa một hiệu ứng đo được — chiều (dịch chuyển có dấu)
hoặc biên độ — đủ ổn định để sống qua **hai cửa sổ chia theo thời gian** sau khi
kể đa phép thử; **và** ba cửa sổ phiên hand-picked của batch `gold-intraday`
(`ny-morning` 08:00-12:00, `london-open` 02:00-06:00, `asia` 19:00-02:00) nằm ở
**đầu** bảng xếp hạng 24 giờ, tức nhãn của chúng mang một phép chọn chưa khai.

## 2. Falsifier — cụ thể, bắn được

**F1 (chính).** Nếu **không** mốc giờ nào cho hiệu ứng **cùng dấu VÀ p<0,05 trên
CẢ HAI cửa sổ** (ở bất kỳ thước đồng hồ nào), **VÀ** không cửa sổ hand-picked
nào nằm trong **top 3** của bảng xếp hạng cửa-sổ-liền-kề cùng độ dài (24 cửa sổ
ứng viên mỗi độ dài) theo **cả hai** đại lượng trên **cả hai** cửa sổ thời gian
=> **giờ-trong-ngày không chứa gì đo được VÀ các nhãn phiên không mang phép chọn
ẩn.** Hai kết luận. **Tiêu 0 ô cổng.** (Tiền lệ AUDNZD: khai 432, tiêu 0.)

**F2 (dụng cụ).** Mốc giờ nào có **< 200 ngày quan sát** trong một cửa sổ là
**không đo được** (`null`), **không phải 0**. Nếu quá 2/24 mốc rơi xuống dưới
sàn đó thì thước giờ này không đọc được và job dừng ở đó.

**F3 (đồng hồ).** Nếu một mốc sống ở thước **giờ NY có DST** mà chết ở thước
**UTC cố định** (hay ngược lại), thì nó là **hiện vật đồng hồ** (hiện vật cửa sổ
thứ 13), **không phải cơ chế** — báo như vậy, không mang nó vào cổng.

## 3. Dữ liệu, thước, sàn cỡ mẫu — khai trước

- **Bộ:** `XAUDUKA-15m.parquet`, **378.749 nến**, **2010-06-01 -> 2026-05-31**
  (16 năm chẵn). `volume` = **0 ở 378.749/378.749 dòng** (phụ lục 8 mục I, phụ
  lục 9 mục III: đường ống chế ra số 0) => **không dùng volume**, không nhắc nó
  như bằng chứng cho "giờ có thanh khoản".
- **Hai cửa sổ chia theo thời gian, hai nửa chẵn:**
  - **W1** 2010-06-01 -> 2018-06-01
  - **W2** 2018-06-01 -> 2026-05-31
  (cùng mốc chia với phép đo drift của phụ lục 5 mục B, nơi drift **lật dấu**:
  -0,0040R/phiên -> +0,0205R/phiên. Drift là hiện vật thứ 9 => xem mục 4.)
- **Ngày giao dịch:** chỉ **thứ Hai-thứ Sáu trên đồng hồ New York**, khớp filter
  `weekdays` của batch.
- **Hai thước đồng hồ, đo CẢ HAI:**
  - **NY-DST** — tái dựng **đúng** luật của `fd_core::clock` (luật Mỹ sau 2007:
    Chủ nhật thứ hai tháng 3 02:00 -> Chủ nhật đầu tháng 11 02:00). Đây là đồng
    hồ engine thật sự dùng cho `Filter::hours`.
  - **UTC cố định** — không DST.
- **Chuẩn hoá:** chia cho **biên độ ngày NY trung bình 20 ngày TRƯỚC ĐÓ**
  (`DR20`, đơn vị USD; **không** kể ngày đang đo => không nhìn trước, không tự
  chứa). Đây là thước `quiet-swing`/`riskDailyRanges` đã dùng. Đơn vị kết quả:
  **phần của một biên độ ngày điển hình**. Theo phụ lục 5 mục F và phụ lục 9 mục
  II, **nói rõ thước**: đây **không** phải ATR(15m).
- **Sàn cỡ mẫu:** tiền kiểm >= **200 ngày** mỗi mốc giờ mỗi cửa sổ. Cổng (nếu
  tiêu ô): >= **40 lệnh**, đếm tay (tool kiểm sàn 30, cổng desk 40).

## 4. Hai đại lượng, và vì sao trừ trung bình trong ngày

Mỗi **ngày** cho **một** quan sát cho **mỗi** giờ => **mẫu KHÔNG chồng lấn**, t
đọc được trực tiếp (bài học `agent/lead-lag`: lấy mẫu chồng lấn biến t +1,33
thành t +3,78 — một dương nhầm làm ra từ hư không). ~**2.000 ngày mỗi cửa sổ**,
tức ~**4.000 quan sát mỗi giờ** trên 16 năm.

1. **Dịch chuyển có dấu** (chiều):
   `s[d,h] = (close cuối giờ h - open đầu giờ h) / DR20[d]`
   **Đại lượng CHÍNH = `s[d,h] - mean_h'(s[d,h'])`** (trừ trung bình 24 giờ
   **trong cùng ngày**). Lý do: **drift là hiện vật thứ 9 và nó LẬT DẤU giữa hai
   cửa sổ**; một drift dương làm **mọi** giờ có trung bình dương, nên test so với
   0 sẽ gắn cờ cho đồng hồ cái thuộc về drift. Trừ trung bình trong ngày hỏi
   đúng câu hỏi đồng hồ: **giờ nào CẦM cái dịch chuyển của ngày**.
   **Đại lượng PHỤ = `s[d,h]` thô, test so với 0** (mang drift, khai rõ).
2. **Biên độ:**
   `r[d,h] = (max high trong giờ h - min low trong giờ h) / DR20[d]`
   **Đại lượng CHÍNH = `r[d,h] - mean_h'(r[d,h'])`**. Biên độ luôn > 0 nên
   "khác 0" là vô nghĩa; câu hỏi thật là **rộng/hẹp hơn một giờ trung bình của
   cùng ngày đó**. Cặp theo ngày => vẫn một quan sát mỗi ngày, không chồng lấn.

t hai phía, một mẫu: `t = mean / (sd/sqrt(n))`.

## 5. SỔ ĐA PHÉP THỬ — đếm TRƯỚC, khai số

| khối | phép so |
|---|---|
| **CHÍNH**: 24 giờ x 2 đại lượng (chính) x 2 cửa sổ, thước **NY-DST** | **96** |
| PHỤ 1: y hệt trên thước **UTC** | 96 |
| PHỤ 2: đại lượng **thô so với 0** (dấu), 24 giờ x 2 thước x 2 cửa sổ | 96 |
| **TỔNG p-value khai trước** | **288** |

Mô tả, **không có p-value** (không vào sổ trên): 8 bảng xếp hạng 24 giờ (2 đại
lượng x 2 cửa sổ x 2 thước) và 4 cửa sổ hand-picked x 2 đại lượng x 2 cửa sổ
thời gian x 2 thước = **32 lượt đọc thứ hạng**.

**Số học may rủi, khai trước:**
- Ở p<0,05, **96** phép so cho **~4,8** cú đạt p<0,05 **do may rủi**; **288**
  cho **~14,4**.
- **Vì thế sàng là CẶP, không phải một cú**: cùng dấu VÀ p<0,05 trên **cả hai**
  cửa sổ. Mỗi cặp giờ x đại lượng không-hiệu-ứng có xác suất
  `0,05 x 0,05 x 0,5 = 0,00125` => trên **48** cặp chính, kỳ vọng **0,06** mốc
  sống **do may rủi**; trên cả 144 cặp (3 khối), **0,18**.
- `agent/month-clock` đo được sàng "cùng dấu" **một mình** cho 64/108 qua so với
  54 của may rủi => **vô dụng một mình**. Nên **không** báo "cùng dấu" như một
  kết quả.

**Ô cổng khai trước: 0.** Chỉ tiêu nếu tiền kiểm cho ít nhất một mốc sống qua
cả hai cửa sổ; khi đó **thêm ghi chú có ngày vào CUỐI file này** khai số ô trước
khi chạy.

## 6. Câu hỏi riêng về HỒ SƠ — và nó được đọc thế nào

`gold_intraday_batch()` (`crates/fd-backtest/src/hypotheses.rs:137-170`) khai ba
cửa sổ **bằng tay**: `Filter::hours(800,1200)` (`ny-morning`),
`Filter::hours(200,600)` (`london-open`), `Filter::hours(1900,200)` (`asia`),
cộng `Filter::flat(1630,1815)` trên mọi dòng.

Đọc bằng **thứ hạng**, trên **cả hai** cửa sổ thời gian:
1. Thứ hạng của **từng giờ thành phần** trong bảng 24 giờ, mỗi đại lượng.
2. Thứ hạng của **cửa sổ** trong **24 cửa sổ liền kề cùng độ dài** (4 giờ cho
   `ny-morning`/`london-open`, 7 giờ cho `asia`) — đây mới là phép đọc sắc, vì
   nó so cái được chọn với **mọi** cái có thể chọn cùng hình dạng.
3. `flat 16:30-18:15` đọc riêng: phụ lục 5 mục C khai nó là **luật carry** (số
   lần vượt mốc 17:00 NY), **không** phải một phép chọn theo biên độ — nên nếu
   nó **không** nằm đầu bảng biên độ thì lời khai đó nhất quán.

**Cách đọc:** top 3/24 theo một đại lượng => nhãn **mang** một phép chọn (và mọi
kết quả gắn nhãn đó thừa hưởng nó). Giữa bảng (hạng 8-17) => **không mang**.

## 7. Luật đọc khác — nhắc để không trượt

- **Filter giờ ĐỔI tập lệnh chứ không CHIA nó.** `month-clock` đo tổng hai phần
  bù **lớn hơn** bản không-filter (+17...+22 lệnh) vì engine giữ một vị thế. Nên
  nếu có tiêu ô cổng: **không** cộng số lệnh các mốc rồi so với tổng.
- Nếu tiêu ô cổng: `PF_r` **cạnh** `PF_usd` (khai đơn vị; dùng `E = total_r/n`
  vì `expectancy` 3 chữ số không đủ), **drawdown USD** cạnh mọi số lợi nhuận
  (`_pct` là SÀN; `_pct > 100%` => dòng đã cháy), `--exit-mix` **và kiểm luật của
  cơ chế có nổ** (tiền lệ `tsmom/120d`: PF 2,236 `SURVIVES`, luật nổ 0 lần), tỉ
  lệ `cap_lots`, và **arm có guards là arm duy nhất chủ cho phép**.
- **Không trích một con số đã công bố như dữ kiện** (phụ lục 8 mục II: chỉ
  295/452 ô chạy lại khớp `PF_usd` đã in). Mọi con số trong báo cáo là con số
  tôi tự chạy ra.
- Sáu từ bị cấm: score, composite, confluence, bias, rank, strength. (Tiếng Việt
  dùng "thứ hạng/xếp hạng" cho thứ tự của một đại lượng có đơn vị — không phải
  một điểm tổng hợp.)

## 8. Thứ job này KHÔNG hứa

- **Không** hứa chỉ tiêu drawdown nào ở tiền kiểm (tiền kiểm không chạy engine,
  không có đường vốn).
- **Không** mở `data-sealed/`.
- **Không** đọc `volume` (cột số 0 chế ra).
- **Không** sửa `engine.rs`, không chạm `main`, `config/local.toml`, VPS,
  `collect.exe` (pid 5044, 38720).
