# KẾT QUẢ — quy ước ĐÃ PHÂN GIẢI từ mã: `0,28` là **bề rộng báo giá**, bị trừ **đúng một lần mỗi vòng**. **Falsifier KHÔNG bắn.** Định giá lại là **rẻ hơn**, và nó dịch **0/28 ô** qua cổng.

Đăng ký: `docs/decisions/2026-10-11-gold-reprice.md`, commit **một mình nó** ở
`4132fba`, trước lần gọi `search.exe` đầu tiên của nhánh.
Binary `/e/rust/fd-stop-width/target-sw/release/search.exe` (= `29c8635`, đúng
HEAD worktree này) ⇒ **không build gì**.
Thiết kế: file **đóng băng** của `agent/cost-breakeven` (`83c43eb`), copy
**byte-identical** (`md5 d0b8d8cfb715d38f67080d00c3ccef34`), **không thêm, không
bỏ, không đổi một dòng hay một tham số nào**.

---

## 0. Trả lời, một câu

`rules.spread` của engine là **bề rộng báo giá bid–ask** — một đại lượng **một
chiều** — và engine trừ **đúng một lần bề rộng đó cho cả vòng**. Đó là cách
tính **đúng** của một bề rộng báo giá, nên **không có nửa nào bị thiếu**:
`ask − bid` mà logger ghi là **cùng một đại lượng**, và thay 0,28 → 0,220 là
một phép thay **cùng loại**. **Con số đúng là 0,220, RẺ HƠN.** Falsifier
**không bắn**, và **0,440 là sai**.

---

## 1. Phân giải, từ mã — SÁU chỗ độc lập, không một chỗ nào đoán

