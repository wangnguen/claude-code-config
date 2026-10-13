//! Building blocks every screen uses, so borders, titles and key hints look
//! and behave the same everywhere. Widths are always measured in terminal
//! columns, never bytes, and text that does not fit is cut with "…" rather
//! than silently clipped at the border.

use console::{measure_text_width, truncate_str};
use ratatui::{prelude::*, widgets::*};

use super::theme::*;
use crate::config::VERSION;

/// Smallest terminal the screens are laid out for.
pub const MIN_WIDTH: u16 = 64;
pub const MIN_HEIGHT: u16 = 20;

/// A key and what it does, as shown in hint bars.
pub type Hint<'a> = (&'a str, &'a str);

/// Cut `text` to `width` columns, ending in "…" when it had to be cut.
pub fn fit(text: &str, width: usize) -> String {
    if measure_text_width(text) <= width {
        text.to_string()
    } else {
        truncate_str(text, width, "…").into_owned()
    }
}

/// A model id cut to `width`. Ids that do not fit lose the "claude-" every
/// one of them starts with before anything else, which keeps the part that
/// tells them apart.
pub fn fit_model(id: &str, width: usize) -> String {
    if measure_text_width(id) <= width {
        return id.to_string();
    }
    fit(id.strip_prefix("claude-").unwrap_or(id), width)
}

/// Several model ids for one column: when any of them needs shortening they
/// all drop the prefix, so a list never mixes long and short spellings.
pub fn fit_models<'a>(ids: impl IntoIterator<Item = &'a str>, width: usize) -> Vec<String> {
    let ids: Vec<&str> = ids.into_iter().collect();
    let all_fit = ids.iter().all(|id| measure_text_width(id) <= width);
    ids.iter()
        .map(|id| {
            if all_fit {
                id.to_string()
            } else {
                fit(id.strip_prefix("claude-").unwrap_or(id), width)
            }
        })
        .collect()
}

/// Cut from the middle, for paths and URLs whose end matters as much as
/// their start.
pub fn fit_middle(text: &str, width: usize) -> String {
    crate::ui::shorten(text, width)
}

/// The frame every screen sits in: a rounded border titled "ccc › <screen>",
/// the version on the right, and the screen's key hints on the last line.
/// Returns the area left for the screen's own content.
pub fn page(frame: &mut Frame, area: Rect, title: &str, hints: &[Hint]) -> Rect {
    let heading = Line::from(vec![
        Span::styled(" ccc ", Style::default().fg(ACCENT).bold()),
        Span::styled("› ", Style::default().fg(DIM)),
        Span::styled(format!("{title} "), Style::default().fg(Color::White).bold()),
    ]);
    // Both corners and the heading stay clear of the version.
    let room = (area.width as usize).saturating_sub(heading.width() + 2);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(BG))
        .title(heading)
        .title(version_title(crate::release::known_update(), room).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [body, hints_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    hint_bar(frame, hints_area.inner(Margin { horizontal: 1, vertical: 0 }), hints);
    body.inner(Margin { horizontal: 1, vertical: 0 })
}

/// This build's version, then the newer release when one is out, with the
/// command that installs it when `room` allows.
fn version_title(latest: Option<&str>, room: usize) -> Line<'static> {
    let current = Span::styled(format!(" v{VERSION} "), Style::default().fg(DIM));
    let Some(latest) = latest else {
        return Line::from(current);
    };
    let room = room.saturating_sub(current.width());
    let long = format!("→ {latest} · ccc update ");
    let notice = if measure_text_width(&long) <= room { long } else { format!("→ {latest} ") };
    Line::from(vec![current, Span::styled(notice, Style::default().fg(WARNING).bold())])
}

/// Key hints on one line. The list is in priority order: hints are kept from
/// the front until the line is full, except the last one ("? help" by
/// convention), which always stays so the full list is one key away.
pub fn hint_bar(frame: &mut Frame, area: Rect, hints: &[Hint]) {
    let line = hint_line(hints, area.width as usize);
    frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
}

const HINT_GAP: &str = "  ";

fn hint_width((key, label): &Hint) -> usize {
    measure_text_width(key) + 1 + measure_text_width(label)
}

pub fn hint_line<'a>(hints: &[Hint<'a>], width: usize) -> Line<'a> {
    let Some((pinned, rest)) = hints.split_last() else {
        return Line::default();
    };
    let gap = HINT_GAP.len();
    let mut used = hint_width(pinned);
    let mut kept: Vec<Hint> = Vec::new();
    for hint in rest {
        let needed = hint_width(hint) + gap;
        if used + needed > width {
            break;
        }
        used += needed;
        kept.push(*hint);
    }
    kept.push(*pinned);

    let mut spans = Vec::new();
    for (i, (key, label)) in kept.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(HINT_GAP));
        }
        spans.push(Span::styled(key, Style::default().fg(KEY_HINT).bold()));
        spans.push(Span::styled(format!(" {label}"), Style::default().fg(TEXT_DIM)));
    }
    Line::from(spans)
}

