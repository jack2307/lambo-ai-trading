# Đăng ký trước — volume profile trên VÀNG: có đo được hay không (`agent/vprofile-gold`)

**Ngày:** 2026-10-10. Nhánh `agent/vprofile-gold`, cắt từ `agent/stop-width`
(`29c8635`). Commit này **chỉ có file này**, trước dòng code đầu tiên và trước
receipt đầu tiên (brief §5).

Câu hỏi của job này **không phải** "profile khối lượng có qua cổng không" mà
**"nó có đo được không trên dữ liệu desk đang có"**. Hai câu đó khác nhau và
job này trả lời câu thứ nhất chỉ khi câu thứ hai ra "có".

## 1. Giả thuyết, một câu

Nếu cột `volume` của `XAUUSD-15m` (feed duy nhất của desk có vàng với cột khác
0) là **tick volume** — số lần giá đổi, không phải khối lượng — và nếu chính cột
đó **đổi cách đo ở 2023-12**, thì một "volume profile" trên vàng **không phải
một phép đo một đại lượng duy nhất**, và mọi kết quả cổng của nó là hiện vật của
nguồn số, không của thị trường.

## 2. Tiền kiểm ĐÃ LÀM BẰNG ĐỌC MÃ, trước khi tiêu một ô nào

Đọc `crates/fd-engine/src/price_levels.rs` (2.091 dòng) và
`crates/fd-engine/src/profile.rs` (376 dòng), theo đúng thứ tự brief yêu cầu:

- **`profile.rs` KHÔNG phải price-volume profile.** Nó là **strike profile** —
  histogram qua **strike của option prints** (`build_profile(trades: &[OptionTrade])`),
  với `ProfileMode::{Premium,Volume,NetPremium,AbsNetPremium,OpenInterest}`.
  `ProfileMode::Volume` ở đây là **số hợp đồng option**, không phải volume của
  nến. Hàm `value_area(strikes, total, pct)` là bước đi value-area cổ điển,
  dùng được chung về mặt thuật toán nhưng đầu vào là strike.
- **`price_levels.rs` LÀ nến, và profile giá của nó ĐÃ TỒN TẠI:**
  `activity_profile(bars, bucket_size_price, value_area_pct, buckets_per_atr)
  -> Option<BarProfile>` (dòng 449), trả `poc` / `vah` / `val` dạng
  `PriceLevel`, cộng `activity_total_bar_buckets` /
  `activity_in_value_area_bar_buckets`.
- **Và nó đã tự khai là TIME PROFILE, không phải volume profile.** Trường
  `BarProfile::measure` là một `&'static str` **luôn bằng `"TIME_AT_PRICE"`**
  (dòng 301 và 538), và doc comment dòng 293–300 nói nguyên văn: *"Not volume.
  The bar feeds here carry `volume: None` on the gold tape and a tick count of
  unknown provenance elsewhere — `fd_indicators::vwap` already documents that
  it substitutes 1.0 per bar where the feed has none … A profile built on that
  would be a time profile wearing a volume profile's name, so this one is a
  time profile wearing its own."*
  Luật cộng dồn (dòng 437–441 + 478–487): **mỗi nến cộng `1.0` vào MỌI bucket
  mà dải `low..high` của nó chạm**. `activity_profile` **không đọc `bar.volume`
  một lần nào** — kiểm được: `grep -n volume price_levels.rs` chỉ ra dòng doc
  293–298, một `volume: None` trong fixture test (dòng 1572), và không có chỗ
  nào khác.
- **Không có cơ chế nào trong engine đọc POC.** `activity_profile` /
  `BarProfile` chỉ xuất hiện ở `crates/fd-api/src/levels.rs:751` (route đọc
  `/api/paper/levels`) và ở `lib.rs:42-43` (re-export). `grep -i "poc\b"` trên
  `crates/fd-engine/src/lib.rs` ra **0 kết quả**. ⇒ **chưa có ô cổng nào từng
  tiêu cho họ profile giá**, và không có gì để "đo cái đã có" ở phía cổng.

