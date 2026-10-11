# Đăng ký trước — xếp hạng chi phí/R của MỌI instrument trong kho, TRƯỚC khi đo cơ chế (`agent/instr-cost-screen`)

**Ngày:** 2026-10-11. Nhánh `agent/instr-cost-screen`, cắt từ `agent/stop-width`
(`29c8635`). Commit này **chỉ có file này**, trước dòng code đầu tiên (brief §5).

**Job này KHÔNG chạy cơ chế nào và KHÔNG tiêu ô cổng.** Nó quyết định ô nào đáng
tiêu về sau. Không có cổng để qua hay trượt; mục 7 nói rõ thay vào đó đọc gì.

## 1. Giả thuyết, một câu

`agent/mould-target` khuyến nghị desk "thôi trả tiền sửa carry trên vàng 15m, và
chi vào CHI PHÍ hoặc **sang thị trường khác**" — **nếu "sang thị trường khác" là
một hướng đi được bằng dữ liệu đang có, thì phải tồn tại ít nhất một instrument
trong kho có chi phí/R thấp hơn vàng theo hệ số đủ để đóng khe `1,901× → 8,7×`
của khuôn**, tức **≥ 4,576× rẻ hơn** (8,7 / 1,901), và con số đó phải sống sót ba
phép kiểm: spread là ĐO (không GIẢ ĐỊNH), kho nến phủ **cả hai** cửa sổ, và
`swap` không phải một `null` đang đóng vai 0.

## 2. Tiền kiểm bằng ĐỌC MÃ NGUỒN + ĐỌC CONFIG, trước khi tiêu một phép đo nào

1. **Dụng cụ đã có, không viết lại.** `crates/fd-backtest/src/cost_table.rs` đã
   có `cost_fraction_of_r` (triệt tiêu `lots` và `contract_size` tường minh, có
   test ghim), `per_bar`, `summarise` (phân vị nearest-rank), `bucket`,
   `daily_bars` (ranh giới 17:00 NY = UTC+3 nửa đêm). Và
   `crates/fd-backtest/src/bin/cost_table.rs` đã in bảng ba thước. Job này
   **thêm một binary mới** bên cạnh nó, **không sửa** nó, vì nó là binary đã sinh
   receipt.
2. **`swap_*_per_lot` là `#[serde(default)]` f64**
   (`crates/fd-core/src/config.rs:347-350`) ⇒ market **không khai** swap nhận
   **0,0 im lặng**. `markets.gold` và `markets.btc` không khai ⇒ **0,0 của chúng
   là `null` đóng vai 0, không phải một phép đo.** Khai báo trước: bảng phải có
   cột phân biệt *khai tường minh* với *nhận mặc định*.
3. **Khuyết điểm 17 không phổ quát** — đã đọc config:
   `markets.xauusd` / `markets.eurusd` / `markets.btcusd` khai
   `starting_equity_usd = 100,0`; `markets.gold` / `xauduka` / `xagduka` /
   `eurduka` / `btc` **không khai** ⇒ thừa kế `[trading] 10_000,0`
   (`engine.rs:354`). Với `raw_lots = risk_usd/(risk × contract_size)` và
   `lots = (floor(raw/lot_step)×lot_step).max(min_lot)` (`engine.rs:729-730`),
   bảng phải in `raw_lots` ở stop trung vị của **từng** market và nói market nào
   bị kẹp `min_lot`.
4. **Spread: provenance chỉ sống trong COMMENT.** Không có key nào trong config
   nói "ĐO" hay "GIẢ ĐỊNH" ⇒ **không chương trình nào phân biệt được**, và đó là
   một khuyết điểm riêng. Provenance trong bảng của tôi là **chép tay từ comment
   của `config/default.toml`**, và nguồn mỗi dòng được trích nguyên văn.
5. **AUDNZD có nến trên đĩa mà config nhánh này KHÔNG khai.**
   `data/bars/AUDNZD-{1m,5m,15m}.parquet` tồn tại; `grep -rni audnzd` trên cây
   `29c8635` ra **0 kết quả**. Block `[markets.audnzd]` sống ở nhánh `agent/aud`
   với `spread = 0.00030` và comment tự ghi **`# NOT MEASURED`**. Job này
   **cherry-pick CHỈ block config đó** (thêm một market mới = thuần cộng thêm,
   không đổi một con số nào của market cũ, nên không receipt nào mất khả năng
   tái lập), để AUDNZD là một **hàng** trong bảng chứ không phải một chú thích.

