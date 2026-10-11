# Đăng ký trước — `agent/cost-breakeven`: spread phải xuống bao nhiêu thì một dòng gần cổng QUA cổng?

Ngày 11/10/2026. Commit **một mình nó**, trước dòng code đầu tiên và trước
file thiết kế. Brief: `/e/rust/AGENT-BRIEF-2026-10-07-AUDIT.md` + phụ lục 5, 6,
7, 8, 9, 10. Worktree `/e/rust/fd-cost-breakeven`, nhánh `agent/cost-breakeven`
cắt từ `agent/stop-width` `29c8635`.

## 1. Giả thuyết, một câu

**Tồn tại một `spread*` không âm — nhỏ hơn 0,28 USD/vòng của config — mà ở đó
một dòng gần cổng nhất trong hồ sơ qua CẢ HAI chân cổng (`PF_r >= 1,200` VÀ
`E >= +0,050 R`) với `n >= 40`, trên CẢ HAI cửa sổ, ở arm guards ON.**

`spread*` là **spread nhỏ nhất trên lưới đã khai** thoả cả ba điều kiện đó.
Nếu không điểm nào trên lưới thoả thì `spread*` **không tồn tại** — in
`null`, không in 0 (brief §8).

## 2. Tiêu chí CHỌN DÒNG — khai TRƯỚC khi chạy, và nó là một bộ lọc số

    min( PF_r(cua so 1), PF_r(cua so 2) )  trong  [1,050 ; 1,200)
    VA  n >= 40 tren CA HAI cua so
    VA  arm guards ON
    VA  spread = 0,28 (gia tri config)

Đọc từ các receipt **đã công bố có dòng `lbar_line`** (nên `PF_r` là số in ra,
không phải số suy từ `PF_usd` — phụ lục 7 mục A cấm suy). Hai registry duy
nhất trên cây này có `PF_r` in ra ở arm guards ON trên hai cửa sổ:

- `agent/mould-target` `receipts/mould-target/C-{is,oos}-s28.txt` (153 dòng khai,
  149 dòng đọc được `lbar_line`, 145 dòng có `n >= 40` ở cả hai cửa sổ)
- `agent/hour-screen` `receipts/hourscreen/gate-W{1,2}-guardson.txt` (8 dòng)

Bộ lọc **không nhìn** `E`, `Lbar`, drawdown, verdict, hay bất kỳ số nào về chi
phí — nó chỉ nhìn `PF_r` và `n`. Lý do: `PF_r` là chân **bất biến với mẫu số**
(phụ lục 7 mục B + kết quả `hour-screen`), nên "gần cổng" đo bằng `PF_r` là
cách duy nhất không bị cỡ stop khai báo bẻ.

**Kết quả của bộ lọc, chạy trước mọi lần gọi `search.exe` của nhánh này** — 10
dòng từ `mould-target` + 1 dòng từ `hour-screen` = **11 dòng ứng viên**:

| dòng | base | min `PF_r` | n (W1/W2) | `Lbar` | chân ràng buộc |
|---|---|---|---|---|---|
| `qs-lb3` | quiet-swing lookbackSessions=3 | 1,1730 | 494 / 452 | 0,208 | expectancy |
| `ts-l5` | tsmom lookbackDays=5 | 1,1048 | 749 / 744 | 0,169 | expectancy |
| `ts-l20` | tsmom lookbackDays=20 | 1,1005 | 612 / 618 | 0,195 | expectancy |
| `ts-l10` | tsmom lookbackDays=10 | 1,0926 | 628 / 649 | 0,182 | expectancy |
| `qs-lb30` | quiet-swing lookbackSessions=30 | 1,0913 | 398 / 344 | 0,288 | PF (Lbar>=0,250) |
| `tsmom` | tsmom mặc định (lookback 60) | 1,0886 | 527 / 538 | 0,218 | expectancy |
| `qs-h15` | quiet-swing holdSessions=15 | 1,0741 | 393 / 374 | 0,243 | expectancy |
| `qs-qp020` | quiet-swing quietPct=0,20 | 1,0712 | 241 / 209 | 0,240 | expectancy |
| `qs-h30` | quiet-swing holdSessions=30 | 1,0672 | 393 / 374 | 0,243 | expectancy |
| `qs-h2` | quiet-swing holdSessions=2 | 1,0523 | 521 / 480 | 0,191 | expectancy |
| `ovn1900/long` | session-hold 19:00→02:00 NY, MoTuWeTh | 1,0703 | 1644 / 1635 | 0,087 | expectancy |

