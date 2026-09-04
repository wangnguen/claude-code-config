use console::Emoji;
use ratatui::prelude::Color;

// ── Cross-platform icons (emoji with ASCII fallback) ──

pub static ICON_KEY:    Emoji = Emoji("🔑 ", "(K) ");
pub static ICON_SEARCH: Emoji = Emoji("🔍 ", "(?) ");
pub static ICON_OK:     Emoji = Emoji("✅", "[OK]");
pub static ICON_FAIL:   Emoji = Emoji("❌", "[X]");
pub static ICON_WAIT:   Emoji = Emoji("⏳", "[..]");
pub static ICON_STAR:   Emoji = Emoji("★", "*");
pub static ICON_PLAY:   Emoji = Emoji("▶", ">");
pub static ICON_CHECK:  Emoji = Emoji("✓", "+");
pub static ICON_CROSS:  Emoji = Emoji("✗", "x");

// ── Theme colors ──

pub const BG: Color = Color::Rgb(15, 15, 25);
pub const PANEL_BG: Color = Color::Rgb(22, 22, 35);
pub const MODAL_BG: Color = Color::Rgb(28, 28, 48);
pub const BORDER: Color = Color::Rgb(50, 50, 75);
pub const ACCENT: Color = Color::Rgb(130, 190, 255);
pub const DIM: Color = Color::Rgb(100, 100, 120);
pub const TEXT: Color = Color::Rgb(180, 180, 200);
pub const TEXT_DIM: Color = Color::Rgb(120, 120, 140);
pub const HIGHLIGHT_BG: Color = Color::Rgb(35, 40, 65);
pub const SUCCESS: Color = Color::Rgb(80, 220, 120);
pub const ERROR: Color = Color::Rgb(255, 100, 100);
pub const ROW_ALT: Color = Color::Rgb(25, 25, 40);
