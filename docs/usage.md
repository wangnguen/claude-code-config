# Hướng dẫn sử dụng

## Hệ thống hoạt động thế nào

Bạn không gọi thẳng API của Anthropic. Luồng thực tế:

```
Claude Code trên máy bạn
      │
      │  virtual API key riêng của bạn
      ▼
LiteLLM Proxy
      │
      │  Anthropic API key production của công ty
      ▼
Claude model
```

Mỗi người được cấp **một virtual key riêng**. LiteLLM dùng key đó để biết ai đang
gọi API, giới hạn model nào được dùng, và theo dõi chi phí. Vì vậy tuyệt đối
không dùng key của người khác — usage sẽ ghi sai người.

## Model `-vn`

Virtual key của team chỉ được phép dùng các model có hậu tố `-vn`:

```
claude-opus-5-vn
claude-sonnet-5-vn
claude-haiku-4-5-vn
claude-opus-4-8-vn
claude-sonnet-4-6-vn
```

Các tên **không có** hậu tố (`claude-sonnet-5`, `claude-haiku-4-5-20251001`, …)
là danh sách built-in của Claude Code. Chọn một trong số đó sẽ nhận:

```
API Error: 403 key not allowed to access model.
This key can only access models=['claude-opus-5-vn', ...]
```

Đây **không phải** lỗi thiếu quyền có thể xin cấp thêm — chúng đơn giản là những
id model khác, không tồn tại trên gateway. Luôn chọn tên có `-vn`.

Hậu tố viết **thường** (`-vn`), không phải `-VN`.

Danh sách trên chỉ là ví dụ; gateway có thể thêm model mới bất cứ lúc nào.
`ccc models` luôn cho thấy danh sách hiện tại của key bạn.

## Các slot model

Claude Code đọc model cho từng "slot" từ phần `env` của
`.claude/settings.local.json`:

| Slot | Biến | Ý nghĩa |
|---|---|---|
| `main` | `ANTHROPIC_MODEL` | Model khi mở phiên mới |
| `sonnet` | `ANTHROPIC_DEFAULT_SONNET_MODEL` | Alias `sonnet` trong `/model` trỏ tới đây |
| `opus` | `ANTHROPIC_DEFAULT_OPUS_MODEL` | Alias `opus` (và `opusplan`) |
| `haiku` | `ANTHROPIC_DEFAULT_HAIKU_MODEL` | Alias `haiku`, đồng thời chạy các tác vụ nền |
| `custom` | `ANTHROPIC_CUSTOM_MODEL_OPTION` | Không bắt buộc: thêm 1 dòng tuỳ ý vào `/model` |

`ANTHROPIC_SMALL_FAST_MODEL` đã bị Claude Code đánh dấu deprecated. `ccc` vẫn
ghi nó cho các bản Claude Code cũ, luôn bằng giá trị slot Haiku.

Bốn slot đầu **phải có giá trị** là model key bạn dùng được. Slot nào để trống
thì alias tương ứng rơi về id built-in và nhận 403.

### Model nào đang thật sự được dùng

`ccc models current` (và dòng `Model` ở splash, `ccc version`, màn Home của TUI)
cho biết một phiên Claude Code **mới** mở trong thư mục này sẽ chạy model nào.
ccc tính theo đúng thứ tự của Claude Code:

1. `ANTHROPIC_MODEL`, nếu có nguồn nào đặt
2. nếu không có: trường `model`, chỗ `/model` lưu lựa chọn (`~/.claude/settings.json`)
3. nếu không có: `ANTHROPIC_DEFAULT_MODEL`
4. nếu không có: mặc định của Claude Code là `opus`

Alias như `opus` hay `sonnet[1m]` được dịch qua slot tương ứng. Với mỗi biến,
file settings thắng biến trong shell; giữa các file thì thứ tự là managed policy,
`.claude/settings.local.json`, `.claude/settings.json`, rồi user settings.

Vì `ccc lite` ghi `ANTHROPIC_MODEL`, model chọn bằng `/model` sẽ **không** có tác
dụng ở thư mục đó; `ccc models current` báo dòng `Ignored` cho trường hợp này.
Muốn đổi thì dùng `ccc models pick`.

Lệnh chỉ đọc file. `--model` hoặc đổi `/model` trong phiên đang chạy thì không
lưu ở đâu để đọc được, xem bằng `/status` trong Claude Code.

