use console::{style, Emoji};
use indicatif::{ProgressBar, ProgressStyle};

// ── Cross-platform icons ──

pub static ICON_OK:    Emoji = Emoji("✅", "[OK]");
pub static ICON_FAIL:  Emoji = Emoji("❌", "[X]");
pub static ICON_SEARCH:Emoji = Emoji("🔍", "(?)");
pub static ICON_DOC:   Emoji = Emoji("📋", "(i)");

// ── Box drawing ──

pub fn print_header(icon: &Emoji, title: &str) {
    let width = 50;
    let border: String = "─".repeat(width);
    println!("  ╭{border}╮");
    println!("  │  {} {:<w$}│", icon, style(title).cyan().bold(), w = width - 4);
    println!("  ├{border}┤");
}

pub fn print_separator() {
    let border: String = "─".repeat(50);
    println!("  ├{border}┤");
}

pub fn print_footer() {
    let border: String = "─".repeat(50);
    println!("  ╰{border}╯");
}

pub fn print_row(label: &str, value: &str) {
    println!("  │  {:<12} {:<w$}│", style(label).dim(), value, w = 50 - 15);
}

pub fn print_check(ok: bool, label: &str, detail: &str) {
    let icon = if ok { &ICON_OK } else { &ICON_FAIL };
    let styled_label = if ok {
        style(label).green().to_string()
    } else {
        style(label).red().to_string()
    };
    let detail_styled = if ok {
        style(detail).to_string()
    } else {
        style(detail).red().to_string()
    };
    println!("  │  {} {:<16} {:<w$}│", icon, styled_label, detail_styled, w = 50 - 21);
}

pub fn print_result_line(pass: usize, fail: usize) {
    let total = pass + fail;
    let result_text = if fail == 0 {
        style(format!("All {total} checks passed")).green().bold().to_string()
    } else {
        format!(
            "{} {}",
            style(format!("{pass}/{total} passed")).green().bold(),
            style(format!("{fail} failed")).red().bold(),
        )
    };
    println!("  │  {:<w$}│", result_text, w = 50 - 2);
}

// ── Spinner ──

pub fn spinner(msg: &str) -> ProgressBar {
    let sp = ProgressBar::new_spinner();
    sp.set_style(
        ProgressStyle::with_template("  │  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✓"]),
    );
    sp.set_message(msg.to_string());
    sp.enable_steady_tick(std::time::Duration::from_millis(80));
    sp
}
