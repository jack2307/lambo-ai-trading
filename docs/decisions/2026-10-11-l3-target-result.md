# KẾT QUẢ — lớp L3: **mức giá KHÔNG phải câu trả lời, nhưng "target không theo stop" THÌ CÓ** — và dự báo biên độ của 1m vẫn thua ở cả ba lớp

**Ngày:** 2026-10-12. Nhánh `agent/l3-target`, cắt từ `29c8635`.
Đăng ký trước: `docs/decisions/2026-10-11-l3-target.md` (commit `fc5c558`,
**chỉ docs**; ghi chú thêm cùng ngày trong `7ca4828`).
Receipt: `receipts/l3-target/` — 2 tiền kiểm + 16 lần chạy + `TABLE-all-cells.txt`.

**Outcome: F1 BẮN, và F2 BẮN.** Cả hai nhánh của falsifier mà brief viết ra đều
bắn, nên kết luận lớn của brief áp dụng: **cả ba lớp stop đã đo xong.**

    F1 : L3 (target la MOT MUC GIA) KHONG danh bai L2 — dPF_r AM 8/8,
         ke ca arm co guards tren CA HAI cua so
    F2 : doi chung (c) — target la HANG SO khoang cach — KHONG chi "an bang"
         ma DANH BAI L3 that o 8/8 (dPF_r -0,0236 den -0,0732)
    => cai mua duoc la viec TACH TARGET KHOI STOP, khong phai muc gia.

Và thứ đáng giữ nhất của đợt này là cái **dương** nằm bên trong một trục bị bác:

    L3con(fc) - L2(fc)  = +0,0004 ... +0,0350 PF_r, DUONG 8/8
      (ca hai cua so, ca hai arm, ca hai co stop)
    khoang con thieu de qua cong = 1,200 - 0,8368 = +0,3632 PF_r
    => cai dung nhat do duoc tra 0,1% den 9,6% cua khoang can. Dau DUNG,
       do lon nho hon mot bac.

---

## 1. Việc, một câu

Dựng lớp **L3** — `Exits::Engine`, stop lấy từ **dự báo biên độ** của nến tới,
và target là **một MỨC GIÁ** (POC/VAH/VAL của một **time profile**) thay vì
`reward_risk × risk` — rồi so với **L2 cùng cỡ stop trung bình** và với **hai đối
chứng tách được hai giả thuyết**: mức **xáo trộn trong thời gian**, và target là
một **hằng số khoảng cách** cùng khoảng cách trung bình.

## 2. `Exits::` — L3 THẬT, khai bằng MÃ và ĐO

    crates/fd-strategy/src/range_stop_l3.rs
        fn exits(&self) -> Exits { Exits::Engine }

và target là một **giá**, không phải một tỉ số:

    targetSource = 1 : Intent::Enter { target: Some(close(i) +- d(i)), .. }
    targetSource = 0 : Intent::Enter { target: None, .. }
                       -> open_position (engine.rs:735-744) dan ra
                          `risk x reward_risk` = L2

Cơ chế **không có exit riêng nào**: `on_bar` trả `Intent::None` khi có vị thế,
nên stop / target / `max_hold_ms` **là của engine**.

**STOP ĐƯỢC THI HÀNH — đo, không giả định.** `--exit-mix` ở **112/112** dòng:

    STOP no o 39,97% den 72,32% so lenh    (vach khai truoc: >= 20%)
    range-sizing doc 63,7-65,2% (f=1,0) va 44,7-47,0% (f=2,0) — TRUNG dai
    doi lap: `stop-width` (Exits::Strategy) doc stop no 0-3 lan / 1.355 lenh

⇒ **F5 KHÔNG bắn.** Trục này không phải một phép đổi đơn vị.

**Và TARGET cũng nổ.** `vprofile-gold` cảnh báo rằng một "target theo mức" có thể
chỉ đóng **6,0–7,4%** vị thế (và khi đó nó không phải một target). Đo được ở đây:

    L3 muc  : TARGET no 19,25% den 30,16% so lenh   => KHONG phai mot target gia
    L2      : TARGET no 17,20% den 30,28%
    L3 hangso: TARGET no  7,42% den 13,56%          <= F4 BAN o 4 o (xem muc 6)

## 3. Cổng

**0 / 112 dòng, 0 / 64 ô. Không ô nào gần.**

    PF_r cao nhat ca truc L3      : 0,8368   (vach 1,200)
      — va no la mot o L2/atr, tuc mot DOI CHUNG, khong phai co che
    E cao nhat ca truc L3         : -0,083 R (vach +0,050R)
    PF_r cao nhat ca bang 112 dong: 0,8579   (dong tai tao (a) cua range-sizing)
    n                             : 4.515 den 7.497  (cong doi 40)

**Đếm tay, không đọc verdict của tool** (tool kiểm sàn `need 30`, cổng desk là
**40**): `n` nhỏ nhất cả bảng là **4.515** ⇒ **chân số lệnh không ràng buộc ở bất
kỳ ô nào**.

**Chân nào ràng buộc:** `Lbar` từ **0,4503** đến **0,8089 R** ⇒
**`Lbar ≥ 0,250R` ở 64/64 ô** ⇒ theo hằng đẳng thức `E = Lbar × (PF_r − 1)`
**chân expectancy là DƯ ở mọi ô**, và **chân PF là chân duy nhất ràng buộc**.
Hằng đẳng thức kiểm từng dòng: **phần dư lớn nhất 0,00009 R trên 112 dòng**.