Ba ứng viên brief nêu đều **có mặt hoặc được giải thích**:
- `qs-lb3` — vào, và nó là dòng gần cổng nhất của hồ sơ theo bộ lọc này.
- `ovn1900/long` — vào.
- `level-reversion` — **KHÔNG vào, và lý do là tiêu chí, không phải kết quả**:
  nó chỉ có **MỘT cửa sổ** (basis cuộn), nên nó trượt điều kiện "n >= 40 trên
  cả hai cửa sổ" của chính bộ lọc; thêm nữa nó là `needs_options` nên nó cần
  một timeline mà cả `mould-target` lẫn job này đều không nạp
  (`timeline: none` in trong mọi header). Nếu nó được chạy sau thì **phải nói
  thẳng là một cửa sổ** (brief yêu cầu) — ở đây nó **không chạy**.

**Ba dòng đối chứng** (khai cùng lúc, đếm vào ô, KHÔNG phải ứng viên):
- `px-1s` — mốc parity/ruler, phải tái tạo số của `mould-target`.
- `ovn1900/short` — **probe cố ý nhìn trước** (phụ lục 10 mục VII): nó dùng
  **đúng cùng giờ, cùng phơi nhiễm, cùng sizing** với `ovn1900/long` chỉ đổi
  chiều, và nó in `PF_r 0,6459`. Nó **PHẢI trượt** mọi điểm spread không âm.
  Nếu nó qua thì phép đo của tôi không đo chi phí mà đo một thứ khác.
- `ema-cross` — probe thứ hai, dòng lỗ sâu ở arm này (`G` −3,091). Cũng phải
  trượt mọi điểm không âm.

## 3. Falsifier — cụ thể và bắn được

**F1 (falsifier của brief).** Nếu `spread*` của **mọi** dòng trong 11 ứng viên
nằm **dưới 25% của 0,28 = 0,0700 USD/vòng** (kể cả khi `spread*` không tồn tại
ở bất kỳ spread không âm nào), thì **"chi vào CHI PHÍ" chết trên vàng 15m**:
không cơ cấu hoàn phí nào bù được bốn lần, và desk thôi hỏi hướng đó.

