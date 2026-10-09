# Đăng ký trước — thoát MỘT PHẦN: chốt một phần ở một mức, giữ phần còn lại (`agent/partial-exit`)

**Ngày:** 2026-10-09. Nhánh `agent/partial-exit`, cắt từ `agent/stop-width`
(`29c8635`, có `lbar_line` in `Lbar`/`PF_r` và drawdown trong receipt).
Worktree `/e/rust/fd-partial-exit`.

Commit này **chỉ có file này**, trước dòng code đầu tiên và trước receipt đầu
tiên (brief §5).

## 1. Giả thuyết, một câu

Hai đầu của trục quản lý vị thế đã đo và chúng ngược nhau — **giữ nguyên tới
target** (`intraday/donchian-breakout` cửa sổ A, `--fixed`: TARGET 49,
PF_usd 1,117, E +0,054R) và **chốt hết sớm bằng trailing** (`agent/m4`:
trail 0,5R kích hoạt từ +0,33R biến 49 lần về đích thành 17 và +0,054R thành
−0,019R; 37 dòng có đổi số, trung bình −0,0175R) — nên **nếu lý do trail thất
bại là "cắt hết phần đuôi", thì cắt một PHẦN đuôi phải mất một PHẦN thiệt hại,
và câu hỏi là phần bảo hiểm mua lại được có nhiều hơn phần đuôi mất đi không.**

Prior đo được (sổ `xau-stoch`, 73 lệnh đã đóng, tiền thật): **81% lệnh chạm
+0,33R, chỉ 11% chạm +2,00R.** Đó là nguồn của ba mức chốt dưới đây, không phải
một lưới chọn sau khi thấy số.

## 2. TIỀN KIỂM ĐÃ LÀM BẰNG ĐỌC MÃ NGUỒN, trước khi tiêu một ô nào — và nó đòi một bản vá engine

**Engine HIỆN KHÔNG thoát một phần được.** Đọc mã:

- `engine.rs:273-320` — `pub struct Live` có **`lots: f64`**, một con số duy
  nhất. Không có trường khối lượng còn lại, không có khối lượng đã chốt.
- `engine.rs:915-922` — `pub fn close_position(position: Live, ...) -> Trade`
  **nhận `Live` theo GIÁ TRỊ và trả về một `Trade`**. Nó luôn đóng toàn bộ
  `position.lots`. Không có đường nào đóng một phần.
- `grep -rin partial crates/fd-backtest/src/engine.rs` → **0 kết quả** nào là
  một cơ chế (chỉ `PartialEq`, `partial_ord`). Trục này **chưa từng được đo**,
  không phải **đã bị bác bỏ**.
- `engine.rs:978` — `r: round4(points / position.risk)`. **`r` KHÔNG đọc
  `lots`.** ⇒ Nếu hiện thực thoát một phần bằng cách đẩy hai `Trade` riêng,
  mỗi chân sẽ khai **R đầy đủ** trong khi nó chỉ cầm một phần vị thế: một chân
  chốt nửa ở +0,33R rồi nửa còn lại dừng ở −1,00R sẽ in **+0,33 và −1,00**
  (tổng −0,67R) thay vì **−0,335R** thật. Và **`n` sẽ phình lên** — mỗi vị thế
  thành hai lệnh — nên chân "≥ 40 lệnh" và cả mẫu số của `E = total_r/n` đọc
  sai. **Hình thức "hai Trade" bị loại bằng đọc mã, trước khi chạy.**

⇒ **Chi phí bản vá, khai trước (tiền lệ `agent/n3`: đo ~330 dòng rồi mới
làm):** ~330–380 dòng, **nằm hết ở lớp LUẬT**, cạnh `trail_stop` vốn đã là một
luật thoát mức-rules:

| chỗ | việc | dòng (ước) |
|---|---|---|
| `fd-core/src/config.rs` | `PartialExitConfig { enabled, at_r, fraction }` + `[trading.partial]`, **off by default** | ~45 |
| `engine.rs` | `TradingRules.partial`; `Live` thêm `banked_r`/`banked_pnl_usd`/`partial_taken` (`#[serde(default)]` cho state file cũ) | ~25 |
| `engine.rs` | `partial_level()` + `take_partial()` — cùng hình dạng `trail_stop()` | ~55 |
| `engine.rs` | `close_position` **gộp** phần đã chốt vào MỘT `Trade` | ~25 |
| `engine.rs` | nối vào bước 2 của vòng lặp, thứ tự bi quan | ~30 |
| `engine.rs` | `Trade.banked_r` + `Metrics.partials` + `metrics_of` + `empty()` | ~30 |
| `search.rs` | `--partial=<at_r>,<fraction>` / `off`, dòng header, vào bảng `ALWAYS` | ~30 |
| test | 1 file mới: luật nổ · null đi qua cùng luật · **tắt thì BIT-IDENTICAL** · đẳng thức R có trọng số · stop thắng partial trên cùng một nến | ~120 |
| 5 chỗ khai `Trade {` trong test helper | thêm một trường | ~6 |

