use anyhow::{bail, Result};

use crate::api::{get_api_config, model_filter_for};
use crate::config::{default_settings_path, read_json, write_json, CccSettings, ENV_GATEWAY_DISCOVERY};
use crate::ui;
use crate::utils::{print_json_pretty, redact_secrets};

#[derive(clap::Subcommand)]
pub enum ConfigCmd {
    /// Set a config value (e.g. base_url, model, model_filter)
    Set {
        /// Config key (base_url, model, sonnet_model, opus_model, haiku_model, custom_model, model_filter, …)
        key: String,
        /// New value
        value: String,
    },
    /// Get a config value
    Get {
        /// Config key (base_url, model, sonnet_model, opus_model, haiku_model, custom_model, model_filter, …)
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
    ("custom_model", "ANTHROPIC_CUSTOM_MODEL_OPTION", "One extra /model entry with any id"),
    ("discovery", ENV_GATEWAY_DISCOVERY, "Fetch the model list from the gateway (0 or 1)"),
    ("disable_traffic", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "Disable non-essential traffic (0 or 1)"),
];

/// Stored in ~/.ccc/ccc.json rather than in the template: it is a rule ccc
/// applies, not something Claude Code reads.
const MODEL_FILTER: &str = "model_filter";
const MODEL_FILTER_HELP: &str =
    "Suffix a usable model must end in: a suffix, `none` for no filter, or `auto` (-vn on the team gateway only)";

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
    if key == MODEL_FILTER {
        return set_model_filter(value);
    }
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
    if key == MODEL_FILTER {
        println!("{}", describe_model_filter());
        return Ok(());
    }
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
    ui::print_row(MODEL_FILTER, &describe_model_filter());

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
    eprintln!("  {MODEL_FILTER:<18} — {MODEL_FILTER_HELP}");
}

fn set_model_filter(value: &str) -> Result<()> {
    let mut settings = CccSettings::load();
    settings.model_filter = match value.trim().to_lowercase().as_str() {
        "auto" => None,
        // PowerShell 5 drops an empty "" argument, so "none" has to exist.
        "" | "none" | "off" => Some(String::new()),
        _ => Some(value.trim().to_string()),
    };
    settings.save()?;
    println!("model_filter: {}", describe_model_filter());
    Ok(())
}

/// The filter in effect, and why: "-vn (auto: team gateway)".
fn describe_model_filter() -> String {
    let (base_url, _) = get_api_config();
    let filter = model_filter_for(&base_url);
    let shown = if filter.is_empty() { "none".to_string() } else { filter };
    match CccSettings::load().model_filter {
        Some(_) => format!("{shown} (set in ccc.json)"),
        None if crate::api::is_team_gateway(&base_url) => format!("{shown} (auto: team gateway)"),
        None => format!("{shown} (auto: other gateway)"),
    }
}
