# KẾT QUẢ — ở chân trời 15m, hình trong ruột nến chứa thông tin BIÊN ĐỘ, không chứa thông tin CHIỀU

**Ngày:** 2026-10-10. Nhánh `agent/intrabar-shape`. Đăng ký:
`docs/decisions/2026-10-10-intrabar-shape.md` (+ hai ghi chú thêm cùng ngày).
Receipt: `receipts/intrabar-shape/precheck-xauduka-2026-10-10.txt` (216 dòng).
Dụng cụ: `py/research/intrabar_shape_precheck.py`.

**Outcome:** **Falsifier F2 BẮN.** Tiêu **0 ô cổng**. Nhưng nó bắn kèm một đối
chứng dương làm kết luận **chính xác hơn** câu brief dự tính: cùng tám đặc trưng,
cùng mẫu, cùng mô hình — **đổi kết cuộc từ CHIỀU sang BIÊN ĐỘ thì chúng sáng lên
tới `|t| = 38,1`**. Nên không phải "1m vô dụng ở 15m". Mà là:

    chieu nen 15m tiep theo : 1m THEM 0           (0/8 dac trung, 2 buoc mau)
    bien do nen 15m tiep theo: 1m THEM THAT, LON  (6/16 qua vach, dong dau 2 cua so)

## 1. Việc, một câu

Dựng hình thời-gian-tại-giá của mỗi nến 15m **thuần từ O/H/L/C của 15 nến 1m**
(`XAUDUKA-1m`, 5.635.777 nến, 16 năm) và hỏi nó có phân biệt được kết cuộc của
nến 15m **tiếp theo** hơn chính O/H/L/C của nến 15m đó không.

## 2. Cổng

**0 ô cổng tiêu.** Tiền kiểm chạy trước, F2 bắn ở tiền kiểm ⇒ không tiêu ô cổng
(brief §5; tiền lệ AUDNZD khai 432 ô, tiêu 0). **Không có PF, không có
expectancy, không có drawdown để báo** — và nói thẳng rằng không có, chứ không
in số 0.

## 3. Falsifier bắn chưa, ở đâu

| | bắn? | ở đâu |
|---|---|---|
| **F1** đồng dấu hai cửa sổ, `|t| ≥ 2,95` | **KHÔNG bắn ở bước 1** (4/8), **BẮN ở bước 2 và 4** (0/8) | trục A/B |
| **F2** đối chứng bắt buộc (cùng mô hình, cùng mẫu, bỏ 8 đặc trưng 1m) | **BẮN — 4/4 fold, cả hai cửa sổ** | trục C, và lần thứ hai ở trục E (0/8) |
| **F3** nhân quả | **KHÔNG bắn** | C1 pass 8×5 mốc, C2 pass 5/5, C3 pass |
| **F4** hiện vật cửa sổ | **bắn ở 1 đặc trưng** (`max_1m_share` lật dấu `t −2,176 → +0,277`) | trục A |

### F1 sống ở bước 1 là một ảo giác của tự tương quan, và trục B bắt được

Bốn đặc trưng qua vạch cùng dấu ở bước 1 — và **không cái nào** qua ở bước 2 hay
bước 4. Tỉ số `|t|₁/|t|₄` so với `√4 = 2,00`:

| đặc trưng | t(W1) b1 → b4 | tỉ số W1 | t(W2) b1 → b4 | tỉ số W2 |
|---|---|---|---|---|
| `t_high` | −3,434 → −2,347 | 1,46 | −4,017 → −1,720 | **2,34** |
| `extreme_order` | −4,160 → −2,356 | 1,77 | −3,489 → −0,820 | **4,26** |
| `path_eff` | −3,906 → −1,621 | **2,41** | −4,408 → −1,692 | **2,61** |
| `late_push` | −4,449 → −1,372 | **3,24** | −6,319 → −4,279 | 1,48 |

⇒ **Cả bốn đều có ít nhất một cửa sổ mà `|t|` tụt NHANH hơn `√k`**, tức `|t|` ở
bước 1 **bị phồng** — đúng cái bẫy `lead-lag` (`t +1,33` thành `t +3,78`). Điều
khoản đọc khai trước ở đăng ký §7 nói con số bước 4 là con số được báo ⇒ **0/8**.

