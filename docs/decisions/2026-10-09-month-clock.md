# Đăng ký trước — đồng hồ THÁNG: ngày trong tháng có đo được gì trên vàng 15m? (`agent/month-clock`)

**Ngày:** 2026-10-09. Nhánh `agent/month-clock`, cắt từ `agent/stop-width`
(`29c8635`, nhánh có bản vá `lbar_line` in `Lbar` + `PF_r` cạnh `PF_usd`).
Binary nếu phải tiêu ô cổng: `/e/rust/fd-stop-width/target-sw/release/search.exe`
(phụ lục 7 mục A, phụ lục 8 mục VI).

Commit này **chỉ có file này**, trước dòng code đầu tiên (brief §5).

## 1. Giả thuyết, một câu

`crates/fd-strategy/src/filter.rs` có `Hours`, `Sessions`, `Weekdays`, `Flat`,
`VolRegime`, `VolAbs`, `News` — **không một gate nào theo NGÀY TRONG THÁNG** —
nên nếu dòng tiền tái cân bằng cuối tháng để lại dấu trên vàng, **đồng hồ tháng
là một trục chưa từng được hỏi**, và prior đó phải trả lời được bằng biên độ /
dịch chuyển có dấu của nến 15m đo theo ngày trong tháng trên 16 năm `xauduka`.

**Prior, không phải bằng chứng.** Tái cân bằng cuối tháng là hiện tượng có tài
liệu ở thị trường tài chính và vàng nằm trong rổ nhiều quỹ. Desk có 16 năm dữ
liệu; đăng ký này tồn tại để **prior không được tự biến thành kết luận**.

## 2. TIỀN KIỂM — chạy TRƯỚC khi viết filter, trước khi tiêu một ô cổng nào

Tiền lệ: trục AUDNZD khai 432 ô, F2 bắn ở tiền kiểm, tiêu **0** (brief §5).

### 2.1 Dữ liệu và hai cửa sổ

`data/bars/XAUDUKA-15m.parquet` — đo trực tiếp, không qua engine. Span thật
(đọc ra, không trích hồ sơ): **2010-06-01 00:00 → 2026-05-31 23:45 UTC,
378.749 nến, đúng 192 tháng.**

    cua so W1: 2010-06-01 -> 2018-06-01   (96 thang)
    cua so W2: 2018-06-01 -> 2026-06-01   (96 thang)

Đúng chỗ cắt mà phụ lục 5 mục B dùng để đo drift (và drift **lật dấu** ở chỗ
cắt đó: −0,0040R → +0,0205R mỗi phiên) ⇒ cửa sổ này đã được chứng minh là chỗ
hồ sơ lật, nên nó là phép thử hai-cửa-sổ khó nhất có sẵn, không phải dễ nhất.

⚠️ `volume` của feed này là **0 ở 0/50.000 mẫu** (phụ lục 8 mục I) ⇒ tiền kiểm
này **không đọc volume**, chỉ `open/high/low/close/time`.

### 2.2 Hai thước — KHAI CẢ HAI, đo cả hai, báo cả hai

- **Thước A — ngày dương lịch trong tháng** `dom ∈ 1..31`, lấy từ **ngày New
  York** của nến (`fd_core::clock::new_york_local`, có luật DST từ 2007).
- **Thước B — ngày giao dịch thứ N tính TỪ CUỐI tháng** `tdfe ∈ 1..23`
  (`tdfe = 1` là ngày giao dịch NY cuối cùng của tháng). Đây là thước mà prior
  tái cân bằng thật sự nói về: quỹ chốt theo **phiên cuối tháng**, không theo
  "ngày 31".

Hai thước **khác nhau thật**: ngày 31 không tồn tại ở 5 tháng, và `dom = 30`
có thể là phiên cuối tháng hoặc phiên thứ ba từ cuối tuỳ tháng rơi vào thứ mấy.

### 2.3 Hai đại lượng, chuẩn hoá theo biên độ ngày

Đơn vị quan sát là **một ngày giao dịch New York**. Mỗi ngày d cho đúng hai số,
tổng hợp từ nến 15m trong ngày NY đó:

    amp(d)  = mean( high - low ) tren moi nen 15m cua ngay d      [diem gia]
    disp(d) = ( close cuoi ngay ) - ( open dau ngay )             [diem gia, CO DAU]

