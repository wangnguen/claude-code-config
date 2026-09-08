use console::{colors_enabled, measure_text_width, style, truncate_str, Emoji, Term};
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::OnceLock;

// ── Cross-platform icons ──

pub static ICON_OK:    Emoji = Emoji("✅", "[OK]");
pub static ICON_FAIL:  Emoji = Emoji("❌", "[X]");
pub static ICON_SEARCH:Emoji = Emoji("🔍", "(?)");
pub static ICON_DOC:   Emoji = Emoji("📋", "(i)");

/// Column the value/detail starts at, counted from the box's inner edge.
const LABEL_WIDTH: usize = 16;
/// Widest icon cell: "[OK]" in the ASCII fallback, two columns as an emoji.
const ICON_WIDTH: usize = 4;

// ── Box drawing ──
//
// Every line is padded by display width, never by byte or char count: colours
// add invisible ANSI bytes, an emoji icon takes two columns while its ASCII
// fallback takes three or four, and a path can be longer than the whole box.
// Measuring what the terminal will actually show is the only way the right
// border lands in the same column on every line.

/// Interior width of the box, fixed for the whole run so consecutive lines
/// cannot disagree even if the window is resized mid-command.
fn width() -> usize {
    static WIDTH: OnceLock<usize> = OnceLock::new();
    *WIDTH.get_or_init(|| {
        let columns = Term::stdout().size().1 as usize;
        // 4 columns go to the indent and the two borders.
        columns.saturating_sub(4).clamp(50, 88)
    })
}

fn rule(left: char, right: char) {
    println!("  {left}{}{right}", "─".repeat(width()));
}

/// One row of the box, trimmed or padded to exactly the interior width.
fn row(content: &str) {
    let inner = width();
    println!("  │{}│", pad(&truncate_str(content, inner, "…"), inner));
}

/// Append spaces until `text` occupies `columns` display columns.
fn pad(text: &str, columns: usize) -> String {
    let visible = measure_text_width(text);
    format!("{text}{}", " ".repeat(columns.saturating_sub(visible)))
}

