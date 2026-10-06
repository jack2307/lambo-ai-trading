# Receipt — trục luật thoát: lưới trailing stop trên vàng

Ngày 2026-10-06. Nhánh `agent/m4`, worktree `/e/rust/fd-a4`.
Đăng ký trước: `docs/decisions/2026-10-06-trailing-exit.md` (commit riêng,
trước lệnh `search.exe` đầu tiên).

## Thiết lập, nguyên văn từ header receipt

    binary:   /e/rust/fd-wt-crt/target/release/search.exe (04/10 15:13, KHÔNG build lại)
    market:   xauusd 15m
    data:     E:/rust/flowdesk/data (bars from E:/rust/flowdesk/data\bars\XAUUSD-15m.parquet)
    spread:   0.28 per round trip
    max hold: 4 h (14400000 ms) from [trading] max_hold_ms, no per-market override
    news:     747 events (2010-01-08 -> 2027-12-08) from E:/rust/flowdesk/data\news\events.parquet
    news scope: USD (news_currencies)
    guards:   off (every number unguarded)
    null sides: coin - 50/50, carries no drift
    seeds:    200 matched null runs mỗi dòng; walk-forward 4 fold
    Cửa sổ A: --from=2025-07-01 --to=2025-10-01 (6044 bars)
    Cửa sổ B: --from=2025-04-01 --to=2025-07-01

Dòng `news:` in ra `E:/rust/flowdesk/data\news\events.parquet` — đúng nguồn
mà `--data=` khai. `data-sealed/` không được trỏ tới, không đọc, không đếm.

## Bước 1 — sàng lọc không trail (26 ô)

`--batch=gold-intraday --trail=off --exit-mix`, 13 dòng × 2 cửa sổ.
Receipt đầy đủ: `m4-baseline-notrail-A.txt`, `m4-baseline-notrail-B.txt`.

Luật chọn đã khai trước: >= 40 lệnh out-of-sample ở CẢ HAI cửa sổ, rồi
min(PF_A, PF_B) lớn nhất, lấy ba.

| cơ chế | A lệnh / PF | B lệnh / PF | min PF | vào lưới |
|---|---|---|---|---|
| compression/bb-fade | 54 / 1.153 | 75 / 1.320 | **1.153** | có |
| intraday/donchian-breakout | 199 / 1.119 | 155 / 1.023 | **1.023** | có |
| intraday/bb-fade | 396 / 0.828 | 439 / 0.778 | **0.778** | có |
| london-open/donchian-breakout | 48 / 0.765 | 40 / 0.835 | 0.765 | không (thứ tư) |
| ny-morning/donchian-breakout | 73 / 1.704 | 41 / 0.752 | 0.752 | không |
| intraday/ema-cross | 54 / 0.703 | 46 / 0.895 | 0.703 | không |
| intraday/rsi-reversion | 69 / 0.701 | 216 / 0.959 | 0.701 | không |
| expansion/donchian-breakout | 41 / 1.062 | **39** / 0.714 | null | loại: B < 40 lệnh |
| asia/rsi-reversion | 56 / 0.924 | **28** / 0.994 | null | loại: B < 40 lệnh |
| asia/bb-fade | **31** / 0.713 | 58 / 0.681 | null | loại: A < 40 lệnh |
| compression/rsi-reversion | **26** / 0.926 | **29** / 1.014 | null | loại: cả hai < 40 |
| ny-morning/ema-cross | **14** / 2.567 | **14** / 0.214 | null | loại: cả hai < 40 |
| expansion/ema-cross | **3** / 0.000 | **4** / 0.833 | null | loại: cả hai < 40 |

(`min PF` = null, không phải 0: với ô dưới sàn cỡ mẫu thì luật chọn không
tính được một con số, chứ không phải tính ra số không.)

`ny-morning/donchian-breakout` là dòng duy nhất engine gắn `SURVIVES` ở cửa
sổ A: 73 lệnh, PF 1.704, expect +0.303R, phân vị 97% — **null p50 0.944**.
Cùng luật đó ở cửa sổ B cho PF **0.752**, expect **−0.099R**, 41 lệnh. Nó
không vào lưới chính vì luật min-PF, và con số B là lý do luật đó đúng.

Batch file ba cơ chế: `docs/hypotheses/2026-10-06-trail-grid.toml`. Arm đối
chứng `--trail=off` trên batch file này tái hiện **đúng từng chữ số** ba dòng
của baseline (`m4-grid-A-trail-off.txt`, `m4-grid-B-trail-off.txt`) — bằng
chứng batch file là bản sao nguyên văn, không sửa tham số nào.

