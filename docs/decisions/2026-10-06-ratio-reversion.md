# Đăng ký trước — 06/10/2026: giá trị tương đối vàng/bạc (tỉ số XAU/XAG hồi quy)

Agent n2, nhánh `agent/n2`, worktree `/e/rust/fd-b2`.
Brief: `AGENT-BRIEF-2026-10-06-B.md`. Commit này **chỉ có tài liệu**, trước
dòng code đầu tiên của cơ chế.

## 1. Giả thuyết

Tỉ số XAU/XAG (cùng feed Dukascopy, cùng đồng hồ 15m) hồi quy về trung bình
động của chính nó, nên khi log-tỉ số lệch `zEnter` độ lệch chuẩn khỏi trung
bình `lookback` bar, tỉ số sẽ đi ngược lại — và một lệnh MỘT CHÂN đặt theo
chiều đó qua được cổng desk trên **cả hai** cửa sổ.

## 2. Khai trước: đây KHÔNG phải pairs trade, và vì sao

Engine giao dịch **một** instrument (`BarContext` mang một slice `bars`;
`Intent::Enter` có một `side`). Một pairs trade hai chân không biểu đạt được
mà không viết lại engine, nên **tôi không đo pairs trade**.

Cái tôi đo: **tín hiệu tính từ tỉ số, vào lệnh một chân**. Tỉ số ở cực trên →
short chân chính; cực dưới → long chân chính. Hệ quả phải nói thẳng:

- Một chân **vẫn phơi nhiễm drift của chân đó.** Lợi thế "một tỉ số không có
  drift instrument để vay" chỉ đạt được **một phần** — nó áp cho *tín hiệu*,
  không áp cho *lệnh*.
- Vì vậy tôi đo **cả hai chân**: chân vàng (`--market=xauduka
  --companion=XAGDUKA`) và chân bạc (`--market=xagduka --companion=XAUDUKA`).
  Cơ chế định nghĩa `ratio = close(chân chính) / close(companion)`, nên đổi
  chân chính là nghịch đảo tỉ số, tức z đổi dấu — cùng một luật, không phải
  luật thứ hai.

Chi phí: bạc đắt gấp ~4,5× vàng ở cùng cỡ stop (vàng 1,5 ATR 4,04% R / bạc
17,32% R). Mọi số chi phí trong báo cáo lấy từ dòng `cost-matched null: ...
cost X% of R` mà output tự in, **kèm cỡ stop**.

## 3. Tiền kiểm dữ liệu — đã chạy TRƯỚC khi đăng ký, và nó đã làm yếu tiền đề

Brief buộc kiểm tỉ số có thật hồi quy trong hai cửa sổ **trước** khi tin bất
cứ PF nào. Tôi chạy trước khi viết đăng ký này, và công bố ở đây làm bằng
chứng tiền nghiệm — không phải để chọn tham số.

Đồng bộ: **372.557 bar khớp dấu thời gian chính xác** (98,37% của XAU,
99,56% của XAG). Điều kiện cần cho một tỉ số có nghĩa: đạt.

| | cửa sổ A (07→10/2025) | cửa sổ B (04→07/2025) |
|---|---|---|
| drift tỉ số cả cửa sổ | **−9,75%** (91,64→82,71) | −0,06% (91,71→91,65) |
| drift hai chân | XAU +16,63% / XAG +29,22% | XAU +6,07% / XAG +6,14% |
| VR(32) tỉ số | 0,798 | 0,921 |
| VR(32) XAU một mình | 0,931 | 1,021 |
| AR(1) log-tỉ số | b=−0,00093 **t=−1,49** nửa đời 746 bar | b=−0,00083 **t=−1,53** nửa đời 837 bar |

Ba điều tiền kiểm nói, và cả ba đều chống lại giả thuyết:

1. **Variance ratio ủng hộ tiền đề**: tỉ số VR<1 ở mọi chân trời trong cả hai
   cửa sổ, và luôn THẤP HƠN VR của từng chân. Tỉ số thật sự ít trend hơn chân
   của nó.
