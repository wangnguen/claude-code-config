use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::config::{
    default_claude_dir, read_json, set_auth_token, write_json, ENV_GATEWAY_DISCOVERY, SETTINGS_FILE,
};
use crate::utils::{confirm, copy_dir_recursive, prompt_secret};

const LITE_BASE_URL: &str = "https://litellm-proxy-ep-cncyfugmcnadc6g4.a02.azurefd.net";

// Every model below must carry the `-vn` suffix. The names without it are
// Claude Code's built-in list, which the team's virtual keys cannot reach —
// selecting one returns "403 key not allowed to access model". Leaving any of
// these unset is not an option either: Claude Code then falls back to a
// built-in id (e.g. claude-haiku-4-5-20251001) and fails the same way.
//
// `[1m]` picks the 1M-context variant. Only Sonnet and Opus have one; Haiku
// 4.5 does not, so the two Haiku slots stay bare. Keep these in sync with
// default/.claude/settings.local.json — `ccc lite` overwrites the copied file.
const LITE_MODEL: &str = "claude-sonnet-5-vn[1m]";
const LITE_SMALL_FAST_MODEL: &str = "claude-haiku-4-5-vn";
const LITE_DEFAULT_SONNET_MODEL: &str = "claude-sonnet-5-vn[1m]";
const LITE_DEFAULT_OPUS_MODEL: &str = "claude-opus-5-vn[1m]";
const LITE_DEFAULT_HAIKU_MODEL: &str = "claude-haiku-4-5-vn";

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

    copy_dir_recursive(&source, target).context("Failed to copy .claude folder")?;

    // Build the lite env config, preserving any existing settings.
    let mut json = read_json(&target_settings).unwrap_or_else(|_| serde_json::json!({}));
    set_auth_token(&mut json, &auth_token);
    let env = &mut json["env"];
    env["ANTHROPIC_BASE_URL"] = serde_json::Value::String(LITE_BASE_URL.to_string());
    env[ENV_GATEWAY_DISCOVERY] = serde_json::Value::String("1".to_string());
    env["ANTHROPIC_MODEL"] = serde_json::Value::String(LITE_MODEL.to_string());
    env["ANTHROPIC_SMALL_FAST_MODEL"] = serde_json::Value::String(LITE_SMALL_FAST_MODEL.to_string());
    env["ANTHROPIC_DEFAULT_SONNET_MODEL"] = serde_json::Value::String(LITE_DEFAULT_SONNET_MODEL.to_string());
    env["ANTHROPIC_DEFAULT_OPUS_MODEL"] = serde_json::Value::String(LITE_DEFAULT_OPUS_MODEL.to_string());
    env["ANTHROPIC_DEFAULT_HAIKU_MODEL"] = serde_json::Value::String(LITE_DEFAULT_HAIKU_MODEL.to_string());
    write_json(&target_settings, &json)?;

    crate::utils::ignore_claude_dir();

    println!("Copied default .claude config to current directory.");
    println!("Applied lite config (base_url: {LITE_BASE_URL}).");
    println!("Models pinned to the {} group.", crate::api::VN_SUFFIX);
    println!("Run 'ccc models' to confirm your key can reach them.");
    Ok(())
}
