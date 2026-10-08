# Cỡ stop ĐƯA được những dòng đó qua cổng — nhưng chỉ vì ở họ này cỡ stop là một ĐƠN VỊ ĐO, và khi nó thành một cái stop thật thì chân PF sụp

**Ngày:** 2026-10-08. Nhánh `agent/stop-width`. Đăng ký trước:
`docs/decisions/2026-10-08-stop-width.md` (commit `9db70c1`, chỉ docs; ghi chú
thêm `6eca007`). Receipt: `receipts/stopwidth/`.

**Câu hỏi:** `agent/gate-legs` đo `E = Lbar × (PF − 1)` và tách hoàn hảo 87/87
dòng bị loại một chân có `Lbar < 0,250R`. `Lbar` là đại lượng của cỡ stop.
**Đổi cỡ stop có đưa những dòng đó qua cổng không?**

**Trả lời:** Có — **13/40 ô qua cổng trên CẢ HAI cửa sổ ở arm không-guards, và
đúng 1/40 ở arm có guards (arm duy nhất chủ cho phép)**. Nhưng cái đáng giữ
không phải con số đó. Nó là: ở họ `close/*` cái "stop" **chưa bao giờ là một
lệnh stop** — nó là mẫu số của `R` — nên ba phần tư cái "qua cổng" kia là
**khai lại đơn vị, không phải một cơ chế tốt hơn**; và ngay khi guards biến nó
thành một stop thật thì **chân PF sụp xuống dưới 1,200 trước khi chân expectancy
kịp lên +0,050R** ở 2 trong 3 nhãn.

---

## 1. Việc, một câu

Quét **cỡ stop** (`riskDailyRanges`, 10 giá trị từ 4,000 xuống 0,060 biên độ
ngày New York 20 ngày) trên 4 nhãn họ `close/<window>` × 2 cửa sổ đã đăng ký ×
2 arm guards = **200 ô**, và in `Lbar` cạnh PF/expectancy/drawdown/exit-mix ở
từng ô để hằng đẳng thức của cổng kiểm được từng dòng.

## 2. Cổng

**Guards OFF (fixed replay), qua cổng trên CẢ HAI cửa sổ: 13 / 40.**
Năm ô tốt nhất theo drawdown, với cái giá của chúng ghi ngay cạnh:

| ô | IS (xauduka 2018-06→2025-04) | OOS (xauusd 2025-04→2026-09) |
|---|---|---|
| `close/1615-2200` f=0,300 | PF 1,3040 E +0,0570R Lbar 0,2302 n 1355 **sụt 1.935,07 USD = 9,21%** | PF 1,3330 E +0,1100R Lbar 0,3273 n 284 **sụt 14,55 USD = 10,72%** |
| `close/1615-2200` f=0,250 | PF 1,3170 E +0,0680R Lbar 0,2750 n 1355 **sụt 2.381,70 USD = 9,89%** | PF 1,3240 E +0,1320R Lbar 0,3923 n 284 **sụt 20,05 USD = 13,67%** |
| `close/1615-1815` f=0,120 | PF 1,3050 E +0,0500R Lbar 0,1827 n 1355 **sụt 2.911,64 USD = 14,92%** | PF 1,2600 E +0,0800R Lbar 0,3000 n 284 **sụt 11,93 USD = 9,50%** |
| `close/1615-2000` f=0,180 | PF 1,2740 E +0,0560R Lbar 0,2261 n 1355 **sụt 4.186,52 USD = 20,36%** | PF 1,6180 E +0,1870R Lbar 0,3225 n 284 **sụt 12,70 USD = 7,77%** |
| `close/1615-2200` f=0,060 | PF 1,6160 E +0,2670R Lbar 1,0638 n 1355 sụt 16.016,60 USD = 8,91% | PF 1,2230 E +0,5330R n 284 **sụt 196,13 USD = 49,57%** |

Dòng cuối là ví dụ tại sao phải in drawdown: nó có `Lbar` cao nhất, `E` cao
nhất, **và một cú sụt 49,57% đường vốn trên cửa sổ xác nhận.** Nó **không phải
ứng viên.** Bốn dòng trên nó cũng không, vì lý do ở mục 3.

