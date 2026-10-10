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

---

## Ghi chú thêm 2026-10-10, SAU lần chạy — KẾT QUẢ: F1 BẮN, và basis ĐÚNG cũng trượt cổng

Basis cuộn ước đúng tới **+0,01 USD** ở trung bình. Ở đúng basis đúng đó cơ chế
**trượt** cổng. Hai ô duy nhất qua cổng trong 72 ô đòi một hằng số lệch **+3,76
và +4,06 USD VỀ CÙNG MỘT PHÍA** so với trung vị của cửa sổ. Thứ không sửa được
không phải **ước lượng** mà là **bài toán**.

Dụng cụ: `search.exe` build từ nhánh này
(`cargo +stable-x86_64-pc-windows-gnu build --release -p fd-backtest --bin
search`, target dir `target-ob/` trong worktree này). **Không** dùng
`fd-stop-width/target-sw/release/search.exe`: nó không biết `--basis-roll=`
và nuốt im lặng cờ nó không biết.

Receipt: `docs/research/runs/2026-10-10-options-rolling-basis/` — 18 file
(`roll-<W>-<arm>`, `const-<offset>-<arm>` cho ba phân vị tôi đo được,
`parent-<offset>-<arm>` cho ba hằng số của `options-first`) + `SUMMARY.txt`.
Lệnh, giống nhau ở cả 18 trừ cờ basis và `--guards`:

    search --market=xauusd --interval=15m --mode=hypotheses --fixed --exit-mix
           --config=config-ofirst --data=/e/rust/flowdesk/data --seeds=200
           --basis-ref=GC-1m
           --batch-file=docs/hypotheses/2026-10-09-options-one-window.toml
           {--basis-roll={120|480|1440} --basis-roll-min=5 | --basis-offset=<usd>}
           [--guards]

**Không công bố phân vị, và không con số nào dưới đây là một phân vị.**

### Số của CHÍNH lần chạy này, vì kho đang được ghi trong lúc đọc

    prints   55.131       (options-first doc 53.357 -> 53.479 hai ngay truoc)
    frames    9.491       (options-first doc 9.429 / 9.431)
    bars    100.586       XAUUSD-15m 2022-06-16 -> 2026-09-17 (DONG BANG)
    GC-1m    18.709       2026-09-06 22:08 -> 2026-10-09 20:59
    trung       786 / 100.586 bar = 0,8%   <- toan bo phep do nay
    quan sat    787        cap (GC-1m close, XAUUSD-15m close) trong cung mot nen
                           (probe Python doc 785: no lay dung phut thu 14 cua nen,
                            engine lay bar GC CUOI bat dau truoc gio dong nen.
                            Hai cach ghep, hai so, CA HAI DUNG - mean +42,16 vs
                            +42,17, p10 +39,66 vs +39,67. Ghi ca hai.)

Một receipt trên trục này chỉ so được với một run in **đúng** bốn số đầu.

### KẾT QUẢ CHÍNH — basis cuộn KHÔNG ĐƯA ô nào qua cổng, và nó vẫn đổi theo `W`

`level-reversion`, arm **guards** (arm duy nhất chủ cho phép). Cổng đếm tay
với **40** lệnh, không phải `need 30` của tool:

| basis | n | PF_r (R) | PF_usd (USD) | khe | E (R) | Lbar (R) | DD USD | DD % | cổng |
|---|---|---|---|---|---|---|---|---|---|
| **CUỘN W=120m** | 108 | **1,1310** | 1,0650 | +0,0660 | +0,0799 | 0,6100 | 4,51 | 4,44 | **TRƯỢT (PF_r)** |
| **CUỘN W=480m** | 110 | **1,1021** | 1,0510 | +0,0511 | +0,0631 | 0,6185 | 4,22 | 4,14 | **TRƯỢT (PF_r)** |
| **CUỘN W=1440m** | 104 | **1,0604** | 1,0974 | −0,0370 | +0,0379 | 0,6281 | 3,80 | 3,65 | **TRƯỢT (PF_r + E)** |
| hằng +39,66 (p10 đo được) | 101 | 1,0704 | 1,2179 | −0,1475 | +0,0432 | 0,6131 | 7,25 | 6,92 | TRƯỢT |
| hằng +41,26 (p10 của cha) | 100 | 1,0057 | 1,0470 | −0,0413 | +0,0036 | 0,6378 | 7,22 | 7,07 | TRƯỢT |
| hằng +42,16 (mean đo được) | 105 | 0,8884 | 0,8932 | −0,0048 | −0,0745 | 0,6671 | 7,58 | 7,42 | TRƯỢT |
| hằng +43,70 (mean của cha) | 108 | **0,1735** | 1,1790 | −1,0055 | −2,8924 | **3,4998** | 2,84 | 2,70 | TRƯỢT (thoái hoá) |
| hằng +45,48 (p90 đo được) | 97 | **1,4259** | 1,3437 | +0,0822 | +0,2348 | 0,5514 | 3,66 | 3,36 | **QUA cả ba chân** |
| hằng +45,78 (p90 của cha) | 94 | **1,3883** | 1,4015 | −0,0132 | +0,2170 | 0,5589 | 3,77 | 3,48 | **QUA cả ba chân** |

Đọc ba dòng đầu cạnh sáu dòng sau:

1. **Dải sáu hằng số đi từ `PF_r` 0,1735 đến 1,4259.** Ba ô cuộn nằm **1,0604
   – 1,1310**, tức **trọn trong dải đó**, giữa +41,26 (1,0057) và +45,48
   (1,4259). Không trùng với bất cứ hằng số nào.
