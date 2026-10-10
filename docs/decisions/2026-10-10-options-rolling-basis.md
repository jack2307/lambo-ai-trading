# 2026-10-10-options-rolling-basis: basis CUỘN thay hằng số, và 24 ô chạy lại

**Viết:** 2026-10-10, **trước dòng code đầu tiên** và trước khi có bất kỳ số
lệnh / PF / expectancy nào trên trục này trong worktree này.
**Agent:** `options-basis`, nhánh `agent/options-basis`, worktree
`/e/rust/fd-options-basis`.

**Gốc nhánh, đo chứ không nhận từ brief.** Brief ghi worktree cắt từ
`agent/stop-width`. `git log -1` khi tôi mở worktree đọc **`29c8635`**, đúng tip
của `agent/stop-width`, **không** phải `e619e07`; và `git merge-base --is-ancestor`
nói `29c8635` là tổ tiên đúng nghĩa của `agent/options-first`. Nên tôi
**fast-forward** `agent/options-basis` lên `e619e07` (`--ff-only`, không merge
commit, không cherry-pick), và từ đó worktree mang bản vá đường `--fixed` cho
options, `lbar_line`, `config-ofirst/` và sáu receipt của `options-first`. Hai
cách đọc khác nhau về cùng một worktree; con số của tôi là `29c8635 → e619e07`,
và điểm đến thì cả hai khớp.

**Binary: tự build trong `target-ob/` của worktree này.**
`/e/rust/fd-stop-width/target-sw/release/search.exe` **không** biết
`--basis-offset=` lẫn cờ cuộn của job này, và một binary cũ **nuốt im lặng** cờ
nó không biết (chạy, exit 0, không in gì) ⇒ dùng nó ở đây sẽ cho một receipt
trông như một phép đo mà thật ra là run không-basis.
**Trục:** basis GC → XAUUSD spot, thứ duy nhất đứng giữa hồ sơ và một phán
quyết trên họ cơ chế dòng tiền options vàng.

## Đăng ký cha, và vì sao job này tồn tại

`docs/decisions/2026-10-09-options-one-window.md` (nhánh `agent/options-first`)
đã tiêu **24 ô** và đóng lại ở **CHƯA QUYẾT ĐƯỢC**, vì F3 của chính nó bắn:

    basis +41,26 (p10)   100 lenh   PF_r 1,0057   E +0,0036 R   -> truot chan PF
    basis +43,70 (mean)  108 lenh   PF_r 0,1735   E -2,8924 R   -> truot chan PF
    basis +45,78 (p90)    94 lenh   PF_r 1,3883   E +0,2170 R   -> qua ca ba chan

Một thay đổi **4,52 USD** trong một **hằng số** — nằm trong chính p10–p90 của
basis đã đo — lật `PF_r` từ **0,1735** sang **1,3883**. Và basis không phải một
hằng số: GC − XAUUSD trên 6.718 phút trùng cho mean **+43,70**, sd **1,91**,
p10 **41,26** / p90 **45,78**, và trôi đơn điệu **45,66 → 44,19 → 43,59 →
41,35** qua bốn tứ phân vị. Ở `entryAtr = 0,35` dung sai vào lệnh là **1,58
USD** còn sai số dư của basis là **1,91 USD** — **sai số lớn hơn dung sai.**

Đăng ký cha tự ghi mục số 1 trong "thứ gì sẽ giải quyết nó": **một basis cuộn,
không phải một hằng số.** Đây là job đó.

## Giả thuyết, một câu

Trên đúng cửa sổ đã đo (`XAUUSD-15m`, 196,5 giờ trùng tape, 786/100.586 bar =
0,8%), một basis **CUỘN chỉ dùng thông tin quá khứ** cho **một** phán quyết ổn
định cho họ bốn cơ chế đọc options — ổn định theo nghĩa **không đổi qua ba độ
dài cửa sổ cuộn** — thay cho ba phán quyết khác nhau mà ba hằng số cho ra.

## THIẾT KẾ, chốt và khai TRƯỚC khi chạy

### 1. Basis cuộn ước từ đâu

Hai chuỗi, và **cả hai đều là thứ engine đang cầm trong tay ở chỗ gọi**:

* **trục GC**: `Frame::spot` của timeline — spot mà engine options đọc ra từ
  tape COMEX GC, một frame mỗi **300.000 ms**.
