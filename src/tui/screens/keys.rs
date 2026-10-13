use std::collections::BTreeMap;

use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

use super::super::app::{Action, Ask, KeyOp, Prompt, Snapshot};
use super::super::theme::*;
use super::super::widgets::{self, fit};
use crate::config::KeysStore;
use crate::utils::mask_key;

const HINTS: [widgets::Hint; 9] = [
    ("a", "add"),
    ("u", "use here"),
    ("d", "default"),
    ("m", "models"),
    ("s", "test all"),
    ("n", "rename"),
    ("r", "remove"),
    ("Esc", "back"),
    ("?", "help"),
];

#[derive(Default)]
pub struct KeysScreen {
    selected: usize,
}

impl KeysScreen {
    /// Highlight a key by name, e.g. the one just added.
    pub fn select(&mut self, name: &str, store: &KeysStore) {
        if let Some(i) = store.keys.keys().position(|k| k == name) {
            self.selected = i;
        }
    }

    /// Start on the key this folder uses, or else the default.
    pub fn focus_current(&mut self, snap: &Snapshot) {
        let current = folder_key_name(snap).or_else(|| snap.store.active.clone());
        if let Some(name) = current {
            self.select(&name, &snap.store);
        }
    }

    /// Keep the highlight on a row that exists after a removal.
    pub fn clamp(&mut self, store: &KeysStore) {
        self.selected = self.selected.min(store.keys.len().saturating_sub(1));
    }

    fn selected_name(&self, snap: &Snapshot) -> Option<String> {
        snap.store.keys.keys().nth(self.selected).cloned()
    }

    pub fn handle(&mut self, code: KeyCode, snap: &Snapshot) -> Action {
        let count = snap.store.keys.len();
        let on_selected = |make: fn(String) -> Action| match self.selected_name(snap) {
            Some(name) => make(name),
            None => Action::Notify { text: "No keys yet. Press a to add one.".into(), success: false },
        };
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < count {
                    self.selected += 1;
                }
                Action::None
            }
            KeyCode::Char('a') => Action::Prompt(Prompt::KeyName),
            KeyCode::Char('u') => on_selected(|n| Action::Key(KeyOp::Use(n))),
            KeyCode::Char('d') => on_selected(|n| Action::Key(KeyOp::Default(n))),
            KeyCode::Char('m') => on_selected(Action::PickModelsWith),
            KeyCode::Char('n') => on_selected(|n| Action::Prompt(Prompt::Rename { old: n })),
            KeyCode::Char('r') | KeyCode::Delete => on_selected(|n| Action::Confirm(Ask::RemoveKey(n))),
            KeyCode::Char('s') => Action::TestAllKeys,
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => Action::Back,
            _ => Action::None,
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        snap: &Snapshot,
        results: &BTreeMap<String, (bool, String)>,
    ) {
        let body = widgets::page(frame, area, "Keys", &HINTS);

        if snap.store.keys.is_empty() {
            render_empty(frame, body);
            return;
        }

        let [table_area, detail_area] = if body.width >= 96 {
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(body)
        } else {
            Layout::vertical([Constraint::Min(5), Constraint::Length(8)]).areas(body)
        };
        self.render_table(frame, table_area, snap);
        if let Some(name) = self.selected_name(snap) {
            render_detail(frame, detail_area, snap, &name, results.get(&name));
        }
    }

    fn render_table(&self, frame: &mut Frame, area: Rect, snap: &Snapshot) {
        let here = folder_key_name(snap);
        let block = widgets::panel("Saved keys").title_bottom(
            Line::from(vec![
                Span::styled(format!(" {SYM_STAR} default "), Style::default().fg(Color::Yellow)),
                Span::styled(format!(" {SYM_HERE} this folder "), Style::default().fg(SUCCESS)),
            ])
            .right_aligned(),
        );

        let rows: Vec<Row> = snap
            .store
            .keys
            .iter()
            .map(|(name, value)| {
                let default = snap.store.active.as_deref() == Some(name.as_str());
                let in_folder = here.as_deref() == Some(name.as_str());
                Row::new(vec![
                    Cell::from(Span::raw(name.clone())),
                    Cell::from(Line::from(vec![
                        Span::styled(if default { SYM_STAR } else { " " }, Style::default().fg(Color::Yellow)),
                        Span::raw(" "),
                        Span::styled(if in_folder { SYM_HERE } else { " " }, Style::default().fg(SUCCESS)),
                    ])),
                    Cell::from(Span::styled(mask_key(value), Style::default().fg(TEXT_DIM))),
                ])
                .style(Style::default().fg(TEXT))
            })
            .collect();

        // The name column is as wide as the longest name, so the markers and
        // the key sit right next to it instead of across the screen.
        let name_width = snap
            .store
            .keys
            .keys()
            .map(|n| console::measure_text_width(n))
            .max()
            .unwrap_or(0)
            .clamp(8, 32) as u16;
        let table = Table::new(
            rows,
            [Constraint::Length(name_width), Constraint::Length(3), Constraint::Min(11)],
        )
        .column_spacing(2)
        .header(
            Row::new(vec!["Name", "", "Key"])
                .style(Style::default().fg(ACCENT).bold()),
        )
        .row_highlight_style(Style::default().bg(HIGHLIGHT_BG).fg(Color::White).bold())
        .highlight_symbol(Span::styled(format!("{SYM_POINTER} "), Style::default().fg(ACCENT)))
        .highlight_spacing(HighlightSpacing::Always)
        .block(block);

        let mut state = TableState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(table, area, &mut state);
    }
}

