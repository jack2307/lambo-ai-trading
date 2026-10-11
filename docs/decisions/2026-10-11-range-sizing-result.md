# KẾT QUẢ — tín hiệu biên độ của 1m là THẬT và KHÔNG TIÊU ĐƯỢC: đặt stop theo biên độ dự báo làm `PF_r` XẤU ĐI, 8/8 phép so, cả hai cửa sổ, cả hai arm

**Ngày:** 2026-10-11. Nhánh `agent/range-sizing`. Đăng ký trước:
`docs/decisions/2026-10-11-range-sizing.md` (commit `b9d2a8b`, **chỉ docs**;
ghi chú thêm cùng ngày `acd852f`). Receipt: `receipts/range-sizing/` — 1 tiền
kiểm + 12 lần chạy + `TABLE-all-cells.txt`.

**Outcome: F1 BẮN.** Stop lấy từ biên độ **dự báo** của nến tới **không** đánh
bại stop ATR cùng cỡ trung bình — nó **thua ở cả 8 phép so** (2 cửa sổ × 2 arm
× 2 cỡ stop), và thua ở **arm có guards** trên **cả hai** cửa sổ. Cùng với kết
quả chiều đêm 10/10, **cả họ "độ phân giải mịn hơn" đóng**: ở chân trời 15m,
nến 1m không mua được gì **ở chiều** lẫn **ở cỡ**.

    t phan du cua tin hieu : +28,303 (W1) / +38,060 (W2)   <- lon nhat ho so
    dPF_r no mua duoc      : -0,0085 (W1) / -0,0206 (W2)   <- AM, arm co guards
    khoang can de qua cong : +0,4885 (W1) / +0,4127 (W2)
    => tin hieu giao -1,74% / -4,99% cua khoang can. Dau SAI, va do lon
       nho hon hai bac so voi cai can.

---

## 1. Việc, một câu

Dựng chuỗi `m(i)` — biên độ **dự báo ngoài mẫu** của nến 15m tới, OLS khớp đi bộ
tiến từ **6 đặc trưng 15m + 8 đặc trưng 1m** (`XAUDUKA-1m`, 5.635.777 nến) —
rồi dùng nó **đặt cỡ stop** cho một cơ chế `Exits::Engine` mới (`range-stop`),
so với stop ATR **cùng cỡ trung bình** và với **chính dự báo đó xáo trộn trong
thời gian**.

## 2. Cơ chế — khai bằng `Exits::` trong MÃ

    crates/fd-strategy/src/range_stop.rs
        fn exits(&self) -> Exits { Exits::Engine }

Và nó **không có exit riêng nào**: `on_bar` trả `Intent::None` khi có vị thế,
nên stop / target (`reward_risk` × risk, do `open_position` dẫn ra) /
`max_hold_ms` **là của engine**. Lý do không dùng `donchian-breakout` làm nền:
cơ chế đó tự thoát ở đường giữa kênh, nên cỡ stop sẽ là **một trong hai** thứ
quyết định kết cuộc thay vì là thứ duy nhất.

**Stop ĐƯỢC THI HÀNH, đo chứ không giả định** — `--exit-mix` ở mọi ô:

    stopAtr 1,0 : STOP 63,7-65,2% so lenh   (6.263-8.115 lan)
    stopAtr 2,0 : STOP 44,7-47,0% so lenh   (3.434-4.244 lan)

Đối lập với `stop-width`, nơi ở `f ≥ 1,000` cái stop nổ **0–3 lần trong 1.355
lệnh** vì cơ chế là `Exits::Strategy`. **Ở đây nó là exit chính.** Đây là điều
kiện duy nhất khiến trục này không phải một phép đổi đơn vị, và nó thoả.

## 3. Cổng

**0 / 48 ô qua, và không ô nào gần.** `PF_r` cao nhất cả bảng là **0,8644**
(W2, guards-off, `shuf`, f=2,0) so với vạch **1,200**; `E` cao nhất là
**−0,0730 R** so với vạch **+0,050R**. Đếm tay, không đọc verdict của tool
(tool kiểm sàn 30 lệnh, cổng desk là 40 — ở đây `n` từ **7.522** đến **12.598**
nên chân số lệnh không ràng buộc ở bất kỳ ô nào).

