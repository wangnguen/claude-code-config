use anyhow::{anyhow, bail, Result};

use crate::api::{
    get_api_config, get_current_key, is_listed, list_models_at, matches_filter, model_filter_for,
    usable_models,
};
use crate::config::{
    effective_settings_path, env_value, load_local_for_edit, read_json_or_default, write_json,
    ENV_AUTH_TOKEN, ENV_BASE_URL,
};
use crate::checks::Status;
use crate::slots::{self, CUSTOM, SLOTS};
use crate::ui;
use crate::utils::{ignore_local_settings, mask_key};

#[derive(clap::Subcommand)]
pub enum ModelsCmd {
    /// List the models the current key can reach (the default)
    List,
    /// Choose this folder's models in a full-screen picker
    Pick,
    /// Set one model slot in this folder's .claude/settings.local.json
    Set {
        /// Slot: main, sonnet, opus, haiku or custom
        slot: String,
        /// Model id, optionally ending in [1m]; `none` clears the custom slot
        model: String,
        /// Write it even when the gateway does not list it or cannot be reached
        #[arg(long)]
        force: bool,
    },
}

pub fn run(subcmd: Option<ModelsCmd>) -> Result<()> {
    match subcmd.unwrap_or(ModelsCmd::List) {
        ModelsCmd::List => list(),
        ModelsCmd::Pick => crate::tui::run(crate::tui::Screen::Models),
        ModelsCmd::Set { slot, model, force } => set(&slot, &model, force),
    }
}

fn list() -> Result<()> {
    let api_key = get_current_key()
        .ok_or_else(|| anyhow!("API key not set. Run 'ccc key add' first."))?;
    let (base_url, _) = get_api_config();
    let filter = model_filter_for(&base_url);

    println!();
    ui::print_header(&ui::ICON_DOC, "Gateway Models");
    ui::print_row("API", &format!("{}/v1/models", base_url.trim_end_matches('/')));
    ui::print_row("Key", &mask_key(&api_key));
    ui::print_row(
        "Filter",
        if filter.is_empty() { "none — every listed model is usable" } else { &filter },
    );
    ui::print_separator();

    let sp = ui::spinner("Fetching model list...");
    let result = list_models_at(&base_url, &api_key);
    sp.finish_and_clear();

    let models = match result {
        Ok(models) if models.is_empty() => {
            ui::print_check(false, "Result", "gateway returned no models");
            ui::print_footer();
            println!();
            return Ok(());
        }
        Ok(models) => models,
        Err(e) => {
            ui::print_check(false, "Result", &e.to_string());
            ui::print_footer();
            println!();
            return Ok(());
        }
    };

    // Which slots of this folder point at each model, so the list doubles as
    // a view of the current setup.
    let json = effective_settings_path()
        .map(|(path, _)| read_json_or_default(&path))
        .unwrap_or_else(|| serde_json::json!({}));
    let current = slots::read_all(&json);

    // The gateway can list models the key is not entitled to, so mark which
    // ones are safe to select rather than printing a flat list.
    let label_width = models.iter().map(|m| m.len()).max().unwrap_or(0).clamp(16, 32);
    for id in &models {
        let usable = matches_filter(id, &filter);
        let used_by: Vec<&str> = current
            .iter()
            .enumerate()
            .filter(|(_, v)| v.as_deref().is_some_and(|v| is_listed(v, std::slice::from_ref(id))))
            .map(|(i, _)| SLOTS[i].name)
            .collect();
        let detail = match (usable, used_by.is_empty()) {
            (false, _) => "not for this key".to_string(),
            (true, true) => "usable".to_string(),
            (true, false) => format!("usable · {}", used_by.join(", ")),
        };
        let status = if usable { Status::Pass } else { Status::Fail };
        ui::print_status_with(status, id, &detail, label_width);
    }

    let usable = usable_models(&models, &filter).len();
    ui::print_separator();
    ui::print_row("Total", &models.len().to_string());
    ui::print_row("Usable", &usable.to_string());
    ui::print_footer();
    println!();

    if usable == 0 {
        println!("No usable model. Check that you are using your own key.");
    } else {
        println!("Choose with 'ccc models pick', or 'ccc models set <slot> <model>'.");
    }

    Ok(())
}

fn set(slot: &str, model: &str, force: bool) -> Result<()> {
    let index = slots::find(slot)
        .ok_or_else(|| anyhow!("Unknown slot '{slot}'. Use one of: {}.", slots::names()))?;
    let spec = &SLOTS[index];
    let clear = matches!(model.trim().to_lowercase().as_str(), "none" | "off" | "-" | "");
    if clear && !spec.optional {
        bail!(
            "The {} slot cannot be empty: `{}` would fall back to a built-in model and fail with 403.",
            spec.name,
            spec.name
        );
    }
    let value = model.trim().to_string();
    if !clear && slots::has_1m(&value) && !slots::supports_1m(&value) {
        bail!("{value}: only Sonnet and Opus models have a 1M variant. Drop the [1m].");
    }

    let (base_url, _) = get_api_config();
    if !clear && !force {
        verify(index, &value, &base_url)?;
    }

    let (path, mut json) = load_local_for_edit()?;
    slots::write(&mut json, index, if clear { None } else { Some(&value) });
    // Models belong to a gateway. A folder that names models but no gateway
    // would send them wherever the next settings file points.
    if env_value(&json, ENV_BASE_URL).is_none() {
        json["env"][ENV_BASE_URL] = serde_json::Value::String(base_url);
    }
    write_json(&path, &json)?;

    if clear {
        println!("Cleared the {} slot in {}.", spec.name, path.display());
    } else {
        println!("{} → {value}  ({})", spec.label, path.display());
    }
    if index == slots::HAIKU {
        println!("{} follows it, for older Claude Code versions.", slots::LEGACY_FAST_ENV);
    }
    if env_value(&json, ENV_AUTH_TOKEN).is_some() {
        ignore_local_settings();
    }
    println!("Restart Claude Code to pick it up.");
    Ok(())
}

/// Make sure the gateway offers `value` to the current key.
fn verify(index: usize, value: &str, base_url: &str) -> Result<()> {
    let api_key = get_current_key().ok_or_else(|| {
        anyhow!("No API key to check {value} with. Add one with 'ccc key add', or pass --force.")
    })?;
    let filter = model_filter_for(base_url);

    let sp = ui::spinner("Checking the model with the gateway...");
    let listed = list_models_at(base_url, &api_key);
    sp.finish_and_clear();

    let models = listed
        .map_err(|e| anyhow!("Couldn't verify {value}: {e}. Pass --force to write it anyway."))?;
    if is_listed(value, &usable_models(&models, &filter)) {
        return Ok(());
    }
    if index == CUSTOM {
        // The custom entry exists for ids Claude Code would not list itself;
        // say what will happen and let it through.
        println!("Note: {value} is not in your key's model list. Claude Code accepts it; the gateway may not.");
        return Ok(());
    }
    bail!("{value} is not available to this key. Run 'ccc models' to see what is, or pass --force.")
}
