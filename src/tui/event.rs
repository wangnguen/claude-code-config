use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::stdout;

use super::app::App;

pub fn run_key_tui() {
    if enable_raw_mode().is_err() { return; }
    let mut stdout = stdout();
    if execute!(stdout, EnterAlternateScreen).is_err() {
        disable_raw_mode().ok();
        return;
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(_) => { disable_raw_mode().ok(); return; }
    };

    let mut app = App::new();

    loop {
        app.poll_status();
        terminal.draw(|f| app.render(f, f.area())).ok();
        if app.should_quit { break; }

        let timeout = if app.has_pending_status() {
            std::time::Duration::from_millis(50)
        } else {
            std::time::Duration::from_secs(60)
        };

        if !event::poll(timeout).unwrap_or(false) { continue; }

        let event = match event::read() {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press { continue; }
            app.handle_event(key.code);
        }
    }

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();
}