## Bước 2 — TOÀN BỘ lưới trail 3×3, cả hai cửa sổ

Lưới khai trước: `distance_r` thuộc {0,5; 1,0; 1,5} × `activate_r` thuộc
{0,33; 1,0; 2,0} = **9 cặp**, không phải 100. `activate_r` lấy từ phân bố
MFE của sổ `xau-stoch` (73 lệnh đã đóng, tiền thật): 81% chạm +0,33R, 11%
chạm +2,00R.

Công bố cả 9 cặp × 3 cơ chế × 2 cửa sổ, không chỉ ô thắng. Cổng
`PF >= 1,200` VÀ `expect >= +0,050R`, sàn cỡ mẫu 40 lệnh. Cột `qua cong`
ghi cửa sổ nào qua cả hai chân cổng; `khong` là không cửa sổ nào.
Ghi chú `(y nguyen off)` nghĩa là dòng đó trùng khít dòng `--trail=off` của
chính nó ở **cả hai** cửa sổ.

### `compression/bb-fade`

| trail d/a (R) | A lệnh | A PF | A expect | A null p50 | A pct | B lệnh | B PF | B expect | B null p50 | B pct | qua cong |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **off (đối chứng)** | 54 | 1.153 | +0.060 | 0.825 | 71% | 75 | 1.320 | +0.092 | 0.824 | 87% | B |
| 0.5/0.33 | 68 | 0.877 | -0.036 | 0.750 | 62% | 71 | 0.935 | -0.015 | 0.704 | 66% | khong |
| 0.5/1.0 | 22 | 0.977 | -0.013 | 0.832 | 60% | 75 | 1.283 | +0.083 | 0.763 | 87% | B |
| 0.5/2.0 *(y nguyen off)* | 54 | 1.153 | +0.060 | 0.825 | 71% | 75 | 1.320 | +0.092 | 0.824 | 87% | B |
| 1.0/0.33 | 55 | 0.726 | -0.114 | 0.755 | 47% | 67 | 1.094 | +0.025 | 0.711 | 80% | khong |
| 1.0/1.0 | 22 | 0.952 | -0.022 | 0.801 | 62% | 75 | 1.320 | +0.092 | 0.758 | 86% | B |
| 1.0/2.0 *(y nguyen off)* | 54 | 1.153 | +0.060 | 0.825 | 71% | 75 | 1.320 | +0.092 | 0.824 | 87% | B |
| 1.5/0.33 | 55 | 1.188 | +0.073 | 0.818 | 76% | 62 | 1.171 | +0.051 | 0.693 | 80% | khong |
| 1.5/1.0 | 34 | 1.178 | +0.079 | 0.782 | 74% | 75 | 1.320 | +0.092 | 0.784 | 84% | B |
| 1.5/2.0 *(y nguyen off)* | 54 | 1.153 | +0.060 | 0.825 | 71% | 75 | 1.320 | +0.092 | 0.824 | 87% | B |

### `intraday/donchian-breakout`

| trail d/a (R) | A lệnh | A PF | A expect | A null p50 | A pct | B lệnh | B PF | B expect | B null p50 | B pct | qua cong |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **off (đối chứng)** | 199 | 1.119 | +0.062 | 0.908 | 85% | 155 | 1.023 | +0.017 | 0.978 | 61% | khong |
| 0.5/0.33 | 220 | 1.093 | +0.022 | 0.890 | 90% | 205 | 1.153 | +0.047 | 0.941 | 88% | khong |
| 0.5/1.0 | 190 | 1.137 | +0.037 | 0.906 | 90% | 169 | 1.007 | +0.008 | 0.958 | 61% | khong |
| 0.5/2.0 *(y nguyen off)* | 199 | 1.119 | +0.062 | 0.908 | 85% | 155 | 1.023 | +0.017 | 0.978 | 61% | khong |
| 1.0/0.33 | 172 | 1.097 | +0.027 | 0.880 | 88% | 148 | 1.106 | +0.029 | 0.978 | 72% | khong |
| 1.0/1.0 | 156 | 1.169 | +0.049 | 0.899 | 91% | 165 | 1.012 | +0.007 | 0.976 | 58% | khong |
| 1.0/2.0 *(y nguyen off)* | 199 | 1.119 | +0.062 | 0.908 | 85% | 155 | 1.023 | +0.017 | 0.978 | 61% | khong |
| 1.5/0.33 | 156 | 1.123 | +0.035 | 0.898 | 88% | 177 | 1.091 | +0.049 | 0.987 | 68% | khong |
| 1.5/1.0 | 174 | 1.146 | +0.048 | 0.898 | 87% | 177 | 1.116 | +0.062 | 0.987 | 78% | khong |
| 1.5/2.0 *(y nguyen off)* | 199 | 1.119 | +0.062 | 0.908 | 85% | 155 | 1.023 | +0.017 | 0.978 | 61% | khong |

