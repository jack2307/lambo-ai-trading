# Kết quả — đồng hồ THÁNG trên vàng 15m (`agent/month-clock`)

**Ngày:** 2026-10-09. Đăng ký: `docs/decisions/2026-10-09-month-clock.md` (hai
commit chỉ-docs, `de39d1a` và `b491cec`, trước dòng code đầu tiên).
Receipt: `receipts/monthclock/`.

Một dòng: **chu kỳ tháng không chứa một hiệu ứng CHIỀU nào đo được trên vàng ở
khung 15m** (0/54 mốc, đúng mức tung xu); nó chứa **đúng một** hiệu ứng **biên
độ** bền lạ thường (ngày giao dịch thứ 4 từ cuối tháng hẹp hơn 6–9%), và hiệu
ứng đó **làm chi phí/R XẤU ĐI 6,8–12,6%** chứ không mở một chân cổng nào —
**0/24 ô qua cổng**, và dấu của chênh lệch **lật giữa hai cửa sổ ở 2/3 cơ chế**.

## 1. Cổng: 0

**0/24 ô** qua `PF ≥ 1,200` VÀ `E ≥ +0,050R` VÀ `n ≥ 40` trên cả hai cửa sổ.
Bảng đầy đủ: `receipts/monthclock/TABLE-24-gate-cells.txt`.

- `n` nhỏ nhất là **81 lệnh** ⇒ **không ô nào bị sàn cỡ mẫu loại**, F3 không bắn.
- **Chân ràng buộc:** ở họ `close/*` là **expectancy** (`Lbar` 0,0645–0,0741 R,
  dưới 0,250 R ⇒ `BINDS`); ở `rsi2` và `donch` là **PF** (`Lbar` 0,42–0,53 R ⇒
  chân expectancy **DƯ**, đúng hằng đẳng thức của phụ lục 6 mục I).
- Hằng đẳng thức `E = Lbar × (PF_r − 1)` đúng **24/24 ô**, phần dư lớn nhất
  **0,00007 R** (vạch cổng là +0,050 R).
- **Không ô nào cháy** (`dd% > 100`: 0/24), nhưng `donch/notTD4` ở W1 in
  **99,99%** và `_pct` là **SÀN** ⇒ **sổ đó coi như chết**; đừng đọc PF của nó.
- Cả hai arm cho cùng kết luận. Ở `c2200`, arm có guards gần như **y hệt** arm
  không guards (`OPEN_LOSS_CAP` nổ **1 lần trong 1.504 lệnh**) ⇒ kết quả này
  **không** phải thứ chỉ sống trong arm chủ đã cấm; nó thua ở cả hai.

## 2. Falsifier: F1 bắn một nửa — và nửa nó bắn là nửa đáng

| | khai | bắn? |
|---|---|---|
| **F1** chiều | 0 mốc đồng dấu + p<0,05 hai cửa sổ | **BẮN cho `Disp`** (0/54). KHÔNG bắn cho `Amp` (1/54). |
| **F2** hiện vật cửa sổ | mốc chỉ sống một nửa ≈ mức may rủi | **BẮN ở bước đo**: 2/3 cơ chế lật dấu. |
| **F3** sàn cỡ mẫu | mọi cơ chế < ~840 lệnh/cửa sổ | **không bắn** — 10/10 ứng viên đạt. |
| **F4** luật có nổ | filter không đổi tập lệnh | **không bắn** — filter nổ, xem §5. |
| **F5** arm giao dịch được | chỉ sống ở arm không-guards | **không bắn** (cả hai arm thua như nhau). |
| **F6** cháy | `dd% > 100` | **không bắn** (0/24; cao nhất 99,99%). |

**F1 bắn cho đại lượng có dấu, và đó là kết luận sạch của job:** sau khi kể đến
**216 phép so**, **0/54 mốc dịch-chuyển-có-dấu** đồng dấu ở cả hai cửa sổ với
p<0,05. Số mốc p<0,05 ở một cửa sổ là **6 (W1) / 9 (W2)** so với kỳ vọng may
rủi **5,4** — **đúng mức tung xu**. ⇒ Desk **đóng được một đồng hồ mà trực giác
ai cũng tin là có**: không có "cuối tháng thì vàng đi lên/xuống" đo được ở 15m.

