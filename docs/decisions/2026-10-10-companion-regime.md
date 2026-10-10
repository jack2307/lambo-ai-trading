# Đăng ký trước — companion làm BỘ LỌC CHẾ ĐỘ ĐỒNG THỜI trên lệnh của một cơ chế vàng

**Ngày:** 2026-10-10. Nhánh `agent/companion-regime`, cắt từ `agent/stop-width`
(`29c8635`). Commit này **chỉ có file này**, trước dòng code đầu tiên (brief §5).

**Giả thuyết (1 câu):** trạng thái **ĐỒNG THỜI** của một instrument thứ hai
(bạc hoặc EUR, đọc trên **nến đã đóng cùng nến tín hiệu**) phân biệt được kết
cuộc các lệnh vàng của một cơ chế đã có **nhiều hơn** một bộ lọc **ngẫu nhiên
cùng tỉ lệ cắt**, và phân biệt **cùng một chiều** trên **cả hai** cửa sổ của desk.

**Đây KHÔNG phải trục nào đã đóng.** `agent/n2` giao dịch **tỉ số**; `agent/aud`
giao dịch **cross bản địa** (đóng ở tiền kiểm, chi phí 38,16% R); `agent/lead-lag`
dùng companion làm **dự báo đi trước** (đóng ở tiền kiểm, 0 ô cổng). Ở đây vị thế
**luôn là vàng**, cơ chế **không đổi**, companion chỉ trả lời **có vào hay không**.
Chiều này là chiều `agent/lead-lag` tự khai là **còn mở**: trễ 0 vàng/bạc
`+0,6900` (cửa sổ A) / `+0,7446` (B).

## 1. Cái sẽ đo ở TIỀN KIỂM (trước khi tiêu ô cổng)

Chạy cơ chế vàng **không có** bộ lọc companion, lấy **danh sách lệnh thật** của
engine, rồi **gán nhãn từng lệnh** theo trạng thái companion ở **nến tín hiệu**
(nến trước nến khớp — engine fill ở `bar.open` của nến sau, `engine.rs` bước 1/3).
Rồi so kết cuộc hai nhóm.

Đại lượng quyết định: **ΔE = E(trong trạng thái) − E(ngoài trạng thái)**, đơn vị
**R**, với `E = total_r / n` (phụ lục 8 §IV: `expectancy` in 3 chữ số không đủ).
Cạnh nó: `n`, win rate, `PF_r` (từ `r`, **không** phải `PF_usd`), `Lbar`, tỉ lệ cắt.

**Tiền kiểm là một PHÂN HOẠCH, không phải một bộ lọc.** Nó hỏi đúng một câu:
*trạng thái companion có mang thông tin về kết cuộc không?* Nếu có thì mới tiêu ô
cổng, và lúc đó **bộ lọc ĐỔI TẬP LỆNH** (engine giữ một vị thế nên tổng hai phần
bù **lớn hơn** bản không-filter — `month-clock` +17…+22 lệnh, `m8` +36) ⇒ **phải
báo số lệnh cả ba nhánh.**

## 2. Đối chứng: bộ lọc NGẪU NHIÊN cùng tỉ lệ cắt

Một bộ lọc cắt 50% lệnh sẽ đổi PF kể cả khi nó vô nghĩa. Đối chứng:
**xáo trộn mảng trạng thái companion TRONG THỜI GIAN** (Fisher–Yates trên các
nến có trạng thái xác định, **giữ nguyên tần số biên của từng trạng thái**), gán
lại nhãn cho đúng những lệnh đó, tính lại ΔE. **1.000 lần rút, seed 12345.**

Báo: phân vị hai phía của ΔE quan sát được trong phân phối null, **và** phần trăm
cú dịch mà đối chứng tái tạo được (`vprofile-gold` đo đối chứng tái tạo **45%**).

## 3. Đa phép thử — ĐẾM TRƯỚC

| trục | giá trị | số |
| --- | --- | --- |
| companion | `XAGDUKA`, `EURDUKA` | 2 |
| định nghĩa trạng thái | S1 `dir`, S2 `move`, S3 `vol` | 3 |
| cơ chế vàng | `ema-cross`, `rsi-reversion`, `donchian-breakout`, `bb-fade` | 4 |
| **= phép so khai trước** | | **24** |

Mỗi phép so đọc trên **2 cửa sổ** (A 2010-06-01→2018-06-01, B 2018-06-01→2026-06-01)
× **2 arm guards** (ON = arm duy nhất chủ cho phép; OFF = đọc thêm, **không** tính
là phép thử) ⇒ **96 con số**, **24 phép so**.

