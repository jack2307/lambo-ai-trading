# Đăng ký trước — điều kiện hoá theo chế độ biến động (trục m8)

Ngày: 06/10/2026. Nhánh `agent/m8`, worktree `/e/rust/fd-a8`.
Viết và commit MỘT MÌNH NÓ trước lệnh `search.exe` đầu tiên.

## Giả thuyết

Một cơ chế đảo chiều cần biến động **co lại** và một cơ chế theo xu hướng cần
biến động **giãn ra**; nếu trộn cả hai chế độ vào một con số PF duy nhất thì
hai nửa triệt tiêu nhau, nên chia theo chế độ (tỉ lệ ATR14/ATR100) sẽ làm lộ
ra một nửa có edge. Dự đoán cụ thể: `rsi-reversion` và `bb-fade` lên PF ở dải
**co**, `ema-cross` và `donchian-breakout` lên PF ở dải **giãn**, so với cùng
cơ chế không điều kiện hoá.

Giả thuyết thay thế (và là cái tôi cho là có xác suất cao hơn, dựa trên 18
phiên bản đảo chiều của CRT đều lỗ với PF 0,457–1,055): cơ chế lỗ **đều** ở
mọi chế độ, tức là nó đang trả spread chứ không đang làm gì cả, và chia theo
chế độ chỉ chia nhỏ mẫu mà không di chuyển PF theo hướng giả thuyết.

## Dải — KHAI TRƯỚC, không chỉnh sau khi thấy kết quả

Trục là `vol:14/100` = ATR(14) / ATR(100) trên nến tín hiệu. Biên của filter
là **bao gồm hai đầu** (`ratio >= min && ratio <= max`, đã đọc từ
`crates/fd-strategy/src/filter.rs:373`), nên tôi hạ biên trên của hai dải đầu
xuống để ba dải là một **phân hoạch** thật, không đếm đôi:

    co          vol:14/100:0.0-0.8999
    bình thường vol:14/100:0.9-1.1999
    giãn        vol:14/100:1.2-99.0

Cộng thêm một **arm gộp** (không có filter biến động nào) cho từng cơ chế, làm
mốc so sánh — nếu không có nó thì câu "chia theo chế độ làm lộ ra edge" không
có gì để so với.

Tính chất kiểm tra được: tổng số lệnh ba dải <= số lệnh arm gộp, và phần thiếu
chính là các lệnh vào trong lúc ATR100 còn đang khởi động (ratio = NaN, filter
đóng). Tôi sẽ đối chiếu con số này.

## Cơ chế — 9, không phải 13

Batch `gold-intraday` khai 13 hàng, nhưng 4 trong số đó (`expansion/`×2,
`compression/`×2) khác hàng `intraday/` tương ứng **chỉ ở chỗ có filter biến
động**. Vì dải biến động chính là biến tôi đang điều khiển, 4 hàng đó trùng
với 4 hàng `intraday/` một khi tôi gán dải. Nên tập cơ chế thật là 9 cặp
(chiến lược × cửa giờ):

1. `ema-cross` — weekdays + flat 1630-1815
2. `rsi-reversion` — weekdays + flat
3. `donchian-breakout` — weekdays + flat
4. `bb-fade` — weekdays + flat
5. `ema-cross` — + hours 0800-1200 (NY morning)
6. `donchian-breakout` — + hours 0800-1200
7. `donchian-breakout` — + hours 0200-0600 (London open)
8. `rsi-reversion` — + hours 1900-0200 (Asia)
9. `bb-fade` — + hours 1900-0200 (Asia)

Dải `giãn` của tôi (1.2-99.0) trùng **đúng** với `expansion()` của batch
`gold-intraday`, nên hai hàng `expansion/donchian-breakout` và
`expansion/ema-cross` của batch cũ là một phép đối chiếu chéo sẵn có.

## Đa phép thử — đếm trước

    9 cơ chế × (3 dải + 1 arm gộp) × 2 cửa sổ = 72 ô

Khai **72 ô**. Không phải 78: con số 78 (13×3×2) đếm đôi 4 cơ chế mà điểm
khác biệt duy nhất của chúng là filter biến động tôi đang thay thế.

Chạy: 2 lần gọi `search.exe` (một cửa sổ một lần), mỗi lần 36 hàng từ cùng
một `--batch-file`.

`volabs:14` (biến động **tuyệt đối**) **KHÔNG** nằm trong quét chính: thêm nó
là 54 ô nữa và tôi không có ngân sách đa phép thử cho nó. Khai điều kiện:
tôi chỉ chạy arm `volabs` nếu một dải `vol:14/100` qua cổng ở **cả hai** cửa
sổ với >= 40 lệnh, và chỉ trên đúng cơ chế đó.