## 3. Sổ đa phép thử: khai vs xem

| | khai | xem |
|---|---|---|
| tiền kiểm | **216** (brief job khai 124 = một đại lượng; tôi đo hai, và thước B có 23 mốc không 31) | **216**, không hơn một phép |
| ô cổng | **12**, sửa lên **24** ở ghi chú có ngày trước khi chạy | **24** |
| ô đếm lệnh (§2.6.1, không phải ô cổng) | khai là bước bắt buộc | **20** (10 cơ chế × 2 cửa sổ) |
| lần gọi binary | — | **7** (1 bỏ + 2 đếm + 4 cổng) |

Lần gọi đầu bị bỏ và nói ra: `--market=xauusd` cộng `[run]` trong file batch
**không đặt được cửa sổ dưới `--fixed`** (phụ lục 8 mục VI) nên nó chạy
xauusd 2022-06→2026-09 thay vì xauduka. Nó **không** vào sổ ô vì nó không trả
lời câu hỏi nào; `receipts/monthclock/COUNTS-unfiltered.txt` giữ lại làm chứng.

Dương-giả kỳ vọng ở sàng đã khai: **0,135 mốc**. Qua sàng: **1**. BH q=0,05 trên
216 p-value giữ **đúng 1** phép so. Sàng "cùng dấu" một mình: **64/108** qua,
kỳ vọng may rủi 54 ⇒ **vô dụng một mình, đúng như khai trước**.

## 4. Con số đáng giữ nhất — và nó âm cho trục này

**Mốc duy nhất sống: biên độ ngày giao dịch thứ 4 từ cuối tháng.**

    W1 2010-06..2018-06   Amp 0,0850 vs 0,0933   -0,0083 bien do ngay NY 20d  = -8,9%  p 0,00005
    W2 2018-06..2026-06   Amp 0,0911 vs 0,0976   -0,0065 bien do ngay NY 20d  = -6,6%  p 0,0324
    n = 96 moi cua so (mot thang cho DUNG MOT quan sat; 192 thang = 192 quan sat)

Bền một cách hiếm trên hồ sơ này: **cùng dấu và p < 0,05 ở cả hai nửa tại CẢ
NĂM chỗ cắt cửa sổ** (2014-06 / 2016-06 / 2018-06 / 2020-06 / 2022-06, p xấu
nhất 0,0324); hoán vị trong-thăng 20.000 lần cho p một phía **0,0002 / 0,0059**;
**11/12 tháng trong năm cùng dấu**; và **thi hành được** — thước chỉ-dùng-lịch
(không cần biết ngày lễ) cho **−9,0% / −6,4%**, p **0,0000 / 0,0383**.

**Và engine xác nhận nó một cách độc lập, bằng một thước khác, trên tập lệnh:**

    cua so  co che  stop 2,0 ATR(15m)  TD4 -> con lai     chi phi/R  TD4 -> con lai
    W1      rsi2       2,73 -> 2,96 diem   (-7,8%)        10,25% -> 9,46%   (+8,4%)
    W1      donch      2,76 -> 3,00 diem   (-8,0%)        10,14% -> 9,33%   (+8,7%)
    W2      rsi2       3,91 -> 4,41 diem  (-11,3%)         7,15% -> 6,35%  (+12,6%)
    W2      donch      4,17 -> 4,46 diem   (-6,5%)         6,71% -> 6,28%   (+6,8%)

**4/4, hai cửa sổ, hai cơ chế, cùng dấu**, và độ lớn (6,5–11,3%) **bọc đúng**
con số tiền kiểm (6,4–9,0%) dù đo bằng ATR(15m) chứ không bằng biên độ ngày NY
20 ngày. (Chi phí/R = spread 0,28 / stop; luôn nói cỡ nến — đây là **ATR 15m**,
không phải biên độ ngày.)