Ba định nghĩa trạng thái (mọi thứ đọc trên **nến companion đã đóng**, khớp
timestamp **CHÍNH XÁC**, thiếu bar ⇒ `null`, **không** lấy bar trước):

- **S1 `dir`** — dấu thay đổi close-to-close 1 nến của companion, chuẩn hoá theo
  ATR(14) của **chính companion**: `k = change / atr`. "Trong trạng thái" =
  `k` **cùng dấu** với chiều lệnh vàng **và** `|k| ≥ 0,25`.
- **S2 `move`** — `|k| ≥ 0,75` ("companion đang động mạnh"), bất kể chiều.
- **S3 `vol`** — `atr/close` của companion **trên** trung vị trượt 500 nến của
  chính nó ("companion đang ồn"). Trung vị trượt chỉ đọc nến đã đóng.

Ngưỡng `0,25` / `0,75` / `500` / `trung vị` **chốt ở đây, không quét**: quét
ngưỡng là nhân số phép thử lên và đó đúng là cách hồ sơ này đã làm ra ~6.000 ô và
0 phương pháp.

## 4. Falsifier — cụ thể và bắn được

- **F1 (chính).** Với **cả 24** phép so: `|ΔE|` **không** vượt phân vị 97,5 của
  null xáo trộn-thời-gian, **hoặc** dấu của ΔE **khác nhau** giữa cửa sổ A và B,
  **hoặc** một nhóm có **< 40 lệnh**. ⇒ **trạng thái companion đồng thời không
  phân biệt kết cuộc hơn một bộ lọc ngẫu nhiên cùng tỉ lệ cắt** ⇒ **cả hai chiều
  của hook companion đã đóng** (đi trước = `lead-lag`; đồng thời = job này), và
  **tiêu 0 ô cổng**.
- **F2 (nhân quả).** Trạng thái tính từ chuỗi companion **cắt ở nến k** phải
  **bằng từng bit** trạng thái tính từ chuỗi đầy đủ, ở **mọi** nến ≤ k. Kèm
  **chứng thực dương**: một trạng thái cố ý đọc nến **sau** (dịch −1) **phải**
  làm test đổi giá trị — nếu không thì test không bắn được gì và phép đo vô giá
  trị. Viết thành test Rust, chạy ở `--release`.
- **F3 (trần).** Nếu một phép so qua F1 nhưng nhóm tốt hơn có `E < +0,050R` trên
  **cả hai** cửa sổ thì bộ lọc **không thể** đưa cơ chế nào qua cổng ⇒ **tiêu 0
  ô cổng** và nói thẳng vậy.
- **F4 (cỡ mẫu).** Nhóm < 40 lệnh ⇒ **"không đo được"**, **không phải** "không
  có edge" (brief §4). In `null` chứ không in 0.
- **F5 (tập lệnh).** Nếu tiền kiểm qua và tôi tiêu ô cổng: nếu tổng số lệnh của
  hai nhánh filter-on + filter-off **không** lớn hơn số lệnh bản không-filter thì
  tôi đã đọc sai cái engine làm — báo, đừng im.

## 5. Cách đọc

- **Cổng, nếu tiêu ô:** `PF ≥ 1,200` **và** `E ≥ +0,050R` **và** `≥ 40 lệnh`,
  trên **cả hai** cửa sổ. `PF_r` **cạnh** `PF_usd`, khai đơn vị (phụ lục 7A, 9 §I).
  `E = Lbar × (PF_r − 1)`; chân expectancy dư khi `Lbar ≥ 0,250R`. Nói rõ **chân
  nào ràng buộc**. Drawdown USD cạnh mọi số lợi nhuận; `_pct > 100%` ⇒ dòng đã
  cháy, không đọc PF của nó. `--exit-mix` và **kiểm luật của cơ chế có nổ**.
  Kiểm `cap_lots`. Tool kiểm sàn 30 lệnh, **cổng desk là 40 — đếm tay**.
- **Phân vị không phải cổng**, và `null p50 = 0,000` nghĩa là **không calibrate
  được**. `count match` ngoài băng ⇒ **không công bố phân vị**.
- **Không hứa** gì ngoài những thứ trên. Cụ thể: tôi **không** hứa một chỉ tiêu
  drawdown ở tiền kiểm (tiền kiểm là phân hoạch, không có đường vốn mới).
- Mọi con số có đơn vị. Sáu từ bị cấm: score, composite, confluence, bias, rank,
  strength.

## 6. Cái đã biết trước, để không tự lừa

- `cost/R` của vàng trên **hai cửa sổ thật**: **12,58%** (A) / **8,35%** (B) ở
  1,5 ATR(15m) — **không** phải 4,04% (đó là trung vị cửa sổ 2 năm gần nhất).
  Bộ lọc **không** thêm chi phí, nhưng một ΔE nhỏ hơn tiếng ồn của chính chi phí
  thì không dùng được.