⇒ **Kết luận tiền kiểm đọc mã, khai trước khi chạy:** cái đã tồn tại là
**profile SỐ NẾN** (`TIME_AT_PRICE`). Profile **khối lượng chưa tồn tại**. Nên
tiền kiểm 1 của brief không phải một phép so sánh giả thuyết — nó là phép so
sánh **cái đã có** (arm A) với **cái tối thiểu phải viết** (arm B), và desk đã
pre-commit trong mã rằng arm B không đáng tin. **Job này đo xem pre-commit đó
có đúng không, bằng số.**

## 3. Một dữ kiện cấu trúc, khai NGAY, vì nó có thể một mình đóng trục

Hai cửa sổ đã đăng ký của hồ sơ vàng là **IS `xauduka` 2018-06-16 → 2025-04-10**
và **OOS `xauusd` 2025-04-11 →**. Nhưng phụ lục 8 mục I đo được `XAUDUKA-15m`
có `volume > 0` ở **0/50.000 mẫu**. Metadata parquet của hai file cũng khai
thẳng điều đó:

    XAUDUKA-15m : source=dukascopy, feed=bid, 378.749 nen  (khong co khoa meta volume)
    XAUUSD-15m  : source=mt5, broker_symbol=XAUUSD.sc, 100.586 nen,
                  meta volume = "tick_volume (price changes per bar), not contracts"
    GC-1m       : 18.709 nen, khong meta

⇒ **Cửa sổ IS của hồ sơ KHÔNG dựng được profile khối lượng nào.** Nên hai cửa sổ
của job này **buộc phải cả hai nằm trong `XAUUSD-15m`** (2022-06-16 → 2026-09-17),
và đó **chính là nơi vết nối 2023-12 nằm**. Đây là tiền đề của tiền kiểm 2 và 3.

## 4. Dòng code duy nhất job này viết

`volume_profile()` cạnh `activity_profile()` trong `price_levels.rs`, **khác
arm A ở ĐÚNG MỘT chỗ: trọng số mỗi bucket chạm**, để tiền kiểm 1 là một phép so
sánh một biến:

    arm A  (DA CO)  activity_profile : moi bucket cham  +=  1.0
    arm B1 (VIET)   volume_profile   : moi bucket cham  +=  bar.volume
    arm B2 (VIET)   volume_profile   : moi bucket cham  +=  bar.volume / so_bucket_cham

B1 giữ nguyên quy ước của A ("nến rộng đóng góp nhiều hơn"), B2 **bảo toàn**
volume của nến. Khai cả hai **trước** khi xem số: nếu hai biến thể trả lời khác
nhau thì bản thân chuyện đó là một kết quả, không phải một lựa chọn sau khi
thấy số. `measure` của B1/B2 là `"TICK_VOLUME_AT_PRICE"` — **không bao giờ in
`"VOLUME_AT_PRICE"`**, vì cột đó không phải khối lượng (luật §8: `null != 0`,
và một cái tên sai là một con số sai).

`volume: Option<f64>` = `None` ⇒ **bucket không được cộng gì và nến đó bị đếm
riêng** (`bars_without_volume`), **không cộng 1.0 thay thế**. Đó đúng là khuyết
điểm `fd_indicators::vwap` mắc (`lib.rs:800`, `_ => 1.0`) và brief gọi là tiền
lệ `vwap_fade` im lặng thành TWAP.

Harness đo: một test trong `crates/fd-engine/tests/` đọc nến từ CSV bằng `std`
(không thêm dependency nào vào `fd-engine`; CSV do một script Python xuất từ
parquet ở scratchpad), gọi **chính `activity_profile` đã có** cho arm A, và in
một dòng mỗi cửa sổ. Phân tích CSV đầu ra bằng Python. ⇒ arm A **là mã đã có**,
không phải một bản port.

## 5. BA TIỀN KIỂM — mỗi cái có thể đóng trục, và mỗi cái tiêu 0 ô cổng

### P1 — profile khối lượng có phân biệt được với profile SỐ NẾN không?

Dựng A, B1, B2 trên **cùng nến, cùng bucket, cùng cửa sổ, cùng value_area_pct**.
So **POC** và **VAH/VAL** bằng **khoảng cách giá** (USD, và quy ra tick = 0,01
USD vì `digits = 2`), không bằng chỉ số bucket.

