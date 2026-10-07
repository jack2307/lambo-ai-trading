# Đăng ký: đọc lại hồ sơ `rescore` bằng null đã sửa

**Viết:** 2026-10-07 · **Nhánh:** `agent/reread-null` (cắt từ `agent/instr-repair`)
**Trạng thái:** đăng ký, chưa chạy dòng nào.

## Giả thuyết (1 câu)

Khuyết điểm (7) — `--null-sides=` được in trong header của mọi mode nhưng chỉ
`hypotheses` truyền nó vào null — đã làm phân vị `matched` của hồ sơ `rescore`
bị đọc sai, và đọc lại bằng binary `agent/instr-repair` sẽ làm ít nhất một
phân vị đổi quá 3 điểm.

## Falsifier (cụ thể, bắn được)

**F1 (tiền kiểm, bắn trước khi tiêu ô cổng).** Nếu chạy cùng một dòng `rescore`
hai lần — cùng batch file, cùng cửa sổ, cùng `--seeds=`, cùng
`--direction-samples=`, cùng `--spread=`, chỉ khác `--null-sides=coin` so với
`--null-sides=exposure` — mà **mọi con số dưới header trùng nhau từng byte**,
thì cờ đó không hề đổi một phép đo nào trong `rescore`, nên **không phân vị nào
có thể đổi**, và giả thuyết trên bị bác ngay ở tiền kiểm. Tiêu 0 ô cổng.

**F2 (nếu F1 không bắn).** Nếu không phân vị nào trong các dòng đã đo lại đổi
quá **3 điểm phân vị**, việc khai sai là vô hại trên thực tế.

## Đa phép thử — đếm TRƯỚC

Đếm quy mô khai sai (grep header mọi receipt `rescore` đã commit trên mọi
nhánh) **trước** khi đo bất cứ gì; con số đó là kết quả đầu tiên.

Ô sẽ đọc lại nếu F1 không bắn: **286** —
`agent/n6` 12 receipt × 22 dòng = **264** (hai cửa sổ `xauusd:15m`
2025-04-01→2025-07-01 và 2025-07-01→2025-10-01; share 0,39/0,45/0,52; spread
0,28/0,21; guards on/off), cộng `2026-09-23-rebate-rescore` primary
`xauusd:15m` 2025-09-13→2026-09-12 = **22**.

Tiền kiểm F1: 3 cặp A/B = 6 lượt chạy, trên cửa sổ R1, R5 và primary 09-23.

## Cách đọc

- So **từng dòng**: `mnullG`/`mnullN` cũ so mới, và `null p50` cũ so mới.
- `null p50 = 0,000` nghĩa là **không calibrate được**, không phải một phép đo.
- Count match ngoài băng ⇒ **không công bố phân vị** dòng đó.
- Dưới 40 lệnh ⇒ **không kết luận** (verdict của tool kiểm 30, cổng desk là 40).
- Chiều kỳ vọng là **phân vị tụt** (null khớp phơi nhiễm drift đã từng làm ô
  `SURVIVES` duy nhất rơi từ phân vị 98 về trong nhiễu). Phân vị **tăng** là
  chuyện đáng nói hơn và phải nói rõ.
- Đồng nhất thức `reb/R = share × (cost/R)` của n6 **không phụ thuộc vào null**,
  nên kết luận "hoàn phí không tạo được họ cơ chế" đứng nguyên bất kể phân vị
  đổi thế nào. Trục này không lật nó.

**Trục này không có cổng** — nó không đề cử cơ chế nào.