**Chân nào ràng buộc:** `Lbar` từ **0,5200** đến **0,7272 R**, tức
**`Lbar ≥ 0,250R` ở 48/48 ô** ⇒ theo hằng đẳng thức `E = Lbar × (PF_r − 1)`
(`agent/gate-legs`) **chân expectancy là DƯ ở mọi ô** và **chân PF là chân duy
nhất ràng buộc**. Hằng đẳng thức kiểm được từng dòng trên receipt: phần dư lớn
nhất đọc được là **0,00007 R**.

**⚠️ 12/48 ô in `max_drawdown_pct > 100%` ⇒ 12 DÒNG ĐÃ CHÁY** (khuyết điểm 14).
Và cả 48 ô nằm trong dải **98,57% – 100,24%** với `ddUSD ≈ 10.000` = đúng
`starting_equity_usd` ⇒ **mọi dòng ở đây mất gần hết hoặc hết sổ.** Nên:

- **`PF_usd` và drawdown của trục này KHÔNG ĐỌC ĐƯỢC.** Engine giao dịch qua mốc
  cháy vì `lots = (...).max(min_lot)`, nên `PF_usd` của một dòng đã cháy **bị
  làm đẹp**.
- **`PF_r` và `E` vẫn đọc được** vì `r = points/risk` không đọc `lots`. Đó là
  lý do đăng ký §7 khai trước rằng con số quyết định là `ΔPF_r`.

## 4. Falsifier — cái nào bắn, ở đâu

| | bắn? | ở đâu |
|---|---|---|
| **F1** dự báo thắng ATR cùng cỡ, hai cửa sổ, arm guards | **BẮN — 8/8 phép so ÂM** | bảng 3 |
| **F2** xáo trộn tái tạo ≥ 50% cú dịch | **BẮN MỘT PHẦN, và nó lật ngược giả thuyết** | §5 |
| **F3** `fc1m` thắng `fc15` | **KHÔNG bắn** (6/8 dương) — nhưng cả hai âm so với ATR | bảng 3b |
| **F4** C1 trượt hoặc probe không trượt | **KHÔNG bắn** — sau khi vá một khuyết điểm của chính dụng cụ | §6 |
| **F5a** `p90/p10 < 1,20` | **KHÔNG bắn** — đo **1,5653–1,6149** | tiền kiểm |
| **F5b** `corr(m_fc1m, m_fc15) ≥ 0,995` | **KHÔNG bắn** — đo **+0,9555 / +0,9621** | tiền kiểm |
| **F6** sống ở một cửa sổ | **KHÔNG bắn** — cái âm **đồng dấu cả hai cửa sổ** | bảng 3 |
| **F7** `_pct > 100%` | **BẮN ở 12/48 ô** | §3 |

### F1, con số: `ΔPF_r` ở `stopAtr` y nhau, tập lệnh y nhau, cỡ stop trung bình y nhau

    cs  arm   f     PF_r(atr)   fc15      fc1m      shuf   | d.fc1m
    W1  off  1,0     0,7221    0,7139    0,7211    0,7110  | -0,0010
    W1  off  2,0     0,8254    0,7965    0,7877    0,8203  | -0,0377
    W1  ON   1,0     0,7115    0,6992    0,7030    0,7040  | -0,0085
    W1  ON   2,0     0,8133    0,7926    0,7849    0,8108  | -0,0284
    W2  off  1,0     0,7853    0,7708    0,7713    0,7791  | -0,0140
    W2  off  2,0     0,8579    0,8455    0,8470    0,8644  | -0,0109
    W2  ON   1,0     0,7873    0,7644    0,7667    0,7803  | -0,0206
    W2  ON   2,0     0,8542    0,8441    0,8451    0,8580  | -0,0091

**`d.fc1m` âm ở 8/8.** Không có một ô nào, một arm nào, một cửa sổ nào, một cỡ
stop nào mà stop dự báo thắng stop ATR. Và vì cái âm **đồng dấu ở cả hai cửa
sổ** nên nó **không** phải hiện vật cửa sổ thứ 13 — nó là một kết quả.