**⚠️ F10 BẮN: 16/64 ô in `max_drawdown_pct > 100%` ⇒ 16 DÒNG ĐÃ CHÁY** (khuyết
điểm 14). Và cả 64 ô nằm trong dải **96,56% – 100,18%** với `ddUSD` từ
**9.716,52** đến **10.755,45 USD** quanh đúng `starting_equity_usd = 10.000` ⇒
**mọi dòng ở đây mất gần hết hoặc hết sổ.** Nên:

- **`PF_usd` và drawdown của trục này KHÔNG ĐỌC ĐƯỢC.**
- **`PF_r` và `E` đọc được** vì `r = points/risk` không đọc `lots`. Đó là lý do
  đăng ký §2 khai trước rằng con số quyết định là `ΔPF_r`.

**Carry:** cột `swap$` đọc **0 ở 112/112 dòng**, và config khai
`markets.xauduka.trading.swap_long_per_lot = 0.0` / `swap_short_per_lot = 0.0`
cùng `[trading] commission_per_lot = 0.0` ⇒ **`expectancy_net == expectancy`**
(`engine.rs:1345` ghim đúng tính chất đó ở chi phí 0). Luật "`PF_r` mù carry"
**không cắn trục này**, và đó là một phép đo chứ không phải một giả định.

## 4. Falsifier — cái nào bắn, ở đâu

| | bắn? | ở đâu |
|---|---|---|
| **F1** L3 thắng L2 cùng cỡ stop, hai cửa sổ, arm guards | **BẮN — ÂM 8/8** | §5 bảng P1 |
| **F2** đối chứng (c) hằng số ăn bằng L3 thật | **BẮN — nó ĂN HƠN, 8/8** | §5 bảng P3 |
| **F3** xáo trộn tái tạo ≥ 50% cú dịch | **BẮN ở 6/8 ô**, không bắn ở 2 ô (W1, f=1,0) | §6 |
| **F4** TARGET nổ dưới 10% | **BẮN ở 4/16 ô `L3con`** (f=1,0: 7,42–9,98%) | §6 |
| **F5** STOP nổ dưới 20% | **KHÔNG bắn** — đo 39,97–72,32% | §2 |
| **F6** đối chứng (a) không tái tạo được | **KHÔNG bắn** — tái tạo **khớp hoàn toàn** | §7 |
| **F7** 32 dòng trùng lặp không y hệt | **KHÔNG bắn** — **16/16 nhóm PASS** | §7 |
| **F8** C1 trượt hoặc một probe không trượt | **KHÔNG bắn** — sau khi vá **phép đo**, không phải vạch | §8 |
| **F9** sống ở một cửa sổ | **KHÔNG bắn** — P1/P2/P3 **đồng dấu cả hai cửa sổ** | §5 |
| **F10** `_pct > 100%` | **BẮN ở 16/64 ô** | §3 |

## 5. BỐN PHÉP SO QUYẾT ĐỊNH — khai TRƯỚC khi chạy ô nào (đăng ký §2)

Luật so đã khai: **`stopAtr` CỐ ĐỊNH và cỡ stop trung bình ĐÃ KHỚP** ở mọi phép
so; **không một phép so nào đặt hai ô khác cỡ stop cạnh nhau.** Kiểm chéo bằng
thước của chính engine (`realised stop: median`): hai ô được so lệch nhau
**≤ 0,4%** (ví dụ W1 guards-ON f=1,0: L2/fc **1,094 ATR** vs L3/fc **1,090 ATR**;
L2/atr **1,098** vs L3/atr **1,099**). **Không** dùng thước
`risk_usd = |swap$|/|Δtotal_r|` (ADDENDUM-10 §III: nó sai 2,22 lần).

    cs  arm  f     L2(fc)  L3tru(fc) L3tru(atr) L3con(fc) L3shf(fc) |      P1       P2       P3       P4
    W1  off  1,0   0,6687    0,6237     0,6379    0,6969    0,6637  | -0,0450  -0,0142  -0,0732  -0,0400
    W1  off  2,0   0,7590    0,7159     0,7318    0,7796    0,7328  | -0,0431  -0,0159  -0,0637  -0,0169
    W1  ON   1,0   0,6682    0,6352     0,6496    0,6935    0,6724  | -0,0330  -0,0144  -0,0583  -0,0372
    W1  ON   2,0   0,7593    0,7192     0,7363    0,7769    0,7322  | -0,0401  -0,0171  -0,0577  -0,0130
    W2  off  1,0   0,7411    0,7329     0,7488    0,7761    0,7303  | -0,0082  -0,0159  -0,0432  +0,0026
    W2  off  2,0   0,8205    0,8000     0,8105    0,8236    0,8073  | -0,0205  -0,0105  -0,0236  -0,0073
    W2  ON   1,0   0,7395    0,7231     0,7402    0,7693    0,7252  | -0,0164  -0,0171  -0,0462  -0,0021
    W2  ON   2,0   0,8109    0,7855     0,7973    0,8113    0,7806  | -0,0254  -0,0118  -0,0258  +0,0049

    P1 = L3tru(fc) - L2(fc)      P2 = L3tru(fc) - L3tru(atr)
    P3 = L3tru(fc) - L3con(fc)   P4 = L3tru(fc) - L3shf(fc)

### P1 — **F1 BẮN: mức giá không cứu được L2.** `ΔPF_r` ÂM 8/8

