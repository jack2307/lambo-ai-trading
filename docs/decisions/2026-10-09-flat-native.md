# Đăng ký trước — hai cơ chế ĐÃ PHẲNG SẴN qua mốc rollover, đo trong khuôn `rollover-flat` (`agent/flat-native`)

**Ngày:** 2026-10-09. Nhánh `agent/flat-native`, cắt từ `agent/stop-width`
(`29c8635`). Commit này **chỉ có file này**, trước dòng code đầu tiên và trước
receipt đầu tiên (brief §5).

Brief: `/e/rust/AGENT-BRIEF-2026-10-07-AUDIT.md` + phụ lục 5, 6, 7. Cả bốn áp
dụng nguyên.

## 1. Giả thuyết, một câu

`gap-fade` và `intraday-momentum` vượt **0** mốc rollover 17:00 NY theo cấu
trúc (`swap$` in **0** ngay trong arm swap, đo bởi `agent/gross-ceiling`), nên
chúng **không trả carry** — thứ đã xoá sạch mọi cơ chế chân trời dài — **và**
chúng phủ đúng cửa sổ 16:00–18:00 NY mà `agent/rollover-flat` khai là chưa đo;
**nếu cửa sổ đó chứa gì thì một trong hai phải có `gross/phiên` ≥ 8,7 × spread
một lượt** (yêu cầu của khuôn, đo bởi `gross-ceiling`; cao nhất cả registry là
3,94×).

## 2. TIỀN KIỂM ĐÃ LÀM BẰNG ĐỌC MÃ, trước khi tiêu ô nào — phụ lục 7 mục B

Phân loại stop theo `Exits::` trong mã, **không theo tên tham số**:

| cơ chế | khai | file:dòng | ⇒ loại stop |
|---|---|---|---|
| `gap-fade` | `Exits::Engine` | `crates/fd-strategy/src/gap_fade.rs:60` | **THI HÀNH** |
| `intraday-momentum` | `Exits::Strategy` | `crates/fd-strategy/src/intraday_momentum.rs:65` | **MẪU SỐ** (self-managed) |

`engine.rs:481` đặt `self_managed = strategy.exits() == Exits::Strategy`, và
`check_exit` (`engine.rs:772-777`) mở đầu bằng `if position.self_managed {
return None }`.

⇒ **Hai cơ chế này thuộc HAI LOẠI KHÁC NHAU và phải đọc bằng hai luật khác
nhau:**

- **`gap-fade`: `0,117 điểm giá` của `gross-ceiling` là một STOP THẬT**, không
  phải một đơn vị ⇒ **cỡ stop là biến phải quét** (phụ lục 7 mục B). Nó không
  quét bằng `riskDailyRanges` mà bằng **hình học của chính cú gap**:
  `gap_fade.rs:96` đặt `stop = bar.open + gap × stopGapMult`, nên `stopGapMult`
  **là** cỡ stop, và `minGapAtr` là ngưỡng lọc tín hiệu.
- **`intraday-momentum`: stop của nó là mẫu số** (`tsmom::sizing_stop`,
  `riskDailyRanges × biên độ ngày NY trung bình 20 ngày`) ⇒ một quét
  `riskDailyRanges` ở đây là **quét đơn vị, không phải phép đo** (đúng kết luận
  `agent/stop-width`). Nó được quét **chỉ để khẳng định phân loại bằng số**
  (F7), không để tìm một ô qua cổng.

**Dự đoán khai TRƯỚC khi chạy, từ chỗ `0,117 điểm` phải đến:**
`gap_fade.rs:88-96` đặt stop cách `bar.open` một khoảng `|gap| × stopGapMult`,
nhưng lệnh là một LỆNH CHỜ khớp ở **open của nến sau**, còn engine tính
`risk = |entry − stop|`. Trên cú mở cửa Chủ nhật, nến sau mở đã đi phần lớn
đường tới stop ⇒ `risk` **sụp về gần 0** trong khi khoảng cách giá thì không.
Nên `0,117` **không phải một stop hẹp đồng đều** mà là **đuôi của phân phối
`|entry − stop|`**. Phép thử: dòng `the method's own realised stop: median ...
points` (engine tự in) sẽ in một số **lớn hơn nhiều** so với trung bình điều
hoà mà F3 đo. Nếu đúng vậy thì cú cháy đến từ **hình học entry-vs-stop**, và
nới `stopGapMult` là cách duy nhất chữa nó bằng tham số.

