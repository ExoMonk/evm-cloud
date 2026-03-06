use std::fs;
use std::process::{Command, Stdio};

use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};

const CLUSTER_NAME: &str = "evm-cloud-local";

pub(crate) fn cluster_exists() -> Result<bool> {
    let output = run_cmd("kind", &["get", "clusters"])?;
    Ok(output.lines().any(|l| l.trim() == CLUSTER_NAME))
}

pub(crate) fn cluster_reachable() -> Result<bool> {
    let context = format!("kind-{CLUSTER_NAME}");
    let status = Command::new("kubectl")
        .args(["cluster-info", "--context", &context])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| CliError::ToolFailed {
            tool: "kubectl".into(),
            details: e.to_string(),
        })?;
    Ok(status.success())
}

fn create_cluster(config_yaml: &str, color: ColorMode) -> Result<()> {
    let tmp =
        std::env::temp_dir().join(format!("evm-cloud-kind-config-{}.yaml", std::process::id()));
    fs::write(&tmp, config_yaml).map_err(|source| CliError::Io {
        source,
        path: tmp.clone(),
    })?;

    let tmp_path = tmp.clone();
    let result: Result<()> = output::with_spinner("Clustering", color, || {
        let status = Command::new("kind")
            .args([
                "create",
                "cluster",
                "--name",
                CLUSTER_NAME,
                "--config",
                &tmp_path.to_string_lossy(),
                "--wait",
                "60s",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| CliError::ToolFailed {
                tool: "kind".into(),
                details: e.to_string(),
            })?;

        if !status.success() {
            return Err(CliError::ToolFailed {
                tool: "kind".into(),
                details: "cluster creation failed".into(),
            });
        }
        Ok(())
    });

    let _ = fs::remove_file(&tmp);
    result?;

    set_context()?;
    Ok(())
}

pub(crate) fn delete_cluster(color: ColorMode) -> Result<()> {
    if !cluster_exists()? {
        output::warn(
            &format!("No {CLUSTER_NAME} cluster found. Nothing to do."),
            color,
        );
        return Ok(());
    }

    output::with_spinner("Removing cluster", color, || {
        let status = Command::new("kind")
            .args(["delete", "cluster", "--name", CLUSTER_NAME])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| CliError::ToolFailed {
                tool: "kind".into(),
                details: e.to_string(),
            })?;

        if !status.success() {
            return Err(CliError::ToolFailed {
                tool: "kind".into(),
                details: "cluster deletion failed".into(),
            });
        }
        Ok(())
    })
}

pub(crate) fn ensure_cluster(config_yaml: &str, force: bool, color: ColorMode) -> Result<()> {
    let exists = cluster_exists()?;

    if exists && force {
        delete_cluster(color)?;
    } else if exists {
        if !cluster_reachable()? {
            return Err(CliError::ClusterUnreachable {
                name: CLUSTER_NAME.into(),
            });
        }
        set_context()?;
        return Ok(());
    }

    create_cluster(config_yaml, color)
}

pub(crate) fn name() -> &'static str {
    CLUSTER_NAME
}

fn set_context() -> Result<()> {
    let context = format!("kind-{CLUSTER_NAME}");
    let status = Command::new("kubectl")
        .args(["config", "use-context", &context])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| CliError::ToolFailed {
            tool: "kubectl".into(),
            details: e.to_string(),
        })?;

    if !status.success() {
        return Err(CliError::ToolFailed {
            tool: "kubectl".into(),
            details: format!("failed to set context {context}"),
        });
    }
    Ok(())
}

fn run_cmd(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| CliError::ToolFailed {
            tool: program.into(),
            details: e.to_string(),
        })?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