2. **Và nó vẫn đổi theo tham số cuộn**: `PF_r` đi **1,0604 → 1,1021 → 1,1310**
   **đơn điệu theo `W`** (cửa sổ dài hơn → `PF_r` thấp hơn), biên độ **0,0706**
   qua một dải `W` gấp **12 lần**. Khoảng cách từ ô cuộn tốt nhất tới vạch cổng
   là `1,200 − 1,1310 = 0,0690` ⇒ **cái tham số cửa sổ một mình nó dịch
   102% khoảng cách còn lại tới cổng.** Ước lượng vẫn quyết định nhiều như thị
   trường.
3. ⇒ **F1 BẮN, đúng như khai trước.**

### Ước lượng cuộn ĐÚNG — nó dựng lại basis thật tới 0,01 USD ở trung bình

Đây là con số quyết định cách đọc mọi thứ ở trên, và nó chỉ đọc được sau khi
tôi đếm theo **bar** thay vì theo frame. `basis AS EACH TRADABLE BAR SAW IT`,
một giá trị mỗi nến giao dịch được, đặt cạnh **basis đương thời thật** (787
quan sát GC-1m − XAUUSD-15m):

    THAT (duong thoi)  n 787 mean +42,17 sd 2,31 p10 +39,67 p50 +41,72 p90 +45,48
                       tu phan vi theo thoi gian +45,16 -> +42,86 -> +40,82 -> +39,86
    CUON W=120         n 743 mean +42,18 sd 2,27 p10 +39,81 p50 +41,76 p90 +45,42
                       tu phan vi theo thoi gian +45,15 -> +42,87 -> +40,80 -> +39,91
    CUON W=480         n 776 mean +42,25 sd 2,31 p10 +39,99 p50 +41,63 p90 +45,51
                       tu phan vi theo thoi gian +45,30 -> +43,08 -> +40,74 -> +39,89
    CUON W=1440        n 776 mean +42,52 sd 2,24 p10 +40,15 p50 +41,69 p90 +45,82
                       tu phan vi theo thoi gian +45,55 -> +43,57 -> +40,79 -> +40,18

Sai số của ước lượng **chỉ dùng quá khứ** so với sự thật:

    W=120 : mean lech +0,01 USD   p50 lech +0,04 USD   sd lech -0,04   drift tai lap trong 0,05 USD
    W=480 : mean lech +0,08 USD   p50 lech -0,09 USD   sd lech  0,00
    W=1440: mean lech +0,35 USD   p50 lech -0,03 USD   sd lech -0,07

⇒ **Bài toán ước lượng basis đã GIẢI.** Một trung vị cuộn 2 giờ, nhân quả,
không đọc một phút tương lai nào, dựng lại basis đương thời với sai số trung
bình **+0,01 USD** trên **2,31 USD** độ phân tán — và tái lập cả cái drift đơn
điệu +45,15 → +39,91.

**Và ở đúng cái basis gần-hoàn-hảo đó, `level-reversion` đọc `PF_r` 1,1310 và
TRƯỢT cổng.**

⚠️ **Tôi tự sửa một con số của chính mình**: trước khi có dòng theo-bar này,
tôi đọc "trung vị ước lượng cuộn áp thực tế là +38,96 … +40,24" từ dòng
`APPLIED` **tính theo FRAME** — và con số đó **sai**, vì 63% frame là bản trùng
dấu thời gian đóng ở đúng cái instant cuối tháng 9 nơi basis thấp nhất. Dòng
theo-bar mới là dòng đúng, và nó nói điều **ngược lại**: ước lượng cuộn
**không** lệch thấp, nó **đúng**. Giữ lại cả hai con số và lý do, vì cái bẫy
"tỉ lệ tính theo frame" là khuyết điểm 18 ở dưới và nó bắt tôi một lần thật.

### Tự đánh: "có phải ước lượng CUỘN của tôi mới là cái tệ?" — KHÔNG, và giờ có hai cách bác

**Cách thứ nhất, trực tiếp:** ước lượng cuộn sai **+0,01 USD** ở trung bình
(W=120). Nó không thể là mắt yếu.

**Cách thứ hai, bằng chính bảng cổng.** Xếp sáu hằng số theo khoảng cách tới
basis đương thời đo được (p50 = **+41,72**, mean = **+42,17**):

    hang GAN basis thuc nhat          lech so voi p50    PF_r      cong
      +41,26                            -0,46            1,0057   TRUOT
      +42,16                            +0,44            0,8884   TRUOT
      +39,66                            -2,06            1,0704   TRUOT
    uoc luong CUON (sai <= 0,35 USD)
      W=1440                            +0,03 (p50)      1,0604   TRUOT
      W=480                             -0,09 (p50)      1,1021   TRUOT
      W=120                             +0,04 (p50)      1,1310   TRUOT
    hang XA basis thuc nhat, VE MOT PHIA
      +43,70                            +1,98            0,1735   TRUOT (thoai hoa)
      +45,48                            +3,76            1,4259   QUA
      +45,78                            +4,06            1,3883   QUA

1. **Một basis ĐÚNG trượt cổng.** Cả ba hằng số gần sự thật **và** cả ba ô
   cuộn — tức sáu cách đọc đúng — cho `PF_r` **0,8884 – 1,1310**, dưới vạch
   1,200 không mập mờ.
2. **Hai ô duy nhất qua cổng đòi sai +3,76 và +4,06 USD, VỀ CÙNG MỘT PHÍA.**
   Sai cùng cỡ về **phía kia** (+39,66, lệch −2,06) **không** qua. Một phán
   quyết chỉ tồn tại khi hằng số lệch cao **không phải một phát hiện về thị
   trường** — nó là một phát hiện về hằng số.