Âm ở **cả hai arm**, **cả hai cửa sổ**, **cả hai cỡ stop**. Ở arm có guards —
**arm duy nhất chủ cho phép** — nó đọc **−0,0330 / −0,0401 (W1)** và
**−0,0164 / −0,0254 (W2)**. Vì cái âm **đồng dấu ở cả hai cửa sổ** nên nó
**không** phải hiện vật cửa sổ; độ lớn thì có (W1 ~2–3× W2).

### P2 — dự báo biên độ **vẫn thua** stop ATR phẳng, ngay cả khi target đã được tha ra. ÂM 8/8

**−0,0105 đến −0,0171**, 8/8. Và trong hình target **tốt nhất** (hằng số khoảng
cách) nó cũng âm 8/8: `L3con(fc) − L3con(atr)` = **−0,0040 đến −0,0229**.

⇒ Đây là phát biểu cuối về tín hiệu biên độ 1m: **ở cả ba lớp — L1 (mẫu số),
L2 (target theo stop), L3 (target là mức giá) — nó không tiêu được.** Và ở L3
nó không còn cái cớ "target đi theo stop" nữa.

### P3 — **F2 BẮN, và nó bắn theo chiều NÓI ĐƯỢC NHIỀU NHẤT**

Đối chứng (c) — target là **hằng số khoảng cách** cùng khoảng cách trung bình,
tức **bỏ phần "mức giá", giữ phần "không theo stop"** — **ăn HƠN** L3 thật ở
**8/8** ô (`ΔPF_r` −0,0236 đến −0,0732). Và quan trọng hơn:

    L3con(fc) - L2(fc) :  +0,0282  +0,0206  +0,0253  +0,0176   (W1)
                          +0,0350  +0,0031  +0,0298  +0,0004   (W2)
                          DUONG 8/8
    L3con(atr) - L2(atr): +0,0352  +0,0204  +0,0342  +0,0151   (W1)
                          +0,0253  -0,0073  +0,0156  -0,0074   (W2)
                          duong 6/8 — W2 f=2,0 LAT DAU

⇒ **Cái mua được là việc TÁCH TARGET KHỎI STOP, không phải mức giá.** Đúng câu
brief viết: *"nếu (c) ăn bằng L3 thật thì cái mua được là việc tách target khỏi
stop, không phải mức giá — và đó là một kết luận sạch, khác hẳn."* Nó ăn **hơn**.

Và cái dương đó **sạch hơn ở nhánh stop BIẾN THIÊN** (8/8) so với nhánh stop
phẳng (6/8) — hợp cơ học: khi stop biến thiên thì một target `rr × risk` biến
thiên theo nó, nên việc tách ra **đáng hơn**.

### P4 — xáo trộn: xem §6

## 6. F3 và F4 — hai chỗ kết quả này TỰ GIỚI HẠN

**F3 — đối chứng (b) tái tạo được bao nhiêu phần cú dịch `L3tru − L2`:**

    cs  arm  f     L3tru-L2   L3shf-L2    ti le
    W1  off  1,0   -0,0450    -0,0050    +11,1%     <- F3 KHONG ban
    W1  off  2,0   -0,0431    -0,0262    +60,8%
    W1  ON   1,0   -0,0330    +0,0042    -12,7%     <- F3 KHONG ban
    W1  ON   2,0   -0,0401    -0,0271    +67,6%
    W2  off  1,0   -0,0082    -0,0108   +131,7%
    W2  off  2,0   -0,0205    -0,0132    +64,4%
    W2  ON   1,0   -0,0164    -0,0143    +87,2%
    W2  ON   2,0   -0,0254    -0,0303   +119,3%

**Bắn ở 6/8 ô** (≥50%) ⇒ ở phần lớn bảng, cú dịch **tái tạo được bởi một mức xáo
trộn cùng phân phối khoảng cách** ⇒ cái làm `PF_r` xấu đi là **việc target là một
khoảng cách biến thiên**, không phải **thời điểm** của mức. Nhưng ở **W1, f=1,0,
cả hai arm** nó đọc **11,1%** và **−12,7%** ⇒ ở đó **thời điểm của mức thật CÓ
nghĩa, và nghĩa đó là XẤU HƠN** — cùng cái lật mà `range-sizing` §5 gặp ở
`stopAtr 2,0`. **Hai cách đọc, cả hai đóng trục**, và không cách nào cứu nó.

**F4 — BẮN Ở 4 Ô, và nó bắn vào đúng cái DƯƠNG của §5.** Đối chứng (c) ở
`stopAtr = 1,0` nổ TARGET **7,42% / 8,07% / 9,44% / 9,98%** — **dưới vạch 10%
tôi tự khai**. Lý do: hằng số (c) là **trung bình** của một phân phối lệch phải
(W1 **8,904 USD** so với trung vị **5,640**), và ở `stopAtr 1,0` stop trung bình
chỉ **1,6897 USD** ⇒ target cách **~5,3R**. ⇒ **Ở 4 ô đó, "target hằng số" trên
thực tế gần bằng "KHÔNG CÓ TARGET, chỉ có stop + trần 4h".**

**Đây là cách đọc trung thực nhất của cái dương, và nó MẠNH HƠN cách đọc kia:**

    L2 (target 1,8R) -> L3con (target ~5,3R, no 7,4-10,0%)
      f=1,0: +0,0282 +0,0253 (W1)  +0,0350 +0,0298 (W2)   <- lon nhat
      f=2,0: +0,0206 +0,0176 (W1)  +0,0031 +0,0004 (W2)   <- gan 0 o W2

