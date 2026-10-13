use anyhow::{anyhow, bail, Context, Result};
use std::fs;
use std::time::Duration;

use crate::config::{ccc_home, REPO, VERSION};
use crate::release::{self, is_newer};
use crate::utils::{confirm, copy_dir_recursive};

pub fn run() -> Result<()> {
    println!("Current version: {VERSION}");
    println!("Checking for updates...");

    let latest_tag = release::latest_tag(Duration::from_secs(20))?;
    let latest_version = latest_tag.trim_start_matches('v');
    // Saves the start-up screens their own look for a while.
    release::remember(Some(latest_version));

    if !is_newer(latest_version, VERSION) {
        if latest_version == VERSION {
            println!("Already up to date!");
        } else {
            println!("This build ({VERSION}) is newer than the latest release ({latest_version}).");
        }
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
    // A plain release download, not an API call: it redirects to GitHub's
    // asset storage and is not counted against the API's rate limit.
    let download_url = format!("https://github.com/{REPO}/releases/download/{latest_tag}/{asset_name}");

    println!("Downloading {asset_name}...");

    let tmp_zip = std::env::temp_dir().join("ccc-update.zip");
    let tmp_dir = std::env::temp_dir().join("ccc-update");

    // Download zip
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(300)))
        .build()
        .into();
    let mut resp = agent
        .get(&download_url)
        .header("User-Agent", "ccc")
        .call()
        .map_err(|e| anyhow!("Download failed: {e}"))?;
    match resp.status().as_u16() {
        200 => {
            let bytes = resp
                .body_mut()
                .with_config()
                .limit(200 * 1024 * 1024)
                .read_to_vec()
                .context("Failed to download")?;
            fs::write(&tmp_zip, &bytes).context("Failed to save zip")?;
        }
        404 => bail!("Asset {asset_name} not found in release {latest_tag}."),
        status => bail!("Download failed: HTTP {status}"),
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

    release::mark_installed();
    println!("Updated to {latest_version}!");
    println!("Restart your terminal to use the new version.");
    Ok(())
}

