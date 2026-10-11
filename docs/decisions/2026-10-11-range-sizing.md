# Đăng ký trước — đặt CỠ STOP bằng BIÊN ĐỘ DỰ BÁO của nến tới

**Ngày:** 2026-10-11. Nhánh `agent/range-sizing` (cắt từ `agent/stop-width`
`29c8635`). Commit này **chỉ docs**, trước dòng code đầu tiên.

**Giả thuyết, một câu:** một stop tỉ lệ với **biên độ DỰ BÁO** của nến 15m tới —
dự báo dựng từ nến 1m đã đóng, thứ `agent/intrabar-shape` đo được với `t` phần dư
**+28,303 (W1) / +38,060 (W2)** trên `max_1m_share` — cho `PF_r` cao hơn một stop
ATR **cùng cỡ trung bình**, trên **cả hai cửa sổ**, ở **arm có guards**.

**Vì sao trục này khác một phép đổi đơn vị:** phụ lục 7 §B đo được rằng ở cơ chế
**tự quản thoát** (`Exits::Strategy`), `check_exit` (`engine.rs:772-777`) trả
`None` trước mọi thứ ⇒ nới/thu stop đổi **0 tín hiệu, 0 lệnh, 0 USD**. Trục này
vì thế chạy trên một cơ chế **`Exits::Engine`**, và dòng mã khai nó được dẫn ở §2.

---

## 1. Cơ chế — khai bằng `Exits::` trong MÃ, không bằng tên tham số

Một cơ chế **MỚI**, `range-stop`, viết trong `crates/fd-strategy/src/range_stop.rs`,
khai `fn exits(&self) -> Exits { Exits::Engine }`. Lý do viết mới chứ không dùng
`donchian-breakout`: cơ chế đó tự phát `Intent::Exit` ở đường giữa kênh, nên cỡ
stop không phải thứ duy nhất quyết định kết cuộc. `range-stop` **không có exit
riêng nào** — stop, target và `max_hold_ms` là của engine, nên cỡ stop là **biến
duy nhất** giữa các arm.

    vao   : close(i) > donchian_upper(i)  -> LONG
            close(i) < donchian_lower(i)  -> SHORT      (period = 20, mac dinh registry)
    stop  : d(i) = stopAtr x ATR14(i) x m(i)
    target: close(i) +/- reward_risk x d(i)             (L2, [trading] reward_risk = 1.8)
    thoat : CHI stop / target / max_hold_ms = 4h = 16 nen 15m

`m(i)` là **cỡ số nhân stop**, lấy từ `cmp.close` (nến companion khớp **chính
xác** dấu thời gian, `NaN` khi thiếu — `fd-indicators/src/companion.rs`
`aligned_change`, `// missing, not zero and not the previous value`). Bốn nguồn:

| nhãn | `m(i)` | vai |
|---|---|---|
| `atr` | **1,000** (hằng) | **ĐỐI CHỨNG (a)** — stop ATR, cùng cỡ trung bình |
| `fc15` | dự báo biên độ từ **6 đặc trưng 15m** | cô lập phần 1m |
| `fc1m` | dự báo biên độ từ **6 đặc trưng 15m + 8 đặc trưng 1m** | **PHƯƠNG PHÁP** |
| `shuf` | `m` của `fc1m` **xáo trộn trong thời gian** | **ĐỐI CHỨNG (b)** |

**Điều khoản quyết định tính so sánh được:** cả bốn nhãn **từ chối lệnh khi
`cmp.close` là `NaN`**, kể cả `atr` (nó không đọc giá trị đó, nhưng nó đọc *sự tồn
tại* của nó). Vì **4,9209%** nến 15m thiếu nến 1m (18.638 nến, đo bởi
`intrabar-shape`), nếu `atr` chạy trên mọi nến thì phép so là phép so **hai mẫu
khác nhau**. Bốn nhãn chạy trên **đúng cùng một tập nến**.

## 2. Dự báo — nhân quả bằng cấu trúc, không bằng lời

- **Kết cuộc mô hình:** `y(i) = (high(i+1) − low(i+1)) / ATR14(i)` — đúng đại
  lượng `intrabar-shape` trục G đo, và nó `NaN` khi nến `i+1` không cách đúng
  900.000 ms (qua cuối tuần / nghỉ phiên ⇒ **từ chối, không đo**).
