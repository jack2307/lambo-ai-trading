# Kết quả — volume profile trên VÀNG: ĐO ĐƯỢC, và bị bác bỏ 0/72 (`agent/vprofile-gold`)

Đăng ký: `docs/decisions/2026-10-10-vprofile-gold.md` (commit `4af0b7b`, chỉ
docs) + ghi chú thêm cùng ngày (commit `e99eaa7`, trước ô cổng đầu tiên).
Receipt: `receipts/vprofile-gold/`. Batch:
`docs/hypotheses/2026-10-10-vprofile-gold.toml`.

## 0. Một câu

**Volume profile trên vàng ĐO ĐƯỢC** — ba tiền kiểm không cái nào bắn — và nó
**bị bác bỏ hợp lệ: 0/72 ô qua cổng**, `PF_r` cao nhất **0,8765** trên 72 ô mà
**mọi ô đều lỗ**. Đó là F3, không phải F1: kết luận là *"đo được và thua"*,
không phải *"không đo được"*.

## 1. Tiền kiểm đọc mã: profile đã tồn tại, và nó là profile SỐ NẾN

- `crates/fd-engine/src/profile.rs` **không** phải price-volume profile: nó là
  **strike profile** trên option prints (`build_profile(trades: &[OptionTrade])`).
  `ProfileMode::Volume` ở đó là **số hợp đồng option**, không phải volume nến.
- `crates/fd-engine/src/price_levels.rs:449` — **`activity_profile` ĐÃ TỒN TẠI**,
  trả `poc`/`vah`/`val`, và **tự khai** `measure: &'static str` **luôn bằng
  `"TIME_AT_PRICE"`**. Doc comment dòng 293–300 viết trước job này: *"Not
  volume … A profile built on that would be a time profile wearing a volume
  profile's name, so this one is a time profile wearing its own."* Nó
  **không đọc `bar.volume` một lần nào**.
- **0 cơ chế nào trong engine đọc POC** — `activity_profile` chỉ xuất hiện ở
  `fd-api/src/levels.rs:751` (route đọc) và re-export. ⇒ **chưa có ô cổng nào
  từng tiêu cho họ profile giá.**

⇒ Job này đo **cái đã có** làm arm đối chứng, và viết **đúng một** hàm mới:
`volume_profile()` cạnh nó, **khác đúng một dòng** — trọng số mỗi bucket chạm
là `bar.volume` thay vì `1.0`.

## 2. Ba tiền kiểm: KHÔNG CÁI NÀO BẮN ⇒ không có cớ tiêu 0 ô

### P1 — phân biệt được với profile số nến (không bắn)

10.323 dòng profile trên 100.586 nến `XAUUSD-15m`, 2 độ dài cửa sổ × 3 sơ đồ
bucket × 3 arm. Ở bucket **một tick (0,01 USD)**:

    cua so   arm              trung vi |dPOC|       p90 |dPOC|
    1 ngay   B1 tick volume   0,31 USD (31 tick)    9,17 USD (917 tick)
    5 ngay   B1 tick volume   0,51 USD (51 tick)   12,39 USD (1.239 tick)

Điều khoản "≤ 1 tick" trượt **31–51 lần** ở trung vị. Ở bucket của chính route
(ATR(14,15m)/4), hai POC rơi cùng một bucket chỉ **63,7%** (1 ngày) / **64,1%**
(5 ngày) số cửa sổ, và p90 của |ΔPOC| là **3,500** / **2,550 ATR(14,15m)**.
⇒ **cột `volume` có thêm thứ gì đó.** Tiền lệ `vwap_fade` (im lặng thành TWAP)
**không** tái diễn ở đây.

