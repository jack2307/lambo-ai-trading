# Đăng ký trước — 09/10/2026: cỡ stop ở loại stop THI HÀNH

**Nhánh:** `agent/enforced-stop` (cắt từ `agent/stop-width`).
**Binary:** `/e/rust/fd-stop-width/target-sw/release/search.exe` (có `lbar_line`,
nên `PF_r = 1 + E/Lbar` đọc được từ receipt — phụ lục 7 mục A).
**Không build.** Không sửa một dòng `crates/`.

## 1. Giả thuyết, một câu

Ở một cơ chế mà cái stop **được engine thi hành** (`Exits::Engine`), nới cỡ stop
đổi **tập thoát** chứ không chỉ đổi đơn vị của R — nên tồn tại một cỡ stop mà
`PF_r ≥ 1,200` **và** `E ≥ +0,050R` **và** `n ≥ 40` trên **cả hai cửa sổ trong
arm có guards**; ở loại stop **MẪU SỐ** (`Exits::Strategy`) câu đó vô nghĩa vì
tập lệnh bất biến.

## 2. TIỀN KIỂM BẮT BUỘC — phân loại bằng MÃ, không bằng tên tham số

Phụ lục 7 mục B. Đọc `Exits::` trong `crates/fd-strategy/src/`, và `check_exit`
ở `crates/fd-backtest/src/engine.rs:770-777` (`if position.self_managed { return
None }`). **Tiền kiểm này tiêu 0 ô cổng.**

Năm họ của 57 dòng `gate-legs` (tên lấy từ
`receipts/gate-legs-table.txt` trên `agent/gate-legs`, **không thu hoạch lại**),
và cơ chế gốc của mỗi họ, truy từ chính file đăng ký của nó:

| họ (gate-legs) | base | file khai | `Exits::` | loại |
|---|---|---|---|---|
| `fx/us-long` | `session-hold` | `docs/hypotheses/2026-09-14-fx-local-hours.toml:20` | `session_hold.rs:52-54` `Strategy` | **MẪU SỐ** |
| `box/b2` | `volman-box` | `docs/hypotheses/2026-09-14-volman-box-vantage.toml` | `volman_box.rs:75-77` `Engine` | **THI HÀNH** |
| `crt-nocap/4h-mid` | `crt` | `docs/hypotheses/2026-10-04-crt-ceiling-lifted.toml:57` | `crt.rs:110-112` `Engine` | **THI HÀNH** |
| `pdhl/*` | `pdhl` | `docs/hypotheses/2026-09-13-pdhl.toml` | `pdhl.rs:74-76` `Engine` | **THI HÀNH** |
| `struct-80-f14` | `far-stop-break` | `docs/research/designs/2026-09-23-designed-1-contrast.toml:58` | `far_stop_break.rs:89-93` `Engine` | **THI HÀNH** |

⇒ `fx/us-long` là **đúng cùng một lớp** với `close/<window>` mà `agent/stop-width`
đã đo: nới stop ở đó đổi **0 tín hiệu, 0 lệnh, 0 USD**. **Không quét.** Báo phân
loại và bỏ qua. (Và nó là cùng một `session-hold` — cùng một file mã, cùng một
`riskDailyRanges`, nên kết luận của `stop-width` chuyển sang nguyên vẹn.)

## 3. TIỀN KIỂM THỨ HAI — "thi hành" chưa đủ: có **trục cỡ stop tách khỏi tín
hiệu** không?

Cũng tiêu **0 ô cổng**. Đọc chỗ `Intent::Enter { stop, target }` được dựng:

| base | stop dựng từ | target dựng từ | núm cỡ stop | tách khỏi tín hiệu? |
|---|---|---|---|---|
| `volman-box` | biên đối diện của box (`lo`/`hi`), `volman_box.rs:112-116` | `hi + boxes×h` | **KHÔNG CÓ** (`boxBars` đổi luôn cả tín hiệu) | **không** |
| `far-stop-break` | `stopMode 0` biên xa kênh / `stopMode 1` `stopAtr×ATR`, `far_stop_break.rs:117-123` | `risk × riskReward` | `stopAtr` (chỉ ở `stopMode 1`); `struct-80-f14` là `stopMode 0` ⇒ trục duy nhất là `period`, đổi luôn tín hiệu | **không ở ô đã công bố** |
| `pdhl` | mức hôm qua / đỉnh nến chạm **± `bufferPips×pipSize`**, `pdhl.rs:98-108` | `close ± risk × riskReward` | `bufferPips` | **có**, nhưng target **tỉ lệ với risk** |
| `crt` | `sweep_extreme ± bufferPips×pipSize`, `crt.rs:157-159` | `target=1`: `|entry − trung điểm c1|`, `crt.rs:186-193` — **một MỨC GIÁ, không phụ thuộc stop** | `bufferPips` | **có, và hình học TÁCH RỜI** |

