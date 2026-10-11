# Đăng ký trước — định giá lại spread vàng, và quy ước MỘT CHIỀU vs VÒNG TRÒN

Nhánh `agent/gold-reprice`, cắt từ `29c8635`. Commit này đi **một mình**, trước
lần gọi `search.exe` đầu tiên của nhánh. Cho phép của chủ:
`AGENT-BRIEF-ADDENDUM-11.md` §0.3 — *"ĐƯỢC PHÉP định giá lại vàng về spread
0,220 — nhưng phải chốt quy ước MỘT CHIỀU vs VÒNG TRÒN trước"*.

## 1. Giả thuyết, một câu

`rules.spread` của engine là **bề rộng báo giá bid–ask** (một đại lượng một
chiều), và engine trừ **đúng một lần bề rộng đó cho mỗi vòng** ⇒ con số
`ask − bid` mà `py/live/spread_log.py` ghi là **cùng một đại lượng**, nên thay
0,28 bằng 0,220 là một phép thay **cùng loại**, và nó làm chi phí **RẺ HƠN**,
không đắt hơn.

## 2. Falsifier — cụ thể và bắn được

**F1 (falsifier của brief).** Nếu đọc mã cho thấy tổng chi phí engine trừ cho
một vòng là **½ × `rules.spread`** (tức 0,28 là một số một chiều bị dùng như
vòng tròn), thì hồ sơ đang **thiếu một nửa** và con số đúng là **0,440 — ĐẮT
HƠN**. Bắn bằng: `apply_costs` trừ ở **mấy** chỗ và **bao nhiêu** mỗi chỗ;
`cost_fraction_of_r` nhân `spread` với **1** hay **2**.

**F2.** Nếu `apply_costs` chỉ được gọi ở entry (không ở exit) trên một đường
đóng lệnh nào, tổng vòng tròn **không** bằng `spread` ⇒ F1 bắn trên đường đó.
Bắn bằng: liệt kê **mọi** call site và đối chiếu với **mọi** đường đóng lệnh.

**F3.** Nếu `py/live/spread_log.py` ghi một đại lượng **khác** `ask − bid`
(ví dụ đã nhân đôi, hay là một nửa), thì 0,220 **không** cùng loại với 0,28.
Bắn bằng: đọc dòng tính cột `spread` của logger.

**F4 (đối chứng DƯƠNG, phụ lục 11 §VIII).** Bốn lần chạy ở `--spread=0.28`
phải **tái tạo tới chữ số in ra** các dòng `agent/cost-breakeven` đã công bố ở
`s = 0,28`. Nếu không tái tạo được thì dụng cụ của tôi **không nhìn thấy**, và
mọi con số dịch chuyển ở `s = 0,220` không đọc được.

**F5 (probe cố ý nhìn trước, phải TRƯỢT).** `ovn1900/short` — cùng giờ, cùng
phơi nhiễm, cùng sizing như `ovn1900/long`, chỉ lật chiều, `PF_r` 0,6459. Nó
**phải trượt cổng ở cả 0,28 và 0,220 trên cả hai cửa sổ**. Nếu định giá lại làm
probe này qua cổng thì phép đo là mù.

**F6.** Câu config tự khai *"0,28 nằm trên MỌI spread từng quan sát"*
(`config/default.toml` quanh dòng 306–333) phải **sai** trên log hiện tại nếu
có bất kỳ mẫu nào > 0,28. Bắn bằng: đếm trên `data/spreads/XAUUSD_sc.csv`.

## 3. Đa phép thử — đếm ô TRƯỚC

| | khai |
|---|---|
| dòng | **14** (file thiết kế đóng băng của `agent/cost-breakeven`, `83c43eb`, **không thêm, không bỏ, không đổi một tham số**) |
| cửa sổ | **2** (IS 2010-06-01→2018-06-01, OOS 2018-06-01→2026-06-01, **ghim cả hai đầu tường minh**) |
| arm guards | **1** (chỉ ON — arm duy nhất chủ cho phép) |
| điểm spread | **2** (0,28 = đối chứng tái tạo; 0,220 = định giá lại) |
| ô | **14 × 2 × 2 = 56**, **4 lần chạy** |
| **ô cổng tiêu** | **0** — `agent/cost-breakeven` đã đo `spread* = null` ở **mọi** spread ≥ 0 trên cả 11 ứng viên, **kể cả spread = 0** ⇒ **0,220 > 0 nên không dòng nào có thể qua cổng**, và tôi **không** đi tìm một dương tính. Chỉ dòng nào qua mới tiêu ô cổng. |

**KHÔNG quét cỡ stop** (bẫy 2 phụ lục 10): cỡ stop giữ ở giá trị riêng của từng
dòng ở cả 56 ô. **KHÔNG công bố phân vị** — `count match` của `qs-lb3` đọc 2,96
(ngoài băng) và `null p50` 0,929 < 1 (brief §4).

## 4. Cách đọc