### F2 bắn hai đường độc lập

**Đường 1 — `ΔR²_oos` và độ chính xác dấu, ngoài mẫu, cùng tập dòng:**

| cửa sổ | fold | đối chứng (6 đặc trưng 15m) | + 8 đặc trưng 1m | Δ |
|---|---|---|---|---|
| W1 | nửa1→nửa2 | R² +0,000075 · dấu 52,077% | R² +0,000052 · 51,923% | **R² −0,000023 · dấu −0,154 điểm%** |
| W1 | nửa2→nửa1 | R² +0,000120 · 51,797% | R² +0,000128 · 51,498% | R² +0,000008 · **dấu −0,299 điểm%** |
| W2 | nửa1→nửa2 | R² −0,000031 · 51,411% | R² −0,000259 · 51,335% | **R² −0,000229 · dấu −0,076 điểm%** |
| W2 | nửa2→nửa1 | R² +0,000223 · 50,983% | R² +0,000151 · 50,907% | **R² −0,000072 · dấu −0,076 điểm%** |

**Độ chính xác dấu xấu đi 4/4 fold. `ΔR²_oos` âm 3/4.** n_fit = n_đọc =
88.208 (W1) / 90.487 (W2) mỗi fold.

**Đường 2 — `t` của PHẦN DƯ** (đã trừ phần 6 đặc trưng 15m giải thích được;
đây là dạng đơn biên của chính falsifier, và nó đọc sạch hơn `ΔR²` đa biến):

    buoc 1: 0/8 qua vach cung dau     buoc 4: 0/8

Bốn "người sống sót" F1 sụp ngay khi trừ phần nến 15m đã mang:

| đặc trưng | t(W1) thô → phần dư | t(W2) thô → phần dư |
|---|---|---|
| `t_high` | −3,434 → **−0,391** | −4,017 → **−0,218** |
| `extreme_order` | −4,160 → **−0,957** | −3,489 → **+1,645** ← lật dấu |
| `path_eff` | −3,906 → **+0,944** ← lật | −4,408 → **+1,720** ← lật |
| `late_push` | −4,449 → **−2,684** | −6,319 → **−2,767** |

## 4. Vì sao chúng sụp — trục D, và đây là con số đáng giữ thứ hai

`R²` của mỗi đặc trưng 1m **hồi quy lên 6 đặc trưng của chính nến 15m**:

| đặc trưng | R²(1m ~ 15m) W1 | W2 | phần MỚI |
|---|---|---|---|
| `path_eff` | **0,9294** | **0,9315** | 7,1% / 6,9% |
| `extreme_order` | 0,6851 | 0,7056 | 31,5% / 29,4% |
| `late_push` | 0,5234 | 0,5311 | 47,7% / 46,9% |
| `t_low` | 0,5158 | 0,5421 | 48,4% / 45,8% |
| `t_high` | 0,5148 | 0,5415 | 48,5% / 45,9% |
| `max_1m_share` | 0,1442 | 0,1682 | 85,6% / 83,2% |
| `tpo_poc_pos` | **0,0065** | **0,0069** | **99,35% / 99,31%** |
| `tpo_skew` | **0,0035** | **0,0111** | **99,65% / 98,89%** |

Và tương quan từng cặp: `path_eff` với `ret_15` = **+0,9640 (W1) / +0,9651
(W2)**; `extreme_order` với `ret_15` = +0,826/+0,839.

⇒ **Cấu trúc của cái âm này, phát biểu đúng:** bốn đặc trưng **có** dự báo chiều
ở bước 1 là những đặc trưng **nến 15m đã mang** (R² 0,51–0,93); hai đặc trưng
**thật sự mới** — `tpo_skew` và `tpo_poc_pos`, tức **time profile thuần**, 99%
phương sai không nằm trong O/H/L/C — **không dự báo chiều**: `tpo_skew`
t = +4,304 (W1) → **+0,892** (W2); `tpo_poc_pos` +2,699 / +2,055, **cả hai dưới
vạch 2,95** và cả hai chết ở bước 4 (+0,391 / +0,271).