## Model discovery: nên bật, không bắt buộc

`CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1` khiến `/model` liệt kê tên model
thật lấy từ gateway. Tắt đi thì `/model` chỉ hiện các alias (`default`,
`sonnet`, `opus`, `haiku`, …) — **vẫn dùng được bình thường**, vì mỗi alias trỏ
tới slot tương ứng ở trên. Điều kiện thật sự để không bị 403 là đủ các slot,
không phải discovery.

Lưu ý: discovery chỉ giữ những id có chữ `claude` hoặc `anthropic`. Model tên
khác vẫn chạy nếu đặt vào slot, nhưng không chọn được theo tên trong `/model`.

## 1M context (`[1m]`)

Tên model `-vn` có thể mang thêm marker `[1m]` để chọn bản context 1 triệu token:

```
claude-sonnet-5-vn[1m]
claude-opus-5-vn[1m]
```

Marker này là **client-side**. Claude Code tách `[1m]` ra, gửi id trần
(`claude-sonnet-5-vn`) tới gateway, và chuyển `[1m]` thành beta header
long-context. Hệ quả:

- `ccc models` không bao giờ liệt kê tên có ngoặc vuông — gateway chỉ biết id trần.
- Config ghi `claude-sonnet-5-vn[1m]` mà gateway trả `claude-sonnet-5-vn` là
  **đúng**, không phải lệch. `ccc check` và `ccc doctor` đã strip marker trước
  khi so khớp (hàm `base_model_id()` trong `api.rs`).

Chỉ Sonnet và Opus có bản 1M. Haiku 4.5 không có, nên
`ANTHROPIC_SMALL_FAST_MODEL` và `ANTHROPIC_DEFAULT_HAIKU_MODEL` để trần.

Marker đặt **sau** hậu tố `-vn`: `claude-sonnet-5-vn[1m]` đúng,
`claude-sonnet-5[1m]-vn` sai.

## Cài đặt

### Windows

```powershell
irm https://raw.githubusercontent.com/wangnguen/claude-code-config/main/install.ps1 | iex
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/wangnguen/claude-code-config/main/install.sh | bash
```

Script tải binary từ GitHub Releases, giải nén vào `~/.ccc`, và thêm thư mục đó
vào PATH. **Khởi động lại terminal** sau khi cài.

## Thiết lập lần đầu

Vào thư mục project, chạy:

```bash
ccc lite
```

Lệnh này sẽ:

1. Hỏi virtual key của bạn (nhập ẩn, không hiện ra màn hình)
2. Hỏi gateway danh sách model của key đó. Gateway từ chối key thì dừng luôn,
   **không ghi gì**; gateway không liên lạc được thì vẫn tiếp tục với model mặc định
3. Copy thư mục `.claude/` mẫu vào project
4. Ghi `.claude/settings.local.json` với base URL của LiteLLM, key của bạn, bật
   model discovery, và điền các slot model bằng tên `-vn`. Model mặc định nào
   key không dùng được thì tự đổi sang model gần nhất mà key có
5. Thêm `.claude/settings.local.json` vào `.gitignore` nếu project là git repo

Sau đó kiểm tra:

```bash
ccc check
```

Tất cả phải pass (dòng vàng 🟡 là lưu ý, không phải lỗi). Rồi khởi động lại
Claude Code và mở `/model` — phải thấy các model `-vn` với nhãn "From gateway".

Muốn đổi model sau này: `ccc models pick`.

## Chế độ tương tác

Gõ `ccc` không kèm lệnh sẽ mở dấu nhắc tương tác: banner kèm key và tình trạng
model của thư mục, danh sách lệnh hay dùng (mỗi lệnh có một dòng giải thích; nằm
bên phải banner, hoặc bên dưới khi cửa sổ hẹp), rồi vòng lặp `›` để gõ lệnh mà
không cần lặp lại `ccc`.

```
  › check
  › doctor
  › key use work
  › clear
  › exit
```

Bên trong đó:

- Gõ tên lệnh trần (`check`) hoặc kèm tiền tố quen tay (`ccc check`) đều được
- `?` in lại danh sách lệnh, `help` liệt kê **toàn bộ** lệnh
- `clear` (hoặc `/clear`, `cls`, Ctrl-L) dọn màn hình **cùng cả scrollback**, rồi
  in lại banner. Ctrl-L giữ nguyên dòng đang gõ, `clear` thì không