### `intraday/bb-fade`

| trail d/a (R) | A lệnh | A PF | A expect | A null p50 | A pct | B lệnh | B PF | B expect | B null p50 | B pct | qua cong |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **off (đối chứng)** | 396 | 0.828 | -0.081 | 0.910 | 32% | 439 | 0.778 | -0.104 | 0.955 | 10% | khong |
| 0.5/0.33 | 305 | 0.702 | -0.103 | 0.857 | 10% | 460 | 0.664 | -0.118 | 0.907 | 4% | khong |
| 0.5/1.0 | 423 | 0.902 | -0.040 | 0.877 | 60% | 407 | 0.725 | -0.126 | 0.929 | 4% | khong |
| 0.5/2.0 | 396 | 0.828 | -0.081 | 0.910 | 32% | 439 | 0.768 | -0.108 | 0.955 | 6% | khong |
| 1.0/0.33 | 363 | 0.781 | -0.088 | 0.859 | 28% | 445 | 0.733 | -0.109 | 0.917 | 8% | khong |
| 1.0/1.0 | 423 | 0.907 | -0.037 | 0.870 | 62% | 407 | 0.701 | -0.135 | 0.950 | 2% | khong |
| 1.0/2.0 | 413 | 0.893 | -0.047 | 0.910 | 46% | 439 | 0.778 | -0.104 | 0.955 | 10% | khong |
| 1.5/0.33 | 339 | 0.815 | -0.082 | 0.867 | 36% | 404 | 0.700 | -0.137 | 0.945 | 2% | khong |
| 1.5/1.0 | 417 | 0.921 | -0.034 | 0.870 | 62% | 404 | 0.695 | -0.142 | 0.952 | 2% | khong |
| 1.5/2.0 | 413 | 0.891 | -0.048 | 0.910 | 45% | 439 | 0.778 | -0.104 | 0.955 | 10% | khong |

Tổng hợp 54 dòng trail so với dòng `--trail=off` của chính nó:

    17 / 54 dòng trùng khít đối chứng (trail trơ), 37 / 54 dòng đổi số
    trong 37 dòng đổi: 11 dòng expectancy TĂNG, 26 dòng KHÔNG tăng
    thay đổi trung bình trên 37 dòng đó: expectancy −0,0175R, PF −0,0457

## Falsifier — đã bắn

**Falsifier 1 (không qua cổng trên cả hai cửa sổ): BẮN.**
`0 / 27` ô trail (9 cặp × 3 cơ chế) qua cổng trên cả hai cửa sổ. Trên một
cửa sổ: 5 ô qua ở cửa sổ B, tất cả nằm ở `compression/bb-fade`, và **bốn
trong năm ô đó trùng khít `--trail=off`** (ba ô cột `a=2,0R`, cộng
`1,0/1,0` và `1,5/1,0` vốn cũng cho đúng 75 lệnh / PF 1,320 / +0,092R như
off) — tức không phải tác động của trail.

Ô duy nhất qua cổng mà trail thực sự đổi lệnh: `compression/bb-fade`
`d=0,5 / a=1,0` ở cửa sổ B — 75 lệnh, PF 1,283, +0,083R, phân vị 87%
(**null p50 0,763**). Cùng ô đó ở cửa sổ A: **22 lệnh, PF 0,977, −0,013R**
— dưới sàn cỡ mẫu 40 và trượt cả hai chân cổng.

**Falsifier 2 (ô thắng A không phải ô thắng B): BẮN.**
Ô trail có PF lớn nhất mà vẫn đạt >= 40 lệnh và trail thực sự gặm:

- cửa sổ A: `compression/bb-fade` `d=1,5 / a=0,33` — 55 lệnh, PF **1,188**,
  +0,073R (thiếu 0,012 PF để qua cổng).
- cửa sổ B: `compression/bb-fade` `d=0,5 / a=1,0` — 75 lệnh, PF **1,283**,
  +0,083R.

Hai ô khác nhau ở **cả hai** tham số. Chéo nhau thì sụp: ô thắng của A cho
PF 1,171 / +0,051R ở B (trượt cổng), ô thắng của B cho PF 0,977 / −0,013R /
22 lệnh ở A. Trail không mang sang giữa hai cửa sổ.

