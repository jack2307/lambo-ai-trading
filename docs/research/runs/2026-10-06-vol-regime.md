# Điều kiện hoá theo chế độ biến động — ĐO XONG, trục chết

Ngày: 06/10/2026. Nhánh `agent/m8`. Đăng ký trước: `docs/decisions/2026-10-06-vol-regime.md`
(commit f4eb7fd, commit chỉ-docs, trước lệnh `search.exe` đầu tiên).

Receipt: `receipts/2026-10-06-vol-regime-A.txt`, `receipts/2026-10-06-vol-regime-B.txt`.
Batch: `receipts/vol-regime-batch.toml` (36 hàng).
Binary: `/e/rust/fd-wt-crt/target/release/search.exe` (04/10 15:13). Không build gì.

Nguồn mà receipt in ra, nguyên văn:

    data:  E:/rust/flowdesk/data (bars from E:/rust/flowdesk/data\bars\XAUUSD-15m.parquet)
    news:  747 events (2010-01-08 → 2027-12-08) from E:/rust/flowdesk/data\news\events.parquet

Không chạm `data-sealed/`. Cửa sổ A giữ 6044 / 100586 nến; cửa sổ B giữ 5862 / 100586.
Spread 0,28 USD một lượt khứ hồi; swap 0,00 cả hai chiều (cửa flat 16:30–18:15 làm
đúng việc của nó trên cả 72 ô). `--fixed`, 200 lần chạy null khớp mỗi hàng, null
`coin` 50/50.

## Kết quả: 0 ô qua cổng trên cả hai cửa sổ

54 ô có dải (9 cơ chế × 3 dải × 2 cửa sổ). Qua cổng (PF >= 1,200 **và**
expectancy >= +0,050R) với >= 40 lệnh:

| cửa sổ | ô dải qua cổng, >= 40 lệnh |
|---|---|
| A (07–09/2025) | `th/m3` 140 lệnh PF 1,288 exp +0,125R (null p50 0,922) · `th/m6` 46 lệnh PF 2,748 exp +0,563R (null p50 0,936) · `gian/m6` 50 lệnh PF 1,221 exp +0,087R (null p50 0,887) |
| B (04–06/2025) | `co/m3` 100 lệnh PF 1,236 exp +0,099R (null p50 1,017) |

Giao của hai cột: **rỗng**. Không một cặp (dải, cơ chế) nào qua cổng với >= 40
lệnh trên cả hai cửa sổ. **0 của 54.**

## Falsifier đã bắn — cả hai nhánh

**Nhánh (1)** — "không dải nào qua cổng với >= 40 lệnh trên cả hai cửa sổ": bắn.
Bảng trên là chỗ nó bắn.

**Nhánh (2)** — "dải thắng ở A khác dải thắng ở B": bắn ở `m3`
(`donchian-breakout`, weekdays + flat, không cửa giờ) — cơ chế duy nhất có ô dải
qua cổng ở **cả hai** cửa sổ:

    cua so A:  co 1,129 (95 lenh)   th 1,288 (140 lenh)   gian 1,016 (64 lenh)  -> thang: th
    cua so B:  co 1,236 (100 lenh)  th 0,979 (166 lenh)   gian 0,899 (50 lenh)  -> thang: co

Dải thắng dịch từ **bình thường** sang **co**, và dải thắng của cửa sổ A rơi
xuống PF 0,979 ở cửa sổ B. Cả ba dải của `m3` đều có >= 50 lệnh ở cả hai cửa sổ,
nên đây **không** phải lỗi cỡ mẫu — đây là dải thắng không mang sang được.

Dải thắng giống nhau ở 4 trong 9 cơ chế (`m1` th, `m6` th, `m7` th, `m9` co),
nhưng **cả bốn đều lỗ tiền ở ít nhất một cửa sổ**: `m1` A PF 0,932; `m6` B PF
0,815; `m7` PF 0,997 / 0,962; `m9` PF 0,661 / 0,643. Dải ổn định mà không có edge.

## Giả thuyết của tôi sai, và sai theo hướng ngược lại

