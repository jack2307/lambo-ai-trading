# Trục: cùng cơ chế, khác lớp tài sản — batch `gold-intraday` trên EURUSD

Ngày đăng ký: 2026-10-06. Nhánh `agent/m5`, worktree `/e/rust/fd-a5`.
Binary: `/e/rust/fd-wt-crt/target/release/search.exe` (không build).

## Giả thuyết

Bốn chương trình nghiên cứu của desk đóng với 0 phương pháp sống sót, và gần
như tất cả đều đo trên VÀNG. Giả thuyết: 13 cơ chế của batch `gold-intraday`
không phải vô dụng — chúng chỉ không hợp với vàng; **giữ nguyên cơ chế, đổi
instrument sang EURUSD** thì ít nhất một ô qua cổng trên CẢ HAI cửa sổ.

Dự đoán cụ thể trước khi chạy: tôi KHÔNG kỳ vọng điều này đúng. Chi phí theo R
là nghi can số một — EUR có spread tuyệt đối nhỏ hơn vàng ~2000 lần nhưng stop
cũng nhỏ hơn cùng cỡ, nên tỉ số `spread/stop` có thể tệ hơn vàng (1,06% R).
Nếu tỉ số tệ hơn, nó định hình mọi kết quả còn lại và phải nói ra trước.

## Falsifier

**Nếu 0 ô qua cổng trên cả hai cửa sổ, trục này chết và tôi tuyên bố nó chết.**
Khi đó kết luận của desk là: vấn đề nằm ở CƠ CHẾ, không ở instrument — một
kết luận âm nhưng nó đóng hẳn một hướng lớn.

Falsifier này bắn được: 26 ô chính đều in PF và expectancy, cổng đọc trực tiếp
từ hai số đó, không cần phán đoán.

Điều kiện "không đo được" (KHÔNG được in thành 0): nếu dòng
`bounds: ... kept N of M bars` cho thấy một cửa sổ thiếu bar, hoặc một ô có
< 40 lệnh, thì ô đó là `null` — không phải một kết quả âm.

## Đa phép thử — đếm TRƯỚC

- Chính: 13 giả thuyết × 2 cửa sổ × 1 market (`eurduka`, 15m) = **26 ô**.
- Phụ, đã khai trước: cùng 13 giả thuyết × 2 cửa sổ trên `eurusd` (bar riêng
  của broker, nguồn feed khác) = **26 ô**. Chạy vô điều kiện, để falsifier
  được kiểm trên hai feed độc lập chứ không một.
- **Tổng đã khai: 52 ô (run × row).**

Không tính là ô: `--exit-mix` chẩn đoán (cùng luật, chỉ in thêm phân rã lý do
thoát) và phép đo thanh khoản theo giờ New York trên bar thô (đo dữ liệu, không
đo luật).

## Cách đọc

- Cổng, không chỉnh: **PF >= 1,200 VÀ expectancy >= +0,050R**.
- Cỡ mẫu tối thiểu **40 lệnh**. Dưới ngưỡng này không kết luận gì, kể cả khi
  PF đẹp (CRT: cùng luật cùng cửa sổ cho PF 1,753/14 lệnh và PF 0,682/178 lệnh).
- "Có dấu hiệu" chỉ khi qua cổng trên **cả hai** cửa sổ.
- Mọi phân vị viết kèm `null p50` ngay cạnh. Null trung vị của desk có
  PF 0,867 < 1, nên phân vị cao KHÔNG có nghĩa là có lãi.
- Null mặc định `coin` (đúng cái mà mọi phân vị đã công bố được đọc so với).
- Cửa sổ A: `--from=2025-07-01 --to=2025-10-01`.
  Cửa sổ B: `--from=2025-04-01 --to=2025-07-01`.

## Ba thứ phải tự kiểm trước khi tin số nào

1. **Chi phí theo R của EUR** — lấy từ dòng `cost-matched null: ... cost X% of R`
   mà chính output in ra, không suy diễn. So với vàng 1,06% R.
2. **Giờ giao dịch.** Batch mang `flat 16:30-18:15` và `weekdays`, cộng các cửa
   `hours 0800-1200` / `0200-0600` / `1900-0200` — tất cả theo đồng hồ New York
   và tất cả đặt theo phiên của VÀNG. Phải nói rõ: đang đo cơ chế, hay đang đo
   một filter đặt cho thị trường khác? Kiểm bằng cách đo biên độ/volume theo giờ
   New York trên bar EURDUKA thô, và bằng 4 dòng `intraday` của batch (chỉ
   `weekdays`+`flat`, không có cửa phiên) làm ablation sẵn có.
3. **Phủ dữ liệu.** Đã kiểm trước khi đăng ký, từ chính parquet:
   EURDUKA-15m 398 220 bar, 2010-06-01 → 2026-05-31; cửa sổ A 6 336 bar,
   cửa sổ B 6 240 bar. EURUSD-15m 61 247 bar, 2024-03-29 → 2026-09-15;
   cửa sổ A 6 336 bar, cửa sổ B 6 240 bar. Cả hai feed phủ đủ cả hai cửa sổ.
   Vẫn phải đối chiếu lại với dòng `bounds:` của từng receipt.

## Không được chạm

Không `cargo build/test/run`. Không ghi vào `/e/rust/flowdesk`. Không mở
`data-sealed/`. Không sửa cổng. Không đổi batch — điểm của trục này là giữ
nguyên cơ chế và chỉ đổi instrument.