## Cách đọc

- Cổng (không chỉnh): `profit factor >= 1,200` **VÀ** `expectancy >= +0,050R`.
- Cỡ mẫu tối thiểu để **kết luận**: **40 lệnh**. Dưới 40 lệnh thì ô được
  **báo cáo** kèm số lệnh nhưng **không kết luận** — PF 1,753 trên 14 lệnh và
  PF 0,682 trên 178 lệnh là cùng một luật trên cùng một cửa sổ.
- Mọi PF viết kèm số lệnh và kèm `null p50` cạnh phân vị.
- Cả hai cửa sổ: A = 2025-07-01..2025-10-01, B = 2025-04-01..2025-07-01.
  Qua một cửa sổ là chưa có gì.
- `--fixed`: tham số cố định trên toàn cửa sổ, không chọn trong fold. Lý do:
  biến duy nhất tôi muốn thay đổi là dải, và walk-forward 4 fold còn chia nhỏ
  mẫu thêm 4 lần nữa trên một trục đã chia nhỏ mẫu sẵn.
- Timeframe 15m (mặc định `config/default.toml`), `--market=xauusd`.
- Báo cả ba dải, kể cả dải tệ. **Không** đi tìm dải tối ưu rồi báo cáo nó.

## Falsifier — cụ thể, bắn được

Trục này tôi TUYÊN BỐ CHẾT nếu **bất kỳ** điều nào sau đây:

1. Không dải nào, trên không cơ chế nào, qua cổng (PF >= 1,2 VÀ
   expectancy >= +0,05R) với >= 40 lệnh trên **cả hai** cửa sổ; **HOẶC**
2. Có ô qua cổng, nhưng **dải thắng ở cửa sổ A khác dải thắng ở cửa sổ B**
   đối với cùng cơ chế. "Dải thắng" = dải có PF cao nhất trong số các dải có
   >= 40 lệnh; nếu không dải nào của một cơ chế đạt 40 lệnh thì cơ chế đó là
   **không đo được** cho phép so sánh này, không phải "thắng dải nào cả".

Nếu falsifier bắn, kết luận là: điều kiện hoá theo biến động không mang sang
được, và trục m8 chết. Đó là một kết quả hợp lệ và tôi sẽ báo đúng như thế.

Dự đoán phụ (ghi trước để kiểm được): tôi dự đoán vấn đề chi phối sẽ là
**chia nhỏ mẫu** — rằng phần lớn trong 54 ô có dải sẽ dưới 40 lệnh, nên
falsifier (1) bắn vì "không đo được" chứ không vì "đo được và lỗ".

## Những gì không được làm trong trục này

- Không `cargo build` / `cargo test` / `cargo run` (đĩa E: còn ~32 GB cho 10
  agent). Dùng `/e/rust/fd-wt-crt/target/release/search.exe`.
- Không mở, không trỏ `--data=` vào `data-sealed/`.
- Không ghi gì vào `/e/rust/flowdesk`.
- Không nâng ngưỡng cổng, không nới cỡ mẫu tối thiểu, không dịch biên dải sau
  khi thấy kết quả. Nếu buộc phải thêm dải thì ô cũ vẫn tính là một "look" và
  phải ghi thêm ghi chú có ngày vào cuối file này, không viết lại dòng cũ.

---

## Ghi chú thêm — 06/10/2026, SAU khi chạy (không viết lại dòng nào ở trên)

1. **Sổ đa phép thử đã chốt**: khai 72 ô, đã xem **đúng 72 ô** (36 hàng × 2 lần
   gọi `search.exe`). Lệch 0. Không hàng nào thêm, không biên dải nào dịch.
2. **Arm điều kiện `volabs:14` KHÔNG chạy**: điều kiện khai trước là "một dải
   `vol:14/100` qua cổng ở cả hai cửa sổ với >= 40 lệnh". Điều kiện không thoả
   (0 của 54 ô dải), nên arm này là **không đo**, không phải 0.
3. **Một điều khai ở trên hoá ra SAI, và tôi để nguyên dòng cũ**: tôi khai
   "tổng số lệnh ba dải <= số lệnh arm gộp". Đo được là tổng ba dải có thể
   **vượt** arm gộp, tới **+36 lệnh** (`m3` cửa sổ B: 280 gộp vs 316 ba dải).
   Nguyên nhân: engine giữ một vị thế (`crates/fd-backtest/src/engine.rs:490`),
   nên chặn một lệnh vào làm sổ rỗng và một tín hiệu muộn hơn được vào. Ba dải
   không phải một phân hoạch của tập lệnh pooled.
4. **Falsifier đã bắn ở cả hai nhánh.** Kết quả đầy đủ:
   `docs/research/runs/2026-10-06-vol-regime.md`.