⚠️ **Chọn mẫu phải nói ra:** `MAX_BUCKETS = 5_000` (`price_levels.rs:432`) loại
mọi cửa sổ rộng hơn 50 USD ở bucket 0,01 ⇒ sơ đồ một tick chỉ còn
**369/379 (97,4%)** cửa sổ chế độ A nhưng **378/721 (52,4%)** chế độ B ở cửa sổ
1 ngày, và **13/144 (9,0%)** ở cửa sổ 5 ngày. Nên sơ đồ một tick **thiên lệch về
chế độ A**, và bucket chính của báo cáo này là ATR(14)/4.

### P2 — dừng yên qua vết nối 2023-12 (không bắn theo đúng ba điều khoản)

B1 đạt **0/3** điều khoản, B2 đạt **1/3** (cần ≥2). Nhưng **lý do nó không bắn
đáng hơn cái không-bắn**, và nó được đo chứ không suy đoán:

**POC và value area chỉ đọc trọng số TƯƠNG ĐỐI bên trong một cửa sổ**, nên chúng
**bất biến** với mọi phép nhân cột `volume` bởi một hằng số. Ghim thành một
**đẳng thức** bằng test `a_constant_volume_reproduces_the_time_profile_exactly`:
với volume hằng số, `volume_profile` trả **đúng cùng** POC/VAH/VAL với
`activity_profile`, và tổng chênh **đúng bằng** hằng số đó. Vết nối 2023-12 là
một bước nhảy **MỨC**, nên nó **triệt tiêu** trong mỗi cửa sổ.

⇒ Đại lượng phải kiểm tính dừng là **HÌNH DẠNG của cột tick bên trong cửa sổ**.
Đo trên **1.100 ngày giao dịch**, bốn đại lượng không-đơn-vị, KS hai mẫu:

    dai luong (trong mot ngay giao dich)  che do A   che do B   ty so   KS D    KS p
    cv = do lech / trung binh               0,6364     0,3189   0,501   0,823   4,6e-149
    gini                                    0,3372     0,1797   0,533   0,842   9,5e-156
    top-10% share                           0,2362     0,1518   0,642   0,776   1,6e-132
    entropy chuan hoa                       0,9587     0,9879   1,030   0,849   1,2e-158
    --- de doi chieu, MUC: ---
    trung vi tick/nen                      1183,0     1562,5    1,321   0,446   5,2e-44

`KS D = 0,82–0,85` ⇒ **hai phân phối gần như rời nhau.** Và chiều của nó là
chiều **về phía profile số nến**: entropy chuẩn hoá lên **0,9879**, tức cột
trọng số ở chế độ B **gần phẳng** — và theo đẳng thức ở trên, một cột phẳng
**CHÍNH LÀ** profile số nến.

Theo tháng, bước nhảy ở **đúng 2023-11 → 2023-12** và **không bao giờ quay lại**:
`cv` **0,652 → 0,344** (×0,53), rồi **0,24–0,46 suốt 34 tháng** tiếp theo so với
**0,43–0,75 ở 18 tháng** trước đó. Cùng tháng với bước nhảy mức
(trung vị tick/nến **1.140,8 → 391,0**, ×0,34).

### P3 — cách chia hai cửa sổ đủ lệnh (không bắn)

    co che hoi quy value area, arm A, cua so 1 ngay, nguong 0:
      1.076 tin hieu  ->  cat tai 2023-12-01: 369 / 707   (ca hai >= 40)
    cua so 5 ngay:
        213 tin hieu  ->  cat tai 2023-12-01:  75 / 138   (ca hai >= 40)

⇒ **Câu của brief "mọi cách chia cho hai nửa ≥40 tín hiệu đều phải cắt bên
trong chế độ A" KHÔNG đúng cho cơ chế này.** Vết nối **là** một điểm chia dùng
được, và nó là hai cửa sổ của job này. Lệnh thật: **385–1.017 mỗi ô**, dư xa
chân `n ≥ 40`.

## 3. Phép đo hậu kiểm, kèm đối chứng — khai rõ là POST-HOC

