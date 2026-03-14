//! In-app sound pack browser and installer.
//! Downloads packs from multiple GitHub registries (PeonPing/og-packs primary,
//! weykon/agent-hand-packs secondary).

use std::path::PathBuf;

/// A remote sound pack registry (GitHub repository).
struct Registry {
    /// GitHub API endpoint for the git tree listing.
    api_tree: &'static str,
    /// Raw content base URL (for downloading files).
    raw_base: &'static str,
    /// Path for `gh api` CLI call (no leading slash).
    gh_api_path: &'static str,
}

/// All known pack registries, in priority order.
/// PeonPing/og-packs is the primary community source.
const REGISTRIES: &[Registry] = &[
    Registry {
        api_tree: "https://api.github.com/repos/PeonPing/og-packs/git/trees/main",
        raw_base: "https://raw.githubusercontent.com/PeonPing/og-packs/main",
        gh_api_path: "repos/PeonPing/og-packs/git/trees/main",
    },
    Registry {
        api_tree: "https://api.github.com/repos/weykon/agent-hand-packs/git/trees/main",
        raw_base: "https://raw.githubusercontent.com/weykon/agent-hand-packs/main",
        gh_api_path: "repos/weykon/agent-hand-packs/git/trees/main",
    },
];

/// Metadata for a pack available in the registry.
#[derive(Debug, Clone)]
pub struct RegistryPack {
    /// Directory name in the repo (e.g. "ra2_soviet_engineer").
    pub name: String,
    /// Human-friendly display name from manifest (e.g. "Soviet Engineer").
    pub display_name: Option<String>,
    /// Whether it's already installed locally.
    pub installed: bool,
}

/// Fetch the list of available packs from all registries.
/// Priority: relay server `/api/packs` → `gh` CLI → direct GitHub API.
pub async fn fetch_pack_list(relay_url: Option<&str>) -> Result<Vec<RegistryPack>, String> {
    let pack_names = fetch_pack_names(relay_url).await?;

    let installed = super::SoundPack::list_installed();
    let mut packs: Vec<RegistryPack> = pack_names
        .into_iter()
        .map(|name| RegistryPack {
            installed: installed.contains(&name),
            display_name: None,
            name,
        })
        .collect();

    // Sort: uninstalled first, then alphabetical
    packs.sort_by(|a, b| {
        a.installed
            .cmp(&b.installed)
            .then(a.name.cmp(&b.name))
    });

    Ok(packs)
}

/// Fetch pack names from all registries, merging and deduplicating results.
async fn fetch_pack_names(relay_url: Option<&str>) -> Result<Vec<String>, String> {
    // 1. Try relay server for the primary registry (cached, no rate limit)
    if let Some(url) = relay_url {
        let packs_url = format!("{}/api/packs", url.trim_end_matches('/'));
        if let Ok(relay_names) = fetch_from_relay(&packs_url).await {
            // Relay covers primary registry; also fetch additional registries
            let mut all_names = relay_names;
            for registry in REGISTRIES.iter().skip(1) {
                if let Ok(extra) = fetch_tree_for_registry(registry).await {
                    all_names.extend(extra);
                }
            }
            all_names.sort();
            all_names.dedup();
            return Ok(all_names);
        }
    }

    // 2. Fall back to fetching from all registries directly
    let mut all_names: Vec<String> = Vec::new();
    let mut last_err = String::from("No registries available");

    for registry in REGISTRIES {
        match fetch_tree_for_registry(registry).await {
            Ok(names) => all_names.extend(names),
            Err(e) => last_err = e,
        }
    }

    if all_names.is_empty() {
        return Err(last_err);
    }

    all_names.sort();
    all_names.dedup();
    Ok(all_names)
}

/// Fetch pack names from a single registry.
async fn fetch_tree_for_registry(registry: &Registry) -> Result<Vec<String>, String> {
    let tree_json = fetch_tree_json_for(registry).await?;
    let tree = tree_json["tree"]
        .as_array()
        .ok_or("Invalid registry format")?;

    Ok(tree
        .iter()
        .filter(|item| item["type"].as_str() == Some("tree"))
        .filter_map(|item| item["path"].as_str().map(String::from))
        .collect())
}