Cái dương **lớn nhất ở đúng chỗ target gần như không nổ**. Nên phát biểu đúng
không phải "tách target khỏi stop trả tiền" mà là:

> **Cái target `reward_risk = 1,8` ĐANG LẤY TIỀN, và đây là lần đầu hồ sơ ĐO
> được nó lấy bao nhiêu: +0,0176 đến +0,0350 `PF_r` ở `stopAtr = 1,0`, dương
> trên cả hai cửa sổ và cả hai arm, khi thay nó bằng một target xa ~5,3R.**

Nó nối đúng vào **ADDENDUM-11 §IV.B**: `Exits::Engine` **không nói được "có
stop, không target"** (khuyết điểm 5, đã có test trong cây), `reward_risk` nằm ở
`[trading]` **không có override theo market**, nên **mọi ô `Exits::Engine` của hồ
sơ đều bị target 1,8R chặn**. `L3con` ở `stopAtr 1,0` là thứ **gần nhất** cây này
từng biểu đạt được "một stop và không target" — và nó đọc **dương 4/4**.

**Nhưng nó KHÔNG đưa ai qua cổng.** `PF_r` cao nhất của cả họ `L3con` là
**0,8295**, còn thiếu **+0,3705** so với vạch 1,200 — tức cái đo được trả
**0,1% đến 9,6%** của khoảng cần.

## 7. ĐỐI CHỨNG DƯƠNG — bốn cái, và cả bốn PASS

**(a) L2 cùng cỡ stop — đối chứng BIẾT TRƯỚC ĐÁP ÁN. TÁI TẠO KHỚP HOÀN TOÀN.**
Chạy lại **chính** cấu hình `agent/range-sizing` (cơ chế `range-stop`, TOML của
nó, file companion `XAURFC1M` dựng lại bằng **script của nó, không sửa một chữ**):

    doi chung (a), arm guards-ON, nhan atr/f1.0 vs fc/f1.0
      W1 : PF_r 0,7115  vs  0,7030  => dPF_r -0,0085   (ho so: -0,0085) OK
      W2 : PF_r 0,7873  vs  0,7667  => dPF_r -0,0206   (ho so: -0,0206) OK
      n  : 9.677 / 9.691 (W1) va 11.818 / 11.856 (W2)  — KHOP TUYET DOI
    va ca 8 gia tri PF_r cua arm guards-off cung khop: 0,7221 / 0,7211 /
      0,8254 / 0,7877 (W1) · 0,7853 / 0,7713 / 0,8579 / 0,8470 (W2)

Và tiền kiểm của nó tái tạo **từng con số**: `ΔR²_oos` **+0,006178 / +0,009181**,
mặt nạ **338.423 / 378.749 = 89,3528%**, **37.779** nến tín hiệu, và **cả sáu**
hằng số khớp cỡ tới **6 chữ số** (W1 fc1m **1,289088**, W2 **1,209946**).
⇒ **F6 KHÔNG bắn. Dụng cụ nhìn thấy được.**

