use anyhow::Result;
use std::path::Path;
use std::process::Command;

use crate::api::is_vn_model;
use crate::config::{
    ccc_home, default_settings_path, local_settings_path, read_json_or_default,
    ENV_AUTH_TOKEN, ENV_GATEWAY_DISCOVERY, MODEL_ENV_KEYS, SETTINGS_FILE, VERSION,
};
use crate::ui;

pub fn run() -> Result<()> {
    println!();
    ui::print_header(&ui::ICON_DOC, &format!("ccc doctor v{VERSION}"));

    let mut pass = 0;
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

        let json = read_json_or_default(&global_path);
        if auth_state(&json).0 {
            ui::print_check(true, "Auth token", "set (template)");
            pass += 1;
        } else {
            ui::print_check(false, "Auth token", "not set. Run 'ccc key'");
            fail += 1;
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
        ui::print_check(true, SETTINGS_FILE, &local_path.display().to_string());
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

        let discovery = json["env"][ENV_GATEWAY_DISCOVERY].as_str();
        if discovery == Some("1") {
            ui::print_check(true, "Discovery", "enabled");
            pass += 1;
        } else {
            ui::print_check(false, "Discovery", "set it to 1 to see gateway models");
            fail += 1;
        }

        // A model pinned to a built-in name is the usual cause of
        // "403 key not allowed to access model".
        ui::print_separator();
        for name in MODEL_ENV_KEYS {
            match json["env"][*name].as_str().filter(|v| !v.is_empty()) {
                Some(value) if is_vn_model(value) => {
                    ui::print_check(true, name, value);
                    pass += 1;
                }
                Some(value) => {
                    ui::print_check(false, name, &format!("{value} — built-in name, will 403"));
                    fail += 1;
                }
                None => {
                    ui::print_check(false, name, "not set — falls back to a built-in, will 403");
                    fail += 1;
                }
            }
        }
    } else {
        ui::print_check(false, SETTINGS_FILE, "not found. Run 'ccc init'");
        fail += 1;
    }

    // Anything that outranks the file above and can silently undo it.
    ui::print_separator();
    let managed = managed_settings_path();
    if Path::new(&managed).exists() {
        ui::print_check(false, "Managed policy", &managed);
        fail += 1;
    } else {
        ui::print_check(true, "Managed policy", "none");
        pass += 1;
    }

    let overrides: Vec<&str> = ["ANTHROPIC_BASE_URL", "ANTHROPIC_MODEL", ENV_AUTH_TOKEN]
        .into_iter()
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
    ui::print_result_line(pass, fail);
    ui::print_footer();
    println!();

    Ok(())
}

/// (auth token set, legacy api key set)
fn auth_state(json: &serde_json::Value) -> (bool, bool) {
    let is_set = |name: &str| json["env"][name].as_str().is_some_and(|v| !v.is_empty());
    (is_set(ENV_AUTH_TOKEN), is_set(crate::config::ENV_API_KEY))
}

/// System-wide policy file, which outranks every other settings source.
fn managed_settings_path() -> String {
    if cfg!(target_os = "windows") {
        r"C:\ProgramData\ClaudeCode\managed-settings.json".to_string()
    } else if cfg!(target_os = "macos") {
        "/Library/Application Support/ClaudeCode/managed-settings.json".to_string()
    } else {
        "/etc/claude-code/managed-settings.json".to_string()
    }
}

fn find_claude() -> Option<String> {
    let cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
    if let Ok(output) = Command::new(cmd).arg("claude").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().lines().next().unwrap_or("").to_string();
            if !path.is_empty() {
                return Some(path);
            }
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
    if let Ok(output) = Command::new("claude").arg("--version").output() {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() {
                return Some(version);
            }
        }
    }
    None
}