/// A titled panel inside a page.
pub fn panel(title: &str) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL_BG))
        .title(Span::styled(format!(" {title} "), Style::default().fg(ACCENT).bold()))
}

/// A label/value line with the label padded to `label_width` columns and the
/// value cut to whatever room is left in `width`.
pub fn row<'a>(label: &str, label_width: usize, value: Vec<Span<'a>>, width: usize) -> Line<'a> {
    let label = format!("{:<label_width$} ", fit(label, label_width));
    let mut room = width.saturating_sub(measure_text_width(&label));
    let mut spans = vec![Span::styled(label, Style::default().fg(DIM))];
    for span in value {
        if room == 0 {
            break;
        }
        let text = fit(&span.content, room);
        room = room.saturating_sub(measure_text_width(&text));
        spans.push(Span::styled(text, span.style));
    }
    Line::from(spans)
}

/// A rectangle of at most `width` × `height` centred in `area`.
pub fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

/// What to show instead of a screen that would not fit.
pub fn too_small(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled("Window too small", Style::default().fg(WARNING).bold())),
        Line::from(""),
        Line::from(format!("Needs {MIN_WIDTH}×{MIN_HEIGHT}, have {}×{}.", area.width, area.height)),
        Line::from("Enlarge the window, or press q to quit."),
    ];
    let height = (lines.len() as u16).min(area.height);
    let y = area.y + area.height.saturating_sub(height) / 2;
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(Style::default().fg(TEXT).bg(BG))
            .wrap(Wrap { trim: true }),
        Rect::new(area.x, y, area.width, height),
    );
}

/// The frame of a spinner for a counter that ticks while work is pending.
pub fn spinner(tick: usize) -> &'static str {
    SPINNER[tick % SPINNER.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn hint_line_keeps_the_last_hint_when_space_runs_out() {
        let hints = [("a", "add"), ("u", "use here"), ("d", "default"), ("?", "help")];
        let line = hint_line(&hints, 20);
        let shown = text(&line);
        assert!(shown.contains("? help"), "{shown}");
        assert!(measure_text_width(&shown) <= 20, "{shown}");
        assert!(shown.starts_with("a add"));
    }

    #[test]
    fn hint_line_shows_everything_when_it_fits() {
        let hints = [("a", "add"), ("?", "help")];
        assert_eq!(text(&hint_line(&hints, 80)), "a add  ? help");
    }

    #[test]
    fn fit_cuts_by_columns() {
        assert_eq!(fit("hello", 10), "hello");
        let cut = fit("hello world", 6);
        assert_eq!(measure_text_width(&cut), 6);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn fit_model_drops_the_common_prefix_first() {
        assert_eq!(fit_model("claude-sonnet-5-vn[1m]", 30), "claude-sonnet-5-vn[1m]");
        assert_eq!(fit_model("claude-sonnet-5-vn[1m]", 16), "sonnet-5-vn[1m]");
        assert_eq!(measure_text_width(&fit_model("claude-sonnet-5-vn[1m]", 8)), 8);
    }

    #[test]
    fn fit_models_shortens_all_or_none() {
        let ids = ["claude-opus-5-vn", "claude-sonnet-5-vn[1m]"];
        assert_eq!(fit_models(ids, 30), vec!["claude-opus-5-vn", "claude-sonnet-5-vn[1m]"]);
        assert_eq!(fit_models(ids, 20), vec!["opus-5-vn", "sonnet-5-vn[1m]"]);
    }

    #[test]
    fn row_never_exceeds_its_width() {
        let line = row("Label", 8, vec![Span::raw("a very long value that cannot fit")], 20);
        assert!(line.width() <= 20, "{}", text(&line));
    }

    #[test]
    fn centered_clamps_to_the_area() {
        let r = centered(100, 50, Rect::new(0, 0, 40, 10));
        assert_eq!(r, Rect::new(0, 0, 40, 10));
    }

    #[test]
    fn version_title_names_a_newer_release_and_fits_its_room() {
        assert_eq!(text(&version_title(None, 80)), format!(" v{VERSION} "));

        let roomy = text(&version_title(Some("9.9.9"), 80));
        assert!(roomy.ends_with("→ 9.9.9 · ccc update "), "{roomy}");

        // The heading of the longest screen title at the narrowest supported width.
        let room = MIN_WIDTH as usize - 2 - " ccc › Keys › Test every key ".chars().count();
        let tight = text(&version_title(Some("10.10.10"), room));
        assert!(measure_text_width(&tight) <= room, "{tight}");
        assert!(tight.contains("10.10.10"), "{tight}");
    }
}