/// The saved key this folder uses, if it is one of them.
fn folder_key_name(snap: &Snapshot) -> Option<String> {
    if snap.project.key_applied() {
        snap.project.key_name.clone()
    } else {
        None
    }
}

fn render_detail(
    frame: &mut Frame,
    area: Rect,
    snap: &Snapshot,
    name: &str,
    result: Option<&(bool, String)>,
) {
    let block = widgets::panel(name);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width as usize;

    let value = snap.store.keys.get(name).map(String::as_str).unwrap_or("");
    let is_default = snap.store.active.as_deref() == Some(name);
    let in_folder = folder_key_name(snap).as_deref() == Some(name);

    let yes = |text: &str| vec![Span::styled(text.to_string(), Style::default().fg(SUCCESS))];
    let no = |text: &str| vec![Span::styled(text.to_string(), Style::default().fg(TEXT_DIM))];
    let test = match result {
        Some((true, msg)) => vec![Span::styled(format!("{SYM_OK} {msg}"), Style::default().fg(SUCCESS))],
        Some((false, msg)) => vec![Span::styled(format!("{SYM_FAIL} {msg}"), Style::default().fg(ERROR))],
        None => no("not tested — s tests every key"),
    };

    let lines = vec![
        widgets::row(" Key", 14, vec![Span::styled(mask_key(value), Style::default().fg(TEXT))], width),
        widgets::row(
            &format!(" {SYM_STAR} Default"),
            14,
            if is_default { yes("yes — `ccc init` puts it in new folders") } else { no("no — d makes it the default") },
            width,
        ),
        widgets::row(
            &format!(" {SYM_HERE} This folder"),
            14,
            if in_folder { yes("yes — Claude Code uses it here") } else { no("no — u uses it here") },
            width,
        ),
        widgets::row(" Last test", 14, test, width),
        Line::from(""),
        Line::from(Span::styled(
            fit(" m picks this folder's models using this key", width),
            Style::default().fg(TEXT_DIM),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_empty(frame: &mut Frame, area: Rect) {
    let block = widgets::panel("Saved keys");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width as usize;

    let step = |n: &str, text: &str| {
        Line::from(vec![
            Span::styled(format!(" {n}  "), Style::default().fg(KEY_HINT).bold()),
            Span::styled(fit(text, width.saturating_sub(5)), Style::default().fg(TEXT)),
        ])
    };
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(" No keys saved yet.", Style::default().fg(Color::White).bold())),
        Line::from(""),
        step("1", "Press a and give the key a name (work, personal…)"),
        step("2", "Paste the virtual API key you were given"),
        step("3", "Press u to use it in this folder, then pick models"),
        Line::from(""),
        Line::from(Span::styled(
            fit(&format!(" The first key becomes the default ({SYM_STAR}) for new folders."), width),
            Style::default().fg(TEXT_DIM),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