**F1b (phía ngược).** Nếu **có** dòng với `spread* >= 60% của 0,28 = 0,1680
USD/vòng**, đó là con số đầu tiên của hồ sơ chỉ vào một việc làm được, và nó
phải được in kèm mọi cảnh báo của mục 5 và 7.

**F2 (parity — đối chứng DƯƠNG, phụ lục 10 mục VII).** Chân `--spread=0.28`
của tôi phải tái tạo `n`, `PF_r`, `PF_usd`, `E`, `Lbar`, `sized down` và
drawdown USD đã công bố cho **13** dòng (11 ứng viên + `px-1s` + `ovn1900/short`).
Nếu không khớp thì theo phụ lục 8 mục II đó là **bình thường ở hồ sơ này** —
tôi báo nó và **số của tôi thắng** (brief §8), nhưng mọi `spread*` phải được
đọc trên thang của chính tôi, không trộn với số đã công bố.

**F3 (đối chứng ÂM — quét có thật sự đọc chi phí không).** Trên **mọi** dòng,
`E` phải **tăng đơn điệu** khi spread giảm trên lưới. Nếu một dòng không đơn
điệu, dòng đó không phải một phép đo chi phí sạch và `spread*` của nó là xấp xỉ
(và nguyên nhân gần nhất là F4 dưới đây). Và hai probe `ovn1900/short`,
`ema-cross` **phải không có `spread*`** ở mọi điểm không âm.

**F4 (tập lệnh đổi — `F7` của `mould-target`).** `mould-target` đo được quét
spread đổi tập lệnh ở **168/594 ô**, chia **17/296 (5,7%) guards-off** vs
**151/298 (50,7%) guards-ON**. Tôi chạy **chỉ** arm guards ON, nên tôi phải
**in `n` ở MỌI điểm spread** và đánh dấu mọi dòng có `n` đổi. `spread*` của một
dòng có `n` đổi là một xấp xỉ, và tôi nói ra.

**F5 (khuyết điểm 16 — `delta` nửa spread rò vào đơn vị rủi ro).**
`risk = f × biên_độ_ngày + delta`, `delta = s/2` theo phụ lục 7 mục C
(`R² = 0,99685`, `delta = 0,138 USD` ở `s = 0,28`). Tôi khớp
`risk(s) = A + k·s` **trên lưới của chính tôi, từng dòng từng cửa sổ**, in `k`
và `R²`. Nếu `k` không nằm trong `[0,30 ; 0,70]` thì phụ lục 7 mục C **không**
mô tả đúng cây này và cách đọc phải là cách ghim đơn vị. Và tôi in `spread*`
**hai lần**: (a) như engine tính (đơn vị R trôi theo spread) và (b) **ghim đơn
vị R ở giá trị của `s = 0,28`**, vì `PF_r` bất biến với `delta` còn `E` thì
không — nên khuyết điểm 16 chỉ bóp đúng chân đang ràng buộc ở 10/11 ứng viên.

**F6 (cháy).** `max_drawdown_pct > 100%` ⇒ dòng đã cháy (phụ lục 6 mục IV,
khuyết điểm 14) ⇒ không đọc `PF` của nó, loại khỏi mọi `spread*`.

**F7 (cửa sổ).** Nếu `spread*` tồn tại ở một cửa sổ mà không ở cửa sổ kia, đó
là **hiện vật cửa sổ thứ 13**, không phải một dòng qua cổng. Cổng đòi cả hai.

## 4. Lưới spread — và tại sao có cả phía âm

**Lưới chính, 8 điểm, đây là CÂU HỎI:**

    --spread= 0.28  0.24  0.20  0.16  0.12  0.08  0.04  0.00

**Lưới ĐỐI CHỨNG, 6 điểm, KHÔNG phải câu hỏi và loại khỏi cổng:**

    --spread= -0.04 -0.08 -0.12 -0.16 -0.20 -0.28

Lý do chạy phía âm: nếu `spread* ` không tồn tại ở `s = 0` thì câu trả lời
"không có spread nào đủ" cần một **độ lớn**, và brief cấm suy bằng đại số
(mục 1: "quét THẬT, không phải một phép chia"). Một `--spread=` âm là
`apply_costs` chạy ngược dấu (`engine.rs:902-912`), nên nó **đo** chỗ `E` cắt
+0,050 R thay vì ngoại suy tuyến tính.

⚠️ **Spread âm KHÔNG phải một khoản hoàn phí.** Nó cải thiện **giá khớp** và
đồng thời **co `risk`** qua `delta = s/2`; một khoản hoàn phí tiền mặt thì cộng
USD mà không chạm giá khớp lẫn đơn vị R. Mọi số ở phía âm được in dưới nhãn
**ĐỐI CHỨNG**, là một yêu cầu kỹ thuật về độ lớn, **không** phải một giá đi mua
được và **không** phải một mức hoàn phí.

## 5. `spread*` là một con số KỸ THUẬT — khai trước để không ai đọc sai

- Nó **không phải** một mức giá đi mua được. Tôi **không biết** sàn nào cho
  spread nào, và **cơ cấu hoàn phí/commission của desk là thứ chủ biết, không
  phải thứ tôi đo**. Tôi in `spread*` cạnh spread hiện hành của chính config để
  chủ tự so. Tôi **không** viết "đạt được" hay "không đạt được".
- **0,28 là ĐO hay GIẢ ĐỊNH?** `config/default.toml:308-333` tự khai: 0,28 là
  **một lần đọc duy nhất** từ terminal VantageMarkets-Live 21 ngày 12/09/2026
  (`XAUUSD.sc` 28 điểm). Cùng khối chú thích đó ghi tiếp rằng `py/live/spread_log.py`
  **đã lấy 2.315 mẫu trong 12,8 giờ** và phân phối **thấp và hẹp hơn**:
  **p10 0,210 / p50 0,220 / p90 0,220 / max 0,260** — tức **0,28 nằm trên MỌI
  spread từng quan sát được và tính đắt khoảng 27%**. Nó không được sửa trong
  config vì mọi receipt trong `docs/decisions/` đã đo ở 0,28. ⇒ Khi báo
  `spread*` tôi phải in **ba mốc**: `0,28` (config, 1 mẫu), `0,220` (p50 của
  2.315 mẫu), `0,260` (max đã quan sát). Và nói rõ mẫu 12,8 giờ **không phủ**
  giờ rollover 21:00 UTC lẫn một ngày cuối tuần.
- `markets.btcusd` ghi `spread = 17.05 # MEASURED: 3.4x the 5.00 assumed`, nên
  cây này **có** phân biệt ĐO/GIẢ ĐỊNH trong chú thích — và với `xauduka` câu
  trả lời là **ĐO, nhưng một mẫu, và mẫu lớn hơn nói nó quá cao**.