**Bắn khi:** trung vị `|POC_B − POC_A|` ≤ **1 tick (0,01 USD)** và phân vị 90 ≤
**1 tick**, ở **bucket mịn nhất** (0,01 USD) — tức ở độ phân giải mà "một tick"
là một phát biểu có nghĩa. Ở bucket thô (ATR/4) báo thêm **tỉ lệ cửa sổ trùng
đúng cùng bucket**, vì ở đó "trong một tick" không phân giải được và báo nó như
một phép kiểm sẽ là một con số vô nghĩa.

Nếu P1 bắn: **cột `volume` không thêm gì; "volume profile" trên vàng CHÍNH LÀ
time profile đã có**, và desk đã có nó rồi.

### P2 — profile có DỪNG YÊN qua vết nối 2023-12 không?

Trước hết **đo lại chính vết nối** (brief §8: số của tôi thắng số của brief):
trung vị tick/nến theo tháng và theo năm, `p99/p50`, % nến ≥ 2,5× trung vị
tháng. Khai trước rằng tôi **không** trích con số của brief như một dữ kiện.

Rồi ba đại lượng của profile, mỗi cửa sổ một dòng:

1. **bề rộng value area** — `(VAH − VAL)`, báo bằng **USD** và bằng
   **ATR(14) của cửa sổ** (ATR tính bằng USD nên nó lớn lên cùng giá vàng; bề
   rộng tính bằng USD một mình sẽ trôi vì giá, không vì profile).
2. **vị trí POC trong dải giá cửa sổ** — `(POC − min_low) / (max_high − min_low)`,
   0..1, **không chuẩn hoá được bằng gì khác nên nó là đại lượng không-đơn-vị
   duy nhất ở đây**.
3. **độ tập trung** — `activity[POC] / tổng`, và entropy chuẩn hoá của histogram
   (`H / ln(số bucket)`), cả hai không đơn vị.

So phân phối **chế độ A (2022-06-16 … 2023-11-30)** với **chế độ B
(2023-12-01 … 2026-09-17)**: thống kê KS hai mẫu, trung vị mỗi chế độ, và hiệu
trung vị.

**Và arm A là ĐỐI CHỨNG.** Chạy đúng ba đại lượng đó cho **cả** time profile:

- nếu **chỉ** B1/B2 dịch qua vết nối ⇒ cái dịch là **cột volume**, và profile
  của hai nửa là **hai đại lượng khác nhau**.
- nếu **cả** A **và** B dịch ⇒ cái dịch là thị trường/giá vàng, không phải cột
  volume, và P2 **không** bắn cho riêng volume. Không có đối chứng này thì P2
  là một phép đo không đọc được.

**Bắn khi:** với ≥2 trong 3 đại lượng, KS của arm B qua vết nối cho
**p < 0,01** VÀ `|hiệu trung vị|` của arm B lớn hơn của arm A ít nhất **2 lần**.
Hai điều kiện cùng lúc, vì cỡ mẫu lớn làm KS bắn với một hiệu vô nghĩa.

Nếu P2 bắn: **profile của hai nửa là hai đại lượng khác nhau**, và một phép đo
hai-cửa-sổ **không đo được cùng một thứ ở hai đầu**. Brief nói đây là chỗ trục
này nhiều khả năng chết, và **nói ra bằng số là một kết quả hợp lệ**.

### P3 — có cách chia nào cho cả hai nửa đủ lệnh mà KHÔNG cắt vào trong chế độ A?

Chế độ A chiếm **17,5/51 tháng ≈ 34%** của `XAUUSD-15m`. Cơ chế của job này
(nếu P1 và P2 cho đi tới) là **hồi quy value area**: nến đóng ngoài value area
của cửa sổ trước → vào theo chiều POC. Đếm **mốc thời gian tín hiệu** của cơ
chế đó trên toàn bộ feed, rồi hỏi: cắt **đúng tại 2023-12-01** có cho **cả hai
nửa ≥ 40 tín hiệu** không.

