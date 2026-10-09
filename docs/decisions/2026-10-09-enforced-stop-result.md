# KẾT QUẢ — 09/10/2026: ở loại stop THI HÀNH, cỡ stop đổi tập thoát thật, nhưng hai chân cổng kéo NGƯỢC nhau dọc trục đó

Đăng ký: `docs/decisions/2026-10-09-enforced-stop.md` (commit `4b961de`, chỉ docs).
Binary: `/e/rust/fd-stop-width/target-sw/release/search.exe` (có `lbar_line`).
**Không build, không sửa một dòng `crates/`.**
Receipt: `receipts/enfstop/`, bảng 64 ô: `receipts/enfstop/TABLE-all-64-cells.txt`.

## 1. Việc, một câu

Phân loại **bằng mã** năm họ của 57 dòng `gate-legs` thành stop **MẪU SỐ** vs
stop **THI HÀNH**, rồi quét cỡ stop **chỉ** ở các họ thi hành có trục tách khỏi
tín hiệu — và ở đó đo xem nới stop có đưa được một ô qua cổng không.

## 2. Phân loại — kết quả tiền kiểm, 0 ô cổng tiêu cho nó

| họ | base | `Exits::` | loại | quét? |
|---|---|---|---|---|
| `fx/us-long` | `session-hold` | `session_hold.rs:52-54` **`Strategy`** | **MẪU SỐ** | **không** |
| `box/b2` | `volman-box` | `volman_box.rs:75-77` `Engine` | THI HÀNH | **không — không có trục** |
| `struct-80-f14` | `far-stop-break` | `far_stop_break.rs:89-93` `Engine` | THI HÀNH | **không — không có trục ở `stopMode 0`, và ô gốc chạy trên `data-sealed`** |
| `pdhl/*` | `pdhl` | `pdhl.rs:74-76` `Engine` | THI HÀNH | **có** (L2) |
| `crt-nocap/4h-mid` | `crt` | `crt.rs:110-112` `Engine` | THI HÀNH | **có** (L3) |

**`fx/us-long` là ĐÚNG CÙNG MỘT FILE MÃ** với họ `close/<window>` mà
`agent/stop-width` đã quét (`session_hold.rs`, cùng `riskDailyRanges`) ⇒ kết
luận "chỉ đổi đơn vị" chuyển sang **nguyên vẹn**, không cần một ô nào.

**Ba lớp, không phải hai** — đó là sửa chữa chính của job này cho phụ lục 7 mục B:

    L1 MAU SO      Exits::Strategy                  -> doi DON VI. 0 lenh doi.
    L2 THI HANH, target = rr x risk                  -> stop va target gian CUNG
                                     nhau; con lai la cost/R + do phan giai.
    L3 THI HANH, target = MOT MUC GIA (crt target 0/1) -> stop gian, target DUNG
                                     YEN ⇒ RR that su doi.

`box/b2` và `struct-80-f14` thi hành stop nhưng **không có núm cỡ stop tách
khỏi tín hiệu**: `volman-box` lấy stop từ **biên đối diện của chính cái box đã
sinh ra tín hiệu** (`volman_box.rs:112-116`), núm duy nhất là `boxBars`, đổi nó
là đổi cơ chế; `struct-80-f14` là `stopMode = 0`, stop là **biên xa của kênh
Donchian đã sinh ra tín hiệu**, núm duy nhất là `period`. ⇒ quét chúng đòi
**thêm một tham số vào mã**, mà brief cấm thêm trục. **Tiêu 0 ô.**

## 3. Cổng — **0/64 ô qua. Không một ô nào.**

Đọc bằng **`PF_r`** (phụ lục 7 mục A), đếm lệnh bằng tay (cổng desk **40**, tool
in `need 30`), **arm có guards** — arm duy nhất chủ cho phép.

    o co PF_r >= 1,200                           : 2/62  (ca hai n=5)
    o co PF_r >= 1,200 VA n >= 40                : 0/62
    o qua ca ba chan tren CA HAI cua so          : 0/64

Ô gần nhất trong cả 64, arm có guards, `n ≥ 40` ở **cả hai** cửa sổ:
**`pdhlbuf/b0060`** (đệm 6,0 USD, stop thực 8,21 điểm):

    IS  xauduka 5m  n=159  PF_r 1,0310  PF_usd 1,039  E +0,014R  Lbar 0,4411R
                           net +278,54 USD  sut 1.262,70 USD = 10,95% dinh von
    OOS xauusd  5m  n=226  PF_r 0,9647  PF_usd 0,918  E -0,019R  Lbar 0,5480R
                           net   -7,01 USD  sut    11,62 USD = 11,46% dinh von