3. ⇒ **Phán quyết "qua cổng" của `options-first` ở +45,78 là hiện vật của một
   hằng số đặt cao hơn trung vị cửa sổ 4,06 USD.**

### Và đây là chỗ sâu nhất: `PF_r` nhạy ở thang 0,3 USD, trong khi basis có 2,3 USD phân tán thật

Ba ô cuộn đều **đúng** (lệch ≤ 0,35 USD so với sự thật), mà `PF_r` của chúng
vẫn đi **1,1310 → 1,1021 → 1,0604**. Đặt cạnh trung bình basis mỗi ô:

    mean basis +42,18  ->  PF_r 1,1310
    mean basis +42,25  ->  PF_r 1,1021
    mean basis +42,52  ->  PF_r 1,0604
    => 0,34 USD trong trung binh basis dich PF_r 0,0706
       va 0,0706 = 102% khoang cach con lai tu o tot nhat (1,1310) toi vach 1,200

⇒ **Đại lượng quyết định phán quyết đòi độ chính xác cỡ 0,3 USD.** Nhưng:

    phan tan THAT cua basis (sd)                      2,31 USD  = 7,6 lan
    be rong p10-p90 cua basis                         5,81 USD  = 19 lan
    dung sai vao lenh o entryAtr 0,35                 1,58 USD  = 5,2 lan
    giai toa underlying GIUA CAC THANG, cung phut p90 35,10 USD = 115 lan

Và `PF_r` **không phải hàm đơn điệu hay trơn** của basis: qua sáu hằng số nó đi
1,0704 → 1,0057 → 0,8884 → **0,1735** → 1,4259 → 1,3883, tức sụp ở +43,70 rồi
nhảy ở +45,48. **Một phán quyết đọc từ nó là một phán quyết đọc từ chỗ mà một
hàm bậc, nhạy ở 0,3 USD, tình cờ rơi vào trong một dải phân tán 2,3 USD.**

Đó là lý do thật, và nó **không** phải "ước lượng chưa đủ tốt": **không độ
chính xác nào là đủ**, vì đại lượng cần đo nhỏ hơn độ phân tán thật của chính
nó.

### Vì sao basis không sửa được bằng ước lượng tốt hơn: tape ở TRÊN BỐN THÁNG HỢP ĐỒNG

Đây là phần trả lời "vì sao", và nó đo được chứ không suy:

    26 symbol expiry trong 55.131 print
    underlying_price trung binh theo symbol: 4.146,00 (G2RV6) ... 4.385,32 (OG4U6)
      => 239 USD, vi la quyen chon tren GC U6 / V6 / X6 / Z6 (09,10,11,12-2026)
    do giai toa underlying_price TRONG CUNG MOT PHUT (8.316 phut co >1 print):
      mean +10,66  sd 15,48  p50 +1,00  p90 +35,10  max +58,10 USD
      khac 0 o 81,1% so phut;  > 5 USD o 29,5%

`level-reversion` và `flow-at-level` đọc **cluster**, và cluster **gộp** các
mức rút ra từ mọi context đó — **đọc ra từ mã, không suy**:

    engine.rs:308-329   derived = contexts.iter().flat_map(levels_from_context)
                        => moi muc cua MOI expiry do vao MOT danh sach
    engine.rs:332-333   cluster_levels(&derived, distance)
                        => rồi gom cum tren danh sach da tron do
    engine.rs:299-300   contexts.sort_by(dte)  => contexts.first() la expiry GAN NHAT,
                        tuc MOT thang hop dong, tuc MOT truc gia

Một con số basis — hằng hay cuộn — dịch cả cụm đi **cùng một lượng**, nên nó
**không thể** đưa các mức thuộc bốn tháng khác nhau về một trục. Dung sai vào lệnh ở `entryAtr = 0,35` là **1,58 USD**; độ giải toả
giữa các tháng ở p90 là **35,10 USD**, tức **22 lần dung sai**. ⇒ phép hiệu
chỉnh không phải **thiếu chính xác**, nó **đặt sai bài**.

**Và bằng chứng xác nhận nằm ngay trong bảng của chính phép đo này:**
`maxpain-magnet` không đọc cluster gộp — nó đọc `options.contexts.first()`
(`builtin.rs:529`), tức expiry gần nhất, tức **một** tháng hợp đồng, tức
**một** trục giá. Nó là cơ chế duy nhất trong bốn cái mà phán quyết **KHÔNG** phụ thuộc basis:

    maxpain-magnet, arm guards, qua CA CHIN cau hinh basis:
      PF_r 1,5580 - 1,8718   E +0,1875 - +0,2606   Lbar 0,2941 - 0,3500
      n 24 - 28   DD 2,44 - 3,37 USD = 2,25 - 3,15% cua dinh

`PF_r` của nó đi qua chín basis khác nhau mà **không hề cắt vạch 1,200 lần
nào** — trong khi `level-reversion` cắt vạch đó **ba lần** trong cùng chín
basis. **Cơ chế đọc MỘT context thì basis-ổn định; cơ chế đọc cluster GỘP thì
không.** Đó là cùng một chẩn đoán, đo từ hai phía độc lập.

⚠️ Nhưng `maxpain-magnet` chạy **24–28 lệnh**, dưới sàn **40** ở cả chín ô ⇒
**KHÔNG CÓ KẾT LUẬN**, **không phải** một ứng viên và **không phải** bị bác bỏ.
Nó chỉ là chỉ dấu về **hình dạng** của một phép đo đọc được, không phải một
phép đo.

