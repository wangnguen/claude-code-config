use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::api::{list_models_at, model_filter_for, usable_models, ApiError, TEAM_BASE_URL};
use crate::config::{
    default_claude_dir, read_json, set_auth_token, write_json, ENV_BASE_URL, ENV_GATEWAY_DISCOVERY,
    SETTINGS_FILE,
};
use crate::slots::{self, HAIKU, MAIN, OPUS, SLOTS, SONNET};
use crate::ui;
use crate::utils::{confirm, copy_dir_recursive, prompt_secret};

// The team gateway's defaults. Every one carries the `-vn` suffix: the names
// without it are Claude Code's built-in list, which the team's virtual keys
// cannot reach — selecting one returns "403 key not allowed to access model".
// No slot may stay unset either: Claude Code then falls back to a built-in id
// and fails the same way.
//
// `[1m]` picks the 1M-context variant. Only Sonnet and Opus have one; Haiku
// 4.5 does not, so the Haiku slot stays bare. Keep these in sync with
// default/.claude/settings.local.json — `ccc lite` overwrites the copied file.
// Before writing, `ccc lite` checks them against the key's model list and
// swaps any the key cannot reach for the closest model it can.
const LITE_MODELS: [(usize, &str); 4] = [
    (MAIN, "claude-sonnet-5-vn[1m]"),
    (SONNET, "claude-sonnet-5-vn[1m]"),
    (OPUS, "claude-opus-5-vn[1m]"),
    (HAIKU, "claude-haiku-4-5-vn"),
];

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

    // Ask for the virtual key before writing anything. Input is hidden so the
    // key never lands in the terminal scrollback or a screen share.
    let auth_token = prompt_secret("Enter your virtual API key (input hidden)")?;
    if auth_token.is_empty() {
        bail!("A virtual API key is required.");
    }

    // Ask the gateway first: a key it rejects should not end up in the folder,
    // and the answer says which of the defaults this key can actually use.
    let filter = model_filter_for(TEAM_BASE_URL);
    let sp = ui::plain_spinner("Checking your key with the gateway...");
    let listed = list_models_at(TEAM_BASE_URL, &auth_token);
    sp.finish_and_clear();
    let usable = match listed {
        Ok(models) => Some(usable_models(&models, &filter)),
        Err(ApiError::Rejected(msg)) => bail!(
            "The gateway rejected this key: {msg}\nNothing was written. Check the key and run 'ccc lite' again."
        ),
        Err(e) => {
            println!("Couldn't verify the key ({e}).");
            println!("Writing the default models; run 'ccc check' once the gateway is reachable.");
            None
        }
    };

    copy_dir_recursive(&source, target).context("Failed to copy .claude folder")?;

    // Build the lite env config, preserving any existing settings.
    let mut json = read_json(&target_settings).unwrap_or_else(|_| serde_json::json!({}));
    set_auth_token(&mut json, &auth_token);
    json["env"][ENV_BASE_URL] = serde_json::Value::String(TEAM_BASE_URL.to_string());
    json["env"][ENV_GATEWAY_DISCOVERY] = serde_json::Value::String("1".to_string());
    for (slot, model) in LITE_MODELS {
        slots::write(&mut json, slot, Some(model));
    }

    let mut swapped = Vec::new();
    if let Some(usable) = &usable {
        if usable.is_empty() {
            println!("Warning: this key has no {filter} model. Ask for access, then run 'ccc check'.");
        } else {
            swapped = slots::fit(&mut json, usable);
        }
    }
    write_json(&target_settings, &json)?;

    crate::utils::ignore_local_settings();

    println!("Copied default .claude config to current directory.");
    println!("Applied lite config (base_url: {TEAM_BASE_URL}).");
    if usable.as_ref().is_some_and(|u| !u.is_empty()) && swapped.is_empty() {
        println!("Models: the defaults are all available to your key.");
    }
    for change in &swapped {
        println!(
            "Models: {} → {} (the default is not available to your key).",
            SLOTS[change.slot].label,
            change.to
        );
    }
    println!("Change them any time with 'ccc models pick'.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{matches_filter, TEAM_MODEL_FILTER};

    #[test]
    fn lite_defaults_belong_to_the_team_group() {
        for (_, model) in LITE_MODELS {
            assert!(matches_filter(model, TEAM_MODEL_FILTER), "{model}");
        }
    }

    #[test]
    fn lite_defaults_match_the_shipped_template() {
        let template: serde_json::Value =
            serde_json::from_str(include_str!("../../default/.claude/settings.local.json")).unwrap();
        for (slot, model) in LITE_MODELS {
            assert_eq!(template["env"][SLOTS[slot].env], model, "{}", SLOTS[slot].env);
        }
        assert_eq!(template["env"][slots::LEGACY_FAST_ENV], LITE_MODELS[3].1);
        assert_eq!(template["env"][ENV_BASE_URL], TEAM_BASE_URL);
    }
}
