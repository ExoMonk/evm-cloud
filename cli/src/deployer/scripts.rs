use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{CliError, Result};
use crate::handoff::WorkloadHandoff;

include!(concat!(env!("OUT_DIR"), "/script_checksums.rs"));

const K3S_DEPLOY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../deployers/k3s/deploy.sh"
));
const K3S_TEARDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../deployers/k3s/teardown.sh"
));
const K3S_RENDER_VALUES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../deployers/k3s/scripts/render-values.sh"
));
const EKS_POPULATE_VALUES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../deployers/eks/scripts/populate-values-from-config-bundle.sh"
));
const COMPOSE_DEPLOY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../deployers/compose/deploy.sh"
));

pub(super) struct ExtractedScripts {
    pub(super) k3s_deploy: PathBuf,
    pub(super) k3s_teardown: PathBuf,
    pub(super) compose_deploy: PathBuf,
}

pub(super) fn extract_scripts(tmp_dir: &Path) -> Result<ExtractedScripts> {
    let k3s_deploy = write_script(
        tmp_dir,
        "k3s/deploy.sh",
        K3S_DEPLOY,
        K3S_DEPLOY_SHA256,
        "deployers/k3s/deploy.sh",
    )?;
    let k3s_teardown = write_script(
        tmp_dir,
        "k3s/teardown.sh",
        K3S_TEARDOWN,
        K3S_TEARDOWN_SHA256,
        "deployers/k3s/teardown.sh",
    )?;
    let _k3s_render_values = write_script(
        tmp_dir,
        "k3s/scripts/render-values.sh",
        K3S_RENDER_VALUES,
        K3S_RENDER_VALUES_SHA256,
        "deployers/k3s/scripts/render-values.sh",
    )?;
    let _eks_populate_values = write_script(
        tmp_dir,
        "eks/scripts/populate-values-from-config-bundle.sh",
        EKS_POPULATE_VALUES,
        EKS_POPULATE_VALUES_SHA256,
        "deployers/eks/scripts/populate-values-from-config-bundle.sh",
    )?;
    let compose_deploy = write_script(
        tmp_dir,
        "compose/deploy.sh",
        COMPOSE_DEPLOY,
        COMPOSE_DEPLOY_SHA256,
        "deployers/compose/deploy.sh",
    )?;

    extract_chart_assets(tmp_dir)?;

    Ok(ExtractedScripts {
        k3s_deploy,
        k3s_teardown,
        compose_deploy,
    })
}

fn extract_chart_assets(tmp_dir: &Path) -> Result<()> {
    for (relative_path, contents) in CHART_ASSETS {
        let path = tmp_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| CliError::Io {
                source,
                path: parent.to_path_buf(),
            })?;
        }

        fs::write(&path, contents).map_err(|source| CliError::Io {
            source,
            path: path.clone(),
        })?;
    }

    Ok(())
}

fn write_script(
    tmp_dir: &Path,
    relative_path: &str,
    contents: &str,
    expected_sha256: &str,
    script_name: &str,
) -> Result<PathBuf> {
    if !contents.starts_with("#!") {
        return Err(CliError::HandoffInvalid {
            field: script_name.to_string(),
            details: "bundled script missing shebang".to_string(),
        });
    }

    verify_checksum(contents, expected_sha256, script_name)?;

    let path = tmp_dir.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::BundledScriptWrite {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(&path, contents).map_err(|source| CliError::BundledScriptWrite {
        path: path.clone(),
        source,
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .map_err(|source| CliError::BundledScriptWrite {
                path: path.clone(),
                source,
            })?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).map_err(|source| CliError::BundledScriptWrite {
            path: path.clone(),
            source,
        })?;
    }

    Ok(path)
}

fn verify_checksum(contents: &str, expected_sha256: &str, script_name: &str) -> Result<()> {
    let digest = Sha256::digest(contents.as_bytes());
    let found = format!("{digest:x}");
    if found == expected_sha256 {
        return Ok(());
    }

    Err(CliError::BundledScriptChecksumMismatch {
        script: script_name.to_string(),
    })
}

pub(super) fn write_handoff_file(tmp_dir: &Path, handoff: &WorkloadHandoff) -> Result<PathBuf> {
    let path = tmp_dir.join("workload_handoff.json");
    let bytes = serde_json::to_vec_pretty(handoff).map_err(CliError::OutputParseError)?;
    fs::write(&path, bytes).map_err(|source| CliError::Io {
        source,
        path: path.clone(),
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .map_err(|source| CliError::Io {
                source,
                path: path.clone(),
            })?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms).map_err(|source| CliError::Io {
            source,
            path: path.clone(),
        })?;
    }

    Ok(path)
}

pub(super) struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    pub(super) fn new() -> Result<Self> {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "evm-cloud-deployer-{}-{}",
            std::process::id(),
            suffix
        ));

        fs::create_dir_all(&path).map_err(|source| CliError::Io {
            source,
            path: path.clone(),
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)
                .map_err(|source| CliError::Io {
                    source,
                    path: path.clone(),
                })?
                .permissions();
            perms.set_mode(0o700);
            fs::set_permissions(&path, perms).map_err(|source| CliError::Io {
                source,
                path: path.clone(),
            })?;
        }

        Ok(Self { path })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_dir_all(&self.path) {
            eprintln!(
                "warning: failed to clean up temp workspace {}: {err}",
                self.path.display()
            );
        }
    }
}