## 6. Sổ đa phép thử — khai TRƯỚC

| | khai |
|---|---|
| dòng | **14** (11 ứng viên + 3 đối chứng) |
| cửa sổ | **2** (2010-06-01→2018-06-01, 2018-06-01→2026-06-01) |
| arm guards | **1** — chỉ guards ON (arm duy nhất chủ cho phép) |
| điểm spread chính (>= 0) | **8** |
| điểm spread đối chứng (< 0) | **6** |
| ô chính | 14 × 2 × 8 = **224** |
| ô đối chứng | 14 × 2 × 6 = **168** |
| tổng ô | **392** trong **28 lần chạy** |
| ô cổng | **0 bây giờ**; chỉ dòng nào qua cả hai chân trên cả hai cửa sổ mới tiêu |

**Không quét cỡ stop.** Bẫy 2 của phụ lục 10: ở arm guards ON "stop tự-quản chỉ
là mẫu số" **SAI** — lệch tới 81% và số lệnh đổi (1982 → 1962) vì
`max_open_loss_r = 2.0` tính bằng R và `notional 300%` chặn
`lots = risk_usd/(risk × contract_size)`. Nên **cỡ stop cố định ở giá trị của
chính dòng** và **chỉ spread di chuyển**. 10/11 ứng viên là `Exits::Strategy`
(tự quản) và đó là lý do thứ hai.

**Không công bố phân vị.** `--seeds=2` truyền chỉ vì mode đòi một con số;
`null p50` của hồ sơ dưới 1, `count match` ngoài băng ở các dòng này
(`qs-lb3` 2,96), và `SURVIVES` phụ thuộc chỗ ngồi trong file TOML. Không một
phân vị nào được đọc hay in.

## 7. Thứ sẽ KHÔNG đo được, khai trước

1. **Spread thật theo giờ.** Mọi điểm trên lưới là spread **phẳng ở mọi giờ**.
   Sàn thật nới spread rộng nhất đúng ở mốc nghỉ; `ovn1900/long` vào lệnh
   19:00 NY, hai giờ sau mốc 17:00 ⇒ mọi `spread*` của nó là **lạc quan**.
2. **Commission và hoàn phí.** Không có trong engine
   (`commission_per_lot = 0.0`). `spread*` chỉ là spread.