> ⚠️ Và nó là một ngoại lệ đáng ghi của ADDENDUM-8 §II (*"hồ sơ không chạy lại
> về đúng số của chính nó"*, 65,3%): **ở đây nó chạy lại về đúng số của chính nó,
> 16/16 dòng.** Khác biệt duy nhất so với 51 dòng không tái tạo được: `range-sizing`
> **ghim cả hai đầu cửa sổ trên dòng lệnh** và **in số bar**. Đó chính là thứ
> ADDENDUM-11 §VII nói, và đây là bằng chứng ngược chiều cho nó.

**(d) 32 dòng trùng lặp CÓ Ý.** Nhãn `L2/*` **không đọc `cmp.high`**, nên chúng
phải in số y hệt ở cả ba file companion: **16/16 nhóm PASS** (`n` và `PF_r` trùng
tới chữ số in ra; `sized down` cũng trùng: 98,6% / 81,6% / 97,6% / 76,0%).
⇒ **F7 KHÔNG bắn.** Việc nạp một file companion **không rò** vào nhãn không đọc nó.

**(e) Parity số học của chính MỨC — Rust ↔ Python, TỪNG BIT.**
`crates/fd-engine/tests/profile_parity.rs`, 3 test PASS:

    fixture 600 nen LCG tat dinh (chi so nguyen + chia 100.0)
    bucket = bucket_size_price(2,5 ; 4,0) = 0,625
    POC 1030,8474999999999   VAH 1035,5349999999999   VAL 1008,66
    ban Rust `fd_engine::price_levels::activity_profile` ra DUNG ba so do
    va `profile.measure == "TIME_AT_PRICE"`

Không có bước này thì câu *"mức của tôi là `activity_profile` của desk"* là một
**lời khai**. Giờ nó là một phép đo.

**(f) Cỡ stop khớp, đo bằng thước của engine** — xem §5.

## 8. NHÂN QUẢ — C1 + **NĂM** probe, và một khuyết điểm của PHÉP ĐO

    tang DU BAO (muon nguyen tu range-sizing, chay lai nguyen ban):
      C1 PASS : 14 dac trung + pred + m, x 5 moc cat, 0 gia tri doi (TUNG BIT)
      PROBE_peek_m16   TRUOT C1 o 5/5 moc
      PROBE_y          TRUOT C1 o 5/5 moc
      PROBE_fullfit    TRUOT C1 o 5/5 moc   <- probe duy nhat test TANG KHOP
      C3 PASS, NULL!=0 PASS, XAO-TRON PASS
    tang MUC (moi):
      C1 PASS : chuoi d, x 5 moc cat, 0 gia tri doi (TUNG BIT)
      PROBE_profile_i1    TRUOT C1 o 5/5 moc  (cua so profile GOM nen i+1)
      PROBE_profile_ahead TRUOT C1 o 5/5 moc  (cua so ket thuc 48 nen SAU i)

⇒ **F8 KHÔNG bắn.** Năm probe cố ý nhìn trước, **cả năm trượt 5/5**.

**Nhưng hai lần đo đầu KHÔNG đạt, và nguyên nhân là PHÉP ĐO — đây là khuyết điểm
dùng được cho mọi agent sau:**

> **C1 bằng cách CẮT CHUỖI có lực gần bằng 0 với một cái rò MỘT NẾN trong một
> chuỗi MỨC.** `PROBE_profile_i1` đọc **2/5** mốc với mốc cắt ở phân vị trơn, rồi
> **3/5** khi cắt ngay sau một nến tín hiệu. Lý do: một cái rò một nến chỉ hiện
> ra ở nến `i = mốc − 1` (mọi nến `i ≤ mốc − 2` có cửa sổ nằm **trọn** trong lát
> đã cắt nên hai chuỗi bằng nhau **một cách hợp pháp**), và chỉ khi đúng nến đó
> là **nến tín hiệu CÓ MỘT MỨC** — 41,1% nến tín hiệu không có mức nào thoả nên
> `d` của chúng là NaN ở **cả hai** chuỗi. Nến tín hiệu ~10% nến ⇒ mốc trơn cho
> kỳ vọng **~0,5/5**.
> ⇒ **Vá dụng cụ, không hạ vạch:** mốc cắt đặt **ngay sau một nến tín hiệu có
> mức**, cộng một probe thứ hai (`PROBE_profile_ahead`) **không phụ thuộc chỗ
> cắt**. Vạch giữ nguyên **5/5**.
> **Một "C2 PASS" trên một chuỗi mức với mốc cắt trơn KHÔNG NÓI GÌ CẢ.**

Và khi C1 **trượt ở chữ số cuối**: tôi mượn nguyên bản vá của `range-sizing`
(`predict_rows` cộng từng phần tử, vì **GEMM của BLAS đổi thứ tự cộng khi số dòng
của lô đổi**) và **không** hạ chuẩn xuống "trong 1 ulp".

## 9. MỨC — nó là TIME PROFILE, và nó không phải volume

Hằng số **lấy từ MÃ**, không quét: `buckets_per_atr = 4,0`
(`fd-api/src/levels.rs:154`), `value_area_pct = 0,70` (`:158`),
`profile_days = 5` (`:126`) ⇒ cửa sổ **480 nến 15m NGAY TRƯỚC nến `i`** (nến `i`
LOẠI), bucket `= ATR14(i)/4,0` (`price_levels.rs:419`).

`price_levels.rs:300` **tự khai** `measure = "TIME_AT_PRICE"`, và
`activity_profile` cộng **1,0 vào MỌI bucket mà dải low→high của nến chạm** —
thời-gian-tại-giá. **Trong báo cáo này nó không một lần nào được gọi là volume
profile.** Và `columns=["time","open","high","low","close"]` ở **mọi** lần đọc
parquet ⇒ cột `volume` **không được mở một lần nào** (`XAUDUKA-1m.volume` null
trên **5.635.777/5.635.777** dòng; bản 15m mang một số 0 **chế ra**).

**Mức đo được, trên 37.745 nến tín hiệu:**

    muc nao duoc chon : POC 17,4%   VAH 21,8%   VAL 19,8%   KHONG CO MUC 41,1%
    mat na CHUNG (m co so & d co so): 20.340 / 37.779 nen tin hieu = 53,84%
    khoang cach d : W1 p10 1,224  p50 5,640  p90 19,978  mean  8,904 USD
                    W2 p10 1,433  p50 7,471  p90 30,875  mean 13,919 USD
                    d/ATR14 p50 : 3,995 (W1)  3,536 (W2)
    target/stop THUC TE (= reward_risk cua L3, KHONG phai hang so):
      stopAtr 1,0 : p10 0,825  p50 4,058  p90 13,607   (W1)
      stopAtr 2,0 : p10 0,412  p50 2,029  p90  6,804   (W1)
      L2 dung 1,800 o MOI lenh

**Và đây là cơ học của P1, đọc được từ `--exit-mix`** (W1 guards-off, nhánh fc):

    f=1,0 : L2     STOP 65,91  TARGET 27,55  TIMEOUT  6,54   hold  84,7 min
            L3 muc STOP 66,05  TARGET 20,41  TIMEOUT 13,54   hold 101,4 min
    f=2,0 : L2     STOP 46,59  TARGET 17,45  TIMEOUT 35,94   hold 206,4 min
            L3 muc STOP 42,07  TARGET 26,95  TIMEOUT 30,97   hold 176,2 min

⇒ **Ở `f=1,0` mức nằm XA hơn 1,8R (p50 4,06R) nên nó đổi TARGET thành TIMEOUT**
(−7,1 điểm% TARGET, +7,0 TIMEOUT) — **đúng cái bệnh L2 mắc**, chỉ vì lý do khác.
**Ở `f=2,0` mức nằm ĐÚNG TẦM 1,8R (p50 2,03R) nên nó làm điều ngược lại**: TARGET
**+9,5 điểm%**, TIMEOUT **−5,0**, STOP **−4,5**, hold ngắn hơn 30 phút.

> **Và `PF_r` XẤU ĐI ở CẢ HAI.** Ở `f=2,0` cơ chế **làm đúng việc nó được thiết
> kế để làm** — nhiều target hơn, ít hết giờ hơn, ít bị quét hơn — và vẫn mất
> **−0,0431 PF_r**. Đó là con số nói nhiều nhất trong trục: **cái sai không phải
> "target không vào kịp". Những lệnh chạm mức là những lệnh ĐÁNG LẼ chạy xa
> hơn.** Một mức time-profile ở phía trước giá là một chỗ **giá dừng lại** — và
> bán ở đó là bán đúng cái đuôi phải trả cho một sổ có 60–66% lệnh chết ở stop.

## 10. `sized_down_by_guard` — đọc từ FIELD, mọi ô

`hypotheses.rs:752`; `guards: nothing acted` **nghĩa là** `sized_down == 0`,
**không phải** chuỗi thiếu (ADDENDUM-11 §VI). Đo:

    arm guards-ON : cap_lots thu nho 72,0% den 98,7% so lenh
    arm guards-off: `null` — KHONG PHAI 0: khong co dong guards nao de doc

⇒ **mọi `ΔPF_usd` ở đây có một phần là phân bố cỡ lot dịch**, và đó là lý do
đăng ký §2 khai trước rằng con số quyết định là `ΔPF_r`. **Cả hai chân cổng mù
với `cap_lots`.**

**Khe `PF_usd − PF_r` đo trên 64 ô phân biệt của tôi — và nó BÁC một phát biểu
của `range-sizing`:**

    PF_usd CAO hon PF_r : 15/64 o = 23,4%
      W1 : 15/32 (46,9%)      W2 : 0/32 (0,0%)
    |khe| : 0,0012 .. 0,1136
    o co hai chan nam hai phia vach 1,200 : 0/64

Và **trong W1, chiều của khe do HÌNH TARGET quyết định** — cùng cửa sổ, cùng
instrument, cùng arm, cùng tham số, chỉ đổi hình target:

    W1  L2    : USD cao hon 4/8
    W1  L3tru : USD cao hon 0/8
    W1  L3shf : USD cao hon 8/8
    W1  L3con : USD cao hon 3/8
    W2  : 0/8 o CA BON hinh

⇒ `range-sizing` §11 kết luận khe này là **"một HẰNG SỐ CỦA CỬA SỔ"** (W1 100% /
W2 0%). **Số của tôi bác nó:** 16 dòng tái tạo của nó **đọc lại đúng 8/8 và 0/8**,
nhưng trên **cùng W1** bốn hình target cho **0/8, 3/8, 4/8, 8/8**. Chiều của khe
**không phải hằng số cửa sổ** — nó là **CỬA SỔ × HÌNH THOÁT**, và **hình thoát
một mình lật được dấu trong một cửa sổ**. Đây là phép đo thứ **mười** cho
ADDENDUM-11 §V.2, và nó nói: **đừng trích tỉ lệ của ai, kể cả của chính
`range-sizing`.**

## 11. Khuyết điểm 16 — δ là một phần BIẾN THIÊN của R, và nó thiên vị CÓ LỢI

Số **từ MÃ**, không từ phép khớp (`apply_costs` `engine.rs:902-912` + `risk`
`engine.rs:717-719` cho **`d risk/d s = +1/2` chính xác**):

    delta = spread / 2 = 0,28 / 2 = 0,1400 diem gia

**Và đẳng thức miễn phí, đúng trên 112/112 dòng vì nó là một đẳng thức:**
`δ/R = (cost/R engine in ra) ÷ 2` — ví dụ `cost/R 19,57%` ⇒ `δ/R 9,79%`.
(Phụ lục 7 khớp regression ra 0,138, lệch 1,4%; **phép khớp không phân giải được
đại lượng này**.)

Độ lớn, trên đúng nến giao dịch được:

    stopAtr 1,0 : delta = 18,75% cua R o p10(d) ... 4,66% o p90(d)   (W1)
                          15,26%               ... 2,36%             (W2)
    stopAtr 2,0 :          9,38%               ... 2,33%             (W1)
                           7,63%               ... 1,18%             (W2)

⇒ biến thiên **4,0 lần (W1) / 6,5 lần (W2)** của một thành phần **không khai
báo** của mẫu số R, đi **cùng hướng** với dự báo (stop hẹp ⇒ δ chiếm phần to
hơn) ⇒ nó **thiên vị `ΔPF_r` theo hướng CÓ LỢI cho nhánh stop hẹp**. Cái âm đo
được **không** giải thích được bằng nó (δ đẩy ngược chiều), nhưng nó là một
thành phần không sạch của mỗi con số R ở đây. **Đếm, không sửa.**

## 12. Chi phí/R — bốn thứ phải nói (ADDENDUM-9 §II)

**cửa sổ + thống kê + cỡ stop + spread ĐO ĐƯỢC hay GIẢ ĐỊNH:**

    nguon spread : 0,28 = `markets.xauduka.spread` trong CAU HINH, KHONG do
                   boi job nay (ADDENDUM-11 §II treo quy uoc mot-chieu vs
                   vong-tron cho job `gold-reprice`; neu 0,28 von la mot so
                   MOT CHIEU thi moi con so duoi day la mot SAN)
    thong ke     : trung vi (p50) cua d tren nen giao dich duoc
    W1 (2010-06-01 -> 2018-06-01, 190.889 bar):
      stopAtr 1,0 => 20,53% cua R (stop p50 1,364 USD)
      stopAtr 2,0 => 10,27% cua R (stop p50 2,728 USD)
    W2 (2018-06-01 -> 2026-06-01, 187.860 bar):
      stopAtr 1,0 => 13,95% cua R (stop p50 2,007 USD)
      stopAtr 2,0 =>  6,97% cua R (stop p50 4,014 USD)

Và dòng `cost-matched null` mà **engine tự in**, trên cùng các ô: **19,10–19,98%**
(W1, f=1,0), **9,43–9,89%** (W1, f=2,0), **12,90–13,12%** (W2, f=1,0),
**6,43–6,53%** (W2, f=2,0). Khác số Python vì engine lấy **cỡ stop danh nghĩa
của null** chứ không lấy trung vị thực tế — **hai thước, báo cả hai.**

Con số **4,04%** lưu hành là trung vị của **cửa sổ 2 năm gần nhất** và **sai cửa
sổ** cho mọi thứ ở đây.

## 13. Sổ đa phép thử — khai vs xem

    khai (dang ky §7, truoc dong code dau tien):
      64 o cong moi   /  112 dong in ra
        L3: 3 nguon muc x 2 stopSource x 2 stopAtr x 2 cua so x 2 arm = 48 o
        L2: 2 stopSource x 2 stopAtr x 2 cua so x 2 arm               = 16 o
        + 32 dong TRUNG LAP CO Y (nhan L2 x 3 file) = doi chung (d)
        + 16 dong TAI TAO doi chung (a), 0 o cong moi (dap an DA CONG BO)

    xem:
      64 o cong phan biet  /  112 dong in ra  /  16 dong tai tao
      => KHOP TUNG CON SO. KHONG THEM MOT O NAO sau khi thay so.

    tien kiem (0 o cong): doi chung (e) parity, C1 x 5 moc, 5 probe x 5 moc,
      C3, NULL!=0, XAO-TRON, phan phoi muc, phan phoi d, 2 hang so khop co,
      2 hang so (c), ti le target-sau-entry, 8 so chi phi/R, 8 so delta/R.
      => tieu 0 o cong.

    Khong co lan chay nao bi huy. Khong co o nao chay hai lan voi tham so khac.

**Phân vị KHÔNG công bố** (ADDENDUM-8 §VI: `SURVIVES` phụ thuộc **chỗ ngồi trong
file TOML**, null p95 1,297 vs 2,036 trên cùng một phương pháp). `seeds = 20` chỉ
để dòng `null p50` tồn tại. **Cổng là cổng.**

## 14. Thứ KHÔNG đo được, và vì sao

- **`PF_usd` và drawdown.** 16/64 ô in `_pct > 100%`, cả 64 ở 96,56–100,18% với
  `ddUSD` quanh `starting_equity_usd = 10.000` ⇒ **sổ đã cháy hoặc gần cháy ở
  mọi ô**. Không đọc. **Không in một số 0 thay cho chúng.**
- **`wrong_side_stop`.** `grep -rn wrong_side_stop crates/` → **0 kết quả** trên
  cây này ⇒ **không có bộ đếm** ⇒ **không đo được**. Và phiên bản target của nó
  (*target nằm sau entry sau khi nến `i+1` gap*) **đo được ở Python chứ không ở
  engine**: **1/8.811 (0,0113%) ở W1** và **5/11.529 (0,0434%) ở W2** nến tín
  hiệu. Nhỏ, nhưng là một số **đếm được** chứ không phải một giả định.
- **Trần `max_hold_ms = 4h` = 16 nến.** **KHÔNG nới** — nó được **khai**, đúng
  như `range-sizing`, để phép so là phép so. Và vì cái dương của §6 đi qua
  TIMEOUT, **một phần dấu của nó có thể là thuộc tính của đúng cái trần đó.**
  Không đo được ở đây. (`agent/hold-cap` có `--max-hold`; một job sau cherry-pick
  nó **nên** đo `L3con` ở trần cao hơn — xem §16.)
- **`reward_risk` khác 1,8.** `reward_risk` ở `[trading]` **không có override
  theo market** (ADDENDUM-11 §IV.B) nên mọi nhãn L2 ở đây là L2 **ở đúng 1,8**.
  Kết quả P1/P3 là kết quả **so với 1,8**, không phải so với "mọi L2".
- **Filter `flat 16:30-18:15`.** `range-stop-l3` chạy **`filters = []`** ⇒ nó
  **KHÔNG** miễn nhiễm với dấu giá nhà cung cấp quanh mốc nghỉ 17:00 NY
  (ADDENDUM-10 §IV: `open` của nến mở lại là **đáy nến ở 19,02%** nến `XAUDUKA`).
  **Kiểm, và câu trả lời là: không có filter đó.** Nó ảnh hưởng cả tám nhãn như
  nhau nên khó giải thích một `ΔPF_r`, nhưng nó ở trong **mức** của mọi `PF_r`.
- **Một luật vào lệnh, không phải mọi luật.** Kết luận là về `donchian 20` trên
  `xauduka:15m`. Một luật vào lệnh khác có phân bố nến tín hiệu khác.
- **Một cửa sổ profile, không phải mọi cửa sổ.** 480 nến, 4 bucket/ATR, value
  area 70% — **ba hằng số lấy từ mã, không quét**. Một cửa sổ profile khác có thể
  cho mức khác. **Không quét nó** là một lựa chọn chống đa phép thử, không phải
  một phép đo.
- **46,2% nến tín hiệu bị từ chối** (41,1% không có mức + phần mất do mặt nạ `m`).
  Kết luận là về **tập nến có một mức**, không phải mọi nến phá kênh.

## 15. Thứ một người đọc có thể hiểu sai

- **Không** nói "mức time-profile không có thông tin". Nói: **nó có hướng, và
  hướng đó lấy tiền** — bán ở mức là bán đúng cái đuôi phải trên một sổ có 60–66%
  lệnh chết ở stop, và ở `f=2,0` nó **làm tăng TARGET 9,5 điểm%** mà vẫn mất
  0,0431 `PF_r`.
- **Không** nói "L3con là một cơ chế". `PF_r` cao nhất của họ đó là **0,8295**,
  thua cổng **0,37**. Nó là một **phép đo về cái target 1,8R**, không phải một
  ứng viên.
- **Không** đọc `PF_usd` của bất kỳ dòng nào ở đây.
- **Không** nói "F3 không bắn" là một kết quả dương: ở 2 ô nó không bắn **vì bản
  thật hỏng NHIỀU HƠN bản xáo trộn**, không vì bản thật tốt hơn.
- **Không** nói gì về volume. Cột đó không được mở một lần nào.
- **Không** trích khe `PF_usd − PF_r` của tôi như một tỉ lệ (23,4%). §10 là một
  lý do để **không** trích tỉ lệ, không phải một tỉ lệ mới để trích.

## 16. Cái ĐÓNG, và cái MỞ

**ĐÓNG — và đây là kết luận lớn, đúng như brief viết:**

    L1 (mau so, self-managed) : noi/thu stop doi 0 tin hieu, 0 lenh, 0 USD
    L2 (target = rr x risk)   : dPF_r AM 8/8  (range-sizing)
    L3 (target = MOT MUC GIA) : dPF_r AM 8/8  (day)
    va o L3, voi target DA TACH KHOI STOP, du bao VAN thua stop ATR phang:
      P2 AM 8/8, va trong hinh target tot nhat cung AM 8/8

⇒ **Tín hiệu biên độ của 1m — `t` phần dư +28,303 (W1) / +38,060 (W2), `ΔR²_oos`
dương 4/4 fold, cái `t` lớn nhất hồ sơ từng đo trên một thứ nhân quả — KHÔNG
TIÊU ĐƯỢC Ở BẤT KỲ LỚP NÀO.** Cộng với kết quả chiều (0/8 đặc trưng, `|t| < 1`)
và kết quả L2, **họ "độ phân giải mịn hơn" ĐÓNG HOÀN TOÀN**, và desk **thôi hỏi
về dữ liệu tick/1m/5m cho mọi lớp cơ chế ở 15m.**

**Sẽ KHÔNG mở lại bằng:** một nguồn mức khác, một cửa sổ profile khác, thêm đặc
trưng 1m, 5m hay tick, hay một mô hình dự báo biên độ tốt hơn. Vấn đề **không
phải** chất lượng dự báo — nó đọc được, tái tạo được, và tiền kiểm ở đây xác nhận
lại nó. Vấn đề là **ở mọi lớp, cái mà dự báo dự báo chính là cái chuẩn hoá mẫu số
R hoặc cái định target**, nên một dự báo đúng làm hai phía của tỉ số đi cùng
nhau.

**MỞ — một thứ, và nó KHÔNG phải về độ phân giải:**

> **Cái target `reward_risk = 1,8` lấy +0,0176 đến +0,0350 `PF_r`** ở `stopAtr`
> 1,0, dương **4/4** (hai cửa sổ × hai arm), đo bằng cách thay nó bằng một target
> xa ~5,3R (nổ 7,4–10,0%). Đây là lần đầu hồ sơ **đo** được cái giá của
> `reward_risk` — ADDENDUM-11 §IV.B chỉ nói được rằng nó **chặn** mọi ô
> `Exits::Engine`.
>
> **Việc đáng làm tiếp, và nó là một QUYẾT ĐỊNH CỦA CHỦ, không phải của tôi:**
> cherry-pick `--max-hold` của `agent/hold-cap` (HEAD `18ae836`) và đo
> `L3con`/`L3tru` ở trần **cao hơn 4h**. Lý do: cái dương đo được **đi qua
> TIMEOUT** (L3con ở f=1,0 có TIMEOUT 19,3–19,6% so với 5,2–6,5% của L2), nên
> **một phần của nó là thuộc tính của trần**, và ADDENDUM-11 §III.1 đã đo rằng
> **nhấc trần làm TIMEOUT → STOP ở 5/6 cặp cửa sổ** — tức nó có thể xoá đúng cái
> dương này. **Tôi KHÔNG tự nới trần** (đăng ký §12 khai là không), và tôi
> **không** đề xuất nó như một ứng viên: khoảng thiếu là **+0,37 `PF_r`** còn cái
> đo được là **+0,035**, tức **9,6%** của khoảng cần ở ô tốt nhất và **0,1%** ở ô
> tệ nhất.

**Và một thứ cho mọi agent sau, không phải về vàng:** **C1 bằng cách cắt chuỗi có
lực gần bằng 0 với một cái rò MỘT NẾN trong một chuỗi MỨC** (§8). Nếu một job sau
báo "C2 PASS" trên một chuỗi mức/profile với mốc cắt ở phân vị trơn, **con số đó
không nói gì cả** — phải đặt mốc ngay sau một nến mà cơ chế SẼ hành động và nến
đó phải CÓ một giá trị.