**Nói cách khác: cái mà nến 15m "ném đi" đúng là một thông tin — và nó không
phải thông tin về chiều của nến tiếp theo.**

## 5. Trục G — đối chứng DƯƠNG, và đây là con số đáng giữ nhất

Cùng 8 đặc trưng, cùng 176.416 / 180.975 dòng, cùng mô hình, **chỉ đổi kết cuộc
thành `y_rng = (high(i+1) − low(i+1)) / ATR14(i)`**:

| cửa sổ | fold | đối chứng R²_oos | + 1m | Δ |
|---|---|---|---|---|
| W1 | nửa1→nửa2 | +0,089949 | +0,093729 | **+0,003780** |
| W1 | nửa2→nửa1 | +0,101052 | +0,108963 | **+0,007911** |
| W2 | nửa1→nửa2 | +0,139120 | +0,150443 | **+0,011323** |
| W2 | nửa2→nửa1 | +0,122437 | +0,129915 | **+0,007478** |

`t` của phần dư (đã trừ hết 6 đặc trưng 15m), **cùng dấu ở cả hai cửa sổ**:

    max_1m_share  t = +28,303 (W1)  +38,060 (W2)
    t_high        t = +12,130 (W1)   +9,879 (W2)
    t_low         t =  +8,896 (W1)   +9,264 (W2)

⇒ **Dụng cụ nhìn thấy được.** Cùng đường ống, cùng mẫu, cùng vạch: ở biên độ nó
đọc `|t| = 38`, ở chiều nó đọc `|t| < 1`. Nên "F2 bắn ở chiều" **không phải một
dụng cụ mù** — nó là một phép đo. Đây là cái làm kết luận đọc được.

## 6. Phủ dữ liệu

    1m   : 5.635.777 nen   2010-06-01T00:00 -> 2026-05-31T23:59 (UTC)
    15m  :   378.749 nen   2010-06-01T00:00 -> 2026-05-31T23:45 (UTC)
    o 15m ma 1m khong co bucket nao:  0   (15m `resampled_from` chinh 1m)

    du 15/15 nen 1m : 360.111 nen 15m  (95,0791%)
    thieu (<15)     :  18.638 nen 15m  ( 4,9209%)  => DAC TRUNG NULL, khong 0
      14: 11.000 · 13: 3.046 · 12: 1.332 · 11: 673 · <=10: 2.587 · 1: 98

Mẫu thực của phép so: **176.416** dòng (W1) / **180.975** (W2) có **đủ cả 14 đặc
trưng + y** — và **đối chứng chạy trên đúng cùng tập dòng đó**, nếu không thì
phép so là phép so hai mẫu khác nhau. Test tự kiểm khẳng định 352 nến thiếu trên
lát thử mang **0** giá trị số.

## 7. Volume — không đọc, và đo lại trước khi tin

    XAUDUKA-1m.parquet   volume null_count = 5.635.777 / 5.635.777  (100%)
    XAUDUKA-15m.parquet  volume null_count = 0, gia tri duy nhat = {0.0}

Khớp phụ lục 9 §III: nhà cung cấp **không công bố gì**; bước `null → 0` của đường
ống **chế ra** số 0 ở 15m. `read_bars()` của dụng cụ này truyền
`columns=["time","open","high","low","close"]` — cột `volume` **không được mở ra
một lần nào**. Mỗi phút **một trọng số bằng nhau** ⇒ `tpo_skew` và `tpo_poc_pos`
**chính là** time profile, thứ duy nhất dữ liệu này nói thật.

## 8. Nhân quả — test viết TRƯỚC khi đo, kèm bẫy tự bắn

    C1 PASS : 8 dac trung x 5 moc cat, 0 gia tri doi (so sanh bit, NaN==NaN)
    C2 PASS : PROBE_peek_m16 TRUOT C1 o 5/5 moc => test BAT DUOC nhin truoc
    C3 PASS : 8 khe tren lat thu, y co so o 0 trong so do