**(a) `engine.rs:901-912` — `apply_costs`, nguyên văn:**

    /// Half the spread against the trader on every side.
    pub(crate) fn apply_costs(price: f64, side: Side, entering: bool, rules: &TradingRules) -> f64 {
        let half = rules.spread / 2.0;

entry long `price + half`, exit long `price − half`. ⇒ **tổng một vòng =
`half + half` = `rules.spread` × 1.** Không phải × ½, không phải × 2.

**(b) `engine.rs:12-13`, doc-comment của chính module:** *"Half the spread is
charged on entry and half on exit; commission is per lot per side."* — spread
tách đôi, commission mỗi chiều. Hai đại lượng khác nhau, khai riêng.

**(c) `engine.rs:153` — field `Trade::spread` tự khai đơn vị:** *"The spread
this trade actually paid, per unit of price, round turn."* Và `engine.rs:763`
gán `spread: Some(rules.spread)` — **nguyên con số, không nhân đôi**. Tức engine
**tự khai** rằng `rules.spread` **là** hoá đơn cả vòng.

**(d) `cost_table.rs:63-75` — `cost_fraction_of_r`, CÓ TEST GHIM:**
`cost_money = spread * lots * contract_size` — **spread × 1**. Test
`the_ratio_is_dimensionless_so_lots_and_contract_size_cancel` khẳng định
`0.28 / 2.00 = 14.0% of R`. Nếu 0,28 là một chiều phải nhân đôi thì test này
phải đọc 28,0%. **Nó đọc 14,0%.**

**(e) `rebate.rs:24-31`, nguyên văn:** *"ONE WHOLE ROUND-TURN SPREAD PER TRADE,
and the word round-turn is load-bearing. `engine::apply_costs` charges half the
spread on entry and half on exit, and the two halves are one spread; crediting a
share of each side would be twice the truth."* ⇒ nhánh hoàn phí của **chính repo
này đã giải đúng câu hỏi này**, và `fd-api/tests/rebate_parity.rs` ghim nó.

**(f) `search.rs:932`:**
`round_trip = rules.spread * contract_size + 2.0 * commission_per_lot` —
**spread MỘT lần, commission HAI lần, trong cùng một biểu thức.** Nếu spread cần
nhân đôi, nó đã nằm cạnh số `2.0` kia.

**Call site — F2 không bắn.** Mọi đường đóng lệnh đều gọi
`apply_costs(…, entering=false)`: `engine.rs:518` (strategy exit), `572`, `637`
(end of data), `787` (STOP), `796` (TARGET), `801` (TIMEOUT), `guards.rs:302`
(OPEN_LOSS), `307` (WEEKEND), `311` (NEWS), `paper.rs:682/714/730`. **Không có
đường nào trừ ở entry rồi quên exit.**

**(g) `py/live/spread_log.py:116`, nguyên văn: `spread = t.ask - t.bid`.**
Doc-comment khai nó ghi `time_utc,bid,ask,spread,mid,spread_bp` và *"nothing
else. It never aggregates, never decides, and never trades."* ⇒ cột `spread` là
**bề rộng báo giá tức thời**. **F3 không bắn.** (Doc-comment cũng khai
`symbol_select` **cố ý không gọi**: thêm một symbol vào Market Watch sẽ làm một
symbol không đọc được thành đọc được, và nó **không** nằm trong danh sách
read-only đã duyệt của tài khoản — symbol trả về rỗng thì **báo missing một
lần rồi bỏ qua**.)

**(h) `config/default.toml` nói gì — NGUYÊN VĂN**, vì brief yêu cầu đọc nguyên
văn xem nó nói "spread" hay "round trip":

    #   XAUUSD.sc  contract 1 oz     spread 28 pts    = $0.28
    # Spreads are per unit of underlying; times contract size for the per-lot cost.

Nó nói **"spread 28 pts"**. Nó **KHÔNG** nói "round trip", **không một lần
nào**. 28 points là `symbol_info.spread` của MT5 = `ask − bid` tính bằng point —
**chính xác cùng đại lượng** với cột của logger. ⇒ hai số **cùng loại**, phép
thay là **cùng loại**.

### Vì sao falsifier KHÔNG THỂ bắn — chỗ tiện nhất để đọc

Bề rộng báo giá `W`. Một lượt long: mua ở ask, bán ở bid. So với mid:
`(ask − mid) + (mid − bid) = W/2 + W/2 = W`. ⇒ **chi phí cả vòng, tính bằng
điểm giá, BẰNG bề rộng một chiều.** "Một chiều 0,220" và "cả vòng 0,220" là
**CÙNG MỘT SỐ.**

Và nó **bất biến với quy ước nến**: nến bid thì long vào ở `bar+W` ra ở `bar+0`;
nến ask thì vào `bar+0` ra `bar−W`; nến mid thì `W/2 + W/2`. **Cả ba đều tổng
bằng `W`.** Chỉ **chỗ chia** giữa entry và exit đổi — và chỗ chia chỉ chạm **δ**
(khuyết điểm 16), **không** chạm hoá đơn.

⇒ Kịch bản "thiếu một nửa" đòi engine trừ **½ × spread** cho cả vòng. **Nó trừ
1 ×.** Con số **0,440 là sai**, và **hồ sơ KHÔNG lạc quan ở các dòng trả
spread** — nó **bảo thủ**, trừ ba giờ ở mục 2.

---

## 2. Con số đúng — kèm PHÂN VỊ nào và GIỜ nào. Đo lại từ file, không trích

`data/spreads/XAUUSD_sc.csv`, **n = 8.967**, **2026-09-15T02:01:39Z →
2026-09-17T03:50:08Z**. Script `gr_spread.py`, output `spread-measured.txt`.

    min 0,2100  p10 0,2100  p50 0,2200  p90 0,2200
    p95 0,2600  p99 0,2900  max 0,2900  mean 0,22039

**⇒ con số đúng là `p50 = 0,2200`** của n=8.967 mẫu phủ 49,8 giờ. Trung bình
đồng-hồ-phẳng **0,22039** lệch khỏi nó **0,18%** ⇒ **với một dòng rải đều trên
đồng hồ, 0,220 là con số đúng.**

So sánh chính xác, vì hai cách nói đều lưu hành: 0,28 **cao hơn 0,220 là
27,3%**; 0,220 **thấp hơn 0,28 là 21,4%**. Cùng một phép thay.

### 2a. Câu config tự khai ĐÃ SAI — F6 BẮN

Config viết: *"the configured 0.28 is above every spread ever observed"*.

    mau > 0,28  :  251 / 8.967  =  2,80%
    mau >= 0,28 :  315 / 8.967  =  3,51%
    max         :  0,2900   ( > 0,28 )
    ca 251 mau nam trong GIO UTC 20, 21, 22 — khong mot mau nao ngoai do

Câu đó **đúng ở mẫu cũ** (n=2.315, 12,8 giờ, max 0,260) và **sai ở mẫu hiện
tại**. Nó cần một **ghi chú có ngày**, không phải một lần viết lại dòng cũ.

### 2b. GIỜ NÀO — và đây là chỗ cả hai số đều chỉ nói một nửa

Giờ trong **mọi** filter là **giờ NEW YORK**, không phải UTC — engine tự có
test: `fd-strategy/src/filter.rs::hours_are_read_on_the_new_york_clock`
(*"13:00 UTC in September is 09:00 New York"*). Log phủ tháng 9 ⇒ NY = UTC−4.

    NY hr  (UTC)      n     p50      mean      flat-0,220 so voi gio do
    00-04  (04-08)  ~360   0,2100   0,2148       +4,8%   (0,220 BI QUAN)
    05-11  (09-15)  ~360   0,2200   0,2155        0,0%
    12     (16)      360   0,2100   0,2149       +4,8%
    13-15  (17-19)   360   0,2200   0,2203        0,0%
    16:00  (20)      360   0,2600   0,24564     -15,4%   <-- 0,220 THU IT
    17:00  (21)      359   0,2900   0,27504     -24,1%   <-- 0,220 THU IT
    18:00  (22)      359   0,2600   0,24259     -15,4%   <-- 0,220 THU IT
    19-23  (23-03)  ~400   0,2100   0,2149       +4,8%

Dải p50 đi **0,2100 → 0,2900 = 38% của p50**, và `--spread=` nhận **một** số ⇒
**mọi chi phí/R trong hồ sơ là chi phí ở MỘT ĐIỂM của một phân phối 38%.**

### 2c. Cách định giá một DÒNG — suy thẳng từ `apply_costs`

Engine trừ nửa spread ở **nến khớp** và nửa ở **nến đóng** ⇒ hoá đơn của một
dòng **không phải** p50 toàn ngày mà là

    chi phi mot vong cua dong = ( spread[gio VAO] + spread[gio RA] ) / 2

| dòng | giờ vào → ra (NY) | hoá đơn thật | flat-0,220 | flat-0,28 |
|---|---|---|---|---|
| `ovn1900/long` | 19:00 → 02:00 | **0,2100** | **+4,8% bi quan** | +33,3% bi quan |
| một dòng 16:00 → 18:00 | 16:00 → 18:00 | **0,2600** | **−15,4% THU ÍT** | +7,7% bi quan |
| hai chân ở 17:00 | 17:00 → 17:00 | **0,2900** | **−24,1% THU ÍT** | **−3,4% THU ÍT** |

**⚠️ Đây là phát hiện đáng giữ nhất của job.** Ba giờ mà 0,220 thu ít —
**NY 16:00 / 17:00 / 18:00** — là **chính xác** dải mà `ADDENDUM-10 §IV` cô lập
được **toàn bộ** tiền của hồ sơ: *"`16:00-18:00` G **6,061** so với
`14:00-16:00` G **0,101** — sụt 60 lần"*, và **NY 17:00 = mốc nghỉ CME**, nơi
`open` của nến mở lại chính là đáy nến ở 19,02% số nến. ⇒ **định giá lại về một
số 0,220 PHẲNG biến một con số BẢO THỦ thành một con số LẠC QUAN ở đúng cái họ
dòng duy nhất mà hồ sơ còn coi là có tiền.** Và **0,28 cũng đã lạc quan 3,4%** ở
giờ đó.

---

## 3. Nó đổi GÌ — và nó đổi HAI thứ, ngược chiều

56 ô khai trước, **56 ô xem**, **4 lần chạy**. Bảng đầy đủ: `reprice-table.txt`.
Cửa sổ **ghim cả hai đầu tường minh**, số bar **in ra**:

    IS   --from=2010-06-01 --to=2018-06-01   kept 190.889 of 378.749 bars
    OOS  --from=2018-06-01 --to=2026-06-01   kept 187.860 of 378.749 bars

### 3a. Đối chứng DƯƠNG — F4 BẮN, ở mức mạnh nhất có thể

Hai lần chạy ở `--spread=0.28` **giống TỪNG BYTE** receipt `s0.28` của
`agent/cost-breakeven`, **cả hai cửa sổ, 0 dòng khác**, sau khi bỏ đúng một dòng
mang **đường dẫn worktree** của file thiết kế (khác theo cấu tạo). 14 dòng × 2
cửa sổ = **28/28 ô tái tạo**. ⇒ dụng cụ **nhìn thấy được**, nên mọi số dịch
chuyển dưới đây **đọc được**.

### 3b. Hai hiệu ứng, và chúng ngược chiều — đo được, không suy

`d risk/d s = +1/2` xác nhận từ mã: `engine.rs:700` `entry = apply_costs(...)`
đã dịch `+s/2`; `engine.rs:717-719` `risk = |entry − stop|` với stop cấu trúc
**chưa** dịch ⇒ `risk = f × range + s/2`. Hạ spread **cũng co mẫu số R**.

    hieu ung 1 (TOT) : hoa don moi vong giam 0,060 diem gia  =>  E tang
    hieu ung 2 (XAU) : mau so R co lai 0,030 diem gia        =>  Lbar co lai,
                       moi thu phong to, ke ca lo

**`Lbar` giảm ở ĐÚNG 28/28 ô** (−0,0005 … −0,0018 R trên 13 dòng;
−0,0085 / −0,0113 trên `ema-cross` stop hẹp) — đó là hiệu ứng 2, đo được,
**không một ô ngoại lệ**. `E` tăng ở **28/28 ô**. Hai cột đi ngược nhau, đúng
như khai trước.

### 3c. Và nó dịch **0/28 ô** qua cổng

    spread 0,28   : 0 dong qua CA HAI cua so   (4 o qua IS, 1 o qua OOS)
    spread 0,220  : 0 dong qua CA HAI cua so   (4 o qua IS, 1 o qua OOS)
    => dinh gia lai doi 0 ket cuc cong tren 14/14 dong

Toàn bộ biên độ dịch chuyển, 11 ứng viên:

    PF_r : +0,0060 .. +0,0143   (trung vi ~ +0,0090)
    E    : +0,0012 .. +0,0035 R
    Lbar : -0,0005 .. -0,0010 R
    DD$  : giam 1,4 .. 131,4 USD  (giam o 28/28 o)

**Một CHÂN lật, không một Ô nào lật:** `qs-h2` IS `PF_r` **1,1965 → 1,2108**
vượt vạch 1,200 — nhưng `E` của nó **+0,0400 < +0,050** nên ô vẫn trượt.
**1/28 ô lật một chân; 0/28 ô qua cả hai.**

Hai dòng gần cổng nhất mà brief chỉ định:

| dòng | cửa sổ | `PF_r` 0,28 → 0,220 | `E` 0,28 → 0,220 | `Lbar` | `n` | DD$ |
|---|---|---|---|---|---|---|
| `qs-lb3` | IS | 1,1865 → **1,1994** | +0,0387 → **+0,0412** | 0,2077 → 0,2067 | 494 → 494 | 880,24 → 872,25 |
| `qs-lb3` | OOS | 1,1730 → **1,1824** | +0,0361 → **+0,0379** | 0,2085 → 0,2077 | 452 → 452 | 1036,10 → 1014,37 |
| `ovn1900/long` | IS | 1,0899 → **1,1321** | +0,0078 → **+0,0113** | 0,0868 → 0,0853 | 1644 → 1644 | 936,71 → 820,25 |
| `ovn1900/long` | OOS | 1,0746 → **1,1028** | +0,0073 → **+0,0099** | 0,0978 → 0,0967 | 1635 → 1635 | 1004,49 → 873,12 |

`qs-lb3` vẫn **thiếu 0,0088 R** (IS) và **0,0121 R** (OOS) ở chân expectancy.
`ovn1900/long` **thiếu 0,0387 / 0,0401 R** — nó nhạy với spread **gấp 3–4 lần**
các dòng khác (`ΔPF_r` +0,0422 / +0,0282) vì `Lbar` của nó **nhỏ nhất bộ
(0,087)** nên `PF_r = 1 + E/Lbar` phóng to mọi thay đổi `E`; **không** phải vì
nó rẻ hơn. Và đúng dòng đó là dòng mà mục 2c cho thấy **cả 0,28 lẫn 0,220 đều
bi quan** (hoá đơn thật 0,2100).

**`expectancy_net`**: `swap 0,00/0,00` và `commission_per_lot = 0.0` trên
`xauduka` ⇒ `E_net ≡ E` **theo cấu tạo**, và engine **không in cột** (nó chỉ in
khi cột có thể khác). **Không phải một số 0 đọc từ một bộ đếm.**

**`sized_down_by_guard`**: receipt in `guards: nothing acted`, mà
`hypotheses.rs:747` khai **có nghĩa là `sized_down == 0`** (đọc thẳng field,
không grep chuỗi) ⇒ **0 ô cưỡi trần notional**, ở cả hai spread. `n` **bất biến
ở 27/28 ô**; ô duy nhất đổi là `ema-cross` IS **3134 → 3135** (+1 lệnh) — đúng
họ `Exits::Engine` mà phụ lục 11 §VI mô tả, và **nó là đối chứng, không phải
ứng viên**.

### 3d. F5 — probe cố ý nhìn trước, và nó TRƯỢT như phải trượt

`ovn1900/short`: cùng giờ, cùng phơi nhiễm, cùng sizing như `ovn1900/long`,
**chỉ lật chiều**.

    0,28  : PF_r 0,6459 (IS) / 0,7336 (OOS),   E -0,0393 / -0,0314
    0,220 : PF_r 0,6711 (IS) / 0,7527 (OOS),   E -0,0359 / -0,0288

**Trượt cả hai chân, cả hai cửa sổ, cả hai spread.** Định giá lại **không** làm
probe qua cổng ⇒ phép đo **không mù**. `ema-cross` (probe thứ hai) cũng trượt
mọi nơi (`PF_r` ≤ 0,8249).

### 3e. Thứ chặn vẫn là CỬA SỔ, và định giá lại không chạm vào nó

4 ô qua **IS** (`ts-l10`, `qs-h15`, `qs-qp020`, `qs-h30`) và 1 ô qua **OOS**
(`qs-lb30`) — **không giao nhau một dòng nào**, ở **cả hai** spread. Đúng hiện
vật cửa sổ thứ 13 mà `cost-breakeven` đã đo. **Giá không phải thứ chặn**, và
`cost-breakeven` đã đo `spread* = null` **kể cả ở spread 0** ⇒ **0,220 > 0 nên
không dòng nào có thể qua, và tôi không đi tìm một dương tính.**

**Phân vị: KHÔNG công bố.** `count match` của `qs-lb3` đọc **2,96** — receipt tự
in `** outside the band: this percentile is unmatched **` — và `null p50` của nó
**0,929 < 1** (brief §4).

---

## 4. CÁI GIÁ — nói thẳng

**Định giá lại làm mọi receipt cũ không tái lập.** Bốn điều, khai rõ:

1. **Tôi KHÔNG sửa `config/default.toml`.** Lý do không phải e dè: chính config
   viết *"It is NOT changed here … Repricing is an explicit, recorded amendment
   — the way the SPREAD repricing was done,
   `docs/decisions/2026-09-16-spread-repricing-scale.txt` — not a silent
   edit."* Tiền lệ đó **là một bản ghi, và config vẫn giữ 0,28 tới hôm nay.**
   Làm **cùng hình dạng** nghĩa là **thêm một bản ghi quy mô có ngày**, không
   phải đổi con số. Mọi phép đo ở đây dùng cờ `--spread=`. **0 chạm
   `config/local.toml`, `config/accounts.toml`, `main`.** Việc có đổi con số
   trong config hay không **vẫn là quyết định của chủ** — và mục 2c là lý do
   nên quyết nó cùng với câu hỏi "một số hay một trục theo giờ".
2. **Hai điều kiện config tự đặt cho bản sửa đổi: một ĐÃ đủ, một CHƯA.**
   *"the 21:00 UTC rollover hour and a weekend"* — giờ rollover **đã phủ**
   (359 mẫu ở UTC 21, và nó **chính là giờ đắt nhất**). **Cuối tuần thì CHƯA**:
   log chỉ có **Thứ Ba, Thứ Tư, Thứ Năm** 15–17/09, **49,8 giờ**, và
   `ADDENDUM-11 §I` ghi *"cả ba log … không chạy từ 17/09"*. ⇒ **điều kiện thứ
   hai của chính config vẫn chưa thoả.**
3. **Mọi đính chính là ghi chú có ngày THÊM vào CUỐI. 0 dòng bị xoá.** Bản ghi
   quy mô: `docs/decisions/2026-10-11-gold-reprice-scale.txt`, thuần **THÊM**.
4. **Không tạo lớp đính chính thứ hai.** `agent/receipt-reconcile` đo được hồ sơ
   nhiễm ở **tầng ĐẶC TẢ** — 25/34 dòng hỏng **chỉ vì** `--from`/`--to` bỏ
   trống, và **cả 25 trở về đúng** khi ghim lại cửa sổ (0/34 là lỗi đơn vị). ⇒
   chỗ cần sửa là **đặc tả**, không phải giá. Một lớp giá thứ hai sẽ **nhân**
   chỗ nhiễm. Bốn receipt của tôi **ghim cả hai đầu cửa sổ tường minh và in số
   bar của chính lần chạy**.

---

## 5. Sổ đa phép thử: khai vs xem

| | khai (`4132fba`) | xem |
|---|---|---|
| dòng | 14 (11 ứng viên + 3 đối chứng) | **14** |
| cửa sổ | 2, ghim hai đầu | **2** |
| arm guards | 1 (chỉ ON) | **1** |
| điểm spread | 2 (0,28 ; 0,220) | **2** |
| ô | 56 | **56, 0 ô không đọc được** |
| lần chạy | 4 | **4** |
| **ô cổng** | **0** | **0** |

**Không thêm dòng, không đổi cửa sổ, không thêm arm, không chỉnh một tham số
nào, không dịch một ngưỡng nào, không hạ một sàn cỡ mẫu nào. Không quét cỡ
stop** (bẫy 2 phụ lục 10).

**Hai thứ đưa vào SAU khi thấy số, dán nhãn tại chỗ:**

1. **Bảng giờ NY** (mục 2b / 2c). Nó **không** là ô backtest — nó là một lần
   đọc một file CSV có sẵn cộng một phép suy từ `apply_costs`. Nó vào báo cáo vì
   brief yêu cầu khai **"phân vị nào và giờ nào"**.
2. **Phép định giá theo dòng** `(s[vào] + s[ra])/2`. Suy từ mã, không hiệu
   chỉnh gì, không khớp tham số nào.

`qs-h15` và `qs-h30` in **y hệt trên IS** (393 lệnh, mọi con số giống) — cùng
một sổ, như `cost-breakeven` đã khai ⇒ **ứng viên phân biệt được là 10, không
phải 11**. Khai ở đây, không giấu trong bảng.

---

## 6. Thứ KHÔNG đo được, và vì sao

1. **Spread theo giờ TRONG engine.** `--spread=` là **một** số; không có
   override theo giờ. Bảng 2c là một phép định giá **ngoài** engine — nó
   **không** chạy qua backtest. Để đo thật cần một trục spread-theo-giờ trong
   `TradingRules`: **một thay đổi mã, và là quyết định của chủ.**
2. **Giờ vào/ra thực tế của 12 dòng không khai giờ.** Mục 2c định giá được
   `ovn1900/long` và `/short` vì thiết kế khai `from=1900 to=200`. 12 dòng còn
   lại (`quiet-swing`, `tsmom`, `ema-cross`) vào/ra **theo tín hiệu**, và
   receipt **không mang phân phối giờ vào/ra** ⇒ **không định giá được theo
   dòng**. Cần một cột mới trong output; **không suy được** từ receipt.
3. **Cuối tuần, và mọi thứ sau 17/09.** Log **không có** một ngày cuối tuần nào
   và dừng ở 17/09 ⇒ **spread lúc mở cửa Chủ Nhật không đo được**, và đó là
   **điều kiện thứ hai của chính config**.
4. **`XAUUSD` (feed của sàn) chưa định giá lại được.** Mọi ô ở đây trên
   `xauduka` (nến Dukascopy, **chi phí Vantage**). Nến `XAUUSD` **đóng băng ở
   2026-09-17** (`ADDENDUM-10 §IX`), và chạy `mt5_export.py` là **job khác**.
5. **Commission và hoàn phí.** `commission_per_lot = 0.0`; `[rebate]` ở
   `config/accounts.toml` **không chạm**. `0,220` ở đây là **chỉ spread**, không
   một đồng commission hay hoàn phí nào trong đó.
6. **Phân vị.** `count match` 2,96 **ngoài băng**, `null p50` 0,929 < 1 ⇒
   **không công bố** (brief §4). `--seeds=2` truyền chỉ vì mode đòi một con số.
7. **`wrong_side_stop`** — `grep -rn wrong_side_stop crates/` trả **0** trên cây
   này ⇒ **không có bộ đếm, không đo được**, và **không một số 0 nào** được đọc
   từ nó.
8. **Excursion trong lệnh.** Mọi drawdown là đường vốn **đã đóng lệnh**.
   `avg_mae` ở 0,220: `qs-lb3` **−0,397 / −0,385 R**, `ovn1900/long`
   **−0,194 / −0,212 R**. **Không ô nào in `_pct > 100%`** ⇒ **không dòng nào
   đã cháy.**
9. **Quy ước nến của `XAUDUKA`.** Config khai nó là **"Dukascopy bid feed"**.
   Nếu nến là **bid** thì chỗ chia đúng là `+W` ở entry long và `0` ở exit, chứ
   không phải `W/2 + W/2`. **Tổng một vòng KHÔNG đổi (= W, mục 1)**, nhưng **δ**
   của khuyết điểm 16 sẽ là `W` chứ không phải `W/2` cho long. Đã đo
   `δ = 0,138 ≈ 0,28/2` ⇒ engine thi hành `W/2 + W/2`. **ĐẾM, KHÔNG SỬA** — nó
   chạm `sizing_stop` và sẽ viết lại mọi receipt.
