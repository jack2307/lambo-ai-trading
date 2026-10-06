# Hồi quy trên cross bản địa (AUDNZD) — đăng ký trước

Ngày: 2026-10-06. Nhánh `agent/aud`, cắt từ `agent/n2`.
Commit này **chỉ có file docs**, viết trước dòng code đầu tiên.

## Xuất phát

Agent `n2` đã đo họ **giá trị tương đối** hôm nay và đóng nó vì lý do **kiến
trúc, không phải vì PF xấu**:

> tỉ số `ln(XAU/XAG)` thực chất là **89–104% một instrument bạc** (vàng chỉ
> mang 4–11% phương sai). Chân mang tín hiệu trả **14,69% R** ở stop 1,5 ATR
> và qua **1/144** ô; chân rẻ (vàng, 3,45% R) mang gần như không có tín hiệu.
> Hai chân thật **không biểu đạt được** mà không viết lại lõi engine.

Chủ máy chỉ sang MQL5 Market. Bản ghi grid dài nhất ở đó (Waka Waka, 70+ tháng
lãi liên tiếp từ 2018) là **hồi quy về trung bình trên AUDCAD / NZDCAD /
AUDNZD** — **đúng họ cơ chế trên**, nhưng ở một dạng biểu đạt mà n2 không có:
**cross là chính cái tỉ số, dưới dạng MỘT symbol giao dịch được, với MỘT
spread FX.** Vấn đề "một chân trả 4,26× chi phí" biến mất, không phải vì ta
sửa engine mà vì sàn đã gộp hai chân thành một hợp đồng.

## Giả thuyết

Hồi quy z-score của `ln(close)` trên AUDNZD qua được cổng ở chỗ tỉ số tổng hợp
vàng/bạc không qua, **vì và chỉ vì** số hạng chi phí thấp hơn nhiều khi tỉ số
là một instrument bản địa.

## Falsifier — cả ba khai trước, cả ba bắn được

**F1 (chính).** 0 ô qua cổng trên **cả hai** cửa sổ ⇒ họ giá trị tương đối
**đóng ở CẢ HAI dạng biểu đạt** (tỉ số tổng hợp hai instrument của n2, và cross
bản địa một instrument ở đây). Khi đó bản ghi 70 tháng của MQL5 là bằng chứng
về **hình dạng chi trả của grid recovery**, không phải về cơ chế nền — và desk
đóng được một họ mà thị trường bán lẻ tin là có.

**F2 (tiền đề).** Nếu chi phí/R đo được của AUDNZD **không thấp hơn đáng kể**
14,69% của chân bạc ở cùng cỡ stop, thì tiền đề của phép thử này sai và kết
quả **không nói gì** về họ cơ chế — nó chỉ nói về một instrument. Ngưỡng: F2
bắn nếu AUDNZD ≥ 10,0% R ở stop 1,5 ATR.

**F3 (hướng).** Nếu tiền kiểm cho `corr(z, cú đi sau đó)` **DƯƠNG** (tiếp
diễn) ở cả hai cửa sổ — đúng như n2 đo được ở lookback 96 với t tới +12,88 —
thì một luật hồi quy đang đứng trước đoàn tàu, và tôi khai điều đó **trước
khi** chạy cổng, không sau.

## Tiền kiểm, chạy TRƯỚC khi chốt tham số

1. **Chi phí/R** của AUDNZD ở stop 1,5 / 2,0 / 3,0 ATR — lấy từ dòng
   `cost-matched null: ... cost X% of R` mà binary tự in, ghi kèm cỡ stop.
   Đây là chân F2.
2. **Variance ratio** VR(32) và **nửa đời hồi quy**, hai cửa sổ. Nửa đời quyết
   độ dài cửa sổ: n2 đo được rằng **cổng ≥40 lệnh cộng cửa sổ 3 tháng cùng
   nhau CẤM đo mọi cơ chế có chân trời quá ~1 ngày** (nửa đời tỉ số vàng/bạc
   là 7,8–8,7 ngày ⇒ tối đa ~8 lệnh không chồng lấn so với 40 lệnh cổng đòi).
3. **Dấu của `corr(z, cú đi sau đó)`** ở mỗi lookback, hai cửa sổ. Chân F3.

## Cửa sổ — và vì sao KHÔNG dùng 3 tháng

Vì phát hiện (2) ở trên, dùng **hai cửa sổ 4 năm không chồng nhau**, giống
cách `agent/n1` đã làm (và chính độ dài đó là thứ làm kết quả n1 đọc được):

    A: 2022-06-01 -> 2026-05-31
    B: 2018-06-01 -> 2022-06-01

Dữ liệu DUKA trên đĩa phủ 2010-06-01 → 2026-05-31; AUDNZD tải đúng span đó để
so sánh được với XAUDUKA / XAGDUKA / EURDUKA.

## Cổng — không chỉnh

    profit factor >= 1,200  VÀ  expectancy >= +0,050R  VÀ  >= 40 lệnh
    trên CẢ HAI cửa sổ

Phân vị không phải cổng. `null p50` viết cạnh mọi phân vị. Chạy **cả hai arm
guards** và báo cả hai. `--exit-mix` bật, và **kiểm luật của chính nó có nổ
không** trước khi tin con số nào (bẫy `tsmom/120d`: PF 2,236, engine in
`SURVIVES`, luật của nó nổ 0 lần).

