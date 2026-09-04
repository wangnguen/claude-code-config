# Kiến trúc code

## Cấu trúc thư mục

```
ccc/
├── Cargo.toml                  Khai báo project + danh sách thư viện
├── Cargo.lock                  Phiên bản chính xác của mọi thư viện (commit vào git)
│
├── src/                        Toàn bộ mã nguồn
│   ├── main.rs                 Điểm vào. Định nghĩa CLI, điều hướng sang lệnh
│   ├── config.rs               Đường dẫn file, đọc/ghi JSON, hằng số, KeysStore
│   ├── api.rs                  Gọi gateway: lấy danh sách model, kiểm tra key
│   ├── ui.rs                   Hàm vẽ khung, dòng kẻ, dấu tick cho terminal
│   ├── utils.rs                Hàm dùng chung: nhập liệu, copy thư mục, che key
│   │
│   ├── commands/               Mỗi file = một lệnh con của `ccc`
│   │   ├── mod.rs              Khai báo các file trong thư mục này
│   │   ├── lite.rs             `ccc lite`
│   │   ├── init.rs             `ccc init`
│   │   ├── key.rs              `ccc key ...`
│   │   ├── config.rs           `ccc config ...`
│   │   ├── check.rs            `ccc check`
│   │   ├── models.rs           `ccc models`
│   │   ├── doctor.rs           `ccc doctor`
│   │   ├── permission.rs       `ccc permission`
│   │   ├── show.rs             `ccc show`
│   │   ├── update.rs           `ccc update`
│   │   ├── version.rs          `ccc version`
│   │   └── completions.rs      `ccc completions`
│   │
│   └── tui/                    Giao diện toàn màn hình
│       ├── mod.rs
│       ├── app.rs              Bộ điều phối: giữ state, xử lý sự kiện, vẽ
│       ├── component.rs        Định nghĩa Action / KeyOp / ModeSwitch
│       ├── event.rs            Vòng lặp đọc phím
│       ├── input.rs            Ô nhập liệu
│       ├── theme.rs            Màu sắc, icon
│       └── components/         Các thành phần giao diện
│           ├── home.rs             Màn hình chính
│           ├── dashboard.rs        Bảng tổng quan key
│           ├── key_table.rs        Bảng danh sách key
│           ├── status_dashboard.rs Màn hình kiểm tra key
│           ├── input_modal.rs      Hộp thoại nhập
│           ├── confirm_modal.rs    Hộp thoại xác nhận
│           ├── header.rs
│           ├── footer.rs
│           └── toast.rs            Thông báo ngắn
│
├── default/.claude/            Config mẫu, đóng gói kèm binary
│   └── settings.local.json     KHÔNG BAO GIỜ chứa API key — file này lên git
│
├── docs/                       Tài liệu (thư mục bạn đang đọc)
├── install.ps1                 Script cài cho Windows
├── install.sh                  Script cài cho macOS/Linux
└── .github/workflows/
    └── release.yml             CI: kiểm tra compile, bump version, build, release
```

## Dữ liệu nằm ở đâu

Có ba nơi chứa trạng thái, dễ nhầm lẫn nhất trong repo này:

```
~/.ccc/                             ← "nhà" của ccc
├── ccc.exe                             binary
├── keys.json                           kho key: { active, keys: {tên: giá trị} }
└── .claude/
    └── settings.local.json             config MẪU (Claude Code KHÔNG đọc file này)

<project>/                          ← thư mục làm việc của bạn
└── .claude/
    └── settings.local.json             config THẬT mà Claude Code đọc
```

Điểm quan trọng: `~/.ccc/.claude/settings.local.json` chỉ là **bản mẫu** để
`ccc init` và `ccc lite` copy đi. Claude Code không bao giờ đọc nó. Nhiều người
sửa file này rồi thắc mắc sao không có tác dụng. `ccc doctor` có in cảnh báo về
điều này.

## Thứ tự tìm API key

Hàm `get_current_key()` trong [`src/api.rs`](../src/api.rs) tìm theo thứ tự:

```
1. ./.claude/settings.local.json   → env.ANTHROPIC_AUTH_TOKEN
                                   → env.ANTHROPIC_API_KEY   (biến cũ, tương thích ngược)
2. ~/.ccc/keys.json                → key đang được đặt active
3. ~/.ccc/.claude/settings.local.json
```

