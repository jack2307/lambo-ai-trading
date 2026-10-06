# Đã đo: batch `gold-intraday` trên EURUSD — trục instrument CHẾT

Ngày 2026-10-06. Nhánh `agent/m5`. Đăng ký:
`docs/decisions/2026-10-06-eur-instrument-axis.md` (commit 1f295ea, docs một mình).
Binary `/e/rust/fd-wt-crt/target/release/search.exe`, không build.
Receipt: `receipts/eur{duka,usd}-gold-intraday-win{A,B}.txt` +
`receipts/eurduka-gold-intraday-winA-exitmix.txt`.

## 1. Falsifier ĐÃ BẮN

Khai trước: "0 ô qua cổng trên cả hai cửa sổ ⇒ trục chết, vấn đề nằm ở cơ chế
chứ không ở instrument."

**0 ô qua cổng trên cả hai cửa sổ. Trên CẢ HAI feed EUR.** Trục chết.

| feed | cửa sổ A (07–10/2025) | cửa sổ B (04–07/2025) | giao của hai cửa sổ |
|---|---|---|---|
| `eurduka` (Dukascopy) | `compression/bb-fade` PF 1,281 / +0,141R / 30 lệnh / phân vị 96% (null p50 0,789) | `asia/bb-fade` PF 1,393 / +0,195R / 38 lệnh / 100% (null p50 0,765) | **rỗng** |
| `eurusd` (bar broker) | 0 ô qua cổng (`compression/bb-fade` PF 1,486 / +0,220R nhưng 27 lệnh) | `asia/bb-fade` PF 1,653 / +0,293R / 35 lệnh / 100% (null p50 0,752) | **rỗng** |

Hai ô duy nhất qua cổng engine đều dưới sàn cỡ mẫu **40 lệnh** mà đăng ký đã
khai trước (30 và 38 lệnh; engine tự chỉ đòi 30). Theo luật đã khai, chúng là
`null` — không đo được, **không phải** một kết quả dương.

Và điều đáng đọc nhất không phải là chúng thiếu lệnh, mà là **ô nào "chạy"
LẬT giữa hai cửa sổ**, lật y nguyên trên cả hai feed độc lập:

- `compression/bb-fade`: cửa sổ A PF 1,281 / 1,486 → cửa sổ B PF 0,860 / 0,843.
- `asia/bb-fade`: cửa sổ B PF 1,393 / 1,653 → cửa sổ A PF 1,100 / 1,167.

Cùng một luật, cùng một instrument, hai quý liền nhau, kết luận ngược nhau. Cái
được đo ở đây là **cửa sổ**, không phải cái cổng regime hay cổng phiên.

## 2. Chi phí theo R của EUR: TỆ HƠN VÀNG, và con số 1,06% không so được

Lấy từ chính dòng `cost-matched null: ... cost X% of R` của output, 13 dòng
cửa sổ A trên `eurduka`: **7,37% – 20,83% của R** (stop 1,5–2,0 ATR,
spread 0,00014/vòng).

Tôi tự kiểm lại từ bar thô (ATR14 Wilder, cùng hai cửa sổ) và được 14,81% ở
stop 1,5 ATR cửa sổ A — khớp với 14,74% mà output in. Phép kiểm qua.

So apples-to-apples, **cùng luật stop 1,5 ATR, cùng hai cửa sổ này**:

| | cửa sổ A | cửa sổ B |
|---|---|---|
| Vàng 15m (XAUDUKA, spread 0,28) | 4,79% R | 3,08% R |
| EUR 15m (EURDUKA, spread 0,00014) | **14,81% R** | **10,22% R** |

**EUR đắt gấp ~3,1 lần vàng theo R**, đúng như dự đoán đã đăng ký: spread tuyệt
đối nhỏ hơn ~2000 lần nhưng stop nhỏ hơn cùng cỡ, và tỉ số là thứ đáng đọc.

Kèm một hiệu chỉnh cho hồ sơ desk: con số **vàng 1,06% R** đang được trích như
chi phí của vàng KHÔNG so được với batch này. Nó đến từ
`docs/research/runs/2026-09-24-cost-matched-null/` và `2026-10-02-drift-null/`,
nơi control stop là **8,85–8,95 ATR (26,22 điểm)** hoặc **2,426 ATR (26,65
điểm)** — một stop cấu trúc rộng, không phải 1,5 ATR của `gold-intraday`. Trên
cùng luật stop 1,5 ATR, vàng là **3,08–4,79% R**, không phải 1,06%. Mọi so sánh
chi phí giữa hai chương trình phải nói stop của nó là bao nhiêu ATR.

## 3. Giờ giao dịch: tôi đang đo cơ chế, với ĐÚNG MỘT filter bị lệch

Đo trên bar thô EURDUKA vs XAUDUKA, Mon–Fri, 2025-04-01→2025-10-01, biên độ
trung vị của nến 15m theo **giờ New York** (hai cửa sổ nằm trọn trong EDT):

- `hours 0800-1200` (nhãn "ny-morning", đặt theo COMEX): **đúng cho cả hai.**
  Vàng đỉnh ở 09:00 NY (100%), EUR đỉnh ở 10:00 NY (100%). Trùng nhau vì đó là
  chồng phiên London–New York. Filter này KHÔNG lệch.
- `hours 0200-0600` ("london-open"): EUR 02:00–04:00 NY = 70–79% của giờ đỉnh,
  đang lên; London mở 03:00 NY = 79%. Cửa này phủ đúng phiên nó định phủ.
- `hours 1900-0200` ("asia"): EUR 33–45%, có một nhô lên 54–60% ở 20:00–21:00 NY
  (Tokyo mở). Vàng cũng vậy (58–77%, Thượng Hải). Nhãn "asia" bắt đúng phiên
  trên cả hai.
