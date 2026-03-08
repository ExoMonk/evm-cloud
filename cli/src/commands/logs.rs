use std::path::PathBuf;
use std::process::{Command, Stdio};

use clap::Args;

use crate::config::schema::ComputeEngine;
use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};
use crate::post_deploy;
use crate::preflight::{self, ProjectKind};
use crate::ssh::{self, SshOverrides};
use crate::terraform::TerraformRunner;

const VALID_SERVICES: &[&str] = &[
    "rindexer",
    "erpc",
    "clickhouse",
    "postgres",
    "caddy",
    "grafana",
    "prometheus",
];

const MONITORING_SERVICES: &[&str] = &["grafana", "prometheus"];

#[derive(Args)]
/// Stream logs from a deployed service
pub(crate) struct LogsArgs {
    /// Service to stream logs from
    #[arg(default_value = "rindexer")]
    service: String,

    /// Follow log output (stream continuously)
    #[arg(short = 'f', long)]
    follow: bool,

    /// Number of historical lines to show
    #[arg(long, default_value_t = 100)]
    tail: u32,

    /// Target a specific pod by name (K8s only, bypasses label selector)
    #[arg(long)]
    pod: Option<String>,

    /// Terraform directory
    #[arg(short = 'd', long, default_value = ".")]
    dir: PathBuf,

    /// Terraform module name
    #[arg(long, default_value = "evm_cloud")]
    module_name: String,

    /// Environment name for multi-env projects
    #[arg(long, env = "EVM_CLOUD_ENV")]
    env: Option<String>,

    /// SSH private key override
    #[arg(long)]
    ssh_key: Option<PathBuf>,

    /// SSH user override
    #[arg(long)]
    ssh_user: Option<String>,

    /// SSH port override
    #[arg(long, default_value_t = 22)]
    ssh_port: u16,
}

pub(crate) fn run(args: LogsArgs, color: ColorMode) -> Result<()> {
    // 1. Validate service name
    let service = args.service.to_lowercase();
    if !VALID_SERVICES.contains(&service.as_str()) {
        return Err(CliError::InvalidArg {
            arg: service,
            details: format!("unknown service. Valid: {}", VALID_SERVICES.join(", ")),
        });
    }

    let follow = args.follow;

    // 3. Resolve project root and kind
    let preflight = preflight::run_checks(&args.dir, true)?;
    let project_root = &preflight.resolved_root;

    // 4. Resolve env context
    let env_ctx = crate::env::resolve_env(args.env.as_deref(), project_root)?;
    if let Some(ref ctx) = env_ctx {
        output::info(&format!("[env: {}]", ctx.name), color);
    }

    let terraform_dir = match &preflight.project_kind {
        ProjectKind::EasyToml => project_root.join(".evm-cloud"),
        ProjectKind::RawTerraform => project_root.clone(),
    };

    // 5. Init TerraformRunner, load handoff
    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    let runner = match env_ctx.as_ref() {
        Some(ctx) => runner.with_env(ctx),
        None => runner,
    };
    let handoff = crate::handoff::load_from_state(&runner, &terraform_dir, &args.module_name)?;

    // 6. Info line
    output::info(&format!("Tailing {} logs...", service), color);

    // Warn if --pod is used on non-K8s engine
    if args.pod.is_some()
        && !matches!(
            handoff.compute_engine,
            ComputeEngine::K3s | ComputeEngine::Eks
        )
    {
        output::warn("--pod is only supported on K8s engines, ignoring", color);
    }

    // 7. Branch on compute engine
    let status = match handoff.compute_engine {
        ComputeEngine::Ec2 | ComputeEngine::DockerCompose => {
            // Check for CloudWatch path (EC2 only)
            let cw_group = handoff
                .runtime
                .ec2
                .as_ref()
                .and_then(|rt| post_deploy::non_empty(rt.cloudwatch_log_group.as_deref()));

            if let Some(log_group) = cw_group {
                if handoff.compute_engine == ComputeEngine::Ec2 {
                    match which::which("aws") {
                        Ok(_) => {
                            return run_cloudwatch(&log_group, follow, color);
                        }
                        Err(_) => {
                            output::warn("aws CLI not found, falling back to SSH", color);
                        }
                    }
                }
            } else if handoff.compute_engine == ComputeEngine::Ec2 {
                output::info("No CloudWatch log group configured, using SSH", color);
            }

            // SSH + docker compose logs path
            let ssh_ctx = ssh::resolve(
                &handoff,
                project_root,
                &preflight.project_kind,
                SshOverrides {
                    key: args.ssh_key,
                    user: args.ssh_user,
                    port: if args.ssh_port != 22 {
                        Some(args.ssh_port)
                    } else {
                        None
                    },
                },
            )?;

            let mut remote_cmd = format!(
                "docker compose -f /opt/evm-cloud/docker-compose.yml logs {} --tail {}",
                service, args.tail
            );
            if follow {
                remote_cmd.push_str(" -f");
            }

            let mut cmd = ssh::stream_command(&ssh_ctx, &remote_cmd, follow);
            cmd.status().map_err(|err| CliError::CommandSpawn {
                command: "ssh".to_string(),
                source: err,
            })?
        }
        ComputeEngine::K3s | ComputeEngine::Eks => {
            let kubeconfig = crate::kubeconfig::resolve_or_generate(
                &handoff,
                project_root,
                &terraform_dir,
                None,
            )?;

            let project = post_deploy::sanitize_namespace(&handoff.project_name);
            let namespace = if MONITORING_SERVICES.contains(&service.as_str()) {
                "monitoring".to_string()
            } else {
                project
            };

            let mut cmd = Command::new("kubectl");

            if let Some(ref pod_name) = args.pod {
                cmd.args([
                    "logs",
                    pod_name,
                    "-n",
                    &namespace,
                    "--tail",
                    &args.tail.to_string(),
                    "--all-containers=true",
                ]);
            } else {
                let k8s_name = match service.as_str() {
                    "rindexer" => "indexer",
                    "erpc" => "rpc-proxy",
                    other => other,
                };
                let label = format!("app.kubernetes.io/name={k8s_name}");
                cmd.args([
                    "logs",
                    "-l",
                    &label,
                    "-n",
                    &namespace,
                    "--tail",
                    &args.tail.to_string(),
                    "--all-containers=true",
                    "--prefix",
                    "--max-log-requests=20",
                ]);
            }

            if follow {
                cmd.arg("-f");
            }

            cmd.env("KUBECONFIG", &kubeconfig);
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());

            cmd.status().map_err(|err| CliError::CommandSpawn {
                command: "kubectl".to_string(),
                source: err,
            })?
        }
    };

    // 8. Forward exit code
    crate::error::tool_exit_status(
        status,
        match handoff.compute_engine {
            ComputeEngine::Ec2 | ComputeEngine::DockerCompose => "ssh",
            ComputeEngine::K3s | ComputeEngine::Eks => "kubectl",
        },
    )
}

fn run_cloudwatch(log_group: &str, follow: bool, color: ColorMode) -> Result<()> {
    output::info("Using CloudWatch Logs (may lag 30-60s vs real-time)", color);

    let mut cmd = Command::new("aws");
    cmd.args(["logs", "tail", log_group, "--format", "short"]);
    if follow {
        cmd.args(["--follow", "--since", "5m"]);
    }
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status().map_err(|err| CliError::CommandSpawn {
        command: "aws".to_string(),
        source: err,
    })?;

    crate::error::tool_exit_status(status, "aws logs tail")
}