**Guards ON (arm duy nhất giao dịch được, phụ lục 5 mục D): đúng 1 / 40.**

    close/1615-2200 long, f = 0,250 bien do ngay NY (20 ngay), guards on
      IS   PF_usd 1,2060  E +0,0580R  Lbar 0,2851  n 1355
           sut 3.347,46 USD = 19,19% dinh von   | OPEN_LOSS_CAP 37/1355 = 2,7%
           notional cap da size down 772/1355 = 57,0% so lenh
      OOS  PF_usd 1,3250  E +0,1270R  Lbar 0,3921  n  284
           sut    20,53 USD = 14,05% dinh von   | OPEN_LOSS_CAP 21/284 = 7,4%
           notional cap da size down 24/284 = 8,5% so lenh

Nó qua cổng thật: hai chân và ≥40 lệnh trên **cả hai** cửa sổ, đếm tay.
**Cái giá là một cú sụt một-phần-năm đường vốn đã đóng lệnh**, và cổng không có
chữ nào về chuyện đó. Nó cũng **cưỡi trần notional**: 57% số lệnh IS bị
`cap_lots` thu nhỏ, và **cả hai chân cổng đều không thấy việc đó** (mục 4).

## 3. Falsifier — cái nào bắn, ở đâu

**F2 (tiền kiểm) BẮN, và nó là kết luận của job.** Đọc mã trước khi tiêu ô nào:
`session_hold` khai `Exits::Strategy`, và `check_exit` (`engine.rs:772-777`) mở
đầu bằng `if position.self_managed { return None }`. Nên cái stop
`riskDailyRanges` **không bao giờ được thi hành**. Số đo khẳng định, ở arm
không-guards:

- **Tập lệnh y hệt ở cả 10 cỡ stop**, từng nhãn: n = 1355 / 1355 / 1355 / 1375
  (IS) và 284 / 284 / 284 / 291 (OOS) không đổi một lệnh, và `--exit-mix` in
  cùng một dòng ở cả mười (`window closed 1355`, v.v.).
- `E` và `Lbar` tỉ lệ `1/f`: `E × f` và `Lbar × f` đi ngang xuống cả cột.
- ⇒ **Ở họ này, "nới stop" không đổi tín hiệu, không đổi lệnh, không đổi đồng
  USD nào. Nó chỉ đổi con số R chia cho cái gì.** Chân expectancy của cổng đo
  **mẫu số khai báo**, không đo cơ chế — và vượt được bằng cách khai lại đơn vị.
- Và **chiều của brief bị lật**: để `Lbar` to ra phải làm stop **HẸP** lại.
  f = 2,000 và f = 4,000 (hướng "nới rộng") cho `Lbar` **nhỏ hơn** hồ sơ.

**F2 TRƯỢT hai điều khoản số của chính nó, và cả hai lần cái trượt lại đáng
hơn:**

1. Khai "`E × f` không đổi quá **1%**". Đo được **−10,0%** trên dải f từ 4,000
   xuống 0,060 (`Lbar × f` = 0,02320 → 0,02088, `close/1615-1815` IS). **Tìm
   ra nguyên nhân:** `risk = |entry − stop|` không bằng `f × biên độ ngày`, vì
   `sizing_stop` lấy từ `bar.close` của nến tín hiệu còn `entry` đã bị
   `apply_costs` dịch đi. Khớp mô hình một tham số `risk = f·R̄ + δ` lên mười
   điểm: **R² = 0,99685**, `δ/R̄ = 0,00654`, và với `R̄ ≈ 21,1 USD` đo được thì
   **δ = 0,138 USD ≈ đúng một nửa spread 0,28 vòng.**
   ⇒ **Nửa spread rò vào đơn vị rủi ro.** Ở f = 0,060 nó là **10,9%** của một R,
   nên `R` ở đó được định nghĩa trên một stop rộng hơn 10,9% so với cái khai.
   Đếm, không sửa.
2. Khai "`PF` dao động dưới **0,050**". **Sai hẳn, và đây là con số đáng giữ
   nhất của job** — xem mục 4.

