# Kết quả: hồ sơ `rescore` không khai sai null, nhưng 158/264 ô của n6 không đọc được

**Ngày:** 2026-10-07 · **Nhánh:** `agent/reread-null`
**Binary:** `/e/rust/fd-instr-repair/target/release/search.exe` (build 07/10 01:35)
**Cổng:** trục này **không có cổng** — không đề cử cơ chế nào, tiêu **0 ô cổng**.

## 1. ĐẾM TRƯỚC — quy mô khai sai là **0**

Grep header mọi receipt `rescore` đã commit trên **39 nhánh**: 30 blob duy nhất
chứa `== rebate rescore`, trong đó 2 là file `.md` quyết định ⇒ **28 receipt**.

| `null sides:` khai | số receipt | nghĩa |
|---|---:|---|
| không có dòng này | 8 | chạy 23–24/09, header chưa có dòng đó ⇒ không thể khai sai |
| `coin` | 19 | khai đúng thứ `rescore` thật sự chạy |
| `exposure` | **1** | `receipts/instr-repair/D-rescore-exitmix.txt` (`agent/floor-audit`) |

Và **receipt duy nhất khai khác `coin` đã tự tố giác trong cùng header**:

    flags:    ** --null-sides IS NOT READ BY --mode=rescore — only by
              --mode=hypotheses — so nothing below was changed by it **

⇒ **Số receipt `rescore` đã công bố khai một null khác `coin` trong khi âm thầm
chạy null tung xu: 0.** Không có hồ sơ nào phải đọc lại vì cớ này. Cụ thể
`agent/n6` (12 receipt) và `2026-09-23-rebate-rescore` đều **không** khai sai:
n6 khai `coin`, hồ sơ 09-23 không in dòng đó.

## 2. Tiền đề của brief sai: bản vá là KHAI BÁO, không phải HÀNH VI

`agent/instr-repair` **không** làm `--null-sides=` có tác dụng trong `rescore`.

- `BY_MODE` (`search.rs:73`): `("null-sides", &["hypotheses"])`.
- Test `search.rs:1810`: `assert_eq!(one("rescore", "--null-sides=exposure").len(), 1,`
  `"rescore keeps the coin-flip null it published")`.
- `null_sides` chỉ đi vào `hypotheses.rs` (`run_hypothesis_fixed_as`,
  `rescore_hypothesis`); **không dòng nào** trong đường `rescore` đọc nó.

Nên **phép đo lại mà brief yêu cầu không tồn tại**: cờ đó không thể đổi một con
số `rescore` nào. Đo để chứng minh, không chỉ đọc mã — **9 lượt chạy, 3 cửa sổ**,
cùng batch file, cùng cửa sổ, cùng `--seeds=200 --direction-samples=1000`:

| cửa sổ | batch | nhánh so | md5 phần thân (bỏ dòng header 16) |
|---|---|---|---|
| `xauusd:15m` 2025-07-01→2025-10-01 (n6 R1) | `2026-10-06-rebate-optimal-family.toml` | coin vs exposure | `516db657b7c766a8be4fcf570be91b1d` **cả hai** |
| `xauusd:15m` 2025-04-01→2025-07-01 (n6 R5) | như trên | coin vs exposure | `d7cafa45db730497e941c1dd675041fe` **cả hai** |
| `xauusd:15m` 2025-09-13→2026-09-12 (hồ sơ 09-23) | `2026-09-23-rebate-rescore.toml` | coin vs exposure vs ratio | `37b89c3a4760bbff078f5489e3a88047` **cả ba** |

`diff` của mỗi cặp dài **đúng một dòng**: chính dòng `null sides:`. Mọi phân vị,
mọi `null p50`, mọi `null p95`, mọi verdict **trùng từng byte**.

