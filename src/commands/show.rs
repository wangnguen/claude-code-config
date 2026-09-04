use anyhow::{bail, Result};

use crate::config::{default_settings_path, local_settings_path, read_json};
use crate::utils::{print_json_pretty, redact_secrets};
use crate::ShowTarget;

pub fn run(target: ShowTarget) -> Result<()> {
    match target {
        ShowTarget::Config => show_config(),
        ShowTarget::Global => show_global(),
    }
}

fn show_config() -> Result<()> {
    let local = local_settings_path();
    let path = if local.exists() {
        local
    } else {
        default_settings_path()?
    };

    if !path.exists() {
        bail!("No settings found. Run 'ccc init' or 'ccc key' first.");
    }

    let json = read_json(&path)?;
    println!("Config: {}", path.display());
    println!("--- (secrets masked)");
    print_json_pretty(&redact_secrets(&json));
    Ok(())
}

fn show_global() -> Result<()> {
    let path = default_settings_path()?;

    if !path.exists() {
        bail!("Global settings not found at: {}. Please run the install script first.", path.display());
    }

    let json = read_json(&path)?;
    println!("Global config: {}", path.display());
    println!("--- (secrets masked)");
    print_json_pretty(&redact_secrets(&json));
    Ok(())
}