- **Đặc trưng 15m (6):** `ret_15 · body · uw · lw · close_pos · rng_atr`, **y
  nguyên** bộ `CONTROL` của `intrabar-shape`.
- **Đặc trưng 1m (8):** `tpo_skew · tpo_poc_pos · t_high · t_low ·
  extreme_order · path_eff · late_push · max_1m_share`, **cả tám**, không chỉ ba
  cái thắng đêm qua — chọn hậu nghiệm là chính cái brief cấm.
- **Mô hình:** OLS. **Khớp đi bộ tiến:** khớp lại ở **mốc đầu mỗi tháng lịch**,
  chỉ dùng dòng có dấu thời gian **nhỏ hơn** mốc đó, tối thiểu **20.000** dòng
  trước dự báo đầu tiên. Mọi dự báo là **ngoài mẫu**.
- **Chuẩn hoá cỡ:** `m(i) = pred(i) / trung_bình_trượt(pred, 2000 dự báo hữu hạn
  gần nhất tính CẢ i)`, rồi **kẹp [0,50 ; 2,00]**. Trung bình trượt làm
  `mean(m) ≈ 1` ⇒ **cỡ stop trung bình khớp nhau ở cùng `stopAtr`**, và §5 đo nó
  chứ không giả định.
- **Xáo trộn:** hoán vị các giá trị `m` hữu hạn **trong thời gian**, seed khai
  trước `20261011`, **giữ nguyên vị trí `NaN`** ⇒ cùng tập nến, cùng phân phối cỡ
  stop, **mất hết thông tin thời điểm**.
- **Volume:** `read_bars()` truyền `columns=["time","open","high","low","close"]`.
  Cột `volume` của `XAUDUKA-1m` **null trên 5.635.777/5.635.777 dòng** và 15m
  mang một số 0 chế ra (phụ lục 9 §III) ⇒ **không mở nó một lần nào**.
- **Đường truyền vào engine:** ba parquet companion
  (`XAURFC15`, `XAURFC1M`, `XAURFCSH`) ở `data-rs/bars/`, mỗi dòng
  `time = t15`, `open = high = low = close = m(i)`, nạp bằng `--companion=`.
  Dòng `companion:` của receipt khai file nào đã nạp; **không nạp gì thì mọi
  `cmp` series là `NaN`** và cơ chế **không vào lệnh nào** — không phải vào lệnh ở
  một số 0 chế ra.

**Tính nhân quả, dựng theo đúng cách bốn agent đêm qua làm:**

- **C1** — cắt chuỗi ở **5 mốc**, mọi giá trị trước mốc phải **bằng từng bit**
  (`NaN == NaN`), cho cả 14 đặc trưng, `pred`, và `m`.
- **C2** — **probe CỐ Ý NHÌN TRƯỚC phải TRƯỢT C1**: (i) `PROBE_peek_m16` đọc
  close **phút 16** (phút đầu của nến `i+1`); (ii) `PROBE_y` đưa chính `y(i)` vào
  làm biến hồi quy. Nếu probe **không** trượt thì C1 **không chứng minh gì** và
  không có con số nào được công bố.
- **C3** — `y` không đo qua khe; nến thiếu nến 1m ⇒ `m = NaN`, **không điền,
  không về 0**.

## 3. Cổng, hai cửa sổ, hai arm

    PF >= 1,200  VA  expectancy >= +0,050R  VA  >= 40 lenh   tren CA HAI cua so

Cửa sổ, **đúng hai cửa sổ của hồ sơ** (phụ lục 5 §B, nơi drift lật dấu
−0,0040R → +0,0205R mỗi phiên), đặt bằng **cờ CLI** `--from=`/`--to=` vì `[run]`
không đặt được cửa sổ dưới `--fixed`:

    W1  2010-06-01 -> 2018-06-01       W2  2018-06-01 -> 2026-06-01

Arm: `--guards` **ON** (arm **duy nhất** chủ cho phép, phụ lục 5 §D) và OFF.
Kết quả chỉ sống ở arm OFF **sẽ được nói thẳng là không giao dịch được**.