**Quyết định thiết kế quan trọng nhất, học từ `agent/n3`: luật này nằm trong
RULES, KHÔNG trong strategy.** `control::RandomEntry` (`control.rs:143`) cũng
phát `Intent::Enter`, và nó **không khai `fn exits()`** nên nó nhận mặc định
`Exits::Engine` (`registry.rs:256`) và `target: None` ⇒ engine tự đặt target ở
`reward_risk × risk`. ⇒ **null đi qua đúng luật thoát một phần đó mà không một
dòng nào viết riêng cho null.** Nếu bản của tôi cần đường riêng cho null thì
tôi đang làm sai, và test `the_matched_null_takes_the_same_partial` ghim điều đó.

**Thứ bản vá KHÔNG làm, khai trước:**

- **Không chạm vị thế tự quản** (`self_managed`), đúng luật `trail_stop`. ⇒ Họ
  `close/*` / `session_hold` (`Exits::Strategy`) là **no-op**. Một số 0 ở đó là
  **không đo được**, không phải bác bỏ.
- **Không dời stop về breakeven sau khi chốt.** Đó là một can thiệp KHÁC (và nó
  chính là trail). Giữ trục sạch: chỉ thoát một phần.
- **Chỉ MỘT lần chốt mỗi vị thế**, không phải một cái thang.
- **Không đổi cỡ lệnh vào.** Vị thế vẫn mở đúng `lots` như hiện trạng.

**Kế toán đã chốt trước khi viết code** — một vị thế = **một** `Trade`:

    r    = (points/risk) x (lots_con_lai/lots_goc) + banked_r
    pnl  = points x lots_con_lai x cs - comm(lots_con_lai) + swap(lots_con_lai) + banked_pnl
    banked_r = (points_chot/risk) x (lots_chot/lots_goc)

`n` = **số vị thế**, bất biến so với hiện trạng. Phí hoa hồng và spread tổng
**không đổi** khi chẻ (`comm = per_lot × lots × 2` cộng lại đúng bằng cả cục;
nửa spread tính trên `lots` của từng chân) ⇒ **thoát một phần KHÔNG tốn thêm
chi phí giao dịch**. Nếu nó xấu đi thì nó xấu vì phần đuôi, không vì phí.

**Thứ tự trên MỘT nến, bi quan, khai trước:** `check_exit` chạy TRƯỚC. Nếu nó
trả `Stop` thì **đóng hết ở stop** (đọc bi quan: −1,00R đầy đủ, không được
nửa +0,33R). Nếu nó trả `Target`/`Timeout`/guard **và** nến đó cũng chạm mức
chốt mà chưa chốt, thì **chốt phần trước rồi đóng phần còn lại** (bi quan:
`f×0,33 + (1−f)×1,8` nhỏ hơn `1,8`).

## 3. Dự đoán cụ thể, viết TRƯỚC khi thấy số — và nó là một dự đoán ÂM

Hằng đẳng thức của cổng (phụ lục 6 mục I, đúng 200/200 ô):
`E = Lbar × (PF_r − 1)`, `Lbar = |Σ(−R)|/n`.

**Thoát một phần LÀM NHỎ MỖI CÚ LỖ.** Một lệnh chạm +0,33R rồi dừng in
`r = 1,33f − 1`: ở `f = 0,50` là **−0,335R** thay vì −1,000R, ở `f = 0,25` là
**−0,6675R**. Với prior 81% lệnh chạm +0,33R, **`Lbar` phải tụt mạnh** — ở
`at_r = 0,33 / f = 0,50` tôi dự đoán `Lbar` về khoảng **một phần ba** giá trị
hiện trạng trên phần lệnh có chạm mức.