**Cơ chế của cái âm, đọc được từ `--exit-mix`** (W1, guards-off, f=2,0):

    atr/f2.0 : STOP 3615  TARGET 1530  TIMEOUT 2613   hold 201,0 min
    fc/f2.0  : STOP 3616  TARGET 1432  TIMEOUT 2686   hold 198,5 min

Số STOP **không đổi** (3615 → 3616) còn **TARGET rơi 98 lệnh và TIMEOUT tăng
73**. Đó chính là hướng mà `Exits::Engine` lớp L2 phải đi: target là
`reward_risk × risk`, nên **nới stop trên một nến sắp rộng cũng nới target đúng
tỉ lệ**, và cái target xa hơn đó phải vào được trong **trần `max_hold_ms = 4h`
= 16 nến 15m**. Dự báo đúng về biên độ ⇒ R to hơn ở đúng nơi giá đi nhiều ⇒ **ít
R hơn cho cùng một cú đi, và nhiều lần hết giờ hơn.**

⇒ **Giả thuyết của brief ("ít bị quét ở nến rộng, R chặt hơn ở nến hẹp") KHÔNG
xảy ra**: nó không làm giảm số lần bị quét (STOP đi ngang), nó chỉ **đổi target
thành timeout**.

## 5. F2 — xáo trộn, và nó lật ngược giả thuyết chứ không chỉ đóng trục

    d.shuf / d.fc1m :  11,10   0,14   0,88   0,09   0,44  -0,60   0,34  -0,42

Tỉ số không đọc được như một phần trăm (mẫu số bé và cả hai dấu), nên đọc theo
**độ lớn tuyệt đối**, và nó tách sạch theo cỡ stop:

    stopAtr 1,0 : |d.fc1m| 0,0010-0,0206   |d.shuf| 0,0062-0,0111
                  => xao tron hong BANG hoac HON ban that
    stopAtr 2,0 : |d.fc1m| 0,0091-0,0377   |d.shuf| 0,0025-0,0065
                  => ban that hong 1,4x den 11,4x NHIEU HON xao tron

**Hai kết luận khác nhau, cả hai đều đóng trục:**

1. Ở `stopAtr = 1,0`, **phần lớn — có ô là toàn bộ và hơn — cú dịch được tái
   tạo bởi một dự báo xáo trộn cùng phân phối.** Tức ở đó cái làm `PF_r` xấu đi
   là **việc cho stop biến thiên**, không phải việc nó biến thiên **theo cái
   gì**. Đúng vạch F2 đã khai: trục đóng.
2. Ở `stopAtr = 2,0`, **bản thật hỏng NHIỀU HƠN bản xáo trộn 1,4–11,4 lần.**
   Đây mạnh hơn F2: thời điểm của dự báo **không** vô nghĩa — nó **có hướng, và
   hướng đó sai tiền**. Nới stop đúng vào nến sắp rộng là một quyết định **tệ
   hơn** nới nó ngẫu nhiên.

## 6. Đối chứng DƯƠNG — hai cái, và cả hai bắt được một khuyết điểm trước khi báo số âm

**Đối chứng 1 (tiền kiểm): dụng cụ đọc được tín hiệu, và nó TÁI TẠO `intrabar-shape`.**

    DELTA R2_oos (1m THEM vao, CUNG TAP DONG) = +0,006178 (W1)  +0,009181 (W2)
    intrabar-shape truc G do                  = +0,003780 ... +0,011323 (4 fold)
    corr(pred, y) thuc te                     = +0,3168 (W1)   +0,3758 (W2)

**Dương ở cả hai cửa sổ**, trong dải của họ. Nên "F1 bắn" **không** phải một
dụng cụ mù: cùng đường ống đọc được tín hiệu biên độ, rồi đọc được rằng nó
không trả tiền.