### Sàn 40 lệnh: ba cơ chế vẫn KHÔNG CÓ KẾT LUẬN, và basis cuộn KHÔNG đưa cái nào lên

Khai trước ở F5 nên phải báo số:

| cơ chế | n qua 6 hằng số | n qua 3 `W` cuộn | sàn 40 |
|---|---|---|---|
| `level-reversion` | 94 – 108 | **104 – 110** | qua sàn, có phán quyết |
| `maxpain-magnet` | 24 – 28 | **27 – 28** | TRƯỢT SÀN cả 9 ô |
| `flow-at-level` | 20 – 30 | **20 – 27** | TRƯỢT SÀN cả 9 ô |
| `flow-momentum` | 4 | **4** | TRƯỢT SÀN cả 9 ô |

**Basis cuộn đổi số lệnh ít, và không đổi kết luận nào về sàn.** Ba cơ chế kia
vẫn **không có kết luận**: `27 of 36` ô (ba cơ chế × chín basis × một arm, và
tương tự ở arm kia) trượt sàn trên chân số lệnh một mình. `flow-at-level` ở
`W=120m` in `PF_r 2,1021` trên **20 lệnh** — đó là một con số, không phải một
phán quyết, và nó được in ra đây đúng để không ai đọc nó như một phán quyết.

### ĐỐI CHỨNG GIỮ: `flow-momentum` in GIỐNG HỆT ở cả chín basis

Nó không đọc một mức giá nào (chỉ `bull_ratio_15m` và
`net_flow_velocity_norm`), nên nó là đối chứng của chính phép dịch:

    arm guards   : 1 ban in duy nhat qua 9/9 cau hinh basis (3 cuon + 6 hang)
    arm noguards : 1 ban in duy nhat qua 9/9 cau hinh basis
      n 4   PF_r 0,5897   PF_usd 0,5404   E -0,3111 R   Lbar 0,7581 R
      stop thuc 1,516 ATR14(15m) = 12,82 diem   exits STOP 3 / TARGET 1
      DD 2,35 USD = 2,32% cua dinh   count match 12,25
    (so khop o: +39,66 +41,26 +42,16 +43,70 +45,48 +45,78 W=120m W=480m W=1440m)

⇒ **F4 KHÔNG bắn.** Phép dịch chạm giá và chỉ chạm giá — kể cả ở đường cuộn,
nơi 10–43 frame bị **từ chối** (làm rỗng `clusters` + `contexts`): việc từ chối
đó không chạm một trường dòng tiền nào, đúng như thiết kế khai trước.

### Vùng trùng thiếu: bao nhiêu bar bị TỪ CHỐI, đếm theo BAR

Mẫu số đúng là **bar**, không phải frame (xem khuyết điểm dưới):

    W=120m :  786 bar co frame,  743 giu LEVELS,  43 bar TU CHOI = 5,5%
    W=480m :  786 bar co frame,  776 giu LEVELS,  10 bar TU CHOI = 1,3%
    W=1440m:  786 bar co frame,  776 giu LEVELS,  10 bar TU CHOI = 1,3%

Cửa sổ ngắn từ chối nhiều hơn vì `K = 5` quan sát trên nến 15m đòi ≥ 75 phút
dữ liệu liền. **Từ chối được in là `null` và đếm riêng, không bao giờ in thành
một offset 0,00.** Và việc từ chối **không** phải thứ giải thích kết quả: ở
`W=480` và `W=1440` nó chỉ chạm 1,3% số bar mà `PF_r` vẫn lệch 0,0417.

### Cổng: chân nào ràng buộc, và hằng đẳng thức

    Lbar >= 0,250 R o 72/72 o  => chan expectancy DU o TAT CA, khong rang buoc lan nao
    (duoi day dem TREN MOT ARM: 36 o = 9 basis x 4 co che)
    chan 40 lenh  : 27 of 36 o  (maxpain-magnet, flow-at-level, flow-momentum)
    chan PF_r     :  9 of 36 o  (level-reversion), trong do 2 qua (ca hai la hang so cao nhat)

Hằng đẳng thức `E = Lbar × (PF_r − 1)` đúng **72/72 ô** (hai arm), phần dư lớn
nhất **0,00009 R**. (Mốc so sánh: `hour-screen` 32/32 dư ≤ 0,00005 R;
`pfr-audit` 452/452.)

### `PF_usd` so với `PF_r`, ĐO TRÊN BỘ NÀY — và nó là phép đo độc lập thứ BẢY

Phụ lục 9 §I nói chiều của khe **không phải hằng số của engine** và phải đo
trên bộ dữ liệu của mình, đừng trích tỉ lệ của ai. Đo trên 72 ô ở đây
(`khe = PF_r − PF_usd`):

    PF_r doc CAO hon PF_usd o 50/72 o = 69,4%

Đặt cạnh sáu phép đo trước, nó là **cái thứ bảy và nó KHÔNG đứng về phụ lục 8**:

    phu luc 8        PF_usd CAO hon o 320/452 o (71%)
    partial-exit     PF_usd THAP hon o  62/84 dong (73,8%)
    vprofile-gold    do ARM quyet dinh (guards off 4/36, guards ON 30/36)
    vprofile-btc     PF_usd CAO hon o  29/48 (60,4%)
    hour-screen      PF_r   CAO hon o  23/32 o (72%)
    companion-regime khe LAT DAU giua hai cua so tren CUNG MOT dong, 2/4 co che
    DAY (options)    PF_r   CAO hon o  50/72 o (69,4%)