- `exit` / `quit` / `q`, Ctrl-D trên dòng trống, hoặc Ctrl-C để thoát — cả ba
  đường đều in một dòng chào tạm biệt, nên phiên không bao giờ tắt lặng lẽ như
  vừa crash
- Con trỏ đi được: ←/→, Home/End, Backspace/Delete, Ctrl-U xoá cả dòng
- Lệnh gõ sai được clap báo lỗi ngay tại chỗ, không làm thoát phiên

Mọi lệnh chạy trong đây đi qua đúng bộ phân tích tham số của dòng lệnh, nên
không có chuyện một lệnh cư xử khác nhau giữa hai chế độ.

## Các lệnh

### Thiết lập

| Lệnh | Việc nó làm |
|---|---|
| `ccc lite` | Thiết lập project cho LiteLLM gateway của team (khuyên dùng) |
| `ccc init` | Copy config mẫu, áp key mặc định từ `keys.json` |
| `ccc init --base-url <url>` | Như trên nhưng trỏ sang gateway khác, tự chọn model theo gateway đó |
| `ccc permission` (`ccc p`) | Thêm danh sách tool được phép vào settings |
| `ccc ui` | Mở giao diện toàn màn hình: key, model, config, kiểm tra |

### Model

| Lệnh | Việc nó làm |
|---|---|
| `ccc models current` | Model mà một phiên Claude Code mới trong thư mục này sẽ chạy, lý do, và giá trị từng slot. Chỉ đọc file, không gọi mạng |
| `ccc models` | Liệt kê model gateway trả về cho key, cái nào dùng được, slot nào đang dùng |
| `ccc models pick` | Chọn model cho từng slot của thư mục này bằng phím mũi tên + Enter, chọn xong là lưu luôn |
| `ccc models set <slot> <model>` | Đặt một slot; kiểm với gateway trước khi ghi |

```bash
ccc models set opus claude-opus-5-vn
ccc models set sonnet 'claude-sonnet-5-vn[1m]'
ccc models set custom none              # xoá slot custom
ccc models set opus some-model --force  # ghi luôn, không hỏi gateway
```

Slot hợp lệ: `main`, `sonnet`, `opus`, `haiku`, `custom`. `ccc` từ chối để
trống slot bắt buộc và từ chối `[1m]` cho model không có bản 1M.

### Kiểm tra

| Lệnh | Việc nó làm |
|---|---|
| `ccc check` | Kiểm thư mục đã setup chưa, key, discovery, và từng slot model |
| `ccc doctor` | Kiểm môi trường: Claude Code, config, và thứ đang ghi đè config (không cần mạng) |
| `ccc show config` | In config hiện tại (key đã che) |

### Quản lý key

```bash
ccc key                 # mở TUI ở màn Keys
ccc key add <tên> [giá trị]   # bỏ trống giá trị thì được hỏi, nhập ẩn
ccc key list
ccc key default [tên]   # đặt key mặc định trong ~/.ccc/keys.json
ccc key use [tên]       # áp key cho thư mục hiện tại
ccc key remove [tên]
ccc key rename
ccc key status          # test tất cả key
```

Phân biệt hai lệnh dễ nhầm:

- `ccc key default` — chọn key nào là **mặc định toàn cục**, lưu trong
  `~/.ccc/keys.json`. `ccc init` sẽ dùng key này.
- `ccc key use` — áp key vào **thư mục project hiện tại**, ghi vào
  `.claude/settings.local.json`.

### Cấu hình

```bash
ccc config list
ccc config get base_url
ccc config set sonnet_model claude-sonnet-5-vn[1m]
```

Lưu ý shell: `[` và `]` là ký tự đặc biệt trong PowerShell và zsh. Nếu gặp lỗi
parse, bọc giá trị trong nháy đơn: `ccc config set sonnet_model 'claude-sonnet-5-vn[1m]'`.

Các khoá hợp lệ: `base_url`, `main_model`, `fast_model`, `model`, `sonnet_model`,
`opus_model`, `haiku_model`, `custom_model`, `discovery`, `disable_traffic`,
`model_filter`.

