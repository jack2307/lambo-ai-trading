# Vận hành bàn flowdesk

Sổ tay cho người trực bàn, không phải cho người sửa code.

Viết bằng tiếng Việt vì nó được đọc lúc vội, trong phiên RDP, thường là khi
có gì đó không chạy. Phần còn lại của `docs/` là hồ sơ kỹ thuật và vẫn bằng
tiếng Anh; file này là ngoại lệ có chủ đích.

Mọi lệnh bên dưới chạy trong PowerShell, tại `C:\flowdesk`.

---

## 0. Bàn này đang là gì (21/09/2026)

**Một tài khoản tiền thật, một model.**

| | |
|---|---|
| Tài khoản | `33708517`, tên broker **"Lambo V12"** |
| Vốn | 100.000 USC = $1.000 |
| Model | DeepSeek, sổ `ai-xau-ds-ctx` |
| Hệ số | `lot_scale = 10` → mỗi lệnh liều ~$10 = 1% tài khoản |
| Terminal | `C:\MT5-v12\terminal64.exe` |

Tài khoản cũ **`33705331` đã tắt** (`enabled = false`), còn 382 USC.

Ngoài ra có **29 sổ giấy** chạy nghiên cứu, không đụng tiền. Chín trader AI
kéo chúng: bảy DeepSeek, một Opus, một Terra.

---

## 1. Việc đầu tiên, luôn luôn

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File deploy\status.ps1
```

Nó **chỉ đọc**, không khởi động và không dừng gì. Đọc nó trước khi kết luận
có gì hỏng.

Bình thường phải thấy: API up, **traders 9, executors 1, pollers 4, MT5
terminals 3**, và dòng `REAL MONEY executor: vantage-v12 at x10`.

---

## 2. Sau khi máy khởi động lại

**Tự lên, không cần làm gì:**

- `fd-api` (API + giao diện web)
- Telegram watch
- **Bảy sổ DeepSeek**, gồm cả `ai-xau-ds-smc`

**KHÔNG tự lên, phải làm tay sau khi vào RDP:**

1. **Mở các terminal MT5.** Poller cần terminal, mà terminal sống trong phiên
   RDP của anh — lúc máy vừa boot chưa ai đăng nhập nên chưa có phiên nào.
   Mở `C:\MT5-cent\terminal64.exe` và `C:\MT5-v12\terminal64.exe`, kiểm tra
   **AutoTrading đang bật** (nút Algo Trading trên thanh công cụ).

2. **Poller** — nguồn nến cho mọi thứ:
   ```powershell
   powershell -File py\live\start_pollers.ps1 -Terminal "C:\MT5-cent\terminal64.exe" -Symbols cent
   ```

   `-Symbols` nhận **tên bộ symbol**, chỉ `cent` hoặc `standard` — đưa `.sc`
   vào là nó từ chối. Không cần `-FillOnOpen`: `[prices] fill_on_open = true`
   trong `config\accounts.toml` đã bật sẵn và poller tự đọc.

3. **Opus và Terra** (hai model dùng gói thuê bao, SYSTEM không với tới được):
   ```powershell
   powershell -File py\live\start_ai_traders.ps1 -Detached
   ```
   Lệnh này khởi động lại **cả chín** trader, kể cả bảy con đã tự lên. Không
   sao — nó dừng rồi bật lại toàn bộ.

4. **Executor tiền thật** — làm cuối cùng, và làm **trong RDP chứ không qua
   SSH**:
   ```powershell
   powershell -File py\live\start_executors.ps1 -Live -AllowReal
   ```

> **Vì sao executor không tự lên.** Đây là lựa chọn, không phải thiếu sót.
> Luật của bàn này: `-Live` và `-AllowReal` là **quyền do người đang tỉnh
> tiêu**. Một task tự vào lệnh tiền thật sau khi máy reboot lúc 3 giờ sáng,
> với terminal có thể chưa đăng nhập xong, là tiêu quyền đó hộ anh.

---

## 3. Dừng khẩn cấp

**Dừng một sổ trên tài khoản thật, và đóng luôn vị thế đang mở:**

```powershell
Set-Content C:\flowdesk\data\live\vantage-v12\ai-xau-ds-ctx\STOP "ly do, ngay thang"
```

Executor thấy file này trong vòng ~15 giây, **đóng vị thế rồi tự thoát**. Đây
là công tắc ngắt, dùng nó thay vì `Stop-Process`.

**Dừng một sổ trên MỌI tài khoản:**

```powershell
Set-Content C:\flowdesk\data\paper\<ten-so>\STOP "ly do"
```

**File STOP không tự mất.** Sổ đứng im tới khi có người xoá file. Muốn chạy
lại thì xoá nó rồi khởi động lại executor.

**Dừng hết tiền thật ngay lập tức:**

```powershell
Get-CimInstance Win32_Process -Filter "name='python.exe'" |
  Where-Object { $_.CommandLine -like '*mt5_executor*' } |
  ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