C2 là điều khoản quan trọng nhất: một đặc trưng **cố tình** đọc close của **phút
16** (phút đầu của nến `i+1`) **phải** trượt C1, và nó trượt ở cả 5 mốc. **Một
test luôn pass là một test vô giá trị** — C1 chỉ đọc được vì C2 chứng minh nó bắt
được leakage. Tính chất bắt chước từ `fd-indicators/src/companion.rs::aligned_change`
(khớp `bar.time` **chính xác**, bar thiếu thì `continue`, không lấy bar trước).

## 9. Trùng khớp với thước — hai cửa sổ ĐÚNG của hồ sơ

    W1  2010-06-01 -> 2018-06-01    190.889 nen 15m
    W2  2018-06-01 -> 2026-06-01    187.860 nen 15m

**Trùng chính xác** hai cửa sổ của hồ sơ (phụ lục 5 §B), nơi **drift lật dấu**
(−0,0040R → +0,0205R mỗi phiên). `XAUDUKA` hết ở 2026-05-31 ⇒ W2 đầy. Không cắt
cửa sổ nào khác, nên không có hiện vật cửa sổ do chọn lát.

## 10. Sổ đa phép thử — khai vs xem

    khai (dang ky + 2 ghi chu):  104 phep so        xem: 104
      A  don bien buoc 1            16                16
      B  don bien buoc 2 va 4       32                32
      C  mo hinh long nhau (F2)      4                 4
      D  R2(1m ~ 15m)                0 (ti le)        16 ti le in ra
      E  t cua phan du              32                32
      F  tuong quan tung cap         0 (in ra)        16 in ra
      G  bien do                    20                20
    ------------------------------------------------------------
    O CONG TIEU:                      0                 0

Vạch Bonferroni **|t| ≥ 2,95** (α = 0,05 / 16 phép so chính) khai trước và
**không hạ sau khi thấy số**. Không đặc trưng nào, không bước mẫu nào được thêm
sau khi đọc kết quả; hai trục thêm (D/E/F và G) **khai bằng ghi chú có ngày ở
CUỐI file đăng ký**, mỗi cái **trước** khi chạy nó.

## 11. Chi phí I/O — kho lớn nhất hồ sơ từng chạm

    tong lan chay (ca selftest + 104 phep so + ghi cache):  18,1 s tuong
      doc 5.635.777 nen 1m (5 cot, khong co volume)       :  4,56 s
      doc    378.749 nen 15m                              :  0,04 s
      xep 1m vao o 15 phut (mang 378.749 x 15 x 4)         :  1,44 s
      8 dac trung + doi chung + ket cuoc                  :  3,89 s
    dinh bo nho (PeakWorkingSetSize):  2.465 MB
    lop dac trung ghi ra dia: target-ib/intrabar/XAUDUKA-intrabar-shape.npz
                              30.169.916 byte = 28,8 MB
    df -h /e:  truoc 16 GB avail  ->  sau 13 GB avail

⇒ **Kho 5,6 triệu nến KHÔNG đắt.** Nó đọc trong 4,6 s và toàn bộ trục vừa trong
2,5 GB RAM. Cú tụt 3 GB của `df` **không phải** từ job này (cache 28,8 MB) —
thủ phạm là `pagefile.sys` như brief §2; **không xoá `target/`**, và 13 GB vẫn
trên mức dừng 3 GB.

Một khuyết điểm của dụng cụ, **đã vá, và nó là một ca `null != 0`**: lần chạy đầu
in `dinh bo nho nan MB`. Nguyên nhân: không khai `restype` nên ctypes coi
`GetCurrentProcess()` là `c_int` và handle giả `-1` **bị cắt còn 32 bit**. Nó in
`nan` chứ **không in 0** — đúng luật §8 — và vì chi phí I/O là mục bắt buộc của
báo cáo nên hàm đó được sửa (`restype` + `argtypes`) rồi chạy lại.

## 12. Thứ KHÔNG đo được, và vì sao

- **Cổng.** Không ô nào. Tiền kiểm bắn trước ⇒ không có PF, không có expectancy,
  không có drawdown, không có `exit-mix`, không có `cap_lots`. Không in số 0 thay
  cho chúng.
- **Chân trời khác 15m.** Kết cuộc đo là **một** nến 15m tiếp theo, đúng chỗ
  engine khớp (tín hiệu nến `i` → OPEN nến `i+1`). Nói gì về 1h/1D là ngoại suy.