Nghĩ ra **sau khi** thấy mục P2, nên **không** tính là falsifier đã đăng ký.
Chia P1 theo chế độ, arm B1 (một biến), bucket ATR(14)/4, cửa sổ 1 ngày — tỉ lệ
cửa sổ mà POC khối lượng rơi **cùng bucket** với POC số nến:

    volume THAT     : che do A 46,2%  ->  che do B 73,0%   (+26,8 diem)
    volume XAO TRON : che do A 71,2%  ->  che do B 83,2%   (+12,0 diem)
    --------------------------------------------------------------------
    khoang cach that-vs-xaotron (= luong thong tin cua cot):
                      che do A -25,0 diem  ->  che do B -10,2 diem   (giam 2,45 lan)

Đối chứng: cột `volume` **xáo trộn bên trong từng ngày giao dịch** (seed
20261010), giữ nguyên phân phối, phá liên hệ với giá. Nó tái tạo **12,0 trong
26,8 điểm** cú dịch ⇒ gần **45%** cú dịch chỉ là phân phối phẳng đi; phần còn
lại là thông tin thật của cột.

⇒ **Cột tick volume của vàng nói về chỗ giá đã giao dịch mạnh hơn 2,45 lần ở
nửa đầu so với nửa sau.** Không phải hai đại lượng rời nhau (P2 không bắn),
nhưng **cũng không phải một đại lượng cùng độ mạnh ở hai đầu.**

## 4. Cổng: 0/72 ô — và 24 ô trong đó là ĐỐI CHỨNG

Cơ chế: `vprofile-reversion` (`crates/fd-strategy/src/vprofile_reversion.rs`),
`Exits::Engine` — **lớp L3** (stop thi hành + target là một MỨC GIÁ, lớp duy
nhất có câu trả lời sạch cho "nới stop làm gì"), xác định bằng `Exits::` trong
mã chứ không bằng tên tham số (F7 không bắn).

    cong: PF_r >= 1,200 VA E >= +0,050R VA n >= 40 tren CA HAI cua so
    guards off : 0/18 dong qua
    guards ON  : 0/18 dong qua
    => 0/72 o

`PF_r` cao nhất mỗi lần chạy (**không ô nào là một pass**):

    w1-guards-off   time/d5/t0.25   PF_r 0,8715   E -0,1285 R   n 387   dd   69,92 USD = 66,76% dinh
    w1-guards-on    time/d5/t0.25   PF_r 0,8765   E -0,1229 R   n 369   dd   10,03 USD =  9,98% dinh
    w2-guards-off   tvsp/d5/t0.00   PF_r 0,8090   E -0,1850 R   n 742   dd  103,30 USD = 85,90% dinh
    w2-guards-on    tvsp/d5/t0.25   PF_r 0,7996   E -0,1947 R   n 726   dd   29,07 USD = 28,63% dinh

**Cả 72 ô đều lỗ.** `E` trong bảng là `Lbar × (PF_r − 1)` đọc từ dòng `lbar_line`
(không phải `expectancy` 3 chữ số), và **hằng đẳng thức đúng 72/72 ô**, phần dư
lớn nhất **0,00009 R**.

**Arm volume so với arm đối chứng SỐ NẾN**, ghép đôi trên (cửa sổ, guards, độ
dài cửa sổ, ngưỡng):

    TICK_VOLUME / PER_TOUCHED_BUCKET : thang doi chung 15/24 cap
                                       trung binh dPF_r +0,0414, dE +0,0447 R
       W1 (che do A): 7/12,  dPF_r +0,0454
       W2 (che do B): 8/12,  dPF_r +0,0374
    TICK_VOLUME / SPREAD             : thang doi chung 16/24 cap
                                       trung binh dPF_r +0,0128, dE +0,0151 R
       W1 (che do A): 4/12,  dPF_r -0,0415   <- TE HON doi chung
       W2 (che do B): 12/12, dPF_r +0,0672   <- TOT HON doi chung, 12/12

