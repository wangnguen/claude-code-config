use anyhow::{Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Password;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Read a secret without echoing it to the terminal.
pub fn prompt_secret(message: &str) -> Result<String> {
    let value = Password::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .allow_empty_password(true)
        .interact()
        .context("Input cancelled")?;
    Ok(value.trim().to_string())
}

/// Make sure the project's .gitignore covers .claude/, which now holds a
/// virtual API key. No-op when there is no .gitignore or it already covers it.
pub fn ignore_claude_dir() {
    let path = Path::new(".gitignore");
    // Only touch git repos; elsewhere a stray .gitignore would be noise.
    if !path.exists() && !Path::new(".git").exists() {
        return;
    }
    let content = fs::read_to_string(path).unwrap_or_default();
    if content.lines().any(|l| {
        matches!(l.trim(), ".claude/" | ".claude" | "/.claude/" | "/.claude")
    }) {
        return;
    }
    let sep = if content.ends_with('\n') { "" } else { "\n" };
    if fs::write(path, format!("{content}{sep}.claude/\n")).is_ok() {
        println!("Added '.claude/' to .gitignore (it now contains your key).");
    }
}

pub fn prompt(message: &str) -> Result<String> {
    print!("{message}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).context("Failed to read input")?;
    Ok(input.trim().to_string())
}

pub fn confirm(message: &str) -> bool {
    let answer = prompt(message).unwrap_or_default();
    matches!(answer.to_lowercase().as_str(), "y" | "yes")
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

pub fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() > 8 {
        let first: String = chars[..4].iter().collect();
        let last: String = chars[chars.len() - 4..].iter().collect();
        format!("{first}...{last}")
    } else {
        "****".to_string()
    }
}

/// Mask every secret-looking env value so a config can be shown on a screen
/// share or pasted into a chat without leaking the key.
pub fn redact_secrets(value: &serde_json::Value) -> serde_json::Value {
    let mut out = value.clone();
    if let Some(env) = out.get_mut("env").and_then(|e| e.as_object_mut()) {
        for (name, entry) in env.iter_mut() {
            if !(name.contains("TOKEN") || name.contains("KEY")) {
                continue;
            }
            if let Some(secret) = entry.as_str() {
                *entry = serde_json::Value::String(mask_key(secret));
            }
        }
    }
    out
}

pub fn print_json_pretty(value: &serde_json::Value) {
    match colored_json::to_colored_json(value, colored_json::ColorMode::Auto(colored_json::Output::StdOut)) {
        Ok(colored) => println!("{colored}"),
        Err(_) => {
            if let Ok(json_str) = serde_json::to_string_pretty(value) {
                println!("{json_str}");
            }
        }
    }
}
