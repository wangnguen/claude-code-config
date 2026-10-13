use std::io::{stdin, stdout, IsTerminal};
use std::time::Duration;

use anyhow::{bail, Result};
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use super::app::{App, Screen};

/// Open the full-screen manager on `screen` and run it until the user leaves.
pub fn run(screen: Screen) -> Result<()> {
    if !stdout().is_terminal() || !stdin().is_terminal() {
        bail!(
            "This screen needs an interactive terminal. Use the plain commands instead: \
             'ccc key list|add|use', 'ccc models' and 'ccc models set <slot> <model>'."
        );
    }

    // Look for a newer release before taking over the screen: the header
    // reads the answer on every frame and must never wait for it.
    crate::release::available_update();

    // try_init also installs a panic hook that restores the terminal, so a bug
    // can never leave the shell stuck in raw mode on the alternate screen.
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, screen);
    ratatui::try_restore()?;
    result
}

fn event_loop(terminal: &mut DefaultTerminal, screen: Screen) -> Result<()> {
    let mut app = App::new(screen);
    loop {
        app.tick();
        terminal.draw(|frame| app.render(frame))?;
        if app.should_quit {
            return Ok(());
        }

        // Redraw often while a spinner turns, otherwise only on input.
        let timeout = if app.busy() {
            Duration::from_millis(80)
        } else {
            Duration::from_secs(1)
        };
        if !event::poll(timeout)? {
            continue;
        }
        // Resize and other events need nothing but the redraw at the top.
        // Windows reports key releases too; only presses count.
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key);
        }
    }
}