```

Cách này **không đóng vị thế** — nó chỉ ngắt người trông. Vị thế còn nguyên
trên sàn với stop và target của nó. Dùng khi cần chặn lệnh MỚI ngay lập tức;
dùng file STOP khi muốn đóng cái đang có.

---

## 4. Ba cái bẫy đã trả giá bằng tiền và bằng thời gian

**Launcher tự khoá chính nó.** Trước 21/09, `start_ai_traders.ps1` đổ log vào
một tên file cố định. Các trader nó sinh ra **thừa kế handle** và giữ file đó,
nên lần chạy sau mở không được và **chết câm** — trong khi vẫn báo "đã khởi
động, pid ...". Bàn này không khởi động lại được trader suốt hai ngày mà không
ai biết. Đã sửa bằng log có dấu thời gian.

> **Dấu hiệu nhận ra:** một log mà **dấu thời gian không nhúc nhích**. Nếu
> nghi ngờ, so giờ khởi động tiến trình với giờ sửa file log.

**`-Only` và `-Model` DỪNG những gì chúng bỏ sót.** `start_ai_traders.ps1`
luôn dừng *toàn bộ* trader trước khi bật phần được chọn. Chạy
`-Model deepseek-flash` giữa phiên là **giết Opus và Terra** mà không bật
lại. Tôi đã tự làm đúng chuyện này lúc 21:23 ngày 21/09. Task boot giờ có
`-OnlyIfNoneRunning` để từ chối khi đã có trader chạy — **nhưng gõ tay thì
không có lưới đó**. Muốn khởi động lại mọi thứ thì dùng `-Detached`, không
kèm bộ lọc nào.

**Terminal sống trong phiên RDP của anh.** Ngắt kết nối RDP thì được —
**đăng xuất thì không**. Đăng xuất là terminal chết, poller mù, executor mất
đường tới sàn. Luật của máy này: **DISCONNECT, đừng LOG OFF.**

---

## 5. Số liệu ở đâu

| Cần gì | Ở đâu |
|---|---|
| Giao diện | `http://127.0.0.1:8138` trong RDP, hoặc cổng 8139 qua tunnel — xem §6 |
| Lãi lỗ tiền thật | `data\live\vantage-v12\ai-xau-ds-ctx\broker.json` |
| Nhật ký lệnh thật | cùng thư mục, `executor.jsonl` |
| Quyết định của AI | `data\paper\<ten-so>\decisions.jsonl` |
| Tiền API đã tốn | trường `cost_usd` trong file trên |
| Log khởi động trader | `data\paper\logs\start_ai_traders-*.out` |
| Log task boot | `data\paper\logs\boot-traders-*.out` |

---

## 6. Vào bàn từ máy ở nhà (Cloudflare Tunnel)

Bàn phục vụ **hai cổng**, và khác nhau chỗ nào thì đó chính là toàn bộ lớp
bảo vệ của nó:

| Cổng | Ai dùng | Mật khẩu |
|---|---|---|
| **8138** | chín trader AI, executor, bốn poller — chúng gọi liên tục và **không mang mật khẩu** | không có, và không được thêm |
| **8139** | `cloudflared`, tức là anh từ máy ở nhà | **mọi request đều phải có phiên đăng nhập** |

> **Tunnel phải trỏ vào 8139, tuyệt đối không phải 8138.** Trỏ nhầm là mở
> toàn bộ nút "Dừng sổ", "Tạm dừng" và "Xoá credential" ra internet.

Vì sao phải hai cổng chứ không phải một cái khoá? Vì `cloudflared` chạy
**ngay trên máy này**. Mọi request từ internet đi qua nó đều đến với địa chỉ
`127.0.0.1`. Một luật kiểu "cho qua nếu là loopback" sẽ cho qua **tất cả**.

### Đặt mật khẩu (làm một lần)

```powershell
cd C:\flowdesk
.\fd-api.exe --set-password
```

Nó hỏi hai lần, không hiện chữ, tối thiểu **12 ký tự**, và từ chối nếu ngắn
hơn. Kết quả ghi vào `config\local.toml` — cùng file đang giữ key DeepSeek,
file này **không vào git**. Mật khẩu gốc **không được lưu ở đâu cả**, chỉ có
muối ngẫu nhiên và một verifier PBKDF2.

Nếu cần đặt bằng script: `$env:FD_AUTH_PASSWORD = '...'` rồi chạy lệnh trên,
**xong nhớ `Remove-Item Env:FD_AUTH_PASSWORD`** — và nó đã nằm trong lịch sử
PowerShell rồi.

### Bật cổng 8139

Thêm vào `config\local.toml`:

```toml
[auth]
enabled = true
```

rồi khởi động lại `fd-api`. Hoặc chạy tay với `--auth`.

