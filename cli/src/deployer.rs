use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

use crate::error::{CliError, Result};
use crate::handoff::WorkloadHandoff;
use crate::output::ColorMode;

include!(concat!(env!("OUT_DIR"), "/script_checksums.rs"));

const K3S_DEPLOY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../deployers/k3s/deploy.sh"));
const K3S_TEARDOWN: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../deployers/k3s/teardown.sh"));
const K3S_RENDER_VALUES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../deployers/k3s/scripts/render-values.sh"));
const EKS_POPULATE_VALUES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../deployers/eks/scripts/populate-values-from-config-bundle.sh"));
const COMPOSE_DEPLOY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../deployers/compose/deploy.sh"));

#[derive(Debug, Clone, Copy)]
pub(crate) enum Action {
    Deploy,
    Teardown,
}

#[derive(Debug)]
pub(crate) struct DeployLockGuard {
    path: PathBuf,
}

impl DeployLockGuard {
    pub(crate) fn acquire(root: &Path) -> Result<Self> {
        let path = root.join(".evm-cloud-deploy.lock");
        let created = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);

        match created {
            Ok(_) => Ok(Self { path }),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(CliError::DeployLockBusy { path })
            }
            Err(source) => Err(CliError::Io {
                source,
                path,
            }),
        }
    }
}

impl Drop for DeployLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(crate) struct InvokeOptions<'a> {
    pub(crate) passthrough_args: &'a [String],
    pub(crate) quiet_output: bool,
    pub(crate) color: ColorMode,
    pub(crate) compute_engine: String,
}

pub(crate) fn invoke_deployer(handoff: &WorkloadHandoff, action: Action, options: InvokeOptions<'_>) -> Result<()> {
    let temp = TempWorkspace::new()?;
    let scripts = extract_scripts(temp.path())?;
    let handoff_path = write_handoff_file(temp.path(), handoff)?;

    let mut default_args = vec![handoff_path.display().to_string()];
    let script_path = match (handoff.compute_engine.as_str(), action) {
        ("k3s", Action::Deploy) => scripts.k3s_deploy.clone(),
        ("k3s", Action::Teardown) => scripts.k3s_teardown.clone(),
        ("ec2", Action::Deploy) | ("docker_compose", Action::Deploy) => scripts.compose_deploy.clone(),
        ("ec2", Action::Teardown) | ("docker_compose", Action::Teardown) => {
            default_args.push("--teardown".to_string());
            scripts.compose_deploy.clone()
        }
        ("eks", _) => {
            return Err(CliError::DeployerUnsupportedEngine {
                compute_engine: "eks".to_string(),
            })
        }
        (other, _) => {
            return Err(CliError::DeployerUnsupportedEngine {
                compute_engine: other.to_string(),
            })
        }
    };

    let mut args = default_args;
    args.extend_from_slice(options.passthrough_args);

    let mut command = Command::new(script_path);
    command.args(args).stdin(Stdio::inherit());

    if options.quiet_output {
        // Pipe stdout so we can filter [evm-cloud] progress lines.
        // Merge stderr into stdout so we capture error output too.
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    } else {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    }

    apply_sanitized_env(&mut command);

    if !options.quiet_output {
        let output = command.output().map_err(|err| CliError::Other(err.into()))?;
        return exit_status_to_result(output.status);
    }

    // Quiet mode: stream [evm-cloud] prefixed lines in real-time,
    // mapping them to curated status lines.
    let mut child = command.spawn().map_err(|err| CliError::Other(err.into()))?;
    let engine = options.compute_engine.clone();
    let color = options.color;

    let stdout = child.stdout.take();
    if let Some(stdout) = stdout {
        let reader = BufReader::new(stdout);
        let mut rindexer_idx = 0u32;
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            let Some(msg) = line.strip_prefix("[evm-cloud] ") else {
                continue;
            };
            if let Some(formatted) = format_deploy_line(msg, &engine, color, &mut rindexer_idx) {
                eprintln!("{formatted}");
            }
        }
    }

    let output = child.wait_with_output().map_err(|err| CliError::Other(err.into()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            eprintln!("{}", stderr.trim());
        }
    }

    exit_status_to_result(output.status)
}

/// Paint text orange (ANSI yellow/33) when color is enabled.
fn orange(text: &str, mode: ColorMode) -> String {
    if matches!(mode, ColorMode::Never)
        || (matches!(mode, ColorMode::Auto) && !std::io::IsTerminal::is_terminal(&std::io::stderr()))
    {
        text.to_string()
    } else {
        format!("\x1b[33m{text}\x1b[0m")
    }
}