**F1 (của brief) KHÔNG bắn.** Có ô `Lbar ≥ 0,250R` **và** `PF ≥ 1,200` **và**
`n ≥ 40` trên cả hai cửa sổ, trong arm giao dịch được (`close/1615-2200`
f=0,250: Lbar 0,2851 / 0,3921). ⇒ **Hai chân cổng KHÔNG phải một đánh đổi cứng.**

Nhưng cấu trúc của F1 **bắn cục bộ, ở arm có guards, ở 2 trong 3 nhãn**:
`close/1615-1815` chạm `PF 1,1930` (IS, f=0,180) khi `E` mới tới +0,0290R, và
`close/1615-2000` chạm `PF 1,1310` (IS, f=0,180) khi `E` mới +0,0440R —
**hết PF trước khi tới +0,050R**. Chỉ nhãn dài nhất (tới 22:00) còn đủ PF.

**F3 KHÔNG bắn**: ô sống nằm ở arm **có** guards, không phải arm chủ đã cấm.

**F4 BẮN, và nó vạch ranh giới cơ chế.** Tỉ lệ `OPEN_LOSS_CAP` theo cỡ stop
(`close/1615-2200`, guards on):

    f      4,000   2,000   1,000   0,500   0,300   0,250   0,180   0,120   0,090   0,060
    IS      0,0%    0,0%    0,0%    0,2%    1,6%    2,7%    6,6%   15,7%   25,5%   45,5%
    OOS     0,0%    0,0%    0,4%    0,7%    6,0%    7,4%   15,5%   29,9%   41,5%   58,1%

⇒ Ở `f ≥ 1,000` — **tức ở mọi dòng hồ sơ đã công bố** — cái stop nổ **0–3 lần
trong 1.355 lệnh**: nó không phải một stop. Ở `f ≤ 0,120` nó là exit chính và
cơ chế **đã thành "stop ở 2f biên độ ngày"**, không còn là "giữ tới hết cửa sổ".
Những ô có `Lbar` thoải mái nhất **đúng là những ô đã vượt ranh giới đó.**
Ô sống (f=0,250) nằm ở 2,7% / 7,4% — vẫn là cơ chế đồng hồ.

**F5 KHÔNG bắn.** 0/200 ô in `max_drawdown_pct > 100%`, nên **không dòng nào
cháy** và khuyết điểm 14 không chạm tới hồ sơ này. Dòng gần nhất:
`close/1400-1630` f=0,060 IS guards-off, **98,44% đỉnh vốn** (lãi lỗ ròng
**−9.684,62 USD** trên sổ 10.000 USD ⇒ sổ còn **315 USD**) — cách mốc cháy
**1,56 điểm phần trăm**.

## 4. Con số đáng giữ nhất: hai cái PF trên CÙNG MỘT tập lệnh

Tập lệnh không đổi một lệnh nào. Chỉ mẫu số cỡ lệnh đổi. Khoảng chạy của hai
cách tính PF xuống mười cỡ stop:

| run / nhãn | `PF_usd` | dải | `PF_r` | dải |
|---|---|---|---|---|
| IS `close/1615-2200` | 1,2520 … **1,6160** | **0,3640** | 1,2467 … 1,2507 | **0,0040** |
| IS `close/1400-1630` (sổ lỗ) | **0,7530 … 0,5510** | 0,2020 | 0,7577 … 0,7661 | 0,0084 |
| IS `close/1615-2000` | 1,2470 … 1,3310 | 0,0840 | 1,2454 … 1,2523 | 0,0069 |
| OOS `close/1615-2000` | 1,5660 … 1,7120 | 0,1460 | 1,5800 … 1,5824 | 0,0024 |

⇒ `PF_usd` của cổng dịch tới **0,3640** còn `PF_r` trên **cùng những lệnh đó**
dịch **0,0040** — gấp **91 lần**. Và **dấu của cái dịch đi theo dấu của sổ**:
sổ lãi thì đòn bẩy cao làm `PF_usd` ĐẸP lên (1,252 → 1,616), sổ lỗ thì nó XẤU
đi (0,753 → 0,551). Đây là brief §6(b) ở một dạng mới, đo trên một trục duy
nhất với mọi thứ khác giữ nguyên, và nó định danh thẳng: **`PF_r` là thuộc tính
của phương pháp, `PF_usd` là thuộc tính của đòn bẩy.**

Hệ quả cụ thể cho cổng:

- **5 / 200 ô có `PF_usd` và `PF_r` ở HAI PHÍA vạch 1,200 của chính cổng.**
  (`close/1615-1815` f=0,180 IS: 1,1930 vs 1,2263; `close/1615-2200` f=0,180
  IS: 1,1530 vs 1,2024; f=0,120 IS: 1,1390 vs 1,2023; `close/1615-1815`
  f=0,060 OOS: 1,1750 vs 1,2512; `close/1615-2200` f=0,060 OOS: 1,1900 vs
  1,2371.)
- **Đọc cổng bằng `PF_r` thay vì `PF_usd` đổi số ô sống ở arm giao dịch được
  từ 1 thành 3 trên 40** (thêm `close/1615-2200` f=0,120 và f=0,180).
  ⇒ **Đơn vị của chân PF quyết định câu trả lời**, và hồ sơ chưa từng khai nó.
- Trần notional (`max_notional_pct_equity = 300`) **vô hình với cả hai chân**:
  ở ô sống nó thu nhỏ **772/1355 = 57%** số lệnh IS, mà `r = points / risk`
  không thấy `lots` nên `E` không nhúc nhích, và `PF` là một tỉ số nên cũng gần
  như không. **Cổng có thể qua trong một cấu hình mà guard sàn đã thu nhỏ sổ,
  và cổng không biết.**

## 5. Hằng đẳng thức — kiểm được từng dòng, và nó KHÔNG hỏng

Bản vá `lbar_line` (mục 4 đăng ký) in `Lbar = |avg_loss_r| × (1 − win_rate)` và
`PF_r = avg_win_r × win_rate / Lbar` **đo trực tiếp từ `Metrics`**, không suy
ra từ cặp PF/E — nên phép kiểm không vòng tròn.

    E = Lbar x (PF_r - 1)   dung tren 200/200 o
    |phan du| lon nhat tren toan bo 200 o = 0,00008 R

Nó **chỉ** đúng khi PF đọc bằng `PF_r`. Với `PF_usd` thì phần dư ở
`close/1615-2200` IS f=0,060 là `0,2670 − 1,0638×(1,6160−1) = −0,389R` —
**lớn gấp gần 8 lần cả vạch +0,050R của cổng.** Chỗ hằng đẳng thức "hỏng" chính
xác là chỗ `engine.rs:1070-1076` trộn hai đơn vị, đúng như phụ lục 6 mục I đã
đếm; không có chỗ hỏng nào khác.

## 6. Hiện vật cửa sổ thứ MƯỜI MỘT: cỡ stop (mục V phụ lục 6)

- **f cần để qua chân expectancy khác nhau giữa hai cửa sổ, và IS luôn đòi hẹp
  hơn 1,5–2,8 lần**: `close/1615-1815` IS f≤0,120 / OOS f≤0,180;
  `close/1615-2000` IS f≤0,180 / OOS f≤0,500; `close/1615-2200` IS f≤0,300 /
  OOS f≤0,500.
- **Dấu của `dPF_usd` theo chiều thu hẹp LẬT giữa hai cửa sổ.**
  `close/1615-1815`: IS 1,2710 → **1,3350** (tăng khi hẹp lại), OOS 1,3250 →
  **1,2400** (giảm). Cùng nhãn, cùng arm, cùng trục.
- Arm có guards: **1/40 ô qua cả hai cửa sổ**, trong khi **riêng OOS có 14/40**
  và **riêng IS có 1/40**. ⇒ 13 ô là ô một-nửa, và cửa sổ IS là cửa sổ chặn.
  (Arm không-guards: riêng OOS 18/40, riêng IS 13/40, cả hai 13/40.)

⇒ **Cỡ stop là hiện vật cửa sổ thứ mười một.** Hai khoảng f có giao nhau nên
vẫn tồn tại một f thoả cả hai — nhưng **"cỡ stop tối ưu" là một phát biểu về
cửa sổ, không về thị trường.**

## 7. Dòng `PF 2,065` — nó không phải một dòng của hai cửa sổ

