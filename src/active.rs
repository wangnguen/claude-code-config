//! The model a new Claude Code session in this folder starts on, worked out
//! the way Claude Code itself decides, without asking the gateway.
//!
//! Two rules from Claude Code's documentation decide it:
//!
//! - An `env` entry in a settings file replaces the same variable inherited
//!   from the shell, and between files the higher one wins: managed policy,
//!   then `.claude/settings.local.json`, then `.claude/settings.json`, then
//!   the user's `~/.claude/settings.json`. The shell counts only when no file
//!   sets the variable.
//! - The starting model is `ANTHROPIC_MODEL` if set, else the `model` key
//!   (which `/model` saves in user settings), else `ANTHROPIC_DEFAULT_MODEL`,
//!   else Claude Code's own default, which is `opus` for gateway users.
//!
//! A `--model` flag or a `/model` switch inside a running session is out of
//! reach of any file, so `/status` in Claude Code stays the final word.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::{
    env_value, local_settings_path, managed_settings_path, read_json_or_default,
    user_claude_settings_path,
};
use crate::slots::{self, HAIKU, MAIN, OPUS, SLOTS, SONNET};

/// The variable that names the starting model; also the Main slot.
const ENV_MODEL: &str = "ANTHROPIC_MODEL";
/// Claude Code's own fallback, used only when nothing else picks a model.
const ENV_DEFAULT_MODEL: &str = "ANTHROPIC_DEFAULT_MODEL";
/// What Claude Code starts on when nothing at all selects a model.
const BUILT_IN_DEFAULT: &str = "opus";

/// Where a setting came from, highest precedence first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Managed,
    Local,
    Project,
    User,
    Shell,
    BuiltIn,
}

impl Origin {
    pub fn describe(self) -> &'static str {
        match self {
            Origin::Managed => "managed policy",
            Origin::Local => "this folder's settings",
            Origin::Project => "this folder's shared settings",
            Origin::User => "your user settings",
            Origin::Shell => "your shell",
            Origin::BuiltIn => "Claude Code's built-in default",
        }
    }

    /// Two or three words, for a column.
    pub fn short(self) -> &'static str {
        match self {
            Origin::Managed => "managed policy",
            Origin::Local => "this folder",
            Origin::Project => "project settings",
            Origin::User => "user settings",
            Origin::Shell => "shell",
            Origin::BuiltIn => "built-in",
        }
    }
}

/// One settings file Claude Code reads, as JSON.
pub struct Layer {
    pub origin: Origin,
    pub path: PathBuf,
    pub json: Value,
}

/// Everything Claude Code reads for this folder, in precedence order, and the
/// shell variables it inherits.
pub struct Sources {
    pub layers: Vec<Layer>,
    shell: fn(&str) -> Option<String>,
}

impl Sources {
    /// The files that exist here, and the real shell environment.
    pub fn load() -> Self {
        let candidates = [
            (Origin::Managed, Some(managed_settings_path())),
            (Origin::Local, Some(local_settings_path())),
            (Origin::Project, Some(Path::new(".claude").join("settings.json"))),
            (Origin::User, user_claude_settings_path()),
        ];
        let layers = candidates
            .into_iter()
            .filter_map(|(origin, path)| path.map(|p| (origin, p)))
            .filter(|(_, path)| path.exists())
            .map(|(origin, path)| Layer { json: read_json_or_default(&path), origin, path })
            .collect();
        Sources { layers, shell: shell_env }
    }

    /// A variable as a session sees it: the highest file that sets it, or else
    /// the shell.
    pub fn env(&self, name: &str) -> Option<(String, Origin)> {
        self.layers
            .iter()
            .find_map(|layer| env_value(&layer.json, name).map(|v| (v, layer.origin)))
            .or_else(|| (self.shell)(name).map(|v| (v, Origin::Shell)))
    }

    /// The `model` key from the highest file that sets it.
    fn model_key(&self) -> Option<(String, Origin)> {
        self.layers.iter().find_map(|layer| {
            layer.json["model"]
                .as_str()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(|v| (v.to_string(), layer.origin))
        })
    }

    /// Whether this folder has settings of its own that Claude Code reads.
    pub fn has_local(&self) -> bool {
        self.layers.iter().any(|l| l.origin == Origin::Local)
    }
}

