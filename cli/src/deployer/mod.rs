mod lock;
mod output;
mod scripts;

use std::io::BufRead;
use std::process::{Command, Stdio};

use crate::config::schema::ComputeEngine;
use crate::error::{CliError, Result};
use crate::handoff::WorkloadHandoff;
use crate::output::ColorMode;

pub(crate) use lock::DeployLockGuard;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Action {
    Deploy,
    #[allow(dead_code)]
    Teardown,
}

pub(crate) struct InvokeOptions<'a> {
    pub(crate) passthrough_args: &'a [String],
    pub(crate) quiet_output: bool,
    pub(crate) color: ColorMode,
    pub(crate) compute_engine: ComputeEngine,
    /// If set, the child process PID is written here after spawn.
    /// Used by the timeout wrapper to kill the child on timeout.
    pub(crate) child_pid: Option<std::sync::Arc<std::sync::atomic::AtomicU32>>,
}

pub(crate) fn invoke_deployer(
    handoff: &WorkloadHandoff,
    action: Action,
    options: InvokeOptions<'_>,
) -> Result<()> {
    let temp = scripts::TempWorkspace::new()?;
    let extracted = scripts::extract_scripts(temp.path())?;
    let handoff_path = scripts::write_handoff_file(temp.path(), handoff)?;

    let mut default_args = vec![handoff_path.display().to_string()];
    let script_path = match (handoff.compute_engine, action) {
        (ComputeEngine::K3s, Action::Deploy) => extracted.k3s_deploy.clone(),
        (ComputeEngine::K3s, Action::Teardown) => extracted.k3s_teardown.clone(),
        (ComputeEngine::Ec2, Action::Deploy) | (ComputeEngine::DockerCompose, Action::Deploy) => {
            extracted.compose_deploy.clone()
        }
        (ComputeEngine::Ec2, Action::Teardown)
        | (ComputeEngine::DockerCompose, Action::Teardown) => {
            default_args.push("--teardown".to_string());
            extracted.compose_deploy.clone()
        }
        (ComputeEngine::Eks, _) => {
            return Err(CliError::DeployerUnsupportedEngine {
                compute_engine: handoff.compute_engine.to_string(),
            })
        }
    };

    let mut args = default_args;
    args.extend_from_slice(options.passthrough_args);

    let mut command = Command::new(&script_path);
    command.args(args).stdin(Stdio::inherit());

    if options.quiet_output {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    } else {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    }

    apply_sanitized_env(&mut command);

    if !options.quiet_output {
        let cmd_output = command.output().map_err(|source| CliError::CommandSpawn {
            command: script_path.display().to_string(),
            source,
        })?;
        return exit_status_to_result(cmd_output.status);
    }

    // Quiet mode: stream [evm-cloud] prefixed lines in real-time,
    // mapping them to curated status lines.
    let mut child = command.spawn().map_err(|source| CliError::CommandSpawn {
        command: script_path.display().to_string(),
        source,
    })?;
    if let Some(ref pid_slot) = options.child_pid {
        pid_slot.store(child.id(), std::sync::atomic::Ordering::Relaxed);
    }
    let engine = options.compute_engine;
    let color = options.color;

    let stdout = child.stdout.take();
    if let Some(stdout) = stdout {
        let reader = std::io::BufReader::new(stdout);
        let mut rindexer_idx = 0u32;
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            let Some(msg) = line.strip_prefix("[evm-cloud] ") else {
                continue;
            };
            if let Some(formatted) =
                output::format_deploy_line(msg, engine, color, &mut rindexer_idx)
            {
                eprintln!("{formatted}");
            }
        }
    }

    let cmd_output = child
        .wait_with_output()
        .map_err(|source| CliError::CommandSpawn {
            command: script_path.display().to_string(),
            source,
        })?;

    if !cmd_output.status.success() {
        let stderr = String::from_utf8_lossy(&cmd_output.stderr);
        if !stderr.trim().is_empty() {
            eprintln!("{}", stderr.trim());
        }
    }

    exit_status_to_result(cmd_output.status)
}

fn exit_status_to_result(status: std::process::ExitStatus) -> Result<()> {
    crate::error::map_exit_status(
        status,
        |code| CliError::DeployerFailed { code },
        |signal| CliError::DeployerSignaled { signal },
    )
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