`model_filter` là quy tắc của chính `ccc` (lưu ở `~/.ccc/ccc.json`), không phải
biến Claude Code đọc — xem mục [Gateway khác](#gateway-khác).

Lưu ý: `ccc config` sửa file **mẫu** ở `~/.ccc/.claude/settings.local.json`, mà
Claude Code **không đọc**. File mẫu chỉ được `ccc init` / `ccc lite` copy sang
project. Muốn đổi config đang có hiệu lực thì sửa `.claude/settings.local.json`
trong project.

### Khác

```bash
ccc update       # kiểm tra và cài bản mới
ccc version
ccc completions powershell   # sinh shell completion
```

`ccc`, `ccc ui` và `ccc version` tự báo khi có bản mới (dòng vàng
`Update 0.2.0 is out — run update`, hoặc `→ 0.2.0` ở góc trên TUI). Chúng hỏi
GitHub tối đa 6 tiếng một lần, chờ tối đa 2 giây, và nhớ kết quả trong
`~/.ccc/update-check.json`, nên phần lớn các lần mở không tốn request nào.
`ccc update` thì luôn hỏi lại. Muốn tắt thông báo: đặt biến môi trường
`CCC_NO_UPDATE_CHECK=1`.

## Gỡ cài đặt

```bash
ccc uninstall
```

Lệnh in ra những gì sắp xoá rồi hỏi xác nhận (mặc định là không). Nó xoá:

- Thư mục `~/.ccc/` — binary `ccc`, thư mục `.claude` mẫu, và `keys.json`
- **Toàn bộ key đã lưu** trong `keys.json` — không khôi phục được
- Dòng PATH mà script cài đặt đã thêm (biến môi trường User trên Windows, dòng
  `export PATH` trong `.zshrc`/`.bashrc`/`.profile` trên macOS/Linux)

Nó **không** đụng vào:

- Claude Code
- Thư mục `.claude/` trong các project của bạn

Nghĩa là sau khi gỡ `ccc`, các project vẫn chạy bình thường: key nằm sẵn trong
`.claude/settings.local.json` và Claude Code đọc thẳng file đó, không cần `ccc`.
Muốn xoá sạch key thì phải tự xoá file đó trong từng project.

Trên Windows, `ccc.exe` đang chạy nên không tự xoá được ngay; lệnh xoá mọi thứ
còn lại rồi hẹn xoá nốt thư mục vài giây sau khi tiến trình thoát. Nhớ khởi động
lại terminal để PATH cập nhật.

## Giao diện toàn màn hình

`ccc ui` mở ở màn **Home**, `ccc key` mở ở màn **Keys**; màn **Models** nằm
trong menu của Home. (`ccc` trơn mở [chế độ tương tác](#chế-độ-tương-tác), không phải TUI;
`ccc models pick` hỏi lần lượt ngay trong terminal, không chiếm cả màn hình.)

Mọi màn đều ghi phím tắt ở dòng cuối; `?` mở bảng đầy đủ kèm giải thích. `Esc`
quay lại, `q` thoát, Ctrl-C thoát ở bất cứ đâu. Cửa sổ nhỏ hơn 64×20 thì màn
hình báo cần phóng to thay vì vẽ lệch.

| Màn | Nội dung |
|---|---|
| Home | Tình trạng thư mục (key, gateway, model), 4 bước "Getting started" và bước tiếp theo, lối vào các màn khác (`1`–`4`) |
| Keys | Các key đã lưu: `★` là mặc định cho thư mục mới, `●` là key thư mục này đang dùng |
| Models | Chọn model cho từng slot từ danh sách gateway trả về cho key |
| Config | Những gì Claude Code đọc ở thư mục này, key đã che |
| Check | Giống `ccc check` |

### Màn Keys

| Phím | Hành động |
|---|---|
| `↑` `↓` hoặc `j` `k` | Di chuyển |
| `a` | Thêm key (tên, rồi tới key — phần key nhập ẩn) |
| `u` | Dùng key đang chọn cho thư mục hiện tại |
| `d` | Đặt key đang chọn làm mặc định |
| `m` | Chọn model cho thư mục này bằng key đang chọn |
| `s` | Kiểm tra tất cả key |
| `n` / `r` | Đổi tên / xoá (có hỏi lại) |

### Màn Models

Màn này hỏi gateway danh sách model của key (key của thư mục, hoặc key bạn chọn
bằng `m` ở màn Keys), chỉ cho chọn những model key dùng được, rồi ghi vào
`.claude/settings.local.json`.

| Phím | Hành động |
|---|---|
| `↑` `↓` | Chọn slot (hoặc model, khi đang ở danh sách model) |
| `→` / `Enter` | Chọn model cho slot |
| `←` / `Esc` | Quay lại danh sách slot, rồi về Home |
| `w` / Ctrl-S | Lưu |
| `f` | Tự điền các slot trống hoặc đang dùng model không hợp lệ |
| `1` | Bật/tắt 1M context (chỉ Sonnet và Opus) |
| `g` | Bật/tắt discovery |
| `e` / `x` | Gõ / xoá model Custom |
| `r` | Tải lại danh sách |

Rời màn khi chưa lưu thì `ccc` hỏi lại. Nếu danh sách được lấy bằng một key mà
thư mục chưa dùng, lúc lưu `ccc` hỏi có dùng luôn key đó cho thư mục không —
model phải đi cùng key truy cập được chúng.

## Gateway khác

`ccc` không bị gắn chặt vào gateway của team. Quy tắc "model nào dùng được" tuỳ
gateway:

| Gateway | Bộ lọc | Model dùng được |
|---|---|---|
| Gateway của team | `-vn` | Model gateway trả về có đuôi `-vn` |
| Gateway khác | không lọc | Mọi model gateway trả về cho key |

```bash
ccc init --base-url https://gateway-khac.example.com   # trỏ project sang gateway khác
ccc config set model_filter -xyz                        # tự đặt hậu tố
ccc config set model_filter none                        # tắt lọc
ccc config set model_filter auto                        # về mặc định
```

Gateway phải hỗ trợ Anthropic Messages API (`/v1/messages`). Anthropic không hỗ
trợ chạy Claude Code bằng model không phải Claude qua gateway, kể cả khi gateway
tự chuyển đổi được.

## Xử lý sự cố

### Lỗi 403 "key not allowed to access model"

Bạn đang chọn model không có `-vn`. Chạy `ccc check` — nó sẽ chỉ ra biến nào
đang pin sai. Hoặc mở `/model` trong Claude Code và chọn lại model có `-vn`.

### Không thấy model `-vn` trong danh sách

Kiểm tra theo thứ tự:

1. `ccc check` — discovery đã bật chưa (dòng `Discovery`). Tắt thì `/model` chỉ
   hiện alias; alias vẫn dùng được, chỉ là không thấy tên thật
2. `ccc models` — gateway có thực sự trả về model `-vn` cho key của bạn không
3. `ccc doctor` — có gì ưu tiên cao hơn đang ghi đè không

### Config bị ghi đè

Claude Code đọc cấu hình từ nhiều nguồn. Thứ tự ưu tiên, **cao nhất trước**:

| Ưu tiên | Nguồn |
|---|---|
| 1 | `managed-settings.json` (policy do IT quản lý) |
| 2 | Tham số dòng lệnh (`--model`, `--settings`) |
| 3 | `.claude/settings.local.json` trong project |
| 4 | `.claude/settings.json` trong project |
| 5 | `~/.claude/settings.json` |

Biến môi trường của shell cũng thắng file settings.

Bẫy hay gặp: bạn sửa `~/.claude/settings.json` nhưng project lại có sẵn
`.claude/settings.local.json` từ lần chạy `ccc` trước — file trong project thắng.
`ccc doctor` báo cho bạn biết những nguồn nó nhìn thấy được.

### Model `-vn` hiện trong picker nhưng vẫn 403

Gateway có thể liệt kê nhiều model hơn số model key bạn được phép dùng. Chạy
`ccc models` — nó đánh dấu rõ cái nào "usable", cái nào "not for this key".

## Bảo mật

- **Không gửi virtual key qua chat**, không commit vào source dùng chung.
- `ccc lite` nhập key ở chế độ ẩn, key không nằm lại trong lịch sử terminal.
- `ccc show` và `ccc config list` che key trước khi in, an toàn khi share màn hình.
- `ccc init` / `ccc lite` / `ccc key use` / lưu model (khi file có key) tự thêm
  `.claude/settings.local.json` vào `.gitignore` — chỉ đúng file chứa key, không ignore cả thư mục `.claude/`,
  để `settings.json`, `skills/`, `commands/` của project vẫn commit được như
  bình thường. Nếu `.gitignore` đã có sẵn dòng đó (hoặc đã ignore cả `.claude/`)
  thì `ccc` không ghi thêm gì.
- Key được lưu dạng plaintext trong `~/.ccc/keys.json`. File này chỉ nằm trên
  máy bạn, nhưng đừng đồng bộ nó lên cloud storage dùng chung.
