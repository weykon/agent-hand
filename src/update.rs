use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::session::Storage;

const REPO_API_LATEST: &str = "https://api.github.com/repos/weykon/agent-hand/releases/latest";
const CACHE_TTL_SECS: i64 = 60 * 60 * 24;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UpdateCache {
    last_checked_at: i64,
    latest_tag: Option<String>,
    has_update: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct LatestRelease {
    tag_name: String,
}

fn parse_semver_triplet(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let mut it = v.split('.');
    let major = it.next()?.parse::<u64>().ok()?;
    let minor = it.next()?.parse::<u64>().ok()?;
    let patch_part = it.next()?;
    let patch_digits = patch_part
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();
    let patch = patch_digits.parse::<u64>().ok()?;
    Some((major, minor, patch))
}

fn has_newer_version(current: &str, latest: &str) -> bool {
    let Some(cur) = parse_semver_triplet(current) else {
        return false;
    };
    let Some(lat) = parse_semver_triplet(latest) else {
        return false;
    };
    lat > cur
}

async fn cache_path() -> Result<std::path::PathBuf> {
    let dir = Storage::get_agent_hand_dir()?.join("cache");
    tokio::fs::create_dir_all(&dir).await?;
    Ok(dir.join("update.json"))
}

async fn load_cache() -> Option<UpdateCache> {
    let path = cache_path().await.ok()?;
    let content = tokio::fs::read_to_string(&path).await.ok()?;
    serde_json::from_str(&content).ok()
}

async fn save_cache(cache: &UpdateCache) {
    let path = match cache_path().await {
        Ok(p) => p,
        Err(_) => return,
    };
    let Ok(json) = serde_json::to_string(cache) else {
        return;
    };
    let _ = tokio::fs::write(path, json).await;
}

async fn fetch_latest_tag() -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| crate::Error::Other(e.to_string()))?;

    let resp = client
        .get(REPO_API_LATEST)
        .header("User-Agent", "agent-hand")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| crate::Error::Other(e.to_string()))?;

    let release: LatestRelease = resp
        .json()
        .await
        .map_err(|e| crate::Error::Other(e.to_string()))?;
    Ok(release.tag_name)
}

/// Returns a short statusline suffix when an update is available, e.g. "↑0.2.9 upgrade".
///
/// To avoid hammering the network (statusline runs every few seconds), we cache results for 24h.
pub async fn statusline_update_hint() -> Option<String> {
    let now = chrono::Utc::now().timestamp();

    if let Some(cache) = load_cache().await {
        if now.saturating_sub(cache.last_checked_at) < CACHE_TTL_SECS {
            if cache.has_update {
                if let Some(tag) = cache.latest_tag {
                    return Some(format!("↑{} upgrade", tag.trim_start_matches('v')));
                }
            }
            return None;
        }
    }

    let latest_tag = fetch_latest_tag().await.ok();
    let has_update = latest_tag
        .as_deref()
        .is_some_and(|t| has_newer_version(crate::VERSION, t));

    let cache = UpdateCache {
        last_checked_at: now,
        latest_tag: latest_tag.clone(),
        has_update,
    };
    save_cache(&cache).await;

    if has_update {
        let tag = latest_tag?;
        return Some(format!("↑{} upgrade", tag.trim_start_matches('v')));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_semver_triplet() {
        assert_eq!(parse_semver_triplet("0.2.7"), Some((0, 2, 7)));
        assert_eq!(parse_semver_triplet("v0.2.7"), Some((0, 2, 7)));
        assert_eq!(parse_semver_triplet("0.2.7-rc1"), Some((0, 2, 7)));
        assert_eq!(parse_semver_triplet("bad"), None);
    }

    #[test]
    fn test_has_newer_version() {
        assert!(has_newer_version("0.2.7", "0.2.8"));
        assert!(!has_newer_version("0.2.8", "0.2.8"));
        assert!(!has_newer_version("0.3.0", "0.2.99"));
    }
}
