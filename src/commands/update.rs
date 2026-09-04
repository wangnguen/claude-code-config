use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::fs;

use crate::config::{ccc_home, REPO, VERSION};
use crate::utils::{confirm, copy_dir_recursive};

pub fn run() -> Result<()> {
    println!("Current version: {VERSION}");
    println!("Checking for updates...");

    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body: Value = match ureq::get(&url)
        .header("User-Agent", "ccc")
        .call()
    {
        Ok(mut resp) => {
            let text = resp.body_mut().read_to_string().unwrap_or_default();
            serde_json::from_str(&text).unwrap_or_default()
        }
        Err(e) => bail!("Failed to check for updates: {e}"),
    };

    let latest_tag = body["tag_name"].as_str().unwrap_or("v0.0.0");
    let latest_version = latest_tag.trim_start_matches('v');

    if latest_version == VERSION {
        println!("Already up to date!");
        return Ok(());
    }

    println!("New version available: {latest_version}");
    if !confirm("Update now? (y/N): ") {
        println!("Cancelled.");
        return Ok(());
    }

    let target = if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    } else {
        "x86_64-unknown-linux-gnu"
    };

    let asset_name = format!("ccc-{target}.zip");
    let download_url = body["assets"]
        .as_array()
        .and_then(|assets: &Vec<Value>| {
            assets.iter().find_map(|a| {
                if a["name"].as_str() == Some(&asset_name) {
                    a["browser_download_url"].as_str().map(String::from)
                } else {
                    None
                }
            })
        });

    let download_url = match download_url {
        Some(url) => url,
        None => bail!("Asset {asset_name} not found in release."),
    };

    println!("Downloading {asset_name}...");

    let tmp_zip = std::env::temp_dir().join("ccc-update.zip");
    let tmp_dir = std::env::temp_dir().join("ccc-update");

    // Download zip
    match ureq::get(&download_url).call() {
        Ok(mut resp) => {
            let bytes = resp.body_mut().read_to_vec().context("Failed to download")?;
            fs::write(&tmp_zip, &bytes).context("Failed to save zip")?;
        }
        Err(e) => bail!("Download failed: {e}"),
    }

    // Extract zip
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir).ok();
    }

    let zip_file = fs::File::open(&tmp_zip).context("Failed to open zip")?;
    let mut archive = zip::ZipArchive::new(zip_file).context("Failed to read zip")?;
    archive.extract(&tmp_dir).context("Failed to extract zip")?;

    // Find extracted folder and copy to ccc_home
    let ccc_home = ccc_home()?;

    // On Windows, rename running exe before overwriting
    if cfg!(target_os = "windows") {
        let exe_path = ccc_home.join("ccc.exe");
        let old_exe = ccc_home.join("ccc.old.exe");
        if exe_path.exists() {
            let _ = fs::remove_file(&old_exe);
            fs::rename(&exe_path, &old_exe).ok();
        }
    }

    if let Ok(entries) = fs::read_dir(&tmp_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                copy_dir_recursive(&entry.path(), &ccc_home)
                    .context("Failed to copy updated files")?;
                break;
            }
        }
    }

    // Cleanup
    fs::remove_file(&tmp_zip).ok();
    fs::remove_dir_all(&tmp_dir).ok();

    println!("Updated to {latest_version}!");
    println!("Restart your terminal to use the new version.");
    Ok(())
}