Dòng khởi nguồn job (`close/1400-1630` PF 2,065 / Lbar 0,0272R / 45 lệnh) ở
`receipts/exitmix/G-B-file-2026-09-13-close-reopen-drift.txt` (commit
`9f557cd`), và header receipt đó ghi `--from=2025-04-01 --to=2025-07-01`,
**5.862 nến — một lát ba tháng**, guards ON.

Trên **hai cửa sổ đã đăng ký**, nhãn đó đo được:

    run                     PF_usd            PF_r              n
    IS  guards off          0,5510-0,7530     0,7577-0,7661     1375
    IS  guards on           0,6210-0,7040     0,6582-0,7083     1323
    OOS guards off          0,8550-0,9280     0,9123-0,9162      291
    OOS guards on           0,8790-1,0390     0,9230-1,0548      279
    OOS guards off (w-f)    0,8450-0,9480     0,9187-0,9199      236

**Chưa một ô nào trong 50 ô của nhãn đó đạt PF 1,200** — cao nhất là `PF_usd
1,0390` (OOS, guards on, f = 0,060), còn xa vạch. Không cỡ stop nào cứu
được, đúng như đăng ký đã khai trước khi quét. `PF 2,065` là **hiện vật cửa sổ
ba tháng**, không phải một dòng "profit factor gấp đôi" đang chờ cỡ stop.

## 8. Khuyết điểm đo thêm được — đếm, không sửa

1. **Nửa spread (0,138 USD trong 0,28 vòng) nằm trong đơn vị rủi ro.**
   `risk = f × biên độ ngày + δ`, R² = 0,99685, δ = 0,138 USD. Ở f = 0,060 nó
   là **10,9% của một R**. Mọi `R` của hồ sơ được định nghĩa trên một stop rộng
   hơn cái khai đúng nửa spread.
2. **Mọi dòng `close/*` trên `xauusd` chạy ở LOT TỐI THIỂU.** `xauusd` có
   `starting_equity_usd = 100,0` (tài khoản cent), nên `raw_lots` ở f ≥ 1,000
   rơi xuống dưới `min_lot = 0,01` và bị kẹp: f = 4,000 và f = 2,000 in
   **đồng USD y hệt nhau tới từng cent** (lãi ròng +3,19 USD, sụt 1,06 USD,
   `close/1615-1815`), lots trung bình 0,0101 ở cả hai. ⇒ **`PF_usd` của mọi
   dòng OOS đã công bố là PF của một sổ 0,01 lot, không phải của một sổ rủi ro
   1%**, và luật cỡ lệnh của engine chưa từng có hiệu lực ở đó.
3. **Trần notional vô hình với cổng** (mục 4).

## 9. Thứ KHÔNG đo được, và vì sao

- **`cost/R` không được mode này in ra.** Không có dòng `cost-matched null: …
  cost X% of R` trong `--mode=hypotheses`; chỉ có `spread paid: method X USD`.
  Nên con số dưới đây là **suy ra**, không phải in ra, và nói rõ là suy ra:
  từ lots trung bình của ô chứng (`f = 1,000`, IS, 4,9315 lot trên sổ ~10.400
  USD ở 1% rủi ro) ⇒ **biên độ ngày NY 20 ngày ≈ 21,1 USD** trên xauduka
  2018-06→2025-04; `cost/R = 0,28 / (f × 21,1)` = **1,33%** ở f=1,000,
  **5,31%** ở f=0,250, **22,1%** ở f=0,060. **Luôn kèm thước: "biên độ ngày New
  York 20 ngày", không phải ATR.**
- **Kênh "stop rộng thì chi phí/R nhỏ hơn" của brief KHÔNG tồn tại ở họ này**,
  và đó là số học chứ không phải phép đo thất bại: khi exit là đồng hồ,
  gross/lệnh tính bằng R **và** cost/lệnh tính bằng R đều tỉ lệ `1/f`, nên tỉ
  số của chúng bất biến. Kênh đó chỉ giúp khi exit là giá.
- **Excursion trong lệnh** không có trong `max_drawdown_*` (đường vốn đã đóng
  lệnh); `avg_mae` là field duy nhất thấy nó. Mọi con số sụt ở trên là **SÀN**.
