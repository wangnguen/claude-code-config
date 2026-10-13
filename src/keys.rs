//! Key operations shared by `ccc key …` and the TUI, so both write the same
//! files the same way. Each returns a sentence fit to show the user.

use anyhow::{bail, Result};

use crate::api::validate_key_format;
use crate::config::{load_local_for_edit, set_auth_token, write_json, KeysStore};
use crate::utils::ensure_local_settings_ignored;

pub struct Added {
    pub message: String,
    pub became_default: bool,
}

/// Save a key. The first key saved becomes the default.
pub fn add(name: &str, value: &str) -> Result<Added> {
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() {
        bail!("Name cannot be empty.");
    }
    if let Err(e) = validate_key_format(value) {
        bail!(e);
    }
    let mut store = KeysStore::load();
    let existed = store.keys.contains_key(name);
    store.keys.insert(name.to_string(), value.to_string());
    let became_default = store.active.is_none();
    if became_default {
        store.active = Some(name.to_string());
    }
    store.save()?;
    let message = match (existed, became_default) {
        (true, _) => format!("Updated '{name}'."),
        (false, true) => format!("Added '{name}' and made it the default."),
        (false, false) => format!("Added '{name}'."),
    };
    Ok(Added { message, became_default })
}

pub fn set_default(name: &str) -> Result<String> {
    let mut store = KeysStore::load();
    if !store.keys.contains_key(name) {
        bail!("Key '{name}' not found.");
    }
    store.active = Some(name.to_string());
    store.save()?;
    Ok(format!("'{name}' is now the default for new folders."))
}

/// Write a saved key into this folder's settings.
pub fn use_here(name: &str) -> Result<String> {
    let store = KeysStore::load();
    let Some(value) = store.keys.get(name) else {
        bail!("Key '{name}' not found.");
    };
    let note = apply_token_here(value)?;
    let mut message = format!("This folder now uses '{name}'.");
    if let Some(note) = note {
        message.push(' ');
        message.push_str(&note);
    }
    Ok(message)
}

/// Put a key value into `.claude/settings.local.json` and make sure git
/// ignores that file. Returns the .gitignore note when one was written.
pub fn apply_token_here(value: &str) -> Result<Option<String>> {
    let (path, mut json) = load_local_for_edit()?;
    set_auth_token(&mut json, value);
    write_json(&path, &json)?;
    Ok(ensure_local_settings_ignored())
}

/// Remove a key. When it was the default, the first remaining key takes over.
pub fn remove(name: &str) -> Result<String> {
    let mut store = KeysStore::load();
    if store.keys.remove(name).is_none() {
        bail!("Key '{name}' not found.");
    }
    let mut message = format!("Removed '{name}'.");
    if store.active.as_deref() == Some(name) {
        store.active = store.keys.keys().next().cloned();
        match &store.active {
            Some(next) => message = format!("Removed '{name}'; '{next}' is the default now."),
            None => message = format!("Removed '{name}'. No default key left."),
        }
    }
    store.save()?;
    Ok(message)
}

pub fn rename(old: &str, new: &str) -> Result<String> {
    let new = new.trim();
    if new.is_empty() {
        bail!("Name cannot be empty.");
    }
    if new == old {
        return Ok("Name unchanged.".to_string());
    }
    let mut store = KeysStore::load();
    if store.keys.contains_key(new) {
        bail!("Key '{new}' already exists.");
    }
    let Some(value) = store.keys.remove(old) else {
        bail!("Key '{old}' not found.");
    };
    store.keys.insert(new.to_string(), value);
    if store.active.as_deref() == Some(old) {
        store.active = Some(new.to_string());
    }
    store.save()?;
    Ok(format!("Renamed '{old}' → '{new}'."))
}