⇒ **Cột volume thêm đúng +0,04 PF_r**, cần **+0,33 PF_r** nữa mới tới vạch
1,200. Và **arm nào tốt hơn lại LẬT giữa hai cửa sổ** (`tvsp` 4/12 ở W1 so với
12/12 ở W2) — hiện vật cửa sổ ở ngay trong lựa chọn quy ước trọng số.

## 5. Chi phí/R — đo riêng từng cửa sổ, kèm cỡ stop, và nó chênh 2,3 lần

Lấy từ dòng `cost-matched null: ... cost X% of R` mà output tự in:

    cua so  stop thuc hien (trung vi)        chi phi/R
    W1      0,412-0,481 ATR(14,15m)          32,18% - 39,44% cua R
    W2      0,368-0,435 ATR(14,15m)          14,07% - 17,72% cua R

**Cùng spread 0,28 USD vòng, chi phí/R của W1 gấp ~2,3 lần W2**, vì ATR(15m)
tính bằng USD lớn lên cùng giá vàng (1.800 → 4.374 USD trong chính hai cửa sổ
này). Đó chính xác là điều brief cảnh báo, đo trên cỡ stop của cơ chế này
(**~0,43 ATR(14,15m)**, không phải 1,5 ATR) ⇒ **một con số chi phí/R cho cả hồ
sơ là một con số sai**; con số 4,04% của brief là ở 1,5 ATR(15m) và ở cửa sổ 2
năm gần nhất.

## 6. Falsifier: bắn gì, ở đâu

- **F1 (P1 hoặc P2 bắn ⇒ không đo được): KHÔNG BẮN.** Nên **không có cớ tiêu 0
  ô**, và 72 ô đã tiêu đúng theo đăng ký.
- **F2 (P3 bắn): KHÔNG BẮN.**
- **F3 (đo được mà 0 ô qua cổng): BẮN.** Đây là kết quả của job: **bị bác bỏ
  hợp lệ**, không phải "không đo được".
- **F4 (luật có nổ không): KHÔNG BẮN, nhưng sát ranh giới.** `--exit-mix` bật ở
  cả 4 lần chạy. Luật riêng của cơ chế — **TARGET = thoát ở POC** — nổ:

      cua so profile 1 ngay:  8,1% - 12,4% so lan thoat
      cua so profile 5 ngay:  1,3% -  4,3% so lan thoat   <- co che la mot STOP, khong phai mot reversion
      STOP chiem 81% - 89% moi o

  ⇒ **Ở cửa sổ 5 ngày, cơ chế đã không còn là "hồi quy về POC"** — nó là một
  stop chặt 0,43 ATR với một target gần như không bao giờ tới. Nói ra, vì
  `tsmom/120d` từng in `SURVIVES` với luật nổ **0 lần** (brief §6a).
- **F5 (arm giao dịch được): KHÔNG ÁP DỤNG** — không ô nào qua ở arm nào.
  Arm có guards **không** cứu được gì: 0/18 ở cả hai arm.
- **F6 (cháy): KHÔNG BẮN, nhưng sát.** **0/72 ô** in `max_drawdown_pct > 100%`.
  Tệ nhất: `w2-guards-off time/d1/t0.50` sụt **129,60 USD = 99,26% của đỉnh**,
  net **−97,31 USD** trên một sổ **100 USD**. ⇒ **Arm không-guards của họ này
  cách mốc cháy đúng 0,74 điểm phần trăm.** Arm có guards giữ sụt ở **7,07% –
  42,49%**.
- **F7 (loại stop): KHÔNG BẮN.** `Exits::Engine` xác nhận trong mã; stop được
  thi hành (STOP là 81–89% số lần thoát), nên cỡ stop ở đây **không** phải một
  mẫu số.

## 7. Bốn chỗ brief/phụ lục SAI — số đo thắng (brief §8)