## 3. THƯỚC — và nó phải calibrate trước khi tin, vì thước của `gross-ceiling` KHÔNG dùng được ở đây

`gross-ceiling` lấy đơn vị R bằng `risk_usd = |swap$| / |total_r_net −
total_r|`. **Chính cái làm hai cơ chế này đáng đo lại làm thước đó chết:**
`swap$ = 0` ⇒ bước 1 không có gì để chia. Đó là lý do `gross-ceiling` khai
chúng **NOT MEASURED** (RESULT.md §8.1) và chỉ đưa một ước lượng tự khai là
không phải thước.

**Thước của job này — tuyến HOA HỒNG, lấy stop bằng ĐIỂM GIÁ, không qua USD.**
`engine.rs:941` tính `commission = commission_per_lot × lots × 2`, và
`Trade::r_net = points/risk + (swap − commission)/risk_usd`. Với `swap = 0`,
`contract_size = 1,0` và hoa hồng `c` mỗi lot mỗi chiều:

    r - r_net = 2c·lots / (risk · lots · cs) = 2c / risk      <- lots TRIỆT TIÊU

⇒ từ một arm chỉ khác ở hoa hồng:

    (B1) mean(1/risk)      = (total_r - total_r_net) / (2c·n)
    (B2) spreadR_mỗi_lượt  = 0,28 × mean(1/risk)       [spread phẳng 0,28 điểm]
    (B3) gross_mỗi_lượt    = expectancy + spreadR_mỗi_lượt
    (B4) risk điều hoà     = 1 / mean(1/risk)          [điểm giá]
    (B5) lots trung bình   = spread_paid_USD / (n × 0,28 × cs)

Tuyến này **tốt hơn** bước 2 của `gross-ceiling` ở hai chỗ, và cả hai chỗ đều
khai trước: nó cho **trung bình của tỉ số** `0,28/risk_i` (đúng thứ nằm trong
`expectancy`, là trung bình của `r_i`) thay vì **tỉ số của trung bình**; và nó
**không đi qua `lots`**, nên nó miễn nhiễm với lãi kép, `lot_step`, `min_lot`
(khuyết điểm 17) và `cap_lots` (phụ lục 7 mục C).

`c = 0,05 USD/lot/chiều`, đặt trong một thư mục config **sao chép** `config/`
chỉ đổi `commission_per_lot` của `xauduka` (cùng cách
`fd-rollover-flat/config-swap` chỉ đổi swap). Arm hoa hồng được đọc **CHỈ** để
lấy `(total_r − total_r_net)`; **mọi** PF, drawdown, net USD, exit-mix công bố
đều từ arm không hoa hồng.

**Phiên:** như `gross-ceiling` §2 bước 4, `S` = mean hold của `px-1s` **trong
cùng run**, `sessions_exposed = time_in_market_min / S`,
`gross_mỗi_phiên = gross_mỗi_lượt × n / sessions_exposed`,
`trips_mỗi_phiên = n / sessions_exposed`. Với một cơ chế trong ngày mẫu số này
**nhân lên** (tới ~23× ở hold 1h); `trips/phiên` và `spread/gross` in **cạnh
mọi dòng, luôn luôn**, và một dòng mua gross bằng nhiều lượt được khai là
**không tương thích khuôn**.

**Tỉ số đáng nhất của job, khai trước, bất biến theo đòn bẩy:**

    spreads_cua_gross = trips_moi_phien / (spread/gross)
                      = gross_moi_phien / spreadR_moi_luot
    khuôn cần >= ~8,7   |   cao nhất registry đo được = 3,94  (rsi2-pull, guards off)

## 4. Falsifier — cụ thể và bắn được

**F1 (falsifier của job, do chủ máy khai).** Nếu **cả hai** cơ chế trượt cổng
trên **cả hai** cửa sổ ở arm **có guards**, **VÀ** `spreads_cua_gross` của cả
hai dưới **3,94** trên `min(IS, OOS)`, thì **cửa sổ 16:00–18:00 NY — chỗ duy
nhất khuôn `rollover-flat` chưa phủ — cũng không chứa gì**, và khuôn đó đã được
thử trên toàn bộ phạm vi nó có thể phủ. Kết luận đóng.
**Nhánh ngược:** nếu một dòng vượt **3,94**, nó **không được đề xuất như ứng
viên**; báo kèm drawdown USD, phần dư sau drift, và tỉ lệ cưỡi `cap_lots`, rồi
để chủ máy quyết.