Thiếu **+0,235 của `PF_r`** ở cửa sổ xấu hơn. Và `n = 159` ở IS là **4,7%** của
`n = 3.395` ở cỡ stop hẹp nhất — xem mục 5.

## 4. Falsifier — bắn ở đâu

**F1 (tiền kiểm) KHÔNG bắn, và đó là kết quả dương duy nhất của job.** Ở loại
thi hành tập thoát đổi thật, khác hẳn loại mẫu số. Đo được, `crt` IS arm
guards-off, 8 cỡ stop:

    dem(USD)   0,1   0,3   1,0   3,0   6,0  10,0  20,0  40,0
    STOP       441   417   340   178    86    41     6     0   (32,96% -> 0,00%)
    TARGET     686   689   712   740   750   753   751   751   (+9,5%)
    TIMEOUT    211   224   260   361   433   472   504   510   (+142%)
    n         1338  1330  1312  1279  1269  1266  1261  1261   (-5,8%)

So với `stop-width` ở loại mẫu số: **y hệt ở cả 10 cỡ stop, không đổi một lệnh.**

**Và cái stop THOÁI HOÁ THÀNH MẪU SỐ khi đủ rộng.** Ở `crtbuf/b0400` dòng
`exits:` **không còn STOP nào** (0/1.261 IS, 0/1.243 IS-guards; 15/1.310 OOS).
⇒ **"hai loại stop" của phụ lục 7 mục B là một DẢI LIÊN TỤC, không phải hai
hộp**, và chỗ chuyển đo được: tỉ lệ STOP 32,96% ở đệm 0,1 USD → **0,48%** ở
20 USD → **0,00%** ở 40 USD. Trên cùng một cơ chế, cùng một `Exits::Engine`.

**F2 BẮN, đúng như khai.** `Lbar` đi qua 0,250R ở cả hai họ (`crt` 0,3907 →
0,0300; `pdhl` 0,6781 → 0,2004) và `PF_r` **dưới 1,200 ở mọi cỡ stop trên cả
hai cửa sổ trong arm có guards**. Nhưng cơ chế của cú bắn **mạnh hơn** câu khai,
và đây là con số đáng giữ nhất của job:

**Hai chân cổng kéo NGƯỢC nhau dọc trục cỡ stop.** Nới stop làm `PF_r` **tốt
lên** nhưng làm `Lbar` **nhỏ đi**, mà `Lbar` nhỏ thì chân expectancy đòi `PF_r`
**cao hơn** (`PF_r_cần = max(1,200 ; 1 + 0,050/Lbar)`). `crt` IS arm guards:

    dem(USD)  stop(diem)  PF_r    Lbar    PF_r CAN   thieu
      0,3       3,35     0,6627  0,3605    1,2000   -0,5373
      1,0       4,09     0,6851  0,3043    1,2000   -0,5149   <- thieu it nhat
      3,0       6,08     0,7256  0,2024    1,2470   -0,5214
      6,0       9,05     0,7520  0,1358    1,3682   -0,6162
     20,0      23,03     0,7869  0,0535    1,9346   -1,1477
     40,0      43,03     0,8030  0,0287    2,7422   -1,9392

Nới stop từ 3,35 lên 43,03 điểm (**12,8×**) cải thiện `PF_r` **+0,1403** nhưng
nâng cái vạch chân expectancy đòi **+1,5422** (1,2000 → 2,7422). **Cái vạch dịch
nhanh hơn phương pháp 11,0 lần.** Trên OOS: thiếu ít nhất ở đệm 3,0 USD
(−0,2957), thiếu nhiều nhất ở đệm 40 USD (−0,9155).

⇒ **Ở loại stop thi hành, "nới stop để qua chân expectancy" là một đường cụt
bằng số học**, ngược hẳn loại mẫu số nơi `stop-width` tìm được ô thoả cả hai
(bằng cách **thu hẹp** stop để phóng `Lbar`). Đây là phân biệt cấu trúc mà
đăng ký đặt cược vào, và nó **có**.