- **Phân vị của bốn lần chạy `--fixed` KHÔNG công bố** (chỉ 20 draw, khai trước
  ở ghi chú đăng ký). Lần `--seeds=100` có `count match 1,00` ở mọi ô nhưng
  `long share` method 1,000 vs null 0,500 ở **mọi** ô — null tung xu trên một
  hàng chỉ-long, nên phân vị của nó mang drift của instrument, đúng như chính
  output tự cảnh báo. **Cổng là cổng.**
- **Ba họ còn lại** trong 57 dòng của `gate-legs` (`fx/us-long`,
  `crt-nocap/4h-mid`, `box/b2`, `pdhl/*`, `struct-80-f14`, …) **không đo** —
  job này khai trước là chọn một tập con, và họ `close/*` chiếm 36/87.
  Ở `box/b2` và `crt-nocap/4h-mid` cái stop **là** một stop thật, nên dự đoán
  F2 ở trên **không áp dụng** cho chúng và trục đó còn mở.

## 10. Sổ đa phép thử: khai vs xem

    khai (9db70c1):  4 nhan x 10 co stop = 40 gia thuyet
                     x 2 cua so x 2 arm = 160 o, 4 lan goi binary
    sua (6eca007):   + 1 lan chay walk-forward da phong truoc khi phat hien
                     --fixed la co dong lenh, GIU lai va bao => 200 o, 5 lan
    xem:             200 o, 5 lan. Khong them truc, khong them nhan,
                     khong them co stop sau khi thay so.

## 11. Khuyến nghị để đó, bằng số, không chỉnh cổng

Chủ máy quyết. Số để quyết:

1. **Chân expectancy của cổng, với một cơ chế exit-bằng-đồng-hồ, là một ngưỡng
   ĐÒN BẨY trá hình**, không phải một ngưỡng tín hiệu: nó buộc `Lbar ≥ 0,250R`,
   mà `Lbar` ở họ này là `1/f`. Nếu cổng muốn nói một điều về cơ chế thì chân đó
   phải đi kèm một ràng buộc drawdown, hoặc phải đóng vào một `f` cố định khai
   trước cho từng cơ chế.
2. **Chân PF phải khai đơn vị.** `PF_usd` và `PF_r` ở hai phía vạch 1,200 trên
   **5/200** ô, và đổi đơn vị đổi số ô sống từ **1 thành 3 trên 40**.
3. Nếu vẫn muốn một ứng viên: `close/1615-2200` long, f = 0,250 biên độ ngày NY
   20 ngày, guards on — hai chân + 40 lệnh trên hai cửa sổ, **nhưng sụt 19,19%
   / 14,05% đỉnh vốn, cưỡi trần notional ở 57% số lệnh IS, và cùng nhãn ở
   f = 0,180 trượt chân PF trên IS (1,1530)**. Một ô sát mép, không một họ.

## 12. Hiện vật

- Đăng ký: `docs/decisions/2026-10-08-stop-width.md`
- Batch (40 giả thuyết, trục duy nhất là `riskDailyRanges`):
  `docs/hypotheses/2026-10-08-stop-width.toml`
- Receipt thô, 5 lần chạy: `receipts/stopwidth/{is,oos}-fixed-guards-{off,on}.txt`,
  `receipts/stopwidth/oos-guards-off.txt`
- Bảng gộp 200 ô: `receipts/stopwidth/TABLE-all-200-cells.txt`
- Bảng kiểm tỉ lệ `1/f`: `receipts/stopwidth/TABLE-scaling-guards-off.txt`
- Bản vá duy nhất: `lbar_line` trong `crates/fd-backtest/src/bin/search.rs`,
  in ở `hypotheses` và `rescore`, 4 test mới; bộ test `--release` của bin
  `search` xanh **13/13**, trong đó hai test parity byte-identical với hồ sơ
  đã công bố vẫn xanh.
- Ô chứng `f = 1,000` tái lập **y hệt** hồ sơ:
  `receipts/drawdown/close-reopen-in-sample-fixed.txt` ghi `close/1630-1815`
  lãi ròng +871,64 USD / sụt 320,31 USD / 2,94%; lần chạy này in đúng ba con
  số đó. (Cửa sổ OOS nay có **33.926** nến tới 2026-09-17 thay vì 33.589 tới
  2026-09-11, nên n = **284** thay vì 282 — feed đã dài ra, không phải phép đo
  đổi.)