**Đối chứng 2 (engine): nhãn `atr/*` phải in SỐ Y HỆT ở cả ba lần chạy
companion** (nó không đọc giá trị companion, chỉ đọc sự tồn tại). **PASS 8/8
nhóm** — `n` và `PF_r` trùng tới chữ số in ra trên cả ba file. Đây là parity duy
nhất có được cho một cơ chế mới, và nó là chuẩn của đợt này.

**Nhưng lần chạy đầu TRƯỢT đối chứng 2** — và đó là khuyết điểm đáng nhất tìm
được:

    atr/f1.0 lan chay fc15 : n = 10.888      lan chay fc1m : n = 10.430
                                             lech 4,4% SO LENH

Vì `fc15` không cần nến 1m nên file companion của nó có **357.794** dòng có số
so với **338.423**, và `range-stop` từ chối nến `cmp.close` NaN ⇒ **mỗi file
companion định nghĩa một TẬP NẾN khác.** Điều khoản "cùng tập nến" của đăng ký
được thi hành **trong** một lần chạy nhưng **không giữa** các lần chạy. Hệ quả
nếu không bắt được: `d.fc15` và `d.fc1m` sẽ là hai số đo trên **hai mẫu khác
nhau**, và F3 sẽ không đọc được. Vá: **một mặt nạ NULL chung** (bịt `fc15` về
đúng mặt nạ của `fc1m`), rồi chạy lại cả 12 lần. Sau vá: 338.423 / 378.749 =
**89,3528%** nến ở cả ba file, đối chứng 2 PASS.

**Và nó cũng lật một con số ở tiền kiểm theo cùng một kiểu:**
`ΔR²_oos` ở W1 đọc **−0,000683** khi so hai mô hình trên **hai tập dòng của
riêng chúng**, và **+0,006178** khi so trên **tập dòng chung**. **Đổi dấu.**
Đây đúng là cái `intrabar-shape` §6 cảnh báo, và nó xảy ra với tôi một lần ở
Python và một lần ở Rust.

## 7. Nhân quả — C1/C2/C3, và một khuyết điểm nữa của chính dụng cụ

    C1 PASS : 14 dac trung + pred + m, x 5 moc cat, 0 gia tri doi (TUNG BIT)
    C2 PASS : BA probe co y nhin truoc TRUOT C1 o 5/5 moc:
              PROBE_peek_m16  doc close phut 16 (phut dau cua nen i+1)
              PROBE_y         dua y(i) vao lam BIEN HOI QUY
              PROBE_fullfit   khop OLS MOT lan tren CA chuoi  <- tang KHOP
    C3 PASS : 12 khe tren lat thu, y co so o 0 trong so do
    NULL != 0 PASS : 587 nen thieu nen 1m tren lat thu, 0 mang mot so m

`PROBE_fullfit` là probe thêm vào trước khi chạy, và nó là cái duy nhất test
**tầng khớp**: hai probe kia chỉ test tầng đặc trưng. Điều khoản làm C1 đọc
được: một dòng chỉ vào tập khớp ở mốc `B` khi **kết cuộc của nó đã quan sát
xong trước `B`** (`t15[i] + 2 × 900.000 ≤ B`); không có điều khoản đó thì
`beta` đọc một con số chưa tồn tại và C1 trượt.

**Khuyết điểm của dụng cụ, tìm ra VÀ vá:** phép đo đầu tiên **TRƯỢT C1 ở đúng 1
giá trị trên 1 trong 5 mốc**:

    0.9963305513228559   so voi   0.996330551322856

Không phải rò nhân quả — tập khớp bằng nhau từng dòng nên `beta` bằng nhau. Là
**GEMM của BLAS đổi thứ tự cộng khi số dòng của lô đổi**, và cắt chuỗi đổi số
dòng của lô dự báo. Vá bằng cách cộng **từng phần tử theo trục đặc trưng**
(`predict_rows`), làm mỗi dòng độc lập với mọi dòng khác ⇒ bằng **từng bit** bất
kể lô dài bao nhiêu. **C1 giữ ở mức "từng bit", không hạ xuống "trong 1 ulp"** —
hạ vạch sau khi thấy số là chính cái brief cấm.

## 8. Đối chứng CÙNG CỠ TRUNG BÌNH — và tại sao nó là bắt buộc, bằng số