⇒ **Ba lớp, không phải hai** — đây là phân biệt cấu trúc mà job này đăng ký:

    L1 MAU SO      (Exits::Strategy)                 : doi DON VI. 0 lenh doi.
    L2 THI HANH, hinh hoc TI LE  (target = rr x risk): stop va target gian
                                     CUNG nhau; RR bat bien; chi cost/R va do
                                     phan giai/dong ho doi.
    L3 THI HANH, target la MUC GIA (crt target=0/1)  : stop gian, target DUNG
                                     YEN ⇒ RR that su doi. Day la cau hoi
                                     chien luoc duy nhat trong nam ho.

`box/b2` và `struct-80-f14` **không có trục** ⇒ **không quét**, tiêu **0 ô**.
Quét chúng đòi **sửa mã** (thêm một tham số nhân cỡ stop), mà brief cấm thêm
trục sau khi thấy số, và một tham số mới thì không còn là cơ chế trong hồ sơ.

## 4. Đa phép thử — ĐẾM TRƯỚC

Chỉ hai họ còn lại được quét:

    pdhl (L2)  : 1 nhan (pdhl/fade-allday, mode 0, nhan cua dong n=50 trong
                 gate-legs) x 8 co stop x 2 cua so x 2 arm = 32 o
    crt  (L3)  : 1 nhan (crt-nocap/4h-mid)               x 8 co stop
                 x 2 cua so x 2 arm                        = 32 o
    ------------------------------------------------------------------
    TONG KHAI                                              = 64 o
    so lan goi binary: 2 ho x 2 arm = 4, cong 1 lan plumbing (khai o day,
    KHONG doc verdict tu no)

Tám cỡ stop, **khai trước, không đổi sau khi thấy số** — `bufferPips` ở
`pipSize = 0,1` nên đây là **USD đệm trên vàng**:

    bufferPips   1    3    10    30    60    100   200   400
    dem (USD)  0,1  0,3   1,0   3,0   6,0   10,0  20,0  40,0

`3` là giá trị mặc định của cả hai cơ chế (`pdhl.rs:52`, `crt.rs:86`) ⇒ ô chứng.
`1/3/6` là grid đã khai của `pdhl` (`pdhl.rs:60-63`); năm giá trị trên 6 là mở
rộng **một trục đã có**, không phải trục mới.

Mỗi họ chạy ở **trần `maxRiskAtr` đã khai của chính nó**, không chọn lại:
`pdhl` **3,0** (mặc định, `2026-09-13-pdhl.toml` không ghi đè) và `crt-nocap`
**1000** (`2026-10-04-crt-ceiling-lifted.toml`).

⚠️ **Khai trước một ràng buộc kế thừa:** header của
`2026-10-04-crt-ceiling-lifted.toml` tự tuyên **không verdict "survives" nào
được đọc từ bất kỳ dòng `crt-nocap`**, vì trần được nới sau khi thấy kết quả.
Job này **giữ nguyên** tuyên đó: một ô `crt-nocap` qua cổng ở đây **không phải
ứng viên**, nó chỉ là **đáp ứng theo cỡ stop**; muốn thành ứng viên thì phải
khai lại và chạy lại ở trần đã khai (`maxRiskAtr = 3,0`). Chín dòng của file đó
đã được đếm là chín lần xem; 32 ô ở đây là 32 lần xem **mới**.

Cửa sổ, lấy từ `[run]` đã khai của mỗi file, không chọn lại:

    pdhl : IS xauduka:5m  -> 2025-04-10        OOS xauusd:5m  (toan bo)
    crt  : IS xauduka:15m 2018-06-16->2022-06-16
           OOS xauusd:15m 2022-06-16->2026-09-18

Arm: `--guards` và không `--guards`. **Arm có guards là arm duy nhất chủ cho
phép** (phụ lục 5 mục D, phụ lục 7 mục D). Kết quả chỉ sống ở arm không-guards
⇒ **báo thẳng là không giao dịch được.**