2. **Nhưng phép thử có hướng thì bác bỏ.** corr(z_t, biến động log-tỉ số về
   sau) ở `lookback=96`: **DƯƠNG (tiếp diễn) ở cả hai cửa sổ, mọi chân trời**
   (A: +0,069…+0,167, t tới **+12,88**; B: +0,046…+0,107, t tới +7,98). Ở
   `lookback=480` dấu đảo: B hồi quy ở mọi h (t −1,51…−5,89), A tiếp diễn ở
   h≤32 và chỉ hồi quy ở h=96 (t=−2,70). **Không lookback nào cho hồi quy
   cùng dấu trên cả hai cửa sổ.** Ở lookback 96, một luật hồi quy đang đứng
   trước đoàn tàu — brief buộc nói ra, và tôi nói ra ở đây.
3. **AR(1) không phân biệt được với bước ngẫu nhiên**: t=−1,49 và −1,53, dưới
   2 ở cả hai cửa sổ. Nửa đời 746–837 bar 15m ≈ **7,8–8,7 ngày giao dịch**.

Và phát hiện cấu trúc quan trọng nhất của tiền kiểm — **tỉ số gần như là một
instrument bạc**:

| | A | B |
|---|---|---|
| corr(Δlog tỉ số, Δlog XAU) | **−0,052** | +0,126 |
| corr(Δlog tỉ số, Δlog XAG) | **−0,804** | −0,727 |
| phần phương sai do XAU | **4%** | 11% |
| phần phương sai do XAG | **104%** | 89% |

Vàng đóng góp 4–11% phương sai của tỉ số. Nên **chân vàng — chân rẻ — là chân
mang gần như KHÔNG tín hiệu.** Chân mang tín hiệu là bạc, chân đắt 4,5×. Đây
là khai trước, không phải phát hiện sau khi thấy kết quả.

Tệ hơn: nơi tỉ số CÓ hồi quy (B, lookback=480, t=−3,35…−5,89), chân vàng lại
đi **ngược**: corr(z, biến động vàng về sau) = **+0,098…+0,157, t=+7,09…+11,35**.
Tỉ số hồi quy bằng cách **bạc tăng mạnh hơn vàng**, không bằng vàng giảm. Một
lệnh short vàng ở z cao đứng sai phía với t=+11.

**Tiền kiểm này không thay đổi một tham số nào bên dưới.** Nó là lý do tôi dự
đoán falsifier sẽ bắn, và đăng ký vẫn chạy đủ để bắn nó qua cổng thật.

## 4. Falsifier — cụ thể, và tôi dự đoán nó BẮN

**Falsifier chính:** nếu **0 ô** trong 432 ô khai dưới đây vừa qua cổng
(PF ≥ 1,200 **VÀ** expectancy ≥ +0,050R **VÀ** ≥ 40 lệnh) trên **cả hai** cửa
sổ ở **cùng một** (row, chân, arm guards, arm chân trời), thì họ cơ chế "giá
trị tương đối vàng/bạc, biểu đạt một chân, khung 15m" bị **bác bỏ**.

Falsifier này bắn được: nó chỉ cần đọc 432 dòng output. Nó cũng bác bỏ được
ngược lại — một ô qua cả hai cửa sổ cùng row sẽ không bắn nó.

**Falsifier phụ (luật có nổ không — bẫy §8 của brief):** nếu `--exit-mix` cho
thấy luật của tôi nổ 0 lần (toàn TIMEOUT / NEWS_FLAT / WEEKEND_FLAT) thì mọi
PF của ô đó vô giá trị bất kể nó là bao nhiêu, và tôi báo nó là không đo được
chứ không phải là kết quả.

**Falsifier phụ 2 (phân vị vô nghĩa):** nếu `--null-sides=exposure` cho
null p95 ≥ PF của chính phương pháp, thì phân vị của ô đó là drift chân đó
chứ không phải cơ chế — đúng bài học §9.

## 5. Đa phép thử — đếm trước

Rows (36) = `lookback` {96, 480} × `zEnter` {1,5; 2,0; 2,5}
× `stopAtr` {1,5; 2,0; 3,0} × `revert` {+1 hồi quy, −1 tiếp diễn}

`revert=−1` (tiếp diễn) được khai **vì tiền kiểm đã cho thấy dấu đảo theo
lookback và theo cửa sổ.** Đo chỉ chiều hồi quy rồi đổi sang tiếp diễn sau
khi thấy kết quả mới là gian lận; khai cả hai từ đầu và đếm cả hai thì không.

