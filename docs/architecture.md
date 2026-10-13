# Kiến trúc code

## Cấu trúc thư mục

```
ccc/
├── Cargo.toml                  Khai báo project + danh sách thư viện
├── Cargo.lock                  Phiên bản chính xác của mọi thư viện (commit vào git)
│
├── src/                        Toàn bộ mã nguồn
│   ├── main.rs                 Điểm vào. Định nghĩa CLI, điều hướng sang lệnh
│   ├── config.rs               Đường dẫn file, đọc/ghi JSON, hằng số, KeysStore, CccSettings
│   ├── api.rs                  Gọi gateway: lấy danh sách model, kiểm tra key, bộ lọc model
│   ├── slots.rs                Các slot model: đọc/ghi, gợi ý model, quy tắc [1m] (không I/O)
│   ├── checks.rs               Các bước của `ccc check` dưới dạng dữ liệu (CLI và TUI dùng chung)
│   ├── keys.rs                 Thao tác với key: add/default/use/remove/rename (CLI và TUI dùng chung)
│   ├── project.rs              Tình trạng thư mục hiện tại: key, gateway, slot, discovery
│   ├── release.rs              Bản release mới nhất trên GitHub, thông báo có bản mới khi khởi động
│   ├── ui.rs                   Hàm vẽ khung, dòng kẻ, dấu tick cho terminal
│   ├── utils.rs                Hàm dùng chung: nhập liệu, copy thư mục, che key
│   │
│   ├── commands/               Mỗi file = một lệnh con của `ccc`
│   │   ├── mod.rs              Khai báo các file trong thư mục này
│   │   ├── lite.rs             `ccc lite`
│   │   ├── init.rs             `ccc init [--base-url]`
│   │   ├── key.rs              `ccc key ...`
│   │   ├── config.rs           `ccc config ...`
│   │   ├── check.rs            `ccc check` (in báo cáo của checks.rs)
│   │   ├── models.rs           `ccc models [list|set]`
│   │   ├── pick.rs             `ccc models pick`: chọn model bằng phím mũi tên, không chiếm màn hình
│   │   ├── doctor.rs           `ccc doctor`
│   │   ├── permission.rs       `ccc permission`
│   │   ├── show.rs             `ccc show`
│   │   ├── shell.rs            `ccc` không tham số: vòng lặp tương tác
│   │   ├── splash.rs           Banner + danh sách lệnh của shell
│   │   ├── update.rs           `ccc update`
│   │   ├── uninstall.rs        `ccc uninstall`
│   │   ├── version.rs          `ccc version`
│   │   └── completions.rs      `ccc completions`
│   │
│   └── tui/                    Giao diện toàn màn hình (`ccc ui`, `ccc key`)
│       ├── mod.rs
│       ├── app.rs              Bộ điều phối: Screen, Action, overlay, test vẽ màn hình
│       ├── event.rs            Mở/đóng terminal, vòng lặp đọc phím
│       ├── input.rs            Ô nhập liệu (theo char, an toàn với tiếng Việt)
│       ├── theme.rs            Màu sắc, ký hiệu một cột
│       ├── widgets.rs          Khung trang, thanh phím tắt, cắt chữ theo độ rộng
│       ├── screens/            Mỗi file = một màn
│       │   ├── home.rs             Tình trạng thư mục, Getting started, menu
│       │   ├── keys.rs             Bảng key + chi tiết key đang chọn
│       │   ├── models.rs           Chọn model cho từng slot (lấy danh sách ở thread riêng)
│       │   ├── config.rs           Config Claude Code đọc ở thư mục này
│       │   ├── check.rs            Báo cáo của checks.rs
│       │   ├── status.rs           Test tất cả key song song
│       │   └── help.rs             Bảng `?`
│       └── components/         Hộp thoại dùng chung
│           ├── input_modal.rs      Hộp nhập (có chế độ ẩn cho key)
│           ├── confirm_modal.rs    Hộp hỏi y/n
│           └── toast.rs            Thông báo kết quả
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
├── ccc.json                            tuỳ chọn của ccc: { model_filter } (có thể chưa có)
├── update-check.json                   lần cuối hỏi GitHub bản mới nhất: { checked_at, latest }
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
4. GET {gateway team}/v1/models bằng key đó
       ↓  (gateway từ chối key → dừng, không ghi gì;
       ↓   không liên lạc được → cảnh báo, đi tiếp với model mặc định)
5. Copy ~/.ccc/.claude/  →  ./.claude/
       ↓
6. Đọc ./.claude/settings.local.json, ghi thêm vào phần "env":
       - ANTHROPIC_BASE_URL
       - ANTHROPIC_AUTH_TOKEN          (và xoá ANTHROPIC_API_KEY)
       - CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY = "1"
       - 4 slot model (LITE_MODELS) + ANTHROPIC_SMALL_FAST_MODEL = slot Haiku
       - slots::fit(): slot nào key không dùng được thì đổi sang model gần nhất
       ↓
7. Thêm ".claude/settings.local.json" vào .gitignore nếu là git repo
   (bỏ qua nếu .gitignore đã che file đó, hoặc đã che cả .claude/)
```

