//! Startup version check against npm registry.
//!
//! On startup, checks if a newer version of `@zhing2026/krew` is available
//! on npm. Results are cached for 24 hours in `~/.krew/version_check.toml`.
//! Network failures or parse errors are silently ignored.

use std::cmp::Ordering;
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// npm registry endpoint for the latest published version.
const NPM_LATEST_URL: &str = "https://registry.npmjs.org/@zhing2026/krew/latest";

/// Cache file name inside the user config directory (~/.krew/).
const CACHE_FILENAME: &str = "version_check.toml";

/// How long a cached result stays valid.
const CACHE_TTL_HOURS: i64 = 24;

/// Network request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Cached version check result.
#[derive(Debug, Serialize, Deserialize)]
struct VersionCache {
    latest_version: String,
    checked_at: DateTime<Utc>,
    /// The local version at the time of the check. Used to invalidate cache
    /// when the user upgrades, so a stale cooldown entry doesn't suppress
    /// update notifications for newer releases.
    #[serde(default)]
    current_version: String,
}

/// Check for a newer version and return a warning message if one is available.
///
/// Returns `None` when the check is disabled, the local version is up-to-date,
/// or any error occurs (network, parse, cache I/O).
pub async fn check_for_update(enabled: bool) -> Option<String> {
    if !enabled {
        return None;
    }

    let current = env!("CARGO_PKG_VERSION");
    let cache_path = cache_file_path()?;

    // Try reading cache first.
    // Invalidate cache if: missing, expired, or local version changed (user upgraded).
    let latest = if let Some(cached) = read_cache(&cache_path) {
        let age = Utc::now().signed_duration_since(cached.checked_at);
        let version_changed =
            !cached.current_version.is_empty() && cached.current_version != current;
        if age.num_hours() < CACHE_TTL_HOURS && !version_changed {
            cached.latest_version
        } else {
            fetch_and_cache(&cache_path, current).await
        }
    } else {
        fetch_and_cache(&cache_path, current).await
    };

    // Compare versions.
    if compare_versions(current, &latest)? == Ordering::Less {
        Some(format!(
            "New version v{latest} available (current: v{current}). \
             Run: npm update -g @zhing2026/krew"
        ))
    } else {
        None
    }
}

/// Fetch latest version from npm, write cache, and return the version string.
///
/// On failure, writes the current local version as cache (24h cooldown)
/// and returns it so no update warning is triggered.
async fn fetch_and_cache(cache_path: &PathBuf, current_version: &str) -> String {
    match fetch_latest_version().await {
        Some(version) => {
            write_cache(cache_path, &version, current_version);
            version
        }
        None => {
            // Write current version to trigger 24h cooldown.
            write_cache(cache_path, current_version, current_version);
            current_version.to_string()
        }
    }
}

/// Query npm registry for the latest version of `@zhing2026/krew`.
async fn fetch_latest_version() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .ok()?;

    let resp = client.get(NPM_LATEST_URL).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("version")?.as_str().map(|s| s.to_string())
}

/// Compare two version strings using lexicographic segment comparison.
///
/// Splits each version by `.`, parses segments as `u32`, and compares
/// left-to-right. The first unequal segment determines the result.
/// Missing segments are treated as `0`.
///
/// Returns `None` if any segment fails to parse as `u32`.
pub fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();
    let max_len = a_parts.len().max(b_parts.len());

    for i in 0..max_len {
        let a_num: u32 = a_parts.get(i).unwrap_or(&"0").parse().ok()?;
        let b_num: u32 = b_parts.get(i).unwrap_or(&"0").parse().ok()?;
        match a_num.cmp(&b_num) {
            Ordering::Equal => continue,
            ord => return Some(ord),
        }
    }
    Some(Ordering::Equal)
}

/// Return the cache file path: `~/.krew/version_check.toml`.
fn cache_file_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    Some(home.join(krew_config::USER_CONFIG_DIR).join(CACHE_FILENAME))
}

/// Read and deserialize the version cache file.
fn read_cache(path: &PathBuf) -> Option<VersionCache> {
    let content = std::fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}

/// Write the version cache file. Failures are silently ignored.
fn write_cache(path: &PathBuf, latest: &str, current: &str) {
    let cache = VersionCache {
        latest_version: latest.to_string(),
        checked_at: Utc::now(),
        current_version: current.to_string(),
    };
    if let Ok(content) = toml::to_string(&cache) {
        // Ensure the parent directory (~/.krew/) exists.
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions_minor_behind() {
        assert_eq!(compare_versions("0.9.0", "0.10.0"), Some(Ordering::Less));
    }

    #[test]
    fn test_compare_versions_patch_behind() {
        assert_eq!(compare_versions("1.2.3", "1.2.4"), Some(Ordering::Less));
    }

    #[test]
    fn test_compare_versions_equal() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Some(Ordering::Equal));
    }

    #[test]
    fn test_compare_versions_local_ahead() {
        assert_eq!(compare_versions("1.1.0", "1.0.0"), Some(Ordering::Greater));
    }

    #[test]
    fn test_compare_versions_major_behind_but_later_segments_larger() {
        // 1.10.0 vs 2.0.0 — first segment 1 < 2 decides immediately.
        assert_eq!(compare_versions("1.10.0", "2.0.0"), Some(Ordering::Less));
    }

    #[test]
    fn test_compare_versions_different_segment_count() {
        // 1.0 vs 1.0.1 — missing segment treated as 0.
        assert_eq!(compare_versions("1.0", "1.0.1"), Some(Ordering::Less));
    }

    #[test]
    fn test_compare_versions_different_segment_count_equal() {
        assert_eq!(compare_versions("1.0.0", "1.0"), Some(Ordering::Equal));
    }

    #[test]
    fn test_compare_versions_parse_failure() {
        // Pre-release tags cannot be parsed as u32.
        assert_eq!(compare_versions("1.0.0-beta", "1.0.0"), None);
        assert_eq!(compare_versions("1.0.0", "1.0.0-rc1"), None);
    }
}