Arms:

| arm | chân | cửa sổ | guards | chân trời | ô |
|---|---|---|---|---|---|
| chính | vàng, bạc | A, B | on, off | 4h (chuẩn desk) | 36×2×2×2 = **288** |
| phụ | vàng, bạc | A, B | off | 24h | 36×2×2×1 = **144** |

**Tổng ô cổng đã khai: 432.** Mỗi arm chạy 3 lần cho 3 null
(`coin`, `ratio`, `exposure`) ⇒ **36 run**, **1.296 quan sát (run,row)**.

Với 432 ô, ở mức danh nghĩa 5% tôi kỳ vọng ~22 dương tính giả. Nên **một ô
qua một cửa sổ không là gì**; tiêu chí thật là cùng row qua **cả hai** cửa sổ.

## 6. Cách đọc — ngưỡng chốt trước

- Cổng: PF ≥ 1,200 **VÀ** expectancy ≥ +0,050R **VÀ** **≥ 40 lệnh**.
  Lưu ý: `config/default.toml` đặt `min_trades = 30`, nên nhãn `SURVIVES`
  của engine dùng 30. **Tôi áp 40 của brief khi đọc**, không sửa config.
- Phân vị **không phải cổng**. Mọi phân vị viết kèm `null p50`.
- Cả hai arm guards đều báo (§8 brief: guards thổi cỡ mẫu 3,4×–106×).
- Cả ba null đều báo; `exposure` là null drift để đọc hồ sơ.
- `--exit-mix` bật ở mọi run.

## 7. Chân trời 24h, và cái KHÔNG đo được — khai trước

`max_hold_ms = 14_400_000` (4h) trong `[trading]`, không có override theo thị
trường. Cơ chế khai `Exits::Engine`, nên trần 4h **sẽ** chặn (probe timing
hôm nay: companion-unconfirmed TIMEOUT 5/8 lệnh). 4h = 16 bar 15m.

Nửa đời hồi quy đo được là **746–837 bar ≈ 7,8–8,7 ngày**. Nên arm chính đo
"hồi quy NHANH của tỉ số trong vòng 4 giờ", **không** đo hồi quy 8 ngày mà
tiền kiểm tìm thấy. Arm phụ nâng lên 24h qua một config riêng trong worktree
(`config-h24/`, bản copy chỉ đổi `max_hold_ms`) — **không** chạm
`config/accounts.toml` ngoài worktree.

**Cái không đo được, và số học của nó:** engine giữ một vị thế một lúc
(`ctx.position.is_some()` → `Intent::None`). Một cửa sổ 3 tháng ≈ 64 ngày
giao dịch. Ở chân trời 24h, số lệnh KHÔNG chồng lấn tối đa ≈ 64; ở chân trời
8 ngày ≈ **8**. Cổng đòi ≥ 40 lệnh. Vậy **cổng ≥40 lệnh và cửa sổ 3 tháng
cùng nhau CẤM đo bất cứ cơ chế nào có chân trời quá ~1 ngày.** Chân trời tự
nhiên của họ cơ chế này (8 ngày) nằm ngoài cái dụng cụ đo này đo được. Tôi
khai điều đó ở đây, trước khi chạy, và sẽ không dùng nó như một lời bào chữa
sau khi thấy kết quả.

Tôi KHÔNG đổi sang cửa sổ 12 tháng (dù `xauduka` có 7 năm trên đĩa) vì làm
vậy phá tính so sánh với mọi receipt khác của desk, và vì hai cửa sổ A/B là
giao thức desk. Chi phí của lựa chọn đó được báo ở §6 của báo cáo cuối.

## 8. Không đo bằng thứ bị cấm

`data-sealed/` không được trỏ tới, không đọc, không đếm. `--data=` trỏ
`/e/rust/flowdesk/data`. Receipt sẽ ghi đúng nguồn dòng `news:` in ra (engine
đọc `data/news/events.parquet` theo thư mục `data/` mặc định bất kể `--data=`).

`--rebate-share=` KHÔNG được truyền (hypotheses bỏ qua nó trong im lặng).
Số lần rút null là `--seeds=` (mặc định 200), không phải `--samples=`.
