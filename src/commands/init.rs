use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::config::{
    default_claude_dir, read_json, set_auth_token, write_json, KeysStore, SETTINGS_FILE,
};
use crate::utils::{confirm, copy_dir_recursive, ignore_claude_dir};

pub fn run() -> Result<()> {
    let source = default_claude_dir()?;
    if !source.exists() {
        bail!("Default .claude folder not found at: {}. Please run the install script first.", source.display());
    }

    let target = Path::new(".claude");
    let target_settings = target.join(SETTINGS_FILE);

    if target_settings.exists()
        && !confirm("settings.local.json already exists. Overwrite? (y/N): ")
    {
        println!("Cancelled.");
        return Ok(());
    }

    copy_dir_recursive(&source, target).context("Failed to copy .claude folder")?;

    // Apply default key from keys.json
    let store = KeysStore::load();
    if let Some(key_value) = store.get_active_key() {
        if target_settings.exists() {
            let mut json = read_json(&target_settings)?;
            set_auth_token(&mut json, key_value);
            write_json(&target_settings, &json)?;
            println!("Applied default key '{}' to local config.", store.active.as_deref().unwrap_or(""));
            ignore_claude_dir();
        }
    } else {
        println!("No API key found. Run 'ccc key add' to add one, then 'ccc key use' to apply.");
    }

    println!("Copied default .claude config to current directory.");
    println!("Done!");
    Ok(())
}
