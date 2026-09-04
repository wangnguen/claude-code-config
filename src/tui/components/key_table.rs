use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

use super::super::component::{Action, KeyOp, ModeSwitch};
use super::super::theme::*;
use crate::config::KeysStore;
use crate::utils::mask_key;

pub struct KeyTable {
    pub entries: Vec<(String, String, bool)>, // (name, masked, is_default)
    pub selected: usize,
}

impl KeyTable {
    pub fn load() -> Self {
        let store = KeysStore::load();
        let entries = store.keys.iter()
            .map(|(name, value)| {
                let is_default = store.active.as_deref() == Some(name.as_str());
                (name.clone(), mask_key(value), is_default)
            })
            .collect();
        KeyTable { entries, selected: 0 }
    }

    pub fn reload(&mut self) {
        let store = KeysStore::load();
        self.entries = store.keys.iter()
            .map(|(name, value)| {
                let is_default = store.active.as_deref() == Some(name.as_str());
                (name.clone(), mask_key(value), is_default)
            })
            .collect();
        if self.entries.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
    }

    pub fn selected_name(&self) -> Option<String> {
        self.entries.get(self.selected).map(|(n, _, _)| n.clone())
    }

    pub fn handle_event(&mut self, key: KeyCode) -> Action {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Char('a') => Action::Switch(ModeSwitch::AddName),
            KeyCode::Char('d') => {
                if let Some(name) = self.selected_name() {
                    Action::Op(KeyOp::Default(name))
                } else { Action::None }
            }
            KeyCode::Char('u') => {
                if let Some(name) = self.selected_name() {
                    Action::Op(KeyOp::Use(name))
                } else { Action::None }
            }
            KeyCode::Char('r') => {
                if let Some(name) = self.selected_name() {
                    Action::Switch(ModeSwitch::ConfirmRemove(name))
                } else { Action::None }
            }
            KeyCode::Char('n') => {
                if let Some(name) = self.selected_name() {
                    Action::Switch(ModeSwitch::Rename { old_name: name })
                } else { Action::None }
            }
            KeyCode::Char('s') => Action::Op(KeyOp::Status),
            KeyCode::Left | KeyCode::Char('h') => Action::Switch(ModeSwitch::ToHome),
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 { self.selected -= 1; }
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.entries.len().saturating_sub(1);
                if self.selected < max { self.selected += 1; }
                Action::None
            }
            _ => Action::None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Keys ".to_string())
            .title_style(Style::default().fg(ACCENT).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL_BG));

        if self.entries.is_empty() {
            let empty = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("No keys saved", Style::default().fg(DIM).italic())),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Press ", Style::default().fg(TEXT_DIM)),
                    Span::styled("a", Style::default().fg(Color::Green).bold()),
                    Span::styled(" to add your first key", Style::default().fg(TEXT_DIM)),
                ]),
            ])
            .alignment(Alignment::Center)
            .block(block);
            frame.render_widget(empty, area);
            return;
        }

        let header = Row::new(vec!["", "  Name", "Key", "Default"])
            .style(Style::default().fg(ACCENT).bold())
            .bottom_margin(1);

        let rows: Vec<Row> = self.entries.iter().enumerate().map(|(i, (name, masked, is_default))| {
            let sel = i == self.selected;
            let base = if sel {
                Style::default().bg(HIGHLIGHT_BG).fg(Color::White)
            } else {
                Style::default().fg(TEXT)
            };
            Row::new(vec![
                Cell::from(if sel { format!(" {ICON_PLAY}") } else { "  ".into() }).style(Style::default().fg(ACCENT)),
                Cell::from(format!("  {name}")),
                Cell::from(masked.as_str()).style(Style::default().fg(if sel { TEXT } else { TEXT_DIM })),
                Cell::from(if *is_default { format!(" {ICON_STAR}") } else { "".into() }).style(Style::default().fg(Color::Yellow)),
            ]).style(base)
        }).collect();

        let table = Table::new(rows, [
            Constraint::Length(3),
            Constraint::Percentage(35),
            Constraint::Percentage(45),
            Constraint::Length(10),
        ])
        .header(header)
        .block(block);
        frame.render_widget(table, area);
    }
}