## 3. MỘT định nghĩa chân trời, chốt trước, in ra ở mọi bảng

    THUOC CHINH (P):  stop = 1,5 x ATR14 tren nen 15m, don vi gia cua chinh
                      instrument do.

Chọn nó vì nó là thước **duy nhất** mà mọi con số đã công bố của hồ sơ đứng trên:
vàng **12,58% / 8,35%** hai cửa sổ thật, AUDNZD **38,16%**, BTC đo lại
**2,35% → 7,79%**. Một thước khác thì bảng của tôi không so được với hồ sơ.

Hai thước thứ hai, in cạnh, vì **một thước duy nhất là một lựa chọn chưa khai**:

    THUOC S1:  stop = 1,5 x ATR20 tren nen NGAY PHIEN (ranh gioi 21:00Z
               = 17:00 NY, dung `daily_bars` offset UTC+3)
    THUOC S2:  stop = 1,5 x ATR14 tren nen 4h (= `[trading] max_hold_ms`,
               chan troi moi phuong phap `Exits::Engine` bi dong lenh tai)

`1,5` **không đổi được thứ tự** giữa các instrument: tỉ số là `spread/(k × ATR)`
nên `k` là một phép co giãn thuần, mọi hàng dịch cùng hệ số. Thứ **đổi** thứ tự
là **cỡ nến và chu kỳ ATR** — đó là lý do có ba thước, không phải một.

## 4. BỐN cửa sổ, khai trước, mỗi con số gắn một cửa sổ

Vì ATR tính bằng **USD** nên nó lớn theo mức giá (1,5 ATR vàng 2,23 USD → 16,12
USD) còn spread cố định ⇒ **cùng một spread, chi phí/R chênh ~7 lần giữa hai đầu
hồ sơ của CÙNG một instrument.** Nên không có con số nào được in mà không có cửa
sổ bên cạnh:

    W1  2010-06-01 -> 2018-06-01   (nua dau ho so Dukascopy)
    W2  2018-06-01 -> 2026-06-01   (nua sau; cua so hai feed MT5 nam trong)
    CM  cua so CHUNG moi instrument con song deu phu (tinh ra, khong chon)
    PL  pooled (ca kho) - IN DE DOI CHIEU, KHONG dung de so instrument

**`PL` không dùng để so instrument** và bảng phải nói thế: nó trộn thập kỷ.

## 5. Falsifier — cụ thể và bắn được

- **F1 (falsifier của brief, cái chính).** Bắn khi: **không instrument nào** có
  `cost/R` ở thước P, **trên CẢ W1 VÀ W2**, thấp hơn `cost/R` của vàng 15m cùng
  cửa sổ theo hệ số **≥ 4,576×**, *với spread có provenance ĐO*. F1 bắn ⇒
  **"sang thị trường khác" là hướng KHÔNG đi được bằng dữ liệu đang có; nó đòi
  MUA dữ liệu mới, và đó là quyết định của chủ.**
- **F2 (bẫy BTC, bắn ở tiền kiểm).** Bắn khi instrument rẻ nhất của bảng có
  provenance **GIẢ ĐỊNH** hoặc **một lần đọc**. Bắn ⇒ hàng đó **không được đếm
  cho F1**, và nó được in với chữ `ASSUMED` ngay trên dòng. (Đã biết trước:
  `markets.btc.spread = 5.0` và config tự ghi 5,00 là giả định.)
- **F3 (phủ cửa sổ).** Bắn trên từng hàng khi kho nến của instrument đó chỉ phủ
  **một** cửa sổ. Bắn ⇒ hàng đó là **một cửa sổ, không phải một instrument**, và
  không đủ để trả lời F1 (brief: mọi giả thuyết qua HAI cửa sổ).
- **F4 (`swap` null).** Bắn trên từng hàng khi `swap_long/short` là **0,0 nhận
  mặc định** hoặc 0,0 khai mà comment không dẫn một phép đo trên **đúng
  symbol đó**. Bắn ⇒ chi phí qua đêm của hàng đó **không đo được**, nên con số
  chi phí/R của nó là **CHỈ spread**, một **SÀN**, không phải tổng chi phí.
- **F5 (dấu in nhà cung cấp).** Bắn trên từng hàng khi tỉ lệ `open == low` (hoặc
  `close == low`) ở nến mở lại sau mốc nghỉ 17:00 NY cao hơn tỉ lệ ở nến thường
  theo hệ số ≥ **2×**. Bắn ⇒ instrument đó **mang dấu in của nhà cung cấp** và
  mọi cơ chế đọc `open`/`low` quanh mốc đó đọc một hiện vật đường ống.