⇒ **Bốn kết quả khác nhau trên bảy phép đo.** Chiều của khe là một thuộc tính
của **bộ dữ liệu**, không của engine. **In cả hai, mọi dòng.**

Bốn khe rộng nhất:

    +43,70 guards level-reversion   PF_usd 1,1790 (USD)  PF_r 0,1735 (R)  khe -1,0055
    +43,70 guards flow-at-level     PF_usd 0,6238 (USD)  PF_r 0,0400 (R)  khe -0,5838
    +39,66 guards flow-at-level     PF_usd 1,1275 (USD)  PF_r 1,3277 (R)  khe +0,2002
    +45,78 guards flow-at-level     PF_usd 2,1822 (USD)  PF_r 1,9849 (R)  khe -0,1973

**Và ĐÚNG HAI ô nằm hai phía vạch 1,200 của chính cổng — cả hai đều MỚI, không
có trong hồ sơ, và cả hai ở cùng một offset:**

    +39,66 guards level-reversion : PF_usd 1,2179 (USD) QUA chan PF  /  PF_r 1,0704 (R) TRUOT
    +39,66 guards flow-at-level   : PF_usd 1,1275 (USD) TRUOT        /  PF_r 1,3277 (R) QUA chan PF

Ô thứ nhất là ô đáng chú ý: **đọc bằng USD thì nó qua chân PF; đọc bằng R thì
nó trượt** — và nó được báo là **TRƯỢT**, vì `PF_r` là thuộc tính của phương
pháp còn `PF_usd` là thuộc tính của đòn bẩy; ở đây đòn bẩy là một sổ **100
USD** chạy ở `min_lot` với trần notional bịt gần như mọi lệnh (xem dưới).

⚠️ Và một chỗ tôi tự sửa: ô `+43,70 guards level-reversion` (`PF_usd` 1,1790 /
`PF_r` 0,1735) **KHÔNG** nằm hai phía vạch — **cả hai** đều dưới 1,200. Nó là
ô có khe rộng nhất, không phải một ô lật phán quyết. `options-first` gọi nó là
"OPPOSITE SIDES of the gate"; đếm lại thì 1,1790 < 1,200, nên **câu đó sai** và
số của tôi thắng theo §8.

### Trần notional cưỡi gần như MỌI lệnh của hai cơ chế cluster, và cả hai chân cổng mù với nó

`sized down` trên tổng số lệnh, arm guards:

    level-reversion   cuon 106/108, 108/110, 98/104  = 94,2 - 98,1% so lenh
                      hang 94/94 ... 107/108         = 95,0 - 100%
    flow-at-level     cuon 20/20, 23/23, 27/27       = 100% moi o
                      hang 20/20 ... 30/30           = 100% moi o
    flow-momentum     4/4 o MOI cau hinh             = 100%
    maxpain-magnet    cuon 1/27, 2/28, 2/28          = 3,7 - 7,1%
                      hang 1/24 ... 3/27             = 3,6 - 11,1%
    arm noguards: khong in dong `sized down` o o nao  => 0

`max_notional_pct_equity = 300%` trên sổ 100 USD. `r = points / risk` không đọc
`lots` ⇒ **cả hai chân cổng mù**, và `PF_usd` của mọi dòng là PF của một sổ
0,01 lot bị kẹp trần (khuyết điểm 17 + `cap_lots`). Nói ra theo phụ lục 7 §C.
Đây là lý do thứ ba để trích `PF_r`.

### Khuyết điểm 14 bắn lại, và guards là thứ đứng giữa hai cơ chế này và một sổ cháy

Arm **không-guards**, hằng +43,70 — y như `options-first` đo:

    level-reversion  DD 306,86 USD = 303,85% cua dinh  (von ve -305 USD tren so 100 USD)
    flow-at-level    DD 311,55 USD = 309,48% cua dinh

**Hai dòng đó đã CHÁY và `PF` của chúng không đọc được** (`PF_usd` 0,0442 và
0,0071). Cùng hai ô đó với `--guards` sụt **2,84** và **4,17 USD**. Guard làm
việc đó là `max_open_loss_r = 2.0`.

**Và basis cuộn KHÔNG sinh ra một ô cháy nào:** `_pct` lớn nhất qua 6 ô cuộn
không-guards là **12,89%** (`W=120m`, `level-reversion`, DD 14,16 USD), so với
**303,85%** của hằng +43,70. Vì ước lượng cuộn không bao giờ đặt một cạnh
cluster sát giá như +43,70 làm, nên `risk = entry − (cluster.low − 0,3×ATR)`
không về gần 0 và `Lbar` ở lại **0,61–0,65 R** thay vì nổ lên **3,50 R**.
**Đó là thứ duy nhất basis cuộn sửa được một cách rõ ràng** — nó xoá hiện
tượng thoái hoá đơn vị, nhưng nó **không** đưa cơ chế qua cổng.

### Luật riêng của cơ chế có nổ — F6 không bắn

