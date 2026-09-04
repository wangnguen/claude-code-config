use anyhow::{Context, Result};
use dialoguer::{Select, Input, Confirm};
use dialoguer::theme::ColorfulTheme;

use crate::api::{check_api_key, get_api_config, validate_key_format};
use crate::config::{local_settings_path, read_json_or_default, set_auth_token, write_json, KeysStore};
use crate::utils::{ignore_claude_dir, mask_key};

pub fn run(subcmd: Option<KeyCmd>) -> Result<()> {
    match subcmd {
        Some(KeyCmd::Add { name, value }) => cmd_add(Some(name), Some(value)),
        Some(KeyCmd::List) => { cmd_list(); Ok(()) }
        Some(KeyCmd::Default { name }) => cmd_default(name),
        Some(KeyCmd::Use { name }) => cmd_use(name),
        Some(KeyCmd::Remove { name }) => cmd_remove(name),
        Some(KeyCmd::Rename) => cmd_rename(),
        Some(KeyCmd::Status) => { cmd_status(); Ok(()) }
        Some(KeyCmd::Test { value }) => { cmd_test(&value); Ok(()) }
        None => { cmd_menu(); Ok(()) }
    }
}

#[derive(clap::Subcommand)]
pub enum KeyCmd {
    /// Add or update a named key
    Add {
        /// Key name
        name: String,
        /// API key value
        value: String,
    },
    /// List all saved keys
    List,
    /// Set default key (saved in keys.json)
    Default {
        /// Key name (optional, shows list if omitted)
        name: Option<String>,
    },
    /// Use a key for current folder (.claude/settings.local.json)
    Use {
        /// Key name (optional, shows list if omitted)
        name: Option<String>,
    },
    /// Remove a saved key
    Remove {
        /// Key name (optional, shows list if omitted)
        name: Option<String>,
    },
    /// Rename a saved key
    Rename,
    /// Check all keys status (test API connection)
    Status,
    /// Test an API key without saving it
    Test {
        /// API key value to test
        value: String,
    },
}


fn cmd_menu() {
    crate::tui::run_key_tui();
}

fn select_key(store: &KeysStore, prompt: &str) -> Result<String> {
    let theme = ColorfulTheme::default();
    let names: Vec<&str> = store.keys.keys().map(|s| s.as_str()).collect();
    let default_idx = store.active.as_ref()
        .and_then(|a| names.iter().position(|n| n == a))
        .unwrap_or(0);
    let selection = Select::with_theme(&theme)
        .with_prompt(prompt)
        .items(&names)
        .default(default_idx)
        .interact()
        .context("Selection cancelled")?;
    Ok(names[selection].to_string())
}

fn cmd_add(name: Option<String>, value: Option<String>) -> Result<()> {
    let theme = ColorfulTheme::default();

    let name = match name {
        Some(n) => n,
        None => Input::with_theme(&theme)
            .with_prompt("Key name (e.g. work, personal)")
            .interact_text()
            .context("Input cancelled")?,
    };

    let value = match value {
        Some(v) => v,
        None => Input::with_theme(&theme)
            .with_prompt("API key value")
            .interact_text()
            .context("Input cancelled")?,
    };

    if name.is_empty() {
        eprintln!("Name cannot be empty.");
        return Ok(());
    }

    if let Err(e) = validate_key_format(&value) {
        eprintln!("{e}");
        return Ok(());
    }

    let mut store = KeysStore::load();
    store.keys.insert(name.clone(), value.clone());

    // Set as default if first key
    if store.active.is_none() {
        store.active = Some(name.clone());
        store.save()?;
        println!("Added '{name}' and set as default.");
    } else if store.active.as_deref() != Some(&name) {
        let set_default = Confirm::with_theme(&theme)
            .with_prompt(format!("Set '{name}' as default key?"))
            .default(false)
            .interact()
            .unwrap_or(false);
        if set_default {
            store.active = Some(name.clone());
        }
        store.save()?;
        if store.active.as_deref() == Some(&name) {
            println!("Added '{name}' and set as default.");
        } else {
            println!("Added '{name}'.");
        }
    } else {
        store.save()?;
        println!("Updated '{name}'.");
    }
    Ok(())
}

fn cmd_list() {
    let store = KeysStore::load();

    if store.keys.is_empty() {
        println!("No keys saved. Run 'ccc key add' to add one.");
        return;
    }

    println!("Saved keys:");
    for (name, value) in &store.keys {
        let is_default = store.active.as_deref() == Some(name);
        let marker = if is_default { "*" } else { " " };
        let masked = mask_key(value);
        let tag = if is_default { " (default)" } else { "" };
        println!("  {marker} {name:<20} {masked}{tag}");
    }
}

/// Set default key in keys.json only
fn cmd_default(name: Option<String>) -> Result<()> {
    let mut store = KeysStore::load();

    if store.keys.is_empty() {
        eprintln!("No keys saved. Run 'ccc key add' first.");
        return Ok(());
    }

    let name = match name {
        Some(n) => n,
        None => select_key(&store, "Select default key")?,
    };

    if !store.keys.contains_key(&name) {
        eprintln!("Key '{name}' not found.");
        return Ok(());
    }

    store.active = Some(name.clone());
    store.save()?;
    println!("Set '{name}' as default.");
    Ok(())
}

