use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{CliError, Result};

#[derive(Debug)]
pub(crate) enum ProjectKind {
    EasyToml,
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

    let has_toml = has_evm_cloud_toml(&canonical);
    let has_explicit_root = has_explicit_tf_root(&canonical);
    let marker_mode = read_mode_marker(&canonical)?;

    if let Some(mode) = marker_mode {
        return match mode {
            ModeMarker::Easy => {
                if !has_toml {
                    Err(CliError::InvalidModeMarker {
                        path: canonical.join(".evm-cloud/mode"),
                        value: "easy".to_string(),
                    })
                } else {
                    Ok(PreflightResult {
                        project_kind: ProjectKind::EasyToml,
                        resolved_root: canonical,
                    })
                }
            }
            ModeMarker::Power => {
                if !has_any_tf_files(&canonical)? {
                    Err(CliError::InvalidModeMarker {
                        path: canonical.join(".evm-cloud/mode"),
                        value: "power".to_string(),
                    })
                } else {
                    Ok(PreflightResult {
                        project_kind: ProjectKind::RawTerraform,
                        resolved_root: canonical,
                    })
                }
            }
        };
    }

    if has_toml && has_explicit_root {
        return Err(CliError::ModeRoutingAmbiguous {
            dir: canonical.display().to_string(),
            remediation: "create .evm-cloud/mode with `easy` or `power`".to_string(),
        });
    }

    if has_explicit_root {
        return Ok(PreflightResult {
            project_kind: ProjectKind::RawTerraform,
            resolved_root: canonical,
        });
    }

    if has_toml {
        return Ok(PreflightResult {
            project_kind: ProjectKind::EasyToml,
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

#[derive(Debug, Clone, Copy)]
enum ModeMarker {
    Easy,
    Power,
}

fn read_mode_marker(path: &Path) -> Result<Option<ModeMarker>> {
    let marker_path = path.join(".evm-cloud/mode");
    if !marker_path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&marker_path).map_err(|source| CliError::Io {
        source,
        path: marker_path.clone(),
    })?;

    let value = raw.trim();
    match value {
        "easy" => Ok(Some(ModeMarker::Easy)),
        "power" => Ok(Some(ModeMarker::Power)),
        other => Err(CliError::InvalidModeMarker {
            path: marker_path,
            value: other.to_string(),
        }),
    }
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{run_checks, ProjectKind};
    use crate::terraform::REQUIRED_VERSION_CONSTRAINT;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "evm-cloud-preflight-tests-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    fn write(path: &Path, content: &str) {
        fs::write(path, content).expect("write file");
    }

    #[test]
    fn marker_power_routes_to_raw_terraform() {
        let dir = temp_dir("marker-power");
        fs::create_dir_all(dir.join(".evm-cloud")).expect("create marker dir");
        write(&dir.join(".evm-cloud/mode"), "power\n");
        write(&dir.join("main.tf"), "terraform {}\n");
        write(
            &dir.join("versions.tf"),
            &format!(
                "terraform {{ required_version = \"{}\" }}\n",
                REQUIRED_VERSION_CONSTRAINT
            ),
        );
        write(&dir.join("evm-cloud.toml"), "schema_version = 1\n");

        let result = run_checks(&dir, false).expect("preflight must succeed");
        assert!(matches!(result.project_kind, ProjectKind::RawTerraform));
    }

    #[test]
    fn marker_power_routes_with_non_explicit_tf_root() {
        let dir = temp_dir("marker-power-any-tf");
        fs::create_dir_all(dir.join(".evm-cloud")).expect("create marker dir");
        write(&dir.join(".evm-cloud/mode"), "power\n");
        write(&dir.join("provider.tf"), "terraform {}\n");

        let result = run_checks(&dir, false).expect("preflight must succeed");
        assert!(matches!(result.project_kind, ProjectKind::RawTerraform));
    }

    #[test]
    fn marker_easy_routes_to_toml() {
        let dir = temp_dir("marker-easy");
        fs::create_dir_all(dir.join(".evm-cloud")).expect("create marker dir");
        write(&dir.join(".evm-cloud/mode"), "easy\n");
        write(&dir.join("evm-cloud.toml"), "schema_version = 1\n");
        write(&dir.join("main.tf"), "terraform {}\n");
        write(
            &dir.join("versions.tf"),
            &format!(
                "terraform {{ required_version = \"{}\" }}\n",
                REQUIRED_VERSION_CONSTRAINT
            ),
        );

        let result = run_checks(&dir, false).expect("preflight must succeed");
        assert!(matches!(result.project_kind, ProjectKind::EasyToml));
    }

    #[test]
    fn no_marker_with_toml_and_root_is_ambiguous() {
        let dir = temp_dir("ambiguous");
        write(&dir.join("evm-cloud.toml"), "schema_version = 1\n");
        write(&dir.join("main.tf"), "terraform {}\n");
        write(
            &dir.join("versions.tf"),
            &format!(
                "terraform {{ required_version = \"{}\" }}\n",
                REQUIRED_VERSION_CONSTRAINT
            ),
        );

        let err = run_checks(&dir, false).expect_err("preflight must fail");
        assert!(err.to_string().contains("cannot determine project mode"));
    }
}
