# KẾT QUẢ — hai cơ chế đã phẳng sẵn qua rollover, đo trong khuôn `rollover-flat`

Đăng ký: `docs/decisions/2026-10-09-flat-native.md`, commit **một mình nó** ở
`fec22e6`, trước thiết kế và trước receipt đầu tiên.
Thiết kế: `docs/research/designs/2026-10-09-flat-native.toml`, 12 dòng, đóng băng.
Receipt: `{A,Acomm,C,Ccomm}-duka-{is,oos}.txt`, **8 run, 96 dòng in ra**.

Binary `/e/rust/fd-stop-width/target-sw/release/search.exe` (có `Lbar`/`PF_r`,
drawdown, `expectancy_net`). `git diff 29c8635 HEAD -- crates/ src/ Cargo.toml
Cargo.lock` rỗng ⇒ **không build gì trên nhánh này.**
Mọi receipt in lại `flags: 12 passed, every one of them read by
--mode=hypotheses` và `news: 747 events (2010-01-08 → 2027-12-08) from
E:/rust/flowdesk/data\news\events.parquet` (tức `data/` **mặc định**, bất kể
`--data=`).

---

## 0. Một con số

**Trên 48 ô, `max` của `min(IS, OOS)` của `gross mỗi phiên / spread một lượt`
là `−0,30` — ÂM.** Không một dòng nào trong 12 dòng, ở cả hai arm, có
**gross dương trên cả hai cửa sổ**. Cao nhất cả registry mà `agent/gross-ceiling`
đo được là **+3,94**; khuôn cần **~+8,7**. Ở đây không có số dương để so.

**F1 BẮN.** Cửa sổ 16:00–18:00 NY — chỗ duy nhất khuôn `rollover-flat` tự khai
là chưa phủ — **không chứa gì mà khuôn có thể tiêu**, và khuôn đó đã được thử
trên toàn bộ phạm vi nó có thể phủ. Kết luận đóng.

**Nhưng con số đáng giữ nhất của job không phải cái đó. Nó là:**

    gap/s4 (gap-fade, stopGapMult 4,0), arm CÓ GUARDS
      IS   gross +0,2981 R/phien = +9,46 spread moi luot   <- VUOT +8,7 CUA KHUON
           PF_r 1,0390  E +0,0036R  Lbar 0,0904  n 120  sut 380,15 USD = 3,80%
      OOS  gross -0,1904 R/phien = -8,93 spread moi luot   <- DOI DAU, GAN DOI XUNG
           PF_r 0,6920  E -0,0450R  Lbar 0,1470  n 130  sut 942,13 USD = 9,24%

**Đây là dòng ĐẦU TIÊN trong cả hồ sơ vượt mốc 8,7× mà khuôn cần — trên một
cửa sổ — và nó ở **−8,93×** trên cửa sổ kia.** Hiện vật cửa sổ thứ mười (họ tín
hiệu) không chỉ lật mức, nó lật **đúng cái vạch yêu cầu của khuôn**. Và nó lật
trên một dòng **stop THI HÀNH**, nên không thể gạt đi như một chuyện đơn vị.

**Và một con số thứ ba, về dụng cụ, lớn hơn cả hai cái trên:**

    gap-fade OOS, 130 lenh Y HET, chi khac bat/tat guards:
      PF_usd  0,0508 (arm A)  ->  0,6330 (arm C)     = +0,582
      PF_r    0,0867 (arm A)  ->  0,0867 (arm C)     =  0,000

`cap_lots` dịch `PF_usd` **+0,582** và dịch `PF_r` **0,0000** trên cùng một tập
lệnh. Phụ lục 7 mục A đo khoảng cách lớn nhất là **0,364**; đây là **0,582**.

---

## 1. TIỀN KIỂM — phân loại stop, tiêu 0 ô cổng (phụ lục 7 mục B)

| cơ chế | khai | file:dòng | loại |
|---|---|---|---|
| `gap-fade` | `Exits::Engine` | `crates/fd-strategy/src/gap_fade.rs:60` | **THI HÀNH** |
| `intraday-momentum` | `Exits::Strategy` | `crates/fd-strategy/src/intraday_momentum.rs:65` | **MẪU SỐ** |

