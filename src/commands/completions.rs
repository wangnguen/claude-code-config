use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io::stdout;

pub fn run(shell: Shell) {
    let mut cmd = crate::Cli::command();
    generate(shell, &mut cmd, "ccc", &mut stdout());
}