`--exit-mix`, arm guards, **cả 36 ô**; ba ô cuộn và ô hằng cực trị:

    level-reversion  W=120   STOP 64  TARGET 41  TIMEOUT 1  WEEKEND_FLAT 1  END_OF_DATA 1   hold  30,8 min
    level-reversion  W=480   STOP 66  TARGET 41  TIMEOUT 2  WEEKEND_FLAT 1                  hold  30,4 min
    level-reversion  W=1440  STOP 63  TARGET 37  TIMEOUT 3  WEEKEND_FLAT 1                  hold  36,5 min
    level-reversion  +42,16  STOP 68  TARGET 35  TIMEOUT 1  WEEKEND_FLAT 1                  hold  30,6 min
    level-reversion  +45,78  STOP 51  TARGET 40  TIMEOUT 1  WEEKEND_FLAT 1  END_OF_DATA 1   hold  38,0 min
    maxpain-magnet   W=120   STOP  8  TARGET 13  TIMEOUT 4  NEWS_FLAT 1     END_OF_DATA 1   hold 120,6 min
    maxpain-magnet   W=480   STOP  9  TARGET 13  TIMEOUT 4  NEWS_FLAT 1     END_OF_DATA 1   hold 117,3 min
    maxpain-magnet   W=1440  STOP  9  TARGET 13  TIMEOUT 4  NEWS_FLAT 1     END_OF_DATA 1   hold 117,3 min
    flow-at-level    W=120   STOP  9  TARGET 11                                             hold  26,2 min
    flow-at-level    W=480   STOP 12  TARGET 10  TIMEOUT 1                                  hold  29,4 min
    flow-at-level    W=1440  STOP 17  TARGET  9  TIMEOUT 1                                  hold  22,2 min
    flow-momentum    moi W   STOP  3  TARGET  1                                             hold  67,5 min

Không dòng nào là hiện vật `tsmom/120d`: exit của guard là **0–2** trên mỗi
dòng trong cả 36 ô, còn `STOP`/`TARGET` của chính cơ chế đóng phần lớn. Với
hai cơ chế cluster thì `STOP` **chính là** tuyên bố của cơ chế, vì stop của
chúng **là** cạnh cluster. Và `WEEKEND_FLAT` nổ **0–1 lần** trên mỗi dòng
(cha: 395–478 mỗi 1.000 ở các hàng giữ dài) ⇒ hàng này **sống trong arm chủ
cho phép**, không phải một kết quả chỉ tồn tại ở arm bị cấm.

### Cỡ stop thực và cost/R — bốn thứ phải nói, không phải một

Từ dòng receipt tự in, arm guards, tách **cuộn** và **hằng** để thấy phép dịch
có đổi cỡ stop thực hay không:

    co che           CUON (3 o W)                             HANG (6 offset)
    level-reversion  0,544 - 0,571 ATR = 5,24 - 5,30 diem     0,542 - 0,609 ATR = 5,27 - 5,63 diem
                     cost/R 5,29 - 5,35%                      cost/R 4,97 - 5,31%
    flow-at-level    0,473 - 0,524 ATR = 4,90 - 5,28 diem     0,459 - 0,594 ATR = 4,64 - 5,53 diem
                     cost/R 5,31 - 5,71%                      cost/R 5,06 - 6,04%
    maxpain-magnet   2,017 ATR = 17,24 - 17,30 diem           2,014 - 2,017 ATR = 17,17 - 18,34 diem
                     cost/R 1,63 - 1,64%                      cost/R 1,54 - 1,64%
    flow-momentum    1,516 ATR = 12,82 diem, cost/R 2,21%     Y HET, moi cau hinh

⇒ **Basis cuộn không đổi cỡ stop thực một cách đáng kể**: dải của nó nằm trong
dải của sáu hằng số ở cả bốn cơ chế. Nên nó **không** mua được gì ở phía chi
phí; cái nó đổi là **chỗ đặt cạnh cluster so với giá**, không phải độ rộng.

Bốn thứ phải nói, theo phụ lục 9 §II:

* **cửa sổ** = 2026-09-06 → 2026-09-17, **196,2 giờ**, **11 ngày UTC**, và nó
  là **MỘT** cửa sổ.
* **thống kê** = **trung vị thực hiện được** của chính dòng đó, từ dòng
  `the method's own realised stop` mà receipt tự in.
* **cỡ stop** = cột trên, và nó **KHÔNG phải 1,5 ATR**: receipt tự viết *"the
  method DECLARES stopAtr 1.50 and does not use it"* ở hai cơ chế cluster, vì
  stop của chúng là cạnh cluster. Tiền kiểm cha chọn arm này trên **cost/R
  2,43% ở 1,5 × ATR14(15m)**; cơ chế chạy ở **4,97 – 6,04%**, tức **gấp hơn
  hai lần** con số dùng để chọn arm.
* **spread** = **0,28 GIẢ ĐỊNH theo config**, một lần đọc terminal. Logger đã
  lấy mẫu p50 **0,220**, max **0,260** ⇒ mọi `cost/R` ở đây **tính cao hơn bất
  cứ thứ gì từng quan sát được**, nên nó là **trần**, không phải ước lượng.

### Phân bố giờ NY của 786 bar trùng — basis KHÔNG đứng trên nến mốc nghỉ

`agent/hour-screen` đo rằng nến mở lại sau mốc 17:00 NY mang một dấu in của
nhà cung cấp. Vì tape chỉ phủ 0,8% bộ nến, **chỗ nào trong ngày** vùng trùng
rơi vào là quyết định. Đo được:

    785 bar ghep duoc, chia GAN DEU cho 24 gio NY: 3,6% - 4,6% moi gio
    17:00 NY : null bar — khong co nen trung nao trong gio do (khong phai 0)
    18:00-18:15 NY (nen MO LAI): 8 bar = 1,0% cua vung trung
      open == low tren 0 / 8 = 0,00%
      nen so sanh: 746 nen mo lai trong ca 4 nam co open == low o 6,30%
                   toan bo 100.586 nen XAUUSD-15m co open == low o 2,81%
    basis trung binh theo gio: +41,88 ... +42,77  (sd 1,91 - 2,75)