**Bắn khi:** chia tại vết nối cho một nửa < 40 tín hiệu. Lúc đó **mọi** cách
chia hai-cửa-sổ đủ lệnh đều phải cắt **bên trong** một chế độ, nên một nửa
chứa cả hai cách đo và phép đo hai cửa sổ **không đo được**.

Số tín hiệu đếm ở đây là tín hiệu, **không phải lệnh** — lệnh thật ít hơn (một
tín hiệu khi đang có vị thế bị bỏ). Báo cả hai con số và dùng **số lệnh** cho
chân `n ≥ 40` của cổng.

## 6. Falsifier — cụ thể và bắn được

- **F1 (brief, chính).** Nếu **P1 bắn HOẶC P2 bắn** thì **volume profile KHÔNG
  ĐO ĐƯỢC trên vàng với dữ liệu desk có**, và kết luận là: **desk đừng đổ công
  vào trục này cho tới khi có một nguồn khối lượng thật cho vàng.** Tiêu
  **0 ô cổng** (tiền lệ AUDNZD: khai 432 ô, F2 bắn ở tiền kiểm, tiêu 0).
- **F2.** Nếu **P3 bắn** thì phép đo hai cửa sổ không dựng được ⇒ cũng tiêu
  **0 ô cổng**, và kết luận là một phát biểu về **cỡ mẫu của feed**, không về
  thị trường.
- **F3 (nếu cả ba qua).** Nếu không ô nào qua `PF_r ≥ 1,200` VÀ
  `E ≥ +0,050R` VÀ `n ≥ 40` trên **cả hai** cửa sổ ⇒ **bị bác bỏ hợp lệ**, và
  đó là một kết quả khác hẳn F1: nó nói "đo được và thua", không phải "không
  đo được".
- **F4 (luật có nổ không).** `--exit-mix` bắt buộc mọi ô. Nếu luật của cơ chế
  (thoát ở POC) nổ **0 lần** thì ô đó **không đọc được**, kể cả khi in
  `SURVIVES` (brief §6a, tiền lệ `tsmom/120d`).
- **F5 (arm giao dịch được).** Nếu ô chỉ sống ở arm không-guards ⇒ **nói thẳng
  là không giao dịch được** (phụ lục 5 mục D).
- **F6 (cháy).** `max_drawdown_pct > 100%` ⇒ dòng đã cháy (khuyết điểm 14),
  báo và **không đọc PF của nó**.
- **F7 (loại stop).** Trước khi đọc bất kỳ con số cỡ stop nào, xác định cơ chế
  thuộc **L1 mẫu số / L2 thi hành + target `rr × risk` / L3 thi hành + target
  là MỨC GIÁ** bằng `Exits::` trong mã (phụ lục 8 mục III). Cơ chế thoát ở POC
  là **L3** nếu `Exits::Engine` — kiểm, đừng đoán theo tên.

## 7. Sổ đa phép thử — ĐẾM TRƯỚC

**Tiền kiểm (0 ô cổng):**

    P1/P2: 2 do dai cua so (1 ngay giao dich, 5 ngay giao dich)
         x 3 so do bucket (ATR(14)/4 = mac dinh route; 0,10 USD; 0,01 USD = 1 tick)
         x 3 arm do          (A time, B1 tick-volume, B2 tick-volume bao toan)
         = 18 chuoi profile tren cung mot bo nen
    P3:  1 co che x 1 feed, dem moc tin hieu

**Cổng — CHỈ chạy nếu cả ba tiền kiểm đều KHÔNG bắn:**

    1 ho co che (hoi quy value area)
      x 3 nguong vao (dong ngoai VA; >= 0,25 ATR ngoai VA; >= 0,50 ATR ngoai VA)
      x 2 do dai cua so profile (1 ngay, 5 ngay)
      x 2 cua so do (cat tai 2023-12-01)
      x 2 arm (guards off / guards on)
      = 24 o
      + 24 o lap lai o arm B2  => 48 o toi da

Không trục nào khác. Không thêm ngưỡng, không thêm độ dài cửa sổ sau khi thấy
số: sửa đăng ký = **thêm ghi chú có ngày vào CUỐI file này**.

## 8. Cách đọc