- `MoTuWeThFr`: đúng cho cả hai, cả hai là thị trường 5 ngày.
- **`flat 16:30-18:15` là filter DUY NHẤT làm việc khác nhau.** Trên vàng nó
  bọc một lần NGỪNG CẤU TRÚC: giờ 17:00 NY **không có bar nào** (nghỉ CME), và
  chỉ 351 bar (3,2%) nằm trong cửa — nó gần như là no-op. Trên EUR tape vẫn
  chạy: 17:00 NY có bar, biên độ 3,40 pip (28% giờ đỉnh — giờ mỏng nhất ngày),
  và **787 bar (6,6%), gấp 2,2 lần vàng**, nằm trong cửa. Nên trên EUR nó là
  một **luật thoát cưỡng bức đang hoạt động**, không phải một kỳ nghỉ.
  `--exit-mix` xác nhận bằng số: lý do thoát `flat window` đã bắn
  9/10/16/14/5/2/1 lần trên các dòng của cửa sổ A.

Kết luận của phép kiểm này: 4/5 filter chuyển sang EUR đúng nghĩa, nên **tôi
đang đo cơ chế**. Riêng `flat 16:30-18:15` đổi vai từ no-op sang luật thoát —
nhưng nó không cứu được gì: 4 dòng `intraday` của batch chỉ mang
`weekdays`+`flat` và không có cổng phiên/regime nào, chúng là ablation có sẵn,
và tất cả đều thất bại nặng (cửa sổ A PF 0,400–1,111; cửa sổ B PF 0,578–0,930).

## 4. Phủ dữ liệu: đo được, không có "không đo được" nào vì thiếu bar

Từ dòng `bounds:` của chính receipt:

- `eurduka` A: kept **6 336** of 398 220 bar; B: kept **6 240**.
- `eurusd` A: kept **6 336** of 61 247 bar; B: kept **6 240**.

Khớp chính xác với số tôi đếm từ parquet trước khi đăng ký. Cả hai cửa sổ phủ
đủ trên cả hai feed. Không cửa sổ nào là "không đo được".

Phụ: EURDUKA-15m chạy 2010-06-01 → 2026-05-31 (398 220 bar), EURUSD-15m
2024-03-29 → 2026-09-15 (61 247 bar). `news:` in ra 747 event từ
`E:/rust/flowdesk/data/news/events.parquet`, scope `USD|EUR` theo
`news_currencies` — đúng nguồn mà `--data=` trỏ tới, không phải bẫy `data/`
mặc định. `timeline: none`, `tape: 0 prints`, `companion: none`, `guards: off`,
`trail: off`, `null sides: coin`.

## 5. Hai lỗi hiển thị gặp được (không ảnh hưởng số, ảnh hưởng người đọc)

- `control stop 1.500 ATR = 0.00 points` — cột `points` in 2 chữ số thập phân,
  nên mọi stop của một instrument 5 chữ số đọc ra **0,00**. Dòng ngay dưới
  (`the method's own realised stop: median 1.611 ATR = 0.00 points`) cũng vậy.
  `[markets.eurduka.trading] price_decimals = 5` có trong config nhưng dòng in
  này không dùng nó. Phần trăm của R vẫn đúng (tính từ giá trị thật, không từ
  chuỗi đã làm tròn) — tôi đã đối chiếu lại bằng tay ở mục 2.
- `pct` của nhiều dòng mang cờ `** outside the band **` cho count/cost/exposure
  match (ví dụ `compression/rsi-reversion` cửa sổ A: count match 0,21, cost
  match 0,21). Trên những dòng đó phân vị là **không so được**; cổng (PF +
  expectancy) mới là chân quyết định, và cổng vẫn trả lời rõ.

## 6. Sổ đa phép thử

- Đã khai trước (commit 1f295ea): **52 ô** = 13 giả thuyết × 2 cửa sổ × 2 feed.
- Đã xem: **đúng 52 ô.** Không thêm market, không thêm cửa sổ, không thêm giả
  thuyết, không chỉnh cổng, không đổi batch.
- Cộng 1 lần chạy lại chẩn đoán `--exit-mix` (eurduka, cửa sổ A) đã khai trước
  là không-tính-ô. Đã `diff` và các số cổng của nó **giống hệt** lần chạy gốc;
  nó chỉ thêm phân rã lý do thoát. Không phải một look mới.
- Trong 52 ô: **2 ô** qua cổng engine (ngưỡng 30 lệnh), **0 ô** qua sàn 40 lệnh
  đã khai, **0 ô** qua cổng trên cả hai cửa sổ.

## 7. Kết luận cho desk

Các cơ chế của `gold-intraday` không phải "chỉ không hợp với vàng". Giữ nguyên
cơ chế và đổi sang một lớp tài sản khác (FX major, hai feed độc lập, 7 năm dữ
liệu có sẵn, chi phí thực tế của broker) cho **cùng một kết quả rỗng**. Hướng
"vấn đề là instrument" ĐÓNG.

Hơn nữa EUR làm cho vấn đề **nặng hơn**, không nhẹ hơn: ở 15m EUR đắt gấp ~3
lần vàng theo R, nên cùng một cơ chế phải vượt một hàng rào chi phí cao gấp ba.
Nếu desk còn muốn thử trục instrument lần nữa, điều kiện cần là một instrument
có tỉ số `spread/stop` THẤP hơn vàng ở khung đang đo — EUR 15m không phải nó,
và XAGDUKA (bạc, spread chưa đo theo R) là ứng viên duy nhất còn lại trong kho
chưa bị loại theo tiêu chí này.
