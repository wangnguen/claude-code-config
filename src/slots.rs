//! The model "slots" Claude Code reads from `env`, and the rules for filling
//! them from a gateway's model list. Nothing here touches the disk or the
//! network, so every screen and command shares one definition of what a
//! valid model setup is.

use std::cmp::Ordering;

use serde_json::Value;

use crate::api::{base_model_id, is_listed};

/// One model variable, as people see it in `/model`.
pub struct Slot {
    /// Name used on the command line (`ccc models set sonnet …`).
    pub name: &'static str,
    /// The env variable Claude Code reads.
    pub env: &'static str,
    /// Short label for tables.
    pub label: &'static str,
    /// What the slot does, in one line.
    pub help: &'static str,
    /// Family preferred when picking a model for it automatically.
    pub family: &'static str,
    /// Optional slots may stay empty; the others fall back to a built-in id,
    /// which a gateway key cannot reach.
    pub optional: bool,
}

pub const MAIN: usize = 0;
pub const SONNET: usize = 1;
pub const OPUS: usize = 2;
pub const HAIKU: usize = 3;
pub const CUSTOM: usize = 4;

pub const SLOTS: [Slot; 5] = [
    Slot {
        name: "main",
        env: "ANTHROPIC_MODEL",
        label: "Main",
        help: "The model a new session starts with.",
        family: "sonnet",
        optional: false,
    },
    Slot {
        name: "sonnet",
        env: "ANTHROPIC_DEFAULT_SONNET_MODEL",
        label: "Sonnet",
        help: "What `sonnet` in /model resolves to.",
        family: "sonnet",
        optional: false,
    },
    Slot {
        name: "opus",
        env: "ANTHROPIC_DEFAULT_OPUS_MODEL",
        label: "Opus",
        help: "What `opus` in /model resolves to; also opusplan.",
        family: "opus",
        optional: false,
    },
    Slot {
        name: "haiku",
        env: "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        label: "Haiku",
        help: "What `haiku` resolves to; also runs background tasks.",
        family: "haiku",
        optional: false,
    },
    Slot {
        name: "custom",
        env: "ANTHROPIC_CUSTOM_MODEL_OPTION",
        label: "Custom",
        help: "One extra /model entry; any id, not validated.",
        family: "",
        optional: true,
    },
];

/// Older Claude Code versions read their background model from here. It is
/// deprecated, so ccc never asks about it: it simply follows the Haiku slot.
pub const LEGACY_FAST_ENV: &str = "ANTHROPIC_SMALL_FAST_MODEL";

/// Find a slot by its command-line name, a `*_model` alias as used by
/// `ccc config`, or its env variable.
pub fn find(name: &str) -> Option<usize> {
    let name = name.trim();
    let lower = name.to_lowercase();
    let short = lower.strip_suffix("_model").unwrap_or(&lower);
    SLOTS
        .iter()
        .position(|s| s.name == short || s.env.eq_ignore_ascii_case(name))
}

pub fn names() -> String {
    SLOTS.iter().map(|s| s.name).collect::<Vec<_>>().join(", ")
}

/// The value currently set for a slot.
pub fn read(json: &Value, slot: usize) -> Option<String> {
    crate::config::env_value(json, SLOTS[slot].env)
}

/// Read every slot at once, in `SLOTS` order.
pub fn read_all(json: &Value) -> [Option<String>; 5] {
    std::array::from_fn(|i| read(json, i))
}

/// Set or clear a slot. Writing Haiku keeps the legacy fast-model variable in
/// step, so an older Claude Code never runs background work on a stale id.
pub fn write(json: &mut Value, slot: usize, value: Option<&str>) {
    if !json.is_object() {
        *json = Value::Object(serde_json::Map::new());
    }
    if !json["env"].is_object() {
        json["env"] = Value::Object(serde_json::Map::new());
    }
    let env = json["env"].as_object_mut().expect("env was just made an object");
    let mut set = |name: &str| match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => {
            env.insert(name.to_string(), Value::String(v.to_string()));
        }
        None => {
            env.remove(name);
        }
    };
    set(SLOTS[slot].env);
    if slot == HAIKU {
        set(LEGACY_FAST_ENV);
    }
}

/// Only Sonnet and Opus come in a 1M-context variant.
pub fn supports_1m(id: &str) -> bool {
    let id = base_model_id(id).to_lowercase();
    id.contains("sonnet") || id.contains("opus")
}