**F2 (tiền kiểm, ĐÃ XONG ở mục 2, tiêu 0 ô).** "Stop của `gap-fade` là mẫu số"
— **ĐÃ BẮN TRƯỚC KHI TIÊU Ô**: nó là `Exits::Engine`, stop thi hành. Nên cỡ
stop vào thiết kế như một biến, không như một đơn vị.

**F3 (thước — bắn ở tiền kiểm thì KHÔNG công bố số dẫn xuất nào).** Tuyến
hoa hồng phải tái tạo `px-1s` **trong run của tôi** hai số đã công bố của
`gross-ceiling`/`rollover-flat`:
`spreadR_mỗi_lượt` **1,150% (IS) / 0,826% (OOS)** và
`gross_mỗi_lượt` **−0,0035 (IS) / +0,0203 (OOS)**, sai số **≤ 2% tương đối**.
Trượt ⇒ **không công bố `gross/phiên`, `spread/gross` hay phần dư drift của
dòng nào**; chỉ công bố PF_r/PF_usd/drawdown/exit-mix (những thứ engine tự in).
Thêm, arm hoa hồng phải in **cùng `trades` và cùng `total_r`** như arm thường
(hoa hồng không được đổi tập lệnh); lệch ⇒ thước hỏng, cùng hậu quả.

**F4 (luật của chính cơ chế có nổ — brief §6a).** `--exit-mix` **bắt buộc** mọi
run. `gap-fade` phải thoát chủ yếu bằng `STOP`/`TARGET`; `intraday-momentum`
phải thoát chủ yếu bằng luật riêng (`window closed`). Dòng nào có luật riêng nổ
**0 lần** là **VÔ HIỆU** và không vào bảng, dù PF bao nhiêu.

**F5 (phần dư sau drift — phụ lục 5 mục B).**
`drift_gán = signed_share × gross_mỗi_phiên(px-1s cùng run) ×
(spreadR_dòng / spreadR_px-1s)`; `residual = gross_mỗi_phiên − drift_gán`.
`residual` dưới 25% của gross ⇒ dòng **không mang gì thêm ngoài phơi nhiễm**,
báo là **phơi nhiễm**, không phải cơ chế (`sess-hold` đọc 101,4% drift đúng
kiểu đó).

**F6 (cháy tài khoản — phụ lục 6 mục IV / 7 mục D).** `max_drawdown_pct > 100%`
⇒ **dòng đã cháy, KHÔNG đọc PF của nó**, loại khỏi mọi xếp hạng, nêu tên và số
USD. `gross-ceiling` đã đo `gap-fade/OFF OOS = 272,41%`, nên điều khoản này
**gần như chắc chắn bắn** và nó không phải cảnh báo hình thức.

**F7 (phân loại mẫu số, khẳng định bằng số).** Nếu `intraday-momentum` thật là
mẫu số thì ở arm **không** guards, `riskDailyRanges = 0,5 / 1,0 / 2,0` phải cho
**cùng số lệnh, cùng exit-mix**, và `expectancy`, `Lbar` tỉ lệ `1/f`
(`E × f` đi ngang). Nếu tập lệnh **đổi** thì nó không phải mẫu số thuần và mọi
câu về "đơn vị" của nó bị rút lại. Ở arm **có** guards tập lệnh ĐƯỢC PHÉP đổi
(`guards.rs` `max_open_loss_r = 2,0` thi hành `entry − 2 × risk` kể cả với vị
thế tự quản) — đó là stop thật, và chênh lệch hai arm chính là số đo.

## 5. THIẾT KẾ — 12 dòng, đóng băng, đếm ô TRƯỚC

Thiết kế: `docs/research/designs/2026-10-09-flat-native.toml`.

