use anyhow::{anyhow, bail, Context, Result};
use std::fs;
use std::time::Duration;

use crate::config::{ccc_home, REPO, VERSION};
use crate::utils::{confirm, copy_dir_recursive};

pub fn run() -> Result<()> {
    println!("Current version: {VERSION}");
    println!("Checking for updates...");

    let latest_tag = latest_tag()?;
    let latest_version = latest_tag.trim_start_matches('v');

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

    println!("Updated to {latest_version}!");
    println!("Restart your terminal to use the new version.");
    Ok(())
}

/// The newest release's tag, e.g. "v0.1.10".
///
/// github.com/<repo>/releases/latest answers with a redirect to
/// …/releases/tag/<tag>, so the tag can be read without a body or the API.
/// api.github.com allows 60 unauthenticated requests an hour per IP address;
/// an office behind one address used that up, and every `ccc update` then
/// failed with 403 although a new release was out.
fn latest_tag() -> Result<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    let url = format!("https://github.com/{REPO}/releases/latest");
    let resp = agent
        .get(&url)
        .header("User-Agent", "ccc")
        .call()
        .map_err(|e| anyhow!("Failed to check for updates: {e}"))?;
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    tag_from_redirect(resp.status().as_u16(), location)
}

fn tag_from_redirect(status: u16, location: &str) -> Result<String> {
    if !(300..400).contains(&status) {
        bail!("Failed to check for updates: GitHub answered HTTP {status}.");
    }
    match location.rsplit_once("/releases/tag/") {
        Some((_, tag)) if !tag.is_empty() => Ok(tag.to_string()),
        // Without any release, "latest" points at the release list itself.
        _ => bail!("No release has been published yet."),
    }
}

/// True when `latest` is a higher version than `current`, part by part.
fn is_newer(latest: &str, current: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect() };
    parts(latest) > parts(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_tag_from_the_redirect() {
        let location = "https://github.com/wangnguen/claude-code-config/releases/tag/v0.1.10";
        assert_eq!(tag_from_redirect(302, location).unwrap(), "v0.1.10");
    }

    #[test]
    fn no_release_or_no_redirect_is_an_error() {
        assert!(tag_from_redirect(302, "https://github.com/x/y/releases").is_err());
        assert!(tag_from_redirect(200, "").is_err());
        assert!(tag_from_redirect(429, "").is_err());
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.9", "0.1.10"));
        assert!(!is_newer("0.1.10", "0.1.10"));
        assert!(is_newer("0.2.0", "0.1.99"));
    }
}