impl Sources {
    /// Sources built from JSON values, with no shell: for tests elsewhere.
    #[cfg(test)]
    pub fn from_json(layers: Vec<(Origin, Value)>) -> Self {
        Sources {
            layers: layers
                .into_iter()
                .map(|(origin, json)| Layer { origin, path: PathBuf::from("settings.json"), json })
                .collect(),
            shell: |_| None,
        }
    }
}

fn shell_env(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// How the starting model was chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    /// `ANTHROPIC_MODEL` names it.
    Env,
    /// The `model` key, as `/model` saves it.
    ModelKey,
    /// `ANTHROPIC_DEFAULT_MODEL`, used because nothing else picked one.
    DefaultEnv,
    /// Nothing picked a model.
    BuiltIn,
}

/// The model a new session starts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Active {
    /// What the source says: a full id, or an alias like `opus` or `sonnet[1m]`.
    pub requested: String,
    /// The model id that alias stands for, when an alias slot maps it; equal
    /// to `requested` for a full id.
    pub model: String,
    /// The slot the alias resolves through, when it is one.
    pub via: Option<usize>,
    pub pick: Pick,
    pub origin: Origin,
    /// A `/model` choice that `ANTHROPIC_MODEL` keeps from taking effect.
    pub shadowed: Option<(String, Origin)>,
}

impl Active {
    /// "claude-opus-5-5-vn[1m]", or "opus → claude-opus-5-5-vn[1m]" when an
    /// alias was resolved.
    pub fn display(&self) -> String {
        if self.via.is_some() && self.requested != self.model {
            format!("{} → {}", self.requested, self.model)
        } else {
            self.model.clone()
        }
    }

    /// One line on why this model and not another, for under the table.
    pub fn why(&self) -> String {
        let source = self.origin.describe();
        match self.pick {
            Pick::Env => format!("{ENV_MODEL} in {source}"),
            Pick::ModelKey => format!("saved by /model, in {source}"),
            Pick::DefaultEnv => format!("{ENV_DEFAULT_MODEL} in {source}"),
            Pick::BuiltIn => "nothing selects a model: Claude Code's default".to_string(),
        }
    }
}

/// The model a new session started in this folder would use.
pub fn resolve(sources: &Sources) -> Active {
    let saved = sources.model_key();
    let (requested, origin, pick) = if let Some((value, origin)) = sources.env(ENV_MODEL) {
        (value, origin, Pick::Env)
    } else if let Some((value, origin)) = saved.clone() {
        (value, origin, Pick::ModelKey)
    } else if let Some((value, origin)) = sources.env(ENV_DEFAULT_MODEL) {
        (value, origin, Pick::DefaultEnv)
    } else {
        (BUILT_IN_DEFAULT.to_string(), Origin::BuiltIn, Pick::BuiltIn)
    };
    let shadowed = match pick {
        Pick::Env => saved.filter(|(value, _)| !same_model(value, &requested)),
        _ => None,
    };
    let (model, via) = resolve_alias(sources, &requested);
    Active { requested, model, via, pick, origin, shadowed }
}

