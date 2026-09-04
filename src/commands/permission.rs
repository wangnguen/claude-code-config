use anyhow::Result;

use crate::config::{local_settings_path, read_json_or_default, write_json};
use crate::ui;
use crate::utils::confirm;

/// Permissions installed/merged into Claude Code config.
const DEFAULT_ALLOW: &[&str] = &[
    "Write",
    "Bash",
    "Read",
    "Edit",
    "Glob",
    "Grep",
    "Agent",
    "WebFetch",
    "WebSearch",
];

pub fn run() -> Result<()> {
    let path = local_settings_path();

    // Load existing config (empty object if missing/invalid).
    let mut json = if path.exists() {
        read_json_or_default(&path)
    } else {
        serde_json::json!({})
    };

    // Current allow list (preserve existing entries).
    let mut allow: Vec<String> = json["permissions"]["allow"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    // Compute which permissions are new (merge, dedupe).
    let to_add: Vec<&str> = DEFAULT_ALLOW
        .iter()
        .copied()
        .filter(|p| !allow.iter().any(|existing| existing == p))
        .collect();

    println!();
    ui::print_header(&ui::ICON_DOC, "Claude Code Permissions");
    ui::print_row("Target", &path.display().to_string());
    ui::print_separator();
    for perm in DEFAULT_ALLOW {
        let is_new = to_add.contains(perm);
        let detail = if is_new { "will add" } else { "already set" };
        ui::print_check(!is_new, perm, detail);
    }
    ui::print_footer();
    println!();

    if to_add.is_empty() {
        println!("All permissions already present. Nothing to do.");
        return Ok(());
    }

    if !confirm(&format!(
        "Add {} permission(s) to {}? (y/N): ",
        to_add.len(),
        path.display()
    )) {
        println!("Cancelled.");
        return Ok(());
    }

    // Ensure parent dir exists, then merge and write.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    for perm in &to_add {
        allow.push(perm.to_string());
    }
    json["permissions"]["allow"] =
        serde_json::Value::Array(allow.into_iter().map(serde_json::Value::String).collect());
    write_json(&path, &json)?;

    println!("Added {} permission(s) to {}.", to_add.len(), path.display());
    Ok(())
}