**F3 BẮN theo đúng chữ, nhưng lý do củng cố phân biệt L2/L3 chứ không xoá nó.**
Khai: nếu `PF_r` của L3 không đổi nhiều hơn L2 thì ba lớp sụp về hai. Đo được
(arm guards, `n ≥ 40`):

    L3 crt  : IS 0,6627 -> 0,8030  bien do 0,1403 ; OOS bien do 0,0830
    L2 pdhl : IS 0,5988 -> 1,0310  bien do 0,4322 ; OOS bien do 0,2731

L2 **dịch nhiều hơn 3,1×**. Nhưng số lệnh nói vì sao: `crt` giữ **1.320 →
1.243** (−5,8%) còn `pdhl` **3.395 → 159** (−95,3%). ⇒ **ở L3 đáp ứng `PF_r`
quy được cho cỡ stop; ở L2 nó lẫn với việc mất 95% mẫu.** Phát biểu đúng là:
**L3 là loại duy nhất trong năm họ nơi câu "nới stop" có một câu trả lời sạch**,
còn ở L2 trục cỡ stop **chính là** trục cỡ mẫu (mục 5).

## 5. Ở `pdhl`, trục cỡ stop VÀ trục cỡ mẫu LÀ MỘT

`maxRiskAtr = 3,0` (mặc định đã khai, `pdhl.rs:54`) chặn trên cỡ stop, nên nới
`bufferPips` **không nới stop — nó từ chối lệnh**:

    dem(USD)   0,1   0,3   1,0   3,0    6,0  10,0  20,0  40,0
    n (IS)    3395  3371  2897   905    159    24     2     0
    n (OOS)    308   307   305   294    226   123    24     5

Hai ô cuối của IS (`b0200` n=2, `b0400` **n=0**) **không đọc được**; `n=0` in ra
`PF NaN` và `verdict null` — đúng luật `null != 0`, engine không in 0 ở đó.
`pdhlbuf/b0400` OOS in `PF_usd 4,467 / PF_r 5,2177 / phân vị 100%` trên **5
lệnh** — đó chính xác là cái bẫy cỡ mẫu brief §4 mô tả, **không phải một kết
quả**.

Và `Lbar` của `pdhl` **không bao giờ xuống dưới 0,250R** ở ô `n ≥ 40` nào
(0,4344–0,6781) ⇒ **chân expectancy DƯ trên toàn bộ họ `pdhl`**, cổng ở đó là
`PF_r ≥ 1,200` một chân. Khớp phụ lục 6 mục I.

## 6. `cap_lots` — một ô cưỡi trần ở **98,1%** số lệnh, phá kỷ lục 57% của đợt trước

Phụ lục 7 mục C. Đọc từ `sized down N` trong dòng guards:

    pdhl-is-guards-on  b0001  3332/3395 = 98,1%   <- ky luc moi
    pdhl-is-guards-on  b0003  3302/3371 = 98,0%
    pdhl-oos-guards-on b0003   283/ 307 = 92,2%
    crt-is-guards-on   b0001  1034/1320 = 78,3%
    crt-oos-guards-on  b0001   890/1358 = 65,5%

Và **đo được trực tiếp rằng cả hai chân cổng mù với nó**. Cùng một ô
`crt/b0001` IS, chỉ bật/tắt guards:

    arm         PF_usd   PF_r    net USD    sut USD   sut %   sized down
    guards off  0,631   0,6657  -8333,95   8536,95   83,94%   (khong co)
    guards on   0,722   0,6729  -4888,49   5062,82   50,11%   1034 = 78,3%

Trần notional dịch **`PF_usd` +0,091** mà **`PF_r` chỉ +0,0072** — **gấp 12,6
lần** — trong khi nó cắt **41,3%** số lỗ và **40,7%** cú sụt. ⇒ một cổng đọc
bằng USD **ghi công cho cái trần**, không cho phương pháp. Chính xác điều phụ
lục 7 mục A nói, lần này đo trên một cơ chế stop thi hành.

## 7. HAI CỬA SỔ CỦA HỒ SƠ VÀNG CŨNG LÀ HAI CỠ TÀI KHOẢN — 100 lần

Đọc từ `config/default.toml`: `[trading] starting_equity_usd = 10_000.0`
(dòng 95) áp cho **`xauduka`**, nhưng `[markets.xauusd.trading]
starting_equity_usd = 100.0` (dòng 360). Xác nhận bằng chính receipt: cùng ô
`crt/b0001`, sụt **8.536,95 USD = 83,94%** trên IS và **55,23 USD = 50,92%**
trên OOS ⇒ đỉnh vốn ≈ 10.180 USD vs ≈ 108 USD.