⇒ **Và `E = Lbar(PF_r − 1)` nói chân expectancy KHÓ HƠN khi `Lbar < 0,250R`**
(tách hoàn hảo 87/87 dòng của `agent/gate-legs`). Nên dự đoán khai trước:
**thoát một phần cải thiện PF_r và làm XẤU khả năng qua chân expectancy, cùng
lúc.** Nếu số đo nói thế thì **đó là cách trục này chết**, và tôi phải nói ra
bằng số chứ không bằng chữ. Nếu số đo nói ngược, số đo thắng (§8).

Dự đoán thứ hai: **drawdown USD phải NHỎ hơn** — nửa vị thế thì cú sụt nửa.
Đây có thể là giá trị thật duy nhất của trục này, và **cổng không có chữ nào
về nó** (phụ lục 6 mục IV). Báo nó cạnh mọi số.

## 4. Đa phép thử — ĐẾM TRƯỚC, lưới THÔ

    muc chot  at_r     in {0,33 ; 0,67 ; 1,00} R      <- 0,33 tu prior MFE (81% cham)
    ti le     fraction in {0,25 ; 0,50}               <- 0,50 la "chot mot nua"
                                                         0,25 la lieu NUA => kiem
                                                         dap ung theo lieu luong
    => 6 o. KHONG 3x5, KHONG 10x5.

`at_r` đều **dưới** target `reward_risk = 1,8` (`config/default.toml:98`), nên
cả ba mức là một mức chốt thật chứ không phải chính cái target.

| phần | run | row/run | row |
|---|---|---|---|
| đối chứng `--partial=off` × 2 cửa sổ × 2 arm | 4 | 3 | 12 |
| lưới 6 ô × 2 cửa sổ × 2 arm | 24 | 3 | 72 |
| **tổng khai** | **28** | | **84** |

Vượt 84 thì tôi công bố **cả hai** con số và nói vượt ở đâu. **Không nâng lưới
sau khi thấy kết quả.** Công bố **toàn bộ** lưới, không chỉ ô thắng — đúng như
`agent/m4` đã làm với lưới trail.

Batch file: bản sao nguyên văn `docs/hypotheses/2026-10-06-trail-grid.toml` của
`agent/m4` (3 dòng: `compression/bb-fade`, `intraday/donchian-breakout`,
`intraday/bb-fade`) để hai đầu của trục so được với nhau trên cùng một file.
Cửa sổ A `2025-07-01..2025-10-01`, B `2025-04-01..2025-07-01`; `xauusd` 15m,
spread 0,28/lượt, `max_hold_ms` 4h, `--fixed` (tham số cố định, không chọn).

## 5. Falsifier — cụ thể và bắn được

**F0 (tiền kiểm, ĐÃ BẮN MỘT NỬA):** engine không thoát một phần được ⇒ phải
vá. **Nếu bản vá không giữ được BIT-IDENTICAL khi tắt** (`--partial=off` không
tái hiện đúng từng chữ số dòng hiện trạng, và golden parity của
`-p fd-backtest` không còn qua), **tôi DỪNG và báo chi phí thay vì đo** — một
engine khớp lệnh sai tệ hơn không có phép đo.

**F1 (chính):** trục **TUYÊN BỐ CHẾT** nếu **không ô nào** trong 6 ô tốt hơn
đối chứng `off` trên **cả hai** cửa sổ. "Tốt hơn" đọc theo thứ tự: (a) qua cổng
(`PF_r ≥ 1,200` **và** `E = total_r/n ≥ +0,050R` **và** ≥ 40 lệnh đếm tay) khi
đối chứng không qua; nếu không ô nào qua cổng thì (b) `E` cao hơn đối chứng của
**chính cửa sổ đó**, ở cả hai cửa sổ, cùng một ô.

Bắn F1 thì — **cộng với kết quả trail của `agent/m4`** — mọi can thiệp vào phần
đuôi làm sổ xấu đi, và desk đóng được **cả họ "quản lý vị thế"**.

**F2 (luật không nổ ⇒ KHÔNG ĐO ĐƯỢC, không phải bác bỏ):** nếu `Metrics.partials`
= 0 hoặc dòng đó trùng khít `off` thì ô đó không nói gì về trục. Brief §6(a):
`tsmom/120d` in `SURVIVES` với luật riêng nổ **0 lần**. Tôi in số lần chốt
cạnh mỗi dòng để điều đó không thể ẩn.