/// Shorten from the middle: for a path the file name at the end identifies it
/// as much as the drive at the start, so cutting the tail off loses the most.
fn shorten(text: &str, columns: usize) -> String {
    if measure_text_width(text) <= columns {
        return text.to_string();
    }
    if columns <= 1 {
        return "…".to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let keep = columns - 1;
    let head = (keep - keep * 2 / 3).min(chars.len());
    let tail = (keep * 2 / 3).min(chars.len() - head);
    let start: String = chars[..head].iter().collect();
    let end: String = chars[chars.len() - tail..].iter().collect();
    format!("{start}…{end}")
}

/// A label column plus whatever room is left for the value.
fn labelled(head: &str, value: &str) -> String {
    let room = width().saturating_sub(measure_text_width(head));
    format!("{head}{}", shorten(value, room))
}

pub fn print_header(icon: &Emoji, title: &str) {
    rule('╭', '╮');
    row(&format!("  {icon} {}", style(title).cyan().bold()));
    rule('├', '┤');
}

pub fn print_separator() {
    rule('├', '┤');
}

/// A plain divider, for screens that are not boxed.
pub fn print_rule() {
    println!("  {}", style("─".repeat(width())).dim());
}

/// Status line under a divider: what to type next, kept out of the way.
pub fn print_hint(text: &str) {
    println!("  {}", style(text).dim());
}

pub fn print_footer() {
    rule('╰', '╯');
}

pub fn print_row(label: &str, value: &str) {
    let head = format!("  {} ", pad(&style(label).dim().to_string(), LABEL_WIDTH));
    row(&labelled(&head, value));
}

pub fn print_check(ok: bool, label: &str, detail: &str) {
    let icon = if ok { &ICON_OK } else { &ICON_FAIL };
    let label = if ok {
        style(label).green().to_string()
    } else {
        style(label).red().to_string()
    };
    // The icon gets a fixed cell: "[X]" is one column narrower than "[OK]",
    // which would otherwise shift the label column row by row.
    let head = format!(
        "  {} {} ",
        pad(&icon.to_string(), ICON_WIDTH),
        pad(&label, LABEL_WIDTH)
    );

    let room = width().saturating_sub(measure_text_width(&head));
    let detail = shorten(detail, room);
    let detail = if ok {
        detail
    } else {
        style(detail).red().to_string()
    };
    row(&format!("{head}{detail}"));
}

pub fn print_result_line(pass: usize, fail: usize) {
    let total = pass + fail;
    let result = if fail == 0 {
        style(format!("All {total} checks passed")).green().bold().to_string()
    } else {
        format!(
            "{} {}",
            style(format!("{pass}/{total} passed")).green().bold(),
            style(format!("{fail} failed")).red().bold(),
        )
    };
    row(&format!("  {result}"));
}

// ── Brand mark ──

/// One letter of the wordmark, '#' being ink. Three of these side by side make
/// "ccc"; two pixel rows share a terminal line as half blocks, so the fourteen
/// rows below print as seven lines.
const GLYPH: [&str; 14] = [
    "  #####  ",
    " ####### ",
    "###   ###",
    "###    ##",
    "###      ",
    "###      ",
    "###      ",
    "###      ",
    "###      ",
    "###      ",
    "###    ##",
    "###   ###",
    " ####### ",
    "  #####  ",
];

const GLYPH_GAP: usize = 2;

/// Top-to-bottom stops. The letters start almost lost in the background and
/// resolve into warm cream at the base, so the wordmark reads as rising out of
/// the terminal rather than sitting on it.
const FADE: [(u8, u8, u8); 4] = [
    (58, 52, 110),
    (98, 84, 190),
    (214, 126, 92),
    (245, 230, 211),
];

/// Sideways tint mixed into the fade, to keep the three letters from reading as
/// one flat block.
const TINT: [(u8, u8, u8); 3] = [(91, 214, 240), (74, 144, 226), (138, 123, 240)];

fn sample(stops: &[(u8, u8, u8)], t: f32) -> (u8, u8, u8) {
    let segments = stops.len() - 1;
    let span = 1.0 / segments as f32;
    let index = ((t / span) as usize).min(segments - 1);
    let local = ((t - index as f32 * span) / span).clamp(0.0, 1.0);
    let (from, to) = (stops[index], stops[index + 1]);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * local).round() as u8;
    (mix(from.0, to.0), mix(from.1, to.1), mix(from.2, to.2))
}

fn pixel_colour(x: usize, y: usize, width: usize, height: usize) -> (u8, u8, u8) {
    let fade = sample(&FADE, y as f32 / (height - 1) as f32);
    let tint = sample(&TINT, x as f32 / (width - 1) as f32);
    // Enough tint to separate the letters, not enough to fight the fade.
    let mix = |a: u8, b: u8| (a as f32 * 0.72 + b as f32 * 0.28).round() as u8;
    (
        mix(fade.0, tint.0),
        mix(fade.1, tint.1),
        mix(fade.2, tint.2),
    )
}

fn mark_rows() -> Vec<String> {
    let gap = " ".repeat(GLYPH_GAP);
    GLYPH
        .iter()
        .map(|row| [*row, *row, *row].join(&gap))
        .collect()
}

fn mark_width() -> usize {
    GLYPH[0].len() * 3 + GLYPH_GAP * 2
}

