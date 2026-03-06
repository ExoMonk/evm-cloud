use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use base64::Engine;

use crate::config::schema::ComputeEngine;
use crate::error::{CliError, Result};
use crate::handoff::WorkloadHandoff;

/// Generate a kubeconfig file from handoff data.
///
/// For K3s: decodes base64 kubeconfig and writes to `target_path`.
/// For EKS: runs `aws eks update-kubeconfig`.
/// Returns an error for unsupported compute engines.
pub(crate) fn generate_from_handoff(
    handoff: &WorkloadHandoff,
    project_root: &Path,
    target_path: &Path,
) -> Result<()> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::Io {
            source,
            path: parent.to_path_buf(),
        })?;
    }

    match handoff.compute_engine {
        ComputeEngine::K3s => write_k3s_kubeconfig(handoff, target_path),
        ComputeEngine::Eks => write_eks_kubeconfig(handoff, project_root, target_path),
        other => Err(CliError::KubeconfigUnsupportedEngine {
            compute_engine: other.to_string(),
        }),
    }
}

/// Preferred kubeconfig search paths for a given directory.
pub(crate) fn candidates(dir: &Path) -> Vec<std::path::PathBuf> {
    let is_evm_cloud_workdir = dir.file_name().and_then(|name| name.to_str()) == Some(".evm-cloud");

    if is_evm_cloud_workdir {
        if let Some(parent) = dir.parent() {
            return vec![
                parent.join("kubeconfig.yaml"),
                dir.join("kubeconfig.yaml"),
                parent.join(".evm-cloud").join("kubeconfig.yaml"),
            ];
        }

        return vec![dir.join("kubeconfig.yaml")];
    }

    vec![
        dir.join("kubeconfig.yaml"),
        dir.join(".evm-cloud").join("kubeconfig.yaml"),
    ]
}

/// Resolve an existing kubeconfig or generate one from handoff.
pub(crate) fn resolve_or_generate(
    handoff: &WorkloadHandoff,
    project_root: &Path,
    terraform_dir: &Path,
    explicit: Option<std::path::PathBuf>,
) -> Result<std::path::PathBuf> {
    if let Some(path) = explicit {
        let target = absolutize(terraform_dir, path);
        if target.is_file() {
            return Ok(target);
        }
        generate_from_handoff(handoff, project_root, &target)?;
        return ensure_exists(target);
    }

    let search = candidates(terraform_dir);
    let preferred = search
        .first()
        .cloned()
        .unwrap_or_else(|| terraform_dir.join("kubeconfig.yaml"));

    generate_from_handoff(handoff, project_root, &preferred)?;
    ensure_exists(preferred)
}

pub(crate) fn absolutize(base_dir: &Path, candidate: std::path::PathBuf) -> std::path::PathBuf {
    if candidate.is_absolute() {
        candidate
    } else {
        base_dir.join(candidate)
    }
}

fn ensure_exists(path: std::path::PathBuf) -> Result<std::path::PathBuf> {
    if path.is_file() {
        return Ok(path);
    }
    Err(CliError::KubeconfigNotFound { path })
}

fn write_k3s_kubeconfig(handoff: &WorkloadHandoff, target_path: &Path) -> Result<()> {
    let encoded = handoff
        .runtime
        .k3s
        .as_ref()
        .and_then(|runtime| runtime.kubeconfig_base64.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::HandoffInvalid {
            field: "runtime.k3s.kubeconfig_base64".to_string(),
            details: "missing; cannot generate kubeconfig".to_string(),
        })?;

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| CliError::HandoffInvalid {
            field: "runtime.k3s.kubeconfig_base64".to_string(),
            details: format!("invalid base64 payload: {err}"),
        })?;

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(target_path)
            .map_err(|source| CliError::Io {
                source,
                path: target_path.to_path_buf(),
            })?;
        f.write_all(&decoded).map_err(|source| CliError::Io {
            source,
            path: target_path.to_path_buf(),
        })?;
    }
    #[cfg(not(unix))]
    {
        fs::write(target_path, decoded).map_err(|source| CliError::Io {
            source,
            path: target_path.to_path_buf(),
        })?;
    }

    Ok(())
}

fn write_eks_kubeconfig(
    handoff: &WorkloadHandoff,
    project_root: &Path,
    target_path: &Path,
) -> Result<()> {
    let cluster_name = handoff
        .runtime
        .eks
        .as_ref()
        .and_then(|runtime| runtime.cluster_name.as_ref())
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| CliError::HandoffInvalid {
            field: "runtime.eks.cluster_name".to_string(),
            details: "missing; cannot generate kubeconfig".to_string(),
        })?;

    let aws = which::which("aws").map_err(|_| CliError::PrerequisiteNotFound {
        tool: "aws".to_string(),
    })?;

    let mut command = Command::new(aws);
    command
        .args([
            "eks",
            "update-kubeconfig",
            "--name",
            cluster_name,
            "--kubeconfig",
        ])
        .arg(target_path)
        .current_dir(project_root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(region) = handoff
        .extra
        .get("aws_region")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        command.args(["--region", region]);
    }

    let status = command.status().map_err(|err| CliError::ToolFailed {
        tool: "aws eks update-kubeconfig".to_string(),
        details: err.to_string(),
    })?;

    crate::error::tool_exit_status(status, "aws eks update-kubeconfig")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(target_path, fs::Permissions::from_mode(0o600)).map_err(|source| {
            CliError::Io {
                source,
                path: target_path.to_path_buf(),
            }
        })?;
    }

    Ok(())
}