**F2 đã bắn ở tiền kiểm theo chiều ngược với brief:** stop của `gap-fade`
**là một stop thật**, nên cỡ stop vào thiết kế như một **biến**. Số đo khẳng
định điều đó rất mạnh: `stopGapMult` 0,5 → 1,0 → 4,0 đổi **cả tập thoát**
(STOP 65/73 → 46/56 → 7/9 trên 120/130 lệnh), đổi drawdown **28.014 USD →
988 USD**, và đổi `PF_r` **0,297 → 0,087 → 0,692** (OOS). Đó là một câu hỏi
chiến lược, không phải một phép khai lại đơn vị.

**F7 — `intraday-momentum` là mẫu số thuần, khẳng định bằng số.** Arm không
guards, `riskDailyRanges` = 0,5 / 1,0 / 2,0:

| | n (IS) | n (OOS) | `E` | `E × f` | risk điều hoà (điểm) | tỉ lệ |
|---|---|---|---|---|---|---|
| f = 0,5 | **2012** | **1979** | −0,0330 | −0,0165 | 8,684 | 1,000 |
| f = 1,0 | **2012** | **1979** | −0,0170 | −0,0170 | 17,211 | **1,982** |
| f = 2,0 | **2012** | **1979** | −0,0080 | −0,0160 | 34,276 | **3,947** |

Tập lệnh **không đổi một lệnh**, exit-mix giống nhau tới từng số
(`WEEKEND_FLAT 400, window closed 1612`), `E × f` đi ngang, risk tỉ lệ `f`.
⇒ **Đúng kết luận `agent/stop-width`, trên một cơ chế khác.**

Và cùng phép thử đó **tái tạo khuyết điểm 16 một cách độc lập**: khớp
`risk = f × range + δ` trên ba điểm cho **δ = +0,154 điểm (IS) / +0,19 điểm
(OOS)**, so với nửa spread **0,14** mà phụ lục 7 mục C đo. (Ba điểm này là
trung bình **điều hoà**, nên khớp tuyến tính là xấp xỉ — không phải khớp
R² = 0,99685 của phụ lục 7, và được khai là ước lượng.)

Ở arm **có** guards thì stop mẫu số **thành stop thật** đúng như đăng ký nói:
`OPEN_LOSS_CAP` nổ **2 lần (IS) / 1 lần (OOS)** ở f = 0,5 và **0 lần** ở
f = 1,0 và f = 2,0. Hai lệnh trên 2.012 — guards biến nó thành stop thật, nhưng
ở cỡ này cái stop thật đó gần như không bao giờ bị chạm.

---

## 2. `0,117 điểm giá` KHÔNG phải cái stop. Nó là một ĐUÔI KHÔNG BỊ CHẶN của mẫu số R

Đây là chỗ brief cho hai lựa chọn và **câu trả lời là lựa chọn thứ ba**.

**(a) Cái stop thật thì bình thường.** Engine tự in, dòng
`the method's own realised stop: median ... points`:

| | median stop | = % của R (spread/stop) |
|---|---|---|
| `gap-fade` IS | **3,21 điểm** (2,034 ATR 15m) | **8,71%** |
| `gap-fade` OOS | **5,33 điểm** (1,937 ATR 15m) | **5,25%** |

Không có gì hẹp ở đây. 5,25% của R nằm giữa dải vàng đã biết (1,5 ATR 15m =
4,04%).

**(b) `0,117` là trung bình ĐIỀU HOÀ của `|entry − stop|`, và nó tái tạo được.**
Tuyến hoa hồng (mục 3) đo trên chính receipt:

    gap-fade  IS   risk dieu hoa = 1,009 diem   (spread = 27,74% cua R)
    gap-fade  OOS  risk dieu hoa = 0,123 diem   (spread = 227,68% cua R)

**0,123 so với 0,117 của `gross-ceiling` — khớp trong 5%.** Nên con số đó là
thật, chỉ **không phải cỡ stop**: nó là trung bình điều hoà, và trung bình điều
hoà bị **chi phối bởi phần tử nhỏ nhất**.

**(c) Tái dựng ĐỘC LẬP hình học, đọc thẳng từ parquet**, không qua engine
(`scratchpad/flatnative-gapgeom.py`, chỉ đọc):

| dòng | n engine | n tái dựng | risk điều hoà engine | risk điều hoà tái dựng |
|---|---|---|---|---|
| `gap-fade` IS | 120 | **120** | 1,009 | **1,0089** |
| `gap-fade` OOS | 130 | **130** | 0,123 | **0,1230** |
| `gap/s05` IS/OOS | 120 / 130 | **120 / 130** | 0,539 / 0,723 | **0,5390 / 0,7232** |
| `gap/s4` IS/OOS | 120 / 130 | **120 / 130** | 8,889 / 13,131 | **8,9068 / 13,0997** |
| `gap/daily` IS/OOS | 202 / 178 | **202 / 178** | 1,197 / 0,164 | **1,1966 / 0,1641** |