fn mark_lines() -> Vec<String> {
    let rows = mark_rows();
    let (width, height) = (mark_width(), rows.len());
    let coloured = colors_enabled();

    rows.chunks(2)
        .enumerate()
        .map(|(line_index, pair)| {
            let top: Vec<char> = pair[0].chars().collect();
            let bottom: Vec<char> = pair[1].chars().collect();
            let inked = |row: &Vec<char>, x: usize| row.get(x).is_some_and(|c| *c == '#');
            // Stop at the last inked column so no line carries trailing blanks.
            let end = (0..width)
                .rev()
                .find(|&x| inked(&top, x) || inked(&bottom, x))
                .map_or(0, |x| x + 1);

            let mut line = String::new();
            for x in 0..end {
                let (top_ink, bottom_ink) = (inked(&top, x), inked(&bottom, x));
                if !top_ink && !bottom_ink {
                    line.push(' ');
                    continue;
                }
                if !coloured {
                    line.push(match (top_ink, bottom_ink) {
                        (true, true) => '█',
                        (true, false) => '▀',
                        _ => '▄',
                    });
                    continue;
                }
                // Upper and lower halves sit on different rows of the fade, so
                // each carries its own colour: foreground paints the top half,
                // background the bottom.
                let (y_top, y_bottom) = (line_index * 2, line_index * 2 + 1);
                match (top_ink, bottom_ink) {
                    (true, true) => {
                        let (r, g, b) = pixel_colour(x, y_top, width, height);
                        let (br, bg, bb) = pixel_colour(x, y_bottom, width, height);
                        line.push_str(&format!("\x1b[38;2;{r};{g};{b}m\x1b[48;2;{br};{bg};{bb}m▀"));
                    }
                    (true, false) => {
                        let (r, g, b) = pixel_colour(x, y_top, width, height);
                        line.push_str(&format!("\x1b[49m\x1b[38;2;{r};{g};{b}m▀"));
                    }
                    _ => {
                        let (r, g, b) = pixel_colour(x, y_bottom, width, height);
                        line.push_str(&format!("\x1b[49m\x1b[38;2;{r};{g};{b}m▄"));
                    }
                }
            }
            if coloured {
                line.push_str("\x1b[0m");
            }
            line
        })
        .collect()
}

/// The wordmark plus the caption lines under it, as one block.
fn brand_lines(caption: &str, details: &[&str]) -> Vec<String> {
    let mut lines = mark_lines();
    lines.push(style(caption).dim().to_string());
    lines.extend(details.iter().map(|d| style(d).dim().to_string()));
    lines
}

/// The wordmark, with the caption lines under it the way a splash screen reads:
/// what this is, then the one setting worth knowing.
pub fn print_brand(caption: &str, details: &[&str]) {
    for line in brand_lines(caption, details) {
        println!("  {line}");
    }
}

/// A bordered list whose section titles sit in the border itself, so a group of
/// commands costs one line instead of a blank line plus a heading.
fn menu_box(sections: &[(&str, &[&str])]) -> Vec<String> {
    let entry = |item: &str| format!("  {} {item}", style("›").cyan());
    let inner = sections
        .iter()
        .flat_map(|(title, items)| {
            items
                .iter()
                .map(|i| i.len() + 5)
                .chain(std::iter::once(title.len() + 3))
        })
        .max()
        .unwrap_or(24);

    let titled = |title: &str, left: char, right: char| {
        let label = format!(" {} ", style(title).cyan().bold());
        let fill = inner.saturating_sub(measure_text_width(&label));
        format!(
            "{}{label}{}{}",
            style(left).dim(),
            style("─".repeat(fill)).dim(),
            style(right).dim()
        )
    };

    let mut lines = Vec::new();
    for (index, (title, items)) in sections.iter().enumerate() {
        let (left, right) = if index == 0 {
            ('┌', '┐')
        } else {
            ('├', '┤')
        };
        lines.push(titled(title, left, right));
        for item in *items {
            lines.push(format!(
                "{}{}{}",
                style('│').dim(),
                pad(&entry(item), inner),
                style('│').dim()
            ));
        }
    }
    lines.push(format!(
        "{}{}{}",
        style('└').dim(),
        style("─".repeat(inner)).dim(),
        style('┘').dim()
    ));
    lines
}

/// Splash screen: wordmark on the left, what you can run on the right.
pub fn print_splash(caption: &str, details: &[&str], sections: &[(&str, &[&str])]) {
    let left = brand_lines(caption, details);
    let right = menu_box(sections);
    // The widest line on the left decides the column, so a long model name
    // cannot shove the box sideways on one row and not the others.
    let column = left
        .iter()
        .map(|l| measure_text_width(l))
        .max()
        .unwrap_or(0)
        .max(mark_width());

    println!();
    for i in 0..left.len().max(right.len()) {
        match (left.get(i), right.get(i)) {
            (Some(l), Some(r)) => println!("  {}   {r}", pad(l, column)),
            (Some(l), None) => println!("  {l}"),
            (None, Some(r)) => println!("  {}   {r}", " ".repeat(column)),
            (None, None) => break,
        }
    }
    println!();
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
