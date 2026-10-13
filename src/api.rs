use std::fmt;
use std::path::Path;
use std::time::Duration;

use crate::config::{
    default_settings_path, local_settings_path, setting_env, CccSettings, KeysStore, ENV_API_KEY,
    ENV_AUTH_TOKEN, ENV_BASE_URL,
};

/// LiteLLM proxy the team's virtual keys are issued against.
pub const TEAM_BASE_URL: &str = "https://litellm-proxy-ep-cncyfugmcnadc6g4.a02.azurefd.net";
/// Fallback main model. Must be a gateway model name, not a built-in one.
pub const DEFAULT_MODEL: &str = "claude-sonnet-5-vn[1m]";
/// Suffix marking the models the team gateway's virtual keys may use.
pub const TEAM_MODEL_FILTER: &str = "-vn";
/// How long one /v1/models call may take before the gateway counts as down.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Base URL and main model for this folder: its own settings first, then
/// ccc's template, then the team gateway.
pub fn get_api_config() -> (String, String) {
    let base_url = setting_env(ENV_BASE_URL).unwrap_or_else(|| TEAM_BASE_URL.to_string());
    let model = setting_env("ANTHROPIC_MODEL")
        .or_else(|| setting_env("ANTHROPIC_DEFAULT_SONNET_MODEL"))
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    (base_url, model)
}

pub fn is_team_gateway(base_url: &str) -> bool {
    base_url
        .trim()
        .trim_end_matches('/')
        .eq_ignore_ascii_case(TEAM_BASE_URL)
}

/// The suffix a model id must carry to be usable through `base_url`.
///
/// An explicit `model_filter` in ~/.ccc/ccc.json wins. Otherwise the team
/// gateway means `-vn` — its virtual keys are scoped to that group — and any
/// other gateway means no filter: whatever it lists for the key is usable.
pub fn model_filter_for(base_url: &str) -> String {
    match CccSettings::load().model_filter {
        Some(filter) => filter.trim().to_string(),
        None if is_team_gateway(base_url) => TEAM_MODEL_FILTER.to_string(),
        None => String::new(),
    }
}

/// The filter for the gateway this folder talks to.
pub fn model_filter() -> String {
    model_filter_for(&get_api_config().0)
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

/// True when a model id passes the filter. An empty filter passes everything.
pub fn matches_filter(id: &str, filter: &str) -> bool {
    filter.is_empty()
        || base_model_id(id)
            .to_lowercase()
            .ends_with(&filter.to_lowercase())
}

/// The models from a gateway list that pass the filter.
pub fn usable_models(models: &[String], filter: &str) -> Vec<String> {
    models
        .iter()
        .filter(|m| matches_filter(m, filter))
        .cloned()
        .collect()
}

/// True when `id` (with or without a `[1m]` marker) is in a gateway list.
pub fn is_listed(id: &str, models: &[String]) -> bool {
    let bare = base_model_id(id);
    models.iter().any(|m| m == bare)
}

/// Why a model list could not be fetched. The split matters to callers: a
/// rejected key should stop a setup, an unreachable gateway should not.
#[derive(Debug, Clone)]
pub enum ApiError {
    /// The gateway answered and refused the key.
    Rejected(String),
    /// Nothing came back: DNS, TLS, timeout, connection refused.
    Unreachable(String),
    /// The gateway answered with something other than a model list.
    Unexpected(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Rejected(msg) => write!(f, "key rejected: {msg}"),
            ApiError::Unreachable(msg) => write!(f, "gateway unreachable: {msg}"),
            ApiError::Unexpected(msg) => write!(f, "{msg}"),
        }
    }
}

/// Ask the gateway this folder uses which models the key may use.
pub fn list_models(api_key: &str) -> Result<Vec<String>, ApiError> {
    list_models_at(&get_api_config().0, api_key)
}