⇒ **Dấu in đó không chạm mẫu này**: vùng trùng chỉ có 8 nến mở lại và không
nến nào trong số đó có `open == low`. Và basis **không có cấu trúc trong
ngày** — nó đi ngang quanh +42 ở cả 24 giờ, nên drift của nó là **theo ngày**,
không theo giờ. Basis cuộn vì vậy đứng trên vùng sạch; nếu nó sai thì không
phải vì mốc nghỉ.

### Hai khuyết điểm dụng cụ, ĐẾM không sửa

**Khuyết điểm mới (18): `OptionsTimeline` mang 6.010 / 9.491 frame TRÙNG DẤU
THỜI GIAN = 63%.** `build_timeline` bước một đồng hồ lấy mẫu 300.000 ms nhưng
đóng dấu mỗi frame bằng `snapshot.as_of`, tức **giờ của print CUỐI**, nên qua
khe tape 17 ngày (2026-09-17 → 10-04) mỗi bước lại sinh thêm một frame mang
đúng dấu thời gian cũ. Các frame giống nhau nên **không lookup nào sai** —
nhưng **mọi tỉ lệ tính theo FRAME là vô nghĩa**: `shifted / frames` =
**8.034 / 9.491 = 84,6%** trong khi tỉ lệ thật tính theo **bar** là
**776 / 786 = 98,7%**. Chỉ có **3.059 / 9.491** frame
nằm trong khoảng thời gian bộ nến phủ. ⇒ `Report` của tôi đếm theo **bar** và
in cả số frame trùng; receipt nào trên trục này đọc một tỉ lệ theo frame là
đọc sai.

**Khuyết điểm (19), cùng họ: `Frame::spot` không phải một chuỗi giá.** Đã viết
ở ghi chú tiền kiểm phía trên. Nó không phải một khuyết điểm của kết quả nào đã
công bố (không strategy nào đọc `spot`), nhưng nó là một cái bẫy đã bắt tôi một
lần và sẽ bắt người sau: trường tên là `spot` mà nội dung là *underlying của
print cuối, thuộc bất kỳ tháng nào trong bốn tháng*.

### Sổ đa phép thử: khai 48, XEM 72 — và chỗ vượt nằm ở ĐỐI CHỨNG

Khai trong đăng ký này:

    24 o CUON    = 3 W x 4 co che x 2 arm          <- o cong MOI
    24 o HANG    = 3 offset x 4 co che x 2 arm     <- doi chung, chay lai

Xem thật:

    24 o CUON    dung nhu khai
    48 o HANG    = SAU offset x 4 co che x 2 arm
                   ba cua TOI (+39,66 / +42,16 / +45,48, phan vi do tren dung
                   cua so) va ba cua CHA (+41,26 / +43,70 / +45,78)
    => tong xem 72, khai 48

**Chỗ vượt là 24 ô và nó nằm TRỌN trong nhóm đối chứng, không một ô cổng mới
nào.** Lý do tôi chạy cả sáu thay vì ba: brief đòi "ba offset hằng phải chạy
lại cùng lúc" (tức ba của cha), còn tiền kiểm của tôi cho thấy ba con số đó
**không phải** phân vị của cửa sổ chúng được áp lên — nên bỏ một trong hai
triple thì hoặc không đọc được basis cuộn nằm đâu trong dải cha, hoặc không
đọc được nó nằm đâu trong dải thật. **Tôi khai chỗ vượt ở đây thay vì làm
tròn nó**, và không ô nào trong 48 ô hằng mang một phán quyết: chúng là thước
đo độ nhạy. 24 ô cổng mới là 24 ô cổng mới.

### Phân vị không công bố, và đây là lý do bằng số của chính trục này

    count match, phuong phap so voi null khop cua no:
      level-reversion  0,98 - 1,00   trong bang (cuon: 0,99 / 0,99 / 1,00)
      flow-at-level    1,63 - 2,45   NGOAI
      maxpain-magnet   1,75 - 2,04   NGOAI
      flow-momentum    12,25         NGOAI
    cost match 1,77 - 22,90 ngoai bang o MOI dong
    null p50 duoi 1,000 o moi dong (0,70 - 0,91)

Ba trong bốn cơ chế được đọc với một đối chứng **không cùng cỡ**. Và `null p50
< 1` nghĩa là một phân vị cao ở đây đọc là "lỗ ít hơn vào lệnh ngẫu nhiên",
không phải "có lãi". **Cổng được báo một mình.**

### Phán quyết, bằng đúng những chữ đăng ký này cố định trước

* **F1 BẮN** — nhưng **cơ chế của nó khác thứ F1 hình dung, và tôi nói ra chỗ
  đó.** Theo đúng chữ: basis cuộn cho phán quyết nằm **giữa** dải sáu hằng số
  (`PF_r` 1,0604–1,1310 trong dải 0,1735–1,4259) và **vẫn đổi theo tham số
  cuộn** (Δ 0,0706 `PF_r` qua `W` 120→1440). Cả hai điều kiện thoả.

  **Chỗ F1 nói chưa đúng:** nó giả định lý do là "basis không ước lượng được".
  Đo được thì **ngược lại — basis ước lượng được rất tốt**: trung vị cuộn 2 giờ
  nhân quả dựng lại basis đương thời với sai số trung bình **+0,01 USD** trên
  **2,31 USD** phân tán, và tái lập cả drift. Lý do thật là:
  **`PF_r` nhạy ở thang 0,3 USD trong khi đại lượng nó đọc có 2,31 USD phân tán
  thật và nằm trong một tape có 35,10 USD giải toả giữa các tháng hợp đồng.**
  Tức **không độ chính xác nào là đủ** — không phải **chưa** đủ.

  **Kết luận mà F1 rút ra thì KHÔNG đổi, và giờ được chống đỡ tốt hơn:** tape
  GC và nến XAUUSD không ghép đủ chặt để đo họ cơ chế này, vì cluster **gộp**
  bốn tháng hợp đồng (`engine.rs:308-333`). Desk cần **một nguồn options trên
  chính XAUUSD**, hoặc một tape lọc về **một** tháng với context riêng — không
  phải một phép hiệu chỉnh.