* **trục spot giao dịch được**: `Bar::close` của `XAUUSD-15m`.

Quan sát thô tại bar *j*: `b_j = spot(frame cuối cùng có t <= bar_j.time) −
bar_j.close`. **Không** dùng `data/bars/GC-1m.parquet` cho việc này, vì chuỗi
đó chỉ phủ 2026-09-06 → nay còn `XAUUSD-1m` đã đóng băng ở 2026-09-11, nên cặp
1m/1m chỉ cho **6.718 phút**; cặp (frame, bar 15m) phủ **toàn bộ 786 bar trùng**
— đúng cái cửa sổ sẽ bị đo.

Mỗi quan sát được gắn **thời điểm nó trở thành biết được**: `bar_j.time +
interval_ms` (giãn cách modal của bộ nến, đọc như `companion::interval_ms`),
tức là lúc nến đóng. Một quan sát mà chưa đóng nến thì chưa tồn tại.

### 2. Thống kê, cửa sổ, cadence — khai trước, KHÔNG quét

* **Thống kê: TRUNG VỊ.** Chốt trước, không so với trung bình. Lý do: hiện tượng
  thối ở offset mean mà `options-first` đo được (`Lbar 3,4998 R` ở
  `level-reversion`, `11,0194 R` ở `flow-at-level`, so với 0,47–0,72 R ở hai
  offset kề) sinh ra từ **vài** lệnh mà `risk = entry − (cluster.low − 0,3×ATR)`
  gần 0; một trung vị không bị vài điểm đó kéo. Và vì sd 1,91 phần lớn **là
  drift chứ không phải nhiễu**, trung vị và trung bình trên một cửa sổ ngắn gần
  nhau, nên lựa chọn này **tốn ít và an toàn hơn** — nó không phải một tham số
  được chọn sau khi xem số.
* **Cadence: mỗi FRAME** (mỗi 5 phút), không phải mỗi phiên. Lý do: frame là
  thứ engine đọc; ước basis ở đúng độ phân giải của thứ nó sửa thì không tạo ra
  một bậc nhảy ở ranh giới phiên — và một bậc nhảy ở ranh giới phiên sẽ là một
  hiện vật mới do chính tôi chế ra.
* **Độ dài cửa sổ: BA mức, khai trước, `W ∈ {120, 480, 1440} phút`**
  (2h / 8h / 24h). Ba mức này **là trục nhạy cảm bắt buộc**, đúng vai mà ba
  offset hằng đóng trong đăng ký cha: nếu phán quyết đổi qua ba `W` thì câu trả
  lời là **basis không phải thứ sửa được bằng ước lượng tốt hơn**, không phải
  "lấy cái tốt nhất trong ba".
* **Số quan sát tối thiểu: `K = 5`, giữ NGUYÊN ở cả ba `W`.** Giữ nguyên để
  thứ duy nhất đổi giữa ba ô là độ dài cửa sổ. 5 là mẫu nhỏ nhất mà trung vị
  còn phân biệt được với hai đầu mút.

### 3. NHÂN QUẢ — chỉ thông tin quá khứ, và một test khẳng định nó

Basis cho frame tại `t` lấy **trung vị các quan sát có thời điểm-biết-được
NẰM TRONG `(t − W, t)`**, mốc trên **mở** — nghĩa là một nến đóng đúng tại `t`
cũng **không** vào. Hệ quả số học, viết ra để kiểm được: bar *i* dùng frame tại
`t <= bar_i.time`; một quan sát *j* vào được đòi `bar_j.time + interval < t <=
bar_i.time` ⇒ `j < i`. **Nến hiện tại không bao giờ góp vào basis của chính
nó.**

**Test nhân quả (kiểu `companion::a_truncated_primary_is_a_prefix_and_so_is_a_truncated_companion`):**
cắt chuỗi frame ở một mốc, và cắt chuỗi bar ở cùng mốc đó, **không được đổi một
giá trị basis nào tại hoặc trước mốc đó** — so bằng `fd_core::parity_eq` để
`NaN` khớp `NaN`. Hai phép cắt là hai cách khác nhau mà một phương pháp hai
chuỗi đọc được tương lai; test phải bắt cả hai.

### 4. Khi vùng trùng THIẾU — khai trước, và `null != 0`

Tape phủ **786/100.586 bar = 0,8%**. Ngoài vùng trùng, và trong vùng trùng mà
cửa sổ cuộn có **ít hơn `K` quan sát**, basis là **NULL**.

