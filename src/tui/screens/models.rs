//! The model picker: ask the gateway which models a key can use, then fill
//! each of Claude Code's model slots from that list and save it to this
//! folder's `.claude/settings.local.json`.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{prelude::*, widgets::*};

use super::super::app::{Action, Ask, Prompt, Snapshot};
use super::super::theme::*;
use super::super::widgets::{self, fit, fit_middle, fit_models};
use crate::api::{is_listed, list_models_at, usable_models, ApiError, KeySource};
use crate::config::{
    default_settings_path, env_value, load_local_for_edit, read_json_or_default, set_auth_token,
    write_json, ENV_AUTH_TOKEN, ENV_BASE_URL, ENV_GATEWAY_DISCOVERY,
};
use crate::slots::{self, CUSTOM, SLOTS};
use crate::utils::{ensure_local_settings_ignored, mask_key};

type Values = [Option<String>; 5];

/// Which key the list is fetched with, and against which gateway.
#[derive(Clone)]
struct Source {
    key_name: String,
    key_value: String,
    /// Where the key came from, for the header line.
    from: &'static str,
    /// Whether this folder's settings already hold this key.
    folder_uses_it: bool,
    base_url: String,
    filter: String,
}

/// The folder's model settings when the picker opened.
#[derive(Clone)]
struct Start {
    values: Values,
    discovery: bool,
    /// No settings in this folder yet: the values come from ccc's template.
    from_template: bool,
}

impl Start {
    fn read(snap: &Snapshot) -> Self {
        let project = &snap.project;
        if project.has_local {
            return Start { values: project.slots.clone(), discovery: project.discovery, from_template: false };
        }
        let template = default_settings_path()
            .ok()
            .filter(|p| p.exists())
            .map(|p| read_json_or_default(&p))
            .unwrap_or_else(|| serde_json::json!({}));
        Start {
            values: slots::read_all(&template),
            discovery: template["env"][ENV_GATEWAY_DISCOVERY].as_str() == Some("1"),
            from_template: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Slots,
    Models,
}

struct Picker {
    source: Source,
    start: Start,
    listed: usize,
    usable: Vec<String>,
    values: Values,
    discovery: bool,
    focus: Focus,
    slot: usize,
    cursor: usize,
}

impl Picker {
    fn dirty(&self) -> bool {
        self.values != self.start.values || self.discovery != self.start.discovery
    }

    fn choices(&self) -> Vec<String> {
        slots::ordered_for(self.slot, &self.usable)
    }

    fn slot_ok(&self, i: usize) -> bool {
        match &self.values[i] {
            Some(v) => i == CUSTOM || is_listed(v, &self.usable),
            None => SLOTS[i].optional,
        }
    }
}

enum State {
    Idle,
    Loading {
        source: Source,
        start: Start,
        /// Picks made before a reload, re-applied once the list is back.
        edits: Option<(Values, bool)>,
        rx: Receiver<Result<Vec<String>, ApiError>>,
    },
    Failed {
        source: Option<Source>,
        start: Start,
        message: String,
        rejected: bool,
    },
    Ready(Box<Picker>),
}

pub struct ModelsScreen {
    state: State,
}

impl Default for ModelsScreen {
    fn default() -> Self {
        ModelsScreen { state: State::Idle }
    }
}

impl ModelsScreen {
    /// Open the picker, fetching with `key` (name, value) or, without one,
    /// with the key this folder resolves to.
    pub fn open(&mut self, snap: &Snapshot, key: Option<(String, String)>) {
        let project = &snap.project;
        let start = Start::read(snap);
        let folder_key = project
            .key
            .as_ref()
            .filter(|(_, source)| *source == KeySource::Folder)
            .map(|(value, _)| value.clone());

        let source = match key {
            Some((name, value)) => Source {
                folder_uses_it: folder_key.as_deref() == Some(value.as_str()),
                key_name: name,
                key_value: value,
                from: "chosen in Keys",
                base_url: project.base_url.clone(),
                filter: project.filter.clone(),
            },
            None => match &project.key {
                Some((value, from)) => Source {
                    key_name: project.key_name.clone().unwrap_or_else(|| mask_key(value)),
                    key_value: value.clone(),
                    from: from.describe(),
                    folder_uses_it: *from == KeySource::Folder,
                    base_url: project.base_url.clone(),
                    filter: project.filter.clone(),
                },
                None => {
                    self.state = State::Failed {
                        source: None,
                        start,
                        message: "There is no key to ask the gateway with yet.".into(),
                        rejected: true,
                    };
                    return;
                }
            },
        };
        self.fetch(source, start, None);
    }

    fn fetch(&mut self, source: Source, start: Start, edits: Option<(Values, bool)>) {
        let (tx, rx) = mpsc::channel();
        let (base_url, key) = (source.base_url.clone(), source.key_value.clone());
        thread::spawn(move || {
            let _ = tx.send(list_models_at(&base_url, &key));
        });
        self.state = State::Loading { source, start, edits, rx };
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.state, State::Loading { .. })
    }

    pub fn is_dirty(&self) -> bool {
        matches!(&self.state, State::Ready(p) if p.dirty())
    }

    /// Pick up the gateway's answer once it arrives.
    pub fn poll(&mut self) {
        let State::Loading { rx, .. } = &self.state else {
            return;
        };
        let outcome = match rx.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(ApiError::Unexpected("the lookup stopped unexpectedly".into())),
        };
        let State::Loading { source, start, edits, .. } = std::mem::replace(&mut self.state, State::Idle) else {
            return;
        };
        self.state = match outcome {
            Ok(models) => {
                let usable = usable_models(&models, &source.filter);
                let (values, discovery) = edits.unwrap_or_else(|| (start.values.clone(), start.discovery));
                State::Ready(Box::new(Picker {
                    listed: models.len(),
                    usable,
                    values,
                    discovery,
                    focus: Focus::Slots,
                    slot: 0,
                    cursor: 0,
                    source,
                    start,
                }))
            }
            Err(e) => State::Failed {
                rejected: matches!(e, ApiError::Rejected(_)),
                message: e.to_string(),
                source: Some(source),
                start,
            },
        };
    }

