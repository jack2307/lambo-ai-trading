# Đăng ký trước — hình trong ruột nến 15m, dựng từ 1m, làm NGUỒN TÍN HIỆU (`agent/intrabar-shape`)

**Ngày:** 2026-10-10. Nhánh `agent/intrabar-shape`, cắt từ `agent/stop-width`
(`29c8635`). Commit này **chỉ có file này**, trước dòng code đầu tiên (brief §5).

Đọc trước: `AGENT-BRIEF-2026-10-07-AUDIT.md` + phụ lục 5, 6, 7, 8, 9.

## 1. Giả thuyết, một câu

Trong một nến 15m, **giá tiêu thời gian ở đâu** là thông tin mà bộ O/H/L/C của
nến 15m **ném đi**; nếu hình thời-gian-tại-giá đó dựng từ 15 nến 1m **phân biệt
được kết cuộc của nến 15m TIẾP THEO** hơn chính O/H/L/C của nến đó, thì độ phân
giải mịn hơn chứa thông tin dùng được ở chân trời 15m — và ngược lại thì desk
thôi hỏi về dữ liệu tick/1m cho lớp cơ chế này.

## 2. Vì sao trục này sống sót sau khi volume profile bị đóng

Phụ lục 9 §III đóng họ volume. Trục này **không đọc cột volume**, và tôi đã đo
lại trước khi đăng ký (không trích số của phụ lục):

    XAUDUKA-1m.parquet   5.635.777 dong   volume null_count = 5.635.777  (100%)
    XAUDUKA-15m.parquet    378.749 dong   volume null_count = 0,
                                          gia tri duy nhat = {0.0}   <- CHE RA

⇒ Nhà cung cấp **không công bố gì**; bước `null → 0` của đường ống **chế ra một
số không** ở 15m. Khớp phụ lục 9 §III. **Job này không đọc cột volume ở bất kỳ
đâu, với bất kỳ lý do gì.** Mọi đặc trưng dựng **thuần từ O/H/L/C của nến 1m**,
mỗi phút **một trọng số bằng nhau** — tức nó **chính là** một time profile, và
đó là thứ duy nhất dữ liệu này nói thật.

## 3. Phủ dữ liệu — ĐÃ ĐO TRƯỚC KHI ĐĂNG KÝ

    1m   : 5.635.777 nen, 2010-06-01T00:00 -> 2026-05-31T23:59 (UTC)
    15m  :   378.749 nen, 2010-06-01T00:00 -> 2026-05-31T23:45 (UTC)
    15m store ghi `resampled_from: XAUDUKA-1m.parquet` => cung mot nguon

    so nen 1m trong moi nen 15m:
      15/15 : 360.111 nen 15m  (95,0791%)
      14    :  11.000          ( 2,904%)
      13    :   3.046          ( 0,804%)
      12    :   1.332          ( 0,352%)
      11    :     673          ( 0,178%)
      <=10  :   2.587          ( 0,683%)
      ------------------------------------
      < 15  :  18.638 nen 15m  ( 4,9209%)   <- DAC TRUNG = NULL, KHONG PHAI 0
      o 15m ma 1m khong co bucket nao:  0

⇒ **Luật khai báo: nến 15m không đủ 15 nến 1m ⇒ mọi đặc trưng 1m của nó là
`null`, dòng đó bị LOẠI khỏi cả hai mô hình (phép thử và đối chứng) — không điền,
không về 0.** Đối chứng chạy trên **đúng cùng tập dòng** đó, nếu không thì phép
so là phép so hai mẫu khác nhau.

## 4. ⚠️ Cái phải sợ nhất: nhìn trước. Và cách nó bị bắt.

Engine: tín hiệu ở nến `i` **khớp ở OPEN của nến `i+1`**. Nên hình đọc ở nến `i`
chỉ được gồm 15 phút **của chính nến `i`** — nếu nó chạm phút thứ 16 (phút đầu
của nến `i+1`) thì mọi con số là rác.

**Tính chất bắt chước từ `fd-indicators/src/companion.rs::aligned_change`:** nó
khớp `bar.time` **chính xác**, bar thiếu thì `continue` (`// missing, not zero
and not the previous value`), và có test
`a_truncated_primary_is_a_prefix_and_so_is_a_truncated_companion` — cắt chuỗi thứ
hai ở một mốc **không được đổi** giá trị nào tại hoặc trước mốc đó.

**Test nhân quả của job này, viết TRƯỚC khi đo ô nào** (3 điều khoản):

- **C1 (tiền tố).** Cắt chuỗi 1m ở mốc `T` tuỳ ý. Mọi đặc trưng của mọi nến 15m
  có `time + 900_000 <= T` phải **bằng y nguyên từng bit** so với khi chạy trên
  chuỗi đầy đủ. Lặp trên 5 mốc khác nhau.