| # | nhãn | base | đổi so với mặc định | vì sao |
|---|---|---|---|---|
| 1 | `px-1s` | `session-hold` | from 1800, to 1600, side 1, riskDailyRanges 1,5, rangeDays 20 | **neo thước + neo drift**; phải tái tạo F3 |
| 2 | `gap-fade` | `gap-fade` | — (minGapHours 24, minGapAtr 1,0, stopGapMult 1,0) | dòng `gross-ceiling` đã đo; phải tái tạo |
| 3 | `gap/s05` | `gap-fade` | stopGapMult 0,5 | cỡ stop, hẹp hơn |
| 4 | `gap/s4` | `gap-fade` | stopGapMult 4,0 | cỡ stop, **rộng hơn cả lưới công bố** — chiều F2 nói là chiều phải đi |
| 5 | `gap/2atr` | `gap-fade` | minGapAtr 2,0 | ngưỡng tín hiệu, không phải cỡ stop |
| 6 | `gap/daily` | `gap-fade` | **minGapHours 0,9** | **DÒNG TRẢ LỜI CÂU HỎI**: hạ ngưỡng gián đoạn xuống dưới 1h đọc cú **gap NGÀY** ở nghỉ 17:00→18:00 NY, tức đúng cửa sổ 16:00–18:00 khuôn chưa phủ, mỗi ngày thay vì mỗi tuần |
| 7 | `gap/daily-a05` | `gap-fade` | minGapHours 0,9, minGapAtr 0,5 | cùng cửa sổ, ngưỡng lỏng hơn |
| 8 | `gap/daily-a2` | `gap-fade` | minGapHours 0,9, minGapAtr 2,0 | cùng cửa sổ, ngưỡng chặt hơn |
| 9 | `gap/daily-s4` | `gap-fade` | minGapHours 0,9, stopGapMult 4,0 | cỡ stop trên dòng gap ngày |
| 10 | `intraday-mom` | `intraday-momentum` | — | dòng `gross-ceiling` đã đo; phải tái tạo |
| 11 | `im/f05` | `intraday-momentum` | riskDailyRanges 0,5 | **chỉ cho F7** |
| 12 | `im/f2` | `intraday-momentum` | riskDailyRanges 2,0 | **chỉ cho F7** |

Không dòng nào khác được thêm sau khi thấy số. Không tham số nào khác được
chỉnh. Mặc định ở mọi chỗ không ghi trong bảng.

**Cửa sổ (hai, chia theo thời gian, như `rollover-flat`/`gross-ceiling`):**

    IS   --from=2010-06-01 --to=2018-06-01   (190.889 / 378.749 nến)
    OOS  --from=2018-06-01 --to=2026-06-01   (187.860 / 378.749 nến)

**Arm:** A = guards off, C = guards **on** (arm duy nhất chủ cho phép, phụ lục
5 mục D). Mọi kết luận ứng viên đọc ở **C**; A in ra để chỉ ra guards làm gì.

**SỔ ĐA PHÉP THỬ, khai trước:**

| | khai |
|---|---|
| dòng | **12** |
| cửa sổ | **2** |
| arm guards | **2** |
| **ô xếp hạng** | **48** |
| đọc arm hoa hồng (CHỈ lấy `total_r − total_r_net`) | **48** |
| run | **8** (A/C × hoa-hồng-có/không × IS/OOS) |
| dòng in ra | **96** |
| ô tiền kiểm (đã tiêu: 1 lần chạy 2024-01→2024-06, seeds 5, trên thiết kế của `gross-ceiling`, chỉ để xem binary có in `Lbar` + `realised stop`) | **1** |

**Cài đặt, đóng băng:**

    --mode=hypotheses --fixed --exit-mix --null-sides=exposure --seeds=50
    --interval=15m --data=/e/rust/flowdesk/data --market=xauduka
    --batch-file=<thiết kế ở trên>
    --config=/e/rust/fd-rollover-flat/config          (arm A, C)
    --config=/e/rust/fd-flat-native/config-comm       (arm Acomm, Ccomm)
    --guards                                          (arm C, Ccomm)

Binary: **`/e/rust/fd-stop-width/target-sw/release/search.exe`**.
`git diff 29c8635 HEAD -- crates/ src/ Cargo.toml Cargo.lock` **rỗng** và
`git status` sạch, nên worktree này và binary đó là cùng một chương trình ⇒
**không build gì** (brief §1). Đã kiểm bằng một lần chạy: nó in `Lbar`,
`PF_r`, `max drawdown`, `cost-matched null ... cost % of R` và
`the method's own realised stop: median ... points`.