**Falsifier 3 (trail gần như không kích hoạt): BẮN MỘT PHẦN — đúng ở cột
`activate_r = 2,0R`.** 17 trong 54 dòng trail trùng khít dòng `--trail=off`
của chính nó, và **15 trong 17 dòng đó nằm ở cột `a=2,0R`**. Ở `a=0,33R` và
`a=1,0R` trail đổi hẳn cách thoát (bước 3). Nên lưới KHÔNG phải đo lại luật
cũ — trừ một phần ba lưới mà giờ đã biết là trơ ở cơ chế này.

Lý do trơ đọc được từ source: `trail_stop()` chỉ di chuyển stop khi
`gained >= activate_r * risk` VÀ stop mới tốt hơn stop hiện tại. Với target
cố định dưới 2,0R thì lệnh đã chốt TARGET trước khi MFE kịp tới 2,0R, nên
`a=2,0R` không bao giờ có cơ hội bắn.

## Bước 3 — phép đo phụ `--fixed` (12 ô, khai trong ghi chú thêm cùng ngày)

Lý do phải làm: cột `a=2,0R` không đơn điệu. Ở `intraday/bb-fade` cửa sổ A,
`d=0,5/a=2,0` trùng khít off (396 lệnh) nhưng `d=1,0/a=2,0` cho 413 lệnh —
distance rộng hơn không thể ràng buộc hơn nếu trail chỉ đổi cách thoát.
Nghi phạm: `--mode=hypotheses` **chọn tham số theo từng fold** (walk-forward
4 fold), nên trail đổi cả kết quả in-sample, đổi ô tham số fold chọn, và
đổi hẳn tập lệnh out-of-sample. `--fixed` bỏ bước chọn đó.

`--fixed`, `--trail=off` so với `--trail=0.5,0.33` — ô gặm mạnh nhất
(`m4-fixed-A-trail-off.txt`, `m4-fixed-A-trail-0.5-0.33.txt`,
`m4-fixed-B-trail-off.txt`, `m4-fixed-B-trail-0.5-0.33.txt`):

| cơ chế | cửa sổ | lệnh off -> trail | PF off -> trail | expect off -> trail |
|---|---|---|---|---|
| compression/bb-fade | A | 39 -> 41 | 0.654 -> 0.875 | −0.193R -> −0.036R |
| intraday/donchian-breakout | A | 271 -> 319 | **1.117 -> 0.925** | **+0.054R -> −0.019R** |
| intraday/bb-fade | A | 369 -> 403 | 0.813 -> 0.817 | −0.106R -> −0.065R |
| compression/bb-fade | B | 35 -> 38 | **1.049 -> 0.814** | +0.027R -> −0.046R |
| intraday/donchian-breakout | B | 280 -> 326 | **1.034 -> 0.977** | +0.017R -> −0.005R |
| intraday/bb-fade | B | 385 -> 431 | 0.876 -> 0.763 | −0.059R -> −0.076R |

Bốn trong sáu cặp PF tụt; **không cặp nào qua cổng**; cả ba cơ chế đều rơi
về expectancy âm khi bật trail, kể cả hai cơ chế đang dương khi tắt.

Và `--exit-mix` nói trail đổi thật, đổi mạnh —
`intraday/donchian-breakout`, tham số cố định, cửa sổ A:

    off:         STOP  65, TARGET 49, TIMEOUT 65, flat window 23,
                 lost midline 30, reclaimed midline 38;  mean hold 128.4 min
    0.5R/0.33R:  STOP 248, TARGET 17, TIMEOUT  4, flat window 16,
                 lost midline 11, reclaimed midline 22;  mean hold  63.1 min