**Đọc bằng `PF_r`, không bằng `PF_usd`**, và in **cả hai mọi dòng** (phụ lục 10
§V: bảy phép đo, ba kết quả về chiều của cái khe ⇒ **không trích tỉ lệ của ai**).
Cạnh mỗi số lợi nhuận: **`E = total_r/n`** (3 chữ số của `expectancy` không đủ),
**`Lbar`**, **drawdown USD** (`_pct` là SÀN; `_pct > 100%` ⇒ **dòng đã cháy, không
đọc PF**), **`--exit-mix`**, và **tỉ lệ `sized_down_by_guard`** (field, không grep)
ở **MỌI** ô. **Đếm tay 40** — tool kiểm `need 30`.

**Luật của cơ chế phải NỔ:** nếu `--exit-mix` cho STOP ≈ 0 lần thì cái stop
**không phải một stop** (`stop-width` F4: ở `f ≥ 1,000` nó nổ 0–3 lần trong 1.355
lệnh) và ô đó **không đọc được như một phép đo về cỡ stop**. Báo tỉ lệ STOP ở mọi ô.

**Khuyết điểm 16, và nó ở ngay trong trục này:** `risk = f × thước + δ` với
`δ = 0,138 USD = đúng nửa spread 0,28 vòng` (R² 0,99685), vì `sizing_stop` lấy
`bar.close` nến tín hiệu còn `entry` đã qua `apply_costs`. Stop của trục này
**biến thiên theo nến** ⇒ `δ` là một phần **biến thiên** của `R`. **Đo và nói ra**:
`δ / d(i)` ở phân vị 10 và 90 của `d`.

**Chi phí/R, bốn thứ phải nói** (phụ lục 9 §II): vàng trên **hai cửa sổ này** ở
`1,5 ATR(15m)` đọc **12,58% (W1) / 8,35% (W2)**; con số 4,04% lưu hành là trung vị
của 2 năm gần nhất, **sai cửa sổ**. Ở `stopAtr = 1,0` chi phí/R sẽ **lớn hơn 1,5
lần nữa**. Spread 0,28 là **cấu hình `markets.xauduka`**, không phải một phép đo
của trục này.

**Trần `max_hold_ms = 4h` (16 nến)** là trần của `[trading]`, không có override cho
`xauduka` — và phụ lục 10 §I nhắc rằng chiều của một kết quả có thể là thuộc tính
của đúng cái trần đó. Nó **không** được nới ở trục này; nó được **khai**.

**Phân vị KHÔNG công bố.** `SURVIVES` phụ thuộc chỗ ngồi trong file TOML (null p95
1,297 vs 2,036 trên cùng một phương pháp) và `matched_rate` là tung xu ở cổng hẹp.
`seeds = 20` chỉ để dòng `null p50` tồn tại; **cổng là cổng**.

## 4. Falsifier — cụ thể và bắn được

- **F1 (của brief).** `fc1m` **không** đánh bại `atr` ở `PF_r`, cùng `stopAtr`
  (⇒ cùng cỡ stop trung bình, kiểm ở §5), trên **cả hai** cửa sổ, ở **arm có
  guards** ⇒ **tín hiệu biên độ của 1m là THẬT nhưng KHÔNG TIÊU ĐƯỢC**. Vạch:
  `ΔPF_r = PF_r(fc1m) − PF_r(atr) > 0` ở cả W1 và W2, ở **cả hai** `stopAtr`.
- **F2 (đối chứng b).** `shuf` tái tạo **≥ 50%** của `ΔPF_r` của `fc1m` ở cửa sổ
  nào thì **trục đóng ở cửa sổ đó** (tiền lệ: `vprofile-gold` 45%,
  `companion-regime` 89,6–90,4%).
- **F3 (cô lập phần 1m).** `fc1m` không đánh bại `fc15` ⇒ dù "dự báo thắng ATR",
  **phần 1m** không mua được gì; `intrabar-shape` đã đo `R²(1m ~ 15m)` = 0,14–0,93
  ở sáu trong tám đặc trưng.
- **F4 (dụng cụ, ở TIỀN KIỂM).** C1 trượt, **hoặc** C2 probe **không** trượt ⇒
  **không công bố con số nào**, tiêu 0 ô cổng.
- **F5 (cỡ hiệu ứng, ở TIỀN KIỂM — vạch khai TRƯỚC khi thấy số).**
  - **F5a:** nếu tỉ số liên-thập-phân-vị `p90(m)/p10(m) < 1,20` thì một "stop dự
    báo" nằm trong 20% của một stop phẳng trên 80% số nến ⇒ **trục không có chỗ
    để tiêu tiền**, báo và tiêu **0 ô cổng**.
  - **F5b:** nếu `corr(m_fc1m, m_fc15) ≥ 0,995` thì hai nguồn là **cùng một
    stop** ⇒ F3 không đo được bằng lệnh; vẫn chạy nhưng **nói thẳng** rằng phần
    1m không phân biệt được ở lớp cơ chế.