## Đa phép thử — khai TRƯỚC

    lookback   { 96, 192, 480 }
    zEnter     { 1,5; 2,0; 2,5 }
    stopAtr    { 1,5; 2,0; 3,0 }
    revert     { +1, -1 }        <- arm tiếp diễn khai NGAY TỪ ĐẦU, vì n2 đo
                                    được dấu đảo; không phải thêm sau khi thấy
                                    arm hồi quy thất bại
    = 54 hàng

    54 hàng × 2 cửa sổ × 2 arm guards = 216 ô
    × 3 null (coin / ratio / exposure) = 648 quan sát

**Khai: 216 ô cổng, 648 quan sát.** Tiền kiểm đếm riêng, không có cổng.

## Giới hạn khai TRƯỚC, không phải bào chữa sau

1. **Grid recovery KHÔNG được đo.** Engine giữ một vị thế một lúc
   (`Intent::Enter { .. } if position.is_none()`). Phép thử này đo **cơ chế
   nền**, không đo thứ Waka Waka bán. Đó là chủ ý: nếu cơ chế nền có expectancy
   dương thì grid là đòn bẩy lên nó; nếu âm thì grid chỉ che nó đi, và 70 tháng
   là thời gian trước khi lộ.
2. **Feed Dukascopy bid, khác venue với tài khoản.** Giống hệt caveat đã ghi
   cho XAUDUKA.
3. **Chỉ AUDNZD.** AUDCAD và NZDCAD là phần còn lại của họ; nếu AUDNZD trượt
   cổng thì chúng là hai ô nữa chứ không phải một hướng mới, và phải khai lại.
4. **Spread AUDNZD chưa đo trên tài khoản của desk.** Sẽ lấy từ config, và
   phải nói rõ nó là **báo giá, không phải phép đo** — khác với FOMC spread mà
   `agent/n1` đo được từ `data/spreads/XAUUSD_sc.csv`.

## Code sẽ viết, và vì sao KHÔNG sửa `ratz`

`ratz` **cố ý** trả toàn NaN khi không có companion:
*"a method that reads two series and finds one must take no trades rather than
trade on this series' own value"* (`fd-indicators/src/lib.rs:571`). Đó là một
bất biến an toàn và **không được phá** — phá nó thì mọi receipt của n2 đổi
nghĩa.

Nên: **thêm indicator mới `lnz`** (z-score của `ln(close)` trên một cửa sổ
trượt, không cần companion) và một strategy đọc nó. `ratz` và
`ratio_reversion` **không bị chạm**, nên 36 receipt của n2 vẫn tái lập được
từng số.

---

## Ghi chú 2026-10-06, sau khi đọc config nhưng TRƯỚC khi chạy ô nào

Ba thứ không lường trước trong đăng ký gốc. Dòng cũ để nguyên.

**(a) Không có log spread AUDNZD.** `data/spreads/` chỉ có `BTCUSD_sc.csv`,
`XAGUSD_sc.csv`, `XAUUSD_sc.csv`. Nên `spread` trong config là **báo giá, không
phải phép đo** — khác hẳn con số FOMC mà `agent/n1` đo được từ 8.967 bản ghi
bid/ask thật. Hệ quả: **chạy một quạt `--spread=`** và công bố mức phí mà kết
luận vỡ, thay vì công bố một con số đơn. Nếu kết luận phụ thuộc vào báo giá
thì nó không phải kết luận.

**(b) Carry là cả câu chuyện trên một cross, và đăng ký gốc bỏ sót.**
`agent/n5` đo được hôm nay rằng 4 ô duy nhất qua cổng ở chân trời dài **chỉ
qua khi swap = 0**, và tính theo rate chung thì PF 1,383 → **0,109**. Một cross
AUD/NZD mang chênh lệch lãi suất hai nước; một luật hồi quy giữ vài ngày sẽ
trả hoặc nhận nó. Tài khoản của desk **miễn swap đã đo** — nhưng đo trên
`XAUUSD.sc` (340 lệnh, 87 lệnh qua đêm), **không phải trên AUDNZD.sc**, và
chưa biết sàn có áp cùng ưu đãi cho cross hay không.

⇒ **Khai thêm arm swap**, giống n5: chạy cả `swap = 0` và `swap = rate chung`.
Nếu ô nào chỉ qua ở arm swap = 0 thì nó là **ưu đãi tài khoản, không phải
chiến lược**, và phải viết đúng câu đó.

**(c) `contract_size` không chạm cổng.** Comment trong khối `eurduka` của
chính repo đã ghi: lot tính bằng `risk/(stop × contract_size)` và P&L là
`points × lots × contract_size`, nên **tích số là bất biến** và R, notional,
margin không đổi. Một giá trị sai chỉ sai ở **số lot** — thứ executor gửi cho
MT5. Vô hại trên giấy, không vô hại khi chạy thật. Ghi ra để không ai lấy số
lot từ receipt này.

**Đa phép thử cập nhật:** 216 ô cổng gốc **+ 216 ô arm swap = 432 ô**, cộng
quạt spread (khai sau khi biết mức nào đáng quét, đếm riêng và công bố).