## 5. Falsifier — cụ thể và bắn được

**F1 (tiền kiểm, bắn trước khi tiêu ô cổng).** Nếu với **mọi** cỡ stop, một họ
in **cùng một `trades`** *và* **cùng một `exits:`** (cùng số STOP / TARGET /
TIMEOUT tới từng lệnh), thì cái stop ở đó **không được thi hành dù khai
`Exits::Engine`**, và họ đó là **mẫu số** — bỏ, không đọc cổng.
Đọc ở: dòng `trades` và `--exit-mix` của cùng một receipt.

**F2 (chính).** Nếu ở các họ thi hành, nới stop đưa `Lbar` **qua 0,250R** mà
`PF_r` **sụp dưới 1,200 ở MỌI cỡ stop trên CẢ HAI cửa sổ trong arm có guards**,
thì hai chân của cổng **là một đánh đổi cứng ở loại stop thi hành** — khác hẳn
loại mẫu số, nơi `stop-width` tìm được ô thoả cả hai. Ghi đó là một **phân biệt
cấu trúc**, không phải một thất bại của cơ chế.

**F3.** Nếu `PF_r` của họ **L3** (`crt`, target là mức giá) **không** đổi nhiều
hơn `PF_r` của họ **L2** (`pdhl`, target tỉ lệ) trên cùng dải cỡ stop, thì phân
biệt L2/L3 của mục 3 **không có hệ quả đo được** và phải nói thế — ba lớp sụp
về hai.

## 6. Cách đọc — năm thứ bắt buộc, và không đọc gì khác

1. **`PF_r` cạnh `PF_usd`, luôn khai đơn vị.** Lấy từ dòng `Lbar ... PF_r ...
   PF_usd ... gap` mà `lbar_line` in. **Không suy `PF_r` từ receipt cũ** (phụ
   lục 7 mục A: vòng tròn). Cổng đọc bằng **`PF_r`**; `PF_usd` báo kèm.
2. **`max_drawdown_usd` cạnh mọi số lợi nhuận.** `_pct` là **SÀN**.
   `_pct > 100%` ⇒ **dòng đã cháy, không đọc PF của nó** (khuyết điểm 14).
3. **Cả hai arm**, và arm có guards là arm duy nhất chủ cho phép.
4. **Cỡ stop là hiện vật cửa sổ thứ 11.** Nếu `bufferPips` tối ưu khác nhau
   giữa IS và OOS thì **nói đúng thế**, kèm tỉ số.
5. **`cap_lots`**: đọc `sized down N` trong dòng guards; nếu ô cưỡi trần
   notional thì **báo tỉ lệ `N / trades`** — cả hai chân cổng không thấy nó
   (`r = points/risk` không đọc `lots`).

Thêm, và **không** dùng làm verdict: `null p50` cạnh mọi phân vị
(`null p50 = 0,000` nghĩa là **không calibrate được**); `count match` ngoài băng
⇒ **không công bố phân vị**; `wrong_side_stop` khác 0 ⇒ **dòng không đọc được**;
`_pct > 100%` ⇒ cháy. Verdict của tool kiểm sàn **30** lệnh, cổng desk là **40**
⇒ **đếm bằng tay**.

## 7. Cấm, nhắc lại cho job này

- **`data-sealed/` KHÔNG MỞ.** `struct-80-f14` chỉ tồn tại trong
  `docs/research/runs/2026-09-24-repair-c/era-era3-h4.txt`, mà receipt đó tự in
  `--data=E:/rust/flowdesk/data-sealed`. ⇒ **không tái lập được ô đó**, và đó là
  lý do thứ hai (sau "không có trục") để không quét nó.
- Không `taskkill` `collect.exe` (pid 5044 / 38720), không chạm `data/gold/`,
  `data/btc/`, `config/local.toml`, `main`, VPS.
- `--data=/e/rust/flowdesk/data` chỉ đọc. Bẫy: engine đọc
  `data/news/events.parquet` theo `data/` mặc định **bất kể `--data=`** —
  receipt phải ghi đúng nguồn dòng `news:` in ra.
- Scratchpad dùng chung ⇒ mọi file tạm mang tiền tố `enfstop-`.

## 8. Đĩa

`df -h /e` trước: **26 GB** còn trống (301G, 92%). Không build ⇒ không phình
`target/`. Chỉ dừng nếu dưới **3 GB**; đọc `df` là thông tin, không phải lệnh.