8 cặp cửa sổ-dòng, số lệnh khớp **chính xác**, risk điều hoà khớp **3–4 chữ số
có nghĩa**. Và phân phối đầy đủ của `|entry − stop|` trên `gap-fade` OOS:

    min 0,0010 diem  |  q01 0,171  |  q05 0,846  |  median 5,169  |  max 152,66

**MỘT lệnh trong 130 có `risk = 0,0010 điểm giá — một phần mười xu vàng.** Ở
1% của sổ 10.000 USD, một lệnh đó size ra **100.000 lot**. 1/130 (0,8%) dưới
0,117 điểm; 4/130 (3,1%) dưới **một spread** (0,28 điểm). Biên độ của mẫu số R
trên cùng một dòng là **5 bậc độ lớn** ⇒ **"R" của `gap-fade` không phải một
đơn vị.**

**(d) Nguyên nhân trong mã, chính xác.** `gap_fade.rs:96` đặt
`stop = bar.open + gap × stopGapMult` — tính từ open của **nến tín hiệu**. Lệnh
là một **lệnh chờ**, khớp ở open của **nến sau**. Engine lấy
`risk = |entry − stop|` (`engine.rs:717`), và điều kiện nhận lệnh duy nhất là
`if !(risk > 0.0) { refuse }` (`engine.rs:724`) — **KHÔNG CÓ SÀN cho risk.**
Nên khi nến sau mở gần sát stop, mẫu số sụp mà không ai chặn.

**(e) Hậu quả là cả HAI phía cùng lúc, và đó là chỗ nó khác một quả bom cỡ lệnh
đơn thuần:**
- phía USD: `lots` trung bình **778,7** mỗi lệnh (từ `spread paid 28.346,05 USD
  / (130 × 0,28)`), drawdown **28.014,34 USD = 272,41% của đỉnh**, net
  **−27.729,87 USD** trên sổ 10.000 USD;
- phía R: `|avg_loss_r|` = **3,9262 R** và `Lbar` = **2,5671 R** **trên một stop
  ĐƯỢC THI HÀNH**, thứ chỉ có thể xảy ra nếu mẫu số sụp. `avg_mae` −2,699 R.

⇒ **Khuyết điểm 18, ĐẾM KHÔNG SỬA: `engine.rs:717-724` không có sàn cho
`risk = |entry − stop|`, nên một cơ chế đặt stop theo hình học của nến tín hiệu
và khớp ở nến sau có thể tạo ra một mẫu số R gần 0 — làm phình `lots` VÀ phình
`|r|` cùng lúc.** Nó không phải khuyết điểm §7-2 của brief (định giá stop bị gap
tại `bar.open`); nó ở **entry**, không ở exit, và nó đứng riêng.

**(f) Chữa được bằng tham số, và đây là chỗ "stop thi hành" khác "stop mẫu số".**
`stopGapMult = 4,0` đẩy stop ra xa nến khớp:

| | min risk | risk điều hoà | drawdown | `_pct` | `PF_r` |
|---|---|---|---|---|---|
| `gap-fade` OOS (s=1) | **0,0010 điểm** | 0,123 | 28.014,34 USD | **272,41%** | 0,0867 |
| `gap/s4` OOS (s=4) | **0,986 điểm** | 13,131 | **988,41 USD** | **9,64%** | 0,6920 |
| `gap-fade` IS (s=1) | 0,0540 điểm | 1,009 | 3.594,06 USD | 35,35% | 0,3890 |
| `gap/s4` IS (s=4) | **3,019 điểm** | 8,889 | **400,91 USD** | **3,99%** | **1,0390** |

Ở `agent/stop-width` nới stop **không đổi một lệnh nào**. Ở đây nó đổi mọi thứ.
**Đó là toàn bộ nội dung của phụ lục 7 mục B, đo trên hai cơ chế cùng một job.**

---

## 3. THƯỚC — tuyến hoa hồng, và F3 trượt điều khoản chữ của chính nó (số đo thắng)

Thước `swap$` của `gross-ceiling` **không dùng được** ở đây (`swap$ = 0` theo
cấu trúc). Tuyến thay thế, khai trước ở mục 3 của đăng ký: một arm chỉ khác ở
`commission_per_lot = 0,05`, và vì `commission = c × lots × 2` còn
`risk_usd = risk × lots × cs`, **`lots` triệt tiêu**:

    r - r_net = 2c/risk  =>  mean(1/risk) = (total_r - total_r_net)/(2c·n)

**Parity của arm hoa hồng: 48/48 ô in CÙNG số lệnh và `total_r/n` khớp
`expectancy` tới chữ số in ra** (một ô, `gap-fade` OOS, lệch 0,0005 R = đúng
nửa đơn vị làm tròn của 3 chữ số). Hoa hồng **không đổi tập lệnh.**

**F3 (điều khoản chữ: tái tạo `px-1s` trong 2%) BẮN ở 2 trong 4 số** — và cái
trượt lại là số đáng hơn:

| | tôi đo | `gross-ceiling` | phụ lục 5 mục B (drift đã công bố) |
|---|---|---|---|
| `px-1s` spreadR IS | **1,09%** | 1,150% | — |
| `px-1s` spreadR OOS | **0,82%** | 0,826% | — |
| `px-1s` gross/lượt IS | **−0,0041** | −0,0035 | **−0,0040** |
| `px-1s` gross/lượt OOS | **+0,0204** | +0,0203 | **+0,0205** |

Hai thước **không đo cùng một thứ**: của `gross-ceiling` là **tỉ số của trung
bình** (`spread_usd/n/risk_usd`), của tôi là **trung bình của tỉ số**
(`0,28 × mean(1/risk)`) — và trung bình của tỉ số **mới là thứ nằm trong
`expectancy`**, vốn là trung bình của `r_i = points_i/risk_i`. Nên điều khoản
2% so hai estimator khác nhau, không so một thước với sự thật.

**Với đại lượng mà cái neo tồn tại để tái tạo — drift của phụ lục 5 mục B —
thước của tôi đọc `−0,0041` so với `−0,0040` (lệch 2,5%) và `+0,0204` so với
`+0,0205` (lệch 0,5%), tức SÁT HƠN cả `gross-ceiling`.** Cộng với 8 cặp tái
dựng độc lập ở mục 2(c) và phép thử tỉ lệ 1 : 2 : 4 ở mục 1, thước được công
bố. Ghi chú có ngày đã thêm vào cuối đăng ký (brief §5: sửa = thêm vào cuối).

**Một khoảng chính xác phải nói ra:** `expectancy` in **3 chữ số thập phân**,
còn `gross = expectancy + spreadR` là **hiệu của hai số gần bằng nhau**. Với
`intraday-momentum` (gross ≈ 0,0005 R/lượt) làm tròn ở 3 chữ số **lớn hơn cả
đại lượng**. Nên mọi `gross` ở đây tính từ `E = total_r/n` (`total_r` in 2 chữ
số trên n ≈ 2.000 lệnh ⇒ chính xác hơn ~n/5 lần). Một bảng đọc `expectancy`
3-chữ-số sẽ cho `xSPR` của `im/f2` là **+0,36** thay vì **−0,42** — **lật dấu
kết luận của một dòng chỉ bằng làm tròn.**

---

## 4. Bảng — arm CÓ GUARDS (arm duy nhất chủ cho phép, phụ lục 5 mục D)

`xSPR` = `gross mỗi phiên / spread một lượt`, cả hai bằng R của chính dòng đó,
**bất biến theo đòn bẩy** (định nghĩa của `gross-ceiling` §5). Khuôn cần ~8,7;
cao nhất registry 3,94. `S` = mean hold `px-1s` cùng run (1.313,8 / 1.340,4 ph).

| dòng | n IS/OOS | `PF_r` IS/OOS | `PF_usd` IS/OOS | `Lbar` | `xSPR` IS | `xSPR` OOS | **min** | sụt USD IS/OOS | `_pct` | lượt/phiên | `cap_lots` IS |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `im/f2` | 2012/1979 | 0,608/0,788 | 0,609/0,784 | 0,021 | −0,30 | +6,88 | **−0,30** | 1.558/774 | 15,6/7,7 | 18,2/18,6 | 0 |
| `intraday-mom` | 2012/1979 | 0,608/0,788 | 0,610/0,782 | 0,042 | −0,31 | +6,90 | **−0,31** | 2.870/1.487 | 28,7/14,9 | 18,2/18,6 | 0 |
| `gap/daily-a2` | 50/75 | 0,621/0,390 | 0,762/0,533 | 0,290 | −0,50 | −28,75 | −28,75 | 502/954 | 5,0/9,4 | 8,6/9,7 | 18/50 |
| `im/f05` | 2012/1979 | 0,605/0,785 | 0,608/0,774 | 0,084 | −0,59 | +6,64 | −0,59 | 4.953/2.792 | 49,4/27,9 | 18,2/18,6 | 0 |
| `px-1s` (neo) | 2014/1982 | 0,877/1,047 | 0,875/1,046 | 0,166 | −0,87 | +1,92 | −0,87 | 4.292/2.480 | 39,9/20,3 | 1,00/1,00 | 0 |
| `gap-fade` | 120/130 | 0,389/**0,087** | **0,619/0,633** | 0,486/**2,567** | −1,05 | −0,42 | −1,05 | 932/1.189 | 9,2/11,6 | 14,9/14,2 | **81/120** |
| `gap/daily-s4` | 202/178 | 0,643/0,774 | 0,652/0,754 | 0,124 | −1,87 | −1,72 | −1,87 | 888/1.026 | 8,8/9,9 | 9,1/8,7 | 22/202 |
| `gap/daily-a05` | 548/455 | 0,166/0,104 | 0,410/0,489 | 0,866 | −2,67 | −2,45 | −2,67 | 3.653/2.457 | 36,2/24,5 | 29,0/34,3 | **485/548** |
| `gap/s05` | 120/130 | 0,218/0,297 | 0,628/0,594 | 0,760 | −3,46 | −3,94 | −3,94 | 782/1.038 | 7,8/10,3 | 24,1/19,0 | **96/120** |
| `gap/daily` | 202/178 | 0,378/**0,102** | 0,575/**0,629** | 0,479/**1,995** | −4,19 | −0,83 | −4,19 | 1.550/1.347 | 15,4/13,2 | 15,3/16,5 | 152/202 |
| `gap/2atr` | 40/66 | 0,389/0,390 | 0,567/0,524 | 0,328 | −6,84 | −28,64 | −28,64 | 492/883 | 4,9/8,7 | 8,2/9,1 | 12/40 |
| `gap/s4` | 120/130 | **1,039**/0,692 | 1,032/0,689 | 0,090 | **+9,46** | **−8,93** | −8,93 | **380/942** | **3,8/9,2** | 8,5/7,9 | 12/120 |

Arm **không** guards cùng hình dạng, mọi `xSPR` min vẫn âm (cao nhất
**−0,37**, `px-1s`), và **ba dòng CHÁY** ở đó (mục 6).

**CHÂN CỔNG, đếm bằng tay ở 40 lệnh (không ở `need 30` của tool): 0 / 48 ô.**
Không ô nào qua `PF ≥ 1,200 VÀ expectancy ≥ +0,050R VÀ ≥ 40 lệnh` trên **một**
cửa sổ, nói gì cả hai. Gần nhất là `gap/s4` IS: `PF_r 1,0390` (thiếu 0,161) và
`E +0,0036R` (thiếu 0,046R). **Chân nào ràng buộc:** `Lbar` của `gap/s4` là
**0,0904 R < 0,250 R** ⇒ theo hằng đẳng thức phụ lục 6 mục I, **chân expectancy
RÀNG BUỘC** ở dòng này, không phải chân PF — ngược với 10-trên-10 ô của
`gross-ceiling` và giống `rollover-flat`. Lý do giống: một dòng ghi một phần
nhỏ của R mỗi lượt có `Lbar` nhỏ.

---

## 5. Không một dòng nào TƯƠNG THÍCH KHUÔN, và đó là câu trả lời cấu trúc

| | lượt/phiên phơi nhiễm | tương thích khuôn (≤ 1,5)? |
|---|---|---|
| `px-1s` (chính là khuôn) | **1,00** | có |
| `gap/*` | **7,9 – 35,8** | **không** |
| `intraday-momentum`, `im/*` | **18,2 – 18,6** | **không** |

**PHẲNG QUA ROLLOVER VÌ MÌNH NGẮN KHÔNG PHẢI LÀ MỘT CUỐN SỔ MỘT PHIÊN.**
`gap-fade` giữ 88–170 phút và `intraday-momentum` giữ 72 phút, trên một phiên
dài 1.314–1.399 phút. Chúng vượt 0 mốc 17:00 NY **chính bởi vì** chúng ngắn —
và ngắn nghĩa là trả spread **8 đến 36 lần mỗi phiên** ở chỗ khuôn trả **một
lần**. Đó đúng là cấu trúc chi phí mà `gross-ceiling` đã đo là thứ giết mọi
dòng phía trên cổng (`rsi2-pull` 6,9 lượt/phiên, spread 176% của gross).

⇒ Câu "chúng không trả carry" là **đúng** — `swap$` in **0** ở cả bốn arm swap
của `gross-ceiling` và `expectancy_net` ở đây chỉ lệch bởi hoa hồng tôi tự cài.
Nhưng **không trả carry không mua được gì**, vì tiền tiết kiệm được ở mốc
rollover (phí qua đêm còn 3,4% ở `rollover-flat`) nhỏ hơn hẳn tiền trả thêm cho
8–36 lượt spread. **Hai cơ chế này đổi một chi phí 3,4% lấy một chi phí
170–3.340% của gross.**

---

## 6. Falsifier — cái nào bắn, ở đâu

**F1 — BẮN.** `max` trên 48 ô của `min(IS,OOS)` của `xSPR` = **−0,30**. Dưới
3,94, và dưới 0. Hai điều kiện của F1 (trượt cổng cả hai cửa sổ ở arm có guards
**và** `xSPR` dưới 3,94) đều thoả: cổng 0/48, `xSPR` cao nhất âm.
⇒ **Cửa sổ 16:00–18:00 NY cũng không chứa gì, và khuôn `rollover-flat` đã được
thử trên toàn bộ phạm vi nó có thể phủ.**

**F2 — BẮN Ở TIỀN KIỂM, ngược chiều brief**, tiêu 0 ô cổng. Mục 1.

**F3 — BẮN điều khoản chữ ở 2/4 số, số đo thắng.** Mục 3. Số dẫn xuất **được**
công bố, kèm cả hai cách đọc và kèm hai phép kiểm nội bộ mạnh hơn.

**F4 — KHÔNG bắn. Luật của mọi dòng đều nổ.** `--exit-mix` trên cả 8 run:

    gap/*          : chi STOP / TARGET / TIMEOUT. 0 lan NEWS_FLAT,
                     0 lan WEEKEND_FLAT, 0 lan OPEN_LOSS_CAP, tren CA 8 dong
                     x 2 cua so trong arm CO guards.
    intraday-mom   : window closed 1612/2012 (IS), 1586/1979 (OOS) = 80,1%;
                     WEEKEND_FLAT 400 / 393 = 19,9% (moi thu Sau).

⇒ **Họ `gap/*` sống trong arm chủ cho phép còn gọn hơn cả biến thể phẳng của
`rollover-flat`** (0 trên 120–551 lệnh so với 0–1 trên 1.157–1.944). Không có
cái bẫy `tsmom/120d` ở đây.

**F5 — KHÔNG bắn. Không dòng nào là drift.** Phần dư sau khi trừ
`signed_share × gross/phiên(px-1s) × (spreadR_dòng/spreadR_px-1s)`:

    intraday-mom OOS arm C : gross +0,0846  residual +0,0842   -> drift giai thich 0,5%
    gap-fade     OOS arm C : gross -0,9593  residual -0,7276   -> drift giai thich 24%
    gap/s4       IS  arm C : gross +0,2981  residual +0,2900   -> drift giai thich 2,7%

`signed_share` của họ `gap/*` nằm trong **−0,416 … −0,124** (arm C, IS; thấp
nhất cả tám run là **−0,053**, `gap-fade` OOS) — chúng vào cả hai chiều, nên chúng gần như không gom drift. **Cái lỗ của chúng là của chúng, không
phải của phơi nhiễm.** Không có ca `sess-hold` 101,4% drift nào ở đây.

**F6 — BẮN, 3 cặp dòng-arm, tất cả ở arm KHÔNG guards:**

    gap-fade      / OFF OOS   272,41% cua dinh   28.014,34 USD   net -27.729,87 USD
    gap/daily     / OFF OOS   261,80% cua dinh   26.492,24 USD   net -26.372,33 USD
    gap/daily-a05 / OFF OOS   134,58% cua dinh   13.584,29 USD
    (gap/daily-a05 / OFF IS    98,79% — thieu 1,21 diem, khong chay, bao ra)

**PF của ba dòng đó không được đọc** và chúng không vào bảng mục 4.
**Và một số đáng giữ: ở arm CÓ guards KHÔNG dòng nào cháy** — `cap_lots` kéo
`gap-fade` OOS từ **272,41% xuống 11,58%** (28.014 USD → 1.189 USD). Cái trần
notional của arm chủ cho phép **đúng là thứ chặn khuyết điểm 18 lại** — nhưng
nó chặn bằng cách thu nhỏ **81/120 = 67,5% (IS)** và **75/130 = 57,7% (OOS)**
số lệnh của dòng, và **cả hai chân cổng đều không thấy việc đó** (`r =
points/risk` không đọc `lots`). Tệ nhất: `gap/daily-a05` **485/548 = 88,5%**.

**F7 — KHÔNG bắn, phân loại được khẳng định.** Mục 1.

---

## 7. `wrong_side_stop` KHÔNG ĐƯỢC ĐO — và chưa từng được đo

Brief §7 nói `wrong_side_stop` "đã kiểm 0 lần trên ~580 dòng", và
`gross-ceiling` RESULT.md §1 viết "`wrong_side_stop` is 0 in all 8 receipts".

    grep -rn "wrong_side_stop" crates/   ->  0 ket qua
    grep -ic "wrong_side"  moi receipt   ->  0 (ca cua toi va cua gross-ceiling)

**Không có bộ đếm nào mang tên đó trong mã, và không receipt nào in nó.**
`BacktestResult` (`engine.rs:230-250`) có `skipped_no_atr`, `skipped_by_guard`,
`closed_by_guard`, `sized_down_by_guard` — **không có cái nào cho
`wrong_side_stop`**. Nên con số 0 đó là **một số 0 đọc từ một bộ đếm không tồn
tại**, đúng cái brief §8 cấm (`null` != `0`).
⇒ Trên 96 dòng của tôi, `wrong_side_stop` là **KHÔNG ĐO ĐƯỢC, không phải 0**.
Và câu "0 trên ~580 dòng" trong hồ sơ **không có chỗ nào chống đỡ**. Đếm,
không sửa.

---

## 8. Sổ đa phép thử: khai vs xem

| | khai | xem |
|---|---|---|
| dòng | 12 | **12** |
| cửa sổ | 2 | **2** |
| arm guards | 2 | **2** |
| **ô xếp hạng** | **48** | **48** |
| đọc arm hoa hồng (chỉ `total_r − total_r_net`) | 48 | **48** |
| run | 8 | **8** |
| dòng in ra | 96 | **96** |
| ô tiền kiểm | 1 | **1** |

**Không dòng nào được thêm, không cửa sổ nào đổi, không arm nào thêm, không
tham số nào chỉnh, không ngưỡng nào dịch, không sàn cỡ mẫu nào hạ.**
Hai đại lượng được giới thiệu **sau khi thấy số** và được khai là thế:
`net R mỗi phiên = E × lượt/phiên` (mục 5) và `lots trung bình =
spread_paid/(n × 0,28 × cs)` (mục 2e). Cả hai là tổ hợp của những cột đăng ký
đã khai, và không đổi xếp hạng.
**Một dòng tái dựng độc lập ngoài engine** (mục 2c) — đọc parquet, 0 ô cổng.

**Bias của cực đại, khai như đăng ký:** `+9,46` của `gap/s4` IS là lớn nhất
của 48 ô × 2 cửa sổ nên nó lệch lên. Chuyện đó chỉ quan trọng cho nhánh
**không** bắn; và `min(IS,OOS)` cao nhất là **−0,30**, nên nhánh âm của F1 an
toàn đúng theo hướng đăng ký đã nói.

---

## 9. KHÔNG đo được, và vì sao

1. **Phân vị.** Không công bố dòng nào (đăng ký mục 5). `gross-ceiling` §7 đo
   `SURVIVES` đổi theo **chỗ ngồi trong file TOML**, nên phân vị ở đây sẽ là
   một tuyên bố về thứ tự dòng.
2. **`wrong_side_stop`.** Mục 7 — không có bộ đếm.
3. **Excursion trong lệnh.** Mọi drawdown là đường vốn **đã đóng lệnh**;
   `avg_mae` là field duy nhất thấy vị thế đang mở. Trên `gap-fade` OOS khoảng
   cách đó là cả câu chuyện: `avg_mae` **−2,699 R** trên một stop **được thi
   hành** — chỉ có thể xảy ra nếu mẫu số sụp (mục 2).
4. **`risk` của từng lệnh, trực tiếp.** `Trade::risk_usd` tồn tại trong struct
   nhưng **không printer nào đọc nó**. Mọi con số risk ở đây là trung bình
   **điều hoà** (tuyến hoa hồng) hoặc **trung vị** (dòng engine tự in) — hai
   đầu của một phân phối rộng 5 bậc độ lớn, và **không cái nào là trung bình
   cộng**. Một bản vá in `mean(risk)` và `mean(lots)` sẽ đóng chỗ này bằng một
   dòng; job này **không build** nên không làm.
5. **Khuyết điểm §7-2 của brief** (`check_exit` định giá stop bị gap tại
   `bar.open` trên nến khớp của lệnh chờ). Họ `gap/*` là **đúng hình dạng** của
   nó và `|avg_loss_r| 3,93 R` trên stop thi hành là **đúng dấu vết** của nó,
   nhưng tôi **không tách được** nó khỏi khuyết điểm 18 (mẫu số sụp) bằng những
   cột engine in ra: cả hai cho cùng một triệu chứng. **Đếm cả hai, gán 0.**
6. **Tham số ngoài 12 dòng.** `minGapHours`, `minGapAtr`, `stopGapMult` ở những
   giá trị khác; `firstFrom/firstTo/from/to` của `intraday-momentum` **không
   quét một lần nào** (lưới của nó rỗng *có chủ ý*: "the windows are the
   paper's; a sweep over them would be a search").
7. **Instrument và cỡ nến khác `xauduka` 15m.** Mọi số ở đây là số **15m**.
8. **Spread thật ở chỗ nghỉ phiên.** Feed này phẳng **0,28 ở mọi giờ**, và họ
   `gap/*` vào lệnh **đúng tại nến mở lại** — chỗ một venue thật nới spread
   nhất. Mọi `spread/gross` ở đây là **chặn dưới**, và sai lệch đó **ngược
   hướng giả thuyết**.
9. **`data-sealed/`.** Không mở, không đọc, không trỏ tới, không đếm.

**Một thứ ĐƯỢC đo mà kế hoạch không hứa (và nó sửa một tiền đề của tôi):** gián
đoạn nến của feed `XAUDUKA` 15m, đọc thẳng từ parquet trên cả 378.749 nến:

    2.634 gian doan DUNG 75 phut, mo lai o 22:00 UTC (1.758) hoac 23:00 UTC (946)
      = 18:00 New York ca mua he (UTC-4) va mua dong (UTC-5)
    862 gian doan >= 24h  (cuoi tuan va ngay le)
    42 gian doan 15 < g < 54 phut (lo du lieu le te)

⇒ Nghỉ phiên hằng ngày của feed này là **75 phút, 16:45 → 18:00 New York**, nên
`minGapHours = 0,9` **bắt đủ cả 2.634 lần** và họ `gap/daily` **thật sự đọc cú
gap NGÀY ở đúng cửa sổ 16:00–18:00** khuôn chưa phủ. Việc nó chỉ ra 202 (IS) /
178 (OOS) lệnh **không** phải vì gián đoạn không có, mà vì **cú mở lại 18:00 NY
hầu như luôn mở gần phẳng**: chỉ 202 trong ~1.750 lần mở lại của cửa sổ IS có
gap ≥ 1 ATR(15m), và nới xuống 0,5 ATR mới được 551. **Cửa sổ đó không trống;
nó phẳng.**

---

## 10. Một câu cho chủ máy

Hai cơ chế này **đúng là** phẳng qua mốc rollover và **đúng là** phủ cửa sổ
16:00–18:00 NY, nhưng chúng phẳng **vì chúng ngắn**, nên chúng trả spread 8–36
lần mỗi phiên ở chỗ khuôn trả một lần, và `max min(IS,OOS)` của gross mỗi phiên
trên 48 ô là **ÂM** (−0,30 spread) so với 3,94 cao nhất của registry và ~8,7
khuôn cần. Thứ duy nhất vượt 8,7 ở đâu đó trong hồ sơ là `gap/s4` ở **+9,46
trên cửa sổ IS và −8,93 trên OOS** — hiện vật cửa sổ, không phải cơ chế. Và cú
"cháy tài khoản" của `gap-fade` **không phải một stop hẹp**: cái stop thi hành
là **5,33 điểm** rất bình thường, còn `0,117` là trung bình điều hoà bị **một
lệnh có `|entry − stop| = 0,0010 điểm`** chi phối, vì `engine.rs:724` không có
sàn cho mẫu số R.