pub fn has_1m(id: &str) -> bool {
    id.trim_end().to_lowercase().ends_with("[1m]")
}

/// Add or drop the `[1m]` marker. The marker only goes on models that have a
/// 1M variant; asking for it on any other model returns the bare id.
pub fn with_1m(id: &str, on: bool) -> String {
    let bare = base_model_id(id).to_string();
    if on && supports_1m(&bare) {
        format!("{bare}[1m]")
    } else {
        bare
    }
}

/// Claude Code's gateway discovery keeps only ids that mention Claude, so any
/// other id never shows up in `/model` even with discovery on.
pub fn shows_in_picker(id: &str) -> bool {
    let id = id.to_lowercase();
    id.contains("claude") || id.contains("anthropic")
}

/// Compare with digit runs as numbers, so `4-10` sorts after `4-9` and a
/// version bump never ends up in the middle of the list.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let start_i = i;
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            let start_j = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let x = std::str::from_utf8(&a[start_i..i]).unwrap_or("0");
            let y = std::str::from_utf8(&b[start_j..j]).unwrap_or("0");
            // Compare by length after stripping zeros, then lexically: works
            // for numbers of any size without parsing.
            let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
            let ord = x.len().cmp(&y.len()).then_with(|| x.cmp(y));
            if ord != Ordering::Equal {
                return ord;
            }
        } else {
            let ord = a[i].to_ascii_lowercase().cmp(&b[j].to_ascii_lowercase());
            if ord != Ordering::Equal {
                return ord;
            }
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}

fn in_family(id: &str, family: &str) -> bool {
    !family.is_empty() && id.to_lowercase().contains(family)
}

/// Models in the order a picker should offer them for a slot: the slot's own
/// family first, newest first, then everything else, newest first.
pub fn ordered_for(slot: usize, models: &[String]) -> Vec<String> {
    let family = SLOTS[slot].family;
    let mut ordered = models.to_vec();
    ordered.sort_by(|a, b| {
        in_family(b, family)
            .cmp(&in_family(a, family))
            .then_with(|| natural_cmp(b, a))
    });
    ordered
}

/// The model to suggest for a slot when its current value cannot be used.
/// The newest model of the slot's family wins; a slot whose family the key
/// has none of takes the newest Sonnet, then the newest Claude model.
pub fn suggest(slot: usize, models: &[String]) -> Option<String> {
    if slot == CUSTOM {
        return None;
    }
    let newest = |family: &str| {
        models
            .iter()
            .filter(|m| in_family(m, family))
            .max_by(|a, b| natural_cmp(a, b))
            .cloned()
    };
    newest(SLOTS[slot].family)
        .or_else(|| newest("sonnet"))
        .or_else(|| newest("claude"))
        .or_else(|| models.iter().max_by(|a, b| natural_cmp(a, b)).cloned())
}

/// A slot that `fit` changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub slot: usize,
    pub from: Option<String>,
    pub to: String,
}