- **C2 (bẫy tự bắn — một test luôn pass là một test vô giá trị).** Thêm một đặc
  trưng **cố tình nhìn trước** (`PROBE_peek_m16`, đọc close của phút 16) và đòi
  C1 **PHẢI THẤT BẠI** trên nó. Nếu C2 không bắt được thì C1 không chứng minh gì
  và job dừng.
- **C3 (lưới chính xác).** Kết cuộc chỉ đo khi `time(i+1) - time(i) == 900_000`
  đúng bằng một nến; qua cuối tuần / qua nghỉ phiên thì **từ chối, khai null**,
  không đo. Và nến `i+1` phải tồn tại.

## 5. Đặc trưng — ĐẾM TRƯỚC, 8 đặc trưng 1m và 6 đặc trưng đối chứng

Gọi 15 nến 1m của nến 15m `i` là `j = 1..15`; `O,H,L,C,RNG = H-L` là của nến 15m.
Mọi đặc trưng vô hướng, chuẩn hoá bằng `RNG` hoặc bằng số phút ⇒ so sánh được
giữa hai đầu hồ sơ (giá vàng chênh 7 lần — phụ lục 9 §II).

| # | tên | định nghĩa (chỉ O/H/L/C của 1m) | có hướng? |
|---|---|---|---|
| 1 | `tpo_skew` | (số phút có `(h_j+l_j)/2` ở nửa TRÊN của `[L,H]`) − (ở nửa dưới), chia 15 | có |
| 2 | `t_high` | `(argmax_j h_j − 1)/14` — đỉnh làm ở phút thứ mấy | có |
| 3 | `t_low` | `(argmin_j l_j − 1)/14` | có |
| 4 | `extreme_order` | `t_high − t_low` — đỉnh sau đáy (+) hay trước (−) | có |
| 5 | `path_eff` | `(C − O) / Σ_j |c_j − c_{j-1}|` (`c_0 = O`) — bò dần vs nhảy | có |
| 6 | `late_push` | `(C − c_11) / RNG` — phần biên độ đi trong 4 phút cuối | có |
| 7 | `max_1m_share` | `max_j (h_j − l_j) / RNG` — một cú nhảy vs rải đều | không |
| 8 | `tpo_poc_pos` | vị trí ô thời-gian-dày-nhất trong `[L,H]`, 10 ô, mỗi phút một trọng số bằng nhau trên các ô mà `[l_j,h_j]` phủ | có |

**Đối chứng (bắt buộc, brief): cùng mô hình, cùng mẫu, CHỈ BỎ 8 đặc trưng trên.**
6 đặc trưng chỉ từ O/H/L/C của nến 15m:
`ret_15 = (C−O)/RNG` · `body = |C−O|/RNG` · `uw = (H−max(O,C))/RNG` ·
`lw = (min(O,C)−L)/RNG` · `rng_atr = RNG/ATR14(15m)` · `close_pos = (C−L)/RNG`.

**Kết cuộc (đúng chỗ engine khớp):** `y = (close(i+1) − open(i+1)) / ATR14(i)`,
với `ATR14(i)` tính **tới hết nến `i`** (biết được ở thời điểm quyết định).
`RNG = 0` ⇒ null. ATR chưa warm ⇒ null.

## 6. Hai cửa sổ — TRÙNG KHỚP VỚI THƯỚC CỦA HỒ SƠ

`XAUDUKA` là nguồn 16 năm, và hai cửa sổ của hồ sơ (phụ lục 5 §B) là:

    W1  2010-06-01 -> 2018-06-01
    W2  2018-06-01 -> 2026-06-01

Job này cắt **đúng hai cửa sổ đó**, không cắt khác. Lý do: **drift lật dấu**
giữa chúng (−0,0040R → +0,0205R mỗi phiên), nên cắt sai cửa sổ là một **hiện
vật**, không phải một phép đo. Dữ liệu hết 2026-05-31 ⇒ W2 đầy.

## 7. Sổ đa phép thử — ĐẾM TRƯỚC, khai số

    A. don bien, buoc 1 :  8 dac trung x 2 cua so        = 16 phep so (CHINH)
    B. don bien, giam mau:  8 dac trung x 2 cua so x
                            2 buoc (2 va 4)              = 32 phep so (kiem sqrt(k))
    C. mo hinh long nhau:  (doi chung) vs (doi chung+1m)
                            x 2 cua so x 2 huong fold     =  4 phep so (DOI CHUNG)
    ------------------------------------------------------------------
    TONG KHAI:                                             52 phep so
    O CONG TIEU:                                            0  (tien kiem truoc)

Vạch Bonferroni cho **16 phép so chính** ở α = 0,05: `α/16 = 0,003125` ⇒
**|t| >= 2,95** (hai phía). Khai trước; không hạ vạch sau khi thấy số.

