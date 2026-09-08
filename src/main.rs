mod api;
mod commands;
mod config;
mod tui;
mod ui;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(name = "ccc", about = "Claude Code Config CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum ShowTarget {
    /// Show local config (fallback to global)
    Config,
    /// Show global default config
    Global,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show the current version
    Version,
    /// Copy default .claude config to current directory
    Init,
    /// Copy .claude config with lite proxy env (prompts for auth token)
    Lite,
    /// Show config (default: global)
    Show {
        #[command(subcommand)]
        target: Option<ShowTarget>,
    },
    /// Manage API keys
    Key {
        #[command(subcommand)]
        subcmd: Option<commands::key::KeyCmd>,
    },
    /// Manage config values (base_url, model, etc.)
    Config {
        #[command(subcommand)]
        subcmd: commands::config::ConfigCmd,
    },
    /// Install or update Claude Code permissions
    #[command(visible_alias = "p")]
    Permission,
    /// Check for updates and install latest version
    Update,
    /// Remove ~/.ccc, the saved keys, and the installer's PATH entry
    Uninstall,
    /// Check environment and config status
    Doctor,
    /// Test API connection with current key
    Check,
    /// List the models the current key can reach on the gateway
    Models,
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

/// Rust starts with SIGPIPE ignored, so `ccc models | head` turns a closed pipe
/// into "failed printing to stdout" and a panic trace. Handing the signal back
/// to the OS makes ccc exit quietly like every other command in a pipeline.
#[cfg(unix)]
fn restore_sigpipe() {
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
}

#[cfg(not(unix))]
fn restore_sigpipe() {}

/// Run one parsed command. Separate from main so the interactive shell can send
/// the lines people type through exactly the same path as the command line.
pub fn dispatch(command: Commands) -> Result<()> {
    match command {
        Commands::Version => commands::version::run(),
        Commands::Init => commands::init::run()?,
        Commands::Lite => commands::lite::run()?,
        Commands::Show { target } => commands::show::run(target.unwrap_or(ShowTarget::Global))?,
        Commands::Key { subcmd } => commands::key::run(subcmd)?,
        Commands::Config { subcmd } => commands::config::run(subcmd)?,
        Commands::Permission => commands::permission::run()?,
        Commands::Update => commands::update::run()?,
        Commands::Uninstall => commands::uninstall::run()?,
        Commands::Doctor => commands::doctor::run()?,
        Commands::Check => commands::check::run()?,
        Commands::Models => commands::models::run()?,
        Commands::Completions { shell } => commands::completions::run(shell),
    }
    Ok(())
}

fn main() -> Result<()> {
    restore_sigpipe();
    let cli = Cli::parse();

    match cli.command {
        Some(command) => dispatch(command)?,
        // The key manager still lives at `ccc key`; a bare `ccc` opens the shell.
        None => commands::shell::run()?,
    }

    Ok(())
}