/// Use key for current folder -> .claude/settings.local.json
fn cmd_use(name: Option<String>) -> Result<()> {
    let store = KeysStore::load();

    if store.keys.is_empty() {
        eprintln!("No keys saved. Run 'ccc key add' first.");
        return Ok(());
    }

    let name = match name {
        Some(n) => n,
        None => select_key(&store, "Select key for current folder")?,
    };

    if !store.keys.contains_key(&name) {
        eprintln!("Key '{name}' not found.");
        return Ok(());
    }

    let key_value = store.keys.get(&name).unwrap();
    let local_path = local_settings_path();

    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let mut json = if local_path.exists() {
        read_json_or_default(&local_path)
    } else {
        serde_json::json!({})
    };

    set_auth_token(&mut json, key_value);
    write_json(&local_path, &json)?;
    ignore_claude_dir();

    let folder = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_string());
    println!("Using '{name}' for folder: {folder}");
    Ok(())
}

/// Rename a key
fn cmd_rename() -> Result<()> {
    let mut store = KeysStore::load();

    if store.keys.is_empty() {
        eprintln!("No keys saved.");
        return Ok(());
    }

    let old_name = select_key(&store, "Select key to rename")?;

    if !store.keys.contains_key(&old_name) {
        eprintln!("Key '{old_name}' not found.");
        return Ok(());
    }

    println!("Renaming key: '{old_name}'");
    let theme = ColorfulTheme::default();
    let new_name: String = Input::with_theme(&theme)
        .with_prompt("New name")
        .interact_text()
        .context("Input cancelled")?;

    if new_name.is_empty() {
        eprintln!("Name cannot be empty.");
        return Ok(());
    }

    if new_name == old_name {
        println!("Name unchanged.");
        return Ok(());
    }

    if store.keys.contains_key(&new_name) {
        eprintln!("Key '{new_name}' already exists.");
        return Ok(());
    }

    if let Some(value) = store.keys.remove(&old_name) {
        store.keys.insert(new_name.clone(), value);
        if store.active.as_deref() == Some(&old_name) {
            store.active = Some(new_name.clone());
        }
        store.save()?;
        println!("Renamed '{old_name}' -> '{new_name}'.");
    }
    Ok(())
}

/// Check all keys by calling API
fn cmd_status() {
    let store = KeysStore::load();

    if store.keys.is_empty() {
        println!("No keys saved. Run 'ccc key add' to add one.");
        return;
    }

    let (base_url, _) = get_api_config();

    println!();
    crate::ui::print_header(&crate::ui::ICON_SEARCH, "Key Status Check");
    crate::ui::print_row("API", &format!("{base_url}/v1/models"));
    crate::ui::print_separator();

    let mut pass = 0;
    let mut fail = 0;

    for (name, value) in &store.keys {
        let is_default = store.active.as_deref() == Some(name);
        let label = if is_default { format!("{name} *") } else { name.clone() };

        let sp = crate::ui::spinner(&format!("Checking {name}..."));
        let (ok, msg) = check_api_key(value);
        sp.finish_and_clear();

        let detail = format!("{} {msg}", mask_key(value));
        crate::ui::print_check(ok, &label, &detail);
        if ok { pass += 1; } else { fail += 1; }
    }

    crate::ui::print_separator();
    crate::ui::print_result_line(pass, fail);
    crate::ui::print_footer();
    println!();
}

fn cmd_remove(name: Option<String>) -> Result<()> {
    let mut store = KeysStore::load();

    if store.keys.is_empty() {
        eprintln!("No keys saved.");
        return Ok(());
    }

    let name = match name {
        Some(n) => n,
        None => select_key(&store, "Select key to remove")?,
    };

    if !store.keys.contains_key(&name) {
        eprintln!("Key '{name}' not found.");
        return Ok(());
    }

    store.keys.remove(&name);
    if store.active.as_deref() == Some(&name) {
        store.active = store.keys.keys().next().cloned();
        if let Some(new_default) = &store.active {
            println!("Removed '{name}', default switched to '{new_default}'.");
        } else {
            println!("Removed '{name}'. No default key.");
        }
    } else {
        println!("Removed '{name}'.");
    }

    store.save()?;
    Ok(())
}

fn cmd_test(value: &str) {
    if let Err(e) = validate_key_format(value) {
        eprintln!("Invalid key format: {e}");
        return;
    }

    let (base_url, _) = get_api_config();

    println!();
    crate::ui::print_header(&crate::ui::ICON_SEARCH, "Key Test");
    crate::ui::print_row("API", &format!("{base_url}/v1/models"));
    crate::ui::print_row("Key", &mask_key(value));
    crate::ui::print_separator();

    let sp = crate::ui::spinner("Testing API key...");
    let (ok, msg) = check_api_key(value);
    sp.finish_and_clear();

    crate::ui::print_check(ok, "Result", &msg);
    crate::ui::print_footer();
    println!();
}
