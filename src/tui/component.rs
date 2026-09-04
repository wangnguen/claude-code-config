use crossterm::event::KeyCode;
use ratatui::prelude::*;

/// Action returned by a component after handling an event
pub enum Action {
    /// No action needed
    None,
    /// Quit the application
    Quit,
    /// Show a toast notification
    Notify { text: String, success: bool },
    /// Execute a key operation (add, default, use, remove, rename, status)
    Op(KeyOp),
    /// Switch to a different mode
    Switch(ModeSwitch),
}

/// Key operations that modify state
pub enum KeyOp {
    Add { name: String, value: String },
    Default(String),
    Use(String),
    Remove(String),
    Rename { old_name: String, new_name: String },
    Status,
}

/// Mode transitions
pub enum ModeSwitch {
    Normal,
    ToNormal,
    ToHome,
    ToKeyDashboard,
    ToShow,
    AddName,
    AddValue { name: String },
    Rename { old_name: String },
    ConfirmRemove(String),
}

/// Trait for self-contained UI components (available for future use)
#[allow(dead_code)]
pub trait Component {
    fn handle_event(&mut self, key: KeyCode) -> Action;
    fn render(&self, frame: &mut Frame, area: Rect);
}