/// Fetch pack list from the relay server's cached `/api/packs` endpoint.
async fn fetch_from_relay(url: &str) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent("agent-hand/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Relay fetch failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Relay returned {}", resp.status()));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Relay parse error: {}", e))?;

    json["packs"]
        .as_array()
        .ok_or_else(|| "Invalid relay response format".to_string())?
        .iter()
        .map(|v| {
            v.as_str()
                .map(String::from)
                .ok_or_else(|| "Non-string pack name".to_string())
        })
        .collect()
}

/// Try `gh` CLI first (has auth token = 5000 req/hr), fall back to raw HTTP (60 req/hr).
async fn fetch_tree_json_for(registry: &Registry) -> Result<serde_json::Value, String> {
    // Try gh CLI (authenticated)
    let gh_result = tokio::process::Command::new("gh")
        .args(["api", registry.gh_api_path])
        .output()
        .await;

    if let Ok(output) = gh_result {
        if output.status.success() {
            if let Ok(json) = serde_json::from_slice(&output.stdout) {
                return Ok(json);
            }
        }
    }

    // Fallback: direct HTTP request
    let client = reqwest::Client::builder()
        .user_agent("agent-hand/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let resp = client
        .get(registry.api_tree)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch registry: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        if status.as_u16() == 403 {
            return Err("GitHub rate limit reached. Install `gh` CLI (brew install gh) and run `gh auth login` for higher limits.".to_string());
        }
        return Err(format!("GitHub API returned {}", status));
    }

    resp.json()
        .await
        .map_err(|e| format!("Failed to parse registry: {}", e))
}

/// Install a pack by downloading its manifest and sound files.
/// Tries each registry in order until the pack is found.
/// Returns the local install path on success.
pub async fn install_pack(
    pack_name: &str,
    on_progress: impl Fn(&str),
) -> Result<PathBuf, String> {
    let client = reqwest::Client::builder()
        .user_agent("agent-hand")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let install_dir = dirs::home_dir()
        .ok_or("Cannot determine home directory")?
        .join(".openpeon/packs")
        .join(pack_name);

    // Create directories
    std::fs::create_dir_all(install_dir.join("sounds"))
        .map_err(|e| format!("Failed to create pack directory: {}", e))?;

    // 1. Download manifest — try each registry until one succeeds
    on_progress("Downloading manifest...");
    let (manifest_bytes, raw_base) =
        try_download_manifest_from_registries(&client, pack_name).await?;
    std::fs::write(install_dir.join("openpeon.json"), &manifest_bytes)
        .map_err(|e| format!("Failed to write manifest: {}", e))?;

    // 2. Parse manifest to get sound file list
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| format!("Invalid manifest: {}", e))?;

    let mut sound_files: Vec<String> = Vec::new();
    if let Some(categories) = manifest["categories"].as_object() {
        for (_cat, entry) in categories {
            if let Some(sounds) = entry["sounds"].as_array() {
                for sound in sounds {
                    if let Some(file) = sound["file"].as_str() {
                        if !sound_files.contains(&file.to_string()) {
                            sound_files.push(file.to_string());
                        }
                    }
                }
            }
        }
    }

    // 3. Download each sound file from the same registry
    let total = sound_files.len();
    for (i, file_path) in sound_files.iter().enumerate() {
        let file_name = file_path.rsplit('/').next().unwrap_or(file_path);
        on_progress(&format!("Downloading {}/{}: {}", i + 1, total, file_name));

        let url = format!("{}/{}/{}", raw_base, pack_name, file_path);
        let data = download_file(&client, &url).await?;

        let local_path = install_dir.join(file_path);
        if let Some(parent) = local_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&local_path, &data)
            .map_err(|e| format!("Failed to write {}: {}", file_name, e))?;
    }

    on_progress("Done!");
    Ok(install_dir)
}

/// Try to download a pack's manifest from each registry in order.
/// Returns the manifest bytes and the `raw_base` URL of the registry that served it.
async fn try_download_manifest_from_registries(
    client: &reqwest::Client,
    pack_name: &str,
) -> Result<(Vec<u8>, &'static str), String> {
    let mut last_err = String::new();
    for registry in REGISTRIES {
        let url = format!("{}/{}/openpeon.json", registry.raw_base, pack_name);
        match download_file(client, &url).await {
            Ok(bytes) => return Ok((bytes, registry.raw_base)),
            Err(e) => last_err = e,
        }
    }
    Err(format!(
        "Pack '{}' not found in any registry: {}",
        pack_name, last_err
    ))
}

async fn download_file(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<u8>, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} for {}", resp.status(), url));
    }

    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("Read failed: {}", e))
}
