# Tài liệu `ccc`

`ccc` là CLI viết bằng Rust để cấu hình Claude Code cho team: quản lý virtual API
key và ghi file `.claude/settings.local.json` cho từng project.

| Tài liệu | Dành cho ai | Nội dung |
|---|---|---|
| [usage.md](usage.md) | Mọi thành viên | Cài đặt, từng lệnh, xử lý lỗi 403 và model `-vn` |
| [architecture.md](architecture.md) | Người sửa code | Cấu trúc thư mục, file nào làm gì, dữ liệu nằm ở đâu |
| [rust-primer.md](rust-primer.md) | Người chưa biết Rust | Đủ Rust để đọc và sửa được repo này |
| [development.md](development.md) | Người sửa code | Build, test, release, cách thêm lệnh mới |

## Đọc theo thứ tự nào

- **Chỉ dùng, không sửa code** → đọc [usage.md](usage.md), hết.
- **Muốn sửa code nhưng chưa biết Rust** → [rust-primer.md](rust-primer.md), rồi
  [architecture.md](architecture.md), rồi [development.md](development.md).
- **Đã biết Rust** → [architecture.md](architecture.md) là đủ để bắt đầu.