1. **Duka không "volume = 0"; nó là `null` ở 1m và `0` ở 5m/15m.** Điều tra
   **toàn bộ** (không phải mẫu 50.000):

       XAUDUKA-1m   : null o 5.635.777 / 5.635.777 dong
       XAUDUKA-15m  : DUNG MOT gia tri duy nhat 0.0 o 378.749 dong
       XAUDUKA-5m   : DUNG MOT gia tri duy nhat 0.0 o 1.135.389 dong
       XAGDUKA/EURDUKA/AUDNZD-15m : cung dang 0 tuyet doi
       GC-1m        : null o 18.709 / 18.709 dong
       XAUUSD-15m   : khac 0 o 100.586 / 100.586 dong

   Metadata file 15m ghi `resampled_from = XAUDUKA-1m.parquet`, nhưng
   `fd_store::resample` (`resample.rs:25`) làm
   `row.volume.filter(|v| v.is_finite()).unwrap_or(1.0)` ⇒ nếu nó dựng file đó
   thì mỗi nến 15m phải mang **15,0**, không phải 0. Nên **con số 0 trong các
   file Duka do một bước chuyển đổi khác biến `null` thành `0`** — đúng cái luật
   §8 `null != 0` cấm. **Khuyết điểm 18, đếm không sửa.**
   Và `unwrap_or(1.0)` của `resample` là **khuyết điểm 19, đếm không sửa**: một
   trọng số hằng số mỗi nến **CHÍNH LÀ** time profile theo đẳng thức ở mục P2,
   nên mọi nến 15m/5m mà **route sống** (`fd-api/src/state.rs:177`,
   `routes.rs:315`) dựng từ 1m không-có-volume sẽ mang một volume **chế tạo**.
   Đường backtest **không** chạm nó: `search.rs:139` gọi `read_bars` trực tiếp,
   không resample.
2. **"85 lần ít hơn giữa 2023 và 2026" là về TỈ LỆ NẾN NGOẠI LAI, không phải
   volume.** Trung vị tick/nến **2023 = 1.177**, **2026 = 5.130** ⇒ 2026 **NHIỀU
   HƠN 4,36 lần**. `p99/p50` khớp brief: **4,80 / 3,13 / 1,76**. % nến ≥ 2,5×
   trung vị **tháng** tôi đo là **9,51% (2023) → 0,13% (2024) → 0,00% (2026)**,
   không phải 3,40/0,33/0,04 — mẫu số khác nhau, chiều giống và mạnh hơn.
3. **Chế độ B tự nó cũng KHÔNG dừng yên về mức.** Trung vị tick/nến đi từ
   **388 (2024-02) → 5.976 (2026-03)**, **×15,4 đơn điệu, không bước nhảy**. Nên
   "hai chế độ" đúng về **hình dạng** (cv 0,24–0,46 suốt 34 tháng) nhưng **sai
   về mức** — chế độ B là một đoạn trôi, không phải một mặt bằng.
4. **Chiều của `PF_usd` so với `PF_r` do ARM GUARDS quyết định, không do dấu
   của sổ.** 72 ô **đều lỗ** (`PF_r` lớn nhất 0,8765), vậy mà:

       guards off : PF_usd doc CAO hon PF_r o  4/36 o,  trung binh gap -0,0566
       guards ON  : PF_usd doc CAO hon PF_r o 30/36 o,  trung binh gap +0,0709
       tat ca     : 34/72 (47,2%);  gap min -0,1193, trung vi -0,0031, max +0,2365

   ⇒ **Phụ lục 7 mục A sai** ("dấu của cái dịch đi theo dấu của sổ" — ở đây dấu
   **lật** trên những sổ cùng dấu), **và phụ lục 8 mục III.1 cũng không khớp**
   ("`PF_usd` đọc CAO hơn ở 71%" — ở đây 47,2%, và chia theo arm thì **11% vs
   83%**). Cái quyết định là **arm guards**, vì guards đổi đường vốn mà cỡ lệnh
   lấy từ đó, còn `PF_r` không đọc `lots`. **Đừng suy `PF_r` từ `PF_usd` theo
   một chiều cố định.**