- **Cổng:** `PF_r ≥ 1,200` VÀ `E ≥ +0,050R` VÀ `n ≥ 40` trên **cả hai** cửa sổ.
  Đếm `n` bằng tay (verdict của tool kiểm `need 30`, cổng desk là 40).
- **`E = total_r / n`**, không đọc `expectancy` in 3 chữ số (phụ lục 8 mục IV).
- **`PF_r` cạnh `PF_usd`, khai đơn vị.** Hằng đẳng thức `E = Lbar × (PF_r − 1)`.
  ⚠️ **Mọi dòng `xauusd` chạy ở LOT TỐI THIỂU** (`starting_equity_usd = 100,0`,
  khuyết điểm 17) ⇒ `PF_usd` của job này là PF của một sổ 0,01 lot. Nói ra.
- **Drawdown USD cạnh mọi con số lợi nhuận**; `_pct` là SÀN.
- **Chi phí/R = spread/stop, theo CHÂN TRỜI, kèm cỡ nến của ATR.** Brief ghi
  **12,58% (2010-2018) / 8,35% (2018-2026)** ở hai cửa sổ của hồ sơ, và **4,04%
  là trung vị của cửa sổ 2 năm gần nhất** — ATR tính bằng USD nên nó lớn lên
  cùng giá vàng (1,5 ATR: 2,23 → 16,12 USD) còn spread cố định 0,28 ⇒ **cùng
  spread, chi phí/R chênh 7 lần giữa hai đầu hồ sơ.** Luôn lấy từ dòng
  `cost-matched null: ... cost X% of R` mà output tự in, **kèm cỡ stop**, và
  báo **riêng cho mỗi cửa sổ** — một con số chi phí/R cho cả hồ sơ là một con
  số sai.
- **Phân vị KHÔNG phải cổng.** `null p50 = 0,000` ⇒ không calibrate được.
  `count match` ngoài băng ⇒ **đừng công bố phân vị**. `SURVIVES` phụ thuộc chỗ
  ngồi trong file TOML.
- **Nếu một con số của brief không khớp số tôi đo, SỐ CỦA TÔI THẮNG** và tôi
  báo nó (brief §8). Và tôi **không trích một con số đã công bố như một dữ
  kiện** (phụ lục 8 mục II: chỉ 295/452 ô chạy lại khớp `PF_usd` đã công bố).

---

## Ghi chú thêm — 2026-10-10, SAU ba tiền kiểm, TRƯỚC ô cổng đầu tiên

Không viết lại dòng nào ở trên (brief §5). Đây là kết quả ba tiền kiểm và sổ đa
phép thử được sửa **trước** lần chạy cổng đầu tiên.

### 1. Ba tiền kiểm: KHÔNG CÁI NÀO BẮN

- **P1 không bắn.** Ở bucket một tick (0,01 USD), trung vị |ΔPOC| giữa arm B1
  và arm A là **0,31 USD (31 tick)** ở cửa sổ 1 ngày và **0,51 USD (51 tick)** ở
  5 ngày; p90 là **9,17** và **12,39 USD**. Điều khoản "≤ 1 tick" trượt rất xa.
  Ở bucket của chính route (ATR(14,15m)/4), hai POC chỉ rơi cùng một bucket ở
  **63,7%** (1 ngày) / **64,1%** (5 ngày) số cửa sổ với B1, và p90 của |ΔPOC| là
  **3,500** / **2,550 ATR(14,15m)**. ⇒ cột `volume` **có** thêm thứ gì đó.
- **P2 không bắn** theo đúng ba điều khoản đã khai: B1 đạt **0/3**, B2 đạt
  **1/3** (cần ≥2). Lý do nó không bắn được đo ở mục 2 dưới đây, và nó quan
  trọng hơn chính cái không-bắn.
- **P3 không bắn.** Cơ chế hồi quy value area cho **1.076 tín hiệu** (arm A,
  1 ngày, ngưỡng 0) trên `XAUUSD-15m`; cắt đúng tại **2023-12-01** chia thành
  **369 / 707**, cả hai ≥ 40. Ở cửa sổ 5 ngày: **213 tín hiệu → 75 / 138**.
  ⇒ **Câu "mọi cách chia cho hai nửa ≥40 tín hiệu đều phải cắt bên trong chế độ
  A" KHÔNG đúng cho cơ chế này** (brief §8: số đo thắng). Vết nối **là** một
  điểm chia dùng được, và nó là hai cửa sổ của job này.