- **Hai hiệu ứng NGƯỢC CHIỀU trong cùng một phép đổi.** `d risk/d s = +1/2`
  (`engine.rs:717-719`: `risk = |entry − stop|` với `entry` đã qua
  `apply_costs` còn stop cấu trúc thì chưa) ⇒ hạ spread **cũng co mẫu số R**.
  Chi phí mỗi vòng giảm (tốt cho `E`), mẫu số R co lại (phóng to mọi thứ, kể cả
  lỗ). Báo **cả hai**, đừng báo hiệu số như thể nó là một hiệu ứng.
- **`E = Lbar × (PF_r − 1)`**, in cả `PF_r` và `PF_usd`, **không trích tỉ lệ
  khe của ai** (phụ lục 11 §V.2).
- `expectancy_net` (swap 0, commission 0 trên market này ⇒ `= expectancy`),
  drawdown **USD** cạnh mọi số lợi nhuận, `sized_down_by_guard` đọc **thẳng
  field**.
- **In số bar của chính lần chạy** (phụ lục 11 §VII).
- **Một số thì không đủ.** Spread thật đi 0,210 → 0,290 trong ngày và
  `--spread=` chỉ nhận **một** số ⇒ mọi con số ở đây là chi phí ở **một điểm**
  của một phân phối, và tôi khai phân vị nào + giờ nào ở mục đầu báo cáo.

## 5. Cái giá, khai trước khi đo

Định giá lại làm **mọi receipt cũ không tái lập**. Theo tiền lệ
`docs/decisions/2026-09-16-spread-repricing-scale.txt`: đây là **một bản ghi
quy mô, có ngày, thuần THÊM**. **0 dòng bị xoá**, **0 chỉnh
`config/default.toml`**, **0 chạm `config/local.toml` /
`config/accounts.toml`**. Mọi đính chính là **ghi chú có ngày thêm vào CUỐI**.
Lý do không tạo lớp thứ hai: `agent/receipt-reconcile` đã đo hồ sơ nhiễm ở
**tầng đặc tả** (25/34 dòng hỏng vì cửa sổ bỏ trống), nên một lớp đính chính
thứ hai sẽ nhân chỗ nhiễm lên.

---

## Ghi chú thêm — 2026-10-11, SAU khi chạy. Không một dòng nào ở trên bị sửa.

**F1 (falsifier của brief) KHÔNG BẮN.** Sáu chỗ trong mã, độc lập, cùng nói một
thứ: `rules.spread` bị trừ **đúng một lần cho cả vòng**
(`apply_costs` `half = spread/2` ở **cả** entry **và** exit;
`Trade::spread` tự khai *"round turn"*; `cost_table.rs` `cost_money = spread × 1`
với test ghim *"0.28 / 2.00 = 14.0% of R"*; `rebate.rs` *"the two halves are one
spread; crediting a share of each side would be twice the truth"*;
`search.rs:932` spread **một** lần cạnh commission **hai** lần;
`spread_log.py:116` `spread = t.ask - t.bid`). Và `config/default.toml` nói
**"spread 28 pts"**, **không bao giờ** nói "round trip". ⇒ **0,440 là sai; con
số đúng là 0,220, RẺ HƠN.** Chi tiết: `receipts/gold-reprice/RESULT.md` §1.

**F2 không bắn** (mọi đường đóng lệnh gọi `apply_costs(entering=false)`).
**F3 không bắn** (logger ghi `ask − bid`).
**F4 BẮN DƯƠNG, mức mạnh nhất có thể**: hai lần chạy ở 0,28 giống **từng byte**
receipt `s0.28` của `agent/cost-breakeven`, cả hai cửa sổ, 28/28 ô.
**F5 đúng như phải đúng**: probe `ovn1900/short` **trượt** cả hai chân, cả hai
cửa sổ, cả hai spread ⇒ phép đo không mù.
**F6 BẮN**: câu config *"0,28 nằm trên MỌI spread từng quan sát"* **đã sai** —
251/8.967 mẫu (**2,80%**) vượt 0,28, max 0,2900, **toàn bộ** trong UTC 20/21/22.

**Hai thứ đưa vào SAU khi thấy số** (dán nhãn ở RESULT.md §5): bảng spread theo
**giờ New York** (một lần đọc CSV, không phải ô backtest), và phép định giá theo
dòng `(s[vào] + s[ra])/2` suy thẳng từ `apply_costs`. Không hiệu chỉnh, không
khớp tham số.

**Một thứ tôi KHÔNG hứa ở trên và vẫn tìm thấy**, nên nó được khai ở đây chứ
không trong bảng: **0,220 phẳng THU ÍT 15,4–24,1%** ở **NY 16:00/17:00/18:00**,
và đó **chính xác** là dải mà `ADDENDUM-10 §IV` đo được *toàn bộ* tiền của hồ sơ
(`16:00-18:00` G 6,061 vs `14:00-16:00` G 0,101). ⇒ **cần chủ quyết: một số
phẳng, hay một trục spread theo giờ?** Tôi **không** đổi `config/default.toml`
và **không** chọn thay.