**Chứng cứ ngược (để không kết luận "cờ chết ở mọi nơi"):** cùng binary, cùng
cờ, `--mode=hypotheses`, `qs-h15` trên `xauduka:15m` 2016-06-01→2018-06-01,
`--seeds=20` — cờ đó đổi **tất cả**:

| | coin | exposure |
|---|---:|---:|
| phân vị matched | **90%** | **80%** (−10 điểm) |
| null p50 | 0,952 | 0,858 |
| null p95 | 1,702 | 1,967 |
| count match | 1,50 ⚠ ngoài băng | **1,08** ✓ trong băng |

Cờ sống và mạnh — **trong `hypotheses`**. Trong `rescore` nó trơ.

Receipt: `R1-{coin,exposure}.txt`, `R5-{coin,exposure}.txt`,
`D0923-{coin,exposure,ratio}.txt`, `PC-hyp-{coin,exposure}.txt`.

## 3. `agent/n6`: 264 ô, **0 phán quyết đổi**, phân vị đổi **0 điểm**

Binary đã vá tái hiện n6 **chính xác**. Cả 22 dòng của R1 trùng bản đã công bố
trên `trades / PF gross / PF net / mnullG / mnullN / dirN`, và cả 22 dòng
`matched null p50 … p95` trùng từng chữ số ở **cả hai** nhánh null.

⇒ **0 / 264 ô đổi phán quyết. Phân vị lệch lớn nhất: 0 điểm.** F2 (3 điểm) cũng
không bắn được, vì không có phép đo thứ hai để so.

## 4. Con số đáng giữ nhất — tác hại thật, và nó LỚN HƠN việc khai sai

**158 trong 264 ô của n6 (59,8%) đã ngoài băng count match** ⇒ theo đúng luật
§4 của desk, **phân vị của chúng không đọc được**, độc lập với khuyết điểm null.

| receipt | R1 | R2 | R3 | R4 | R5 | R6 | R7 | R8 | R9 | R10 | R11 | R12 | tổng |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| ngoài băng / 22 | 15 | 15 | 13 | 13 | 13 | 13 | 11 | 11 | 14 | 14 | 13 | 13 | **158 / 264** |

Không ô nào in `null p50 0,000`, nên không ô nào là "không calibrate được" theo
nghĩa đó. Nhưng chỗ đau là chỗ nối hai việc lại:

> `--null-sides=exposure` **chính là** cái công tắc kéo count match về trong băng
> (chứng cứ ngược mục 2: 1,50 → 1,08). Mà `rescore` **bỏ qua** nó.
> ⇒ **158 ô đó không thể kéo về trong băng trên binary này, bằng bất cứ cờ nào.**

Đó là giá thật của khuyết điểm (7): **không phải một lời khai sai trong header
(0 receipt), mà một null không sửa được.** Hồ sơ `rescore` không đọc **quá cao**
hay **quá thấp** — với 158/264 ô nó **không đọc được**, và dụng cụ không có
đường để đọc.

## 5. Hồ sơ 09-23: phán quyết đứng nguyên, nhưng phân vị cao nhất của nó không đọc được

Verdict **không đổi**: `0 of 22 passed all three legs` cả khi công bố lẫn hôm nay.

Phân vị matched **cao nhất** của hồ sơ — `intraday-momentum`, **98% gross /
99% net trên 197 lệnh** — có **count match 14,79**: null rút trung vị **2.914
lệnh** so với **197 lệnh** của method, lệch ~15×. Engine 23/09 **không in dòng
count match nào** (0 lần trong receipt đã công bố), nên hồ sơ in phân vị đó
thành một con số **không kèm cảnh báo nào**. Chạy lại hôm nay: **9 / 22 dòng
ngoài băng** (`ema-cross`, `stoch-reversal`, `ict-sweep-mss-fvg`,
`doji-reversal`, `volume-thrust`, `volman-box`, `intraday-momentum`, …).

