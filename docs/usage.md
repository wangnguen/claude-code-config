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

## Cài đặt

### Windows

```powershell
irm https://raw.githubusercontent.com/wangnguen/ccc/main/install.ps1 | iex
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/wangnguen/ccc/main/install.sh | bash
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
2. Copy thư mục `.claude/` mẫu vào project
3. Ghi `.claude/settings.local.json` với base URL của LiteLLM, key của bạn, bật
   model discovery, và pin sẵn 5 biến model sang tên `-vn`
4. Thêm `.claude/` vào `.gitignore` nếu project là git repo

Sau đó kiểm tra:

```bash
ccc check
```

Tất cả phải pass. Rồi khởi động lại Claude Code và mở `/model` — phải thấy các
model `-vn` với nhãn "From gateway".

## Các lệnh

### Thiết lập

| Lệnh | Việc nó làm |
|---|---|
| `ccc lite` | Thiết lập project cho LiteLLM gateway (khuyên dùng) |
| `ccc init` | Copy config mẫu, áp key mặc định từ `keys.json` |
| `ccc permission` (`ccc p`) | Thêm danh sách tool được phép vào settings |

### Kiểm tra

| Lệnh | Việc nó làm |
|---|---|
| `ccc check` | Kiểm key, model discovery, và từng biến model đã pin |
| `ccc models` | Liệt kê model gateway trả về, đánh dấu cái nào dùng được |
| `ccc doctor` | Kiểm môi trường: Claude Code, config, và thứ đang ghi đè config |
| `ccc show config` | In config hiện tại (key đã che) |

### Quản lý key

```bash
ccc key                 # mở TUI quản lý key
ccc key add <tên> <giá trị>
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
ccc config set sonnet_model claude-sonnet-5-vn
```

Các khoá hợp lệ: `base_url`, `main_model`, `fast_model`, `model`, `sonnet_model`,
`opus_model`, `haiku_model`, `discovery`, `disable_traffic`.

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

## TUI quản lý key

Chạy `ccc` không kèm lệnh, hoặc `ccc key`, sẽ mở giao diện toàn màn hình.

| Phím | Hành động |
|---|---|
| `↑` `↓` hoặc `j` `k` | Di chuyển |
| `a` | Thêm key mới |
| `d` | Đặt key đang chọn làm mặc định |
| `u` | Dùng key đang chọn cho thư mục hiện tại |
| `r` | Xoá key (có xác nhận) |
| `n` | Đổi tên key |
| `s` | Kiểm tra tất cả key |
| `q` hoặc `Esc` | Thoát |

## Xử lý sự cố

### Lỗi 403 "key not allowed to access model"

Bạn đang chọn model không có `-vn`. Chạy `ccc check` — nó sẽ chỉ ra biến nào
đang pin sai. Hoặc mở `/model` trong Claude Code và chọn lại model có `-vn`.

### Không thấy model `-vn` trong danh sách

Kiểm tra theo thứ tự:

1. `ccc check` — biến `CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY` đã bằng `"1"` chưa
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
- `ccc init` / `ccc lite` / `ccc key use` tự thêm `.claude/` vào `.gitignore`.
- Key được lưu dạng plaintext trong `~/.ccc/keys.json`. File này chỉ nằm trên
  máy bạn, nhưng đừng đồng bộ nó lên cloud storage dùng chung.