**F3 (cửa sổ):** nếu ô thắng của cửa sổ A không phải ô thắng của cửa sổ B ở cả
hai tham số thì kết quả là **hiện vật cửa sổ thứ 12**, không phải một cơ chế —
đúng như F2 của `agent/m4` đã bắn với trail.

## 6. Cách đọc

- **`PF_r` cạnh `PF_usd`, khai đơn vị** (phụ lục 7A, phụ lục 8 mục III:
  `PF_usd` đọc **cao hơn** ở 320/452 ô = 71%). Thoát một phần **đổi cỡ lệnh
  giữa đường**, nên nó chạm đúng chỗ hai số này phân kỳ. `lbar_line` in cả hai.
- **`E = total_r/n`**, không đọc `expectancy` 3 chữ số (phụ lục 8 mục IV).
- **`Lbar` và chân nào ràng buộc** ở từng ô.
- **Drawdown USD cạnh mọi số**; `_pct` là SÀN; `_pct > 100%` ⇒ dòng đã cháy,
  không đọc PF của nó.
- **Arm có guards là arm DUY NHẤT chủ cho phép.** Ô chỉ sống ở arm không-guards
  ⇒ nói thẳng là **không giao dịch được**.
- **Phân vị KHÔNG phải cổng**, và `count match` ngoài băng ⇒ không công bố phân
  vị. Baseline của `agent/m4` đã in `count match 0,73 ** outside the band **`
  cho đúng dòng này, nên tôi **không** dựa vào phân vị ở job này.
- `null p50` cạnh mọi phân vị nếu có công bố; `null p50 = 0,000` nghĩa là
  **không calibrate được**.
- **Không trích một con số đã công bố như dữ kiện** (phụ lục 8 mục II: chỉ
  65,3% ô chạy lại khớp). Đối chứng `off` được **chạy lại trong cùng binary
  này**, và nếu nó không khớp 271 lệnh / PF 1,117 / +0,054R của `agent/m4` thì
  tôi báo cả hai con số.

## 7. Thứ đăng ký này KHÔNG hứa

- Không hứa một chỉ tiêu drawdown (phụ lục 5 mục G) — tôi **báo** drawdown, tôi
  không đặt vạch cho nó.
- Không đo được "phần đuôi đáng bao nhiêu ở mức từng lệnh": `avg_mfe` là trường
  duy nhất thấy excursion, và nó đo tới giá THOÁT, nên trên một vị thế đã chốt
  một phần nó vẫn là excursion của **cả** đường đi, không của từng chân.
- Không đo họ tự quản (`Exits::Strategy`) — luật không chạm chúng, cố ý.

---

## Ghi chú thêm 2026-10-09 (sau khi chạy lưới, KHÔNG viết lại dòng nào ở trên)

Lưới đã chạy xong. Kết quả ở `docs/decisions/2026-10-09-partial-exit-result.md`,
receipt ở `receipts/partial/`. Bốn chỗ đăng ký này cần đọc kèm số đo:

1. **F0 bắn một nửa và điều kiện thứ hai của nó giữ được:** engine phải vá
   (~366 dòng code, trong khoảng khai ~330–380), và `--partial=off` tái hiện
   `agent/m4` đúng từng chữ số (271 lệnh / PF_usd 1,1175 / +0,0536R / TARGET 49
   / hold 128,4 min) với 27 test binary của `fd-backtest` qua hết. Nên tôi
   KHÔNG dừng.
2. **Phần test VƯỢT khai:** khai ~120 dòng, viết 350 dòng (11 test). Tổng
   insertions của commit `c3d746e` là 748 dòng. Khai 330–380 là khai phần CODE
   và nó đúng; phần test vượt 230 dòng.
3. **Dự đoán âm ở mục 3 SAI VỀ CƠ CHẾ.** `Lbar` tụt **−24,9%**, không phải về
   một phần ba, và **0/84 dòng có `Lbar < 0,250R`** nên chân expectancy **vẫn
   dư ở mọi dòng**. Trục chết vì `PF_r` cũng tụt (1,1259 → 0,9867), không vì
   hằng đẳng thức. Số đo thắng.
4. **Liều THẬT khác liều KHAI** vì `lot_step`/`min_lot` trên sổ 100 USD: khai
   0,25 → thật 0,170–0,237; khai 0,50 → thật 0,433–0,480. Mọi số đáp ứng đọc
   theo liều thật. Đăng ký này không lường trước điều đó.

Sổ đa phép thử: **khai 28 run / 84 row, xem 28 run / 84 row.**