Trên **đúng nến tín hiệu** (luật vào lệnh của `range-stop` dựng lại trong
Python), ở cùng `stopAtr`, **trước** khi hiệu chỉnh:

    W1 : atr 1,7882 USD   fc15 2,3289 (+30,24%)   fc1m 2,3051 (+28,91%)   shuf 1,7782 (-0,56%)
    W2 : atr 3,2484 USD   fc15 3,9419 (+21,35%)   fc1m 3,9304 (+20,99%)   shuf 3,2539 (+0,17%)

⇒ **Dự báo nới stop thêm 21–29% trên đúng những nến cơ chế vào lệnh** — vì nến
phá kênh **là** nến dự báo nói sẽ rộng. Vượt hẳn vạch 2,0% khai trước. Không
hiệu chỉnh thì mọi `ΔPF_r` ở đây là **một phép đổi cỡ stop**, lớp mà
`agent/stop-width` đã đo trên 200 ô, **không** phải một phép đo về dự báo.

Hằng số khớp cỡ, **một số mỗi (cửa sổ, nguồn), khai ra, chia vào chính file
companion** — và nó là một hằng số **TRONG MẪU**, cái giá phải trả để đối chứng
(a) là một đối chứng:

    W1 : fc15 1,302374   fc1m 1,289088   shuf 0,994411
    W2 : fc15 1,213487   fc1m 1,209946   shuf 1,001699
    kiem lai sau khi chia: +0,0000% / -0,0000% o ca sau

**Kiểm chéo từ phía engine, độc lập với Python:** `realised stop: median` mà
engine tự in, ở cùng ô:

    W1 f1,0 : atr 1,092 ATR   fc1m 1,104 ATR   (lech +1,1%)
    W1 f2,0 : atr 2,093 ATR   fc1m 2,098 ATR   (lech +0,2%)
    W2 f1,0 : atr 1,061 ATR   fc1m 1,074 ATR   (lech +1,2%)
    W2 f2,0 : atr 2,062 ATR   fc1m 2,067 ATR   (lech +0,2%)

Cỡ stop khớp nhau trong **1,2%** theo thước của engine. **Không** dùng thước
`risk_usd = |swap$|/|Δtotal_r|` (phụ lục 10 §III: nó sai **2,22 lần**).

Và một thứ phải nói ra: **`stopAtr` là một nhãn danh nghĩa, không phải cỡ stop
thật.** Ở `stopAtr = 1,000` engine in realised stop **1,061–1,104 ATR** — rộng
hơn 6–10% — vì `risk = |entry − stop|` với `stop` tính từ `close(i)` còn `entry`
là `open(i+1)` đã qua `apply_costs`. Phần lớn 6–10% đó là **cú gap close→open**,
không phải δ.

## 9. Khuyết điểm 16 — δ là một phần BIẾN THIÊN của R ở trục này

Dùng số **từ MÃ**, không từ phép khớp: `apply_costs` (`engine.rs:902-912`) +
`sizing_stop` + `risk` (`engine.rs:717-719`) cho **`d risk/d s = +1/2` chính
xác**, nên

    delta = spread / 2 = 0,28 / 2 = 0,1400 diem gia

(Phụ lục 7 ghi 0,138 từ một phép khớp — lệch 1,4%, và phép khớp **không phân
giải được** đại lượng này.) Vì stop của trục này **biến thiên theo nến**, δ là
một phần **biến thiên** của R, và đây là độ lớn:

    stopAtr 1,0 : delta = 14,34% cua R o p10(d) ... 3,38% o p90(d)   (W1)
                          12,26%               ... 1,85%             (W2)
    stopAtr 2,0 :          7,17%               ... 1,69%             (W1)
                           6,13%               ... 0,93%             (W2)

