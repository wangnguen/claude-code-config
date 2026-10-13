//! The `?` overlay: every key on the current screen, and the two or three
//! ideas a newcomer needs to make sense of it.

use ratatui::{prelude::*, widgets::*};

use super::super::app::Screen;
use super::super::theme::*;
use super::super::widgets;

/// A line of help: keys and what they do, or a plain sentence when the key is empty.
type Entry = (&'static str, &'static str);

fn entries(screen: Screen) -> (&'static str, Vec<Entry>) {
    match screen {
        Screen::Home => (
            "Home",
            vec![
                ("↑ ↓", "move between sections"),
                ("Enter →", "open the highlighted section"),
                ("1 – 4", "open Keys, Models, Config or Check directly"),
                ("q", "quit (Ctrl-C works anywhere)"),
                ("", ""),
                ("", "Getting a folder ready takes four steps:"),
                ("1", "Keys: press a and paste the key you were given"),
                ("2", "Keys: press u so this folder uses it"),
                ("3", "Models: pick models your key can reach, w saves"),
                ("4", "Check: confirm the gateway agrees"),
                ("", "Everything is written to .claude/settings.local.json here."),
            ],
        ),
        Screen::Keys => (
            "Keys",
            vec![
                ("↑ ↓", "move"),
                ("a", "add a key (name, then the key itself)"),
                ("u", "use the highlighted key in this folder"),
                ("d", "make it the default for new folders"),
                ("m", "pick this folder's models with this key"),
                ("s", "test every saved key against the gateway"),
                ("n / r", "rename / remove"),
                ("Esc", "back to Home"),
                ("", ""),
                (SYM_STAR, "Default: the key `ccc init` writes into new folders."),
                ("", "    Stored in ~/.ccc/keys.json."),
                (SYM_HERE, "This folder: the key Claude Code uses here."),
                ("", "    Stored in .claude/settings.local.json."),
            ],
        ),
        Screen::Models => (
            "Models",
            vec![
                ("↑ ↓", "move between slots, or models in the list"),
                ("→ Enter", "choose a model for the slot"),
                ("← Esc", "back to the slots, then to Home"),
                ("w", "save to .claude/settings.local.json"),
                ("f", "fill empty or unusable slots automatically"),
                ("1", "1M context on/off (Sonnet and Opus only)"),
                ("g", "gateway discovery on/off"),
                ("e / x", "type / clear the Custom model"),
                ("r", "reload the model list"),
                ("", ""),
                ("", "Each slot is what Claude Code uses for a name in /model."),
                ("", "Main starts sessions; Haiku also runs background tasks."),
                ("", "Only models the gateway lists for your key are offered."),
                ("", "Discovery off: /model shows aliases, which still work."),
            ],
        ),
        Screen::Config => (
            "Config",
            vec![
                ("↑ ↓", "scroll        PgUp PgDn  page"),
                ("m", "open Models    k  open Keys    t  run Check"),
                ("Esc", "back to Home"),
                ("", ""),
                ("", "This is what Claude Code reads in this folder, with"),
                ("", "secrets masked. ccc's own template (~/.ccc/.claude) is"),
                ("", "only copied by `ccc init`; Claude Code never reads it."),
            ],
        ),
        Screen::Check => (
            "Check",
            vec![
                ("r", "run the checks again"),
                ("↑ ↓", "scroll"),
                ("m / k", "open Models / Keys to fix what failed"),
                ("Esc", "back to Home"),
                ("", ""),
                (SYM_OK, "works"),
                (SYM_WARN, "works, but worth knowing about"),
                (SYM_FAIL, "will fail in Claude Code (usually a 403)"),
            ],
        ),
    }
}

pub fn render(frame: &mut Frame, area: Rect, screen: Screen) {
    let (title, entries) = entries(screen);
    let width = 66.min(area.width.saturating_sub(4));
    let height = (entries.len() as u16 + 4).min(area.height.saturating_sub(2));
    let rect = widgets::centered(width, height, area);
    frame.render_widget(Clear, rect);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(MODAL_BG))
        .title(Span::styled(format!(" Help · {title} "), Style::default().fg(ACCENT).bold()))
        .title_bottom(Line::from(Span::styled(" any key closes ", Style::default().fg(TEXT_DIM))).centered());
    let inner = block.inner(rect).inner(Margin { horizontal: 1, vertical: 1 });
    frame.render_widget(block, rect);

    let text_width = inner.width as usize;
    let lines: Vec<Line> = entries
        .iter()
        .map(|(key, text)| {
            if key.is_empty() && text.starts_with(' ') {
                // A continuation of the line above, aligned with its text.
                let text = format!("{:10}{}", "", text.trim_start());
                Line::from(Span::styled(widgets::fit(&text, text_width), Style::default().fg(TEXT_DIM)))
            } else if key.is_empty() {
                Line::from(Span::styled(widgets::fit(text, text_width), Style::default().fg(TEXT)))
            } else {
                let key = format!("{key:<9}");
                let room = text_width.saturating_sub(10);
                Line::from(vec![
                    Span::styled(key, Style::default().fg(KEY_HINT).bold()),
                    Span::raw(" "),
                    Span::styled(widgets::fit(text, room), Style::default().fg(TEXT)),
                ])
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
