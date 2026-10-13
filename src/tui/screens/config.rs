use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

use super::super::app::{Action, Screen, Snapshot};
use super::super::theme::*;
use super::super::widgets;
use crate::api::matches_filter;
use crate::config::{default_settings_path, read_json_or_default};
use crate::slots::{self, SLOTS};
use crate::utils::{mask_key, redact_secrets};

const HINTS: [widgets::Hint; 6] = [
    ("↑↓", "scroll"),
    ("m", "models"),
    ("k", "keys"),
    ("t", "check"),
    ("Esc", "back"),
    ("?", "help"),
];

#[derive(Default)]
pub struct ConfigScreen {
    scroll: u16,
}

impl ConfigScreen {
    pub fn reset(&mut self) {
        self.scroll = 0;
    }

    pub fn handle(&mut self, code: KeyCode) -> Action {
        match code {
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll = self.scroll.saturating_add(1);
                Action::None
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                Action::None
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(10);
                Action::None
            }
            KeyCode::Home => {
                self.scroll = 0;
                Action::None
            }
            KeyCode::Char('m') => Action::Open(Screen::Models),
            KeyCode::Char('k') => Action::Open(Screen::Keys),
            KeyCode::Char('t') => Action::Open(Screen::Check),
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => Action::Back,
            _ => Action::None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, snap: &Snapshot) {
        let body = widgets::page(frame, area, "Config", &HINTS);
        let block = widgets::panel("What Claude Code reads in this folder");
        let inner = block.inner(body);
        frame.render_widget(block, body);

        let lines = lines(snap, inner.width as usize);
        // Never scroll past the last line.
        let max = (lines.len() as u16).saturating_sub(inner.height);
        let scroll = self.scroll.min(max);
        frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
    }
}

fn lines(snap: &Snapshot, width: usize) -> Vec<Line<'static>> {
    let project = &snap.project;
    let mut out = Vec::new();
    let label = 14;
    let text = |s: String| vec![Span::styled(s, Style::default().fg(TEXT))];
    let dim = |s: String| vec![Span::styled(s, Style::default().fg(TEXT_DIM))];

    let (shown, file_note) = if project.has_local {
        (project.json.clone(), "exists — Claude Code reads it")
    } else {
        let template = default_settings_path()
            .ok()
            .filter(|p| p.exists())
            .map(|p| read_json_or_default(&p))
            .unwrap_or_else(|| serde_json::json!({}));
        (template, "not created yet — showing ccc's template")
    };

    let room = width.saturating_sub(label + 1);
    out.push(widgets::row(" File", label, text(project.local_path.display().to_string()), width));
    out.push(widgets::row(
        " Status",
        label,
        vec![Span::styled(
            file_note.to_string(),
            Style::default().fg(if project.has_local { SUCCESS } else { WARNING }),
        )],
        width,
    ));
    out.push(widgets::row(
        " Base URL",
        label,
        text(widgets::fit_middle(&project.base_url, room)),
        width,
    ));
    out.push(widgets::row(
        " Key",
        label,
        match &project.key {
            Some((value, source)) => vec![
                Span::styled(mask_key(value), Style::default().fg(TEXT)),
                Span::styled(format!("  {}", source.describe()), Style::default().fg(TEXT_DIM)),
            ],
            None => dim("none".into()),
        },
        width,
    ));
    out.push(widgets::row(
        " Discovery",
        label,
        if project.discovery {
            text("on — /model lists the gateway's models".into())
        } else {
            dim("off — /model shows aliases only".into())
        },
        width,
    ));
    out.push(widgets::row(
        " Model filter",
        label,
        if project.filter.is_empty() {
            dim("none — every listed model counts".into())
        } else {
            text(format!("{} — only models ending in {}", project.filter, project.filter))
        },
        width,
    ));

    out.push(Line::from(""));
    out.push(Line::from(Span::styled(" Models", Style::default().fg(ACCENT).bold())));
    let values = slots::read_all(&shown);
    for (i, slot) in SLOTS.iter().enumerate() {
        let (mark, colour) = match &values[i] {
            Some(v) if matches_filter(v, &project.filter) => (SYM_OK, SUCCESS),
            Some(_) => (SYM_FAIL, ERROR),
            None if slot.optional => (SYM_INFO, TEXT_DIM),
            None => (SYM_FAIL, ERROR),
        };
        let value = values[i].clone().unwrap_or_else(|| "not set".into());
        out.push(widgets::row(
            &format!("   {}", slot.label),
            label,
            vec![
                Span::styled(format!("{mark} "), Style::default().fg(colour).bold()),
                Span::styled(widgets::fit_model(&value, room.saturating_sub(2)), Style::default().fg(TEXT)),
            ],
            width,
        ));
    }
    if let Some(fast) = crate::config::env_value(&shown, slots::LEGACY_FAST_ENV) {
        out.push(widgets::row(
            "   Fast",
            label,
            vec![
                Span::styled(format!("{SYM_INFO} "), Style::default().fg(TEXT_DIM)),
                Span::styled(widgets::fit_model(&fast, room.saturating_sub(16)), Style::default().fg(TEXT_DIM)),
                Span::styled("  legacy, = Haiku".to_string(), Style::default().fg(TEXT_DIM)),
            ],
            width,
        ));
    }

    out.push(Line::from(""));
    out.push(Line::from(Span::styled(" Raw settings (secrets masked)", Style::default().fg(ACCENT).bold())));
    // Wrapped rather than cut: this is the one place the full values show.
    let raw = serde_json::to_string_pretty(&redact_secrets(&shown)).unwrap_or_default();
    for line in raw.lines() {
        for chunk in wrap_chars(&format!(" {line}"), width) {
            out.push(Line::from(Span::styled(chunk, Style::default().fg(TEXT_DIM))));
        }
    }
    out
}

/// Split a line into pieces of at most `width` columns, continuation pieces
/// indented under the first. JSON has no spaces to break at, so this goes by
/// character.
fn wrap_chars(line: &str, width: usize) -> Vec<String> {
    const INDENT: &str = "      ";
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut used = 0;
    for c in line.chars() {
        let w = console::measure_text_width(c.encode_utf8(&mut [0; 4]));
        if used + w > width && !current.is_empty() {
            pieces.push(std::mem::take(&mut current));
            current.push_str(INDENT);
            used = INDENT.len();
        }
        current.push(c);
        used += w;
    }
    pieces.push(current);
    pieces
}

#[cfg(test)]
mod tests {
    use super::wrap_chars;

    #[test]
    fn long_lines_wrap_within_width() {
        let pieces = wrap_chars(&"x".repeat(25), 10);
        assert!(pieces.iter().all(|p| console::measure_text_width(p) <= 10), "{pieces:?}");
        assert_eq!(pieces.concat().replace(' ', "").len(), 25);
    }
}
