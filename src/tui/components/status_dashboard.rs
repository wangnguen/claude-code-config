use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};
use std::sync::mpsc;
use std::thread;

use super::super::component::{Action, ModeSwitch};
use super::super::theme::*;
use crate::api::{check_api_key, get_api_config};
use crate::config::KeysStore;
use crate::utils::mask_key;

#[derive(Clone)]
pub struct StatusEntry {
    pub name: String,
    pub masked: String,
    pub is_default: bool,
    pub result: Option<(bool, String)>,
}

pub struct StatusDashboard {
    pub api_url: String,
    pub model: String,
    pub results: Vec<StatusEntry>,
    pub total: usize,
    pub checked: usize,
    pub rx: Option<mpsc::Receiver<(usize, bool, String)>>,
}

impl StatusDashboard {
    pub fn new() -> Option<Self> {
        let store = KeysStore::load();
        if store.keys.is_empty() {
            return None;
        }

        let (api_url, model) = get_api_config();
        let total = store.keys.len();

        let results: Vec<StatusEntry> = store.keys.iter()
            .map(|(name, value)| StatusEntry {
                name: name.clone(),
                masked: mask_key(value),
                is_default: store.active.as_deref() == Some(name.as_str()),
                result: None,
            })
            .collect();

        let (tx, rx) = mpsc::channel();
        for (i, (_, value)) in store.keys.iter().enumerate() {
            let tx = tx.clone();
            let value = value.clone();
            thread::spawn(move || {
                let (ok, msg) = check_api_key(&value);
                let _ = tx.send((i, ok, msg));
            });
        }
        drop(tx);

        Some(StatusDashboard { api_url, model, results, total, checked: 0, rx: Some(rx) })
    }

    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok((idx, ok, msg)) = rx.try_recv() {
                if idx < self.results.len() {
                    self.results[idx].result = Some((ok, msg));
                    self.checked += 1;
                }
            }
            if self.checked >= self.total {
                self.rx = None;
            }
        }
    }

    pub fn is_done(&self) -> bool {
        self.rx.is_none()
    }

    pub fn is_pending(&self) -> bool {
        self.rx.is_some()
    }

    pub fn handle_event(&mut self, key: KeyCode) -> Action {
        match key {
            _ if self.is_done() => Action::Switch(ModeSwitch::Normal),
            _ => Action::None, // ignore keys while checking
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(5),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        let done = self.is_done();

        // Header
        let header_text = if done { "Key Status — Complete" } else { "Key Status — Checking..." };
        let header = Paragraph::new(Line::from(vec![
            Span::styled(format!("  {} ", ICON_SEARCH), Style::default().fg(Color::Cyan)),
            Span::styled(header_text, Style::default().fg(ACCENT).bold()),
        ]))
        .alignment(Alignment::Center)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if done { SUCCESS } else { Color::Cyan }))
            .style(Style::default().bg(PANEL_BG)));
        frame.render_widget(header, layout[0]);

        // Connection info
        let info_block = Block::default()
            .title(" Connection Info ")
            .title_style(Style::default().fg(ACCENT).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL_BG));
        let info_inner = info_block.inner(layout[1]);
        frame.render_widget(info_block, layout[1]);

        let info_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
            .split(info_inner);

        frame.render_widget(Paragraph::new(Line::from(vec![
            Span::styled(" API    ", Style::default().fg(DIM)),
            Span::styled(&self.api_url, Style::default().fg(TEXT)),
        ])), info_layout[0]);

        frame.render_widget(Paragraph::new(Line::from(vec![
            Span::styled(" Model  ", Style::default().fg(DIM)),
            Span::styled(&self.model, Style::default().fg(TEXT)),
        ])), info_layout[1]);

        let ratio = if self.total > 0 { self.checked as f64 / self.total as f64 } else { 0.0 };
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(if done { SUCCESS } else { Color::Cyan }).bg(Color::Rgb(30, 30, 50)))
            .ratio(ratio)
            .label(Span::styled(format!("{}/{}", self.checked, self.total), Style::default().fg(Color::White).bold()));
        frame.render_widget(gauge, Rect::new(info_layout[2].x + 1, info_layout[2].y, info_layout[2].width.saturating_sub(2), 1));

        // Results table
        let results_block = Block::default()
            .title(" Results ")
            .title_style(Style::default().fg(ACCENT).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL_BG));
        let results_inner = results_block.inner(layout[2]);
        frame.render_widget(results_block, layout[2]);

        let header_row = Row::new(vec!["", "  Name", "Key", "Default", "Status"])
            .style(Style::default().fg(ACCENT).bold())
            .bottom_margin(1);

        let rows: Vec<Row> = self.results.iter().map(|entry| {
            let (icon, status_text, status_style) = match &entry.result {
                None => (ICON_WAIT.to_string(), "checking...".to_string(), Style::default().fg(Color::Yellow)),
                Some((true, msg)) => (ICON_OK.to_string(), msg.clone(), Style::default().fg(SUCCESS)),
                Some((false, msg)) => (ICON_FAIL.to_string(), msg.clone(), Style::default().fg(ERROR)),
            };
            let row_style = match &entry.result {
                None => Style::default().fg(Color::Yellow),
                Some((true, _)) => Style::default().fg(TEXT),
                Some((false, _)) => Style::default().fg(Color::Rgb(200, 150, 150)),
            };
            Row::new(vec![
                Cell::from(format!(" {icon}")),
                Cell::from(format!("  {}", entry.name)),
                Cell::from(entry.masked.as_str()).style(Style::default().fg(TEXT_DIM)),
                Cell::from(if entry.is_default { format!(" {ICON_STAR}") } else { "".into() }).style(Style::default().fg(Color::Yellow)),
                Cell::from(status_text).style(status_style),
            ]).style(row_style)
        }).collect();

        let table = Table::new(rows, [
            Constraint::Length(4),
            Constraint::Percentage(22),
            Constraint::Percentage(28),
            Constraint::Length(8),
            Constraint::Percentage(35),
        ])
        .header(header_row);
        frame.render_widget(table, results_inner);

        // Summary footer
        let pass = self.results.iter().filter(|e| matches!(&e.result, Some((true, _)))).count();
        let fail = self.results.iter().filter(|e| matches!(&e.result, Some((false, _)))).count();
        let pending = self.results.iter().filter(|e| e.result.is_none()).count();

        let summary = if done {
            Line::from(vec![
                Span::styled(format!(" {} ", ICON_OK), Style::default().fg(SUCCESS)),
                Span::styled(format!("{pass} passed"), Style::default().fg(SUCCESS).bold()),
                Span::styled("   ", Style::default()),
                Span::styled(format!("{} ", ICON_FAIL), Style::default().fg(if fail > 0 { ERROR } else { DIM })),
                Span::styled(format!("{fail} failed"), Style::default().fg(if fail > 0 { ERROR } else { DIM }).bold()),
                Span::styled("   │   ", Style::default().fg(BORDER)),
                Span::styled("Press any key to return", Style::default().fg(TEXT_DIM)),
            ])
        } else {
            Line::from(vec![
                Span::styled(" Checking... ".to_string(), Style::default().fg(Color::Cyan)),
                Span::styled(format!("({pending} remaining)"), Style::default().fg(DIM)),
            ])
        };

        let footer_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if done { BORDER } else { Color::Cyan }))
            .style(Style::default().bg(PANEL_BG));
        frame.render_widget(Paragraph::new(summary).alignment(Alignment::Center).block(footer_block), layout[3]);
    }
}
