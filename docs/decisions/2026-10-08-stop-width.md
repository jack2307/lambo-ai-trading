# Đăng ký trước — cỡ stop và chân expectancy của cổng (`agent/stop-width`)

**Ngày:** 2026-10-08. Nhánh `agent/stop-width`, cắt từ `agent/drawdown`
(`fd0e718`). Binary dùng: `/e/rust/fd-drawdown/target/release/search.exe`
(có `expectancy_net` và `max_drawdown_usd`) **cộng một bản vá in `Lbar`**
mô tả ở mục 4 dưới đây — bản vá đó là dòng code duy nhất job này viết.

Commit này **chỉ có file này**, trước dòng code đầu tiên và trước receipt đầu
tiên (brief §5).

## 1. Giả thuyết, một câu

`agent/gate-legs` đo được `E = Lbar × (PF − 1)` và tách hoàn hảo 87/87 dòng bị
loại một chân có `Lbar < 0,250R`; **nếu `Lbar` là một đại lượng của cỡ stop chứ
không của tín hiệu, thì đổi cỡ stop phải đưa những dòng đó qua chân expectancy
mà không cần một tín hiệu khác** — và câu hỏi thật là nó đổi tới mức nào thì
cơ chế đã thành một cơ chế khác.

## 2. Tiền kiểm đã làm BẰNG ĐỌC MÃ NGUỒN, trước khi tiêu một ô nào

Họ `close/<window>` (36/87 dòng, và chứa dòng `PF 2,065`) chạy
`base = "session-hold"`. Đọc mã:

- `crates/fd-strategy/src/session_hold.rs`: `fn exits() -> Exits::Strategy`, và
  với `riskDailyRanges > 0` nó phát một `stop` từ `tsmom::sizing_stop`
  = `riskDailyRanges × biên độ ngày NY trung bình (rangeDays=20)`.
- `crates/fd-backtest/src/engine.rs:772-777` — `pub fn check_exit`: **dòng đầu
  là `if position.self_managed { return None }`.** Nên cái stop đó **không bao
  giờ được thi hành**: nó là MẪU SỐ CỠ LỆNH, không phải một lệnh stop.
- `engine.rs:978` — `r: round4(points / position.risk)`, với
  `risk = |entry − stop|`. Nên `R` của mỗi lệnh là **giá chia cỡ stop**, không
  phụ thuộc `lots`.
- `engine.rs:729-730` — `raw_lots = risk_usd / (risk × contract_size)`, rồi
  `lots = (floor(raw/lot_step)×lot_step).max(min_lot)`; `risk_usd = equity ×
  0,01` (lãi kép).
- `engine.rs:1070-1076` — `profit_factor` tính từ `pnl_usd` (USD), còn
  `expectancy` là trung bình `t.r` (R). Đây đúng là nguyên nhân gốc phụ lục 6
  mục I đã đếm.

⇒ **Dự đoán khai TRƯỚC khi quét, trong arm không-guards:**

1. Tập lệnh **y hệt** ở mọi cỡ stop (số lệnh không đổi, `exits:` không đổi).
2. `E` và `Lbar` tỉ lệ **chính xác** với `1/f` (f = `riskDailyRanges`).
3. `PF` (USD) **gần như bất biến** — chỉ lệch qua lãi kép và qua `lot_step`
   làm tròn xuống / `min_lot` kẹp.

⇒ **Chiều của brief bị LẬT.** Để `Lbar` TO ra ở họ này phải làm stop **HẸP**
lại, không nới rộng. Nới rộng làm `Lbar` nhỏ đi. Số đo sẽ nói ai đúng.

⇒ Và arm **có** guards là arm khác hẳn một cách: `guards.rs:295-303`
(`max_open_loss_r = 2,0`) đặt một mức `entry − 2 × risk` **được thi hành kể cả
với vị thế tự quản** (`engine.rs:591` gọi `guard_exit` không hỏi
`self_managed`). Nên **trong arm có guards, thu hẹp cỡ stop LÀ cài một lệnh
stop thật ở 2f × biên độ ngày** — tập lệnh đổi, và đó mới là phép thử cỡ stop
theo nghĩa brief muốn. `max_notional_pct_equity = 300` (`guards.rs:248`,
`cap_lots`) là trần thứ hai.

## 3. Lbar mục tiêu và DẢI STOP — khai trước, tính trước

Lấy từ fixed-replay đã công bố trong `docs/decisions/2026-09-13-close-reopen-drift.md`
(`Lbar = E/(PF−1)`), f hiện tại = 1,000:

| nhãn | cửa sổ IS (xauduka) | f cần để Lbar = 0,250R | cửa sổ OOS (xauusd) | f cần |
|---|---|---|---|---|
| close/1630-1815 | PF 1,254 E +0,006R Lbar 0,0236R | **0,094** | PF 1,299 E +0,012R Lbar 0,0401R | **0,160** |
| close/1630-2000 | PF 1,237 E +0,011R Lbar 0,0464R | **0,186** | PF 1,637 E +0,039R Lbar 0,0612R | **0,245** |
| close/1630-2200 | PF 1,258 E +0,019R Lbar 0,0736R | **0,294** | PF 1,355 E +0,038R Lbar 0,1070R | **0,428** |
| close/1400-1630 | PF 0,739 E −0,017R | PF < 1,200: chân PF loại, không f nào cứu | PF 0,955 E −0,003R | như trên |

**Dải stop khai:** `f = riskDailyRanges ∈ {4,000 · 2,000 · 1,000 · 0,500 ·
0,300 · 0,250 · 0,180 · 0,120 · 0,090 · 0,060}` — mười cỡ, bọc cả năm giá trị
f mục tiêu tính trên, có f = 1,000 làm chứng (đúng hồ sơ) và hai cỡ RỘNG hơn
hồ sơ (4,000 và 2,000) để **bắn thẳng vào chiều mà brief đề xuất**.

**Dòng `PF 2,065 / Lbar 0,027R / 45 lệnh` — khai ngay rằng nó không phải một
dòng của hai cửa sổ.** Nó ở `receipts/exitmix/G-B-file-2026-09-13-close-reopen-drift.txt`
(commit `9f557cd`), và header của receipt đó ghi
`--from=2025-04-01 --to=2025-07-01`, 5.862 nến — **một lát ba tháng**, guards
ON. Cùng nhãn `close/1400-1630` trên cửa sổ OOS đã đăng ký đọc **PF 0,955–0,962**.
Job này đo nhãn đó trên **hai cửa sổ đã đăng ký**, không trên lát ba tháng, và
sẽ nói rõ chênh lệch đó là hiện vật cửa sổ (phụ lục 6 mục V).

## 4. Bản vá duy nhất: in `Lbar` ra, để hằng đẳng thức kiểm được từng dòng

`Metrics` đã có `win_rate` và `avg_loss_r` (`engine.rs:1106-1113`:
`avg_loss_r` là trung bình `t.r` trên tập `pnl_usd <= 0`). Với hàng này
commission = 0 và swap = 0 nên tập `pnl_usd <= 0` **là** tập `r <= 0`, nên

    Lbar     = |avg_loss_r| × (1 − win_rate)
    PF_r     = (avg_win_r × win_rate) / (|avg_loss_r| × (1 − win_rate))

đều **đo được, không suy ra từ cặp PF/E** — đó là điều kiện để phép kiểm hằng
đẳng thức không thành vòng tròn. Thêm một hàm `lbar_line(&Metrics)` bên cạnh
`drawdown_line`, in ở cả `hypotheses` và `rescore`, kèm một test. In cả
`PF_r` **và** `PF_usd` cạnh nhau: hiệu của chúng chính là chỗ đơn vị USD/R
chênh, tức chỗ hằng đẳng thức hỏng.

## 5. Falsifier — cụ thể và bắn được

- **F1 (của brief).** Nếu đẩy `Lbar` qua 0,250R mà **PF < 1,200 ở MỌI cỡ stop
  trên CẢ HAI cửa sổ, cả bốn nhãn**, thì hai chân cổng là **đánh đổi, không
  phải hai rào độc lập**. Bắn khi: không tồn tại ô nào có `Lbar ≥ 0,250R` VÀ
  `PF ≥ 1,200` VÀ `n ≥ 40` trên cả hai cửa sổ.
- **F2 (tiền kiểm, bắn ở arm không-guards).** Bắn khi trong arm không-guards,
  **mọi** f cho cùng số lệnh và cùng `exits:` trên một nhãn, `E × f` không đổi
  quá **1%**, và `PF` dao động dưới **0,050**. Nếu F2 bắn thì kết luận là:
  **ở họ `close/*`, cỡ stop là một ĐƠN VỊ ĐO, không phải một tham số cơ chế —
  chân expectancy của cổng đo mẫu số khai báo, không đo cơ chế**, và nó vượt
  được bằng cách khai lại đơn vị. Đó là một kết luận cấu trúc, không phải một
  ứng viên.
- **F3 (giao dịch được hay không).** Nếu ô duy nhất qua cổng chỉ qua ở arm
  không-guards, hoặc ở arm có guards thì `--exit-mix` cho thấy
  `OPEN_LOSS_CAP` / notional cap đã đổi tập lệnh và PF rơi dưới 1,200, thì
  cổng chỉ qua được trong arm **chủ đã cấm** (phụ lục 5 mục D) ⇒ **không phải
  ứng viên**, và viết đúng thế.
- **F4 (ranh giới cơ chế).** `--exit-mix` bắt buộc ở mọi ô. Khi tỉ lệ
  `OPEN_LOSS_CAP` về **0** thì cỡ stop không còn là một stop; khi nó lên trên
  **50%** thì cơ chế đã thành "stop 2f biên độ ngày", không còn là cơ chế
  "giữ tới hết cửa sổ". Báo tỉ lệ đó ở mọi ô và nói ô nào đã vượt ranh giới.