`--seeds=50` thay vì 200 vì **không một phân vị nào được công bố** trong báo
cáo này (brief §4; và `gross-ceiling` §7 đo được `SURVIVES` đổi theo **chỗ ngồi
trong file TOML**). Null chỉ dùng để đọc `count match` / `cost match` như một
cảnh báo dòng không đọc được.
`--samples= --direction-samples= --rebate-share= --trail= --spread= --params=
--filters= --strategy= --batch= --companion= --null-registered-stop` **không
truyền**: không cờ nào trong số đó được `--mode=hypotheses` đọc cho kế hoạch
này. Mọi receipt phải in lại dòng `flags: N passed, every one of them read by
--mode=hypotheses` và dòng `news:` (engine đọc `data/news/events.parquet` từ
`data/` mặc định **bất kể `--data=`**).

## 6. Cách đọc

Một con số là kết quả: **`max` trên 48 ô của `min(IS, OOS)` của
`spreads_cua_gross`**, với cơ chế giữ nó được nêu tên, kèm `trips/phiên`,
`spread/gross`, drawdown USD, `_pct`, exit-mix, phần dư sau drift, và tỉ lệ
`cap_lots` nếu có.

* **dưới 3,94** ⇒ **F1 bắn**. Cửa sổ 16:00–18:00 NY không chứa gì hơn phần còn
  lại của registry, và khuôn `rollover-flat` đã được thử hết phạm vi. Đóng.
* **3,94 – 8,7** ⇒ báo như một khoảng cách đo được, nêu bias của cực đại, **và
  vẫn không đề xuất**.
* **≥ 8,7** ⇒ báo kèm drawdown, phần dư, `cap_lots`, **trao cho chủ máy**,
  không đề xuất.

Chân cổng đếm **BẰNG TAY ở 40 lệnh**, không ở `need 30` của tool, và khai
**chân nào ràng buộc** qua `Lbar` (phụ lục 6 mục I: chân expectancy dư ⟺
`Lbar ≥ 0,250R`). Khai **đơn vị** của chân PF: `PF_r` cạnh `PF_usd`, cả hai do
engine in (phụ lục 7 mục A).

## 7. KHÔNG đo — khai trước để không hứa

1. **Drawdown trong lệnh.** Mọi drawdown ở đây là đường vốn **đã đóng lệnh**;
   `avg_mae` là field duy nhất thấy vị thế đang mở đi ngược.
2. **Phân vị.** Không công bố, lý do ở mục 5.
3. **Instrument và cỡ nến khác `xauduka` 15m.** Brief §0 mục 3: **dấu** của
   edge có thể là thuộc tính của thước 15m. Mọi số ở đây là số 15m.
4. **Tham số ngoài 12 dòng.** Một cơ chế qua cổng ở tham số khác **không được
   đo** bởi kế hoạch này.
5. **Hai khuyết điểm brief §7** (`wrong_side_stop`; `check_exit` định giá stop
   bị gap tại `bar.open` trên nến khớp của lệnh chờ) — **đếm, không sửa**.
   Khuyết điểm thứ hai **gần như chắc chắn được kích** trên họ `gap/*` (lệnh
   chờ + nến gap là đúng hình dạng của nó), nên nếu nó xuất hiện thì **dòng đó
   không đọc được** và được báo thế, không được chữa.
6. **`data-sealed/`.** Không mở, không đọc, không trỏ tới, không đếm.
7. **Spread thật ở chỗ nghỉ phiên.** Feed này phẳng 0,28 ở mọi giờ; một venue
   thật **nới spread đúng chỗ** một cuốn sổ theo phiên mở lại — và họ `gap/*`
   vào lệnh **đúng tại nến mở lại**. Nên mọi `spread/gross` ở đây là **chặn
   dưới**, và sai lệch đó **ngược hướng giả thuyết**.
8. **Live/VPS.** Không chạm `config/accounts.toml` ngoài worktree,
   `config/local.toml`, VPS 103.19.29.194, `main`, `data/gold/`, `data/btc/`,
   `/e/rust/flowdesk/target/`, và hai process `collect.exe` (pid 5044, 38720).

`df -h /e` trước run đầu: **26 GB khả dụng**.

## 8. Ghi chú thêm (sửa đăng ký = thêm vào CUỐI, không viết lại dòng trên)