⇒ **Ở `stopAtr = 1,0` cửa sổ W1, δ đi từ 14,34% của R xuống 3,38% của R trong
cùng một ô** — một **biến thiên 4,2 lần** của một thành phần không khai báo của
mẫu số R, đi **cùng hướng** với chính dự báo (stop hẹp ⇒ δ chiếm phần to hơn).
Nó **thiên vị** `ΔPF_r` theo hướng **có lợi** cho nhánh có stop hẹp. Cái âm đo
được **không** giải thích được bằng nó (nó đẩy ngược chiều cái âm), nhưng nó là
một thành phần không sạch của mỗi con số R ở đây. **Đếm, không sửa.**

## 10. Chi phí/R — bốn thứ, và nó là thứ to nhất trong hồ sơ ở trục này

**Cửa sổ + thống kê + cỡ stop + spread đo được hay GIẢ ĐỊNH** (phụ lục 9 §II):

    nguon spread : 0,28 = `markets.xauduka.spread` trong CAU HINH, KHONG do boi job nay
    thong ke     : trung vi (p50) cua d tren nen tin hieu
    W1 (2010-06 -> 2018-06) : stopAtr 1,0 => 15,03% cua R (d = 1,862 USD)
                              stopAtr 2,0 =>  7,52% cua R (d = 3,725 USD)
    W2 (2018-06 -> 2026-06) : stopAtr 1,0 => 10,68% cua R (d = 2,621 USD)
                              stopAtr 2,0 =>  5,34% cua R (d = 5,243 USD)

Và dòng `cost-matched null` mà engine tự in, trên cùng các ô: **18,42–18,79%**
(W1, f=1,0), **9,30–9,41%** (W1, f=2,0), **12,23–12,45%** (W2, f=1,0),
**6,23–6,31%** (W2, f=2,0). Khác số Python vì engine lấy **cỡ stop danh nghĩa
của null** (`1,000 ATR = 0,96 điểm`) chứ không lấy trung vị thực tế — **hai
thước, báo cả hai.**

Con số **4,04%** lưu hành là trung vị của cửa sổ 2 năm gần nhất và **sai cửa sổ**
cho mọi thứ ở đây. Nhắc thêm: phụ lục 9 §II đo **12,58% / 8,35%** ở
`1,5 ATR(15m)` trên đúng hai cửa sổ này, và `stopAtr = 1,0` của tôi hẹp hơn 1,5
lần nên **15,03% / 10,68% khớp với nó**, không mâu thuẫn.

## 11. `sized_down_by_guard` — đọc từ FIELD, mọi ô, và nó ĐỔI theo stop của tôi

`hypotheses.rs:752`; `guards: nothing acted` nghĩa là `sized_down == 0`, **không
phải** chuỗi bị thiếu. Arm guards-ON, tỉ lệ số lệnh bị `cap_lots` thu nhỏ:

    W1 f1,0 : atr 98,07%   fc15 96,90%   fc1m 96,96%   shuf 97,64%
    W1 f2,0 : atr 78,28%   fc15 72,82%   fc1m 72,95%   shuf 77,01%
    W2 f1,0 : atr 96,31%   fc15 94,74%   fc1m 94,73%   shuf 95,99%
    W2 f2,0 : atr 75,04%   fc15 71,56%   fc1m 71,81%   shuf 74,86%

**Tỉ lệ bịt THAY ĐỔI theo nguồn stop** (−1,1 đến −5,5 điểm% so với ATR), đúng
như điều cảnh báo: dự báo cố ý làm stop biến thiên ⇒ nó đi thẳng vào trần
`notional 300%` + `max_open_loss_r = 2,0`. **Và đây là thuộc tính của họ
`Exits::Engine` CƯỠI TRẦN NOTIONAL, không phải của arm guards** — `n` ở arm
guards-ON **không** đổi vì bật guards; nó đổi vì stop đổi.

Hệ quả, và nó là lý do đăng ký §7 khai trước con số quyết định: **mọi `ΔPF_usd` ở
đây có một phần là phân bố cỡ lot dịch, không phải cơ chế.**

**Khe `PF_usd − PF_r` đo trên bộ dữ liệu của tôi (48 ô) — và nó TÁCH HOÀN HẢO
THEO CỬA SỔ:**

    PF_usd CAO hon PF_r :  24/48 o = 50,0%
      W1 (2010-06 -> 2018-06) : 24/24  (100%)
      W2 (2018-06 -> 2026-06) :  0/24  (0%)
    |khe| tu 0,0178 den 0,1727
    o co hai chan nam hai phia vach 1,200 : 0/48 (ca hai chan duoi xa)