fn same_model(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// `sonnet`, `opus`, `haiku` and `opusplan` stand for whatever the matching
/// slot holds; `[1m]` on the alias carries over onto the id it resolves to.
fn resolve_alias(sources: &Sources, requested: &str) -> (String, Option<usize>) {
    let lower = requested.trim().to_lowercase();
    let wants_1m = slots::has_1m(&lower);
    let alias = lower.trim_end_matches("[1m]");
    let slot = match alias {
        "sonnet" => SONNET,
        "opus" | "opusplan" | "best" => OPUS,
        "haiku" => HAIKU,
        _ => return (requested.to_string(), None),
    };
    match sources.env(SLOTS[slot].env) {
        Some((id, _)) => {
            let id = if wants_1m { slots::with_1m(&id, true) } else { id };
            (id, Some(slot))
        }
        // Without a slot the alias goes to Anthropic's own model of that
        // family, which a gateway key usually cannot reach.
        None => (requested.to_string(), Some(slot)),
    }
}

/// The value each slot resolves to across every file and the shell, with where
/// it came from.
pub fn slots(sources: &Sources) -> [Option<(String, Origin)>; 5] {
    std::array::from_fn(|i| sources.env(SLOTS[i].env))
}

/// The slot index the active model came in through, for marking it in a table.
pub fn active_slot(active: &Active) -> Option<usize> {
    match (&active.pick, active.via) {
        (_, Some(slot)) => Some(slot),
        (Pick::Env, None) => Some(MAIN),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn shell_sonnet(name: &str) -> Option<String> {
        match name {
            "ANTHROPIC_MODEL" => Some("claude-sonnet-5-vn".into()),
            "ANTHROPIC_DEFAULT_OPUS_MODEL" => Some("claude-opus-4-8-vn".into()),
            _ => None,
        }
    }

    /// No shell unless a test asks for one.
    fn sources(layers: Vec<(Origin, Value)>) -> Sources {
        Sources::from_json(layers)
    }

    #[test]
    fn the_folder_s_anthropic_model_wins_over_a_saved_model_choice() {
        let s = sources(
            vec![
                (Origin::Local, json!({"env": {"ANTHROPIC_MODEL": "claude-sonnet-5-vn[1m]"}})),
                (Origin::User, json!({"model": "opus"})),
            ],
        );
        let active = resolve(&s);
        assert_eq!(active.model, "claude-sonnet-5-vn[1m]");
        assert_eq!((active.pick.clone(), active.origin), (Pick::Env, Origin::Local));
        // /model's saved "opus" never takes effect here, and that is worth saying.
        assert_eq!(active.shadowed, Some(("opus".to_string(), Origin::User)));
        assert_eq!(active_slot(&active), Some(MAIN));
    }

    #[test]
    fn a_saved_alias_resolves_through_the_folder_s_slot() {
        let s = sources(
            vec![
                (Origin::Local, json!({"env": {"ANTHROPIC_DEFAULT_OPUS_MODEL": "claude-opus-5-5-vn"}})),
                (Origin::User, json!({"model": "opus[1m]"})),
            ],
        );
        let active = resolve(&s);
        assert_eq!(active.pick, Pick::ModelKey);
        assert_eq!(active.model, "claude-opus-5-5-vn[1m]");
        assert_eq!(active.display(), "opus[1m] → claude-opus-5-5-vn[1m]");
        assert_eq!(active_slot(&active), Some(OPUS));
        assert!(active.shadowed.is_none());
    }

    #[test]
    fn a_settings_file_replaces_the_shell_and_the_shell_fills_the_gaps() {
        let mut s = sources(vec![(Origin::Local, json!({"env": {"ANTHROPIC_MODEL": "claude-haiku-4-5-vn"}}))]);
        s.shell = shell_sonnet;
        assert_eq!(resolve(&s).model, "claude-haiku-4-5-vn");
        assert_eq!(s.env("ANTHROPIC_DEFAULT_OPUS_MODEL"), Some(("claude-opus-4-8-vn".into(), Origin::Shell)));
    }

    #[test]
    fn managed_policy_outranks_the_folder() {
        let s = sources(
            vec![
                (Origin::Managed, json!({"env": {"ANTHROPIC_MODEL": "claude-opus-5-vn"}})),
                (Origin::Local, json!({"env": {"ANTHROPIC_MODEL": "claude-sonnet-5-vn"}})),
            ],
        );
        assert_eq!((resolve(&s).model, resolve(&s).origin), ("claude-opus-5-vn".to_string(), Origin::Managed));
    }

    #[test]
    fn nothing_set_means_opus_and_an_unmapped_alias_stays_an_alias() {
        let active = resolve(&sources(vec![]));
        assert_eq!(active.pick, Pick::BuiltIn);
        assert_eq!(active.model, "opus");
        assert_eq!(active.via, Some(OPUS));
        assert_eq!(active.display(), "opus");
    }

    #[test]
    fn anthropic_default_model_counts_only_when_nothing_else_picks() {
        let s = sources(vec![(Origin::User, json!({"env": {"ANTHROPIC_DEFAULT_MODEL": "claude-sonnet-5-vn"}}))]);
        let active = resolve(&s);
        assert_eq!((active.pick, active.model.as_str()), (Pick::DefaultEnv, "claude-sonnet-5-vn"));
    }
}
