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

---

# KET QUA 2026-10-07: F2 BAN. 432 o cong KHONG duoc tieu.

## F2 ban, o ca hai cua so

    chi phi/R o stop 1,5 ATR    nguong F2    chan bac cua n2
    A  38,157%                  10,0%        14,69%
    B  36,395%                  10,0%        14,69%

Va khong co co stop nao trong luoi da khai cuu duoc: 2,0 ATR cho 28,6%/27,3%;
3,0 ATR cho 19,1%/18,2%.

**Tien de cua phep thu bi bac bo.** AUDNZD **dat gap 2,5 lan** chan bac ma no
le ra phai re hon. Ly do ro rang khi nhin lai, va dang ghi: mot cross la mot
instrument **bien dong THAP** voi mot spread **RONG** -- ATR 1,5 chi 7,9 pip
trong khi spread cross la 3 pip. Ti so `spread/stop` la thu quyet dinh, va day
la to hop te nhat co the cho ti so do. "Ti so la mot symbol giao dich duoc" co
that, nhung no khong keo theo "re hon".

## Theo dung cach doc da dang ky, toi KHONG tieu 432 o cong

Dang ky viet: F2 ban ⇒ "ket qua **khong noi gi** ve ho co che -- no chi noi ve
mot instrument". Tieu 432 o de do mot thu da tu khai la khong doc duoc chinh la
toi da phep thu ma ho so nay ton tai de chong. So da phep thu: **khai 432, tieu
0**.

## F3 KHONG ban -- va day moi la phan dang giu

Khac han vang/bac. `agent/n2` do duoc dau **DUONG** (tiep dien) tren ti so
vang/bac voi t toi +12,88, nen luat hoi quy cua no dang dung truoc doan tau.
AUDNZD thi nguoc lai:

    VR(32)  0,732 (A) / 0,682 (B)      <- thap hon ca vang/bac (0,798/0,921)
    corr(z, cu di 1 nen), lookback 96:
        A  r = -0,034   t = -10,68   n = 98.664
        B  r = -0,036   t = -10,90   n = 93.521
    dong dau o moi lookback (96/192/480) va o ca hai cua so

t tinh tren mau **KHONG chong lan** (buoc bang chan troi), vi ho so cua desk
ghi rang cua so chong lan phong t len ~sqrt(overlap) va con so drift +6,74 bi
nghi dung loi do.

**Nen: hoi quy tren cross la THAT, dau DUNG, nhat quan hai cua so -- va khong
voi tới duoc.** Kich thuoc kinh te, o z = 2,0:

    ky vong cu di co loi = 0,27 pip      phi mot luot = 3,00 pip
    edge = +0,035 R                      phi = 0,382 R        rong = -0,347 R
    spread hoa von = 0,27 pip  ⇒  can HEP HON BAO GIA 11,0 lan (A) / 10,4 lan (B)

Ke ca khi bao gia 0,00030 sai 3 lan, van con thieu 3,5 lan nua. Day la ly do
ket luan khong phu thuoc vao con so chua do duoc.

## Chan troi: hai duong cong khong bao gio cat nhau

`chi phi/R = spread/(1,5 x ATR)` giam theo ~1/sqrt(chan troi) khi ATR gian.
Nhung **tin hieu cung tan theo chan troi, va tan nhanh hon**:

    chan troi      1 nen (15m)        1 ngay (96)         1 tuan (480)
    A, lookback 96  r=-0,034 t=-10,7   r=+0,034 t=+1,10    r=+0,005 t=+0,07
    B, lookback 96  r=-0,036 t=-10,9   r=+0,006 t=+0,19    r=-0,094 t=-1,32

Chan troi duy nhat co tin hieu (1 nen) la chan troi phi dat nhat theo R. Chan
troi ma phi tra noi (>= 1 ngay) la chan troi **tin hieu bang 0**. Nua doi AR(1)
30-228 ngay khang dinh cung mot dieu: o thang do khai thac duoc, day la buoc
ngau nhien.

⚠️ **Chan troi TRUNG GIAN (1h, 4h) KHONG DUOC DO.** Hai chan troi tren chi
*kep* cau hoi; mot phep do o 4h la mot look moi va phai khai lai.

## Va day la loi giai thich day du cho ban ghi 70 thang ma chu may chi sang

Neu co che nen la **-0,347 R moi lenh**, thi mot he thong hoi quy tren AUDNZD
khong the co 70 thang lai lien tiep tu co che do. Grid recovery **khong phai
mot cai tien cua co che nay -- no la THU DUY NHAT tao ra duong cong von do**:
nhoi them khi dang sai bien nhieu cai lo nho thanh "thang" nho, day ti le thang
len, va don toan bo lo vao mot duoi trai khong duoc do. 70 thang la **thoi gian
truoc khi no lo**, khong phai bang chung co edge.

Dieu nay khop chinh xac voi so hoc DCA da noi voi chu may: ky vong **cong don
theo tung lan vao**, nen ba lan nhoi cho ba lan cai am, tren ba lan khoi luong.

## Thu KHONG do duoc

1. **Spread THAT cua AUDNZD.sc.** Khong co log tick nao tren may. Moi con so
   phi o day xuat phat tu mot bao gia; cach xu ly la cong bo **phi hoa von**
   (0,27 pip) de ket luan khong phu thuoc vao bao gia -- va no khong phu thuoc.
2. **AUDCAD va NZDCAD.** Phan con lai cua ho. Chung la hai o nua, khong phai
   mot huong moi; neu ai muon do thi phai khai lai. Du doan (ghi ra de co the
   sai): CAD crosses co bien dong cao hon mot chut nhung spread cung rong hon,
   nen ti so kho tot hon mot bac do lon.
3. **Chan troi 1h/4h** -- xem tren.
4. **Grid recovery** -- engine giu mot vi the, da khai tu dang ky goc.
5. **Arm swap** -- khai o ghi chu (b) nhung khong tieu, vi F2 ban truoc do.

## Code van giu lai, vi no dung va se con dung

`lnz` (z-score cua `ln(close)`, nhan qua, NaN chu khong phai 0 khi chua du cua
so) va `cross-reversion`, 4 test moi, tong **131 fd-strategy + 30 fd-indicators
+ 38 fd-core, 0 fail**. `ratz` va `ratio_reversion` **khong bi cham**, nen 36
receipt cua `agent/n2` tai lap duoc tung so.