- **Phi tuyến và tương tác.** Mô hình là OLS tuyến tính. Một đặc trưng có thể dự
  báo chiều **chỉ trong một chế độ** mà OLS không thấy. Nhưng điều khoản F2 là
  so **cùng mô hình**, nên cái trượt vẫn là cái trượt **ở lớp mô hình này**.
- **Tám định nghĩa hình, không phải mọi định nghĩa hình.** Kết luận là về 8 định
  nghĩa khai ở §5 của đăng ký trên hai cửa sổ này.
- **Chi phí/R.** Không đo, vì không có lệnh nào. Và nhắc: trên hai cửa sổ này
  vàng đọc **12,58% (2010-2018) / 8,35% (2018-2026)** ở `1,5 ATR(15m)` (phụ lục 9
  §II) — con số **4,04%** lưu hành là trung vị của **2 năm gần nhất**, sai cửa sổ
  cho mọi kết quả 16 năm.
- **`XAUUSD`.** Feed khác, tick volume, **đổi hình ở 2023-12**. Không chạm.

## 13. Cái mở lại được, và cái đã đóng

**ĐÓNG:** *hình trong ruột nến 15m, dựng từ 1m, làm nguồn tín hiệu CHIỀU ở chân
trời 15m.* Đối chứng bắt buộc đọc **0/8 ở cả hai bước mẫu, cả hai cửa sổ**, trong
khi cùng dụng cụ đọc `|t| = 38` trên biên độ. Desk **thôi hỏi về dữ liệu tick/1m
cho lớp cơ chế này** — nghĩa là cho lớp **tín hiệu chiều ở 15m**.

**MỞ, và là thứ duy nhất trục này để lại:** 1m **có** nói về **biên độ** nến sau,
mạnh và đồng dấu hai cửa sổ (`max_1m_share` `|t|` 28,3 / 38,1; `ΔR²_oos` dương
4/4 fold). Đó **không phải một cơ chế** — cổng đo chiều, không đo biên độ — nhưng
nó là một đại lượng dùng được ở **cỡ lệnh / cỡ stop**, tức ở chỗ phụ lục 7 §B
gọi là **mẫu số của R**. Ai muốn mở lại thì mở ở đó, và phải nhớ: phụ lục 7 §B đo
được rằng với cơ chế **tự quản thoát**, đổi cỡ stop **không đổi một lệnh nào** —
nên một dự báo biên độ chỉ đổi đơn vị ở lớp L1, và chỉ là câu hỏi thật ở lớp L2/L3
(`Exits::Engine`).

**Sẽ KHÔNG mở lại bằng:** thêm định nghĩa hình thứ 9, hay đổi sang 5m / tick.
Hai đặc trưng **99% mới** (`tpo_skew`, `tpo_poc_pos`) đã là time profile thuần và
chúng đọc `t` dưới vạch ở bước 1 và **gần 0 ở bước 4**; thêm độ phân giải không
thêm cái mà độ phân giải hiện có đã không có.

## 14. Thứ một người đọc có thể hiểu sai

- **Không** nói "nến 1m vô dụng". Nói: ở **chân trời 15m**, cho **chiều**, với
  **8 định nghĩa này**, trên **hai cửa sổ này**, nó không thêm gì ngoài O/H/L/C
  của nến 15m — và nó **có** thêm cho **biên độ**.
- **Không** nói "F1 không bắn" là một kết quả dương. Bốn đặc trưng qua vạch ở
  bước 1 **là nến 15m nói lại** (R² 0,51–0,93) và **không cái nào** qua ở bước 4.
- **Không** nói gì về volume. Cột đó null 100% trên 1m, và job này không mở nó.
- **`late_push`** (phần dư `t` = −2,684 / −2,767, **cùng dấu hai cửa sổ**, cả hai
  **dưới** vạch 2,95 khai trước, và **chết ở bước 4**: −0,553 / −1,047) là đặc
  trưng gần nhất với một kết quả. Nó **không** là một kết quả ở vạch đã khai, và
  hạ vạch sau khi thấy số là chính cái brief cấm.