**Phân vị 09-23 KHÔNG tái hiện — nhưng không phải vì null sides.** So bản đã
công bố 23/09 với hôm nay, **11 / 20 dòng đọc được đổi quá 3 điểm**, và chiều là
**TĂNG**: `macd-cross` +10, `squeeze-break` +10, `orb` +10, `pdhl` +10,
`ict-sweep-mss-fvg` +28, `volume-thrust` +41, `atr/hivol-london` +5,
`vwap-fade` +4, `keltner-break` +4; giảm chỉ 2 dòng (`bb-fade` −5,
`atr/hivol-orb60` −4). Nghĩa là hồ sơ cũ đọc **quá THẤP**.

Thủ phạm đã được công bố từ trước và **không phải trục này**:
`docs/decisions/2026-09-23-matched-null-repair.md` — *"repairing it moved 69 of
85 percentiles"* — hồ sơ rescore ra trước bản vá đó, và chính nó liệt
`2026-09-23-rebate-rescore/primary-xauusd-15m.txt` là **22 dòng / 2 null phân
biệt**, phục vụ các dòng từ **2 đến 1.297 lệnh**. So với receipt **đã vá**
(`…/matched-null-repair/primary-xauusd-15m-repaired.txt`), hôm nay chỉ còn
**4 / 20 dòng** lệch quá 3 điểm, lớn nhất `atr/hivol-orb60` **14 → 24** (+10,
181 lệnh). Đó là trôi engine còn lại, không phải null sides.

## 6. Không đo được, và vì sao

- **Phân vị `rescore` dưới null khớp phơi nhiễm**: *không đo được trên binary
  này*. `rescore` không có đường nhận `--null-sides=`, nên câu "phân vị tụt hay
  tăng khi null khớp drift" **không có câu trả lời** cho mode này. Muốn có thì
  phải sửa hành vi (`rescore_hypothesis` nhận `NullSides`) — đó là sửa engine,
  ngoài phạm vi trục này.
- **Băng count match của hồ sơ 09-23 tại thời điểm công bố**: không đo được —
  engine 23/09 không in dòng đó (0 lần). Chỉ đọc được trên lần chạy lại.
- **`matched_rate` ở cổng hẹp là tung xu**: với 158/264 ô n6 ngoài băng, phân vị
  của chúng **không công bố**, theo §4. Không thay chúng bằng con số nào.

## 7. Giữ nguyên

Đồng nhất thức `reb/R = share × (cost/R)` của n6 không phụ thuộc null, nên kết
luận **"hoàn phí không tạo được họ cơ chế"** đứng nguyên. Trục này không chạm nó.

## 8. Sổ đa phép thử

Khai **286** ô (264 n6 + 22 hồ sơ 09-23). **Xem 0 ô cổng** — F1 bắn ở tiền kiểm.
Tiêu: **11 lượt chạy** (9 tiền kiểm A/B + 2 chứng cứ ngược). Tiền lệ §5 giữ đúng.

**Khai một lỗi quy trình của chính tôi:** commit đăng ký `1eecda5` lẽ ra đi
**một mình**, nhưng nó mang theo `docs/hypotheses/2026-10-06-rebate-optimal-family.toml`
— file toml của n6 tôi `git checkout agent/n6 --` ra trước khi commit, nên nó
đã nằm trong index. Blob **không đổi một byte**
(md5 `879ad2ed7e48a597e09424adce6535b9`, trùng blob trên `agent/n6`), không phải
mã, và không có số nào của trục này phụ thuộc vào nó. Không viết lại lịch sử;
ghi lại ở đây.

## 9. Việc desk nên ghi lại

Việc khai sai `null sides` trong `rescore` **vô hại trên thực tế**: 0 receipt đã
công bố khai sai, và cờ đó không đổi được một con số `rescore` nào, nên **không
ai phải đọc lại hồ sơ `rescore` vì cớ này nữa.** Điều cần đọc lại là **count
match**, không phải null sides — và nó cần một bản vá **hành vi**, không phải
khai báo.