    pub fn set_custom(&mut self, value: &str) {
        if let State::Ready(p) = &mut self.state {
            let value = value.trim();
            p.values[CUSTOM] = (!value.is_empty()).then(|| value.to_string());
        }
    }

    pub fn discard(&mut self) {
        if let State::Ready(p) = &mut self.state {
            p.values = p.start.values.clone();
            p.discovery = p.start.discovery;
        }
    }

    // ── Keys ──

    pub fn handle(&mut self, key: KeyEvent, _snap: &Snapshot) -> Action {
        let code = key.code;
        match &mut self.state {
            State::Idle | State::Loading { .. } => match code {
                KeyCode::Esc | KeyCode::Left => Action::Back,
                _ => Action::None,
            },
            State::Failed { source, start, .. } => match code {
                KeyCode::Char('r') => match source.clone() {
                    Some(source) => {
                        let start = start.clone();
                        self.fetch(source, start, None);
                        Action::None
                    }
                    None => Action::Open(super::super::app::Screen::Keys),
                },
                KeyCode::Char('k') => Action::Open(super::super::app::Screen::Keys),
                KeyCode::Esc | KeyCode::Left => Action::Back,
                _ => Action::None,
            },
            State::Ready(_) => self.handle_ready(key),
        }
    }

    fn handle_ready(&mut self, key: KeyEvent) -> Action {
        let State::Ready(p) = &mut self.state else {
            return Action::None;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('s') if ctrl => return self.request_save(),
            KeyCode::Char('w') => return self.request_save(),
            KeyCode::Char('r') => {
                let edits = Some((p.values.clone(), p.discovery));
                let (source, start) = (p.source.clone(), p.start.clone());
                self.fetch(source, start, edits);
                return Action::None;
            }
            KeyCode::Char('g') => {
                p.discovery = !p.discovery;
                return Action::None;
            }
            KeyCode::Char('f') => {
                let mut json = serde_json::json!({});
                for (i, value) in p.values.iter().enumerate() {
                    slots::write(&mut json, i, value.as_deref());
                }
                let changes = slots::fit(&mut json, &p.usable);
                p.values = slots::read_all(&json);
                let text = match changes.len() {
                    0 if p.usable.is_empty() => "No usable model to fill the slots with.".to_string(),
                    0 => "Every slot already has a usable model.".to_string(),
                    1 => "Filled 1 slot. Press w to save.".to_string(),
                    n => format!("Filled {n} slots. Press w to save."),
                };
                return Action::Notify { text, success: !p.usable.is_empty() };
            }
            KeyCode::Char('1') => {
                return match p.values[p.slot].clone() {
                    Some(v) if slots::supports_1m(&v) => {
                        p.values[p.slot] = Some(slots::with_1m(&v, !slots::has_1m(&v)));
                        Action::None
                    }
                    _ => Action::Notify {
                        text: "1M context exists only for Sonnet and Opus models.".into(),
                        success: false,
                    },
                };
            }
            KeyCode::Char('e') => {
                return if p.slot == CUSTOM {
                    Action::Prompt(Prompt::CustomModel { current: p.values[CUSTOM].clone() })
                } else {
                    Action::Notify {
                        text: "Only the Custom slot takes a typed id; pick this one from the list.".into(),
                        success: false,
                    }
                };
            }
            KeyCode::Char('x') | KeyCode::Delete | KeyCode::Backspace => {
                return if p.slot == CUSTOM {
                    p.values[CUSTOM] = None;
                    Action::None
                } else {
                    Action::Notify {
                        text: format!(
                            "The {} slot cannot be empty: Claude Code would fall back to a built-in model (403).",
                            SLOTS[p.slot].label
                        ),
                        success: false,
                    }
                };
            }
            _ => {}
        }

        match p.focus {
            Focus::Slots => match key.code {
                KeyCode::Up | KeyCode::Char('k') => p.slot = p.slot.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => p.slot = (p.slot + 1).min(SLOTS.len() - 1),
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                    let choices = p.choices();
                    if choices.is_empty() {
                        return Action::Notify {
                            text: "The gateway lists no usable model for this key.".into(),
                            success: false,
                        };
                    }
                    p.cursor = p.values[p.slot]
                        .as_deref()
                        .and_then(|v| choices.iter().position(|m| is_listed(v, std::slice::from_ref(m))))
                        .unwrap_or(0);
                    p.focus = Focus::Models;
                }
                KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => return Action::Back,
                _ => {}
            },
            Focus::Models => {
                let choices = p.choices();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => p.cursor = p.cursor.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => {
                        p.cursor = (p.cursor + 1).min(choices.len().saturating_sub(1))
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if let Some(model) = choices.get(p.cursor) {
                            // Keep a 1M choice when the new model has a 1M variant.
                            let wants_1m = p.values[p.slot].as_deref().is_some_and(slots::has_1m);
                            p.values[p.slot] = Some(slots::with_1m(model, wants_1m));
                        }
                        p.focus = Focus::Slots;
                    }
                    KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => p.focus = Focus::Slots,
                    _ => {}
                }
            }
        }
        Action::None
    }

    fn request_save(&mut self) -> Action {
        let State::Ready(p) = &self.state else {
            return Action::None;
        };
        for (i, slot) in SLOTS.iter().enumerate() {
            match &p.values[i] {
                None if !slot.optional => {
                    return Action::Notify {
                        text: format!(
                            "Set the {} slot first: an empty slot falls back to a built-in model (403). f fills empty slots.",
                            slot.label
                        ),
                        success: false,
                    };
                }
                Some(v) if i != CUSTOM && !is_listed(v, &p.usable) => {
                    return Action::Notify {
                        text: format!("{v} is not available to this key. Pick another {} model, or press f.", slot.label),
                        success: false,
                    };
                }
                _ => {}
            }
        }
        if p.source.folder_uses_it {
            self.save(false)
        } else {
            Action::Confirm(Ask::SaveModelsWithKey { key_name: p.source.key_name.clone() })
        }
    }

    /// Write the picks to this folder, optionally with the picker's key.
    pub fn save(&mut self, use_key: bool) -> Action {
        let State::Ready(p) = &mut self.state else {
            return Action::None;
        };
        match write(p, use_key) {
            Ok(text) => {
                p.start = Start { values: p.values.clone(), discovery: p.discovery, from_template: false };
                if use_key {
                    p.source.folder_uses_it = true;
                }
                Action::Notify { text, success: true }
            }
            Err(e) => Action::Notify { text: format!("Could not save: {e:#}"), success: false },
        }
    }

    // ── Drawing ──

    pub fn render(&self, frame: &mut Frame, area: Rect, tick: usize) {
        const WAIT_HINTS: [widgets::Hint; 2] = [("Esc", "back"), ("?", "help")];
        const FAIL_HINTS: [widgets::Hint; 4] = [("r", "retry"), ("k", "keys"), ("Esc", "back"), ("?", "help")];
        const SLOT_HINTS: [widgets::Hint; 8] = [
            ("↑↓", "slot"),
            ("→", "choose"),
            ("w", "save"),
            ("f", "fill"),
            ("1", "1M"),
            ("g", "discovery"),
            ("Esc", "back"),
            ("?", "help"),
        ];
        const MODEL_HINTS: [widgets::Hint; 5] =
            [("↑↓", "model"), ("Enter", "pick"), ("←", "slots"), ("w", "save"), ("?", "help")];

        let hints: &[widgets::Hint] = match &self.state {
            State::Idle | State::Loading { .. } => &WAIT_HINTS,
            State::Failed { .. } => &FAIL_HINTS,
            State::Ready(p) if p.focus == Focus::Models => &MODEL_HINTS,
            State::Ready(_) => &SLOT_HINTS,
        };
        let body = widgets::page(frame, area, "Models", hints);

        match &self.state {
            State::Idle => {}
            State::Loading { source, .. } => {
                let lines = vec![
                    Line::from(vec![
                        Span::styled(format!("{} ", widgets::spinner(tick)), Style::default().fg(ACCENT)),
                        Span::styled(
                            format!("Asking the gateway which models '{}' can use…", source.key_name),
                            Style::default().fg(TEXT),
                        ),
                    ]),
                    Line::from(Span::styled(
                        fit_middle(&source.base_url, body.width as usize),
                        Style::default().fg(TEXT_DIM),
                    )),
                ];
                let y = body.y + body.height.saturating_sub(2) / 2;
                frame.render_widget(
                    Paragraph::new(lines).alignment(Alignment::Center),
                    Rect::new(body.x, y, body.width, 2.min(body.height)),
                );
            }
            State::Failed { source, message, rejected, .. } => render_failed(frame, body, source.as_ref(), message, *rejected),
            State::Ready(p) => render_picker(frame, body, p),
        }
    }
}

