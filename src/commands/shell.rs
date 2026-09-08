use anyhow::Result;
use clap::Parser;
use console::style;
use std::io::{self, Write};

use crate::commands::splash;
use crate::Cli;

/// Interactive prompt: type `check` instead of `ccc check`, and keep the
/// gateway answers in one screen. Lines go through the same clap parser as the
/// command line, so there is only ever one definition of what a command means.
pub fn run() -> Result<()> {
    splash::print();

    loop {
        let Some(line) = prompt()? else { break };
        let line = line.trim();

        match line {
            "" => continue,
            "exit" | "quit" | "q" => break,
            "?" => {
                splash::print();
                continue;
            }
            _ => {}
        }

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
        println!();
    }

    Ok(())
}

/// One line from the user, or None when they close the input with Ctrl-D.
fn prompt() -> Result<Option<String>> {
    print!("  {} ", style("›").cyan().bold());
    io::stdout().flush()?;

    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        println!();
        return Ok(None);
    }
    Ok(Some(line))
}