**`t` trên mẫu KHÔNG CHỒNG LẤN.** Cửa sổ kết cuộc của tín hiệu `i` là nến `i+1`,
của `i+1` là nến `i+2` ⇒ các kết cuộc **đã rời nhau** ở bước 1. Nhưng đặc trưng
của hai nến liền kề tự tương quan, nên trục B đo `|t|` ở bước 1 / 2 / 4 và kiểm
tỉ số có khớp `√k` (1,00 / 1,41 / 2,00). **Nếu `|t|` tụt NHANH hơn `√k` thì
`|t|` ở bước 1 bị phồng và con số bước 4 là con số được báo** — đúng cái bẫy
`lead-lag` đã gặp (`t +1,33` thành `t +3,78` do lấy mẫu chồng lấn).

## 8. Falsifier — cụ thể và bắn được

- **F1 (tiền kiểm, đồng dấu hai cửa sổ).** Bắn khi **không** đặc trưng 1m nào
  đạt `|t| >= 2,95` **cùng dấu** ở **cả W1 và W2** (ở bước mẫu được chọn theo
  §7). F1 bắn ⇒ **tiêu 0 ô cổng**, báo, xong. Tiền lệ: trục AUDNZD khai 432 ô,
  F2 bắn ở tiền kiểm, tiêu 0.
- **F2 (ĐỐI CHỨNG BẮT BUỘC, chính là falsifier của brief).** Cùng mô hình, cùng
  mẫu, chỉ bỏ 8 đặc trưng 1m. Bắn khi `ΔR²_oos <= 0` **hoặc** `Δ(độ chính xác
  dấu)_oos <= 0` ở **một trong hai** cửa sổ. F2 bắn ⇒ **độ phân giải mịn hơn
  không chứa thông tin dùng được ở chân trời 15m**, và desk thôi hỏi về dữ liệu
  tick/1m cho lớp cơ chế này. Fit **trong mẫu KHÔNG được dùng để đọc** — một mô
  hình nhiều đặc trưng hơn luôn khớp tốt hơn trong mẫu.
- **F3 (nhân quả).** C1 trượt trên bất kỳ đặc trưng thật, hoặc C2 **không** trượt
  trên `PROBE_peek_m16` ⇒ dụng cụ không đọc được, mọi số bỏ, báo.
- **F4 (hiện vật cửa sổ thứ 13).** Đặc trưng nào sống ở một nửa và lật dấu ở nửa
  kia ⇒ **hiện vật cửa sổ**, không phải cơ chế (phụ lục 9 §IV: đã đếm 12).

## 9. Cách đọc — nếu và chỉ nếu F1 và F2 đều KHÔNG bắn

Chỉ khi đó mới tiêu ô cổng, và khi đó:
`PF >= 1,200` VÀ `E >= +0,050R` VÀ `n >= 40` trên **cả hai** cửa sổ; `n` **đếm
bằng tay** (tool kiểm `need 30`, cổng desk 40); `PF_r` **cạnh** `PF_usd`, khai
đơn vị, `E = total_r/n` (3 chữ số của `expectancy` không đủ — phụ lục 8 §IV);
`max_drawdown_usd` cạnh mọi số lợi nhuận, `_pct > 100%` ⇒ dòng đã cháy;
`--exit-mix` ở mọi ô + **kiểm luật của cơ chế có nổ**; hai arm, và nếu chỉ sống
ở arm không-guards thì **nói thẳng là không giao dịch được**; `cap_lots` báo tỉ
lệ. **Không hứa chỉ tiêu nào về phân vị** — phân vị không phải cổng, và
`SURVIVES` phụ thuộc chỗ ngồi trong file TOML.

## 10. Chi phí I/O — khai trước là sẽ IN RA

5.635.777 nến 1m là kho lớn nhất hồ sơ từng chạm. Receipt phải in **thời gian
tường** và **bộ nhớ đỉnh** của lần chạy. Nếu phải tiền-tính một lớp đặc trưng thì
**ghi ra đĩa trong `target/` của worktree này**. `df -h /e` trước/sau;
**không xoá `target/`** vì thấy `df` thấp (thủ phạm là `pagefile.sys` 21–37 GB);
chỉ dừng nếu **dưới 3 GB**.

## 11. Thứ job này KHÔNG nói

- Không nói gì về volume. Cột đó null 100% trên 1m và chế ra 0 trên 15m.
- Không nói gì về chân trời khác 15m. Kết cuộc đo là **một** nến 15m tiếp theo,
  đúng chỗ engine khớp.
- Không nói gì về `XAUUSD` (feed khác, tick volume, đổi hình ở 2023-12).
- Nếu F1 hoặc F2 bắn, job này **không** nói "1m vô dụng" — nó nói **ở chân trời
  15m, với 8 định nghĩa hình này, trên hai cửa sổ này**, không đo được thông tin
  dùng được vượt O/H/L/C của nến 15m.
