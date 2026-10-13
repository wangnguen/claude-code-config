use anyhow::Result;
use std::path::Path;
use std::process::Command;

use crate::api::{matches_filter, model_filter};
use crate::checks::Status;
use crate::config::{
    ccc_home, default_settings_path, env_value, local_settings_path, managed_settings_path,
    read_json_or_default, ENV_AUTH_TOKEN, ENV_BASE_URL, ENV_GATEWAY_DISCOVERY, SETTINGS_FILE,
    VERSION,
};
use crate::slots::{self, LEGACY_FAST_ENV, SLOTS};
use crate::ui;

pub fn run() -> Result<()> {
    println!();
    ui::print_header(&ui::ICON_DOC, &format!("ccc doctor v{VERSION}"));

    let mut pass = 0;
    let mut warn = 0;
    let mut fail = 0;

    // 1. Claude Code installed
    let claude_found = find_claude();
    match &claude_found {
        Some(path) => {
            ui::print_check(true, "Claude Code", path);
            pass += 1;
            if let Some(ver) = get_claude_version() {
                ui::print_check(true, "Claude version", &ver);
                pass += 1;
            }
        }
        None => { ui::print_check(false, "Claude Code", "not found"); fail += 1; }
    }

    // 2. ccc home
    let home = ccc_home()?;
    if home.exists() {
        ui::print_check(true, "ccc home", &home.display().to_string());
        pass += 1;
    } else {
        ui::print_check(false, "ccc home", "not found");
        fail += 1;
    }

    ui::print_separator();

    // 3. Global config
    let global_path = default_settings_path()?;
    if global_path.exists() {
        ui::print_check(true, "Global config", &global_path.display().to_string());
        pass += 1;

        // The template is meant to carry no key: keys live in keys.json and
        // in each folder. Either way it is information, not a failure.
        let json = read_json_or_default(&global_path);
        if auth_state(&json).0 {
            ui::print_status(Status::Info, "Auth token", "set in the template (keys belong in keys.json)");
        } else {
            ui::print_status(Status::Info, "Auth token", "none in the template, as expected");
        }
        // ccc's "global" file is only a template it copies on init; Claude Code
        // never reads it. Say so, or people edit it and wonder why nothing changed.
        ui::print_row("note", "template only, not read by Claude Code");
    } else {
        ui::print_check(false, "Global config", "not found. Run install script");
        fail += 1;
    }

    ui::print_separator();

    // 4. Local .claude folder
    let local_dir = Path::new(".claude");
    if local_dir.exists() {
        ui::print_check(true, ".claude/", "found");
        pass += 1;
    } else {
        ui::print_check(false, ".claude/", "not found. Run 'ccc init'");
        fail += 1;
    }

    // 5. Local settings
    let local_path = local_settings_path();
    if local_path.exists() {
        ui::print_check(true, "Local settings", &local_path.display().to_string());
        pass += 1;

        let json = read_json_or_default(&local_path);

        match auth_state(&json) {
            (true, true) => {
                ui::print_check(false, "Auth token", "both AUTH_TOKEN and API_KEY set");
                fail += 1;
            }
            (true, false) => {
                ui::print_check(true, "Auth token", "set (local)");
                pass += 1;
            }
            (false, true) => {
                ui::print_check(false, "Auth token", "legacy API_KEY. Run 'ccc key use'");
                fail += 1;
            }
            (false, false) => {
                ui::print_check(false, "Auth token", "not set (local)");
                fail += 1;
            }
        }

        let base_url = json["env"]["ANTHROPIC_BASE_URL"]
            .as_str()
            .filter(|u| !u.is_empty());
        match base_url {
            Some(url) => { ui::print_check(true, "Base URL", url); pass += 1; }
            None => { ui::print_check(false, "Base URL", "not set"); fail += 1; }
        }

        // Optional: without it /model lists only the aliases, and those work
        // as long as every slot below is set.
        let discovery = json["env"][ENV_GATEWAY_DISCOVERY].as_str();
        if discovery == Some("1") {
            ui::print_check(true, "Discovery", "on");
            pass += 1;
        } else {
            ui::print_status(Status::Warn, "Discovery", "off — /model shows aliases only");
            warn += 1;
        }

        // A slot pinned to a model outside the filter, or not pinned at all,
        // is the usual cause of "403 key not allowed to access model".
        // Offline, the filter is all there is to go on; `ccc check` asks the
        // gateway.
        let filter = model_filter();
        let rule = if filter.is_empty() {
            "no model filter; 'ccc check' asks the gateway".to_string()
        } else {
            format!("must end in {filter}")
        };
        ui::print_separator();
        ui::print_row("Models", &rule);
        for (i, slot) in SLOTS.iter().enumerate() {
            let label = format!("{} model", slot.label);
            match slots::read(&json, i) {
                Some(value) if matches_filter(&value, &filter) => {
                    ui::print_check(true, &label, &value);
                    pass += 1;
                }
                Some(value) => {
                    ui::print_check(false, &label, &format!("{value} — not {filter}, will 403"));
                    fail += 1;
                }
                None if slot.optional => ui::print_status(Status::Info, &label, "not set (optional)"),
                None => {
                    ui::print_check(false, &label, &format!("{} not set — falls back to a built-in, will 403", slot.env));
                    fail += 1;
                }
            }
        }
        match env_value(&json, LEGACY_FAST_ENV) {
            Some(value) if matches_filter(&value, &filter) => {
                ui::print_check(true, "Fast (legacy)", &value);
                pass += 1;
            }
            Some(value) => {
                ui::print_check(false, "Fast (legacy)", &format!("{value} — not {filter}, will 403"));
                fail += 1;
            }
            None => ui::print_status(Status::Info, "Fast (legacy)", "not set — deprecated, Haiku slot covers it"),
        }
    } else {
        ui::print_check(false, "Local settings", &format!("no {SETTINGS_FILE}. Run 'ccc lite' or 'ccc init'"));
        fail += 1;
    }

    // Anything that outranks the file above and can silently undo it.
    ui::print_separator();
    let managed = managed_settings_path();
    if managed.exists() {
        ui::print_check(false, "Managed policy", &managed.display().to_string());
        fail += 1;
    } else {
        ui::print_check(true, "Managed policy", "none");
        pass += 1;
    }

    let overrides: Vec<&str> = [ENV_BASE_URL, ENV_AUTH_TOKEN, LEGACY_FAST_ENV]
        .into_iter()
        .chain(SLOTS.iter().map(|slot| slot.env))
        .filter(|name| std::env::var(name).is_ok_and(|v| !v.is_empty()))
        .collect();
    if overrides.is_empty() {
        ui::print_check(true, "Shell env", "no overrides");
        pass += 1;
    } else {
        ui::print_check(false, "Shell env", &format!("{} set in shell", overrides.join(", ")));
        fail += 1;
    }

    // Result
    ui::print_separator();
    ui::print_result_line(pass, warn, fail);
    ui::print_footer();
    println!();

    Ok(())
}

/// (auth token set, legacy api key set)
fn auth_state(json: &serde_json::Value) -> (bool, bool) {
    let is_set = |name: &str| json["env"][name].as_str().is_some_and(|v| !v.is_empty());
    (is_set(ENV_AUTH_TOKEN), is_set(crate::config::ENV_API_KEY))
}

fn find_claude() -> Option<String> {
    let cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
    if let Ok(output) = Command::new(cmd).arg("claude").output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().lines().next().unwrap_or("").to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();

    let candidates = [
        format!("{home}/.local/bin/claude"),
        format!("{home}/.local/bin/claude.exe"),
    ];

    for path in &candidates {
        if Path::new(path).exists() {
            return Some(path.clone());
        }
    }

    None
}

fn get_claude_version() -> Option<String> {
    if let Ok(output) = Command::new("claude").arg("--version").output()
        && output.status.success()
    {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !version.is_empty() {
            return Some(version);
        }
    }
    None
}