⇒ Phép đo thứ **tám** cho phụ lục 10 §V, và là cái **sạch nhất**: chiều của khe
`PF_usd − PF_r` ở đây **không phải một tỉ lệ, nó là một HẰNG SỐ CỦA CỬA SỔ** —
100% một chiều ở W1 và 100% chiều kia ở W2, trên cùng một cơ chế, cùng tham số,
cùng arm. Nó **không đồng ý** với bất kỳ tỉ lệ nào trong bảy phép đo kia (71% /
73,8% / 47,2% / 60,4% / 72% / lật dấu / 69,4%) và nó giải thích tại sao chúng
mâu thuẫn nhau: **mỗi phép đo kia trộn hai cửa sổ theo một tỉ lệ khác.**
**Đừng trích tỉ lệ của ai — kể cả 50,0% của tôi.**

## 12. Sổ đa phép thử — khai vs xem

    khai (dang ky + ghi chu cung ngay):  32 o cong      xem: 32 o cong
      4 nguon stop {atr, fc15, fc1m, shuf} x 2 stopAtr x 2 cua so x 2 arm = 32
      + 8 o TRUNG LAP co y (nhan `atr/*` x 3 file companion) = doi chung duong 2
      => 48 dong in ra, 32 o phan biet.  KHONG THEM O NAO sau khi thay so.

    tien kiem (Python, 0 o cong): C1 x 5 moc, C2 x 3 probe x 5 moc, C3,
      4 so R2_oos, 4 so corr, 6 phan phoi m, 2 corr(m,m), 8 co stop trung binh,
      4 ti so delta/d  — khai 0 o cong, tieu 0 o cong.

    Lan chay dau (48 dong) BI HUY va chay lai sau khi vap doi chung duong 2.
    Khong dong nao cua lan dau duoc bao; con so duy nhat giu lai tu no la
    chinh cai lech 10.888 vs 10.430 o muc 6.

**Phân vị KHÔNG công bố** (phụ lục 8 §VI: `SURVIVES` phụ thuộc chỗ ngồi trong
file TOML). `seeds = 20` chỉ để dòng `null p50` tồn tại. Cổng là cổng.

## 13. Thứ KHÔNG đo được, và vì sao

- **`PF_usd` và drawdown.** 12/48 ô in `_pct > 100%` và cả 48 nằm ở 98,57–100,24%
  với `ddUSD ≈ starting_equity` ⇒ **sổ đã cháy hoặc gần cháy ở mọi ô**. Không
  đọc. Không in một số 0 thay cho chúng.
- **`wrong_side_stop`.** `grep -rn wrong_side_stop crates/` → **0 kết quả** trên
  cây này ⇒ **không có bộ đếm**, nên **không đo được**, và không báo một số 0 đọc
  từ một bộ đếm không tồn tại (phụ lục 9 §VI).
- **Lớp L3** (stop từ dự báo, target là một MỨC GIÁ). Đăng ký khai L2 và chỉ L2.
  §4 cho thấy cái hỏng ở L2 là **target đi theo stop** ⇒ L3 là chỗ duy nhất giả
  thuyết còn một đường sống, và nó **chưa được đo**. Nói thẳng là chưa.
- **Trần `max_hold_ms = 4h` = 16 nến.** Nó **không** được nới; nó được khai. Và
  vì cái hỏng đo được đi qua TIMEOUT, **một phần dấu của kết quả này có thể là
  thuộc tính của đúng cái trần đó** (phụ lục 10 §I nhắc đúng chuyện này).
  Không đo được ở đây.
- **Một luật vào lệnh, không phải mọi luật.** Kết luận là về `donchian 20` trên
  `xauduka:15m`. Một luật vào lệnh khác có thể có phân bố nến tín hiệu khác.
