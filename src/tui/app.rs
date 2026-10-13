use std::cell::Cell;
use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{prelude::*, widgets::*};

use super::components::confirm_modal::ConfirmModal;
use super::components::input_modal::InputModal;
use super::components::toast::Toast;
use super::screens::check::CheckScreen;
use super::screens::config::ConfigScreen;
use super::screens::help;
use super::screens::home::HomeScreen;
use super::screens::keys::KeysScreen;
use super::screens::models::ModelsScreen;
use super::screens::status::StatusDashboard;
use super::theme::*;
use super::widgets;

use crate::config::KeysStore;
use crate::keys;
use crate::project::ProjectState;

/// The screens of the full-screen manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Keys,
    Models,
    Config,
    Check,
}

/// Everything the screens show, read from disk in one go and reloaded after
/// each change, so no screen ever shows a stale copy of another's edit.
pub struct Snapshot {
    pub store: KeysStore,
    pub project: ProjectState,
}

impl Snapshot {
    pub fn load() -> Self {
        let store = KeysStore::load();
        let project = ProjectState::load(&store);
        Snapshot { store, project }
    }
}

/// What a screen asks the app to do after a key press. Screens never write
/// files themselves except through the app, so every change happens in one
/// place and is followed by a reload.
pub enum Action {
    None,
    Quit,
    Open(Screen),
    Back,
    Help,
    Notify { text: String, success: bool },
    Prompt(Prompt),
    Confirm(Ask),
    Key(KeyOp),
    /// Open the model picker using a particular saved key.
    PickModelsWith(String),
    TestAllKeys,
}

/// Text the app is waiting for.
pub enum Prompt {
    KeyName,
    KeyValue { name: String },
    Rename { old: String },
    CustomModel { current: Option<String> },
}

/// A yes/no question and what "yes" means.
pub enum Ask {
    RemoveKey(String),
    /// Save the picked models, and also put the picker's key in this folder.
    SaveModelsWithKey { key_name: String },
    /// Leave the model picker without saving, then do `then`.
    DiscardModels { then: Box<Action> },
}

pub enum KeyOp {
    Add { name: String, value: String },
    Default(String),
    Use(String),
    Remove(String),
    Rename { old: String, new: String },
}

enum Overlay {
    Input(InputModal, Prompt),
    Confirm(ConfirmModal, Ask),
    Toast(Toast),
    Help,
}

pub struct App {
    pub screen: Screen,
    overlay: Option<Overlay>,
    pub should_quit: bool,
    pub snap: Snapshot,
    /// Counts redraws while work is pending, to animate spinners.
    pub tick: usize,
    home: HomeScreen,
    keys: KeysScreen,
    models: ModelsScreen,
    config: ConfigScreen,
    check: CheckScreen,
    /// "Test every key", shown over the Keys screen while open.
    status: Option<StatusDashboard>,
    /// Results of the last key test, by key name.
    pub key_results: BTreeMap<String, (bool, String)>,
    /// Size of the last frame drawn, so key handling knows whether the
    /// "window too small" notice is what the user is looking at.
    last_size: Cell<(u16, u16)>,
}

impl App {
    pub fn new(start: Screen) -> Self {
        Self::with_snapshot(start, Snapshot::load())
    }

    pub fn with_snapshot(start: Screen, snap: Snapshot) -> Self {
        let mut app = App {
            screen: Screen::Home,
            overlay: None,
            should_quit: false,
            snap,
            tick: 0,
            home: HomeScreen::default(),
            keys: KeysScreen::default(),
            models: ModelsScreen::default(),
            config: ConfigScreen::default(),
            check: CheckScreen::default(),
            status: None,
            key_results: BTreeMap::new(),
            last_size: Cell::new((u16::MAX, u16::MAX)),
        };
        app.open(start);
        app
    }

    /// True while something runs in the background and the screen should keep
    /// redrawing.
    pub fn busy(&self) -> bool {
        self.models.is_loading()
            || self.check.is_running()
            || self.status.as_ref().is_some_and(|s| s.is_pending())
    }