**Chưa có mật khẩu mà bật thì nó TỪ CHỐI mở cổng** và in một dòng bắt đầu
bằng `!!` ra console. Nó không bao giờ mở cổng đó ở trạng thái không khoá.
8138 vẫn chạy bình thường trong mọi trường hợp.

### Dựng tunnel (làm một lần, trong RDP)

`cloudflared` đã cài sẵn tại `C:\cloudflared\cloudflared.exe` (bản 2026.9.1,
chữ ký Cloudflare hợp lệ). Chưa đăng nhập, chưa tạo tunnel nào.

**1. Tạo tunnel trên dashboard.** Vào Cloudflare → Zero Trust → Networks →
Tunnels → Create a tunnel → chọn **Cloudflared** → đặt tên. Nó hiện một lệnh
cài kèm **một token dài**.

> Token đó là **bí mật** — ai có nó là dựng được tunnel vào tài khoản anh.
> Đừng dán nó vào chat, vào ticket, vào chỗ nào ngoài cửa sổ RDP.

**2. Cài service trên VPS**, dán token vào chỗ `<TOKEN>`:

```powershell
C:\cloudflared\cloudflared.exe service install <TOKEN>
```

Nó chạy dưới dạng Windows service, **tự lên sau khi máy khởi động lại**.

**3. Trỏ hostname vào cổng 8139.** Trong tunnel vừa tạo → Public Hostname →
Add:

| Trường | Giá trị |
|---|---|
| Subdomain | một tên **khó đoán**, ví dụ `v12-9k3x` — đừng dùng `desk` hay `api` |
| Domain | tên miền của anh trong Cloudflare |
| Type | `HTTP` |
| URL | `127.0.0.1:8139` |

> **`8139`, không phải `8138`.** Gõ nhầm một chữ số là đưa toàn bộ nút "Dừng
> sổ", "Tạm dừng" và "Xoá credential" ra internet **không có mật khẩu**.
> Kiểm lại ô này trước khi bấm lưu.

**4. Nên làm: bật Cloudflare Access.** Zero Trust → Access → Applications →
Add → Self-hosted → chọn hostname vừa tạo → policy `Emails` = email của anh.

Nó bắt đăng nhập **ở biên Cloudflare, trước khi request chạm tới máy anh**.
Nghĩa là kẻ tấn công không tới được cả cái form đăng nhập để mà dò mật khẩu —
lớp giãn-khoá bên trong fd-api trở thành lưới thứ hai chứ không phải lưới duy
nhất. Miễn phí tới 50 người dùng.

**Kiểm tra sau khi xong:**

```powershell
Test-NetConnection 127.0.0.1 -Port 8139     # TcpTestSucceeded phải là True
Get-Service cloudflared | Select Status
```

Rồi mở hostname trên máy nhà: phải thấy **trang đăng nhập**, không phải giao
diện bàn. Thấy thẳng giao diện là **tunnel đang trỏ nhầm 8138** — tắt service
ngay và sửa lại.

### Ba chuyện sẽ gặp

- **Deploy là đăng xuất.** Phiên nằm trong RAM, không ghi đĩa. Khởi động lại
  `fd-api` là mọi phiên mất. Đăng nhập lại, không có gì hỏng.
- **Phiên sống 12 tiếng**, tính từ lúc đăng nhập, không gia hạn khi dùng.
- **Gõ sai nhiều thì bị khoá.** Năm lần đầu miễn phí, sau đó chờ 5s, 10s,
  20s... tối đa 15 phút, và **mật khẩu đúng cũng bị từ chối trong lúc đang
  khoá**. Nếu anh bị khoá ngoài tunnel: vào RDP, dùng 8138, hoặc đợi hết 15
  phút. Phiên đã đăng nhập từ trước **không bị ảnh hưởng**.

---

## 7. Những việc còn treo

- **Ngưỡng lỗ ngày** đang là $300 tính **trên từng sổ**, mà mỗi sổ chỉ có
  $100 — nên nó **chưa bao giờ bắn được và không thể bắn**. Trần số lệnh đã
  nới từ 4 lên 20, nên chỗ này hở hơn trước.
- **Sổ `eur-hours`** mang vốn $277,51 thay vì $100 (do một lệnh hỏng đã bị
  loại trừ nhưng vốn chưa đặt lại), nên nó đang đặt lệnh **to gấp 2,8 lần**
  dự định. Chỉ bẩn số liệu nghiên cứu, không mất tiền thật.
- **Đổi mật khẩu MT5 của 33708517.** Nó từng đi qua một cuộc hội thoại.
  Terminal đã nhớ phiên nên đổi xong không phải làm gì thêm.
- **Hai sổ `plan` và `plan-trigger`** đang chạy tới mốc **30 lệnh** đã cam
  kết trước (hiện 7 và 9). Khoảng 01/10. Đừng tắt sớm.
