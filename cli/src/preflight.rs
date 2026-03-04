use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{CliError, Result};

#[derive(Debug)]
pub(crate) enum ProjectKind {
    EvmCloudToml,
    RawTerraform,
}

#[derive(Debug)]
pub(crate) struct PreflightResult {
    pub(crate) project_kind: ProjectKind,
    pub(crate) resolved_root: PathBuf,
}

pub(crate) fn run_checks(dir: &Path, allow_raw_terraform: bool) -> Result<PreflightResult> {
    let canonical = fs::canonicalize(dir).map_err(|source| CliError::Io {
        source,
        path: dir.to_path_buf(),
    })?;

    if has_evm_cloud_toml(&canonical) {
        return Ok(PreflightResult {
            project_kind: ProjectKind::EvmCloudToml,
            resolved_root: canonical,
        });
    }

    if has_explicit_tf_root(&canonical) {
        return Ok(PreflightResult {
            project_kind: ProjectKind::RawTerraform,
            resolved_root: canonical,
        });
    }

    let sibling_candidates = detect_child_roots(&canonical)?;

    if has_any_tf_files(&canonical)? {
        if !allow_raw_terraform {
            return Err(CliError::RawTerraformOptInRequired {
                dir: canonical.display().to_string(),
            });
        }

        if !sibling_candidates.is_empty() {
            let mut candidates = vec![canonical.display().to_string()];
            candidates.extend(sibling_candidates);
            return Err(CliError::AmbiguousProjectRoot {
                dir: canonical.display().to_string(),
                candidates,
            });
        }

        return Ok(PreflightResult {
            project_kind: ProjectKind::RawTerraform,
            resolved_root: canonical,
        });
    }

    Err(CliError::NoProjectDetected {
        dir: canonical.display().to_string(),
    })
}

fn has_evm_cloud_toml(path: &Path) -> bool {
    path.join("evm-cloud.toml").is_file()
}

fn has_explicit_tf_root(path: &Path) -> bool {
    path.join("main.tf").is_file() && path.join("versions.tf").is_file()
}

fn has_any_tf_files(path: &Path) -> Result<bool> {
    let entries = fs::read_dir(path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| CliError::Io {
            source,
            path: path.to_path_buf(),
        })?;

        let candidate = entry.path();
        if candidate.is_file()
            && candidate
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("tf"))
                .unwrap_or(false)
        {
            return Ok(true);
        }
    }

    Ok(false)
}

fn detect_child_roots(path: &Path) -> Result<Vec<String>> {
    let mut candidates = Vec::new();
    let entries = fs::read_dir(path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| CliError::Io {
            source,
            path: path.to_path_buf(),
        })?;
        let child = entry.path();
        if !child.is_dir() {
            continue;
        }

        if has_evm_cloud_toml(&child) || has_explicit_tf_root(&child) {
            candidates.push(child.display().to_string());
        }
    }

    Ok(candidates)
}