⇒ **`PF_usd` KHÔNG so sánh được giữa hai cửa sổ của cả hai họ**, vì chúng là hai
sổ cách nhau 100 lần. Chỉ `PF_r` so được. Và dấu vết của nó đo được: khoảng cách
`PF_usd − PF_r` ở `b0001` là **+0,049** trên sổ 10.000 USD nhưng **+0,008** trên
sổ 100 USD — **gấp 6 lần**, vì sổ cent bị kẹp ở `min_lot` nên gần như không có
lãi kép để bóp méo. Đây là khuyết điểm 17 của phụ lục 7 **nhìn từ phía khác**:
nó không chỉ làm `PF_usd` của `xauusd` thành PF của sổ 0,01 lot, nó còn làm
**mọi so sánh hai cửa sổ trong hồ sơ vàng là một so sánh hai đòn bẩy.**

## 8. Hiện vật cửa sổ thứ 11 (cỡ stop) — đo được, nhưng NGƯỢC chiều phụ lục 7

Phụ lục 7 mục D nói IS **luôn đòi hẹp hơn 1,5–2,8 lần**. Ở đây:

- `crt`, tối ưu theo **`PF_r` đơn thuần**: IS = đệm **40 USD** (đơn điệu tăng
  tới cùng), OOS = đệm **10 USD** ⇒ **IS đòi RỘNG hơn 4,0 lần**, ngược chiều.
  **Nhưng OOS không định danh được**: `PF_r` của nó phẳng từ đệm 6 đến 40 USD
  (0,9361–0,9539, biên độ **0,0178**). Nên câu đúng là: **IS có tối ưu, OOS
  không có.**
- `crt`, tối ưu theo **thiếu-cổng** (cái quyết định): IS = đệm **1,0 USD**,
  OOS = đệm **3,0 USD** ⇒ lệch **3,0 lần**, và **cả hai nằm ở đầu HẸP của
  trục**, gần giá trị mặc định (0,3 USD). ⇒ **cùng một dải, hai thước đo, hai
  hướng tối ưu ngược nhau.** Thước nào bạn đọc quyết định bạn kết luận "nới
  stop" hay "thu stop".
- `pdhl`, `n ≥ 40`: IS = đệm **6 USD**, OOS = đệm **10 USD** ⇒ **1,67 lần**,
  IS hẹp hơn — chiều này khớp phụ lục 7.

## 9. Hằng đẳng thức `E = Lbar × (PF_r − 1)` — xác nhận trên 62 ô nữa

Phần dư lớn nhất trên **62 ô có số**: **0,00008 R**. Đúng con số phụ lục 7 báo
trên 200 ô, nay trên một lớp cơ chế khác (stop thi hành, hai cỡ nến, hai cỡ
tài khoản). Và **0/62 ô có `PF_usd` và `PF_r` ở hai phía vạch 1,200** — hiện
tượng 5/200 của đợt trước **không tái lập ở đây**, vì không ô nào gần vạch.
Khoảng cách `PF_usd − PF_r` lớn nhất ngoài các ô `n < 40`: **0,1162**
(`pdhl-is-guards-on b0001`, PF_usd 0,7150 vs PF_r 0,5988).

## 10. Dòng gần cháy — và vì sao không đọc PF của nó

Không ô nào in `_pct > 100%`. Nhưng hai ô **cách 0,04 điểm phần trăm**:
`pdhl-is-guards-off b0001` sụt **9.999,06 USD = 99,99%** đỉnh vốn và `b0003`
**9.995,48 USD = 99,96%** — trên sổ 10.000 USD, tức **sổ đã về gần 0**.
**Không đọc `PF_usd` của hai dòng đó.** Arm có guards cùng ô: 85,58% / 86,93% —
trần notional là thứ giữ sổ khỏi về 0, không phải cơ chế.

Và ở chiều ngược lại, con số để chủ máy quyết: `crt` IS arm guards, nới stop từ
3,14 lên 43,03 điểm cắt cú sụt **50,11% → 7,29%** (**6,9 lần**) và lỗ ròng
−4.888,49 → −682,72 USD, trong khi **cổng nói hướng ngược lại** (thiếu-cổng xấu
đi từ −0,527 xuống −1,939). **Cổng và drawdown xếp hạng trục cỡ stop ngược
nhau.**

