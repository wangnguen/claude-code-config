use anyhow::Result;
use clap::Parser;
use console::{measure_text_width, style};
use crossterm::cursor::{MoveTo, MoveToColumn};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use std::io::{self, IsTerminal, Write};

use crate::commands::splash;
use crate::Cli;

/// Interactive prompt: type `check` instead of `ccc check`, and keep the
/// gateway answers in one screen. Lines go through the same clap parser as the
/// command line, so there is only ever one definition of what a command means.
pub fn run() -> Result<()> {
    splash::print();

    loop {
        let line = match read_line()? {
            Input::Line(line) => line,
            // Ctrl-C and Ctrl-D both mean "done here". The sign-off printed on
            // the way out is what separates either from a crash.
            Input::Interrupt | Input::Eof => break,
        };
        let line = line.trim();

        match line {
            "" => continue,
            "exit" | "quit" | "q" => break,
            "?" => {
                splash::print();
                continue;
            }
            // `/clear` spelled the way Claude Code spells it and `cls` the way
            // cmd does, because those are what fingers reach for.
            "clear" | "/clear" | "cls" => {
                clear_screen()?;
                splash::print();
                continue;
            }
            _ => {}
        }

        run_line(line);
        println!();
    }

    splash::print_farewell();
    Ok(())
}

/// Send one typed line through the command line's own parser.
fn run_line(line: &str) {
    // "ccc" in front because clap expects the program name first; typing it
    // anyway is a habit worth tolerating, so drop it if it is there.
    let mut args = vec!["ccc"];
    args.extend(line.split_whitespace().filter(|w| *w != "ccc"));

    match Cli::try_parse_from(args) {
        Ok(cli) => {
            if let Some(command) = cli.command {
                if let Err(err) = crate::dispatch(command) {
                    eprintln!("{}", style(format!("{err:#}")).red());
                }
            }
        }
        Err(err) => {
            let _ = err.print();
        }
    }
}

/// Wipe the screen and the scrollback with it: a "clear" that leaves the old
/// output one scroll away has not cleared much.
fn clear_screen() -> Result<()> {
    execute!(
        io::stdout(),
        Clear(ClearType::All),
        Clear(ClearType::Purge),
        MoveTo(0, 0)
    )?;
    Ok(())
}

// ── Reading a line ──

/// How the user left the prompt.
enum Input {
    /// A line to run.
    Line(String),
    /// Ctrl-C.
    Interrupt,
    /// Ctrl-D on an empty line, or a stdin that has run out.
    Eof,
}

fn prompt() -> String {
    format!("  {} ", style("›").cyan().bold())
}

fn read_line() -> Result<Input> {
    if io::stdin().is_terminal() {
        read_typed_line()
    } else {
        read_piped_line()
    }
}

/// Piped or redirected stdin: there is no terminal to put in raw mode, so take
/// the line as it arrives and let a closed pipe end the session.
fn read_piped_line() -> Result<Input> {
    print!("{}", prompt());
    io::stdout().flush()?;

    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        println!();
        return Ok(Input::Eof);
    }
    // Nothing echoes a piped line back, so echo it here and a transcript reads
    // the way the session would have looked at a terminal.
    println!("{}", line.trim_end());
    Ok(Input::Line(line))
}

/// A line typed at a real terminal. Reading goes through crossterm in raw mode
/// so Ctrl-C arrives as a keystroke instead of killing the process mid-word —
/// the shell owns the interrupt and gets to sign off. Raw mode covers only the
/// typing: every command underneath prints with a plain `\n`, which a raw
/// terminal would not turn back into a carriage return.
fn read_typed_line() -> Result<Input> {
    terminal::enable_raw_mode()?;
    let outcome = edit();
    terminal::disable_raw_mode()?;

    // The newline the terminal would have echoed for us outside raw mode.
    println!();
    outcome
}

fn edit() -> Result<Input> {
    let prompt = prompt();
    let mut chars: Vec<char> = Vec::new();
    let mut cursor = 0usize;

    redraw(&prompt, &chars, cursor)?;
    loop {
        let Event::Key(key) = event::read()? else {
            continue;
        };
        // Windows reports the release as well as the press; only one of the two
        // is a keystroke.
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        // Shift is how a terminal spells a capital letter, so it is not a
        // modifier that should disqualify a character.
        let typed = key.modifiers.difference(KeyModifiers::SHIFT).is_empty();

        match key.code {
            KeyCode::Char('c') if ctrl => return Ok(Input::Interrupt),
            // Only on an empty line, the way a shell does it: with text on the
            // line Ctrl-C is the way out, and this stays a no-op.
            KeyCode::Char('d') if ctrl && chars.is_empty() => return Ok(Input::Eof),
            KeyCode::Char('l') if ctrl => clear_screen()?,
            KeyCode::Char('u') if ctrl => {
                chars.clear();
                cursor = 0;
            }
            KeyCode::Char(c) if typed => {
                chars.insert(cursor, c);
                cursor += 1;
            }
            KeyCode::Enter => return Ok(Input::Line(chars.iter().collect())),
            KeyCode::Backspace if cursor > 0 => {
                cursor -= 1;
                chars.remove(cursor);
            }
            KeyCode::Delete if cursor < chars.len() => {
                chars.remove(cursor);
            }
            KeyCode::Left => cursor = cursor.saturating_sub(1),
            KeyCode::Right if cursor < chars.len() => cursor += 1,
            KeyCode::Home => cursor = 0,
            KeyCode::End => cursor = chars.len(),
            _ => {}
        }
        redraw(&prompt, &chars, cursor)?;
    }
}

/// Repaint the whole line. Tracking what changed would save a few bytes on a
/// line this short and cost far more than it saves in cases to get wrong.
fn redraw(prompt: &str, chars: &[char], cursor: usize) -> Result<()> {
    let columns = terminal::size().map(|(c, _)| c as usize).unwrap_or(80);
    let indent = measure_text_width(prompt);
    // One column stays free: a cursor sitting exactly on the wrap point puts
    // the line onto a second row that this single-row repaint would not clean.
    let room = columns.saturating_sub(indent + 1).max(1);

    // Scroll sideways rather than wrap once the line outgrows the row, keeping
    // the cursor inside the window.
    let start = cursor.saturating_sub(room);
    let visible: String = chars[start..].iter().take(room).collect();

    let mut out = io::stdout();
    queue!(out, MoveToColumn(0), Clear(ClearType::UntilNewLine))?;
    write!(out, "{prompt}{visible}")?;
    queue!(out, MoveToColumn((indent + cursor - start) as u16))?;
    out.flush()?;
    Ok(())
}
