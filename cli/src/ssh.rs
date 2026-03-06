use std::path::PathBuf;
use std::process::Command;

use crate::config::schema::ComputeEngine;
use crate::error::{CliError, Result};
use crate::handoff::WorkloadHandoff;
use crate::post_deploy::non_empty;
use crate::preflight::ProjectKind;

pub(crate) struct SshContext {
    pub host: String,
    pub user: String,
    pub key_path: Option<PathBuf>,
    pub port: u16,
}

/// SSH override values from CLI flags.
pub(crate) struct SshOverrides {
    pub key: Option<PathBuf>,
    pub user: Option<String>,
    pub port: Option<u16>,
}

/// Resolve SSH connection context from handoff + tfvars + CLI overrides.
pub(crate) fn resolve(
    handoff: &WorkloadHandoff,
    project_root: &std::path::Path,
    project_kind: &ProjectKind,
    overrides: SshOverrides,
) -> Result<SshContext> {
    let host = resolve_host(handoff)?;
    let default_user = crate::post_deploy::ssh_user_for(handoff.compute_engine).to_string();

    let tfvars_candidates = match project_kind {
        ProjectKind::EasyToml => vec![
            project_root.join(".evm-cloud").join("secrets.auto.tfvars"),
            project_root.join("secrets.auto.tfvars"),
            project_root.join(".evm-cloud").join("terraform.tfvars"),
        ],
        ProjectKind::RawTerraform => vec![
            project_root.join("secrets.auto.tfvars"),
            project_root.join("terraform.tfvars"),
        ],
    };

    let vars = crate::tfvars_parser::parse_all_existing(&tfvars_candidates)?;

    let key_path = overrides
        .key
        .or_else(|| vars.get("ssh_private_key_path").map(PathBuf::from));

    let user = overrides
        .user
        .or_else(|| vars.get("bare_metal_ssh_user").cloned())
        .unwrap_or(default_user);

    let port = overrides.port.unwrap_or_else(|| {
        vars.get("bare_metal_ssh_port")
            .and_then(|v| v.parse().ok())
            .unwrap_or(22)
    });

    Ok(SshContext {
        host,
        user,
        key_path,
        port,
    })
}

/// Execute a command over SSH and capture stdout.
pub(crate) fn exec(ctx: &SshContext, command: &str, timeout_secs: u32) -> Result<String> {
    let mut cmd = Command::new("ssh");
    cmd.args([
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "UserKnownHostsFile=/dev/null",
        "-o",
        "BatchMode=yes",
        "-o",
        &format!("ConnectTimeout={timeout_secs}"),
    ]);

    if let Some(key) = &ctx.key_path {
        cmd.arg("-i").arg(key);
    }

    if ctx.port != 22 {
        cmd.arg("-p").arg(ctx.port.to_string());
    }

    cmd.arg(format!("{}@{}", ctx.user, ctx.host));
    cmd.arg(command);

    let output = cmd.output().map_err(|err| CliError::ToolFailed {
        tool: "ssh".to_string(),
        details: err.to_string(),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::ToolFailed {
            tool: "ssh".to_string(),
            details: stderr.trim().to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Build a display-friendly SSH command string.
pub(crate) fn command_string(ctx: &SshContext) -> String {
    let mut parts = vec!["ssh".to_string()];

    if let Some(key) = &ctx.key_path {
        parts.push(format!("-i {}", key.display()));
    }

    if ctx.port != 22 {
        parts.push(format!("-p {}", ctx.port));
    }

    parts.push(format!("{}@{}", ctx.user, ctx.host));
    parts.join(" ")
}

fn resolve_host(handoff: &WorkloadHandoff) -> Result<String> {
    match handoff.compute_engine {
        ComputeEngine::Ec2 => handoff
            .runtime
            .ec2
            .as_ref()
            .and_then(|rt| non_empty(rt.public_ip.as_deref())),
        ComputeEngine::DockerCompose => handoff
            .runtime
            .ec2
            .as_ref()
            .and_then(|rt| non_empty(rt.public_ip.as_deref()))
            .or_else(|| {
                handoff
                    .runtime
                    .bare_metal
                    .as_ref()
                    .and_then(|rt| non_empty(rt.host_address.as_deref()))
            }),
        ComputeEngine::K3s => handoff
            .runtime
            .k3s
            .as_ref()
            .and_then(|rt| non_empty(rt.host_ip.as_deref())),
        ComputeEngine::Eks => {
            return Err(CliError::ToolFailed {
                tool: "ssh".to_string(),
                details: "EKS does not use SSH for status probes".to_string(),
            });
        }
    }
    .ok_or_else(|| CliError::HandoffInvalid {
        field: "runtime".to_string(),
        details: "no host address found in handoff for SSH connection".to_string(),
    })
}