⇒ **Nội dung đo được duy nhất của đồng hồ tháng đi SAI CHIỀU cho một cổng:**
ngày hẹp hơn ⇒ cùng spread trên một R nhỏ hơn ⇒ **chi phí/R to ra 6,8–12,6%**.
Không có chiều để bù lại, vì §2 đã đóng chiều.

## 5. Filter có nổ, và nó đổi tập lệnh — không phải đổi đơn vị

Luật của filter nổ, kiểm bằng số lệnh (`--exit-mix` bật ở cả 24 ô):

    c2200   1612 khong-filter  ->   81 (TD4) + 1531 (phan bu) = 1612   dung bang
    rsi2    7392 khong-filter  ->  363       + 7046           = 7409   (+17)
    donch   8768 khong-filter  ->  428       + 8359           = 8787   (+19)

Tỉ lệ cắt **1/20,9 · 1/21,4 · 1/21,5** — khớp `1/21` đã khai trước. Và chỗ
**tổng hai phần bù LỚN HƠN bản không-filter** (+17, +19) là bằng chứng gate
**đổi tập lệnh** chứ không chia nó: chặn một lệnh làm sổ rảnh ra và một lệnh
muộn hơn vào được. `c2200` khớp đúng bằng vì `session-hold` vào ở **một phút cố
định**, không có chuyện chen chỗ.

Luật của từng cơ chế cũng nổ (bẫy brief §6a): `c2200` thoát **window closed
81/81**, đó chính là luật của nó; `rsi2` và `donch` thoát bằng
`STOP / TARGET / TIMEOUT` của chính chúng. Nhãn filter in ra receipt đúng chữ:
`MoTuWeTh + NY 16:15-16:20 + business day 4 from month end (NY)`.

## 6. Hiện vật cửa sổ thứ 12 — đo được, không suy ra

Chênh lệch `PF_r` giữa ngày-mốc và phần bù, **lật dấu giữa hai cửa sổ ở 2/3 cơ
chế, ở CẢ HAI arm**:

    co che   W1 dPF_r   W2 dPF_r    ket luan
    c2200    -0,0753    +0,0510     LAT DAU
    rsi2     -0,1204    -0,0567     cung dau (ngay-moc XAU hon, dung du doan)
    donch    +0,0134    -0,1271     LAT DAU

1/3 cùng dấu **là đúng mức tung xu**. Và ô duy nhất trông đẹp hơn phần bù —
`c2200/TD4` W2 `PF_r 1,3409` so với `1,2899` — là **83 lệnh** đối **1.505**, và
**cùng ô đó ở W1 đọc 1,0823 so với 1,1576**. Đó chính xác là hình dạng cảnh báo
chuẩn của brief §4 (`PF 1,753 / 14 lệnh` cạnh `PF 0,682 / 178 lệnh`).

**Dự đoán đã khai trước ("ngày-mốc sẽ XẤU hơn") cũng chỉ đúng 1/3** ⇒ nó không
chỉ "không có edge", mà **chênh lệch ấy thậm chí không nhất quán theo chiều xấu**.
Phần bù thì **không phân biệt được với bản không-filter**: `c2200` W1
`PF_usd 1,151 → 1,1539`, W2 `1,312 → 1,3061`. Bỏ 1/21 số lệnh đổi **không gì** —
đúng như số học đã nói trước khi chạy.

## 7. Thứ KHÔNG đo được, và vì sao