3. **Excursion trong lệnh.** Mọi drawdown là đường vốn **đã đóng lệnh**;
   `avg_mae` là trường duy nhất thấy excursion, tôi in nó.
4. **`wrong_side_stop`** — không có bộ đếm trên cây này (phụ lục 8 mục III.2).
   Không một số 0 nào được đọc từ nó.
5. **`data-sealed/`** — không mở, không trỏ tới, không đếm.
6. **Instrument khác và cỡ nến khác.** Mọi số là `xauduka` 15m.
7. **`level-reversion`** — một cửa sổ, cần timeline options; không chạy
   (mục 2).
8. **`max_hold_ms`** — không đổi, không có cờ CLI.

## 8. Cài đặt, đóng băng

    --mode=hypotheses --fixed --exit-mix --seeds=2 --guards
    --interval=15m --data=/e/rust/flowdesk/data --market=xauduka
    --config=/e/rust/fd-cost-breakeven/config
    --batch-file=/e/rust/fd-cost-breakeven/docs/research/designs/2026-10-11-cost-breakeven.toml
    --from=<cua so> --to=<cua so>
    --spread=<14 diem tren luoi>

Binary: **`/e/rust/fd-stop-width/target-sw/release/search.exe`** (build
08/10/2026 20:57 từ `agent/stop-width` `29c8635`, **chính là HEAD của worktree
này**) ⇒ **không build gì**. Nó là binary dùng chung duy nhất có đồng thời
`lbar_line`, dòng drawdown và `expectancy_net`, và job này cần cả ba.
`--exit-mix` bật để **kiểm luật của mỗi dòng có nổ** (brief §6a).

Mọi receipt phải in lại dòng
`flags: N passed, every one of them read by --mode=hypotheses`, và dòng `news:`
được in lại trong báo cáo vì engine đọc `data/news/events.parquet` từ `data/`
**mặc định** bất kể `--data=`.

Không truyền: `--samples=`, `--direction-samples=`, `--rebate-share=`,
`--trail=`, `--params=`, `--filters=`, `--strategy=`, `--batch=`,
`--companion=`, `--null-sides=`, `--null-registered-stop`.

Không chạm: `config/accounts.toml` ngoài worktree, `config/local.toml`, VPS
103.19.29.194, `main`, `data/gold/`, `data/btc/`, `data-sealed/`,
`/e/rust/flowdesk/target/`, và hai process `collect.exe` pid **5044** / **38720**.
Không xoá `target/` nào. `df -h /e` trước lần chạy đầu: **17 GB available**.

## 9. Cách đọc kết quả

Một con số là kết quả: **`spread*` nhỏ nhất trên lưới không âm mà một trong 11
ứng viên qua cả hai chân cổng trên cả hai cửa sổ ở arm guards ON** — kèm tên
dòng, `n` ở mọi điểm spread, `PF_r` cạnh `PF_usd`, `E = total_r/n`, `Lbar`,
drawdown **USD**, tỉ lệ `sized_down_by_guard`, exit mix, và `k`/`R²` của khớp
`delta`.

* **`spread*` không tồn tại ở bất kỳ `s >= 0`, trên mọi dòng** — F1 bắn ở dạng
  mạnh nhất: chi phí **không phải** thứ chặn các dòng này; kể cả giao dịch
  **miễn phí** chúng vẫn trượt. Lúc đó con số đáng giữ là **thiếu bao nhiêu R ở
  `s = 0`**, và lưới đối chứng âm nói độ lớn của khoản bù cần có.
* **`spread*` tồn tại nhưng `< 0,0700`** — F1 bắn: phải rẻ hơn 4 lần trở lên.
  "Chi vào chi phí" chết.
* **`spread*` trong `[0,0700 ; 0,1680)`** — khoảng xám. In số, không khuyến nghị.
* **`spread*` >= 0,1680** — F1b: con số đầu tiên của hồ sơ chỉ vào một việc làm
  được. In kèm cả ba mốc spread của mục 5 và mọi cảnh báo của mục 7.