/// Ask a specific gateway which models the key may use.
///
/// This is the same endpoint Claude Code hits when
/// CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1, so it verifies the key and
/// the model entitlement in one call — and unlike a /v1/messages probe it
/// cannot fail just because some unrelated model name is out of scope.
pub fn list_models_at(base_url: &str, api_key: &str) -> Result<Vec<String>, ApiError> {
    let url = format!("{}/v1/models", base_url.trim().trim_end_matches('/'));
    // Status codes come back as responses rather than errors so the gateway's
    // own explanation ("key expired", "budget exceeded") can be shown.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();

    // LiteLLM virtual keys authenticate as bearer tokens; upstream Anthropic
    // uses x-api-key. Send both so the probe works against either endpoint.
    let mut resp = agent
        .get(&url)
        .header("Authorization", &format!("Bearer {api_key}"))
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .call()
        .map_err(|e| ApiError::Unreachable(e.to_string()))?;

    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    parse_models_response(status, &text)
}

/// Turn a /v1/models response into a sorted model list or a typed error.
fn parse_models_response(status: u16, body: &str) -> Result<Vec<String>, ApiError> {
    let json: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let gateway_message = json["error"]["message"]
        .as_str()
        .or_else(|| json["error"].as_str())
        .or_else(|| json["detail"].as_str())
        .map(str::to_string);

    match status {
        200..=299 => match json["data"].as_array() {
            Some(items) => {
                let mut models: Vec<String> = items
                    .iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_string))
                    .collect();
                models.sort();
                models.dedup();
                Ok(models)
            }
            None => Err(ApiError::Unexpected(
                "the gateway answered, but not with a model list — is the base URL right?"
                    .to_string(),
            )),
        },
        401 | 403 => Err(ApiError::Rejected(match gateway_message {
            Some(msg) => format!("{} (HTTP {status})", first_sentence(&msg)),
            None => format!("HTTP {status} — wrong or expired key"),
        })),
        404 => Err(ApiError::Unexpected(
            "HTTP 404 — no /v1/models here; is the base URL right?".to_string(),
        )),
        _ => Err(ApiError::Unexpected(match gateway_message {
            Some(msg) => format!("HTTP {status}: {msg}"),
            None => format!("HTTP {status}"),
        })),
    }
}

/// The first sentence of a gateway message. LiteLLM follows the reason with
/// the key's hash and table names, which help nobody reading a terminal.
fn first_sentence(message: &str) -> &str {
    let message = message.trim();
    match message.find(". ") {
        Some(end) => &message[..end],
        None => message.trim_end_matches('.'),
    }
}

/// Check if an API key is valid by asking the gateway for its model list.
/// On success the message summarises how many models the key can reach.
pub fn check_api_key(api_key: &str) -> (bool, String) {
    match list_models(api_key) {
        Ok(models) => {
            let filter = model_filter();
            if filter.is_empty() {
                (true, format!("{} models", models.len()))
            } else {
                let usable = usable_models(&models, &filter).len();
                (true, format!("{} models, {usable} with {filter}", models.len()))
            }
        }
        Err(e) => (false, e.to_string()),
    }
}

/// Validate API key format
pub fn validate_key_format(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("Key cannot be empty.".to_string());
    }
    if key.chars().count() < 10 {
        return Err("Key is too short.".to_string());
    }
    if key.chars().any(char::is_whitespace) {
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
        if let Some(value) = json["env"][name].as_str()
            && !value.is_empty()
        {
            return Some(value.to_string());
        }
    }
    None
}

/// Where the key in use came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    /// This folder's .claude/settings.local.json — what Claude Code uses.
    Folder,
    /// The default key in ~/.ccc/keys.json.
    Default,
    /// ccc's template settings, which Claude Code never reads.
    Template,
}

impl KeySource {
    pub fn describe(self) -> &'static str {
        match self {
            KeySource::Folder => "this folder",
            KeySource::Default => "ccc default, not applied here",
            KeySource::Template => "ccc template, not applied here",
        }
    }
}