1. **Mốc biên độ TD-4 có phải một cơ chế hay một pha của lịch** — không đo được
   ở job này. Ba dấu hiệu nói nó **không phải** cấu trúc nến 15m:
   - **Đổi chuẩn hoá thì LẬT DẤU**: `amp15m / biên độ của CHÍNH ngày đó` cho
     **−3,2% (p 0,28) / +3,4% (p 0,20)**. Biên độ thô [USD]: p 0,14 / 0,46. Cái
     hẹp đi là **cả NGÀY** so với chuẩn 20 ngày, không phải nến trong ngày.
   - **Mốc kế thừa một pha thứ-trong-tuần**: `tdfe = 4` rơi Thứ Ba **79/192
     (41%)** thay vì ~20%, vì 3/7 số tháng kết thúc vào Thứ Bảy/Chủ Nhật nên
     phiên cuối là Thứ Sáu. Pha đó giải thích **−0,0010 trong −0,0083 (≈12%)**,
     nên không phải toàn bộ — **nhưng trong từng weekday hai cửa sổ không đồng ý
     chỗ hiệu ứng nằm**: W1 ở Thứ Ba (p 0,000, n 37), W2 ở Thứ Ba là **+0,0004
     (p 0,947)**.
   - **Độ lớn nhỏ so với dụng cụ đã có**: hiệu của mốc là **24–31% bề rộng một
     băng `VolAbs` p25–p75**, và `Amp` của mốc nằm ở **phân vị 42 / 49** của
     phân phối ngày — **giữa phân phối**. Đồng hồ tháng **không chỉ ra một chế
     độ biến động mà `VolAbs` chưa chỉ được**.
2. **Hiệu ứng chiều nhỏ hơn ~0,19 biên độ ngày** không đo được ở n ≈ 69–96 — đã
   khai trước ở §2.4 của đăng ký. Nên §2 nói "không có hiệu ứng chiều **đo
   được**", **không** nói "không có hiệu ứng chiều".
3. **Thước B đúng (theo phiên thật, có ngày lễ) không thi hành được** trong một
   `Filter`: gate không biết ngày lễ tương lai. Khớp 86,8%; filter dùng thước
   chỉ-dùng-lịch và mốc sống nguyên dưới nó, nhưng **hai thước không phải một**.
4. **`volume` không đo được trên feed này** — 0 ở 0/50.000 mẫu (phụ lục 8 mục I)
   ⇒ tiền kiểm không đọc nó, và một đồng hồ tháng theo khối lượng
   **chưa từng được đo**, không phải **đã bị bác bỏ**.
5. **Excursion trong lệnh** không có: `_pct` là sàn, `avg_mae` là field duy nhất
   thấy excursion (phụ lục 6 mục IV).
6. **Phân vị KHÔNG công bố.** Receipt tự in rằng null trung vị **lỗ** (p50
   0,790–0,885 < 1) và rằng `c2200` **long share 1,000 so với 0,494 của null**
   ⇒ phân vị của nó **mang drift của instrument**. Hai lý do, và cổng là thứ nói
   chuyện.

## 8. Thứ job này ĐỂ LẠI cho desk, dù cổng là 0

Một trục đã đóng **có dụng cụ**, không phải một trục đóng bằng lời:

- `fd_core::clock::new_york_month_position` — `(ngày trong tháng, ngày làm việc
  thứ N từ cuối tháng)` trên đồng hồ New York, **cố ý không biết ngày lễ** để
  một gate không nhìn lén, + test.
- `Filter::MonthEnd` / `Filter::MonthDays` với spelling `monthend:1-4`,
  `monthend:4-4`, `monthend:!4-4`, `monthdays:25-31`, cùng khuôn `Weekdays`,
  + 3 test. Band không admit được ngày nào là **lỗi parse**, không phải một dòng
  0 lệnh im lặng.
- Và một con số để ai mở lại trục này phải trả lời trước: **216 phép so, 0/54
  mốc chiều, và mốc biên độ duy nhất làm chi phí/R xấu đi 6,8–12,6%.**

## 9. Hai thứ job này KHÔNG đề xuất, như đã khai trước

1. **Không** đề xuất hình dạng "tín hiệu chậm thể hiện bằng nhiều lệnh nhỏ" —
   cổng tính theo **lệnh** nên hình dạng đó đóng bằng số học (phụ lục 6 mục II).
   Một đồng hồ tháng rất dễ rơi vào đúng đó, và nó không phải đường ra.
2. **Không** trích một con số đã công bố làm dữ kiện. Mọi số ở trên là số job
   này chạy ra, trên binary job này build.