## 8. Thứ KHÔNG đo được, và vì sao

- **Volume profile trên 16 năm vàng của hồ sơ: KHÔNG đo được**, và lần này là
  một `null` thật chứ không phải một số 0. Cửa sổ IS đã đăng ký
  (`xauduka` 2018-06 → 2025-04) **không dựng được profile khối lượng nào** vì
  cột đó là 0 ở 15m/5m và `null` ở 1m. Vàng có volume khác 0 **chỉ từ
  2022-06-16** (`XAUUSD-15m`), tức **4,25 năm**, và vết nối chia nó thành
  **1,46 + 2,80 năm**.
- **Khối lượng THẬT của vàng: chưa từng có trên desk này.** Mọi cột khác 0 là
  **tick volume** — số lần giá đổi — và `py/ingest/mt5_export.py:33-34` khai
  thẳng *"CFD real volume is always zero"*. Nên mọi kết quả ở đây là về
  **tick volume at price**, và chúng tôi gọi nó đúng tên đó trong mã
  (`measure: "TICK_VOLUME_AT_PRICE"`), **không bao giờ** `"VOLUME_AT_PRICE"`.
- **Phân vị: KHÔNG công bố.** `count match` nằm trong **0,95–1,00** (trong
  băng) nhưng `null p50` là **0,600–0,806 ở cả 72 ô** — **dưới 1**, tức null
  trung vị của chính nó đã lỗ, nên một phân vị cao ở đây chỉ nói "lỗ ít hơn vào
  lệnh ngẫu nhiên cùng chi phí", không nói có lãi. Chính output in câu đó. Và
  `SURVIVES` phụ thuộc chỗ ngồi trong file TOML. **Cổng là cái nói, và nó nói
  0/72.**
- **Drawdown trong lệnh: KHÔNG đo được.** `max_drawdown_*` là đường vốn
  **đã đóng lệnh**; `avg_mae` là trường duy nhất thấy excursion, và nó đọc
  **−1,012 đến −1,134 R** trên 72 ô (họ `d1` của W1: −1,119 đến −1,126 R).

## 9. Kết luận cho desk

1. **Volume profile trên vàng đo được** — nhưng chỉ trên **4,25 năm**, và cột
   đo được là **tick volume**, không phải khối lượng.
2. **Nó đã bị bác bỏ ở cơ chế hồi quy value area: 0/72 ô**, `PF_r` ≤ 0,8765,
   mọi ô lỗ, cả hai arm, cả hai cửa sổ. Không phải "chưa đo"; đã đo và thua.
3. **Cột volume đóng góp +0,04 PF_r so với profile SỐ NẾN đã có**, và cần thêm
   **+0,33** nữa. ⇒ **Nếu desk muốn làm gì với profile giá thì
   `activity_profile` đã có trong cây và cột volume gần như không thêm gì** —
   đúng cái pre-commit mà `price_levels.rs` đã viết trong doc comment trước khi
   job này bắt đầu. Job này **đo** pre-commit đó: nó đúng về độ lớn, sai về
   tuyệt đối (cột volume **có** phân biệt được).
4. **Trước khi đổ công vào bất kỳ họ nào đọc `volume` của vàng, cần một nguồn
   khối lượng thật** — và một nguồn **dừng yên**: cột hiện có đổi hình dạng
   ×0,53 ở 2023-12 và đổi mức ×15,4 trong 25 tháng sau đó.
5. **Hai khuyết điểm mới để đếm:** 18 (`null` → `0` ở bước chuyển đổi Duka) và
   19 (`resample.rs:25` `unwrap_or(1.0)` chế tạo volume trên đường route sống).
