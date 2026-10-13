use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

use super::super::app::{Action, Screen, Snapshot};
use super::super::theme::*;
use super::super::widgets::{self, fit, fit_middle};

/// The sections, with the one line that tells a newcomer why they would go there.
const ITEMS: [(Screen, &str, &str); 4] = [
    (Screen::Keys, "Keys", "Save keys; choose the one this folder uses"),
    (Screen::Models, "Models", "Choose models your key can use"),
    (Screen::Config, "Config", "See what Claude Code reads in this folder"),
    (Screen::Check, "Check", "Test key, models and discovery"),
];

const HINTS: [widgets::Hint; 5] = [
    ("↑↓", "move"),
    ("Enter", "open"),
    ("1-4", "jump"),
    ("q", "quit"),
    ("?", "help"),
];

#[derive(Default)]
pub struct HomeScreen {
    selected: usize,
}

/// One step of the getting-started list.
struct Step {
    label: &'static str,
    done: bool,
}

fn steps(snap: &Snapshot, check: Option<(usize, usize, usize)>) -> [Step; 4] {
    let project = &snap.project;
    let (ready, total) = project.slots_ready();
    [
        Step { label: "Add a key", done: !snap.store.keys.is_empty() },
        Step { label: "Use it here", done: project.key_applied() },
        Step { label: "Pick models", done: project.has_local && ready == total },
        Step { label: "Run a check", done: check.is_some_and(|(_, _, fail)| fail == 0) },
    ]
}

impl HomeScreen {
    pub fn handle(&mut self, code: KeyCode, _snap: &Snapshot) -> Action {
        match code {
            KeyCode::Up | KeyCode::BackTab => {
                self.selected = (self.selected + ITEMS.len() - 1) % ITEMS.len();
                Action::None
            }
            KeyCode::Down | KeyCode::Tab => {
                self.selected = (self.selected + 1) % ITEMS.len();
                Action::None
            }
            KeyCode::Enter | KeyCode::Right => Action::Open(ITEMS[self.selected].0),
            KeyCode::Char(c @ '1'..='4') => {
                self.selected = c as usize - '1' as usize;
                Action::Open(ITEMS[self.selected].0)
            }
            KeyCode::Esc => Action::Quit,
            _ => Action::None,
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        snap: &Snapshot,
        check: Option<(usize, usize, usize)>,
    ) {
        let body = widgets::page(frame, area, "Home", &HINTS);
        let steps = steps(snap, check);
        // Labels are short enough for two columns down to the minimum width,
        // which keeps the menu below fully on screen at 20 rows.
        let [status, guide, menu, tip] = Layout::vertical([
            Constraint::Length(6),
            Constraint::Length(4),
            Constraint::Length(6),
            Constraint::Min(0),
        ])
        .areas(body);

        render_status(frame, status, snap);
        render_steps(frame, guide, &steps);
        self.render_menu(frame, menu);
        render_tip(frame, tip);
    }

    fn render_menu(&self, frame: &mut Frame, area: Rect) {
        let block = widgets::panel("Go to");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let width = inner.width as usize;
        let lines: Vec<Line> = ITEMS
            .iter()
            .enumerate()
            .map(|(i, (_, name, desc))| {
                let selected = i == self.selected;
                let style = if selected {
                    Style::default().bg(HIGHLIGHT_BG).fg(Color::White)
                } else {
                    Style::default().fg(TEXT)
                };
                let head = format!(
                    " {} {} {:<7} ",
                    if selected { SYM_POINTER } else { " " },
                    i + 1,
                    name
                );
                let room = width.saturating_sub(console::measure_text_width(&head));
                Line::from(vec![
                    Span::styled(head, style.bold()),
                    Span::styled(
                        format!("{:<room$}", fit(desc, room)),
                        if selected { style } else { Style::default().fg(TEXT_DIM) },
                    ),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// Where things are kept, for whoever wonders what this tool just changed.
fn render_tip(frame: &mut Frame, area: Rect) {
    if area.height == 0 {
        return;
    }
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            " Claude Code reads .claude/settings.local.json in this folder;",
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(Span::styled(
            " ccc writes the key and models there. Saved keys: ~/.ccc/keys.json.",
            Style::default().fg(TEXT_DIM),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn render_status(frame: &mut Frame, area: Rect, snap: &Snapshot) {
    let project = &snap.project;
    let block = widgets::panel("This folder");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width as usize;
    let room = width.saturating_sub(10);

    let key = match &project.key {
        Some(_) if project.key_applied() => vec![Span::styled(project.key_label(), Style::default().fg(SUCCESS))],
        Some(_) => vec![Span::styled(project.key_label(), Style::default().fg(WARNING))],
        None => vec![Span::styled("none yet — open Keys and press a", Style::default().fg(WARNING))],
    };
    let rule = if project.filter.is_empty() {
        "any model it lists".to_string()
    } else {
        format!("{} models only", project.filter)
    };
    let gateway_room = room.saturating_sub(rule.len() + 3);
    let gateway = vec![
        Span::styled(fit_middle(&project.base_url, gateway_room), Style::default().fg(TEXT)),
        Span::styled(format!(" · {rule}"), Style::default().fg(TEXT_DIM)),
    ];
    // The model a new session here starts on; the slots are on the Models screen.
    let (ready, total) = project.slots_ready();
    let active = &project.active;
    let model = active.display();
    let models = if !project.has_local {
        vec![
            Span::styled(model, Style::default().fg(WARNING)),
            Span::styled(" · not set up here — open Models", Style::default().fg(TEXT_DIM)),
        ]
    } else {
        let colour = if ready == total { SUCCESS } else { WARNING };
        let note = if ready == total {
            format!(" · discovery {}", if project.discovery { "on" } else { "off" })
        } else {
            format!(" · {} slot(s) unusable", total - ready)
        };
        vec![Span::styled(model, Style::default().fg(colour)), Span::styled(note, Style::default().fg(TEXT_DIM))]
    };

    let lines = vec![
        widgets::row(" Folder", 9, vec![Span::styled(fit_middle(&project.folder, room), Style::default().fg(TEXT))], width),
        widgets::row(" Key", 9, key, width),
        widgets::row(" Gateway", 9, gateway, width),
        widgets::row(" Model", 9, models, width),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_steps(frame: &mut Frame, area: Rect, steps: &[Step; 4]) {
    let block = widgets::panel("Getting started");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let next = steps.iter().position(|s| !s.done);
    let cell = |i: usize| -> Vec<Span> {
        let step = &steps[i];
        let (mark, colour) = if step.done { (SYM_OK, SUCCESS) } else { (SYM_TODO, TEXT_DIM) };
        let mut spans = vec![
            Span::styled(format!(" {mark} "), Style::default().fg(colour).bold()),
            Span::styled(
                format!("{} {}", i + 1, step.label),
                Style::default().fg(if step.done { TEXT_DIM } else { TEXT }),
            ),
        ];
        if next == Some(i) {
            spans.push(Span::styled(" ← next", Style::default().fg(KEY_HINT).bold()));
        }
        spans
    };

    // Steps 1-2 down the left, 3-4 down the right.
    let half = inner.width as usize / 2;
    let lines: Vec<Line> = (0..2)
        .map(|row| {
            let mut left = cell(row);
            let used: usize = left.iter().map(|s| s.width()).sum();
            left.push(Span::raw(" ".repeat(half.saturating_sub(used))));
            left.extend(cell(row + 2));
            Line::from(left)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