- **Filter `flat 16:30-18:15`.** `range-stop` chạy **`filters = []`** ⇒ nó **KHÔNG**
  miễn nhiễm với dấu giá nhà cung cấp quanh mốc nghỉ 17:00 NY (phụ lục 10 §IV:
  `open` của nến mở lại là đáy nến ở **19,02%** nến `XAUDUKA`). **Kiểm, và câu
  trả lời là: không có filter đó.** Nó ảnh hưởng cả bốn nhãn như nhau nên nó
  khó giải thích được một `ΔPF_r`, nhưng nó ở trong **mức** của mọi `PF_r` trên
  bảng.
- **Phi tuyến.** Mô hình là OLS. Một dự báo biên độ phi tuyến có thể khác.

## 14. Cái đóng, và cái mở

**ĐÓNG — và đây là kết luận lớn của trục:** *nến 1m làm nguồn cho CỠ STOP ở chân
trời 15m.* Cộng với đêm 10/10 (*chiều*: 0/8 đặc trưng, `|t| < 1`), **cả họ "độ
phân giải mịn hơn" đóng ở chân trời 15m.** Hình dạng chính xác của kết luận:

    tin hieu 1m ve BIEN DO la THAT:  t phan du 28,3 / 38,1, DELTA R2_oos duong
                                     ca hai cua so, tai tao duoc doc lap
    va no KHONG TIEU DUOC         :  dPF_r AM 8/8, arm co guards, ca hai cua so
    va huong cua no SAI TIEN      :  o stopAtr 2,0 ban that hong 1,4-11,4 lan
                                     NHIEU HON ban xao tron cung phan phoi

**Sẽ KHÔNG mở lại bằng:** thêm đặc trưng 1m thứ 9, đổi sang 5m hay tick, hay một
mô hình dự báo biên độ tốt hơn. Vấn đề **không phải** chất lượng dự báo — dự báo
đọc được và tái tạo được. Vấn đề là **đại lượng R bị chuẩn hoá bởi đúng cái mà
dự báo dự báo**, nên một dự báo đúng làm mẫu số to ra ở đúng nơi tử số to ra.

**MỞ, và là thứ duy nhất trục này để lại:** **lớp L3** — stop từ dự báo, target
là một **mức giá** (không đi theo stop). Ở đó kênh "ít bị quét ở nến rộng" vẫn
tồn tại mà kênh "target xa hơn nên hết giờ" **không** tồn tại. §4 đo được rằng
đúng kênh thứ hai là cái giết nó ở L2. **Nhưng** lưu ý trước cho ai mở: ở L3
`reward_risk` thôi là hằng số, nên hai ô L3 khác cỡ stop **không so sánh được ở
`PF_r`** theo cùng cách, và phụ lục 8 §III gọi L3 là "lớp duy nhất 'nới stop' có
câu trả lời sạch" **chính vì** lý do đó.

## 15. Thứ một người đọc có thể hiểu sai

- **Không** nói "dự báo biên độ sai". Nó đúng: `ΔR²_oos` dương cả hai cửa sổ,
  `corr(pred, y)` **+0,3168 / +0,3758**. Nói: **đúng, và đặt vào mẫu số của R ở
  lớp L2 thì nó lấy tiền chứ không đưa tiền.**
- **Không** nói "stop ATR là một cơ chế". Stop ATR ở đây đọc `PF_r` **0,7115–0,8579**
  — nó cũng thua cổng xa. Phép so là **giữa hai cách sai**, và dự báo là cách
  sai hơn.
- **Không** đọc `PF_usd` của bất kỳ dòng nào ở đây. 12/48 ô đã cháy và cả 48 mất
  gần hết sổ.
- **Không** nói "F3 không bắn" là một kết quả dương. `fc1m` đỡ hơn `fc15` ở 6/8,
  nhưng **cả hai âm so với ATR ở 8/8** ⇒ tám đặc trưng 1m làm một thứ đã sai
  **ít sai hơn**, không làm nó đúng.
- **Không** nói gì về volume. `XAUDUKA-1m.volume` null trên **5.635.777/5.635.777**
  dòng; `columns=["time","open","high","low","close"]` ở mọi lần đọc parquet,
  cột đó **không được mở một lần nào**.