rồi **chuẩn hoá** bằng `base(d)` = biên độ ngày NY trung bình của **20 ngày
giao dịch đã hoàn tất TRƯỚC ngày d**:

    Amp(d)  = amp(d)  / base(d)      Disp(d) = disp(d) / base(d)

`base(d)` cài lại **đúng** luật `average_day_range` của engine
(`crates/fd-strategy/src/tsmom.rs:137`): một ngày dương lịch có span bars **nhỏ
hơn nửa span dài nhất trong cửa sổ** thì **không phải một ngày** (phiên tối Chủ
Nhật 6 giờ trên feed 24h từng kéo trung bình xuống 6–9%, data-integrity
`2026-09-13-close-reopen-drift.md`).

**Vì sao chuẩn hoá là điều kiện bắt buộc:** không có nó, một mốc đọc ra "động
hơn" chỉ vì vài tháng động hơn rơi vào mốc đó. `base(d)` là **ex-ante** (chỉ
ngày đã hoàn tất trước d) nên nó không nhìn lén ngày đang đo.

### 2.4 Không chồng lấn, và CỠ MẪU THẬT — nhỏ, nói ra trước

Mỗi ngày giao dịch vào **đúng một** quan sát, mỗi tháng cho **đúng MỘT** quan
sát cho một mốc ⇒

    16 nam = 192 thang  =>  TRAN TREN la 192 quan sat moi moc
    moi cua so: 96 thang  =>  TRAN TREN la 96 quan sat moi moc moi cua so

Thước A còn mất các lần mốc rơi vào Thứ Bảy / Chủ Nhật / ngày lễ ⇒ thực tế
`dom` kỳ vọng ~**96 × 5/7 ≈ 69** mỗi cửa sổ, và `dom ∈ {29,30,31}` **ít hơn
nữa**. Thước B không mất (mỗi tháng luôn có phiên cuối) ⇒ ~96.

**Đó là cỡ mẫu thật và nó NHỎ.** Với `Disp` lệch chuẩn ~0,8 biên độ ngày, n=69
cho sai số chuẩn ~0,096 ⇒ **một hiệu ứng nhỏ hơn ~0,19 biên độ ngày không phân
biệt được với 0 ở một cửa sổ.** Số đo sẽ nói con số thật.

`t` chạy trên mẫu **không chồng lấn** (Welch hai mẫu: ngày-mốc vs ngày-không-mốc
**trong cùng cửa sổ**), không có nến nào vào hai quan sát, không có cửa sổ
trượt.

### 2.5 ĐA PHÉP THỬ — đếm TRƯỚC, đây là vấn đề lớn nhất của trục này

    2 dai luong (Amp, Disp) x 2 thuoc (A: dom 1..31, B: tdfe 1..23)
      x 2 cua so (W1, W2)
    = 2 x (31 + 23) x 2 = 216 phep so

Brief của job khai **124** (= 31 × 2 thước × 2 cửa sổ, tức **một** đại lượng).
Đếm của tôi lớn hơn vì tôi đo **hai** đại lượng và thước B có 23 mốc, không 31.
Theo luật §8 (số đo thắng con số trong brief) tôi **khai 216 và báo 216**, chứ
không khai 124 rồi chạy 216.

**Ở mức đó, kỳ vọng dương-giả thuần may rủi:**

    216 x 0,05 = 10,8 moc dat p < 0,05 o MOT cua so  <- gan gap doi "~6" brief noi,
                                                        vi brief dem 124 phep so
    sang loc "DONG DAU tren CA HAI cua so" MOT MINH:
      P(dong dau) = 0,5  =>  ~54 / 108 to hop qua  => VO DUNG mot minh

⇒ **Sàng lọc đăng ký là SIGN-AGREEMENT CỘNG p < 0,05 Ở CẢ HAI CỬA SỔ:**

    P(p<0,05 hai phia o ca hai cua so VA cung dau) = 2 x 0,025^2 = 0,00125
    108 to hop (2 dai luong x 54 moc) x 0,00125 = 0,135 moc duong-gia ky vong

