use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

use crate::error::{CliError, Result};
use crate::templates::types::RegistryFile;

const REGISTRY_REPO_OWNER: &str = "ExoMonk";
const REGISTRY_REPO_NAME: &str = "evm-cloud";
const REGISTRY_FILE_PATH: &str = "templates/registry.toml";
const CACHE_TTL_SECS: u64 = 3600; // 1 hour

/// Bundled fallback registry snapshot compiled into the binary from templates/registry.toml.
const BUNDLED_REGISTRY: &str = include_str!("../../../templates/registry.toml");

/// Fetch (or return cached) the template registry.
///
/// - If a fresh cache exists (< 1 hour) and `force_refresh` is false, returns cached.
/// - Otherwise fetches from GitHub (or `registry_url` if provided).
/// - On network failure, falls back to bundled snapshot.
pub(crate) fn fetch_registry(
    force_refresh: bool,
    registry_url: Option<&str>,
) -> Result<RegistryFile> {
    let cache_path = cache_dir().join("registry.toml");

    if !force_refresh {
        if let Some(cached) = try_read_cached(&cache_path) {
            return Ok(cached);
        }
    }

    // Try remote fetch
    match fetch_remote_registry(&cache_path, registry_url) {
        Ok(registry) => Ok(registry),
        Err(_) => {
            // Fall back to cached (even if stale)
            if let Ok(content) = fs::read_to_string(&cache_path) {
                if let Ok(registry) = parse_registry(&content) {
                    return Ok(registry);
                }
            }
            // Fall back to bundled snapshot
            parse_registry(BUNDLED_REGISTRY)
        }
    }
}

fn cache_dir() -> PathBuf {
    dirs_cache_root().join("templates")
}

fn dirs_cache_root() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".evm-cloud")
    } else {
        PathBuf::from("/tmp").join("evm-cloud")
    }
}

/// Read from cache if file exists and is within TTL.
fn try_read_cached(cache_path: &PathBuf) -> Option<RegistryFile> {
    let metadata = fs::metadata(cache_path).ok()?;
    let modified = metadata.modified().ok()?;
    let elapsed = SystemTime::now().duration_since(modified).ok()?;
    if elapsed.as_secs() > CACHE_TTL_SECS {
        return None;
    }
    let content = fs::read_to_string(cache_path).ok()?;
    parse_registry(&content).ok()
}

fn fetch_remote_registry(cache_path: &PathBuf, registry_url: Option<&str>) -> Result<RegistryFile> {
    let url = match registry_url {
        Some(u) => u.to_string(),
        None => format!(
            "https://raw.githubusercontent.com/{}/{}/main/{}",
            REGISTRY_REPO_OWNER, REGISTRY_REPO_NAME, REGISTRY_FILE_PATH,
        ),
    };

    let mut cmd = Command::new("curl");
    cmd.arg("-fLsS").arg(&url);

    // Use GITHUB_TOKEN if available for higher rate limits
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        cmd.arg("-H")
            .arg(format!("Authorization: Bearer {token}"));
    }

    let output = cmd.output().map_err(|source| CliError::Io {
        source,
        path: PathBuf::from("curl"),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::RegistryFetchError {
            details: format!("fetch from {url}: {}", stderr.trim()),
        });
    }

    let content = String::from_utf8_lossy(&output.stdout).to_string();
    let registry = parse_registry(&content)?;

    // Write to cache
    if let Some(parent) = cache_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(cache_path, &content);

    Ok(registry)
}

fn parse_registry(content: &str) -> Result<RegistryFile> {
    toml::from_str(content).map_err(|e| CliError::RegistryFetchError {
        details: format!("failed to parse registry: {e}"),
    })
}

pub(crate) fn templates_cache_dir() -> PathBuf {
    cache_dir().join("cache")
}