fn write(p: &Picker, use_key: bool) -> Result<String> {
    let (path, mut json) = load_local_for_edit()?;
    for (i, value) in p.values.iter().enumerate() {
        slots::write(&mut json, i, value.as_deref());
    }
    if p.discovery {
        json["env"][ENV_GATEWAY_DISCOVERY] = serde_json::Value::String("1".into());
    } else if let Some(env) = json["env"].as_object_mut() {
        env.remove(ENV_GATEWAY_DISCOVERY);
    }
    // Models belong to a gateway; a folder that names models but no gateway
    // would send them wherever the next settings file points.
    if env_value(&json, ENV_BASE_URL).is_none() {
        json["env"][ENV_BASE_URL] = serde_json::Value::String(p.source.base_url.clone());
    }
    if use_key {
        set_auth_token(&mut json, &p.source.key_value);
    }
    write_json(&path, &json)?;

    let mut text = format!("Saved to {}. Restart Claude Code to use them.", path.display());
    if env_value(&json, ENV_AUTH_TOKEN).is_some()
        && let Some(note) = ensure_local_settings_ignored()
    {
        text.push(' ');
        text.push_str(&note);
    }
    Ok(text)
}

fn render_failed(frame: &mut Frame, area: Rect, source: Option<&Source>, message: &str, rejected: bool) {
    let block = widgets::panel("Couldn't load models");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![Line::from(Span::styled(message.to_string(), Style::default().fg(ERROR)))];
    if let Some(source) = source {
        lines.push(Line::from(Span::styled(
            format!("Key {} ({}) · {}", source.key_name, source.from, source.base_url),
            Style::default().fg(TEXT_DIM),
        )));
    }
    lines.push(Line::from(""));
    let advice: &[&str] = match (source.is_some(), rejected) {
        (false, _) => &[
            "1. Press k to open Keys and add the key you were given (a).",
            "2. Press u there to use it in this folder.",
            "3. Come back here: Esc, then 2.",
        ],
        (true, true) => &[
            "The gateway refused this key. It may be mistyped, expired or revoked.",
            "Press k to check the key, or pick another one and press m on it.",
        ],
        (true, false) => &[
            "The gateway could not be reached or did not answer as expected.",
            "Check the network and the base URL (Config), then press r to retry.",
        ],
    };
    lines.extend(advice.iter().map(|a| Line::from(Span::styled(*a, Style::default().fg(TEXT)))));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_picker(frame: &mut Frame, area: Rect, p: &Picker) {
    let [info, lists, notes] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(7), Constraint::Length(4)]).areas(area);

    // Which key, which gateway, how much it offers.
    let width = info.width as usize;
    let counts = if p.source.filter.is_empty() {
        format!("{} listed, all usable", p.listed)
    } else {
        format!("{} listed, {} usable ({})", p.listed, p.usable.len(), p.source.filter)
    };
    let gateway_room = width.saturating_sub(counts.len() + 13);
    let info_lines = vec![
        widgets::row(
            "Key",
            8,
            vec![
                Span::styled(p.source.key_name.clone(), Style::default().fg(Color::White).bold()),
                Span::styled(format!(" · {}", p.source.from), Style::default().fg(TEXT_DIM)),
            ],
            width,
        ),
        widgets::row(
            "Gateway",
            8,
            vec![
                Span::styled(fit_middle(&p.source.base_url, gateway_room), Style::default().fg(TEXT)),
                Span::styled(format!(" · {counts}"), Style::default().fg(TEXT_DIM)),
            ],
            width,
        ),
    ];
    frame.render_widget(Paragraph::new(info_lines), info);

    // Wide enough at 80 columns for the longest team id with its [1m].
    let slots_width = (lists.width * 48 / 100).clamp(30, 44);
    let [slot_area, model_area] =
        Layout::horizontal([Constraint::Length(slots_width), Constraint::Min(20)]).areas(lists);
    render_slots(frame, slot_area, p);
    render_models(frame, model_area, p);
    render_notes(frame, notes, p);
}