Tìm thấy ở bước nào thì dừng ở đó.

## Hai biến chứa key

| Biến | Dùng khi nào |
|---|---|
| `ANTHROPIC_AUTH_TOKEN` | Biến hiện tại. Virtual key của LiteLLM là bearer token |
| `ANTHROPIC_API_KEY` | Biến cũ. Chỉ còn đọc để tương thích, không bao giờ ghi mới |

Hàm `set_auth_token()` trong [`src/config.rs`](../src/config.rs) ghi biến mới
**và xoá biến cũ**, để hai biến không bao giờ tồn tại song song với giá trị khác
nhau.

## Luồng `ccc lite`

Đây là lệnh chính, đọc [`src/commands/lite.rs`](../src/commands/lite.rs):

```
1. Kiểm tra ~/.ccc/.claude/ có tồn tại không
       ↓  (không có → báo lỗi, bảo chạy script cài)
2. Nếu ./.claude/settings.local.json đã có → hỏi ghi đè
       ↓
3. Hỏi virtual key (nhập ẩn)
       ↓
4. Copy ~/.ccc/.claude/  →  ./.claude/
       ↓
5. Đọc ./.claude/settings.local.json, ghi thêm vào phần "env":
       - ANTHROPIC_BASE_URL
       - ANTHROPIC_AUTH_TOKEN          (và xoá ANTHROPIC_API_KEY)
       - CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY = "1"
       - 5 biến model, tất cả đều là tên -vn
       ↓
6. Thêm ".claude/" vào .gitignore nếu là git repo
```

Bước 5 phải pin **đủ cả 5 biến model**. Bỏ trống biến nào thì Claude Code rơi về
id built-in tương ứng và nhận 403. Điều này áp dụng cả cho
`ANTHROPIC_SMALL_FAST_MODEL` — biến dùng cho các việc chạy nền.

## Luồng kiểm tra key

Cả `ccc check`, `ccc models` và `ccc key status` đều gọi
`GET {base_url}/v1/models` chứ không phải `POST /v1/messages`.

Lý do: `/v1/models` không cần quyền với model cụ thể nào. Nếu dùng
`/v1/messages` với một model mà key không có quyền, ta sẽ nhận 403 và tưởng nhầm
là key hỏng — trong khi key hoàn toàn tốt. Ngoài ra `/v1/models` chính là
endpoint mà Claude Code gọi khi bật model discovery, nên kết quả phản ánh đúng
những gì bạn sẽ thấy trong picker.

## Kiến trúc TUI

TUI dùng thư viện [ratatui](https://ratatui.rs). Cấu trúc theo kiểu reducer,
giống Redux:

```
Người dùng bấm phím
       ↓
App::handle_event(key)
       ↓
Component đang active xử lý → trả về một Action
       ↓
App::process_action(action)
       ↓
   ┌───┴────┬──────────┬─────────┐
   ↓        ↓          ↓         ↓
 Quit    Notify   Op(KeyOp)  Switch(ModeSwitch)
                     ↓            ↓
              đổi dữ liệu    đổi màn hình
                     ↓            ↓
              App::render(frame) vẽ lại toàn bộ
```

Điểm mấu chốt: **component không tự thay đổi dữ liệu**. Chúng chỉ trả về một
`Action` mô tả việc cần làm. Chỉ `App` mới thực sự ghi file, sửa `KeysStore`, đổi
màn hình. Nhờ vậy mọi thay đổi trạng thái đều tập trung ở một chỗ, dễ theo dõi.

`Mode` là enum liệt kê các màn hình có thể có
(xem [`src/tui/app.rs`](../src/tui/app.rs)):

```
Home  →  KeyDashboard  →  Normal (bảng key)
                              ↓
              AddName / AddValue / Rename / ConfirmRemove / Status / Message
```

## Quy tắc cần giữ

1. **`default/.claude/settings.local.json` không bao giờ chứa key.** File này
   được git theo dõi, key lọt vào là lên GitHub.
2. **Mọi giá trị bí mật phải qua `mask_key()` hoặc `redact_secrets()` trước khi in.**
3. **Không ghi `ANTHROPIC_API_KEY` mới.** Dùng `set_auth_token()`.
4. **Tên model phải kết thúc bằng `-vn`.** Hàm `is_vn_model()` trong `api.rs` là
   nơi kiểm tra điều này.