/// Map a raw `[evm-cloud] ...` message to a curated CLI status line.
/// Returns `None` for messages that should be suppressed.
fn format_deploy_line(msg: &str, engine: &str, color: ColorMode, rindexer_idx: &mut u32) -> Option<String> {
    let icon = match engine {
        "k3s" => "🛟",
        _ => "⛴️",
    };

    // --- k3s lines ---
    if msg == "Cluster reachable." {
        return Some(format!("     ✔ k3s cluster reachable"));
    }
    if msg == "ESO is ready." {
        return Some(format!("     ✔ ESO is ready"));
    }
    // ClusterSecretStore: <name> applied.
    if let Some(rest) = msg.strip_prefix("ClusterSecretStore ") {
        if let Some(name) = rest.strip_suffix(" applied.") {
            return Some(format!("     {icon} ClusterSecretStore: {}", orange(name, color)));
        }
    }
    if msg.starts_with("Cloudflare origin TLS secret created") {
        return Some(format!("     ✔ Cloudflare origin TLS secret created"));
    }
    // ingress-nginx
    if msg == "ingress-nginx installed." || msg == "ingress-nginx already present." {
        return Some(format!("     ✔ ingress-nginx"));
    }
    // cert-manager
    if msg == "cert-manager installed." || msg == "cert-manager CRDs already present." {
        return Some(format!("     ✔ cert-manager"));
    }
    // kube-prometheus-stack
    if msg == "kube-prometheus-stack installed." || msg == "kube-prometheus-stack already present." {
        return Some(format!("     ✔ kube-prometheus-stack"));
    }
    // Loki
    if msg == "Loki installed." || msg == "Loki already present." {
        return Some(format!("     ✔ Loki"));
    }
    // Promtail
    if msg == "Promtail installed." || msg == "Promtail already present." {
        return Some(format!("     ✔ Promtail"));
    }
    // Dashboards deployed.
    if msg == "Dashboards deployed." {
        return Some(format!("     {icon} Dashboards deployed"));
    }
    // Deploying eRPC (<name>)...
    if let Some(rest) = msg.strip_prefix("Deploying eRPC (") {
        if let Some(name) = rest.strip_suffix(")...") {
            return Some(format!("     {icon} eRPC: {}", orange(name, color)));
        }
    }
    if msg.starts_with("eRPC deployed.") {
        return None; // already showed the deploying line
    }
    // Deploying rindexer instance (<name>)...
    if let Some(rest) = msg.strip_prefix("Deploying rindexer instance (") {
        if let Some(name) = rest.strip_suffix(")...") {
            *rindexer_idx += 1;
            return Some(format!("     {icon} rindexer #{}: {}", rindexer_idx, orange(name, color)));
        }
    }
    // <name> deployed. (rindexer completion)
    if msg.ends_with(" deployed.") && !msg.starts_with("eRPC") {
        return None; // suppress rindexer completion echoes
    }
    if msg == "All workloads deployed successfully." {
        return None; // the CLI prints its own success banner
    }

    // --- compose/docker lines ---
    if msg == "SSH connectivity verified." {
        return Some(format!("     ✔ SSH connectivity verified"));
    }
    if msg == "Uploaded configs." {
        return Some(format!("     ✔ Configs uploaded"));
    }
    if msg == "Secrets pulled to .env" {
        return Some(format!("     ✔ Secrets pulled"));
    }
    if msg == "Restarting containers..." {
        return Some(format!("     {icon} Restarting containers..."));
    }
    if msg == "Verifying containers..." {
        return Some(format!("     {icon} Verifying containers..."));
    }
    if msg == "Deploy complete." {
        return None; // CLI prints its own success banner
    }

    // Suppress everything else (verbose helm output, waiting messages, etc.)
    None
}

fn exit_status_to_result(status: std::process::ExitStatus) -> Result<()> {
    if status.success() {
        return Ok(());
    }

    if let Some(code) = status.code() {
        return Err(CliError::DeployerFailed { code });
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        Err(CliError::DeployerSignaled {
            signal: status.signal(),
        })
    }

    #[cfg(not(unix))]
    {
        Err(CliError::DeployerSignaled { signal: None })
    }
}

struct ExtractedScripts {
    k3s_deploy: PathBuf,
    k3s_teardown: PathBuf,
    compose_deploy: PathBuf,
}

fn extract_scripts(tmp_dir: &Path) -> Result<ExtractedScripts> {
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

fn write_script(tmp_dir: &Path, relative_path: &str, contents: &str, expected_sha256: &str, script_name: &str) -> Result<PathBuf> {
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

fn write_handoff_file(tmp_dir: &Path, handoff: &WorkloadHandoff) -> Result<PathBuf> {
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

fn apply_sanitized_env(command: &mut Command) {
    command.env_clear();
    for key in [
        "PATH",
        "HOME",
        "USER",
        "TMPDIR",
        "SHELL",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "no_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "AWS_PROFILE",
        "AWS_REGION",
        "AWS_DEFAULT_REGION",
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        "KUBECONFIG",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
}

struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    fn new() -> Result<Self> {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("evm-cloud-deployer-{}-{}", std::process::id(), suffix));

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

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::DeployLockGuard;
    use crate::error::CliError;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "evm-cloud-cli-tests-{}-{}-{}",
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

    #[test]
    fn lock_guard_blocks_concurrent_acquisition() {
        let dir = temp_dir("deploy-lock");
        let first = DeployLockGuard::acquire(&dir).expect("first lock must succeed");
        let second = DeployLockGuard::acquire(&dir).expect_err("second lock must fail");

        match second {
            CliError::DeployLockBusy { .. } => {}
            other => panic!("unexpected error: {other}"),
        }

        drop(first);
        let third = DeployLockGuard::acquire(&dir);
        assert!(third.is_ok());
    }
}