/// Resolve current API key: local config → keys.json default → global config
pub fn get_current_key() -> Option<String> {
    current_key_with_source().map(|(key, _)| key)
}

/// The current key and where it was found.
pub fn current_key_with_source() -> Option<(String, KeySource)> {
    if let Some(key) = key_from_settings(&local_settings_path()) {
        return Some((key, KeySource::Folder));
    }

    let store = KeysStore::load();
    if let Some(key) = store.get_active_key() {
        return Some((key.clone(), KeySource::Default));
    }

    if let Ok(global) = default_settings_path()
        && let Some(key) = key_from_settings(&global)
    {
        return Some((key, KeySource::Template));
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
    fn filter_accepts_context_marker() {
        assert!(matches_filter("claude-sonnet-5-vn[1m]", "-vn"));
        assert!(matches_filter("claude-opus-5-vn[1m]", "-vn"));
    }

    #[test]
    fn filter_is_case_insensitive() {
        assert!(matches_filter("claude-haiku-4-5-vn", "-vn"));
        assert!(matches_filter("CLAUDE-SONNET-5-VN", "-vn"));
        assert!(matches_filter("CLAUDE-SONNET-5-VN[1M]", "-VN"));
    }

    #[test]
    fn filter_rejects_builtin_names() {
        assert!(!matches_filter("claude-sonnet-5", "-vn"));
        assert!(!matches_filter("claude-haiku-4-5-20251001", "-vn"));
        // A marker does not make a built-in reachable: the 403 comes from the
        // id, not from the context window.
        assert!(!matches_filter("claude-sonnet-5[1m]", "-vn"));
    }

    #[test]
    fn empty_filter_accepts_everything() {
        assert!(matches_filter("claude-sonnet-5", ""));
        assert!(matches_filter("gpt-4o", ""));
    }

    #[test]
    fn defaults_are_reachable_models() {
        assert!(matches_filter(DEFAULT_MODEL, TEAM_MODEL_FILTER));
    }

    #[test]
    fn team_gateway_is_recognised_with_trailing_slash() {
        assert!(is_team_gateway(TEAM_BASE_URL));
        assert!(is_team_gateway(&format!("{TEAM_BASE_URL}/")));
        assert!(!is_team_gateway("https://example.com"));
    }

    #[test]
    fn listed_compares_bare_ids() {
        let models = vec!["claude-sonnet-5-vn".to_string()];
        assert!(is_listed("claude-sonnet-5-vn[1m]", &models));
        assert!(!is_listed("claude-sonnet-5", &models));
    }

    #[test]
    fn parses_model_list_sorted_and_deduped() {
        let body = r#"{"data":[{"id":"b"},{"id":"a"},{"id":"b"}]}"#;
        assert_eq!(parse_models_response(200, body).unwrap(), vec!["a", "b"]);
    }

    #[test]
    fn rejected_key_keeps_gateway_message() {
        let body = r#"{"error":{"message":"Authentication Error, Invalid proxy server token"}}"#;
        match parse_models_response(401, body) {
            Err(ApiError::Rejected(msg)) => assert!(msg.contains("Invalid proxy server token")),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn rejection_is_cut_to_its_first_sentence() {
        let body = r#"{"error":{"message":"Authentication Error, Invalid proxy server token passed. Received API Key = sk-...cdef, Key Hash (Token) =47f5. Unable to find token"}}"#;
        match parse_models_response(401, body) {
            Err(ApiError::Rejected(msg)) => {
                assert_eq!(msg, "Authentication Error, Invalid proxy server token passed (HTTP 401)")
            }
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn non_list_success_is_unexpected() {
        assert!(matches!(
            parse_models_response(200, "<html></html>"),
            Err(ApiError::Unexpected(_))
        ));
    }

    #[test]
    fn key_format_counts_characters_not_bytes() {
        assert!(validate_key_format("sk-ệệệệệệệ").is_ok());
        assert!(validate_key_format("sk-ab cdefghij").is_err());
        assert!(validate_key_format("short").is_err());
    }
}
