use anyhow::{Context, Result};
use dialoguer::{Select, Input, Confirm};
use dialoguer::theme::ColorfulTheme;

use crate::api::{check_api_key, get_api_config, validate_key_format};
use crate::config::KeysStore;
use crate::keys;
use crate::utils::mask_key;

pub fn run(subcmd: Option<KeyCmd>) -> Result<()> {
    match subcmd {
        Some(KeyCmd::Add { name, value }) => cmd_add(Some(name), value),
        Some(KeyCmd::List) => { cmd_list(); Ok(()) }
        Some(KeyCmd::Default { name }) => cmd_default(name),
        Some(KeyCmd::Use { name }) => cmd_use(name),
        Some(KeyCmd::Remove { name }) => cmd_remove(name),
        Some(KeyCmd::Rename) => cmd_rename(),
        Some(KeyCmd::Status) => { cmd_status(); Ok(()) }
        Some(KeyCmd::Test { value }) => { cmd_test(&value); Ok(()) }
        None => cmd_menu(),
    }
}

#[derive(clap::Subcommand)]
pub enum KeyCmd {
    /// Add or update a named key
    Add {
        /// Key name
        name: String,
        /// API key value; left out, it is asked for with the input hidden
        value: Option<String>,
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


fn cmd_menu() -> Result<()> {
    crate::tui::run(crate::tui::Screen::Keys)
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

    // Hidden, like `ccc lite`: a key typed in the clear ends up in screen
    // shares and scrollback.
    let value = match value {
        Some(v) => v,
        None => crate::utils::prompt_secret("API key value (input hidden)")?,
    };

    if let Err(e) = validate_key_format(value.trim()) {
        anyhow::bail!(e);
    }

    let added = keys::add(&name, &value)?;
    println!("{}", added.message);

    // Offer the default only when there was one already; the first key takes
    // it without asking.
    let store = KeysStore::load();
    let name = name.trim();
    if !added.became_default && store.active.as_deref() != Some(name) {
        let set_default = Confirm::with_theme(&theme)
            .with_prompt(format!("Make '{name}' the default for new folders?"))
            .default(false)
            .interact()
            .unwrap_or(false);
        if set_default {
            println!("{}", keys::set_default(name)?);
        }
    }
    println!("Next: 'ccc key use {name}' to use it in this folder.");
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
    let store = KeysStore::load();

    if store.keys.is_empty() {
        eprintln!("No keys saved. Run 'ccc key add' first.");
        return Ok(());
    }

    let name = match name {
        Some(n) => n,
        None => select_key(&store, "Select default key")?,
    };

    println!("{}", keys::set_default(&name)?);
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

    println!("{}", keys::use_here(&name)?);
    Ok(())
}

/// Rename a key
fn cmd_rename() -> Result<()> {
    let store = KeysStore::load();

    if store.keys.is_empty() {
        eprintln!("No keys saved.");
        return Ok(());
    }

    let old_name = select_key(&store, "Select key to rename")?;

    println!("Renaming key: '{old_name}'");
    let theme = ColorfulTheme::default();
    let new_name: String = Input::with_theme(&theme)
        .with_prompt("New name")
        .interact_text()
        .context("Input cancelled")?;

    println!("{}", keys::rename(&old_name, &new_name)?);
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
    crate::ui::print_result_line(pass, 0, fail);
    crate::ui::print_footer();
    println!();
}

fn cmd_remove(name: Option<String>) -> Result<()> {
    let store = KeysStore::load();

    if store.keys.is_empty() {
        eprintln!("No keys saved.");
        return Ok(());
    }

    let name = match name {
        Some(n) => n,
        None => select_key(&store, "Select key to remove")?,
    };

    println!("{}", keys::remove(&name)?);
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
