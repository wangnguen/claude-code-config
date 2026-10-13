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
/// Where Claude Code sends its requests.
pub const ENV_BASE_URL: &str = "ANTHROPIC_BASE_URL";

fn home_dir() -> Result<PathBuf> {
    let home = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
    } else {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
    }
    .context("Cannot determine home directory (HOME or USERPROFILE not set)")?;
    Ok(PathBuf::from(home))
}

pub fn ccc_home() -> Result<PathBuf> {
    Ok(home_dir()?.join(".ccc"))
}

/// Claude Code's own user settings, ~/.claude/settings.json, or the same file
/// under CLAUDE_CONFIG_DIR when that moves Claude Code's home. `/model` saves
/// its choice here.
pub fn user_claude_settings_path() -> Option<PathBuf> {
    match std::env::var("CLAUDE_CONFIG_DIR") {
        Ok(dir) if !dir.trim().is_empty() => Some(PathBuf::from(dir.trim()).join("settings.json")),
        _ => home_dir().ok().map(|home| home.join(".claude").join("settings.json")),
    }
}

/// System-wide policy file, which outranks every other settings source.
pub fn managed_settings_path() -> PathBuf {
    PathBuf::from(if cfg!(target_os = "windows") {
        r"C:\ProgramData\ClaudeCode\managed-settings.json"
    } else if cfg!(target_os = "macos") {
        "/Library/Application Support/ClaudeCode/managed-settings.json"
    } else {
        "/etc/claude-code/managed-settings.json"
    })
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

/// ccc's own preferences, as opposed to the Claude Code settings it writes.
pub fn ccc_settings_path() -> Result<PathBuf> {
    Ok(ccc_home()?.join("ccc.json"))
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

/// An env value from the settings Claude Code reads in this folder, falling
/// back to ccc's template for folders that only carry part of the config
/// (`ccc key use` in a fresh folder writes the token and nothing else).
pub fn setting_env(name: &str) -> Option<String> {
    let mut candidates = vec![local_settings_path()];
    if let Ok(template) = default_settings_path() {
        candidates.push(template);
    }
    candidates
        .iter()
        .filter(|path| path.exists())
        .find_map(|path| env_value(&read_json_or_default(path), name))
}

/// A non-empty string under `env` in a settings JSON.
pub fn env_value(json: &Value, name: &str) -> Option<String> {
    json["env"][name]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
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

/// Read this folder's `.claude/settings.local.json` for a change, creating the
/// folder when needed. Missing or unreadable files start from `{}` so a broken
/// file never blocks a fix, but the caller writes the whole object back.
pub fn load_local_for_edit() -> Result<(PathBuf, Value)> {
    let path = local_settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let json = if path.exists() {
        read_json_or_default(&path)
    } else {
        Value::Object(serde_json::Map::new())
    };
    Ok((path, json))
}

// ccc preferences: { "model_filter": "-vn" }
#[derive(Serialize, Deserialize, Default)]
pub struct CccSettings {
    /// Suffix a model id must end with to count as usable. `None` means "pick
    /// from the gateway": `-vn` for the team gateway, nothing for any other.
    /// An empty string switches filtering off everywhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_filter: Option<String>,
}

impl CccSettings {
    pub fn load() -> Self {
        ccc_settings_path()
            .ok()
            .filter(|p| p.exists())
            .and_then(|p| read_json(&p).ok())
            .and_then(|json| serde_json::from_value(json).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = ccc_settings_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let value = serde_json::to_value(self).context("Failed to serialize ccc settings")?;
        write_json(&path, &value)
    }
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

    /// The saved name for a key value, so screens can say "work" rather than
    /// a masked token nobody recognises.
    pub fn name_of(&self, value: &str) -> Option<&String> {
        self.keys.iter().find(|(_, v)| v.as_str() == value).map(|(name, _)| name)
    }
}