### 2. Vì sao ba đại lượng của P2 MÙ với vết nối — đo được, không suy đoán

POC và value area **chỉ đọc trọng số TƯƠNG ĐỐI bên trong một cửa sổ**, nên
chúng **bất biến** với mọi phép nhân cột `volume` bởi một hằng số. Vết nối
2023-12 là một bước nhảy **MỨC** (trung vị tick/nến 1.108 → 400,5, ×0,36 trong
một tháng), nên nó **triệt tiêu** trong mỗi cửa sổ. Bất biến đó được ghim thành
một đẳng thức, không phải một phép xấp xỉ, bằng test
`a_constant_volume_reproduces_the_time_profile_exactly`: với volume **hằng số**,
`volume_profile` trả **đúng cùng** POC/VAH/VAL với `activity_profile`.

Nên đại lượng phải kiểm tính dừng là **HÌNH DẠNG của cột tick bên trong cửa
sổ**, không phải mức của nó. Đo trên 1.100 ngày giao dịch, cả bốn đại lượng
không-đơn-vị **nhảy ở đúng 2023-12 và không bao giờ quay lại**:

    dai luong (trong mot ngay)   che do A   che do B   ty so   KS D    KS p
    cv (do lech / trung binh)      0,6364     0,3189   0,501   0,823   4,6e-149
    gini                           0,3372     0,1797   0,533   0,842   9,5e-156
    top-10% share                  0,2362     0,1518   0,642   0,776   1,6e-132
    entropy chuan hoa              0,9587     0,9879   1,030   0,849   1,2e-158
    --- de doi chieu, MUC: ---
    trung vi tick/nen             1183,0     1562,5    1,321   0,446   5,2e-44

`KS D = 0,82–0,85` nghĩa là hai phân phối **gần như rời nhau**. Và chiều của nó
là chiều **về phía profile số nến**: entropy chuẩn hoá lên **0,9879**, tức cột
trọng số ở chế độ B **gần phẳng**, và một cột phẳng **CHÍNH LÀ** profile số nến
theo đẳng thức ở trên.

### 3. Phép đo hậu kiểm (POST-HOC, khai rõ là post-hoc)

Phép đo này **không có trong đăng ký** và được nghĩ ra **sau khi** thấy mục 2,
nên nó **không được tính là một falsifier đã đăng ký**. Nó là chứng cứ, kèm đối
chứng, cho điều mà P2 nhắm tới.

Chia P1 theo chế độ, arm B1 (arm giữ nguyên quy ước của A ⇒ một biến), bucket
ATR(14)/4, cửa sổ 1 ngày — tỉ lệ cửa sổ mà POC khối lượng rơi **cùng bucket**
với POC số nến:

    volume THAT     : che do A 46,2%  ->  che do B 73,0%   (+26,8 diem)
    volume XAO TRON : che do A 71,2%  ->  che do B 83,2%   (+12,0 diem)
    khoang cach that-vs-xaotron: -25,0 diem  ->  -10,2 diem   (giam 2,45 lan)

Đối chứng xáo trộn: cột `volume` được **xáo trộn bên trong từng ngày giao dịch**
(seed 20261010), giữ nguyên phân phối và phá liên hệ với giá. Nó tái tạo **12,0
trong 26,8 điểm** của cú dịch ⇒ gần **45%** cú dịch chỉ là phân phối phẳng đi.
Phần còn lại là **lượng thông tin thật của cột**, và nó **giảm 2,45 lần** qua
vết nối: −25,0 điểm ở chế độ A so với −10,2 điểm ở chế độ B.

⇒ Phát biểu đo được: **cột tick volume của vàng nói về chỗ giá đã giao dịch
mạnh hơn 2,45 lần ở nửa đầu so với nửa sau.** Nó **không** phải hai đại lượng
rời nhau (P2 không bắn), nhưng nó **cũng không phải một đại lượng có cùng độ
mạnh ở hai đầu**.