**Quyết định: TỪ CHỐI VÀO LỆNH, không dùng giá trị cuối.** Cơ chế: frame đó
được giữ nguyên về thời gian và về mọi trường **không phải giá**, nhưng
`clusters` và `contexts` bị **làm rỗng**. Hệ quả, đọc từ `builtin.rs` chứ không
đoán:

* `level-reversion` và `flow-at-level` lặp trên `options.clusters` ⇒ rỗng ⇒
  `Intent::None`.
* `maxpain-magnet` lấy `options.contexts.first()` ⇒ `None` ⇒ `Intent::None`.
* `flow-momentum` chỉ đọc `bull_ratio_15m` và `net_flow_velocity_norm` ⇒
  **không bị chạm**, đúng như phải thế: nó không cần basis.

Vì sao không "giá trị cuối": trên tape này bar đóng băng ở 2026-09-17 còn tape
chạy tới 2026-10-09, nên "giá trị cuối" sẽ là một hằng số cũ **22 ngày** được
áp im lặng — tức là đúng cái khuyết điểm job này đi sửa, mặc áo khác. Và một
frame bị từ chối **không được in thành 0**: receipt phải đếm riêng số frame
NULL và số frame có basis.

**KHÔNG xoá frame NULL khỏi timeline.** Xoá thì `view_from` sẽ trả về một frame
**cũ hơn** cho bar kế tiếp — tức là mức cũ với basis cũ, tệ hơn cả không sửa.

### 5. Hướng dịch, giữ nguyên của đăng ký cha

Dịch **mức xuống**, không dịch **nến lên**: nến là tài khoản. Các trường bị
dịch đúng bằng danh sách `shift_basis` đang dịch — `Frame::spot`,
`cluster.low/high/center`, và `max_pain/poc/w_sup/w_res/call_be/put_be` trong
mọi context. `score`, `dte`, các tỉ lệ dòng tiền và velocity **không phải giá**
và không bị chạm.

## Tiền kiểm, chạy TRƯỚC khi tiêu ô nào (§5: bắn ở tiền kiểm thì tiêu 0 ô)

* **PC1 — chuỗi basis thô trên ĐÚNG cửa sổ sẽ bị đo.** In `n`, mean, sd, p10 /
  p50 / p90 và trung bình bốn tứ phân vị của `b_j` trên 786 bar trùng, cạnh ba
  con số của đăng ký cha (6.718 phút 1m). **Nếu dải này nằm ngoài [41,26;
  45,78] thì ba hằng số KHÔNG kẹp được basis**, và điều đó phải nói ra trước
  khi đọc bất kỳ ô nào.
* **PC2 — test nhân quả** ở §3. Không xanh thì **dừng**, 0 ô.
* **PC3 — đối chứng dịch**: `flow-momentum` phải in **giống hệt** ở cả ba `W`
  **và** giống hệt ba offset hằng, trong cùng một arm. Nó không đọc mức giá nào.
* **PC4 — bộ đếm NULL**: bao nhiêu frame có basis, bao nhiêu NULL, ở mỗi `W`.
  Nếu một `W` cho NULL ở gần như mọi frame trong vùng trùng thì ô của nó không
  phải một phép đo và phải báo thế.

## FALSIFIER, cụ thể và bắn được

**F1 — cái đóng một hướng tốn kém.** Basis cuộn cho phán quyết **nằm GIỮA** ba
hằng số (tức không trùng với bất cứ cái nào trong ba, hoặc trùng cái này ở `W`
này và cái khác ở `W` khác) **và vẫn đổi theo `W`** ⇒ **basis không phải thứ
sửa được bằng một ước lượng tốt hơn.** Kết luận: tape GC và nến XAUUSD **không
ghép đủ chặt** để đo họ cơ chế này, và desk cần **một nguồn options trên chính
XAUUSD**, không phải một phép hiệu chỉnh. Đó là một kết luận sạch và nó **đóng**
một hướng.

**F2.** Phán quyết **ổn định qua cả ba `W`** ⇒ trục này đọc được bằng một basis
cuộn, và kết quả được báo là **MỘT CỬA SỔ**, kèm đúng thứ sẽ xác nhận nó. Dù
dấu là gì. **Không gọi là candidate** (xem §"Cái này KHÔNG phải gì" dưới).