Bước 6 phải điền **đủ 4 slot bắt buộc** (main, sonnet, opus, haiku). Bỏ trống
slot nào thì alias tương ứng trong `/model` rơi về id built-in và nhận 403.
`ANTHROPIC_SMALL_FAST_MODEL` đã deprecated nhưng vẫn được ghi cho Claude Code bản
cũ, luôn bằng slot Haiku (`slots::write` tự đồng bộ).

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

`Screen` là enum các màn (xem [`src/tui/app.rs`](../src/tui/app.rs)); trên mỗi
màn có thể có một overlay:

```
Home ─┬─ Keys ──── (s) Test every key
      ├─ Models    (lấy danh sách model ở thread riêng, poll mỗi lần vẽ)
      ├─ Config
      └─ Check     (chạy checks::run() ở thread riêng)

Overlay: Input (thêm/đổi tên key, model Custom) · Confirm (y/n) · Toast · Help (?)
```

Mọi màn vẽ trong `widgets::page()`: khung, tiêu đề, và thanh phím tắt ở dòng cuối.
Thanh phím tắt bỏ bớt mục từ cuối khi hết chỗ nhưng luôn giữ `? help`. Chữ không
vừa thì cắt có dấu "…" theo độ rộng hiển thị, không bao giờ bị viền cắt mất.

TUI có test vẽ bằng `TestBackend` (cuối `app.rs` và `screens/models.rs`): mọi
màn được vẽ ở 64×20, 80×24, 100×30, 160×48 và kiểm tra viền liền, còn `? help`,
key không bị lộ.

## Quy tắc cần giữ

1. **`default/.claude/settings.local.json` không bao giờ chứa key.** File này
   được git theo dõi, key lọt vào là lên GitHub.
2. **Mọi giá trị bí mật phải qua `mask_key()` hoặc `redact_secrets()` trước khi in.**
3. **Không ghi `ANTHROPIC_API_KEY` mới.** Dùng `set_auth_token()`.
4. **"Model dùng được" = gateway liệt kê cho key + qua bộ lọc.** Bộ lọc là `-vn`
   trên gateway của team, không lọc với gateway khác, hoặc giá trị
   `model_filter` trong `~/.ccc/ccc.json`. Dùng `api::model_filter_for()`,
   `api::matches_filter()` và `api::usable_models()`; đừng tự so hậu tố.
5. **Mọi so sánh với danh sách model của gateway phải đi qua `base_model_id()`.**
   Marker `[1m]` là client-side, gateway không bao giờ trả về tên có ngoặc vuông
   — so khớp nguyên chuỗi sẽ báo lệch giả.
6. **Model mặc định nằm ở hai nơi và phải khớp nhau:**
   `default/.claude/settings.local.json` (file `ccc init` copy) và mảng
   `LITE_MODELS` trong `commands/lite.rs` (ghi đè file đó khi chạy `ccc lite`).
   Test `lite_defaults_match_the_shipped_template` sẽ fail nếu hai nơi lệch nhau.
7. **Slot model chỉ đọc/ghi qua `slots.rs`** (`slots::read`, `slots::write`,
   `slots::fit`), để biến legacy `ANTHROPIC_SMALL_FAST_MODEL` luôn theo slot Haiku.
8. **TUI không in ra stdout.** Trong lúc TUI chiếm màn hình, mọi thông báo đi qua
   Toast; vì vậy có `utils::ensure_local_settings_ignored()` (trả về chuỗi) bên
   cạnh `ignore_local_settings()` (in ra).
