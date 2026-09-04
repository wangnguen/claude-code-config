use anyhow::{bail, Result};

use crate::api::{base_model_id, get_api_config, get_current_key, is_vn_model, list_models, VN_SUFFIX};
use crate::config::{
    effective_settings_path, read_json_or_default, ENV_GATEWAY_DISCOVERY, MODEL_ENV_KEYS,
};
use crate::ui;
use crate::utils::mask_key;

pub fn run() -> Result<()> {
    let api_key = match get_current_key() {
        Some(key) => key,
        None => bail!("API key not set. Run 'ccc key add' first."),
    };

    let (base_url, _) = get_api_config();

    println!();
    ui::print_header(&ui::ICON_SEARCH, "API Connection Check");
    ui::print_row("API", &format!("{base_url}/v1/models"));
    ui::print_row("Key", &mask_key(&api_key));

    let settings = effective_settings_path();
    match &settings {
        Some((path, true)) => ui::print_row("Config", &path.display().to_string()),
        Some((path, false)) => ui::print_row(
            "Config",
            &format!("{} (ccc template, not read by Claude Code)", path.display()),
        ),
        None => ui::print_row("Config", "none found. Run 'ccc lite' or 'ccc init'"),
    }

    ui::print_separator();

    let mut pass = 0;
    let mut fail = 0;

    // 1. The key itself. /v1/models needs no model entitlement, so a failure
    //    here is genuinely the key and not a model-permission problem.
    let sp = ui::spinner("Asking the gateway for your model list...");
    let models = list_models(&api_key);
    sp.finish_and_clear();

    let models = match models {
        Ok(models) => {
            ui::print_check(true, "Auth", &format!("valid, {} models", models.len()));
            pass += 1;
            models
        }
        Err(e) => {
            ui::print_check(false, "Auth", &e);
            ui::print_separator();
            ui::print_result_line(pass, fail + 1);
            ui::print_footer();
            println!();
            return Ok(());
        }
    };

    // 2. Models the key may actually use.
    let vn: Vec<&String> = models.iter().filter(|m| is_vn_model(m)).collect();
    if vn.is_empty() {
        ui::print_check(false, "Models", &format!("no {VN_SUFFIX} model returned"));
        fail += 1;
    } else {
        ui::print_check(true, "Models", &format!("{} with {VN_SUFFIX}", vn.len()));
        pass += 1;
    }

    // 3. Gateway discovery, without which the picker only shows built-ins.
    let json = settings
        .as_ref()
        .map(|(path, _)| read_json_or_default(path))
        .unwrap_or_else(|| serde_json::json!({}));

    let discovery_on = json["env"][ENV_GATEWAY_DISCOVERY]
        .as_str()
        .is_some_and(|v| v == "1");
    if discovery_on {
        ui::print_check(true, "Model discovery", "enabled");
        pass += 1;
    } else {
        ui::print_check(
            false,
            "Model discovery",
            &format!("{ENV_GATEWAY_DISCOVERY} not set to 1"),
        );
        fail += 1;
    }

    // 4. Every pinned model must be one the key can reach. This is the check
    //    that catches a config pinned to a built-in name, which fails at
    //    runtime with "403 key not allowed to access model".
    ui::print_separator();
    for name in MODEL_ENV_KEYS {
        match json["env"][*name].as_str().filter(|v| !v.is_empty()) {
            Some(value) => {
                // Compare on the bare id: a `[1m]` marker is client-side only
                // and never appears in the gateway's list.
                let allowed = models.iter().any(|m| m == base_model_id(value));
                let detail = if allowed {
                    value.to_string()
                } else {
                    format!("{value} — not in your key's model list (will 403)")
                };
                ui::print_check(allowed, name, &detail);
                if allowed { pass += 1 } else { fail += 1 }
            }
            None => {
                ui::print_check(
                    false,
                    name,
                    "not set — Claude Code falls back to a built-in model (will 403)",
                );
                fail += 1;
            }
        }
    }

    ui::print_separator();
    ui::print_result_line(pass, fail);
    ui::print_footer();
    println!();

    Ok(())
}