**F3.** Test nhân quả §3 đỏ ⇒ **dừng ngay, 0 ô cổng**, báo rằng basis cuộn như
tôi dựng nhìn trước.

**F4.** `flow-momentum` **đổi** giữa hai cấu hình basis bất kỳ ⇒ phép dịch của
tôi chạm thứ nó không được chạm ⇒ **dừng**, mọi ô khác không đọc được.

**F5.** Số lệnh của một cơ chế **đổi** so với ba hằng số ⇒ nói rõ đổi bao nhiêu
và ở đâu, vì ba cơ chế `maxpain-magnet` (24–29), `flow-at-level` (20–30),
`flow-momentum` (4) đang **dưới sàn 40 lệnh** và vì thế **KHÔNG CÓ KẾT LUẬN**,
**không phải bị bác bỏ**. Nếu basis cuộn đưa một trong ba lên trên 40 thì ô đó
trở thành một phép đo mới và phải được báo như thế.

**F6.** Luật riêng của cơ chế nổ **0 lần** trên một dòng qua cổng (bẫy
`tsmom/120d`) ⇒ dòng đó không phải một phép đọc cơ chế đó. `--exit-mix` ở mọi ô.

## Cái này KHÔNG phải gì — khai trước để không đọc sai về sau

**Một ô qua cổng ở đây KHÔNG phải một ô qua cổng.** Cổng desk đòi **HAI** cửa
sổ (brief §4); tôi chỉ có **196,5 giờ trùng** và nó là **MỘT** cửa sổ. **Mười
hai hiện vật cửa sổ** trong hồ sơ nói một cửa sổ không phải một kết quả, và cái
thứ 12 **chính là hằng basis** này. **Không có gì ở đây được gọi là candidate.**

**Không công bố phân vị.** `count match` ngoài băng ở **3/4** cơ chế (1,63–12,25
ở đăng ký cha), `cost match` **1,77–22,90**, và `null p50` dưới 1,000 ở mọi
dòng (0,700–0,907), nên một phân vị cao ở đây nghĩa là "lỗ ít hơn vào lệnh ngẫu
nhiên", không phải "có lãi". **Báo cổng một mình**, in `count match` cạnh để
người đọc thấy phân vị không đọc được.

**Không quét tham số.** Mọi ô chạy `default_params()` của chính cơ chế dưới
`--fixed` trên toàn cửa sổ. Không fold, không chọn, không best-of.

## Sổ đa phép thử, khai trước lần chạy đầu

| nhóm | ô | mới hay chạy lại |
|---|---|---|
| basis CUỘN: 3 `W` × 4 cơ chế × 2 arm guards | **24** | **MỚI** |
| đối chứng hằng: 3 offset × 4 cơ chế × 2 arm | **24** | **CHẠY LẠI** ô mà `options-first` đã tiêu |
| tổng xem | **48** | |

**Ô cổng mới tiêu: 24.** Hai mươi bốn ô hằng là **đối chứng bắt buộc chạy cùng
lúc, không phải phép thử mới** — không có chúng thì không đọc được basis cuộn
nằm đâu trong dải ba hằng số, và theo §II của phụ lục 8 (hồ sơ **không chạy lại
về đúng số của chính nó**: chỉ 295/452 ô khớp `PF_usd` đã công bố) **không được
trích số của `options-first` như một dữ kiện** — phải chạy lại và trích số của
chính lần chạy này.

Mọi mở rộng ngoài 48 ô này là **một đăng ký mới**. Vẫn đóng, đừng hỏi lại:
gamma wall (IV/bid/ask null 100%), venue-flag sweep/block (0 true ở mọi print),
và quy nguyên theo từng loại mức (cluster gộp 9 loại, 5 đánh dấu experimental).

## Cách đọc

* **Cổng một mình**, đếm tay với **40**, không phải `need 30` của tool. Và nói
  chân nào ràng buộc: theo phụ lục I/§I chân expectancy **DƯ** đúng khi
  `Lbar >= 0,250R`, và `lbar_line` in `Lbar` ra.
* **`PF_r` cạnh `PF_usd`, khai đơn vị cả hai.** Chiều của khe **không phải hằng
  số của engine** (phụ lục 9 §I: bốn phép đo, bốn kết quả) ⇒ **đo trên bộ này,
  không trích một tỉ lệ**.
