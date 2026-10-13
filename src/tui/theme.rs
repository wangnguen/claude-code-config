use ratatui::prelude::Color;

// ── Symbols ──
//
// Single-column characters only. Emoji are two columns wide in some terminals
// and one in others, which is exactly how a border ends up one cell off.

pub const SYM_OK: &str = "✓";
pub const SYM_FAIL: &str = "✗";
pub const SYM_WARN: &str = "!";
pub const SYM_INFO: &str = "–";
pub const SYM_STAR: &str = "★";
pub const SYM_HERE: &str = "●";
pub const SYM_POINTER: &str = "▶";
pub const SYM_TODO: &str = "•";
pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// ── Theme colors ──

pub const BG: Color = Color::Rgb(15, 15, 25);
pub const PANEL_BG: Color = Color::Rgb(22, 22, 35);
pub const MODAL_BG: Color = Color::Rgb(28, 28, 48);
pub const BORDER: Color = Color::Rgb(60, 60, 90);
pub const ACCENT: Color = Color::Rgb(130, 190, 255);
pub const DIM: Color = Color::Rgb(110, 110, 135);
pub const TEXT: Color = Color::Rgb(190, 190, 210);
pub const TEXT_DIM: Color = Color::Rgb(135, 135, 155);
pub const HIGHLIGHT_BG: Color = Color::Rgb(38, 44, 72);
pub const SUCCESS: Color = Color::Rgb(80, 220, 120);
pub const WARNING: Color = Color::Rgb(240, 200, 90);
pub const ERROR: Color = Color::Rgb(255, 110, 110);
pub const KEY_HINT: Color = Color::Rgb(255, 210, 120);