- **F6 (hiện vật cửa sổ thứ 13).** Kết quả sống ở **một** cửa sổ ⇒ **hiện vật,
  không phải cơ chế** (12 hiện vật đã đếm, phụ lục 9 §IV).
- **F7 (cháy).** Ô nào in `max_drawdown_pct > 100%` ⇒ **dòng đã cháy**, báo và
  **không đọc PF của nó** (khuyết điểm 14).

## 5. Đối chứng CÙNG CỠ TRUNG BÌNH — đo, không giả định

Phụ lục của brief nói đúng: một stop rộng hơn trung bình đổi `PF_r` kể cả khi dự
báo vô nghĩa, và `agent/stop-width` đã đo **lớp đó** (10 cỡ stop, 200 ô). Nên
trước mọi phép so, tiền kiểm in **cỡ stop trung bình đo được** của bốn nguồn,
**trên đúng tập nến tín hiệu** (luật vào lệnh của `range-stop` được dựng lại
trong Python):

    mean( d_atr )   mean( d_fc15 )   mean( d_fc1m )   mean( d_shuf )      (USD, moi cua so)

Vạch khai trước: **lệch ≤ 2,0%** thì gọi là cùng cỡ. Lệch hơn thế thì
`stopAtr` của nhánh dự báo được chia cho tỉ số đo được, **một hằng số mỗi cửa
sổ, khai ra**, và cả hai phiên bản được báo. Kiểm chéo từ phía engine:
`avglots` ở arm **không**-guards tỉ lệ `1/mean(risk)` nên `avglots × mean(d)`
phải đi ngang giữa bốn nhãn — và **không** dùng thước `risk_usd = |swap$|/|Δtotal_r|`
(phụ lục 10 §III: nó sai **2,22 lần**, là trung bình điều hoà có trọng số swap).

**Đối chứng DƯƠNG, bắt buộc** (phụ lục 10 §VII): chứng minh dụng cụ **nhìn thấy
được** trước khi báo một số âm. Hai cái:
1. **Tiền kiểm:** `R²_oos` của `fc1m` trên `y` phải **dương** và **lớn hơn**
   `fc15` ở cả hai cửa sổ — `intrabar-shape` đo `ΔR²_oos` dương **4/4 fold**. Nếu
   nó không tái tạo được thì dụng cụ của tôi khác dụng cụ của họ và tôi báo điều đó.
2. **Engine:** cùng nhãn `atr` phải in **số y hệt** ở cả ba lần chạy companion
   (`XAURFC15` / `XAURFC1M` / `XAURFCSH`) — nếu nó đổi thì việc nạp companion
   đang rò vào nhãn không đọc nó.

## 6. Sổ đa phép thử — khai TRƯỚC, đếm ô

**Tiền kiểm (Python, 0 ô cổng):**

    C1 x 5 moc x (14 dac trung + pred + m)          : 0 phep thu (kiem dinh, khong p-value)
    C2 hai probe x 5 moc                            : 0
    R2_oos di bo tien: 2 mo hinh x 2 cua so         : 4 so
    corr(pred, y) thuc te: 2 mo hinh x 2 cua so     : 4 so
    phan phoi m (p10/p50/p90/mean): 3 nguon x 2 cs  : 6 (mo ta)
    corr(m_fc1m, m_fc15): 2 cua so                  : 2 so
    co stop trung binh tren nen tin hieu: 4 x 2      : 8 so
    delta/d(i) o p10 va p90 cua d: 2 x 2 cua so      : 4 so

**Cổng (Rust, `--mode=hypotheses`, `--fixed`):**

    4 nguon stop x 2 stopAtr {1,0 ; 2,0} x 2 cua so x 2 arm  =  32 O CONG

    12 lan chay: 3 file companion x 2 cua so x 2 arm
    moi lan chay in 4 nhan {atr, fc} x 2 stopAtr = 8 dong
    => nhan `atr` xuat hien 3 lan moi (cua so, arm): 8 dong TRUNG LAP
       chung la DOI CHUNG DUONG so 2 cua §5, khong phai o moi.

    O CONG KHAI: 32.  Khong them o nao sau khi thay so. Them truc nao thi
    GHI CHU CO NGAY vao CUOI file nay, truoc khi chay no.

