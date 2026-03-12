use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::error::{CliError, Result};
use crate::templates::registry::templates_cache_dir;
use crate::templates::types::{RegistryEntry, RegistryFile};

const REPO_OWNER: &str = "ExoMonk";
const REPO_NAME: &str = "evm-cloud";

/// Look up a template by name in the registry.
pub(crate) fn resolve_template<'a>(
    name: &str,
    registry: &'a RegistryFile,
) -> Result<&'a RegistryEntry> {
    registry
        .templates
        .iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| {
            let available: Vec<String> = registry.templates.iter().map(|e| e.name.clone()).collect();
            CliError::TemplateNotFound {
                name: name.to_string(),
                available,
            }
        })
}

/// Ensure the template package is downloaded and cached locally.
/// Returns the path to the cached template directory.
///
/// Resolution order:
/// 1. Local cache (if exists and not forced)
/// 2. Local repo `templates/` dir (if running from within the repo)
/// 3. GitHub tarball download
pub(crate) fn fetch_template_package(entry: &RegistryEntry, force: bool) -> Result<PathBuf> {
    let cache_name = format!("{}-{}", entry.name, entry.version);
    let cache_path = templates_cache_dir().join(&cache_name);

    // If cached and not forced, return existing
    if cache_path.join("template.toml").exists() && !force {
        return Ok(cache_path);
    }

    // Try local repo templates/ directory first (for dev / pre-push usage)
    if let Some(local) = try_local_repo_template(entry) {
        return Ok(local);
    }

    // Download tarball from GitHub, extract template subdir
    let temp_root = std::env::temp_dir().join(format!(
        "evm-cloud-template-fetch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| CliError::SystemClock(err.to_string()))?
            .as_nanos()
    ));

    fs::create_dir_all(&temp_root).map_err(|source| CliError::Io {
        source,
        path: temp_root.clone(),
    })?;

    let archive_path = temp_root.join("repo.tar.gz");

    // Try tag first, then main branch
    let urls = remote_archive_urls();
    let mut last_error: Option<String> = None;
    let mut downloaded = false;

    for url in &urls {
        match download_archive(url, &archive_path) {
            Ok(()) => {
                downloaded = true;
                break;
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
    }

    if !downloaded {
        let _ = fs::remove_dir_all(&temp_root);
        return Err(CliError::RegistryFetchError {
            details: format!(
                "failed to download template package: {}",
                last_error.unwrap_or_else(|| "unknown error".to_string())
            ),
        });
    }

    // Extract archive
    let extract_root = temp_root.join("extract");
    fs::create_dir_all(&extract_root).map_err(|source| CliError::Io {
        source,
        path: extract_root.clone(),
    })?;

    extract_archive(&archive_path, &extract_root)?;

    // Find repo root in extracted archive (GitHub wraps in <repo>-<ref>/)
    let repo_root = find_extracted_repo_root(&extract_root)?;

    let template_src = repo_root.join("templates").join(&entry.path);
    if !template_src.is_dir() {
        let _ = fs::remove_dir_all(&temp_root);
        return Err(CliError::TemplateNotFound {
            name: entry.name.clone(),
            available: vec![],
        });
    }

    // Copy template dir to cache
    fs::create_dir_all(&cache_path).map_err(|source| CliError::Io {
        source,
        path: cache_path.clone(),
    })?;

    copy_dir_recursive(&template_src, &cache_path)?;

    // Clean up temp dir
    let _ = fs::remove_dir_all(&temp_root);

    Ok(cache_path)
}

/// Try to find the template in the local repo's `templates/` directory.
/// This works when the CLI is run from within the evm-cloud repo (dev workflow).
fn try_local_repo_template(entry: &RegistryEntry) -> Option<PathBuf> {
    // Walk up from CWD looking for a `templates/<entry.path>/template.toml`
    let cwd = std::env::current_dir().ok()?;
    let mut dir = cwd.as_path();
    loop {
        let candidate = dir.join("templates").join(&entry.path);
        if candidate.join("template.toml").exists() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

fn remote_archive_urls() -> Vec<String> {
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    vec![
        format!(
            "https://codeload.github.com/{}/{}/tar.gz/refs/tags/{}",
            REPO_OWNER, REPO_NAME, tag
        ),
        format!(
            "https://codeload.github.com/{}/{}/tar.gz/refs/heads/main",
            REPO_OWNER, REPO_NAME
        ),
    ]
}

fn download_archive(url: &str, destination: &PathBuf) -> Result<()> {
    let mut cmd = Command::new("curl");
    cmd.arg("-fLsS").arg(url).arg("-o").arg(destination);

    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        cmd.arg("-H")
            .arg(format!("Authorization: Bearer {token}"));
    }

    let output = cmd.output().map_err(|source| CliError::Io {
        source,
        path: PathBuf::from("curl"),
    })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(CliError::RegistryFetchError {
        details: format!("download from {url}: {}", stderr.trim()),
    })
}

fn extract_archive(archive_path: &PathBuf, extract_root: &PathBuf) -> Result<()> {
    let output = Command::new("tar")
        .arg("-xzf")
        .arg(archive_path)
        .arg("-C")
        .arg(extract_root)
        .output()
        .map_err(|source| CliError::Io {
            source,
            path: PathBuf::from("tar"),
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(CliError::RegistryFetchError {
        details: format!(
            "extract {}: {}",
            archive_path.display(),
            stderr.trim()
        ),
    })
}

fn find_extracted_repo_root(extract_root: &PathBuf) -> Result<PathBuf> {
    let mut children: Vec<PathBuf> = fs::read_dir(extract_root)
        .map_err(|source| CliError::Io {
            source,
            path: extract_root.clone(),
        })?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    children.sort();

    children.into_iter().next().ok_or_else(|| {
        CliError::RegistryFetchError {
            details: "downloaded archive is missing repository root directory".to_string(),
        }
    })
}

fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> Result<()> {
    for entry in fs::read_dir(src).map_err(|source| CliError::Io {
        source,
        path: src.clone(),
    })? {
        let entry = entry.map_err(|source| CliError::Io {
            source,
            path: src.clone(),
        })?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            fs::create_dir_all(&dst_path).map_err(|source| CliError::Io {
                source,
                path: dst_path.clone(),
            })?;
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|source| CliError::Io {
                source,
                path: src_path.clone(),
            })?;
        }
    }
    Ok(())
}