/// Point every required slot at a model from `usable`, keeping any value that
/// already works. A replaced value keeps its 1M preference when the new model
/// has a 1M variant. Custom is never touched: it is the user's own choice.
pub fn fit(json: &mut Value, usable: &[String]) -> Vec<Change> {
    let mut changes = Vec::new();
    if usable.is_empty() {
        return changes;
    }
    for (slot, spec) in SLOTS.iter().enumerate() {
        if spec.optional {
            continue;
        }
        let current = read(json, slot);
        if current.as_deref().is_some_and(|v| is_listed(v, usable)) {
            continue;
        }
        if let Some(pick) = suggest(slot, usable) {
            let wants_1m = current.as_deref().is_some_and(has_1m);
            let to = with_1m(&pick, wants_1m);
            write(json, slot, Some(&to));
            changes.push(Change { slot, from: current, to });
        }
    }
    // The legacy variable follows Haiku even when Haiku itself was fine.
    let haiku = read(json, HAIKU);
    write(json, HAIKU, haiku.as_deref());
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn models(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn natural_order_compares_versions_numerically() {
        assert_eq!(natural_cmp("claude-sonnet-4-10", "claude-sonnet-4-9"), Ordering::Greater);
        assert_eq!(natural_cmp("claude-sonnet-5-vn", "claude-sonnet-4-6-vn"), Ordering::Greater);
        assert_eq!(natural_cmp("a", "a"), Ordering::Equal);
        assert_eq!(natural_cmp("a-1", "a-1-x"), Ordering::Less);
    }

    #[test]
    fn find_accepts_names_aliases_and_env() {
        assert_eq!(find("opus"), Some(OPUS));
        assert_eq!(find("sonnet_model"), Some(SONNET));
        assert_eq!(find("ANTHROPIC_MODEL"), Some(MAIN));
        assert_eq!(find("Custom"), Some(CUSTOM));
        assert_eq!(find("fast"), None);
    }

    #[test]
    fn one_million_marker_only_on_sonnet_and_opus() {
        assert_eq!(with_1m("claude-sonnet-5-vn", true), "claude-sonnet-5-vn[1m]");
        assert_eq!(with_1m("claude-haiku-4-5-vn", true), "claude-haiku-4-5-vn");
        assert_eq!(with_1m("claude-opus-5-vn[1m]", false), "claude-opus-5-vn");
        assert!(has_1m("claude-opus-5-vn[1m]"));
        assert!(!has_1m("claude-opus-5-vn"));
    }

    #[test]
    fn writing_haiku_keeps_the_legacy_variable_in_step() {
        let mut json = json!({});
        write(&mut json, HAIKU, Some("claude-haiku-4-5-vn"));
        assert_eq!(json["env"][LEGACY_FAST_ENV], "claude-haiku-4-5-vn");
        write(&mut json, HAIKU, None);
        assert!(json["env"].get(LEGACY_FAST_ENV).is_none());
    }

    #[test]
    fn clearing_custom_removes_the_variable() {
        let mut json = json!({"env": {"ANTHROPIC_CUSTOM_MODEL_OPTION": "x"}});
        write(&mut json, CUSTOM, Some("  "));
        assert!(json["env"].get("ANTHROPIC_CUSTOM_MODEL_OPTION").is_none());
    }

    #[test]
    fn ordered_puts_the_slot_family_first_newest_first() {
        let list = models(&["claude-haiku-4-5-vn", "claude-opus-4-8-vn", "claude-opus-5-vn", "claude-sonnet-5-vn"]);
        let ordered = ordered_for(OPUS, &list);
        assert_eq!(ordered[0], "claude-opus-5-vn");
        assert_eq!(ordered[1], "claude-opus-4-8-vn");
    }

    #[test]
    fn suggest_falls_back_when_a_family_is_missing() {
        let list = models(&["claude-sonnet-4-6-vn", "claude-sonnet-5-vn"]);
        assert_eq!(suggest(OPUS, &list).as_deref(), Some("claude-sonnet-5-vn"));
        assert_eq!(suggest(CUSTOM, &list), None);
        assert_eq!(suggest(MAIN, &[]), None);
    }

    #[test]
    fn fit_keeps_working_values_and_replaces_broken_ones() {
        let mut json = json!({"env": {
            "ANTHROPIC_MODEL": "claude-sonnet-5-vn[1m]",
            "ANTHROPIC_DEFAULT_SONNET_MODEL": "claude-sonnet-5[1m]",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL": "claude-haiku-4-5-vn",
        }});
        let usable = models(&["claude-haiku-4-5-vn", "claude-opus-5-vn", "claude-sonnet-5-vn"]);
        let changes = fit(&mut json, &usable);

        // Main already works and keeps its marker.
        assert_eq!(json["env"]["ANTHROPIC_MODEL"], "claude-sonnet-5-vn[1m]");
        // Sonnet was a built-in id: replaced, 1M preference kept.
        assert_eq!(json["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"], "claude-sonnet-5-vn[1m]");
        // Opus was missing: filled, no 1M asked for.
        assert_eq!(json["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"], "claude-opus-5-vn");
        // Haiku untouched, legacy variable synced anyway.
        assert_eq!(json["env"][LEGACY_FAST_ENV], "claude-haiku-4-5-vn");
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().all(|c| c.slot == SONNET || c.slot == OPUS));
    }

    #[test]
    fn fit_with_no_usable_models_changes_nothing() {
        let mut json = json!({"env": {"ANTHROPIC_MODEL": "x"}});
        assert!(fit(&mut json, &[]).is_empty());
        assert_eq!(json["env"]["ANTHROPIC_MODEL"], "x");
    }

    #[test]
    fn picker_visibility_follows_discovery_rule() {
        assert!(shows_in_picker("claude-sonnet-5-vn"));
        assert!(shows_in_picker("Anthropic/foo"));
        assert!(!shows_in_picker("gpt-4o"));
    }
}