## 7. Cách đọc — viết trước khi thấy số

- **Con số quyết định là `ΔPF_r` ở arm có guards, trên cả hai cửa sổ.** Mọi thứ
  khác là bối cảnh.
- `PF_usd` in cạnh nhưng **không** quyết định gì (nó là thuộc tính của đòn bẩy, và
  `cap_lots` + `max_open_loss_r = 2.0` + `notional 300%` cắn mạnh hơn khi stop
  hẹp hơn — **đúng cái bẫy 2 của phụ lục 10**: `px-1s` lệch tới **81%** ở guards
  ON so với 1,5–3,9% ở OFF).
- **Dự báo của tôi cố tình làm stop thay đổi ⇒ nó đi thẳng vào trần đó.** Tỉ lệ
  `sized_down_by_guard` được báo ở **mọi** ô, và nếu nó khác nhau giữa `atr` và
  `fc1m` thì **một phần `ΔPF_usd` là trần, không phải cơ chế** — đó chính là lý do
  `ΔPF_r` là con số quyết định.
- Nếu F1 bắn **và** F3 bắn thì kết luận là lớn: **cả họ "độ phân giải mịn hơn"
  đóng** — 1m không mua được gì ở chân trời 15m, **ở chiều** (đêm 10/10) **lẫn ở
  cỡ** (đêm nay).
- Nếu F1 **không** bắn: đây **không** phải một ứng viên desk. Nó là một dòng cần
  (i) kiểm `cap_lots`, (ii) kiểm filter `flat 16:30-18:15` (phụ lục 10 §IV: dấu
  giá nhà cung cấp quanh mốc 17:00 NY, `open` của nến mở lại là đáy nến ở
  **19,02%** nến `XAUDUKA` so với 3,20% nến thường — và **không dòng nào** của
  `mould-target` có filter đó, nên **đừng cho rằng dòng của tôi miễn nhiễm**),
  (iii) chạy lại trên một thước nến khác.
- **Không chạy lại một dòng đã công bố rồi trích số của nó.** Phụ lục 8 §II:
  chỉ 295/452 ô khớp `PF_usd` đã công bố. Mọi con số trong báo cáo là số tôi chạy ra.

---

## Ghi chú thêm — 2026-10-11, TRƯỚC khi chạy tiền kiểm

Hằng số của dụng cụ `py/research/range_sizing_precheck.py`, khai ra để không
ai phải suy chúng từ số:

    REFIT       moc dau moi thang lich (UTC) — khong phu thuoc du lieu
    MIN_TRAIN   20.000 dong truoc du bao dau tien
    TAP KHOP    dong `i` chi vao tap khop o moc `B` khi `t15[i] + 2 x 900_000 <= B`
                (ket cuoc cua no DA quan sat xong truoc `B`) — dieu khoan nay la
                cai lam C1 doc duoc
    NORM_WIN    2.000 du bao huu han gan nhat, TINH CA i
    NORM_MIN      200 du bao trong cua so, duoi do => NaN (khong phai 1,0)
    CLAMP       m kep [0,50 ; 2,00]
    SEED        20261011
    DELTA_USD   0,138 (nua spread 0,28 vong, khuyet diem 16)
    DONCHIAN    20 nen, nen HIEN TAI bi loai (`bars[i-period..i]`)

**Probe thứ ba, thêm vào C2 trước khi chạy:** ngoài `PROBE_peek_m16` (đặc trưng
đọc close phút 16) và `PROBE_y` (đưa `y(i)` vào làm biến hồi quy), thêm
`PROBE_fullfit` — **khớp OLS một lần trên CẢ chuỗi** thay vì đi bộ tiến. Nó test
đúng cái tầng mà hai probe kia không chạm: **tầng KHỚP**. Cả ba phải TRƯỢT C1.

**`py/research/intrabar_shape_precheck.py` được mang nguyên từ
`agent/intrabar-shape`** (không sửa một dòng) và import vào, nên `grid_1m`,
`intrabar_features`, `control_features`, `atr14_15m`, `outcome` là **đúng** hàm
đã qua C1/C2 đêm 10/10 — không phải một bản viết lại.
