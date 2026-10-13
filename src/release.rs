//! Which ccc release is the newest, and whether this build is behind it.
//!
//! `ccc update` asks GitHub every time. The start-up screens (`ccc`, `ccc ui`,
//! `ccc version`) only want to say "a new version is out", which is not worth
//! a round trip on every launch: they reuse the last answer for a while and
//! wait only briefly for a fresh one.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::{ccc_home, read_json, write_json, REPO, VERSION};

/// How long one look at GitHub serves the start-up notice.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// The most start-up waits for GitHub. A missed notice costs nothing; a
/// prompt that hangs on a dead network costs every launch.
const NOTICE_TIMEOUT: Duration = Duration::from_secs(2);
/// Set to anything but "" or "0" and ccc stops looking for releases on its own.
pub const ENV_NO_UPDATE_CHECK: &str = "CCC_NO_UPDATE_CHECK";

/// The newest release's tag, e.g. "v0.1.10".
///
/// github.com/<repo>/releases/latest answers with a redirect to
/// …/releases/tag/<tag>, so the tag can be read without a body or the API.
/// api.github.com allows 60 unauthenticated requests an hour per IP address;
/// an office behind one address used that up, and every `ccc update` then
/// failed with 403 although a new release was out.
pub fn latest_tag(timeout: Duration) -> Result<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(timeout))
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
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect() };
    parts(latest) > parts(current)
}

// ── The start-up notice ──

/// What the last look at GitHub found, kept in ~/.ccc/update-check.json.
#[derive(Serialize, Deserialize, Default)]
struct LastCheck {
    /// Seconds since the Unix epoch.
    #[serde(default)]
    checked_at: u64,
    /// Newest release without the "v", if any look has ever succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest: Option<String>,
}

fn last_check_path() -> Result<PathBuf> {
    Ok(ccc_home()?.join("update-check.json"))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A look from the future means the clock moved back; take a fresh one.
fn is_fresh(checked_at: u64, now: u64) -> bool {
    checked_at <= now && now - checked_at < CHECK_EVERY.as_secs()
}

fn load() -> LastCheck {
    last_check_path()
        .ok()
        .filter(|p| p.exists())
        .and_then(|p| read_json(&p).ok())
        .and_then(|json| serde_json::from_value(json).ok())
        .unwrap_or_default()
}

/// Record what GitHub just said, so the next start-up can skip asking.
pub fn remember(latest: Option<&str>) {
    save(&LastCheck { checked_at: now(), latest: latest.map(str::to_string) });
}

fn save(check: &LastCheck) {
    // Best effort: a notice that cannot be cached is just looked up again.
    if let Ok(path) = last_check_path()
        && path.parent().is_some_and(|dir| dir.exists())
        && let Ok(value) = serde_json::to_value(check)
    {
        let _ = write_json(&path, &value);
    }
}

static NOTICE: OnceLock<Option<String>> = OnceLock::new();
/// Set once `ccc update` has replaced the binary: the version this process
/// would announce is already installed, it just is not the one running.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// A release newer than this build, if there is one. Looks at most once per
/// process, and reuses the last answer across runs for `CHECK_EVERY`.
pub fn available_update() -> Option<String> {
    NOTICE.get_or_init(look_up);
    known_update().map(str::to_string)
}

/// What `available_update` already found, without looking: for screens that
/// redraw many times a second and must never wait on the network.
pub fn known_update() -> Option<&'static str> {
    if INSTALLED.load(Ordering::Relaxed) {
        return None;
    }
    NOTICE.get().and_then(|v| v.as_deref())
}

/// Called after `ccc update` installed a release.
pub fn mark_installed() {
    INSTALLED.store(true, Ordering::Relaxed);
}

fn look_up() -> Option<String> {
    let opted_out = std::env::var(ENV_NO_UPDATE_CHECK).is_ok_and(|v| !v.is_empty() && v != "0");
    if opted_out {
        return None;
    }

    let last = load();
    let latest = if is_fresh(last.checked_at, now()) {
        last.latest
    } else {
        let fetched = latest_tag(NOTICE_TIMEOUT).ok().map(|tag| tag.trim_start_matches('v').to_string());
        // A failed look is recorded too, keeping the last good answer: offline,
        // every launch would otherwise sit out the whole timeout.
        let latest = fetched.or(last.latest);
        save(&LastCheck { checked_at: now(), latest: latest.clone() });
        latest
    };
    latest.filter(|latest| is_newer(latest, VERSION))
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

    #[test]
    fn a_recent_look_is_reused_and_an_old_or_future_one_is_not() {
        let now = 1_000_000;
        assert!(is_fresh(now, now));
        assert!(is_fresh(now - 60, now));
        assert!(!is_fresh(now - CHECK_EVERY.as_secs(), now));
        assert!(!is_fresh(0, now));
        assert!(!is_fresh(now + 60, now));
    }

    #[test]
    fn last_check_reads_old_or_partial_files() {
        let check: LastCheck = serde_json::from_str("{}").unwrap();
        assert_eq!(check.checked_at, 0);
        assert!(check.latest.is_none());
        let check: LastCheck = serde_json::from_str(r#"{"checked_at":5,"latest":"0.2.0"}"#).unwrap();
        assert_eq!(check.latest.as_deref(), Some("0.2.0"));
    }
}