* **`E = total_r / n`**, không dùng field `expectancy` 3 chữ số.
* **Drawdown USD cạnh mọi số lợi nhuận**; `_pct` là **SÀN**; `_pct > 100%` ⇒
  dòng đã **CHÁY**, không đọc `PF` của nó. `[markets.xauusd]` là sổ **100 USD**
  ở `min_lot` ⇒ khuyết điểm 17 áp dụng, và tỉ lệ `sized down` phải báo (khuyết
  điểm `cap_lots`, cả hai chân cổng mù với nó).
* **`--exit-mix` ở mọi ô**, và luật riêng của cơ chế phải nổ.
* **Cỡ stop thực, bằng ATR và bằng điểm giá**, từ dòng receipt tự in; và
  **cost/R kèm cửa sổ + thống kê + cỡ stop + spread đo hay GIẢ ĐỊNH** (phụ lục
  9 §II: bốn thứ, không phải một).
* **Số print và số bar của CHÍNH lần chạy này.** Kho đang được ghi trong lúc
  đọc (`collect.exe` pid 5044 / 38720, tape đi 46.846 → 53.479 trong ba ngày),
  nên hai lần đọc cho hai con số và **cả hai đều đúng**; một receipt trên trục
  này chỉ so được với một run in đúng hai số đó.
* **Arm**: arm **có guards** là arm duy nhất chủ cho phép. Kết quả chỉ sống ở
  arm không-guards ⇒ **nói thẳng là không giao dịch được.**
* `null` không bao giờ in thành `0`.

## Công sức khai trước

Nếu quá ~4 giờ đồng hồ tường thì **dừng và báo thứ đã đo được**, không chạy quá.
Không build `cargo test --workspace` ở debug (phình `target/debug` lên 11 GB);
`--release`, `-p fd-backtest`, target dir riêng `target-ob/` trong worktree này.
Không chạm `config/`, `main`, `data-sealed/`, VPS, hai `collect.exe`.

---

## Ghi chú thêm 2026-10-10, SAU tiền kiểm và TRƯỚC khi tiêu ô nào — PC1 lật một tiền đề của chính đăng ký này

Theo §8 của brief ("số đo thắng"), đây là ghi chú thêm vào cuối, không viết lại
dòng nào ở trên.

### `Frame::spot` KHÔNG phải một chuỗi giá — nó là 26 tháng hợp đồng trộn vào nhau

Đăng ký ở trên chọn trục GC là `Frame::spot`. Đọc mã (`fd-engine/engine.rs:191-195`):
`spot` là `underlying_price` của **print cuối cùng**, bất kể print đó thuộc
expiry nào. Đo trực tiếp trên 55.131 print của kho (915 file parquet):

    26 symbol expiry khac nhau
    underlying_price trung binh theo symbol: 4.146,00 (G2RV6) ... 4.385,32 (OG4U6)
      => 239 USD giua hai dau, vi chung la quyen chon tren CAC THANG GC KHAC NHAU
         (U6=09/2026, V6=10, X6=11, Z6=12)

    Do giai toa cua underlying_price TRONG CUNG MOT PHUT, 8.316 phut co >1 print:
      mean +10,66  sd 15,48  p10 +0,00  p50 +1,00  p90 +35,10  max +58,10 USD
      > 0,01 USD o 6.744/8.316 phut = 81,1%
      > 5,00 USD o 2.456      phut = 29,5%

Và chạy thật với `Frame::spot` làm trục GC cho basis thô **mean +16,48 sd 16,40
min −26,51 p10 +1,50 p50 +10,58 p90 +44,58 max +89,01** — một dải 115 USD, tức
**không phải một basis mà là độ giải toả giữa các tháng hợp đồng.**

⇒ **Sửa thiết kế, khai ở đây trước khi tiêu ô:** trục GC của basis cuộn là
**`data/bars/GC-1m.parquet`** (close), không phải `Frame::spot`. Chuỗi đó do
`collect.exe` ghi, **100,0% không có biên độ** (18.707/18.709 bar có
`o==h==l==c`) nên close chính là giá tham chiếu từng phút, và nó là **một**
chuỗi. Cặp (GC-1m, XAUUSD-15m) phủ **785 bar**, đúng cửa sổ sẽ bị đo — nên lý
do ở §1 để không dùng cặp 1m/1m vẫn đúng, chỉ chuỗi GC là đổi.