- **F5 (cháy).** Ô nào in `max_drawdown_pct > 100%` thì **dòng đó đã cháy**
  (khuyết điểm 14) — báo và **không đọc PF của nó**.

## 6. Sổ đa phép thử — ĐẾM TRƯỚC

    4 nhãn × 10 cỡ stop = 40 giả thuyết trong một file batch
    × 2 cửa sổ (IS xauduka 2018-06-16→2025-04-10; OOS xauusd 2025-04-11→)
    × 2 arm (guards off / guards on)
    = 160 ô, 4 lần gọi binary.

Không có trục nào khác. Không thêm nhãn, không thêm cỡ stop sau khi thấy số:
sửa đăng ký = **thêm ghi chú có ngày vào CUỐI file này**.

`seeds` khai ở mục 7 sau khi đo thời gian một lần chạy; **phân vị KHÔNG phải
cổng** (brief §4) nên nếu `count match` ra ngoài băng thì không công bố phân vị.

## 7. Cách đọc

- Cổng: `PF ≥ 1,200` VÀ `E ≥ +0,050R` VÀ `n ≥ 40` trên **cả hai** cửa sổ. Đếm
  `n` bằng tay (verdict của tool kiểm `need 30`).
- Mọi ô báo: `n · PF_usd · PF_r · E · Lbar · max_drawdown_usd · _pct ·
  exit-mix`. Hằng đẳng thức kiểm theo `PF_r`; khoảng cách tới `PF_usd` là
  phép đo của chỗ đơn vị chênh.
- Chi phí/R = `spread/stop`: spread 0,28 USD, stop = f × biên độ ngày NY 20
  ngày ⇒ **chi phí/R tỉ lệ 1/f**, nên f nhỏ làm chi phí/R TO ra. Lấy số từ
  dòng `cost-matched null` mà output tự in, kèm cỡ stop, và luôn nói
  "biên độ ngày NY 20 ngày" chứ không viết "ATR" trần (phụ lục 5 mục F).
- Nếu f tối ưu khác nhau giữa hai cửa sổ ⇒ **cỡ stop là hiện vật cửa sổ thứ
  mười một**, và viết đúng thế (phụ lục 6 mục V).

---

## Ghi chú thêm — 2026-10-08, sau lần chạy đầu, trước khi đọc kết quả

1. **`fixed = true` trong file batch KHÔNG được `--mode=hypotheses` đọc.**
   `search.rs:307` lấy nó từ `std::env::args().any(|a| a == "--fixed")`; trường
   trong `[run]` của file toml trơ. Lần chạy đầu (OOS, guards off) vì thế ra
   **walk-forward (4 folds)**, không phải fixed-replay — đúng như hồ sơ cũ
   cũng có cả hai bản (`out-of-sample.txt` walk-forward vs
   `out-of-sample-fixed.txt`). Bảng f mục tiêu ở mục 3 tính từ bản **fixed**,
   nên bốn lần chạy chính của job này đi kèm `--fixed`.
   **Lần chạy walk-forward đó được GIỮ và báo, không bỏ** — nó là ô thứ 161–200
   và sổ đa phép thử sửa thành **200 ô, 5 lần gọi binary**.
2. **`seeds`**: lần chạy đầu dùng `--seeds=100`. Bốn lần `--fixed` còn lại dùng
   `--seeds=20` vì **phân vị không phải cổng** (brief §4) và cái job này đo là
   `PF · E · Lbar · drawdown · exit-mix`, không cần null. ⇒ **Phân vị của bốn
   lần đó KHÔNG được công bố**; chỉ phân vị của lần `--seeds=100` được in ra,
   và chỉ khi `count match` trong băng.

## Kết quả — 2026-10-08

Đọc ở `docs/decisions/2026-10-08-stop-width-result.md`. Tóm một dòng: **F2 bắn**
(ở họ `close/*` cỡ stop là đơn vị đo, không phải tham số cơ chế — tập lệnh y hệt
ở cả 10 cỡ, `E` và `Lbar` tỉ lệ `1/f`), **F1 không bắn** (có ô qua cả hai chân
với `Lbar ≥ 0,250R` trong arm giao dịch được), **F4 bắn** ở `f ≤ 0,120`
(`OPEN_LOSS_CAP` thành exit chính ⇒ cơ chế khác), **F3 và F5 không bắn**
(0/200 ô cháy). Cổng: 13/40 ô qua hai cửa sổ ở arm không-guards, **1/40** ở arm
có guards. Hai điều khoản số của F2 trượt, và cả hai cái trượt đáng hơn kết quả:
nửa spread (0,138 USD) rò vào đơn vị rủi ro (R² = 0,99685), và `PF_usd` dịch
tới **0,3640** trên cùng một tập lệnh khi `PF_r` dịch **0,0040**.