Đó là con số làm cho một mốc qua sàng **có nghĩa**. Khai trước: **chỉ mốc qua
sàng này được mang sang bước đo cơ chế.** Không mốc nào qua ⇒ **tiêu 0 ô cổng**,
báo, xong.

Báo kèm Benjamini–Hochberg trên 216 p-value **để đọc**, không phải làm cổng.

### 2.6 SÀN CỠ MẪU của bước đo — khai TRƯỚC, vì đây là chỗ trục này dễ chết nhất

Một filter theo ngày-trong-tháng **cắt cỡ mẫu xuống ~1/30** (thước A) hoặc
~1/21 (thước B). Hồ sơ đã đo đúng chuyện đó hai lần: trục chế độ biến động chia
mẫu làm ba và **53,7% ô rơi xuống dưới 40 lệnh**; và con số cảnh báo chuẩn là
cùng một luật cho `PF 1,753 / 14 lệnh` và `PF 0,682 / 178 lệnh` (brief §4).

    Cong desk: n >= 40 lenh TREN CA HAI cua so
    => mot co che phai co >= 40 x 30 = ~1.200 lenh moi cua so o ban KHONG filter
       de con >= 40 lenh duoi mot filter MOT ngay.

⇒ **Khai trước ba điều:**

1. Trước khi tiêu ô nào, **đếm số lệnh bản không-filter** của mọi cơ chế ứng
   viên trên cả hai cửa sổ. Cơ chế nào dưới ~1.200 lệnh/cửa sổ thì **filter một
   ngày KHÔNG ĐO ĐƯỢC trên nó** — và điều đó in ra là **"không đo được"**, không
   bao giờ in thành 0 và không bao giờ đọc thành "không có edge" (brief §8).
2. Vì thế filter đăng ký là một **BĂNG ngày**, không phải một ngày: bề rộng băng
   chọn **từ số lệnh đo được ở điểm 1**, nhỏ nhất đủ để `n ≥ 40` ở **cả hai**
   cửa sổ. Băng rộng ra là **đánh đổi đã khai**, không phải điều chỉnh sau khi
   thấy số.
3. Ô nào ra 30–39 lệnh sẽ được tool in là **qua** chân số lệnh (`need 30`) trong
   khi **trượt cổng desk 40** (brief §4) ⇒ **đếm `n` bằng tay ở mọi ô.**

## 3. Nếu tiền kiểm cho mốc qua sàng: biến thể filter và sổ ô cổng

Chỉ khi 2.5 cho ít nhất một mốc qua sàng. Khi đó:

**Filter mới, nhỏ, CÙNG KHUÔN `Weekdays`** (một gate vào lệnh, không chạm tín
hiệu, không chạm exit) — một biến thể duy nhất, chọn theo thước mà tiền kiểm
chỉ ra:

    `monthdays:<a>-<b>`  — vao lenh chi khi ngay NY trong thang thuoc [a,b]
    `monthend:<k>`       — vao lenh chi khi ngay giao dich thu <= k tu CUOI thang

Cài trong `Filter`, nhánh `Filter::Weekdays` làm mẫu
(`filter.rs:32 / 130 / 232 / 371`), cộng test parse + test gate như `weekdays`.

**Sổ ô cổng khai trước (chỉ tiêu nếu tiền kiểm qua):**

    co che da co trong registry (chon tu diem 2.6.1, chi co che du ~1.200 lenh)
      x 1 bien the filter (bang do tien kiem chi ra)  x 2 cua so  x 2 arm
    => khai TRAN 3 co che x 1 filter x 2 cua so x 2 arm = 12 o

Không trục nào khác. Không thêm băng, không thêm cơ chế sau khi thấy số: sửa
đăng ký = **thêm ghi chú có ngày vào CUỐI file này**.

Mọi ô báo: `n · PF_usd · PF_r · E (= total_r/n) · Lbar · max_drawdown_usd ·
_pct · exit-mix · cost/R kèm cỡ stop`.

## 4. Falsifier — cụ thể và bắn được

