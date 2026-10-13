//! The checks behind `ccc check`, as data. The command prints them; the TUI
//! shows the same report, so the two can never disagree about what is wrong.

use crate::api::{
    current_key_with_source, get_api_config, is_listed, list_models_at, model_filter_for,
    usable_models, KeySource,
};
use crate::config::{effective_settings_path, env_value, read_json_or_default, ENV_GATEWAY_DISCOVERY};
use crate::slots::{self, LEGACY_FAST_ENV, SLOTS};
use crate::utils::mask_key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    /// Works, but worth knowing about.
    Warn,
    Fail,
    /// Neither good nor bad; not counted.
    Info,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub status: Status,
    pub label: String,
    pub detail: String,
}

impl Item {
    fn new(status: Status, label: impl Into<String>, detail: impl Into<String>) -> Self {
        Item { status, label: label.into(), detail: detail.into() }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Context lines: which gateway, key and file were checked.
    pub rows: Vec<(String, String)>,
    /// Checks, in groups printed with a separator between them.
    pub groups: Vec<Vec<Item>>,
}

impl Report {
    pub fn count(&self, status: Status) -> usize {
        self.groups
            .iter()
            .flatten()
            .filter(|item| item.status == status)
            .count()
    }
}

/// Run every check against the gateway. `Err` only when there is no key to
/// check with at all.
pub fn run() -> Result<Report, String> {
    let (api_key, source) = current_key_with_source().ok_or_else(|| {
        "No API key yet. Add one with `ccc key add <name> <key>`, then `ccc key use <name>`."
            .to_string()
    })?;
    let (base_url, _) = get_api_config();
    let filter = model_filter_for(&base_url);
    let settings = effective_settings_path();

    let mut report = Report::default();
    report.rows.push((
        "API".to_string(),
        format!("{}/v1/models", base_url.trim().trim_end_matches('/')),
    ));
    report.rows.push((
        "Key".to_string(),
        format!("{} ({})", mask_key(&api_key), source.describe()),
    ));
    report.rows.push((
        "Config".to_string(),
        match &settings {
            Some((path, true)) => path.display().to_string(),
            Some((path, false)) => {
                format!("{} (ccc template, not read by Claude Code)", path.display())
            }
            None => "none found. Run 'ccc lite' or 'ccc init'".to_string(),
        },
    ));
    report.rows.push((
        "Model filter".to_string(),
        if filter.is_empty() {
            "none — every model the gateway lists counts".to_string()
        } else {
            format!("{filter} — only models ending in {filter} count")
        },
    ));

    // 0. Whether any of this applies here. Claude Code reads only this
    //    folder's file; a setup that lives in ccc's template or keys.json
    //    checks out fine and still does nothing.
    let mut access = vec![match (&settings, source) {
        (Some((_, true)), KeySource::Folder) => {
            Item::new(Status::Pass, "This folder", "key and settings in .claude/settings.local.json")
        }
        (Some((_, true)), _) => Item::new(
            Status::Warn,
            "This folder",
            "settings but no key — run 'ccc key use <name>'",
        ),
        _ => Item::new(
            Status::Warn,
            "This folder",
            "not set up — Claude Code won't use this; run 'ccc lite'",
        ),
    }];

    // 1. The key itself. /v1/models needs no model entitlement, so a failure
    //    here is genuinely the key and not a model-permission problem.
    let models = match list_models_at(&base_url, &api_key) {
        Ok(models) => {
            access.push(Item::new(Status::Pass, "Auth", format!("valid, {} models listed", models.len())));
            models
        }
        Err(e) => {
            access.push(Item::new(Status::Fail, "Auth", e.to_string()));
            report.groups.push(access);
            return Ok(report);
        }
    };

    // 2. Models the key may actually use.
    let usable = usable_models(&models, &filter);
    access.push(match (usable.len(), filter.is_empty()) {
        (0, true) => Item::new(Status::Fail, "Models", "the gateway lists no models for this key"),
        (0, false) => Item::new(Status::Fail, "Models", format!("no {filter} model listed")),
        (n, true) => Item::new(Status::Pass, "Models", format!("{n} usable")),
        (n, false) => Item::new(Status::Pass, "Models", format!("{n} usable ({filter})")),
    });

    // 3. Gateway discovery. Optional: with it off, /model shows only the
    //    aliases, which still work as long as every slot below is set.
    let json = settings
        .as_ref()
        .map(|(path, _)| read_json_or_default(path))
        .unwrap_or_else(|| serde_json::json!({}));
    let discovery_on = json["env"][ENV_GATEWAY_DISCOVERY].as_str() == Some("1");
    access.push(if discovery_on {
        Item::new(Status::Pass, "Discovery", "on — /model lists the gateway's models")
    } else {
        Item::new(
            Status::Warn,
            "Discovery",
            "off — /model shows aliases only; fine while every slot below is set",
        )
    });
    report.groups.push(access);

    // 4. Every slot must be a model the key can reach. This is the check that
    //    catches a config pinned to a built-in name, which fails at runtime
    //    with "403 key not allowed to access model".
    let mut pinned = Vec::new();
    for (i, slot) in SLOTS.iter().enumerate() {
        let label = format!("{} model", slot.label);
        pinned.push(match slots::read(&json, i) {
            Some(value) => slot_item(&label, &value, &models, &usable, &filter, discovery_on),
            None if slot.optional => Item::new(Status::Info, label, "not set (optional)"),
            None => Item::new(
                Status::Fail,
                label,
                format!("{} not set — falls back to a built-in id (will 403)", slot.env),
            ),
        });
    }
    pinned.push(match env_value(&json, LEGACY_FAST_ENV) {
        Some(value) => slot_item("Fast (legacy)", &value, &models, &usable, &filter, false),
        None => Item::new(
            Status::Info,
            "Fast (legacy)",
            "not set — deprecated; background tasks use the Haiku slot",
        ),
    });
    report.groups.push(pinned);

    Ok(report)
}

fn slot_item(
    label: &str,
    value: &str,
    models: &[String],
    usable: &[String],
    filter: &str,
    discovery_on: bool,
) -> Item {
    if is_listed(value, usable) {
        // Discovery drops ids that do not mention Claude, so such a model
        // works but cannot be picked from /model by name.
        if discovery_on && !slots::shows_in_picker(value) {
            Item::new(Status::Warn, label, format!("{value} — works, but /model will not list it"))
        } else {
            Item::new(Status::Pass, label, value)
        }
    } else if is_listed(value, models) {
        Item::new(
            Status::Fail,
            label,
            format!("{value} — listed, but not a {filter} model (will 403)"),
        )
    } else {
        Item::new(
            Status::Fail,
            label,
            format!("{value} — not in your key's model list (will 403)"),
        )
    }
}
