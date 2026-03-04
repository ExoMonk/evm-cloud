use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{CliError, Result};
use crate::output::ColorMode;

use super::manifests;
use super::profiles::ResourceSet;

pub(crate) fn deploy_clickhouse(
    persist: bool,
    res: &ResourceSet,
    _color: ColorMode,
) -> Result<()> {
    let manifest = manifests::clickhouse_manifest(persist, res);
    kubectl_apply_stdin(&manifest)?;
    kubectl_wait("app=clickhouse", "condition=Ready", 120)?;
    Ok(())
}

pub(crate) fn deploy_anvil(
    fork_url: Option<&str>,
    res: &ResourceSet,
    _color: ColorMode,
) -> Result<()> {
    let chart_dir = extract_chart_to_temp("charts/anvil")?;
    let chart_path = chart_dir.path.join("charts/anvil");

    let values_yaml = format!(
        r#"resources:
  requests:
    cpu: {cpu_req}
    memory: {mem_req}
  limits:
    cpu: {cpu_lim}
    memory: {mem_lim}
"#,
        cpu_req = res.cpu_req,
        mem_req = res.mem_req,
        cpu_lim = res.cpu_lim,
        mem_lim = res.mem_lim,
    );
    let values_path = chart_dir.path.join("anvil-values.yaml");
    fs::write(&values_path, &values_yaml).map_err(|source| CliError::Io {
        source,
        path: values_path.clone(),
    })?;

    let mut extra_args = Vec::new();
    if let Some(url) = fork_url {
        extra_args.push("--set".to_string());
        extra_args.push(format!("anvil.forkUrl={url}"));
    }

    helm_upgrade_install(
        "local-anvil",
        &chart_path,
        &values_path,
        &extra_args,
        120,
    )?;

    Ok(())
}

pub(crate) fn deploy_erpc(values_yaml: &str, _color: ColorMode) -> Result<()> {
    let chart_dir = extract_chart_to_temp("charts/rpc-proxy")?;
    let chart_path = chart_dir.path.join("charts/rpc-proxy");

    let values_path = chart_dir.path.join("erpc-values.yaml");
    fs::write(&values_path, values_yaml).map_err(|source| CliError::Io {
        source,
        path: values_path.clone(),
    })?;

    helm_upgrade_install("local-erpc", &chart_path, &values_path, &[], 120)?;
    Ok(())
}

pub(crate) fn deploy_rindexer(values_yaml: &str, _color: ColorMode) -> Result<()> {
    let chart_dir = extract_chart_to_temp("charts/indexer")?;
    let chart_path = chart_dir.path.join("charts/indexer");

    let values_path = chart_dir.path.join("indexer-values.yaml");
    fs::write(&values_path, values_yaml).map_err(|source| CliError::Io {
        source,
        path: values_path.clone(),
    })?;

    helm_upgrade_install("local-indexer", &chart_path, &values_path, &[], 180)?;
    Ok(())
}

// CHART_ASSETS is used; SHA256 constants are only used by deployer.rs
#[allow(dead_code)]
mod chart_data {
    // Re-export what we need
    include!(concat!(env!("OUT_DIR"), "/script_checksums.rs"));

    pub(super) fn assets() -> &'static [(&'static str, &'static str)] {
        CHART_ASSETS
    }
}

struct TempChartDir {
    path: std::path::PathBuf,
}

impl Drop for TempChartDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn extract_chart_to_temp(chart_prefix: &str) -> Result<TempChartDir> {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let tmp = std::env::temp_dir().join(format!(
        "evm-cloud-local-{}-{}",
        std::process::id(),
        suffix
    ));

    for (relative_path, contents) in chart_data::assets() {
        if !relative_path.starts_with(chart_prefix) {
            continue;
        }
        let path = tmp.join(relative_path);
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

    Ok(TempChartDir { path: tmp })
}

fn helm_upgrade_install(
    release: &str,
    chart_path: &Path,
    values_path: &Path,
    extra_args: &[String],
    timeout_secs: u32,
) -> Result<()> {
    let mut cmd = Command::new("helm");
    cmd.args([
        "upgrade",
        "--install",
        release,
        &chart_path.to_string_lossy(),
        "-f",
        &values_path.to_string_lossy(),
        "--wait",
        "--timeout",
        &format!("{timeout_secs}s"),
    ]);
    for arg in extra_args {
        cmd.arg(arg);
    }
    cmd.stdout(Stdio::null()).stderr(Stdio::null());

    let status = cmd.status().map_err(|e| CliError::ToolFailed {
        tool: "helm".into(),
        details: e.to_string(),
    })?;

    if !status.success() {
        return Err(CliError::ToolFailed {
            tool: "helm".into(),
            details: format!("{release} deployment failed"),
        });
    }
    Ok(())
}

fn kubectl_apply_stdin(manifest: &str) -> Result<()> {
    let mut child = Command::new("kubectl")
        .args(["apply", "-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| CliError::ToolFailed {
            tool: "kubectl".into(),
            details: e.to_string(),
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(manifest.as_bytes()).map_err(|e| CliError::ToolFailed {
            tool: "kubectl".into(),
            details: e.to_string(),
        })?;
    }

    let status = child.wait().map_err(|e| CliError::ToolFailed {
        tool: "kubectl".into(),
        details: e.to_string(),
    })?;

    if !status.success() {
        return Err(CliError::ToolFailed {
            tool: "kubectl".into(),
            details: "apply failed".into(),
        });
    }
    Ok(())
}

fn kubectl_wait(selector: &str, condition: &str, timeout_secs: u32) -> Result<()> {
    let status = Command::new("kubectl")
        .args([
            "wait",
            "--for",
            condition,
            "pod",
            "-l",
            selector,
            "--timeout",
            &format!("{timeout_secs}s"),
        ])
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
            details: format!("wait for {selector} {condition} timed out"),
        });
    }
    Ok(())
}
