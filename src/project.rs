//! What the current folder looks like to Claude Code, read from disk once so
//! the shell banner, the TUI and the commands describe it the same way.

use std::path::PathBuf;

use serde_json::Value;

use crate::api::{current_key_with_source, get_api_config, matches_filter, model_filter_for, KeySource};
use crate::config::{local_settings_path, read_json_or_default, KeysStore, ENV_GATEWAY_DISCOVERY};
use crate::slots::{self, SLOTS};

pub struct ProjectState {
    pub folder: String,
    pub local_path: PathBuf,
    /// Whether `.claude/settings.local.json` exists here.
    pub has_local: bool,
    /// That file's contents, or `{}`.
    pub json: Value,
    pub key: Option<(String, KeySource)>,
    /// The saved name of the key in use, when it is one of the saved keys.
    pub key_name: Option<String>,
    pub base_url: String,
    pub filter: String,
    pub discovery: bool,
    pub slots: [Option<String>; 5],
}

impl ProjectState {
    pub fn load(store: &KeysStore) -> Self {
        let local_path = local_settings_path();
        let has_local = local_path.exists();
        let json = if has_local {
            read_json_or_default(&local_path)
        } else {
            Value::Object(serde_json::Map::new())
        };
        let key = current_key_with_source();
        let key_name = key.as_ref().and_then(|(value, _)| store.name_of(value).cloned());
        let (base_url, _) = get_api_config();
        let filter = model_filter_for(&base_url);
        let discovery = json["env"][ENV_GATEWAY_DISCOVERY].as_str() == Some("1");
        let slots = slots::read_all(&json);
        let folder = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        ProjectState {
            folder,
            local_path,
            has_local,
            json,
            key,
            key_name,
            base_url,
            filter,
            discovery,
            slots,
        }
    }

    /// True when this folder's own settings carry a key.
    pub fn key_applied(&self) -> bool {
        matches!(self.key, Some((_, KeySource::Folder)))
    }

    /// "work · this folder", "a1b2...z9 · ccc default, not applied here", "none".
    pub fn key_label(&self) -> String {
        match &self.key {
            Some((value, source)) => {
                let name = self
                    .key_name
                    .clone()
                    .unwrap_or_else(|| crate::utils::mask_key(value));
                format!("{name} · {}", source.describe())
            }
            None => "none".to_string(),
        }
    }

    /// Required slots that are set to a model passing the filter, out of how
    /// many there are. Offline: whether the gateway lists them is `ccc check`'s job.
    pub fn slots_ready(&self) -> (usize, usize) {
        let required: Vec<usize> = (0..SLOTS.len()).filter(|&i| !SLOTS[i].optional).collect();
        let ready = required
            .iter()
            .filter(|&&i| {
                self.slots[i]
                    .as_deref()
                    .is_some_and(|v| matches_filter(v, &self.filter))
            })
            .count();
        (ready, required.len())
    }
}