* **F2 không bắn**: phán quyết **không** ổn định qua ba `W`.
* **F3 không bắn**: test nhân quả xanh, và hai probe nhìn trước **trượt** nó
  (4/5 và 5/5 mốc).
* **F4 không bắn**: `flow-momentum` in **một** bản duy nhất qua **9/9** cấu
  hình basis, ở **cả hai** arm.
* **F5 áp dụng**: basis cuộn đổi số lệnh ít (`level-reversion` 104–110 so với
  94–108) và **không đưa cơ chế nào từ dưới 40 lên trên 40**. Ba cơ chế kia vẫn
  **KHÔNG CÓ KẾT LUẬN**, **không bị bác bỏ**.
* **F6 không bắn**: luật riêng của cơ chế nổ ở mọi dòng, guard exits 0–2.

**Câu trả lời một dòng, trung thực: basis cuộn dựng được, nhân quả được khẳng
định bằng test, và nó ĐÚNG — sai +0,01 USD ở trung bình. Ở đúng basis đúng đó,
cơ chế TRƯỢT cổng; và cả ba hằng số gần sự thật cũng trượt. Hai ô duy nhất qua
cổng trong 72 ô đòi một hằng số cao hơn trung vị của chính cửa sổ +3,76 và
+4,06 USD, VỀ CÙNG MỘT PHÍA — nên "qua cổng" của `options-first` là hiện vật
của hằng số, không của thị trường. Và cái không sửa được không phải ƯỚC LƯỢNG
mà là BÀI TOÁN: `PF_r` quyết định ở thang 0,3 USD trong khi basis có 2,31 USD
phân tán thật và tape trộn bốn tháng hợp đồng với 35,10 USD giải toả cùng-phút.
Hướng "hiệu chỉnh basis" ĐÓNG.**

**Không có gì ở đây là một candidate.** Một cửa sổ, **196,2 giờ** trùng, **11
ngày UTC**, **0,8%** bộ nến — và cổng desk đòi **hai** cửa sổ.

### Thứ sẽ mở lại việc này, theo thứ tự, và mục 1 không còn là basis

1. **Một nguồn options trên chính XAUUSD**, hoặc một tape GC **đã lọc về MỘT
   tháng hợp đồng** kèm context riêng cho từng tháng thay vì cluster gộp. Đây
   là thứ F1 chỉ ra và nó thay chỗ mà "basis cuộn" từng đứng.
2. **Một cơ chế đọc MỘT context** (`maxpain-magnet` là cái duy nhất đang có)
   **đạt được 40 lệnh.** Nó basis-ổn định qua chín cấu hình — nó là hình dạng
   duy nhất trên trục này mà một phép đo đọc được, và nó thiếu **12–16 lệnh**.
3. **`py/ingest/mt5_export.py` cho XAUUSD, chạy thật.** Nến đóng băng ở
   2026-09-17 còn tape chạy tới 10-09, nên vùng trùng kẹt ở 196,2 giờ và cửa
   sổ thứ hai **không thể tới**, collector chạy bao lâu cũng vậy. Và con số
   này **đã xấu đi**: `options-first` ghi 28% tape không có nến giao dịch được;
   đo lại hôm nay trên 55.131 print thì **21.803 print = 39,5%** nằm sau giờ
   đóng của nến cuối cùng. Mỗi giờ tape bank thêm là một giờ **không đo được**
   cho tới khi ai đó xuất nến XAUUSD khớp.
4. Chưa đáng làm trước (1): quy nguyên theo từng loại mức, mọi phép quét tham
   số, và ba cơ chế không đạt 40 lệnh.

### Đĩa và hạ tầng

`df -h /e`: **16 GB** rảnh (95%) trước → **14 GB** (96%) ngay sau build+run →
**15 GB** (96%) sau khi mọi `search.exe` thoát. Tức con số **dao động 1 GB
trong vài phút mà không ai xoá gì** — đúng cái hiện tượng phụ lục 8 §VI nói:
thủ phạm là `pagefile.sys` tự nới/co theo áp lực bộ nhớ, **không phải
`target/`**. `target-ob/` của worktree này tốn ~2 GB (build release một crate).
**Không xoá `target/` của ai**, và 15 GB còn xa sàn 3 GB. Không chạy
`cargo test --workspace` ở debug lần nào.

Bộ test: **205 test `--release` trên `-p fd-backtest`, 0 lỗi**, trong đó 118
test `--lib` (10 cái mới của `basis.rs`) và cổng golden parity
(`parity.rs`, `timeline_parity.rs`, `paper_parity.rs`) **không đổi**.

Hai `collect.exe` **pid 5044 (gold) và 38720 (btc) còn sống sau khi job xong**,
đã kiểm bằng `tasklist`. Không `taskkill`, không chạm
`/e/rust/flowdesk/target/release/`, `data/gold/`, `data/btc/`. Hai `collect.exe` (pid 5044 gold, 38720 btc) **không bị
chạm**; tape đi 53.479 → **55.131** print giữa `options-first` và run này, tức
chúng vẫn đang ghi.
