use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

use super::component::{Action, KeyOp, ModeSwitch};
use super::components::confirm_modal::ConfirmModal;
use super::components::dashboard::Dashboard;
use super::components::footer::{Footer, FooterMode};
use super::components::header::Header;
use super::components::home::HomeDashboard;
use super::components::input_modal::InputModal;
use super::components::key_table::KeyTable;
use super::components::status_dashboard::StatusDashboard;
use super::components::toast::Toast;
use super::theme::{BG, ICON_CHECK};

use crate::api::validate_key_format;
use crate::config::{local_settings_path, read_json_or_default, set_auth_token, write_json, KeysStore};

// ── App modes ──

enum Mode {
    Home,
    KeyDashboard,
    Normal,
    AddName(InputModal),
    AddValue(InputModal),
    Rename(InputModal),
    ConfirmRemove(ConfirmModal),
    Status(StatusDashboard),
    Message(Toast),
}

// ── App orchestrator ──

pub struct App {
    mode: Mode,
    pub key_table: KeyTable,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        App {
            mode: Mode::Home,
            key_table: KeyTable::load(),
            should_quit: false,
        }
    }

    pub fn has_pending_status(&self) -> bool {
        matches!(&self.mode, Mode::Status(s) if s.is_pending())
    }

    pub fn poll_status(&mut self) {
        if let Mode::Status(dashboard) = &mut self.mode {
            dashboard.poll();
        }
    }

    // ── Event routing ──

    pub fn handle_event(&mut self, key: KeyCode) {
        let action = match &mut self.mode {
            Mode::Home => self.handle_home_event(key),
            Mode::KeyDashboard => self.handle_dashboard_event(key),
            Mode::Normal => self.key_table.handle_event(key),
            Mode::AddName(modal) => modal.handle_event(key),
            Mode::AddValue(modal) => modal.handle_event(key),
            Mode::Rename(modal) => modal.handle_event(key),
            Mode::ConfirmRemove(modal) => modal.handle_event(key),
            Mode::Status(dashboard) => dashboard.handle_event(key),
            Mode::Message(_) => Action::Switch(ModeSwitch::Normal),
        };
        self.process_action(action);
    }

    fn handle_home_event(&mut self, key: KeyCode) -> Action {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Char('k') | KeyCode::Char('1') | KeyCode::Right => Action::Switch(ModeSwitch::ToKeyDashboard),
            KeyCode::Char('s') | KeyCode::Char('2') => Action::Switch(ModeSwitch::ToShow),
            _ => Action::None,
        }
    }

    fn handle_dashboard_event(&mut self, key: KeyCode) -> Action {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Char('a') => Action::Switch(ModeSwitch::AddName),
            KeyCode::Char('s') => Action::Op(KeyOp::Status),
            KeyCode::Right | KeyCode::Char('l') => Action::Switch(ModeSwitch::ToNormal),
            KeyCode::Left | KeyCode::Char('h') => Action::Switch(ModeSwitch::ToHome),
            _ => Action::None,
        }
    }

    fn process_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Quit => self.should_quit = true,
            Action::Notify { text, success } => {
                self.mode = Mode::Message(Toast { text, success });
            }
            Action::Op(op) => self.execute_op(op),
            Action::Switch(switch) => self.switch_mode(switch),
        }
    }

    fn switch_mode(&mut self, switch: ModeSwitch) {
        self.mode = match switch {
            ModeSwitch::Normal => Mode::Normal,
            ModeSwitch::ToNormal => Mode::Normal,
            ModeSwitch::ToHome => Mode::Home,
            ModeSwitch::ToKeyDashboard => Mode::KeyDashboard,
            ModeSwitch::ToShow => {
                // Execute show command and quit
                if let Err(e) = crate::commands::show::run(crate::ShowTarget::Global) {
                    eprintln!("Error: {e}");
                }
                self.should_quit = true;
                Mode::Home
            }
            ModeSwitch::AddName => {
                Mode::AddName(InputModal::new(
                    "Add Key", "Key name (e.g. work, personal):", 1, 2,
                    |name| {
                        if name.is_empty() {
                            Action::Notify { text: "Name cannot be empty.".into(), success: false }
                        } else {
                            Action::Switch(ModeSwitch::AddValue { name })
                        }
                    },
                ))
            }
            ModeSwitch::AddValue { name } => {
                let name_clone = name.clone();
                Mode::AddValue(InputModal::new(
                    format!("Add Key — '{name}'"), "API key value:", 2, 2,
                    move |value| Action::Op(KeyOp::Add { name: name_clone.clone(), value }),
                ))
            }
            ModeSwitch::Rename { old_name } => {
                let old = old_name.clone();
                Mode::Rename(InputModal::new(
                    format!("Rename '{old_name}'"), "New name:", 1, 1,
                    move |new_name| Action::Op(KeyOp::Rename { old_name: old.clone(), new_name }),
                ))
            }
            ModeSwitch::ConfirmRemove(name) => {
                Mode::ConfirmRemove(ConfirmModal { name })
            }
        };
    }

    // ── Key operations ──

    fn execute_op(&mut self, op: KeyOp) {
        match op {
            KeyOp::Add { name, value } => self.do_add(&name, &value),
            KeyOp::Default(name) => self.do_default(&name),
            KeyOp::Use(name) => self.do_use(&name),
            KeyOp::Remove(name) => self.do_remove(&name),
            KeyOp::Rename { old_name, new_name } => self.do_rename(&old_name, &new_name),
            KeyOp::Status => self.do_status(),
        }
    }

    fn notify(&mut self, text: String, success: bool) {
        self.mode = Mode::Message(Toast { text, success });
    }

    fn do_add(&mut self, name: &str, value: &str) {
        if name.is_empty() {
            self.notify("Name cannot be empty.".into(), false);
            return;
        }
        if let Err(e) = validate_key_format(value) {
            self.notify(e, false);
            return;
        }
        let mut store = KeysStore::load();
        let is_first = store.active.is_none();
        store.keys.insert(name.to_string(), value.to_string());
        if is_first { store.active = Some(name.to_string()); }
        if let Err(e) = store.save() {
            self.notify(format!("Failed to save: {e}"), false);
            return;
        }
        let msg = if is_first {
            format!("{} Added '{name}' and set as default.", ICON_CHECK)
        } else {
            format!("{} Added '{name}'.", ICON_CHECK)
        };
        self.notify(msg, true);
        self.key_table.reload();
    }

    fn do_default(&mut self, name: &str) {
        let mut store = KeysStore::load();
        if !store.keys.contains_key(name) {
            self.notify(format!("Key '{name}' not found."), false);
            return;
        }
        store.active = Some(name.to_string());
        if let Err(e) = store.save() {
            self.notify(format!("Failed to save: {e}"), false);
            return;
        }
        self.notify(format!("{} Set '{name}' as default.", ICON_CHECK), true);
        self.key_table.reload();
    }

    fn do_use(&mut self, name: &str) {
        let store = KeysStore::load();
        let key_value = match store.keys.get(name) {
            Some(v) => v.clone(),
            None => { self.notify(format!("Key '{name}' not found."), false); return; }
        };
        let local_path = local_settings_path();
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut json = if local_path.exists() { read_json_or_default(&local_path) } else { serde_json::json!({}) };
        set_auth_token(&mut json, &key_value);
        if let Err(e) = write_json(&local_path, &json) {
            self.notify(format!("Failed to write config: {e}"), false);
            return;
        }
        let folder = std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_else(|_| ".".into());
        self.notify(format!("{} Using '{name}' for {folder}", ICON_CHECK), true);
    }

    fn do_remove(&mut self, name: &str) {
        let mut store = KeysStore::load();
        store.keys.remove(name);
        if store.active.as_deref() == Some(name) {
            store.active = store.keys.keys().next().cloned();
        }
        if let Err(e) = store.save() {
            self.notify(format!("Failed to save: {e}"), false);
            return;
        }
        self.notify(format!("{} Removed '{name}'.", ICON_CHECK), true);
        self.key_table.reload();
    }

    fn do_rename(&mut self, old_name: &str, new_name: &str) {
        if new_name.is_empty() {
            self.notify("Name cannot be empty.".into(), false);
            return;
        }
        if new_name == old_name {
            self.mode = Mode::Normal;
            return;
        }
        let mut store = KeysStore::load();
        if store.keys.contains_key(new_name) {
            self.notify(format!("Key '{new_name}' already exists."), false);
            return;
        }
        if let Some(value) = store.keys.remove(old_name) {
            store.keys.insert(new_name.to_string(), value);
            if store.active.as_deref() == Some(old_name) {
                store.active = Some(new_name.to_string());
            }
            if let Err(e) = store.save() {
                self.notify(format!("Failed to save: {e}"), false);
                return;
            }
            self.notify(format!("{} Renamed '{old_name}' → '{new_name}'.", ICON_CHECK), true);
            self.key_table.reload();
        }
    }

    fn do_status(&mut self) {
        match StatusDashboard::new() {
            Some(dashboard) => self.mode = Mode::Status(dashboard),
            None => self.notify("No keys to check.".into(), false),
        }
    }

    // ── Rendering ──

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Block::default().style(Style::default().bg(BG)), area);

        // Home is full-screen
        if let Mode::Home = &self.mode {
            HomeDashboard::new().render(frame, area);
            return;
        }

        // Key Dashboard is full-screen
        if let Mode::KeyDashboard = &self.mode {
            Dashboard::new().render(frame, area);
            return;
        }

        // Status dashboard is full-screen
        if let Mode::Status(dashboard) = &self.mode {
            dashboard.render(frame, area);
            return;
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(3), Constraint::Min(5), Constraint::Length(3)])
            .split(area);

        // Header
        Header { total: self.key_table.entries.len() }.render(frame, layout[0]);

        // Key table
        self.key_table.render(frame, layout[1]);

        // Footer
        let footer_mode = match &self.mode {
            Mode::Normal => FooterMode::Normal,
            Mode::AddName(_) | Mode::AddValue(_) | Mode::Rename(_) => FooterMode::Input,
            Mode::ConfirmRemove(_) => FooterMode::Confirm,
            Mode::Status(_) | Mode::Message(_) => FooterMode::Dismiss,
            Mode::Home | Mode::KeyDashboard => FooterMode::Dismiss,
        };
        Footer { mode: footer_mode }.render(frame, layout[2]);

        // Modal overlays
        match &self.mode {
            Mode::AddName(modal) | Mode::AddValue(modal) | Mode::Rename(modal) => {
                modal.render(frame, area);
            }
            Mode::ConfirmRemove(modal) => modal.render(frame, area),
            Mode::Message(toast) => toast.render(frame, area),
            _ => {}
        }
    }
}