Tôi dự đoán cơ chế theo xu hướng lên PF ở dải **giãn**. Đo được: trong 5 cơ chế
xu hướng/phá vỡ (`m1 m3 m5 m6 m7`) × 2 cửa sổ = 10 ô, dải **giãn** là dải có PF
**thấp nhất** ở **8 trong 10** ô. Hai ô ngoại lệ (`m5/B` 6 lệnh, `m6/B` 24 lệnh)
đều dưới 40 lệnh.

Ở 3 ô mà dải giãn tự nó có >= 40 lệnh và cơ chế là phá vỡ: `m3/A` PF 1,016 (64
lệnh), `m3/B` PF 0,899 (50 lệnh), `m6/A` PF 1,221 (50 lệnh). Chỉ một trong ba qua
cổng, và nó không mang sang (`m6/B` chỉ còn 24 lệnh, PF 1,145).

Nửa còn lại của giả thuyết — đảo chiều cần dải **co** — tự mâu thuẫn **giữa hai
cơ chế cùng lớp**:

- `rsi-reversion`: dải co nâng PF trên pooled ở **cả hai** cửa sổ (A 0,758 → 1,085; B 0,870 → 1,593), đúng hướng dự đoán. Nhưng hai ô co đó có **23** và **30** lệnh. Dưới 40 ⇒ không kết luận.
- `bb-fade`: dải co là dải **tệ nhất** ở `m9/A` (0,661 / 58 lệnh) và ở `m4/A` (0,684 / 102 lệnh); ở `m9/B` nó là dải tệ thứ hai (0,643 / 52 lệnh, dưới nó là `th` 0,608 / 65 lệnh). Bốn ô co của `bb-fade` đều dưới PF 1,0 với 52–111 lệnh. Ngược hướng dự đoán, và trên mẫu đủ để nói.

Nên: cơ chế duy nhất ủng hộ nửa "co" là cơ chế không đủ mẫu để nói, và cơ chế đủ
mẫu thì bác bỏ nó.

## Chia nhỏ mẫu — cái giá, đo bằng số

- **29 trong 54** ô dải (53,7%) dưới 40 lệnh. Dự đoán phụ đã khai ("phần lớn ô dải sẽ dưới 40 lệnh") đúng về dấu, hơi quá về độ: 53,7% chứ không phải "phần lớn" theo nghĩa áp đảo.
- 13 của 27 ô dải ở cửa sổ A và 12 của 27 ở cửa sổ B đạt >= 40 lệnh.
- Hai ô minh hoạ chính xác cảnh báo của brief, cả hai đều từ quét này: `gian/m8` PF **3,434** trên **5 lệnh** (cửa sổ A) và `gian/m9` PF **2,343** trên **6 lệnh**. Cùng cơ chế `rsi-reversion` ở `m8` trên pooled 25 lệnh: PF 0,994. Không ô nào trong hai ô đó được tính vào kết luận.
- 12 của 17 ô qua cổng trong cả 72 ô có **dưới** 40 lệnh. Nếu đọc cổng mà bỏ cỡ mẫu thì quét này "tìm ra" 17 ô; đọc kèm cỡ mẫu thì còn 5, và 0 mang sang.

## Ba dải KHÔNG phải một phân hoạch của tập lệnh pooled — và tôi đã khai sai điều này

Đăng ký trước nói: tổng số lệnh ba dải <= số lệnh arm gộp, phần thiếu là lúc
ATR100 khởi động. **Sai.** Đo được:

| cơ chế | A pooled vs tổng ba dải | B pooled vs tổng ba dải |
|---|---|---|
| m1 ema-cross | 99 vs 98 (−1) | 92 vs 91 (−1) |
| m2 rsi-reversion | 136 vs 138 (**+2**) | 144 vs 148 (**+4**) |
| m3 donchian-breakout | 271 vs 299 (**+28**) | 280 vs 316 (**+36**) |
| m4 bb-fade | 369 vs 373 (**+4**) | 385 vs 388 (**+3**) |
| m5 ema-cross NY | 34 vs 34 (0) | 18 vs 18 (0) |
| m6 donchian NY | 85 vs 97 (**+12**) | 72 vs 75 (**+3**) |
| m7 donchian London | 61 vs 73 (**+12**) | 60 vs 68 (**+8**) |
| m8 rsi Asia | 25 vs 25 (0) | 45 vs 47 (**+2**) |
| m9 bb-fade Asia | 100 vs 100 (0) | 126 vs 129 (**+3**) |