- **F1 (TIỀN KIỂM, cái quan trọng nhất).** Bắn khi **không mốc ngày-trong-tháng
  nào** (cả 54 mốc, cả hai đại lượng) đạt **cùng dấu VÀ p < 0,05 ở CẢ HAI cửa
  sổ**, sau khi kể đến **216 phép so**. F1 bắn ⇒ **chu kỳ tháng không chứa gì
  đo được trên vàng ở khung 15m**, **tiêu 0 ô cổng**, và desk **đóng được một
  đồng hồ mà trực giác ai cũng tin là có**. Đó là kết luận sạch, không phải một
  thất bại.
- **F2 (hiện vật cửa sổ thứ 12).** Một mốc chỉ sống ở một nửa — p < 0,05 ở W1 và
  không ở W2, hoặc **lật dấu** — là **hiện vật cửa sổ thứ 12**, không phải cơ
  chế (11 hiện vật đã biết: phụ lục 7 mục D). Bắn khi số mốc "chỉ một nửa" ≈ số
  kỳ vọng may rủi (~10,8 ở một cửa sổ) ⇒ báo đúng thế và **không mang sang bước
  đo**.
- **F3 (sàn cỡ mẫu).** Bắn khi mọi cơ chế ứng viên dưới ~1.200 lệnh/cửa sổ ở bản
  không-filter ⇒ **filter ngày-trong-tháng không đo được**, và viết đúng chữ đó,
  **không** viết "không có edge".
- **F4 (luật có nổ không).** `--exit-mix` bắt buộc ở mọi ô (brief §6a:
  `tsmom/120d` in `SURVIVES` với PF 2,236 trong khi luật của nó nổ **0 lần**).
  Bắn khi exit-mix cho thấy filter mới **không đổi tập lệnh** so với bản
  không-filter trên cùng cửa sổ ⇒ gate không nổ, ô không đọc được.
- **F5 (arm giao dịch được).** Kết quả chỉ sống ở arm không-guards ⇒ **nói thẳng
  là không giao dịch được** (phụ lục 5 mục D: chủ chốt không giữ qua tuần).
- **F6 (cháy).** `max_drawdown_pct > 100%` ⇒ dòng đã cháy (khuyết điểm 14),
  **không đọc PF của nó**.

## 5. Hai thứ KHAI TRƯỚC rằng job này sẽ KHÔNG đề xuất

1. **Không đề xuất hình dạng "tín hiệu chậm thể hiện bằng nhiều lệnh nhỏ".** Cổng
   tính **theo lệnh** nên hình dạng đó bị cấm bằng số học, không bằng một phép
   đo thất bại: `+0,050R × 19,9 lượt = +0,994R mỗi chân trời` so với toàn bộ
   gross `+0,3775R` (phụ lục 6 mục II, phụ lục 8 mục V). Một đồng hồ tháng rất
   dễ rơi vào đúng hình dạng này — nên khai trước rằng nó **không phải đường ra**.
2. **Không trích một con số đã công bố như một dữ kiện.** Chỉ 295/452 ô chạy lại
   khớp `PF_usd` đã công bố (65,3%); 51 dòng đã công bố không còn ô nào mang
   đúng số lệnh đã in (phụ lục 8 mục II). Mọi con số trong báo cáo job này là số
   **job này chạy ra**.

## 6. Cách đọc

- Tiền kiểm: hai bảng (Amp, Disp) × hai thước, mỗi dòng một mốc, mỗi cửa sổ một
  cặp `(hiệu số theo biên độ ngày, p)` + `n` thật. Dẫn **hiệu số có đơn vị**
  (biên độ ngày NY 20 ngày), không dẫn p một mình.
- Cổng (chỉ nếu có): `PF ≥ 1,200` VÀ `E ≥ +0,050R` VÀ `n ≥ 40` trên **cả hai**
  cửa sổ. Khai **chân nào ràng buộc** (phụ lục 5 mục A), và nhớ hằng đẳng thức
  `E = Lbar × (PF_r − 1)` ⇒ chân expectancy dư **khi và chỉ khi `Lbar ≥ 0,250R`**
  (phụ lục 6 mục I).
- `E` lấy từ **`total_r/n`**, không từ `expectancy` in 3 chữ số (phụ lục 8 mục IV
  — làm tròn lớn hơn cả `gross` và đủ để lật dấu).
- **Phân vị KHÔNG công bố** trừ khi `count match` trong băng; `null p50 = 0,000`
  nghĩa là **không calibrate được** (brief §4).