## 11. Thứ KHÔNG đo được, và vì sao

- **Không tái lập được `struct-80-f14`.** Dòng duy nhất của nó
  (`docs/research/runs/2026-09-24-repair-c/era-era3-h4.txt`) tự in
  `--data=E:/rust/flowdesk/data-sealed`, mà brief §9 **cấm mở** kho đó (kho cắt
  23/09/2025, 362.803 nến vs 378.749 của kho sống ⇒ không so sánh được). Không
  mở. Nên ô đó **chưa từng được đo trên kho sống**, và đó là một dữ kiện về hồ
  sơ, không phải về cơ chế.
- **`wrong_side_stop` KHÔNG ĐƯỢC IN** bởi `--mode=hypotheses`. Không có dòng
  nào trong 8 receipt. ⇒ **không đo được**, không phải "bằng 0".
- **Phân vị của `pdhl` ở đệm ≥ 6 USD KHÔNG CÔNG BỐ**: `count match` ra ngoài
  băng — **2,96** (IS b0060), **19,58** (IS b0100), **235,00** (IS b0200),
  **9,20** (OOS b0400). Brief §4: ngoài băng thì dựa vào cổng. `crt` thì trong
  băng ở cả 32 ô (0,99–1,04), nên phân vị của `crt` đọc được — và nó nói
  **1%–73%**, không ô nào vượt 95.
- **`null p50` không ô nào in `0,000`** (0,714–0,968) ⇒ control calibrate được.
  Nhưng **mọi `null p50` đều < 1,000** ⇒ "phân vị cao" ở đây nghĩa là "lỗ ít hơn
  vào lệnh ngẫu nhiên", không phải "có lãi" — chính output tự in câu đó.
- **KHUYẾT ĐIỂM MỚI (đếm, không sửa): khoá `null_sides` trong `[run]` của batch
  file là TRƠ; chỉ cờ CLI `--null-sides=` có tác dụng.** Đo bằng hai lần chạy
  **cùng một file**, chỉ khác cờ:

      khong co co CLI   -> header in "null sides: coin - 50/50"
      --null-sides=exposure -> header in "null sides: exposure - ..."
      (receipts/enfstop/crt-is-guards-on.txt vs
       receipts/enfstop/00-plumbing-nullsides-flag.txt)

  `2026-10-09-enforced-stop-crt.toml` khai `null_sides = "exposure"` (copy
  nguyên từ `2026-10-04-crt-ceiling-lifted.toml`) và bị bỏ qua im lặng. Receipt
  gốc `D2-A-ceiling-lifted.txt` in `exposure` **vì nó truyền cờ CLI**, không vì
  file khai. ⇒ **32 ô `crt` của job này chạy so với null TUNG XU**, không so
  được phân vị với D2; cổng thì không phụ thuộc null nên không bị ảnh hưởng.
  Và luật chung: **đọc `null sides:` ở dòng header mà run tự in, đừng đọc file
  khai** — cùng một bẫy khai-vs-hành-vi của phụ lục 5 mục E.2, lần này ở khoá
  TOML chứ không ở cờ.
- **`--direction-samples=` bị `hypotheses` bỏ qua** — bảng cờ đã vá tự tố giác:
  `** --direction-samples IS NOT READ BY --mode=hypotheses — only by
  --mode=rescore — so nothing below was changed by it **`. Bản vá của
  `agent/instr-repair` làm đúng việc của nó.
- **Excursion trong lệnh** không có trong `max_drawdown_*` (đường vốn đã đóng).
  Mọi cú sụt ở trên là **SÀN**. `avg_mae` là field duy nhất thấy nó: `crt` IS
  b0001 **−0,543R**, b0400 **−0,076R**.

## 12. Một con số của brief KHÔNG khớp — và theo luật §8, số của tôi thắng

`receipts/gate-legs-table.txt` xếp `crt-nocap/4h-mid` vào nhóm "bị loại bởi
**chân expectancy một mình**" với **PF 1,309 · E +0,047R · n = 78**. Ô
`crt-nocap/4h-mid` **đã khai** không in ra những số đó. Tái lập chính xác ở cỡ
stop mặc định (`b0003`, `bufferPips = 3`):

    receipt goc  docs/research/runs/2026-10-04-crt/D2-A-ceiling-lifted.txt
                 n=1312  PF 0,708  E -0,122R
    job nay      crt-is-guards-on  b0003
                 n=1312  PF 0,708  E -0,122R      <- khop tung chu so
    receipt goc  D2-B-ceiling-lifted.txt
                 n=1353  PF 0,886  E -0,043R
    job nay      crt-oos-guards-on b0003
                 n=1353  PF 0,886  E -0,043R      <- khop tung chu so