Nguyên nhân, đọc từ source: engine giữ **một** vị thế
(`crates/fd-backtest/src/engine.rs:490` — `Intent::Enter { .. } if position.is_none()`).
Chặn một lệnh vào vì dải không khớp làm sổ **rỗng** ở nến sau, nên một tín hiệu
muộn hơn — tín hiệu mà arm gộp đã bỏ qua vì đang giữ lệnh cũ — được vào. Dải
không lọc ra một tập con của lệnh pooled; nó tạo ra một **lộ trình khác**.

Hệ quả cho trục: dạng mạnh của giả thuyết ("PF pooled là trung bình có trọng số
của PF các dải, hai nửa triệt tiêu nhau") **không đo được bằng dụng cụ này**.
Cái đo được là dạng yếu: PF của cùng luật khi chỉ vào trong một chế độ.

## Phân vị — đọc kèm null p50, như luật yêu cầu

- Trung vị `null p50` trên cả 72 ô: **0,920**. 68 của 72 ô có `null p50 < 1,000`. Nên "phân vị cao" ở đây nghĩa là "lỗ ít hơn vào lệnh ngẫu nhiên cùng chi phí, cùng số lệnh, cùng tỉ lệ long", không phải "có lãi".
- Chỉ **2 của 71** ô (2,8%) đạt phân vị >= 95 (một ô in `null` thay vì số vì mọi seed của null đồng ý). So với 20,6% của chương trình khai 6 mà xem 247. Khai dải trước là cái giữ con số này thấp.
- Ô đạt phân vị cao nhất, `th/m3` ở cửa sổ A (94%), tự receipt đã đánh dấu hai lần là phân vị **không khớp**: `long share 0,607` ngoài dải 40–60% ⇒ phân vị này mang drift của vàng; `cost match 0,70` ⇒ control không trả đúng chi phí mà method trả. Cổng là chân quyết định, và cổng của ô này rơi xuống PF 0,979 ở cửa sổ B.

## Con số đáng giữ nhất

`th/m6` — `donchian-breakout`, weekdays + flat 16:30–18:15 + NY 08:00–12:00, dải
biến động **bình thường** (ATR14/ATR100 trong 0,9–1,2):

    cua so A (07-09/2025):  46 lenh   PF 2,748   exp +0,563R   null p50 0,936   phan vi 100%   SURVIVES
    cua so B (04-06/2025):  40 lenh   PF 0,815   exp -0,082R   null p50 0,987   phan vi  31%   fail

Cùng một luật, cùng một dải, **cả hai cửa sổ đều >= 40 lệnh**, PF rơi từ 2,748
xuống 0,815. Đây là ô duy nhất trong 72 được engine gán nhãn `SURVIVES` ở một
dải — và nó không mang sang.

Ô `SURVIVES` còn lại là `gop/m6` (arm **gộp**, không phải dải): A PF 1,875 / 85
lệnh / phân vị 99% → B PF 0,889 / 72 lệnh. Cùng một chiều rơi.

## Những gì KHÔNG đo được

1. **Arm `volabs:14`** (biến động tuyệt đối, ATR14/close). Đăng ký trước khai điều kiện: chỉ chạy nếu một dải `vol:14/100` qua cổng ở **cả hai** cửa sổ với >= 40 lệnh. Điều kiện **không** thoả (0 của 54), nên arm này **không chạy**. Không phải 0, là **không đo**.
2. **Dạng mạnh của giả thuyết triệt tiêu**: xem mục phân hoạch ở trên. Engine một-vị-thế làm cho ba dải không phải một phân hoạch của tập lệnh pooled (lệch tới +36 lệnh), nên PF pooled không phân rã được thành đóng góp theo dải.
3. **`m5` (`ema-cross` + NY 08:00–12:00) và `m8` (`rsi-reversion` + Asia 19:00–02:00) ở mức dải**: 0 của 3 dải đạt 40 lệnh ở cả hai cửa sổ (`m5`: 2/17/15 và 2/10/6 lệnh; `m8`: 12/8/5 và 15/24/8). Hai cơ chế này là **không đo được** trên trục này, không phải "không có edge ở dải nào".
4. **Biên dải khác 0,9 / 1,2**: khai trước đúng ba dải này và tôi không quét biên. Một biên khác có thể cho con số khác; đó sẽ là một "look" mới và phải khai lại.
5. **Khung thời gian khác 15m**: cả quét ở 15m (mặc định `config/default.toml`). Chia nhỏ mẫu ở 1H/4H sẽ còn tệ hơn, nhưng tôi không đo.
6. **Trần giữ 4 giờ**: `--exit-mix` bật trên cả hai lần chạy. Ở các hàng tôi kết luận trên, TIMEOUT là **15–22%** số lệnh, không phải thiểu số bỏ qua được nhưng cũng không phải 87% như ở `struct-80`:

       gop/m1  A: STOP 59, TARGET 19, TIMEOUT 10, flat window 9, opposite cross 2; mean hold 133,0 min
       th/m3   A: STOP 31, TARGET 30, TIMEOUT 31, flat window 13, mat midline 16, lay lai midline 19; mean hold 133,9 min   (TIMEOUT 31/140 = 22%)
       th/m6   A: STOP 11, TARGET 19, TIMEOUT  9, mat midline 4, lay lai midline 3; mean hold 114,5 min   (9/46 = 20%)
       th/m6   B: STOP 13, TARGET  7, TIMEOUT  6, mat midline 7, lay lai midline 7; mean hold 111,8 min   (6/40 = 15%)

   Nên trần 4h **không** âm thầm giết trục này như nó đã giết `tsmom`, nhưng nó
   **đang** đóng một phần năm số lệnh của các hàng mạnh nhất, và tôi không biết
   các lệnh đó sẽ kết thúc ra sao nếu trần cao hơn. Tôi không kiểm từng hàng
   trong 72.

## Sổ đa phép thử

    Da khai:  72 o (9 co che x (3 dai + 1 arm gop) x 2 cua so)
    Da xem:   72 o — 36 hang x 2 lan goi search.exe
    Lech:     0

Hai lệnh `search.exe`, không lệnh thử nghiệm nào trước đó, không hàng nào thêm
sau khi thấy kết quả, không biên dải nào dịch. Arm `volabs` đã khai điều kiện và
điều kiện không thoả, nên nó không tính là một "look" — nó chưa bao giờ chạy.

## Kết luận

**Trục m8 chết.** Điều kiện hoá theo chế độ biến động (ATR14/ATR100, ba dải khai
trước) không mang sang được giữa hai cửa sổ ba tháng trên XAUUSD 15m: 0 của 54 ô
dải qua cổng trên cả hai cửa sổ, và cơ chế duy nhất có ô dải qua cổng ở cả hai
cửa sổ đổi dải thắng từ "bình thường" sang "co" với cỡ mẫu >= 50 lệnh ở cả ba
dải, tức là không phải lỗi cỡ mẫu.

Bằng chứng ủng hộ giả thuyết thay thế (cơ chế lỗ đều ở mọi chế độ, tức đang trả
spread): `bb-fade` ở `m4` lỗ ở **cả bốn** arm ở **cả hai** cửa sổ — A 0,813
(gộp, 369 lệnh) / 0,684 (co, 102) / 0,878 (bth, 186) / 0,735 (giãn, 85); B 0,876
(gộp, 385) / 0,851 (co, 111) / 0,796 (bth, 212) / 1,016 (giãn, 65). Tám ô, bảy
dưới PF 1,0, mẫu từ 65 đến 385 lệnh. Không có nửa nào để lộ ra.