fn render_slots(frame: &mut Frame, area: Rect, p: &Picker) {
    let block = widgets::panel("Slots").border_style(Style::default().fg(if p.focus == Focus::Slots {
        ACCENT
    } else {
        BORDER
    }));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width as usize;
    // "▶ " + 7-column label + space before the value, space + mark after it.
    let room = width.saturating_sub(2 + 7 + 1 + 2);
    let shown = fit_models(
        p.values.iter().map(|v| v.as_deref().unwrap_or("not set")),
        room,
    );

    let lines: Vec<Line> = SLOTS
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            let selected = i == p.slot;
            let (mark, colour) = match (&p.values[i], p.slot_ok(i)) {
                (None, _) if slot.optional => (SYM_INFO, TEXT_DIM),
                (_, true) => (SYM_OK, SUCCESS),
                (Some(_), false) if i == CUSTOM => (SYM_WARN, WARNING),
                (_, false) => (SYM_FAIL, ERROR),
            };
            let changed = p.values[i] != p.start.values[i];
            let head = format!("{} {:<7}", if selected { SYM_POINTER } else { " " }, slot.label);
            let row_style = if selected && p.focus == Focus::Slots {
                Style::default().bg(HIGHLIGHT_BG)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::styled(head, Style::default().fg(if selected { Color::White } else { TEXT }).bold()),
                Span::raw(" "),
                Span::styled(
                    format!("{:<room$}", shown[i]),
                    Style::default().fg(if changed {
                        ACCENT
                    } else if p.values[i].is_some() {
                        TEXT
                    } else {
                        TEXT_DIM
                    }),
                ),
                Span::raw(" "),
                Span::styled(mark, Style::default().fg(colour).bold()),
            ])
            .style(row_style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_models(frame: &mut Frame, area: Rect, p: &Picker) {
    let title = format!("Models for {}", SLOTS[p.slot].label);
    let focused = p.focus == Focus::Models;
    let block = widgets::panel(&title)
        .border_style(Style::default().fg(if focused { ACCENT } else { BORDER }))
        .title_bottom(
            Line::from(Span::styled(format!(" {SYM_HERE} current "), Style::default().fg(SUCCESS)))
                .right_aligned(),
        );
    let choices = p.choices();

    if choices.is_empty() {
        let text = if p.source.filter.is_empty() {
            "The gateway lists no model for this key.".to_string()
        } else {
            format!("No {} model for this key. Ask for access, or check the filter (Config).", p.source.filter)
        };
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(WARNING)).wrap(Wrap { trim: true }).block(block),
            area,
        );
        return;
    }

    // Two columns go to the highlight symbol, two to the "current" dot.
    let width = block.inner(area).width.saturating_sub(4) as usize;
    let current = p.values[p.slot].as_deref();
    // Room for the id once the widest tag ("1M") and its space are kept free.
    let shown = fit_models(choices.iter().map(String::as_str), width.saturating_sub(3));
    let items: Vec<ListItem> = choices
        .iter()
        .zip(&shown)
        .map(|(id, shown)| {
            let is_current = current.is_some_and(|v| is_listed(v, std::slice::from_ref(id)));
            let mut tags = Vec::new();
            if slots::supports_1m(id) {
                tags.push("1M");
            }
            if p.discovery && !slots::shows_in_picker(id) {
                tags.push("not in /model");
            }
            let tags = tags.join(" ");
            let room = width.saturating_sub(console::measure_text_width(&tags) + 1);
            ListItem::new(Line::from(vec![
                Span::styled(if is_current { SYM_HERE } else { " " }, Style::default().fg(SUCCESS)),
                Span::raw(" "),
                Span::styled(format!("{:<room$}", widgets::fit(shown, room)), Style::default().fg(TEXT)),
                Span::raw(" "),
                Span::styled(tags, Style::default().fg(TEXT_DIM)),
            ]))
        })
        .collect();

    // Without focus the list still scrolls to the current model, unhighlighted.
    let selected = if focused {
        Some(p.cursor)
    } else {
        current.and_then(|v| choices.iter().position(|m| is_listed(v, std::slice::from_ref(m))))
    };
    let list = List::new(items)
        .block(block)
        .highlight_symbol(if focused { "▶ " } else { "  " })
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_style(if focused {
            Style::default().bg(HIGHLIGHT_BG).fg(Color::White).bold()
        } else {
            Style::default()
        });
    let mut state = ListState::default().with_selected(selected);
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_notes(frame: &mut Frame, area: Rect, p: &Picker) {
    let width = area.width as usize;
    let slot = &SLOTS[p.slot];
    let value = p.values[p.slot].as_deref();

    let context = match value {
        _ if p.slot == CUSTOM => "e types any id · x clears it".to_string(),
        Some(v) if slots::supports_1m(v) && slots::has_1m(v) => "1M context: on · 1 turns it off".to_string(),
        Some(v) if slots::supports_1m(v) => "1M context: off · 1 turns it on".to_string(),
        Some(_) => "1M context: not available for this model".to_string(),
        None => "Not set: pick a model with →".to_string(),
    };
    let discovery = if p.discovery {
        "Discovery on: /model lists gateway models · g toggles"
    } else {
        "Discovery off: /model shows aliases only · g toggles"
    };
    let (save_text, save_colour) = if p.dirty() {
        ("Unsaved · w saves to .claude/settings.local.json", WARNING)
    } else if p.start.from_template {
        ("From ccc's template · w writes it to this folder", TEXT_DIM)
    } else {
        ("Saved in .claude/settings.local.json", TEXT_DIM)
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(format!("{}: ", slot.label), Style::default().fg(ACCENT).bold()),
            Span::styled(fit(slot.help, width.saturating_sub(slot.label.len() + 2)), Style::default().fg(TEXT)),
        ]),
        Line::from(Span::styled(fit(&context, width), Style::default().fg(TEXT_DIM))),
        Line::from(Span::styled(fit(discovery, width), Style::default().fg(TEXT_DIM))),
        Line::from(Span::styled(fit(save_text, width), Style::default().fg(save_colour))),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use super::super::super::app::tests::{draw, fake_snapshot, press};
    use super::super::super::app::{App, Screen};
    use super::*;

    fn ready_app(models: &[&str]) -> App {
        let snap = fake_snapshot(true);
        let mut app = App::with_snapshot(Screen::Home, snap);
        let start = Start::read(&app.snap);
        let source = Source {
            key_name: "work".into(),
            key_value: "sk-work-1234567890abcd".into(),
            from: "this folder",
            folder_uses_it: true,
            base_url: crate::api::TEAM_BASE_URL.into(),
            filter: "-vn".into(),
        };
        let models: Vec<String> = models.iter().map(|s| s.to_string()).collect();
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(models)).unwrap();
        let screen = app.models_mut();
        screen.state = State::Loading { source, start, edits: None, rx };
        screen.poll();
        app.screen = Screen::Models;
        app
    }

    const LIST: [&str; 6] = [
        "claude-haiku-4-5-vn",
        "claude-opus-4-8-vn",
        "claude-opus-5-vn",
        "claude-sonnet-4-6-vn",
        "claude-sonnet-5-vn",
        "claude-sonnet-5",
    ];

    #[test]
    fn picker_fits_every_supported_size() {
        let app = ready_app(&LIST);
        for (w, h) in [(64, 20), (80, 24), (120, 40)] {
            let out = draw(&app, w, h);
            assert!(out.contains("Slots"), "{w}x{h}:\n{out}");
            assert!(out.contains("? help"), "{w}x{h}:\n{out}");
            assert!(out.contains("5 usable"), "{w}x{h}:\n{out}");
            for line in out.lines() {
                assert!(line.starts_with('│') || line.starts_with('╭') || line.starts_with('╰'), "{line}");
            }
        }
    }

    #[test]
    fn built_in_ids_are_not_offered() {
        let app = ready_app(&LIST);
        let out = draw(&app, 120, 40);
        assert!(!out.contains("claude-sonnet-5 "), "built-in id offered:\n{out}");
    }

    #[test]
    fn picking_a_model_marks_the_screen_dirty_and_keeps_1m() {
        let mut app = ready_app(&LIST);
        // Main is claude-sonnet-5-vn[1m]; choose the next Sonnet down.
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        let State::Ready(p) = &app.models_mut().state else { panic!("not ready") };
        assert_eq!(p.values[slots::MAIN].as_deref(), Some("claude-sonnet-4-6-vn[1m]"));
        assert!(app.models_mut().is_dirty());
    }

    #[test]
    fn leaving_with_unsaved_picks_asks_first() {
        let mut app = ready_app(&LIST);
        press(&mut app, KeyCode::Char('1')); // toggle 1M off on Main
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.screen, Screen::Models, "left without asking");
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.screen, Screen::Home);
        assert!(!app.models_mut().is_dirty());
    }

    #[test]
    fn one_million_toggle_refuses_haiku() {
        let mut app = ready_app(&LIST);
        for _ in 0..3 {
            press(&mut app, KeyCode::Down);
        }
        press(&mut app, KeyCode::Char('1'));
        let out = draw(&app, 80, 24);
        assert!(out.contains("Sonnet and Opus"), "{out}");
    }

    #[test]
    fn fill_repairs_a_broken_slot() {
        let mut app = ready_app(&LIST);
        if let State::Ready(p) = &mut app.models_mut().state {
            p.values[slots::OPUS] = Some("claude-opus-5".into());
        }
        press(&mut app, KeyCode::Char('f'));
        let State::Ready(p) = &app.models_mut().state else { panic!("not ready") };
        assert_eq!(p.values[slots::OPUS].as_deref(), Some("claude-opus-5-vn"));
    }

    #[test]
    fn save_refuses_a_slot_the_key_cannot_use() {
        let mut app = ready_app(&LIST);
        if let State::Ready(p) = &mut app.models_mut().state {
            p.values[slots::OPUS] = Some("claude-opus-5".into());
        }
        press(&mut app, KeyCode::Char('w'));
        let out = draw(&app, 100, 30);
        assert!(out.contains("not available to this key"), "{out}");
    }

    #[test]
    fn rejected_key_explains_what_to_do() {
        let snap = fake_snapshot(true);
        let mut app = App::with_snapshot(Screen::Home, snap);
        let start = Start::read(&app.snap);
        let screen = app.models_mut();
        screen.state = State::Failed {
            source: None,
            start,
            message: "There is no key to ask the gateway with yet.".into(),
            rejected: true,
        };
        app.screen = Screen::Models;
        let out = draw(&app, 80, 24);
        assert!(out.contains("Press k to open Keys"), "{out}");
    }
}
