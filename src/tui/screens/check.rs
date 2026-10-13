use std::sync::mpsc::{self, Receiver};
use std::thread;

use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

use super::super::app::{Action, Screen};
use super::super::theme::*;
use super::super::widgets;
use crate::checks::{self, Report, Status};

const HINTS: [widgets::Hint; 6] = [
    ("r", "run again"),
    ("↑↓", "scroll"),
    ("m", "models"),
    ("k", "keys"),
    ("Esc", "back"),
    ("?", "help"),
];

enum State {
    Idle,
    Running(Receiver<Result<Report, String>>),
    Done(Result<Report, String>),
}

pub struct CheckScreen {
    state: State,
    scroll: u16,
}

impl Default for CheckScreen {
    fn default() -> Self {
        CheckScreen { state: State::Idle, scroll: 0 }
    }
}

impl CheckScreen {
    pub fn start(&mut self) {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(checks::run());
        });
        self.state = State::Running(rx);
        self.scroll = 0;
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, State::Running(_))
    }

    pub fn poll(&mut self) {
        if let State::Running(rx) = &self.state {
            match rx.try_recv() {
                Ok(result) => self.state = State::Done(result),
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.state = State::Done(Err("the check stopped unexpectedly".into()))
                }
            }
        }
    }

    /// (passed, warnings, failed) of the last finished run.
    pub fn summary(&self) -> Option<(usize, usize, usize)> {
        match &self.state {
            State::Done(Ok(report)) => Some((
                report.count(Status::Pass),
                report.count(Status::Warn),
                report.count(Status::Fail),
            )),
            _ => None,
        }
    }

    pub fn handle(&mut self, code: KeyCode) -> Action {
        match code {
            KeyCode::Char('r') if !self.is_running() => {
                self.start();
                Action::None
            }
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll = self.scroll.saturating_add(1);
                Action::None
            }
            KeyCode::Char('m') => Action::Open(Screen::Models),
            KeyCode::Char('k') => Action::Open(Screen::Keys),
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => Action::Back,
            _ => Action::None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, tick: usize) {
        let body = widgets::page(frame, area, "Check", &HINTS);
        let [results, summary_area] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(body);

        let block = widgets::panel("Key, models and discovery on the gateway");
        let inner = block.inner(results);
        frame.render_widget(block, results);
        let width = inner.width as usize;

        let (lines, summary_line) = match &self.state {
            State::Idle => (vec![], Line::default()),
            State::Running(_) => (
                vec![Line::from(vec![
                    Span::styled(format!(" {} ", widgets::spinner(tick)), Style::default().fg(ACCENT)),
                    Span::styled("Asking the gateway…", Style::default().fg(TEXT)),
                ])],
                Line::default(),
            ),
            State::Done(Err(message)) => (
                vec![
                    Line::from(Span::styled(format!(" {message}"), Style::default().fg(ERROR))),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Press k to add a key, then u to use it in this folder.",
                        Style::default().fg(TEXT),
                    )),
                ],
                Line::default(),
            ),
            State::Done(Ok(report)) => (report_lines(report, width), summary(report)),
        };

        let max = (lines.len() as u16).saturating_sub(inner.height);
        frame.render_widget(Paragraph::new(lines).scroll((self.scroll.min(max), 0)), inner);
        frame.render_widget(Paragraph::new(summary_line).alignment(Alignment::Center), summary_area);
    }
}

fn report_lines(report: &Report, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (label, value) in &report.rows {
        lines.push(widgets::row(
            &format!(" {label}"),
            15,
            vec![Span::styled(value.clone(), Style::default().fg(TEXT_DIM))],
            width,
        ));
    }
    for group in &report.groups {
        lines.push(Line::from(""));
        for item in group {
            let (mark, colour) = match item.status {
                Status::Pass => (SYM_OK, SUCCESS),
                Status::Warn => (SYM_WARN, WARNING),
                Status::Fail => (SYM_FAIL, ERROR),
                Status::Info => (SYM_INFO, TEXT_DIM),
            };
            let label = format!("{:<15} ", widgets::fit(&item.label, 15));
            let room = width.saturating_sub(3 + 16);
            lines.push(Line::from(vec![
                Span::styled(format!(" {mark} "), Style::default().fg(colour).bold()),
                Span::styled(label, Style::default().fg(TEXT)),
                Span::styled(
                    widgets::fit(&item.detail, room),
                    Style::default().fg(if item.status == Status::Pass { TEXT_DIM } else { colour }),
                ),
            ]));
        }
    }
    lines
}

fn summary(report: &Report) -> Line<'static> {
    let (pass, warn, fail) = (
        report.count(Status::Pass),
        report.count(Status::Warn),
        report.count(Status::Fail),
    );
    let mut spans = vec![Span::styled(format!("{pass} passed"), Style::default().fg(SUCCESS).bold())];
    if warn > 0 {
        spans.push(Span::styled(format!(" · {warn} to know"), Style::default().fg(WARNING).bold()));
    }
    if fail > 0 {
        spans.push(Span::styled(format!(" · {fail} failed"), Style::default().fg(ERROR).bold()));
        spans.push(Span::styled(" · m fixes models", Style::default().fg(TEXT_DIM)));
    }
    Line::from(spans)
}