- `XAUDUKA` **không** đặt `starting_equity_usd` riêng ⇒ thừa kế **10.000 USD**,
  nên khuyết điểm 17 (mọi dòng `xauusd` chạy ở lot tối thiểu) **không** áp dụng —
  nhưng tôi sẽ **kiểm**, không giả định.
- Bốn cơ chế đều **không** override `exits()` ⇒ `Exits::Engine` ⇒ stop là
  **THI HÀNH**, không phải mẫu số (phụ lục 7B). Đây là điều kiện để `r` có nghĩa.
- Hồ sơ này chỉ chạy lại về đúng số của chính nó ở **65,3%** ô (phụ lục 8 §II) ⇒
  tôi **không trích** số đã công bố; mọi số trong báo cáo là số tôi chạy ra.

## 7. Thứ job này KHÔNG làm

Không quét lưới tham số cơ chế, không quét ngưỡng trạng thái, không đổi cổng,
không mở `data-sealed/`, không chạm `main`, không chạm `config/accounts.toml`,
`config/local.toml`, VPS, `data/gold/`, `data/btc/`, hay
`/e/rust/flowdesk/target/release/`. `--data=/e/rust/flowdesk/data` **chỉ đọc**.
Không giết `collect.exe` pid 5044 / 38720.

---

## Ghi chú thêm — 2026-10-10, SAU khi thấy bảng tiền kiểm, TRƯỚC khi chạy tiếp

Không viết lại dòng nào ở trên (brief §5). Thêm ba thứ, vì bảng tiền kiểm cho
**1/24** phép so sống sót (`XAGDUKA` × `vol` × `bb-fade`, dE +0,07666 R ở cửa sổ
A / +0,05334 R ở B, 0/1000 lần rút của **cả ba** đối chứng với tới) nên F1 **không
bắn sạch**, và một phép so sống sót phải trả lời ba câu nữa trước khi được gọi là
một hiệu ứng của companion:

1. **NỀN TỰ-TRẠNG-THÁI** (cái tương đương nền tự tương quan của `agent/lead-lag`).
   Tính **cùng một định nghĩa trạng thái trên CHÍNH VÀNG** (`XAUDUKA` làm companion
   của chính nó, khớp timestamp 100%). Biến động của bạc và biến động của vàng
   tương quan rất cao, nên nếu nền vàng phân biệt **bằng hoặc hơn**, thì
   "thông tin của companion" **chính là** chế độ biến động của vàng **đi qua một
   đường khác** — đúng cái `agent/lead-lag` bắt được ở Q4 (companion −0,0259 vs
   nền −0,0268, trùng tới chữ số thứ ba). **Falsifier F6:** nếu `|dE|` của nền
   vàng ≥ `|dE|` của companion trên **cả hai** cửa sổ thì companion **không thêm
   gì** và trục đóng.
2. **ỔN ĐỊNH (F7).** Chẻ 16 năm thành **4 quý 4 năm** (Q1 2010-06..2014-06,
   Q2 2014-06..2018-06, Q3 2018-06..2022-06, Q4 2022-06..2026-06). Nếu dE
   **lật dấu** giữa hai quý liền nhau thì đó là **hiện vật cửa sổ**, không phải
   một chế độ — tiền lệ: ô duy nhất qua F1 của `agent/lead-lag` lật từ +0,0503
   (Q3) sang −0,0259 (Q4).
3. **Đối chứng thứ ba đã thêm** (khai ở đây cho đủ, nó đã chạy): ngoài
   **xáo trộn thời gian** (bộ lọc ngẫu nhiên cùng tỉ lệ cắt, đúng thứ đăng ký
   yêu cầu) tôi đọc thêm **xáo trộn nhãn** (giữ nguyên cỡ hai nhóm) và
   **xáo trộn theo khối 96 nến = một ngày 15m** (giữ cấu trúc trong ngày, để một
   hiệu ứng giờ-trong-ngày không được mượn phân phối null hẹp của xáo trộn đầy).
   Một phép so chỉ được gọi là "phân biệt được" khi **qua cả ba**.

Và một con số phải ghi ngay, vì nó quyết định số ô cổng: **F3 đã bắn ở tiền kiểm.**
`E` **cao nhất của BẤT KỲ nhóm nào của BẤT KỲ phép so nào** là **−0,03902 R**
(arm guards ON) / **−0,02715 R** (arm guards OFF), trong khi cổng đòi
**≥ +0,050R**. ⇒ **không bộ lọc nào dựng từ những trạng thái này đưa được cơ chế
nào qua cổng** ⇒ **tiêu 0 ô cổng**, đúng như đã đăng ký.