Đó là câu trả lời trực tiếp cho câu hỏi MFE. Giữ lại đoạn +0,33R biến 49
TARGET thành 17, cắt thời gian giữ trung bình một nửa, và đổi **+0,054R
thành −0,019R**. Phần đuôi bị cắt đắt hơn phần quay lại giữ được. Dự đoán
viết trong đăng ký trước khi chạy ("giữ phần đã đi đúng sẽ không bù được
phần đuôi bị cắt") đúng ở dấu.

Lưu ý `--fixed` đổi cả cỡ mẫu (`compression/bb-fade` cửa sổ A: 54 lệnh ở
chế độ chọn-theo-fold xuống 39 lệnh ở chế độ cố định), nên bảng này KHÔNG so
được với bảng lưới ở trên. Nó chỉ dùng để so off-vs-trail trong cùng một
chế độ.

## Sổ đa phép thử

| phần | khai | đã xem |
|---|---|---|
| sàng lọc không trail, 2 cửa sổ × 13 dòng | 26 | **26** |
| arm đối chứng `--trail=off` trên batch file, 2 cửa sổ × 3 dòng | 6 | **6** |
| lưới trail, 9 cặp × 3 cơ chế × 2 cửa sổ | 54 | **54** |
| phụ `--fixed`, 4 run × 3 dòng (khai trong ghi chú thêm cùng ngày) | 12 | **12** |
| **tổng** | **98** | **98** |

Khai 98, xem 98. Không nâng lưới sau khi thấy kết quả; không thêm cặp
(distance, activate) nào ngoài 9 cặp đã khai; không mở rộng số cơ chế quá 3.
22 run cho lưới + 4 run cho phép phụ = 26 run, mỗi run ~1,4 s.

## Thứ KHÔNG đo được, và vì sao

1. **Số lần trail kích hoạt.** Engine ghi `ExitKind::Stop` thành nhãn
   `"STOP"` cho cả stop gốc lẫn stop đã ratchet — không có nhãn `TRAIL`.
   `--exit-mix` chỉ cho thấy TARGET/TIMEOUT **chuyển thành** STOP, không đếm
   trực tiếp số lần ratchet. Đếm được thì phải sửa `engine.rs`, và brief §1
   cấm build.
2. **Trail trên chính sổ `xau-stoch`.** Phân bố MFE sinh ra giả thuyết này
   đến từ `xau-stoch` (73 lệnh tiền thật), nhưng `xau-stoch` không nằm trong
   batch `gold-intraday`, nên lưới chạy trên ba cơ chế khác. Giả thuyết được
   đo trên cơ chế đại diện, không trên sổ đã sinh ra nó. Đây là khoảng trống
   lớn nhất của phép đo này.
3. **Tách trail khỏi trần giữ 4 giờ.** Trail làm TIMEOUT tụt từ 65 xuống 4 ở
   `intraday/donchian-breakout`, nên hai luật thoát trộn vào nhau: không
   tách được "trail giữ được lợi nhuận" khỏi "trail đóng trước khi trần 4
   giờ kịp đóng". Phần cải thiện của `compression/bb-fade` cửa sổ A
   (0.654 -> 0.875) có thể là tác động này, không phải tác động trail.
4. **Trail trên cơ chế tự quản thoát.** `trail_stop()` trả `false` ngay khi
   `position.self_managed`, và không bao giờ TẠO stop cho lệnh không có stop.
   Ba cơ chế được chọn đều dùng stop/target của engine nên không rơi vào
   trường hợp này — nhưng kết luận không nói được gì về cơ chế tự quản.
5. **Trail trên thị trường khác xauusd.** Chỉ đo vàng 15m. Chi phí theo R
   của BTC (5,16% ngày thường, 9,96% cuối tuần) khác hẳn vàng (1,06%), và
   một luật thoát sớm hơn trả chi phí nhiều lần hơn, nên kết luận này không
   mang sang BTC được mà không đo lại.
6. **Trail lồng với luật vào khác.** Lưới chỉ đổi cách RA. Không đo được
   "trail + target rộng hơn", vì `--trail=` không kèm cờ nào đổi riêng
   target, và sửa `riskReward` là đổi cơ chế, không phải đổi luật thoát.

## Kết luận

Trục luật thoát, đo bằng lưới trailing stop 9 cặp trên ba cơ chế gần cổng
nhất, cả hai cửa sổ: **0 ô qua cổng trên cả hai cửa sổ.** Falsifier 1 bắn,
falsifier 2 bắn, falsifier 3 bắn một phần (một phần ba lưới trơ ở cơ chế
này). **Trục này chết.**

Con số đáng giữ nhất: `intraday/donchian-breakout`, tham số cố định, cửa sổ
A, trail 0,5R kích hoạt từ +0,33R đổi expectancy từ **+0,054R thành
−0,019R**, trong khi TARGET tụt 49 -> 17 và thời gian giữ trung bình
128,4 -> 63,1 phút. Trail CÓ đổi cách lệnh đóng — đổi rất mạnh, và đổi theo
chiều xấu.

Đĩa: `df -h /e` trước và sau đều **32 GB khả dụng**. Không chạy
`cargo build`, `cargo test`, `cargo run`. Không ghi gì vào
`/e/rust/flowdesk`.
