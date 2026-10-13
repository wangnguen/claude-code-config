use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::api::{list_models_at, model_filter_for, usable_models};
use crate::config::{
    default_claude_dir, read_json, set_auth_token, write_json, KeysStore, ENV_BASE_URL,
    SETTINGS_FILE,
};
use crate::slots::{self, SLOTS};
use crate::ui;
use crate::utils::{confirm, copy_dir_recursive, ignore_local_settings};

pub fn run(base_url: Option<String>) -> Result<()> {
    // Check the URL before anything is copied, so a typo leaves no half-done folder.
    let base_url = match base_url {
        Some(url) => {
            let url = url.trim().trim_end_matches('/').to_string();
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                bail!("--base-url must start with https:// or http:// (got '{url}').");
            }
            Some(url)
        }
        None => None,
    };

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
    let key = store.get_active_key().cloned();
    if let Some(key_value) = &key {
        if target_settings.exists() {
            let mut json = read_json(&target_settings)?;
            set_auth_token(&mut json, key_value);
            write_json(&target_settings, &json)?;
            println!("Applied default key '{}' to local config.", store.active.as_deref().unwrap_or(""));
            ignore_local_settings();
        }
    } else {
        println!("No API key found. Run 'ccc key add' to add one, then 'ccc key use' to apply.");
    }

    println!("Copied default .claude config to current directory.");

    if let Some(url) = base_url {
        point_at_gateway(&target_settings, &url, key.as_deref())?;
    }

    println!("Done!");
    Ok(())
}

/// Switch the copied config to another gateway. The template's models belong
/// to the team gateway, so they are refitted to what this one lists.
fn point_at_gateway(settings: &Path, url: &str, key: Option<&str>) -> Result<()> {
    let mut json = read_json(settings).unwrap_or_else(|_| serde_json::json!({}));
    json["env"][ENV_BASE_URL] = serde_json::Value::String(url.to_string());
    println!("Base URL set to {url}.");

    match key {
        Some(key) => {
            let sp = ui::plain_spinner("Asking the gateway for its models...");
            let listed = list_models_at(url, key);
            sp.finish_and_clear();
            match listed {
                Ok(models) => {
                    let usable = usable_models(&models, &model_filter_for(url));
                    if usable.is_empty() {
                        println!("The gateway lists no usable model for this key; run 'ccc models' to see why.");
                    }
                    for change in slots::fit(&mut json, &usable) {
                        println!("Models: {} → {}", SLOTS[change.slot].label, change.to);
                    }
                }
                Err(e) => {
                    println!("Couldn't list the gateway's models ({e}).");
                    println!("Run 'ccc models pick' once it is reachable.");
                }
            }
        }
        None => println!("Add a key, then run 'ccc models pick' to choose models for this gateway."),
    }

    write_json(settings, &json)
}