- **F6 (lot kẹp).** Bắn trên từng hàng khi `raw_lots < min_lot` ở stop trung vị
  ⇒ mọi `PF_usd` tương lai trên instrument đó là PF của một **sổ kẹp min_lot**
  (khuyết điểm 17), không phải sổ rủi ro 1%.

## 6. Sổ đa phép thử — ĐẾM TRƯỚC, và mỗi thước là một lựa chọn

    instrument:        9  (8 trong config `29c8635` + audnzd cherry-pick)
    thuoc chan troi:   3  (P, S1, S2)   <- moi thuoc la MOT lua chon, khai ra
    cua so:            4  (W1, W2, CM, PL)
    => 9 x 3 x 4 = 108 con so chi phi/R

    kiem kho nen:      24 file parquet tren dia (8 symbol x 3 co nen, EURUSD
                       thieu 1m, GC chi co 1m) -> doc het, bao het
    kiem cot volume:   24 file x 4 trang thai (null / che ra / tick / that)
    dau in nha cung cap: 9 instrument x 4 phep thu (open==low, close==low,
                       open==high, close==high) x 2 lop nen (nen mo lai sau
                       moc nghi / nen thuong) = 72 ti le
    lot kep (khuyet 17): 9 instrument x 1 (raw_lots o stop trung vi thuoc P)

    TONG KHAI: 108 + 24 + 96 + 72 + 9 = 309 con so, 0 o cong, 0 lenh mo.

**Không thêm trục nào sau khi thấy số.** Sửa đăng ký = thêm ghi chú có ngày vào
**CUỐI** file này.

## 7. Cách đọc — không có cổng, nên nói rõ đọc gì

1. **Mọi con số chi phí/R in kèm BỐN thứ**: cửa sổ · thống kê (trung vị
   nearest-rank, kèm p10/p90 và `n`) · cỡ stop (số, trong đơn vị giá của chính
   instrument) · provenance spread (ĐO / GIẢ ĐỊNH / MỘT LẦN ĐỌC).
2. **Hai mốc so sánh in cạnh mỗi hàng**: (a) chi phí/R của **vàng 15m** cùng
   cửa sổ cùng thước; (b) **hệ số rẻ hơn vàng cần để đạt khuôn** = `8,7 / 1,901
   = 4,576×`, và hệ số **thật** của hàng đó.
   ⚠️ Phép quy đổi này khai một giả định: nó giả định **cùng một tín hiệu**
   (cùng gross tính bằng R) được bê sang instrument khác, nên `gross/spread` co
   giãn đúng bằng `cost_vang / cost_instrument`. Giả định đó **chưa đo** và
   bảng phải nói thế.
3. **`null` không bao giờ in thành `0`.** Spread không lập được ⇒ không có tỉ
   số, **và không phải tỉ số của vàng**. File nến rỗng là file rỗng, không phải
   một instrument không có biến động. Cột volume `null` in `null`.
4. **Đối chứng DƯƠNG bắt buộc** (phụ lục 10 §VII): trước khi báo bất kỳ số âm
   nào, chứng minh dụng cụ **nhìn thấy được**:
   - *thước chi phí:* một instrument tổng hợp có spread đúng **1/10** của vàng
     trên **cùng chuỗi nến vàng** phải đọc ra đúng **1/10** chi phí/R và phải
     **QUA** sàng 4,576×. Nếu nó không qua, sàng của tôi mù.
   - *dấu in nhà cung cấp:* một chuỗi nến **cố ý** đặt `open = low` ở mọi nến
     phải đọc **100%**; một chuỗi nến giá liên tục phải đọc **~0%**. Và một
     **probe cố ý nhìn trước** — đặt `open = low` CHỈ ở nến mở lại — phải
     **bắn F5**, để một F5 không-bắn không thể là một test mù.
5. **Nếu một instrument rẻ bất thường: KHÔNG chạy nó.** Việc của job này là
   kiểm con số đó có thật (F2 + F3 + F4), rồi báo nó kèm toàn bộ kiểm chứng và
   để **chủ quyết** có mở một chương trình trên nó hay không (phụ lục 9 §IX).
6. **Chi phí/R là SÀN, không phải tổng chi phí**, ở mọi hàng F4 bắn: nó là
   `spread/stop` và không chứa swap, commission, slippage hay phí mỗi lệnh.
