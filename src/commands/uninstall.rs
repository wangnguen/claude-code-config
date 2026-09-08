use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{ccc_home, keys_path, KeysStore};
use crate::ui;
use crate::utils::confirm;

/// The line install.sh appends above its PATH export.
const RC_MARKER: &str = "# ccc - Claude Code Config";
const RC_FILES: &[&str] = &[".zshrc", ".bashrc", ".profile"];

pub fn run() -> Result<()> {
    let home = ccc_home()?;
    if !home.exists() {
        println!("Nothing to remove: {} does not exist.", home.display());
        return Ok(());
    }

    println!();
    ui::print_header(&ui::ICON_DOC, "Uninstall ccc");
    ui::print_row("Folder", &home.display().to_string());
    ui::print_row("Binary", "ccc executable and .claude template");
    ui::print_row("Keys", &keys_summary());
    ui::print_row("PATH", "entry added by the installer");
    ui::print_separator();
    ui::print_row("Kept", "Claude Code itself");
    ui::print_row("Kept", ".claude/ inside your projects");
    ui::print_footer();
    println!();
    println!("Projects keep working: their .claude/settings.local.json still holds");
    println!("the key and Claude Code reads it directly, without ccc.");
    println!();

    if !confirm(&format!("Delete {} and its saved keys? (y/N): ", home.display())) {
        println!("Cancelled.");
        return Ok(());
    }

    remove_path_entry(&home);
    remove_home(&home)?;

    println!("Uninstalled ccc.");
    println!("Restart your terminal so the PATH change takes effect.");
    Ok(())
}

/// KeysStore::load() reports an unreadable keys.json as empty, so count alone
/// would promise "0 keys" while deleting a file full of them.
fn keys_summary() -> String {
    let count = KeysStore::load().keys.len();
    if count > 0 {
        return format!("{count} saved key(s) — deleted too");
    }
    match keys_path() {
        Ok(path) if path.exists() => "keys.json (unreadable) — deleted too".to_string(),
        _ => "none saved".to_string(),
    }
}

/// Undo the PATH edit the installer made: the user environment variable on
/// Windows, the exported line in a shell rc file elsewhere.
fn remove_path_entry(home: &Path) {
    if cfg!(target_os = "windows") {
        remove_windows_path_entry(home);
    } else {
        remove_unix_path_entry(home);
    }
}

fn remove_windows_path_entry(home: &Path) {
    // Edit the registry value rather than [Environment]::SetEnvironmentVariable:
    // that API returns PATH already expanded, so writing it back would freeze
    // entries like %JAVA_HOME%\bin into whatever they happen to point at today.
    // Reading raw and keeping the value kind leaves the rest of PATH untouched.
    //
    // The filter drops nothing but our own entry — empty segments from a stray
    // ';' stay, so an untouched PATH compares equal and is never rewritten.
    //
    // Single quotes are the only PowerShell delimiter that takes the path
    // literally; doubling one escapes it inside the literal.
    let dir = home.display().to_string().replace('\'', "''");
    let script = format!(
        "$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true); \
         if (-not $key) {{ exit 2 }}; \
         $raw = $key.GetValue('Path', '', 'DoNotExpandEnvironmentNames'); \
         if (-not $raw) {{ exit 2 }}; \
         $dir = '{dir}'.TrimEnd('\\'); \
         $new = (($raw -split ';') | Where-Object {{ $_.TrimEnd('\\') -ne $dir }}) -join ';'; \
         if ($new -eq $raw) {{ exit 3 }}; \
         $key.SetValue('Path', $new, $key.GetValueKind('Path')); \
         $key.Close()"
    );
    let code = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .ok()
        .and_then(|s| s.code());
    match code {
        Some(0) => println!("Removed {} from your user PATH.", home.display()),
        Some(3) => println!("No ccc entry in your user PATH, nothing to change."),
        _ => println!(
            "Could not edit PATH automatically — remove {} from it by hand.",
            home.display()
        ),
    }
}

fn remove_unix_path_entry(home: &Path) {
    let Some(user_home) = home.parent() else { return };
    let needle = home.display().to_string();

    for rc in RC_FILES {
        let path = user_home.join(rc);
        let Ok(content) = fs::read_to_string(&path) else { continue };

        let cleaned: Vec<&str> = content
            .lines()
            .filter(|line| !is_installer_line(line, &needle))
            .collect();
        if cleaned.len() == content.lines().count() {
            continue;
        }

        let mut text = cleaned.join("\n");
        text.push('\n');
        if fs::write(&path, text).is_ok() {
            println!("Removed the ccc PATH line from {}.", path.display());
        }
    }
}

/// Only the two lines install.sh writes, never an unrelated PATH edit.
fn is_installer_line(line: &str, ccc_home: &str) -> bool {
    let trimmed = line.trim();
    trimmed == RC_MARKER || (trimmed.starts_with("export PATH=") && trimmed.contains(ccc_home))
}

fn remove_home(home: &Path) -> Result<()> {
    match running_exe_inside(home) {
        // Windows locks a running executable: everything around it can go now,
        // the file itself only once this process has exited.
        Some(exe) if cfg!(target_os = "windows") => {
            for entry in fs::read_dir(home).context("Failed to read ~/.ccc")?.flatten() {
                let path = entry.path();
                // exe is canonical (\\?\C:\...), so compare like with like.
                if path.canonicalize().ok().as_deref() == Some(exe.as_path()) {
                    continue;
                }
                if path.is_dir() {
                    fs::remove_dir_all(&path).ok();
                } else {
                    fs::remove_file(&path).ok();
                }
            }
            schedule_windows_cleanup(home);
            println!("Removed {} (the running ccc.exe follows in a moment).", home.display());
        }
        _ => {
            fs::remove_dir_all(home)
                .with_context(|| format!("Failed to delete {}", home.display()))?;
            println!("Removed {}.", home.display());
        }
    }
    Ok(())
}

/// The running binary's path when it lives inside the folder being deleted —
/// `ccc` built from source and run from target/ is not affected.
fn running_exe_inside(home: &Path) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let home = home.canonicalize().ok()?;
    exe.starts_with(&home).then_some(exe)
}

fn schedule_windows_cleanup(home: &Path) {
    // One arg per token, never a single pre-quoted string: Rust escapes inner
    // quotes as \" for the C runtime, which cmd.exe does not understand. Passed
    // separately, the path is the only arg Rust has to quote, and it quotes it
    // the way cmd expects.
    //
    // ping is the delay here because timeout needs a console of its own.
    Command::new("cmd")
        .args(["/C", "ping", "127.0.0.1", "-n", "3", ">nul", "&", "rmdir", "/s", "/q"])
        .arg(home)
        .spawn()
        .ok();
}
