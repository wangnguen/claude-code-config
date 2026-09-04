use std::path::Path;

use crate::config::{
    default_settings_path, local_settings_path, KeysStore, ENV_API_KEY, ENV_AUTH_TOKEN,
};

/// LiteLLM proxy the team's virtual keys are issued against.
pub const DEFAULT_BASE_URL: &str = "https://litellm-proxy-ep-cncyfugmcnadc6g4.a02.azurefd.net";
/// Fallback main model. Must be a gateway model name, not a built-in one.
pub const DEFAULT_MODEL: &str = "claude-sonnet-5-vn[1m]";
/// Suffix marking the models this team's virtual keys are allowed to use.
pub const VN_SUFFIX: &str = "-vn";

/// Get API config from global settings
pub fn get_api_config() -> (String, String) {
    let path = match default_settings_path() {
        Ok(p) => p,
        Err(_) => return default_api_config(),
    };
    if path.exists() {
        if let Ok(json) = crate::config::read_json(&path) {
            let base_url = json["env"]["ANTHROPIC_BASE_URL"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_BASE_URL)
                .to_string();
            let model = json["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_MODEL)
                .to_string();
            return (base_url, model);
        }
    }
    default_api_config()
}

fn default_api_config() -> (String, String) {
    (DEFAULT_BASE_URL.to_string(), DEFAULT_MODEL.to_string())
}

/// Strip the context-window marker Claude Code allows after a model name.
///
/// `claude-sonnet-5-vn[1m]` selects the 1M-context variant: Claude Code parses
/// the `[1m]` client-side, sends the bare id to the gateway and turns the
/// marker into a beta header. The gateway's own model list therefore never
/// carries it, so every comparison against that list must strip it first.
pub fn base_model_id(id: &str) -> &str {
    match id.find('[') {
        Some(i) => id[..i].trim_end(),
        None => id,
    }
}

/// True for models this team's virtual keys are entitled to (the `-vn` group).
pub fn is_vn_model(id: &str) -> bool {
    base_model_id(id).to_lowercase().ends_with(VN_SUFFIX)
}

/// Ask the gateway which models the key may use.
///
/// This is the same endpoint Claude Code hits when
/// CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1, so it verifies the key and
/// the model entitlement in one call — and unlike a /v1/messages probe it
/// cannot fail just because some unrelated model name is out of scope.
pub fn list_models(api_key: &str) -> Result<Vec<String>, String> {
    let (base_url, _) = get_api_config();
    let url = format!("{base_url}/v1/models");

    // LiteLLM virtual keys authenticate as bearer tokens; upstream Anthropic
    // uses x-api-key. Send both so the probe works against either endpoint.
    let result = ureq::get(&url)
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .call();

    match result {
        Ok(mut resp) => {
            let text = resp.body_mut().read_to_string().unwrap_or_default();
            let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            match json["data"].as_array() {
                Some(items) => {
                    let mut models: Vec<String> = items
                        .iter()
                        .filter_map(|m| m["id"].as_str().map(str::to_string))
                        .collect();
                    models.sort();
                    Ok(models)
                }
                None => Err(json["error"]["message"]
                    .as_str()
                    .unwrap_or("unexpected response from /v1/models")
                    .to_string()),
            }
        }
        Err(e) => Err(format!("{e}")),
    }
}

/// Check if an API key is valid by asking the gateway for its model list.
/// On success the message summarises how many models the key can reach.
pub fn check_api_key(api_key: &str) -> (bool, String) {
    match list_models(api_key) {
        Ok(models) => {
            let vn = models.iter().filter(|m| is_vn_model(m)).count();
            (true, format!("{} models, {vn} with {VN_SUFFIX}", models.len()))
        }
        Err(e) => (false, e),
    }
}

/// Validate API key format
pub fn validate_key_format(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("Key cannot be empty.".to_string());
    }
    if key.len() < 10 {
        return Err("Key is too short.".to_string());
    }
    if key.contains(' ') {
        return Err("Key cannot contain spaces.".to_string());
    }
    Ok(())
}

/// Read the auth token out of a settings file, preferring the current
/// variable over the legacy one.
fn key_from_settings(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let json = crate::config::read_json(path).ok()?;
    for name in [ENV_AUTH_TOKEN, ENV_API_KEY] {
        if let Some(value) = json["env"][name].as_str() {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Resolve current API key: local config → keys.json default → global config
pub fn get_current_key() -> Option<String> {
    // 1. Try local .claude/settings.local.json
    if let Some(key) = key_from_settings(&local_settings_path()) {
        return Some(key);
    }

    // 2. Try default key from keys.json
    let store = KeysStore::load();
    if let Some(key) = store.get_active_key() {
        return Some(key.clone());
    }

    // 3. Try global settings
    if let Ok(global) = default_settings_path() {
        if let Some(key) = key_from_settings(&global) {
            return Some(key);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_model_id_strips_context_marker() {
        assert_eq!(base_model_id("claude-sonnet-5-vn[1m]"), "claude-sonnet-5-vn");
        assert_eq!(base_model_id("claude-opus-5-vn[1m]"), "claude-opus-5-vn");
    }

    #[test]
    fn base_model_id_passes_bare_names_through() {
        assert_eq!(base_model_id("claude-haiku-4-5-vn"), "claude-haiku-4-5-vn");
        assert_eq!(base_model_id(""), "");
    }

    #[test]
    fn base_model_id_handles_marker_edges() {
        // Stray space before the marker, and a marker with nothing in front.
        assert_eq!(base_model_id("claude-sonnet-5-vn [1m]"), "claude-sonnet-5-vn");
        assert_eq!(base_model_id("[1m]"), "");
    }

    #[test]
    fn is_vn_model_accepts_context_marker() {
        assert!(is_vn_model("claude-sonnet-5-vn[1m]"));
        assert!(is_vn_model("claude-opus-5-vn[1m]"));
    }

    #[test]
    fn is_vn_model_is_case_insensitive() {
        assert!(is_vn_model("claude-haiku-4-5-vn"));
        assert!(is_vn_model("CLAUDE-SONNET-5-VN"));
        assert!(is_vn_model("CLAUDE-SONNET-5-VN[1M]"));
    }

    #[test]
    fn is_vn_model_rejects_builtin_names() {
        assert!(!is_vn_model("claude-sonnet-5"));
        assert!(!is_vn_model("claude-haiku-4-5-20251001"));
        // A marker does not make a built-in reachable: the 403 comes from the
        // id, not from the context window.
        assert!(!is_vn_model("claude-sonnet-5[1m]"));
    }

    #[test]
    fn defaults_are_reachable_models() {
        assert!(is_vn_model(DEFAULT_MODEL));
    }
}
