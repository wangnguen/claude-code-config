# Hướng dẫn phát triển

## Chuẩn bị môi trường

Cần Rust toolchain. Cài bằng [rustup](https://rustup.rs):

```powershell
winget install Rustlang.Rustup
```

Trên Windows, Rust mặc định dùng toolchain MSVC, cần Visual Studio Build Tools
(khoảng 2–4 GB). Muốn nhẹ hơn thì dùng toolchain GNU:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup default stable-x86_64-pc-windows-gnu
winget install -e --id BrechtSanders.WinLibs.POSIX.MSVCRT
```

WinLibs cung cấp `gcc.exe` và `dlltool.exe` mà toolchain GNU cần. Nhớ thêm thư
mục `mingw64\bin` của nó vào PATH.

## Vấn đề với phần mềm diệt virus

Trên máy công ty cài Symantec Endpoint Protection, `cargo build` sẽ thất bại:

```
error: failed to run custom build command for `rustls`
Caused by: Access is denied. (os error 5)
```

Symantec cách ly `build-script-build.exe` với nhãn `Heur.AdvML.B`. Đây là
**false positive**: build script của Rust là file thực thi vừa được compile, chưa
ký số, và chạy ngay lập tức — đúng đặc điểm mà engine heuristic chấm là đáng ngờ.
Mọi project Rust có dependency native đều gặp.

Ba cách xử lý:

### Cách 1 — Build trong Docker (khuyên dùng)

Không cần đụng tới antivirus:

```powershell
docker run --rm -v "${PWD}:/app" -w /app rust:1.98 cargo check --all-targets
```

Container là môi trường Linux nên Symantec không quét vào trong. Kết quả
`cargo check` vẫn bắt được đầy đủ lỗi cú pháp và lỗi kiểu.

Hạn chế: không tạo ra được binary Windows. Muốn có `ccc.exe` thì dùng cách 3.

### Cách 2 — Xin IT thêm exclusion

Yêu cầu thêm ngoại lệ cho:

```
%USERPROFILE%\.cargo\
<đường dẫn project>\target\
```

Đây là yêu cầu chính đáng và tiêu chuẩn cho môi trường phát triển Rust. Nếu muốn
build local lâu dài thì sớm muộn cũng cần.

### Cách 3 — Để GitHub Actions build

Push lên `main`, CI sẽ build cả 4 nền tảng và tạo release. Tải binary về từ đó.

## Quy trình sửa code

```bash
cargo check      # sau mỗi lần sửa — nhanh, chỉ kiểm lỗi
cargo fmt        # format lại trước khi commit
cargo clippy     # gợi ý cải thiện (tuỳ chọn)
```

Trong Docker:

```powershell
docker run --rm -v "${PWD}:/app" -w /app rust:1.98 cargo check --all-targets
```

## Cách thêm một lệnh mới

Ví dụ thêm lệnh `ccc hello`.

### Bước 1 — Tạo file

`src/commands/hello.rs`:

```rust
use anyhow::Result;

use crate::ui;

pub fn run() -> Result<()> {
    println!();
    ui::print_header(&ui::ICON_DOC, "Hello");
    ui::print_row("Message", "xin chào");
    ui::print_footer();
    println!();
    Ok(())
}
```

### Bước 2 — Khai báo module

Thêm vào `src/commands/mod.rs`, giữ thứ tự alphabet:

```rust
pub mod hello;
```

Bỏ qua bước này sẽ bị lỗi `file is not included in the module tree`.

### Bước 3 — Khai báo lệnh

Trong `src/main.rs`, thêm vào enum `Commands`:

```rust
/// Say hello
Hello,
```

Dòng bắt đầu bằng `///` là mô tả hiển thị trong `ccc --help`. Bắt buộc phải có.

### Bước 4 — Điều hướng

Vẫn trong `src/main.rs`, thêm vào khối `match`:

```rust
Some(Commands::Hello) => commands::hello::run()?,
```

Nếu quên bước này, compiler sẽ báo `match` thiếu nhánh — đó là điểm mạnh của Rust,
nó không cho bạn quên.

### Bước 5 — Kiểm tra

```bash
cargo run -- hello
cargo run -- --help
```

## Quy ước code

### Hàm vẽ giao diện

Dùng các hàm trong [`src/ui.rs`](../src/ui.rs), đừng tự `println!` khung viền:

```rust
ui::print_header(&ui::ICON_SEARCH, "Tiêu đề");
ui::print_row("Nhãn", "giá trị");
ui::print_separator();
ui::print_check(true, "Mục kiểm tra", "chi tiết");   // true = ✅, false = ❌
ui::print_result_line(pass, fail);
ui::print_footer();
```

Với thao tác chờ lâu:

```rust
let sp = ui::spinner("Đang xử lý...");
// ... làm việc ...
sp.finish_and_clear();
```

### Xử lý giá trị bí mật

Không bao giờ in thẳng API key:

```rust
use crate::utils::{mask_key, redact_secrets};

println!("{}", mask_key(&key));            // sk-a...z9f
print_json_pretty(&redact_secrets(&json)); // che mọi trường có TOKEN/KEY trong tên
```

### Ghi key vào settings

Luôn dùng `set_auth_token()`, đừng gán trực tiếp:

```rust
use crate::config::set_auth_token;

set_auth_token(&mut json, &token);   // ghi AUTH_TOKEN và xoá API_KEY cũ
```

### Tên model

Mọi tên model phải kết thúc bằng `-vn`, có thể theo sau bởi marker context
`[1m]` (vd `claude-sonnet-5-vn[1m]`). Kiểm tra bằng:

```rust
use crate::api::is_vn_model;

if !is_vn_model(&model) {
    // model này sẽ nhận 403
}
```

`is_vn_model()` tự strip `[1m]` nên nhận cả hai dạng.

Khi **so khớp với danh sách model gateway trả về**, phải strip marker trước —
gateway chỉ biết id trần:

```rust
use crate::api::base_model_id;

let allowed = models.iter().any(|m| m == base_model_id(&model));
```

Dùng `m == model` trực tiếp sẽ báo "not in your key's model list" giả với mọi
config có `[1m]`.

## Quy trình release

CI được định nghĩa trong [`.github/workflows/release.yml`](../.github/workflows/release.yml).
Mỗi lần push lên `main`:

```
1. Job "check"
   └─ cargo check --all-targets trên Ubuntu
      Thất bại → dừng, không tạo tag, không release
        ↓
2. Job "bump-and-release"
   ├─ Đọc tag mới nhất, tăng số patch (0.1.26 → 0.1.27)
   ├─ Sửa version trong Cargo.toml
   ├─ Commit "bump v0.1.27" và push lên main
   └─ Tạo tag v0.1.27
        ↓
3. Job "build" (chạy song song 4 nền tảng)
   ├─ x86_64-pc-windows-msvc
   ├─ x86_64-unknown-linux-gnu
   ├─ x86_64-apple-darwin
   └─ aarch64-apple-darwin
   Mỗi bản đóng gói thành ccc-<target>.zip, kèm thư mục default/.claude/
        ↓
4. Job "release"
   └─ Tạo GitHub Release và đính kèm cả 4 file zip
```

Commit có nội dung chứa `bump v` sẽ bị bỏ qua, nhờ đó commit do CI tự tạo không
kích hoạt lại workflow gây vòng lặp vô hạn.

Version **không** được sửa bằng tay trong `Cargo.toml` — CI tự tăng.

`Cargo.lock` sẽ lệch version so với `Cargo.toml` sau mỗi lần bump, vì bước bump
chỉ sửa `Cargo.toml`. Đó là lý do CI chạy `cargo check` mà không dùng cờ
`--locked`.

## Danh sách kiểm tra trước khi commit

- [ ] `cargo check` không có lỗi
- [ ] `cargo fmt` đã chạy
- [ ] Không có API key nào lọt vào file được git theo dõi
- [ ] `default/.claude/settings.local.json` vẫn không chứa key
- [ ] Nếu thêm lệnh mới: đã cập nhật `README.md` và [usage.md](usage.md)