### 4. Hai chỗ brief SAI, theo số tôi đo (brief §8)

1. **Duka không "volume = 0"; nó là `null` ở 1m và `0` ở 5m/15m.** Điều tra
   toàn bộ (không phải mẫu 50.000): `XAUDUKA-1m` **null ở 5.635.777/5.635.777
   dòng**, còn `XAUDUKA-15m` và `-5m` là **đúng một giá trị duy nhất `0.0`** ở
   378.749 và 1.135.389 dòng. Metadata của file 15m ghi
   `resampled_from = XAUDUKA-1m.parquet`, nhưng `fd_store::resample`
   (`resample.rs:25`) làm `row.volume.filter(..).unwrap_or(1.0)` ⇒ nếu nó đã
   dựng file đó thì mỗi nến 15m phải mang **15,0**, không phải 0. Nên **con số
   0 trong hai file Duka là do một bước chuyển đổi khác biến `null` thành `0`**
   — đúng cái luật §8 `null != 0` cấm, ở một chỗ chưa ai đếm. (Khuyết điểm 18,
   **đếm không sửa**.) `XAGDUKA-15m`, `EURDUKA-15m`, `AUDNZD-15m` mang cùng
   dạng 0 tuyệt đối; `GC-1m` là `null` ở 18.709/18.709 dòng.
2. **"85 lần ít hơn giữa 2023 và 2026" là về TỈ LỆ NẾN NGOẠI LAI, không phải
   volume.** Trung vị tick/nến 2023 = **1.177**, 2026 = **5.130** ⇒ 2026 **NHIỀU
   HƠN 4,36 lần**. Cái giảm 85 lần là % nến ≥ 2,5× trung vị. Số tôi đo cho % đó
   (chuẩn theo trung vị **tháng**) là **9,51% (2023) → 0,13% (2024) → 0,00%
   (2026)**, không phải 3,40/0,33/0,04 — định nghĩa mẫu số khác nhau, chiều thì
   giống và mạnh hơn. `p99/p50` khớp brief: **4,80 / 3,13 / 1,76**.
   Và quan trọng hơn: **mức tick không chỉ có một bước nhảy, nó có một đoạn
   TĂNG ĐƠN ĐIỆU ×15,4** từ 2024-02 (388) tới 2026-03 (5.976) ⇒ **chế độ B tự
   nó cũng không dừng yên về mức**, chỉ dừng yên về hình dạng (cv 0,24–0,46
   suốt 34 tháng, so với 0,43–0,75 ở 18 tháng trước đó).

### 5. Sổ đa phép thử SỬA — 72 ô, không 48, và vì sao

Đăng ký khai 48 ô (B1 + B2). Thêm **24 ô ĐỐI CHỨNG ở `measure = 0`** — chính
`activity_profile` đã commit, profile SỐ NẾN. Lý do: mục 2 đo được rằng ở chế độ
B cột trọng số gần phẳng, nên **một ô volume qua cổng ở chế độ B có thể qua
hoàn toàn nhờ phần số-nến của nó**, và không có arm đối chứng thì câu đó không
đọc được. Khai **trước** lần chạy cổng đầu tiên:

    3 nguong (0 / 0,25 / 0,50 ATR ngoai VA)
      x 2 do dai cua so profile (1 ngay, 5 ngay)
      x 3 measure (0 = SO NEN doi chung, 1 = tick volume/bucket, 2 = tick volume dan deu)
      = 18 dong trong mot file batch
      x 2 cua so do  (W1 2022-06-16..2023-11-30 ; W2 2023-12-01..2026-09-17)
      x 2 arm        (guards off / guards on)
      = 72 o, 4 lan goi binary

Hai cửa sổ **cắt tại vết nối**, theo mục 3 của đăng ký: cửa sổ IS của hồ sơ
(`xauduka`) không dựng được profile khối lượng nào, nên cả hai cửa sổ buộc phải
nằm trong `XAUUSD-15m`.

Binary: build từ nhánh này (`target-vp`), có `lbar_line` của `agent/stop-width`
nên `PF_r` đọc được từ receipt. `--exit-mix` bật ở mọi lần chạy (F4).
