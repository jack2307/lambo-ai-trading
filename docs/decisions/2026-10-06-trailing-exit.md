# Luật thoát: trailing stop — đăng ký trước khi chạy

Ngày: 2026-10-06. Nhánh `agent/m4`, worktree `/e/rust/fd-a4`.
Binary: `/e/rust/fd-wt-crt/target/release/search.exe` (build 04/10 15:13, đã
chứa null kiểm soát drift). KHÔNG build lại.

## Giả thuyết

Mọi cuộc quét của desk tới nay chỉ dùng MỘT luật thoát: stop cố định theo ATR
và target cố định. Cờ `--trail=<distance_r>,<activate_r>` có trong binary và
chưa cuộc quét nào dùng. Giả thuyết: **với cùng tín hiệu vào, một trailing
stop giữ lại được phần đã đi đúng hướng mà luật cố định trả lại, đủ để đẩy
một cơ chế gần cổng qua cổng trên CẢ HAI cửa sổ.**

Lý do nghi: phân bố MFE của sổ `xau-stoch` (73 lệnh đã đóng, tiền thật) có
**81% lệnh chạm +0,33R** nhưng chỉ **11% chạm +2,00R** — phần lớn lệnh đi
đúng một đoạn rồi quay lại. Trailing stop là câu hỏi trực tiếp: đoạn đó giữ
được không, hay giữ nó thì mất phần đuôi còn đắt hơn?

Dự đoán cụ thể (viết trước khi thấy số): activate thấp (0,33R) + distance
hẹp (0,5R) sẽ **tăng tỉ lệ thắng và giảm avgR**, và vì chi phí vàng là
1,06% R mỗi lượt, cái được từ việc cắt phần quay lại sẽ KHÔNG đủ bù phần
đuôi bị cắt. Tôi dự đoán trục này chết. Đo để biết, không để cứu.

## Falsifier — cụ thể, bắn được

Trục này TUYÊN BỐ CHẾT nếu **bất kỳ** điều nào sau xảy ra:

1. Ô tốt nhất của lưới trail không qua cổng (PF >= 1,200 VÀ expectancy >=
   +0,050R, >= 40 lệnh) trên **cả hai** cửa sổ; **hoặc**
2. Ô thắng ở cửa sổ A không phải ô thắng ở cửa sổ B — tức cặp
   (distance_r, activate_r) tốt nhất không trùng nhau giữa hai cửa sổ;
   **hoặc**
3. `--exit-mix` cho thấy trail gần như không kích hoạt — phân bố
   STOP/TARGET/TIMEOUT của ô trail lệch dưới 5 điểm phần trăm so với ô
   `--trail=off` tương ứng. Khi đó tôi đang đo lại đúng luật cũ, và lưới
   không nói gì về trục thoát.

Falsifier 3 bắn được vì engine đã in exit mix cho từng dòng; falsifier 1 và 2
bắn được vì cổng là số cố định và hai cửa sổ chạy cùng một batch file.

## Đa phép thử — ĐẾM TRƯỚC

Lưới thô, khai trước khi chạy:

    distance_r  in {0,5; 1,0; 1,5}
    activate_r  in {0,33; 1,0; 2,0}      => 9 cặp, KHÔNG phải 100

`activate_r` lấy đúng từ phân bố MFE: 0,33R là nơi 81% lệnh chạm, 2,00R là
nơi chỉ 11% chạm, 1,0R ở giữa.

Sổ (run, row) khai trước:

| phần | run | row/run | row |
|---|---|---|---|
| sàng lọc không trail, `--batch=gold-intraday`, 2 cửa sổ | 2 | 13 | 26 |
| arm đối chứng `--trail=off` trên batch file 3 cơ chế, 2 cửa sổ | 2 | 3 | 6 |
| lưới trail 9 cặp × 3 cơ chế × 2 cửa sổ | 18 | 3 | 54 |
| **tổng khai** | **22** | | **86** |

Nếu con số thật vượt 86 thì tôi công bố cả hai con số và nói vượt ở đâu.
Không nâng lưới sau khi thấy kết quả.

## Chọn cơ chế nào để trải lưới — luật chốt TRƯỚC

Không trải lưới lên cả 13 cơ chế (13 × 9 × 2 = 234 ô, đa phép thử nổ).
Luật chọn, một tiêu chí duy nhất, không phải tổ hợp:

> Chạy `--mode=hypotheses --batch=gold-intraday --trail=off` trên cả hai cửa
> sổ. Với mỗi cơ chế lấy **min(PF cửa sổ A, PF cửa sổ B)** và đòi **>= 40
> lệnh out-of-sample ở CẢ HAI cửa sổ**. Ba cơ chế có min-PF lớn nhất được
> trải lưới. Ba, không phải bốn.

