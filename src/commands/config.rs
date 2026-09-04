use anyhow::{bail, Result};

use crate::config::{default_settings_path, read_json, write_json, ENV_GATEWAY_DISCOVERY};
use crate::ui;
use crate::utils::{print_json_pretty, redact_secrets};

#[derive(clap::Subcommand)]
pub enum ConfigCmd {
    /// Set a config value (e.g. base_url, model)
    Set {
        /// Config key (base_url, model, sonnet_model, opus_model, haiku_model)
        key: String,
        /// New value
        value: String,
    },
    /// Get a config value
    Get {
        /// Config key (base_url, model, sonnet_model, opus_model, haiku_model)
        key: String,
    },
    /// List all available config keys and their values
    List,
}

const KNOWN_KEYS: &[(&str, &str, &str)] = &[
    ("base_url", "ANTHROPIC_BASE_URL", "API base URL"),
    ("main_model", "ANTHROPIC_MODEL", "Main model"),
    ("fast_model", "ANTHROPIC_SMALL_FAST_MODEL", "Small/fast model used for background work"),
    ("model", "ANTHROPIC_DEFAULT_SONNET_MODEL", "Default Sonnet model (alias for sonnet_model)"),
    ("sonnet_model", "ANTHROPIC_DEFAULT_SONNET_MODEL", "Default Sonnet model"),
    ("opus_model", "ANTHROPIC_DEFAULT_OPUS_MODEL", "Default Opus model"),
    ("haiku_model", "ANTHROPIC_DEFAULT_HAIKU_MODEL", "Default Haiku model"),
    ("discovery", ENV_GATEWAY_DISCOVERY, "Fetch the model list from the gateway (0 or 1)"),
    ("disable_traffic", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "Disable non-essential traffic (0 or 1)"),
];

fn resolve_env_key(key: &str) -> Option<&'static str> {
    KNOWN_KEYS.iter()
        .find(|(alias, _, _)| *alias == key)
        .map(|(_, env_key, _)| *env_key)
}

pub fn run(subcmd: ConfigCmd) -> Result<()> {
    match subcmd {
        ConfigCmd::Set { key, value } => cmd_set(&key, &value),
        ConfigCmd::Get { key } => cmd_get(&key),
        ConfigCmd::List => cmd_list(),
    }
}

fn cmd_set(key: &str, value: &str) -> Result<()> {
    let env_key = match resolve_env_key(key) {
        Some(k) => k,
        None => {
            eprintln!("Unknown config key: '{key}'");
            eprintln!();
            print_available_keys();
            bail!("Unknown config key: '{key}'");
        }
    };

    let path = default_settings_path()?;
    if !path.exists() {
        bail!("Global settings not found at: {}. Run the install script first.", path.display());
    }

    let mut json = read_json(&path)?;
    json["env"][env_key] = serde_json::Value::String(value.to_string());
    write_json(&path, &json)?;

    println!();
    ui::print_header(&ui::ICON_OK, "Config Updated");
    ui::print_row(key, value);
    ui::print_row("env key", env_key);
    ui::print_footer();
    println!();
    Ok(())
}

fn cmd_get(key: &str) -> Result<()> {
    let env_key = match resolve_env_key(key) {
        Some(k) => k,
        None => {
            eprintln!("Unknown config key: '{key}'");
            eprintln!();
            print_available_keys();
            bail!("Unknown config key: '{key}'");
        }
    };

    let path = default_settings_path()?;
    if !path.exists() {
        bail!("Global settings not found at: {}", path.display());
    }

    let json = read_json(&path)?;
    match json["env"][env_key].as_str() {
        Some(val) => println!("{val}"),
        None => println!("(not set)"),
    }
    Ok(())
}

fn cmd_list() -> Result<()> {
    let path = default_settings_path()?;
    if !path.exists() {
        bail!("Global settings not found at: {}. Run the install script first.", path.display());
    }

    let json = read_json(&path)?;

    println!();
    ui::print_header(&ui::ICON_DOC, "Config Values");

    for (alias, env_key, _desc) in KNOWN_KEYS {
        let val = json["env"][*env_key]
            .as_str()
            .unwrap_or("(not set)");
        ui::print_row(alias, val);
    }

    ui::print_separator();
    println!("  Full config ({}, secrets masked):", path.display());
    ui::print_separator();
    print_json_pretty(&redact_secrets(&json));
    ui::print_footer();
    println!();
    Ok(())
}

fn print_available_keys() {
    eprintln!("Available keys:");
    for (alias, _, desc) in KNOWN_KEYS {
        eprintln!("  {alias:<18} — {desc}");
    }
}