    /// Collect results from background work.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        self.models.poll();
        self.check.poll();
        if let Some(status) = &mut self.status {
            status.poll();
        }
    }

    // ── Input ──

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl-C leaves from anywhere, the way it does everywhere else.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        if let Some(overlay) = self.overlay.take() {
            self.handle_overlay(overlay, key);
            return;
        }

        if let Some(status) = &self.status {
            if status.is_done() {
                self.key_results.extend(status.results());
                self.status = None;
            }
            return;
        }

        if self.too_small_for_screen() {
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                self.should_quit = true;
            }
            return;
        }

        // Keys that mean the same on every screen.
        match key.code {
            KeyCode::Char('?') => return self.apply(Action::Help),
            KeyCode::Char('q') => return self.apply(self.leave(Action::Quit)),
            _ => {}
        }

        let action = match self.screen {
            Screen::Home => self.home.handle(key.code, &self.snap),
            Screen::Keys => self.keys.handle(key.code, &self.snap),
            Screen::Models => self.models.handle(key, &self.snap),
            Screen::Config => self.config.handle(key.code),
            Screen::Check => self.check.handle(key.code),
        };
        let action = match action {
            Action::Back => self.leave(Action::Back),
            other => other,
        };
        self.apply(action);
    }

    /// Leaving the model picker with unsaved picks asks first.
    fn leave(&self, then: Action) -> Action {
        if self.screen == Screen::Models && self.models.is_dirty() {
            Action::Confirm(Ask::DiscardModels { then: Box::new(then) })
        } else {
            then
        }
    }

    fn handle_overlay(&mut self, overlay: Overlay, key: KeyEvent) {
        match overlay {
            Overlay::Toast(_) | Overlay::Help => {}
            Overlay::Input(mut modal, prompt) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    let value = modal.value();
                    self.submit(prompt, value);
                }
                code => {
                    modal.handle_key(code);
                    self.overlay = Some(Overlay::Input(modal, prompt));
                }
            },
            Overlay::Confirm(modal, ask) => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.confirmed(ask, true),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.confirmed(ask, false),
                _ => self.overlay = Some(Overlay::Confirm(modal, ask)),
            },
        }
    }

    fn submit(&mut self, prompt: Prompt, value: String) {
        match prompt {
            Prompt::KeyName => {
                let name = value.trim().to_string();
                if name.is_empty() {
                    self.notify("Name cannot be empty.", false);
                } else {
                    self.apply(Action::Prompt(Prompt::KeyValue { name }));
                }
            }
            Prompt::KeyValue { name } => self.apply(Action::Key(KeyOp::Add { name, value })),
            Prompt::Rename { old } => self.apply(Action::Key(KeyOp::Rename { old, new: value })),
            Prompt::CustomModel { .. } => self.models.set_custom(&value),
        }
    }

    fn confirmed(&mut self, ask: Ask, yes: bool) {
        match ask {
            Ask::RemoveKey(name) if yes => self.apply(Action::Key(KeyOp::Remove(name))),
            Ask::RemoveKey(_) => {}
            Ask::SaveModelsWithKey { .. } => {
                let action = self.models.save(yes);
                self.apply(action);
            }
            Ask::DiscardModels { then } if yes => {
                self.models.discard();
                self.apply(*then);
            }
            Ask::DiscardModels { .. } => {}
        }
    }

    // ── Actions ──

    fn apply(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Quit => self.should_quit = true,
            Action::Open(screen) => self.open(screen),
            Action::Back => self.screen = Screen::Home,
            Action::Help => self.overlay = Some(Overlay::Help),
            Action::Notify { text, success } => self.notify(&text, success),
            Action::Prompt(prompt) => {
                let modal = match &prompt {
                    Prompt::KeyName => InputModal::new(
                        "Add a key · step 1 of 2",
                        "A name you will recognise, e.g. work or personal",
                        "",
                        false,
                    ),
                    Prompt::KeyValue { name } => InputModal::new(
                        format!("Add '{name}' · step 2 of 2"),
                        "Paste the virtual API key you were given (hidden)",
                        "",
                        true,
                    ),
                    Prompt::Rename { old } => {
                        InputModal::new(format!("Rename '{old}'"), "New name", old, false)
                    }
                    Prompt::CustomModel { current } => InputModal::new(
                        "Custom model",
                        "Any model id, e.g. one the gateway hides from discovery",
                        current.as_deref().unwrap_or(""),
                        false,
                    ),
                };
                self.overlay = Some(Overlay::Input(modal, prompt));
            }
            Action::Confirm(ask) => {
                let modal = match &ask {
                    Ask::RemoveKey(name) => ConfirmModal::new(
                        "Remove key",
                        vec![
                            format!("Remove '{name}' from ccc?"),
                            "Folders already using it keep their copy.".to_string(),
                        ],
                        true,
                    ),
                    Ask::SaveModelsWithKey { key_name } => ConfirmModal::new(
                        "Save models",
                        vec![
                            format!("These models were checked against '{key_name}'."),
                            format!("Use '{key_name}' in this folder too?"),
                            "No saves the models and leaves the folder's key alone.".to_string(),
                        ],
                        false,
                    ),
                    Ask::DiscardModels { .. } => ConfirmModal::new(
                        "Unsaved models",
                        vec![
                            "You picked models but did not save them.".to_string(),
                            "Leave without saving?".to_string(),
                        ],
                        true,
                    ),
                };
                self.overlay = Some(Overlay::Confirm(modal, ask));
            }
            Action::Key(op) => self.key_op(op),
            Action::PickModelsWith(name) => {
                let value = self.snap.store.keys.get(&name).cloned();
                match value {
                    Some(value) => {
                        self.models.open(&self.snap, Some((name, value)));
                        self.screen = Screen::Models;
                    }
                    None => self.notify(&format!("Key '{name}' not found."), false),
                }
            }
            Action::TestAllKeys => match StatusDashboard::new(&self.snap.store) {
                Some(dashboard) => self.status = Some(dashboard),
                None => self.notify("No keys to test yet. Press a to add one.", false),
            },
        }
    }

    fn open(&mut self, screen: Screen) {
        match screen {
            Screen::Models => self.models.open(&self.snap, None),
            Screen::Check => self.check.start(),
            Screen::Config => self.config.reset(),
            Screen::Keys => self.keys.focus_current(&self.snap),
            Screen::Home => {}
        }
        self.screen = screen;
    }

    fn key_op(&mut self, op: KeyOp) {
        let result = match op {
            KeyOp::Add { name, value } => keys::add(&name, &value).map(|added| {
                self.keys.select(&name, &KeysStore::load());
                format!("{} Press u to use it in this folder.", added.message)
            }),
            KeyOp::Default(name) => keys::set_default(&name),
            KeyOp::Use(name) => keys::use_here(&name),
            KeyOp::Remove(name) => keys::remove(&name),
            KeyOp::Rename { old, new } => keys::rename(&old, &new).inspect(|_| {
                self.keys.select(new.trim(), &KeysStore::load());
            }),
        };
        self.reload();
        match result {
            Ok(message) => self.notify(&message, true),
            Err(e) => self.notify(&format!("{e:#}"), false),
        }
    }

    fn reload(&mut self) {
        self.snap = Snapshot::load();
        self.keys.clamp(&self.snap.store);
    }

    fn notify(&mut self, text: &str, success: bool) {
        // A save from the model picker changes the folder: reload first so the
        // screen behind the message is already up to date.
        if success {
            self.reload();
        }
        self.overlay = Some(Overlay::Toast(Toast::new(text, success)));
    }

    #[cfg(test)]
    pub fn models_mut(&mut self) -> &mut ModelsScreen {
        &mut self.models
    }

    // ── Rendering ──

    fn too_small_for_screen(&self) -> bool {
        let (w, h) = self.last_size.get();
        w < widgets::MIN_WIDTH || h < widgets::MIN_HEIGHT
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        self.last_size.set((area.width, area.height));
        frame.render_widget(Block::new().style(Style::default().bg(BG)), area);

        if area.width < widgets::MIN_WIDTH || area.height < widgets::MIN_HEIGHT {
            widgets::too_small(frame, area);
            return;
        }

        if let Some(status) = &self.status {
            status.render(frame, area, self.tick);
            return;
        }

        match self.screen {
            Screen::Home => self.home.render(frame, area, &self.snap, self.check.summary()),
            Screen::Keys => self.keys.render(frame, area, &self.snap, &self.key_results),
            Screen::Models => self.models.render(frame, area, self.tick),
            Screen::Config => self.config.render(frame, area, &self.snap),
            Screen::Check => self.check.render(frame, area, self.tick),
        }

        match &self.overlay {
            Some(Overlay::Input(modal, _)) => modal.render(frame, area),
            Some(Overlay::Confirm(modal, _)) => modal.render(frame, area),
            Some(Overlay::Toast(toast)) => toast.render(frame, area),
            Some(Overlay::Help) => help::render(frame, area, self.screen),
            None => {}
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::api::KeySource;
    use crossterm::event::KeyEventKind;
    use ratatui::backend::TestBackend;
    use serde_json::json;

    pub fn fake_snapshot(with_keys: bool) -> Snapshot {
        let mut store = KeysStore::default();
        if with_keys {
            store.keys.insert("work".into(), "sk-work-1234567890abcd".into());
            store.keys.insert("personal".into(), "sk-pers-0987654321wxyz".into());
            store.active = Some("work".into());
        }
        let json = json!({"env": {
            "ANTHROPIC_BASE_URL": "https://litellm-proxy-ep-cncyfugmcnadc6g4.a02.azurefd.net",
            "ANTHROPIC_AUTH_TOKEN": "sk-work-1234567890abcd",
            "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY": "1",
            "ANTHROPIC_MODEL": "claude-sonnet-5-vn[1m]",
            "ANTHROPIC_DEFAULT_SONNET_MODEL": "claude-sonnet-5-vn[1m]",
            "ANTHROPIC_DEFAULT_OPUS_MODEL": "claude-opus-5-vn[1m]",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL": "claude-haiku-4-5-vn",
        }});
        let project = ProjectState {
            folder: r"C:\Users\someone\Desktop\a-project-with-a-rather-long-folder-name".into(),
            local_path: ".claude/settings.local.json".into(),
            has_local: with_keys,
            slots: crate::slots::read_all(&json),
            json,
            key: with_keys.then(|| ("sk-work-1234567890abcd".to_string(), KeySource::Folder)),
            key_name: with_keys.then(|| "work".to_string()),
            base_url: crate::api::TEAM_BASE_URL.into(),
            filter: "-vn".into(),
            discovery: true,
        };
        Snapshot { store, project }
    }

    pub fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Press));
    }

    pub fn draw(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    /// Every row of a full-screen page must start and end on the border.
    fn assert_framed(screen: &str) {
        let lines: Vec<&str> = screen.lines().collect();
        assert!(lines[0].starts_with('╭') && lines[0].ends_with('╮'), "top:\n{screen}");
        let last = lines[lines.len() - 1];
        assert!(last.starts_with('╰') && last.ends_with('╯'), "bottom:\n{screen}");
        for line in &lines[1..lines.len() - 1] {
            assert!(line.starts_with('│') && line.ends_with('│'), "row {line:?}:\n{screen}");
        }
    }

    const SIZES: [(u16, u16); 4] = [(64, 20), (80, 24), (100, 30), (160, 48)];

    #[test]
    fn every_screen_fits_every_supported_size() {
        for with_keys in [true, false] {
            for screen in [Screen::Home, Screen::Keys, Screen::Config] {
                let app = App::with_snapshot(screen, fake_snapshot(with_keys));
                for (w, h) in SIZES {
                    let out = draw(&app, w, h);
                    assert_framed(&out);
                    assert!(out.contains("? help"), "{screen:?} {w}x{h} lost its help hint:\n{out}");
                }
            }
        }
    }

    #[test]
    fn home_lists_every_section_and_the_next_step() {
        let app = App::with_snapshot(Screen::Home, fake_snapshot(false));
        let out = draw(&app, 80, 24);
        for item in ["Keys", "Models", "Config", "Check", "Add a key"] {
            assert!(out.contains(item), "missing {item}:\n{out}");
        }
        assert!(out.contains("next"), "{out}");
    }

    #[test]
    fn keys_screen_explains_default_and_folder_key() {
        let app = App::with_snapshot(Screen::Keys, fake_snapshot(true));
        let out = draw(&app, 100, 30);
        assert!(out.contains("work"), "{out}");
        assert!(out.contains("Default"), "{out}");
        assert!(out.contains("This folder"), "{out}");
        assert!(!out.contains("sk-work-1234567890abcd"), "key shown in the clear:\n{out}");
    }

    #[test]
    fn keys_screen_guides_an_empty_store() {
        let app = App::with_snapshot(Screen::Keys, fake_snapshot(false));
        let out = draw(&app, 80, 24);
        assert!(out.contains("No keys saved yet"), "{out}");
    }

    #[test]
    fn home_navigation_opens_keys_and_back() {
        let mut app = App::with_snapshot(Screen::Home, fake_snapshot(true));
        press(&mut app, KeyCode::Char('1'));
        assert_eq!(app.screen, Screen::Keys);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.screen, Screen::Home);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.screen, Screen::Config);
    }

    #[test]
    fn help_opens_on_every_screen_and_any_key_closes_it() {
        for screen in [Screen::Home, Screen::Keys, Screen::Config] {
            let mut app = App::with_snapshot(screen, fake_snapshot(true));
            press(&mut app, KeyCode::Char('?'));
            let out = draw(&app, 80, 24);
            assert!(out.contains("Help"), "{screen:?}:\n{out}");
            press(&mut app, KeyCode::Char('x'));
            assert!(app.overlay.is_none());
            assert!(!app.should_quit);
        }
    }

    #[test]
    fn add_key_prompt_hides_the_value() {
        let mut app = App::with_snapshot(Screen::Keys, fake_snapshot(true));
        press(&mut app, KeyCode::Char('a'));
        for c in "cá nhân".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        let out = draw(&app, 80, 24);
        assert!(out.contains("cá nhân"), "name should be visible:\n{out}");
        // Step 2 hides what is typed.
        app.overlay = None;
        app.apply(Action::Prompt(Prompt::KeyValue { name: "x".into() }));
        for c in "sk-secret-value".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        let out = draw(&app, 80, 24);
        assert!(!out.contains("sk-secret-value"), "{out}");
        assert!(out.contains("•••"), "{out}");
    }

    #[test]
    fn remove_asks_first_and_no_cancels() {
        let mut app = App::with_snapshot(Screen::Keys, fake_snapshot(true));
        press(&mut app, KeyCode::Char('r'));
        assert!(matches!(app.overlay, Some(Overlay::Confirm(_, Ask::RemoveKey(_)))));
        press(&mut app, KeyCode::Char('n'));
        assert!(app.overlay.is_none());
    }

    #[test]
    fn too_small_window_says_so() {
        let app = App::with_snapshot(Screen::Home, fake_snapshot(true));
        let out = draw(&app, 50, 12);
        assert!(out.contains("Window too small"), "{out}");
    }

    #[test]
    fn ctrl_c_quits_even_inside_a_prompt() {
        let mut app = App::with_snapshot(Screen::Keys, fake_snapshot(true));
        press(&mut app, KeyCode::Char('a'));
        app.handle_key(KeyEvent::new_with_kind(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
        ));
        assert!(app.should_quit);
    }

    #[test]
    fn q_inside_a_prompt_is_text_not_quit() {
        let mut app = App::with_snapshot(Screen::Keys, fake_snapshot(true));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('q'));
        assert!(!app.should_quit);
        match &app.overlay {
            Some(Overlay::Input(modal, _)) => assert_eq!(modal.value(), "q"),
            _ => panic!("prompt closed"),
        }
    }
}