Dùng min của hai cửa sổ chứ không phải cửa sổ gần nhất, vì brief §0: hiệu
quả ở cửa sổ gần đây đã được ĐO là không mang sang được, nên không được dùng
một cửa sổ để chọn.

**Giới hạn đã biết của bước này, nói trước:** bước chọn cơ chế đọc cả hai
cửa sổ, nên cả hai cửa sổ bị nhiễm ở CHIỀU CƠ CHẾ. Cái còn sạch là chiều
trail: cặp (distance, activate) không được chọn theo cửa sổ nào — falsifier 2
đòi ô thắng phải TRÙNG giữa hai cửa sổ, và điều đó không thể đạt bằng cách
chọn riêng từng cửa sổ.

## Cách đọc

- Cổng: `profit factor >= 1,200` VÀ `expectancy >= +0,050R`. Không chỉnh.
- Cỡ mẫu tối thiểu: **40 lệnh**. Dưới đó không kết luận gì (brief §4: cùng
  một luật cùng một cửa sổ cho PF 1,753 ở 14 lệnh và PF 0,682 ở 178 lệnh).
- Phân vị KHÔNG phải cổng. Mỗi lần viết phân vị thì viết `null p50` ngay
  cạnh (null trung vị của desk này có PF 0,867 — dưới 1).
- Hai cửa sổ: A `2025-07-01..2025-10-01`, B `2025-04-01..2025-07-01`. Qua
  một cửa sổ là chưa có gì.
- Công bố **toàn bộ** lưới 9 cặp trong receipt, không chỉ ô thắng.
- Market `xauusd`, interval 15m (mặc định `config/default.toml`), spread
  0,28/lượt (không reprice), `max_hold_ms` 4 giờ.

## Thứ đăng ký này KHÔNG hứa

- Không đo được "trail có giữ được phần MFE" ở mức từng lệnh: engine ghi
  exit kind là `STOP` cho cả stop gốc và stop đã ratchet — không có nhãn
  `TRAIL` riêng. Nên `--exit-mix` chỉ cho thấy TARGET/TIMEOUT **chuyển
  thành** STOP, chứ không đếm trực tiếp số lần trail kích hoạt.
- `trail_stop()` không bao giờ TẠO stop và không chạm position tự quản
  (`self_managed`). Cơ chế nào tự quản thoát thì trail không tác động — nếu
  ba cơ chế được chọn thuộc loại đó, falsifier 3 bắn và trục chết vì không
  đo được, không vì đo ra số xấu.

---

## Ghi chú thêm 2026-10-06 (sau khi chạy lưới, KHÔNG viết lại dòng nào ở trên)

Lưới đã chạy xong và falsifier 1 + 2 đã bắn (chi tiết ở
`receipts/m4-trail-grid.md`). Trong lúc đọc lưới, một điều ngoài dự đoán lộ
ra và cần một phép đo phụ để đọc cho đúng:

Cột `activate_r = 2,0R` cho kết quả **y nguyên** `--trail=off` ở
`compression/bb-fade` và `intraday/donchian-breakout` trên cả hai cửa sổ,
nhưng ở `intraday/bb-fade` cửa sổ A thì `d=0,5/a=2,0` y nguyên off (396 lệnh)
còn `d=1,0/a=2,0` lại khác (413 lệnh). Nếu trail chỉ đổi cách THOÁT thì không
thể như vậy: distance rộng hơn phải ít ràng buộc hơn, không phải nhiều hơn.

Lời giải thích nghi là: `--mode=hypotheses` **chọn tham số theo từng fold**
(walk-forward, 4 fold) chứ không giữ tham số cố định, nên trail đổi cả kết
quả in-sample ⇒ đổi cả ô tham số mà fold chọn ⇒ đổi hẳn tập lệnh
out-of-sample. Phần đổi đó KHÔNG phải tác động của luật thoát.

Phép đo phụ, khai trước khi chạy: `--fixed` (tham số cố định cả cửa sổ, không
chọn gì) với `--trail=off` và với `--trail=0.5,0.33` (ô trail gặm mạnh nhất),
trên cả hai cửa sổ. **4 run × 3 row = 12 row thêm.**

Sổ đa phép thử cập nhật: khai 86 + 12 = **98 ô**. Phép đo phụ này là một
phép CHẨN ĐOÁN, không phải một lần chọn: nó không thể cứu được ô nào, vì
falsifier 1 và 2 đã bắn trên lưới đã khai trước.