### PC1 trên cặp đúng: ba hằng số của `options-first` KHÔNG kẹp được basis của cửa sổ nó áp lên

GC-1m close − XAUUSD-15m close, ghép trong cùng nến 15m (GC phút thứ 14 của nến),
**785 bar ghép được**:

    mean +42,16  sd 2,31  min +36,94  p10 +39,66  p50 +41,71  p90 +45,48  max +47,11
    trung binh bon tu phan vi, THEO THOI GIAN:  +45,15 -> +42,84 -> +40,82 -> +39,86

Tái lập được **hình dạng** của tiền kiểm cha (mean +43,70 sd 1,91, drift
45,66 → 41,35) nhưng **không tái lập con số**, và lệch về phía thấp:

* **drift KHÔNG dừng ở 41,35 — nó đi tiếp tới +39,86.** Tiền kiểm cha đo tới
  2026-09-11 (vì `XAUUSD-1m` đóng băng ở đó); cửa sổ bị đo đi tới 2026-09-17.
* ⇒ **41,26 không phải p10 của cửa sổ này mà là ~p33; 45,78 nằm trên p90 (45,48)
  và gần max (47,11).** Ba hằng số p10/mean/p90 mà `options-first` chạy là
  phân vị của **một cửa sổ khác, ngắn hơn, trên một cặp chuỗi khác** — và trên
  cửa sổ chúng được áp lên, chúng **lệch cao** và **không kẹp** basis.
* Theo phụ lục 8 §II, đây đúng là hạng "hồ sơ không chạy lại về đúng số của
  chính nó", và luật §8 áp: **con số của tôi thắng, và tôi báo nó.**

### Thêm một hệ quả: ba offset hằng là số IN-SAMPLE

Test `a_deliberately_look_ahead_basis_fails_the_same_cut_test` trong
`basis.rs` dùng "trung vị toàn mẫu" làm một trong hai probe nhìn trước, **và
trung vị toàn mẫu đúng là đại lượng mà mỗi giá trị `--basis-offset=` mang.**
Probe đó **trượt** test cắt chuỗi ở **5/5** mốc. ⇒ p10/mean/p90 của toàn vùng
trùng **không phải thứ một người giao dịch biết được tại thời điểm đó**; chúng
là đối chứng, không phải một cấu hình chạy được. Nói ra ở đây vì nó đổi cách
đọc 24 ô hằng: chúng là **thước đo độ nhạy**, không phải ba ứng viên.

### Ghi chú về probe và về chính test của tôi

Bản test cắt chuỗi đầu tiên của tôi cắt theo **chỉ số bar** và **mù**: probe
"đóng dấu quan sát tại giờ MỞ nến" (một số hạng sai, đúng cái "basis đọc phút
hiện tại" mà đăng ký cấm) **đi qua** nó. Sửa thành cắt theo **tính biết được**
(`frame.t <= tau` và `bar.time + interval <= tau`) ở **5 mốc, 4 trong đó KHÔNG
nằm trên lưới 15m**. Đo được: probe đó bị bắt ở **4/5** mốc — **không bắt được
ở đúng mốc nằm trên lưới bar**, vì sai số của nó bằng đúng một interval và một
phép cắt theo interval xoá đúng các bar nguyên. Giữ lại con số 4/5 và lý do,
để không ai "sửa" test về bản mù.

Và một thứ đo được rồi mới biết: **mốc trên ĐÓNG (`(t−W, t]`) cũng nhân quả**,
không phải lỗi — quan sát biết được đúng tại `t` đến từ một nến đóng tại `t`,
còn nến đọc frame tại `t` thì mở tại hoặc sau `t`, nên vẫn là một nến sớm hơn.
Mốc mở mà module này dùng là lựa chọn **bảo thủ**, không phải lựa chọn bắt buộc.

### Sổ đa phép thử: KHÔNG đổi

48 ô xem (24 cuộn mới + 24 hằng chạy lại) giữ nguyên. Thay đổi ở trên là **đổi
chuỗi tham chiếu của một ước lượng**, không phải thêm một ô cổng, và nó được
khai **trước** khi ô nào được tiêu. `W ∈ {120, 480, 1440}` và `K = 5` giữ
nguyên. Không có ô nào bị chi cho `Frame::spot`: lần chạy tiền kiểm ở trên
không đọc một chân cổng nào và con số của nó chỉ dùng để bác bỏ chính nó làm
trục.