⇒ **parity hoàn hảo**, và ô đã khai **trượt CẢ HAI chân** trên **cả hai** cửa
sổ. Dòng `PF 1,309 / n=78` đến từ nơi khác: `agent/m9`,
`receipts/nocap/G-A-file-2026-10-04-crt-ceiling-lifted.txt`, header của nó tự in

    bounds:   --from=2025-07-01 --to=2025-10-01 kept 6044 of 100586 bars
    max hold: 168 h (604800000 ms) from [markets.<id>.trading] max_hold_ms

— **cửa sổ 3 tháng** và **trần giữ 168 h thay vì 4 h**, trên `xauusd`. ⇒ cái
tên `crt-nocap/4h-mid` trong bảng `gate-legs` gom **hai run có cửa sổ khác nhau
VÀ `max_hold_ms` khác nhau** dưới một nhãn, và dòng được chọn để minh hoạ "chỉ
trượt chân expectancy" là **78 lệnh trong 3 tháng với trần giữ gấp 42 lần**.
Hiện vật "đồng hồ (trần giữ)" của phụ lục 5 mục B **đang nằm trong bảng
`gate-legs`**, không phải ngoài nó.

(Ghi rõ: bảng `gate-legs` tự khai là một **harvest 118 head**, không phải một
tập hợp ô đã chạy lại; lỗi là ở việc **đọc** một dòng harvest như một ô, mà job
này cũng đã làm khi chọn họ — nên phần này là tự sửa.)

## 13. Sổ đa phép thử — khai vs xem

    khai (4b961de):  2 ho x 8 co stop x 2 cua so x 2 arm = 64 o
                     + 1 lan plumbing (khong doc verdict)
                     4 lan goi binary + 1
    xem:             64 o, 4 lan goi + 1 lan plumbing. KHONG them truc,
                     KHONG them nhan, KHONG them co stop sau khi thay so.
    tieu 0 o cho:    fx/us-long (mau so), box/b2 (khong co truc),
                     struct-80-f14 (khong co truc + data-sealed bi cam)
    2 trong 64 o co n=0 (pdhl b0400 ca hai arm IS) => 62 o co so.

Ghi chú thêm vào đăng ký (mục dưới cùng của nó, không viết lại dòng cũ): cửa sổ
OOS của `pdhl` chạy `--from=2025-04-11` vì `[run] out_of_sample = "xauusd:5m"`
không ghi biên — chọn mốc liền sau `in_sample_to = 2025-04-10` để hai cửa sổ
không chồng, cùng quy ước `agent/stop-width` dùng.

## 14. Khuyến nghị để đó, bằng số, không chỉnh cổng

1. **Ở loại stop thi hành, trục cỡ stop không mở được cổng** — và không vì cơ
   chế yếu ở một chỗ nào cụ thể, mà vì **hai chân kéo ngược nhau dọc trục đó**
   (vạch dịch nhanh hơn phương pháp **11,0 lần** trên `crt` IS). Nếu chủ muốn
   chân expectancy nói một điều về cơ chế thì ở lớp này nó phải **đóng vào một
   `Lbar` khai trước**, giống kết luận của `stop-width` ở lớp mẫu số — hai lớp,
   cùng một chỗ hỏng.
2. **Cổng và drawdown xếp hạng trục cỡ stop ngược nhau** (6,9 lần sụt vs
   −1,41 của thiếu-cổng). Một chỉ tiêu drawdown bên cạnh cổng sẽ chọn đầu
   **rộng** của trục; cổng hiện tại chọn đầu **hẹp**.
3. **Trần notional là thứ đang giữ sổ khỏi về 0** (99,99% → 85,58% đỉnh vốn), và
   nó dịch `PF_usd` **gấp 12,6 lần** cái nó dịch `PF_r`. Đọc cổng bằng `PF_r`.
4. **Hai cửa sổ vàng của hồ sơ là hai sổ cách nhau 100 lần.** Nếu `xauusd` phải
   ở 100 USD thì mọi so sánh hai cửa sổ trong hồ sơ phải ở `PF_r`, hoặc
   `xauduka` phải hạ xuống 100 USD cho khớp. Chủ quyết.
