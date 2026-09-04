# Rust vừa đủ để sửa repo này

Tài liệu này không dạy Rust từ đầu. Nó giải thích đúng những cú pháp xuất hiện
trong `ccc`, kèm ví dụ lấy thẳng từ code. Đọc xong là sửa được.

## 1. Kiểu chuỗi: `String` và `&str`

Rust có hai kiểu chuỗi, đây là thứ gây bối rối nhất lúc đầu.

| Kiểu | Là gì | Ví von |
|---|---|---|
| `&str` | Chuỗi **mượn**, chỉ đọc | Con trỏ tới chuỗi của người khác |
| `String` | Chuỗi **sở hữu**, sửa được | Chuỗi bạn tự cấp phát |

```rust
// src/api.rs
pub const VN_SUFFIX: &str = "-vn";        // chuỗi hằng → &str

pub fn is_vn_model(id: &str) -> bool {    // nhận &str: chỉ đọc, không lấy quyền sở hữu
    id.to_lowercase().ends_with(VN_SUFFIX)
}                                          // to_lowercase() tạo String mới
```

**Quy tắc thực dụng:** tham số hàm dùng `&str`, giá trị trả về dùng `String`.
Chuyển đổi:

```rust
let s: String = "xin chào".to_string();   // &str → String
let r: &str = &s;                          // String → &str (tự động)
```

Khi gọi hàm nhận `&str` mà bạn đang có `String`, Rust tự chuyển. Không cần làm gì.

## 2. `Option`: giá trị có thể không tồn tại

Rust không có `null`. Thay vào đó dùng `Option<T>`, có đúng hai trạng thái:

```rust
Some(giá_trị)   // có
None            // không có
```

```rust
// src/commands/check.rs
let api_key = match get_current_key() {
    Some(key) => key,                                    // có key → dùng
    None => bail!("API key not set. Run 'ccc key add' first."),  // không có → thoát
};
```

Compiler **bắt buộc** bạn xử lý cả hai nhánh. Đó là lý do Rust không có lỗi
null pointer.

Cách viết ngắn khi chỉ quan tâm trường hợp có:

```rust
// src/config.rs
if let Some(env) = json["env"].as_object_mut() {
    env.remove(ENV_API_KEY);
}
// đọc là: "nếu as_object_mut() trả về Some, đặt tên nó là env rồi làm việc này"
```

## 3. `Result` và dấu `?`: xử lý lỗi

`Result<T>` cũng có hai trạng thái:

```rust
Ok(giá_trị)    // thành công
Err(lỗi)       // thất bại
```

Dấu `?` là cách viết tắt cực kỳ phổ biến:

```rust
// src/commands/lite.rs
pub fn run() -> Result<()> {
    let source = default_claude_dir()?;   // ← dấu ? ở đây
    ...
}
```

Dòng đó có nghĩa:

> Gọi `default_claude_dir()`. Nếu `Ok`, lấy giá trị bên trong gán cho `source`.
> Nếu `Err`, **dừng hàm ngay và trả lỗi đó ra ngoài**.

Không có `?` thì phải viết dài dòng:

```rust
let source = match default_claude_dir() {
    Ok(v) => v,
    Err(e) => return Err(e),
};
```

`Result<()>` nghĩa là "thành công thì không trả về gì". `()` đọc là "unit", tương
đương `void`.

Hàm có dùng `?` thì **bắt buộc** phải trả về `Result`.

### `bail!` và `.context()`

Repo dùng thư viện `anyhow`:

```rust
bail!("A virtual API key is required.");   // dừng ngay với thông báo lỗi này

copy_dir_recursive(&source, target)
    .context("Failed to copy .claude folder")?;   // thêm ngữ cảnh vào lỗi gốc
```

## 4. Mượn: `&` và `&mut`

Đây là đặc trưng riêng của Rust. Mỗi giá trị có đúng một chủ sở hữu. Muốn dùng
mà không lấy quyền sở hữu thì **mượn**:

```rust
&x       // mượn để đọc     — được nhiều người mượn cùng lúc
&mut x   // mượn để sửa     — chỉ một người tại một thời điểm
```

```rust
// src/config.rs
pub fn set_auth_token(json: &mut Value, token: &str) {
    //                       ^^^^ sửa được       ^ chỉ đọc
    json["env"][ENV_AUTH_TOKEN] = Value::String(token.to_string());
}
```

Gọi hàm này:

```rust
let mut json = read_json(&path)?;      // mut = biến này sẽ bị sửa
set_auth_token(&mut json, &token);     // cho mượn quyền sửa
```

Nếu quên `mut` khi khai báo biến, compiler báo lỗi. Cứ thêm vào là xong.

## 5. `struct`: gom dữ liệu

```rust
// src/config.rs
#[derive(Serialize, Deserialize, Default)]
pub struct KeysStore {
    pub active: Option<String>,              // tên key đang mặc định
    pub keys: BTreeMap<String, String>,      // map tên → giá trị key
}
```

`BTreeMap<K, V>` là map sắp xếp theo khoá, tương đương `dict` trong Python nhưng
luôn có thứ tự ổn định.

### `#[derive(...)]` là gì

Đó là **macro sinh code tự động**. Dòng trên bảo compiler:

| Derive | Sinh ra |
|---|---|
| `Serialize` | Code chuyển struct → JSON |
| `Deserialize` | Code chuyển JSON → struct |
| `Default` | Hàm tạo ra giá trị rỗng mặc định |

Không có nó thì phải viết tay hàng chục dòng.

## 6. `impl`: gắn hàm vào struct

```rust
impl KeysStore {
    pub fn load() -> Self {          // KHÔNG có self → gọi qua tên kiểu
        ...
    }

    pub fn save(&self) -> Result<()> {   // CÓ &self → gọi qua biến
        ...
    }
}
```

Cách gọi khác nhau:

```rust
let store = KeysStore::load();   // hai dấu :: vì load() không có self
store.save()?;                    // dấu chấm vì save() nhận &self
```

`Self` (chữ S hoa) là bí danh của chính kiểu đó, ở đây là `KeysStore`.

Ba dạng `self`:

| Viết | Nghĩa |
|---|---|
| `&self` | Chỉ đọc dữ liệu của struct |
| `&mut self` | Sửa dữ liệu của struct |
| `self` | Lấy luôn quyền sở hữu (ít gặp) |

## 7. `enum`: một trong nhiều khả năng

Enum của Rust mạnh hơn enum của các ngôn ngữ khác nhiều — mỗi nhánh có thể mang
dữ liệu riêng:

```rust
// src/tui/component.rs
pub enum Action {
    None,                                          // không mang gì
    Quit,
    Notify { text: String, success: bool },        // mang struct
    Op(KeyOp),                                     // mang một giá trị
    Switch(ModeSwitch),
}
```

Xử lý bằng `match`:

```rust
// src/tui/app.rs
match action {
    Action::None => {}
    Action::Quit => self.should_quit = true,
    Action::Notify { text, success } => {
        self.mode = Mode::Message(Toast { text, success });
    }
    Action::Op(op) => self.execute_op(op),
    Action::Switch(switch) => self.switch_mode(switch),
}
```

`match` **bắt buộc liệt kê đủ mọi nhánh**. Thêm một biến thể mới vào enum mà quên
xử lý ở đâu đó, compiler sẽ chỉ ra ngay. Đây là lý do enum của Rust rất an toàn
khi refactor.

Dùng `_ => {}` nếu muốn gom phần còn lại.

## 8. Iterator và closure

```rust
// src/commands/check.rs
let vn: Vec<&String> = models.iter().filter(|m| is_vn_model(m)).collect();
```

Đọc từ trái sang phải:

| Đoạn | Nghĩa |
|---|---|
| `models.iter()` | Duyệt qua từng phần tử (mượn, không sở hữu) |
| `.filter(|m| ...)` | Giữ lại phần tử nào thoả điều kiện |
| `\|m\| is_vn_model(m)` | Closure — hàm ẩn danh, `m` là tham số |
| `.collect()` | Gom kết quả lại thành `Vec` |

`|m| ...` chính là lambda. Tương đương `lambda m: ...` của Python.

Vài hàm hay gặp trong repo:

```rust
.map(|x| ...)         // biến đổi từng phần tử
.filter(|x| ...)      // lọc
.any(|x| ...)         // có phần tử nào thoả không → bool
.count()              // đếm
.filter_map(|x| ...)  // vừa lọc vừa biến đổi, bỏ qua None
```

## 9. Module: `mod`, `use`, `pub`

Mỗi file `.rs` là một module. Phải **khai báo** thì mới dùng được:

```rust
// src/main.rs — khai báo các module cấp cao nhất
mod api;
mod commands;
mod config;
mod tui;
mod ui;
mod utils;
```

```rust
// src/commands/mod.rs — khai báo các file trong thư mục commands/
pub mod check;
pub mod models;
...
```

`use` là để gọi cho ngắn:

```rust
// src/commands/check.rs
use crate::api::{get_current_key, is_vn_model, list_models};

// nhờ dòng trên, viết được:
let key = get_current_key();
// thay vì:
let key = crate::api::get_current_key();
```

`crate` nghĩa là gốc của project này (`src/`).

`pub` = công khai, module khác dùng được. Không có `pub` = riêng tư trong file.

### Thêm một file mới cần làm gì

Tạo `src/commands/foo.rs` xong **chưa đủ**. Phải thêm vào `src/commands/mod.rs`:

```rust
pub mod foo;
```

Quên bước này thì compiler báo "file is not included in module tree".

## 10. Đọc thông báo lỗi

Lỗi Rust dài nhưng rất cụ thể. Ví dụ thật:

```
error[E0308]: mismatched types
  --> src/commands/check.rs:42:30
   |
42 |     ui::print_row("Config", path);
   |     ------------            ^^^^ expected `&str`, found `&PathBuf`
   |
help: try using a conversion method
   |
42 |     ui::print_row("Config", &path.display().to_string());
```

Cách đọc:

1. `-->` chỉ đúng file và dòng
2. `expected X, found Y` nói bạn đưa sai kiểu
3. `help:` thường cho luôn cách sửa — làm theo là được

Ba lỗi hay gặp nhất khi mới bắt đầu:

| Lỗi | Cách sửa |
|---|---|
| `expected &str, found String` | Thêm `&` phía trước |
| `cannot borrow as mutable` | Thêm `mut` khi khai báo biến |
| `value used after move` | Thêm `.clone()` hoặc dùng `&` để mượn thay vì sở hữu |

## 11. Lệnh cần nhớ

```bash
cargo check     # kiểm tra lỗi, KHÔNG tạo file chạy — nhanh nhất, dùng khi đang sửa
cargo build     # biên dịch ra bản debug
cargo build --release   # biên dịch bản tối ưu, chậm hơn nhưng chạy nhanh
cargo run -- doctor     # chạy thử, phần sau `--` là tham số cho ccc
cargo fmt       # tự động format code
cargo clippy    # gợi ý viết code tốt hơn
```

Khi đang sửa code, chạy `cargo check` liên tục. Nó nhanh hơn `cargo build` nhiều
vì không sinh file thực thi.

Trên máy công ty có Symantec chặn build, xem [development.md](development.md) để
biết cách chạy qua Docker.

## 12. Ví dụ hoàn chỉnh: đọc một hàm thật

```rust
// src/api.rs
pub fn list_models(api_key: &str) -> Result<Vec<String>, String> {
    let (base_url, _) = get_api_config();
    let url = format!("{base_url}/v1/models");

    let result = ureq::get(&url)
        .header("Authorization", &format!("Bearer {api_key}"))
        .call();

    match result {
        Ok(mut resp) => {
            let text = resp.body_mut().read_to_string().unwrap_or_default();
            let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            match json["data"].as_array() {
                Some(items) => Ok(items
                    .iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_string))
                    .collect()),
                None => Err("unexpected response".to_string()),
            }
        }
        Err(e) => Err(format!("{e}")),
    }
}
```

Giải thích từng phần:

| Dòng | Nghĩa |
|---|---|
| `Result<Vec<String>, String>` | Thành công trả danh sách chuỗi, thất bại trả thông báo lỗi |
| `let (base_url, _) = ...` | Hàm trả về cặp giá trị, `_` nghĩa là "bỏ qua giá trị thứ hai" |
| `format!("{base_url}/v1/models")` | Ghép chuỗi, biến trong ngoặc nhọn được thay thẳng vào |
| `.header(...).call()` | Gọi liên tiếp, mỗi hàm trả về đối tượng để gọi tiếp |
| `unwrap_or_default()` | Nếu lỗi thì dùng giá trị rỗng thay vì crash |
| `filter_map(...)` | Lấy trường `id` của từng phần tử, bỏ qua phần tử không có |
| `Ok(...)` / `Err(...)` | Gói kết quả lại trước khi trả về |

Đọc hiểu được hàm này là đã đủ nền để sửa hầu hết code trong repo.
