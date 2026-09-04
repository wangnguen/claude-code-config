use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SETTINGS_FILE: &str = "settings.local.json";
pub const REPO: &str = "wangnguen/claude-code-config";

/// LiteLLM virtual keys are bearer tokens and belong in ANTHROPIC_AUTH_TOKEN.
pub const ENV_AUTH_TOKEN: &str = "ANTHROPIC_AUTH_TOKEN";
/// Legacy variable ccc used to write. Kept only so old configs still resolve.
pub const ENV_API_KEY: &str = "ANTHROPIC_API_KEY";
/// Makes Claude Code ask the gateway which models the key may use.
pub const ENV_GATEWAY_DISCOVERY: &str = "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY";

/// Every variable that pins a model name. All of them must resolve to a model
/// the key is entitled to, including the small/fast one used for background work.
pub const MODEL_ENV_KEYS: &[&str] = &[
    "ANTHROPIC_MODEL",
    "ANTHROPIC_SMALL_FAST_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
];

pub fn ccc_home() -> Result<PathBuf> {
    let home = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
    } else {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
    }
    .context("Cannot determine home directory (HOME or USERPROFILE not set)")?;
    Ok(Path::new(&home).join(".ccc"))
}

pub fn default_claude_dir() -> Result<PathBuf> {
    Ok(ccc_home()?.join(".claude"))
}

pub fn default_settings_path() -> Result<PathBuf> {
    Ok(default_claude_dir()?.join(SETTINGS_FILE))
}

pub fn local_settings_path() -> PathBuf {
    Path::new(".claude").join(SETTINGS_FILE)
}

pub fn keys_path() -> Result<PathBuf> {
    Ok(ccc_home()?.join("keys.json"))
}

/// The settings Claude Code actually reads in this folder. Falls back to ccc's
/// own template, which Claude Code never reads — callers should say which one
/// they got so nobody edits the template expecting it to take effect.
pub fn effective_settings_path() -> Option<(PathBuf, bool)> {
    let local = local_settings_path();
    if local.exists() {
        return Some((local, true));
    }
    default_settings_path().ok().filter(|p| p.exists()).map(|p| (p, false))
}

pub fn read_json(path: &Path) -> Result<Value> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("Invalid JSON in {}", path.display()))
}

/// Read JSON, returning empty object on any error (for non-critical reads)
pub fn read_json_or_default(path: &Path) -> Value {
    read_json(path).unwrap_or_else(|_| Value::Object(serde_json::Map::new()))
}

/// Write the auth token into a settings JSON and drop the legacy
/// ANTHROPIC_API_KEY, so the two can never disagree in the same file.
pub fn set_auth_token(json: &mut Value, token: &str) {
    json["env"][ENV_AUTH_TOKEN] = Value::String(token.to_string());
    if let Some(env) = json["env"].as_object_mut() {
        env.remove(ENV_API_KEY);
    }
}

pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    let pretty = serde_json::to_string_pretty(value)
        .context("Failed to serialize JSON")?;
    fs::write(path, pretty)
        .with_context(|| format!("Failed to write {}", path.display()))
}

// Keys store: { active: "name", keys: { "name": "sk-..." } }
#[derive(Serialize, Deserialize, Default)]
pub struct KeysStore {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
}

impl KeysStore {
    pub fn load() -> Self {
        let path = match keys_path() {
            Ok(p) => p,
            Err(_) => return KeysStore::default(),
        };
        if !path.exists() {
            return KeysStore::default();
        }
        match read_json(&path) {
            Ok(json) => serde_json::from_value(json).unwrap_or_default(),
            Err(_) => KeysStore::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = keys_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let value = serde_json::to_value(self)
            .context("Failed to serialize keys")?;
        write_json(&path, &value)
    }

    pub fn get_active_key(&self) -> Option<&String> {
        self.active.as_ref().and_then(|a| self.keys.get(a))
    }
}
