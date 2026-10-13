//! "Test every key": each saved key asks the gateway for its model list, in
//! parallel, and the results fill in as they arrive.

use std::sync::mpsc::{self, Receiver};
use std::thread;

use ratatui::{prelude::*, widgets::*};

use super::super::theme::*;
use super::super::widgets::{self, fit_middle};
use crate::api::{check_api_key, get_api_config};
use crate::config::KeysStore;
use crate::utils::mask_key;

struct Entry {
    name: String,
    masked: String,
    is_default: bool,
    result: Option<(bool, String)>,
}

pub struct StatusDashboard {
    api_url: String,
    entries: Vec<Entry>,
    rx: Option<Receiver<(usize, bool, String)>>,
}

impl StatusDashboard {
    pub fn new(store: &KeysStore) -> Option<Self> {
        if store.keys.is_empty() {
            return None;
        }
        let (api_url, _) = get_api_config();
        let entries = store
            .keys
            .iter()
            .map(|(name, value)| Entry {
                name: name.clone(),
                masked: mask_key(value),
                is_default: store.active.as_deref() == Some(name.as_str()),
                result: None,
            })
            .collect();

        let (tx, rx) = mpsc::channel();
        for (i, value) in store.keys.values().enumerate() {
            let tx = tx.clone();
            let value = value.clone();
            thread::spawn(move || {
                let (ok, msg) = check_api_key(&value);
                let _ = tx.send((i, ok, msg));
            });
        }

        Some(StatusDashboard { api_url, entries, rx: Some(rx) })
    }

    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok((i, ok, msg)) => {
                    if let Some(entry) = self.entries.get_mut(i) {
                        entry.result = Some((ok, msg));
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                // Every sender is gone: all checks have reported.
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    break;
                }
            }
        }
        if self.entries.iter().all(|e| e.result.is_some()) {
            self.rx = None;
        }
    }

    pub fn is_pending(&self) -> bool {
        self.rx.is_some()
    }

    pub fn is_done(&self) -> bool {
        self.rx.is_none()
    }

    /// Finished results, by key name.
    pub fn results(&self) -> Vec<(String, (bool, String))> {
        self.entries
            .iter()
            .filter_map(|e| e.result.clone().map(|r| (e.name.clone(), r)))
            .collect()
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, tick: usize) {
        let done = self.is_done();
        let hints: &[widgets::Hint] =
            if done { &[("any key", "back to Keys")] } else { &[("…", "testing every key")] };
        let body = widgets::page(frame, area, "Keys › Test every key", hints);
        let [info, table_area] = Layout::vertical([Constraint::Length(2), Constraint::Min(3)]).areas(body);

        let checked = self.entries.iter().filter(|e| e.result.is_some()).count();
        let total = self.entries.len();
        let width = info.width as usize;
        let progress = if done {
            Span::styled(format!("{checked}/{total} tested"), Style::default().fg(SUCCESS).bold())
        } else {
            Span::styled(
                format!("{} {checked}/{total} tested", widgets::spinner(tick)),
                Style::default().fg(ACCENT).bold(),
            )
        };
        frame.render_widget(
            Paragraph::new(vec![
                widgets::row(
                    "Gateway",
                    8,
                    vec![Span::styled(fit_middle(&self.api_url, width.saturating_sub(9)), Style::default().fg(TEXT))],
                    width,
                ),
                widgets::row("Progress", 8, vec![progress], width),
            ]),
            info,
        );

        let rows: Vec<Row> = self
            .entries
            .iter()
            .map(|entry| {
                let (mark, text, colour) = match &entry.result {
                    None => (widgets::spinner(tick), "testing…".to_string(), WARNING),
                    Some((true, msg)) => (SYM_OK, msg.clone(), SUCCESS),
                    Some((false, msg)) => (SYM_FAIL, msg.clone(), ERROR),
                };
                Row::new(vec![
                    Cell::from(Span::styled(mark, Style::default().fg(colour).bold())),
                    Cell::from(entry.name.clone()),
                    Cell::from(Span::styled(entry.masked.clone(), Style::default().fg(TEXT_DIM))),
                    Cell::from(Span::styled(if entry.is_default { SYM_STAR } else { "" }, Style::default().fg(Color::Yellow))),
                    Cell::from(Span::styled(text, Style::default().fg(colour))),
                ])
                .style(Style::default().fg(TEXT))
            })
            .collect();
        let table = Table::new(
            rows,
            [
                Constraint::Length(1),
                Constraint::Min(8),
                Constraint::Length(12),
                Constraint::Length(1),
                Constraint::Fill(2),
            ],
        )
        .header(Row::new(vec!["", "Name", "Key", "", "Result"]).style(Style::default().fg(ACCENT).bold()))
        .block(widgets::panel("Results"));
        frame.render_widget(table, table_area);
    }
}
