use std::path::PathBuf;
use std::process::{Command, Stdio};

use clap::Args;

use crate::config::schema::ComputeEngine;
use crate::error::{CliError, Result};
use crate::handoff::WorkloadHandoff;
use crate::output::{self, ColorMode};
use crate::post_deploy;
use crate::preflight::{self, ProjectKind};
use crate::ssh::{self, SshOverrides};
use crate::terraform::TerraformRunner;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DiscoveredService {
    short_name: String,
    full_name: String,
    kind: ServiceKind,
}

#[derive(Debug, Clone, PartialEq)]
enum ServiceKind {
    Indexer,
    RpcProxy,
    CustomService,
    Database,
    Monitoring,
    Ingress,
}

struct ServiceTarget {
    display_name: String,
    #[allow(dead_code)]
    kind: ServiceKind,
    label_selector: String,
    namespace: String,
    compose_name: Option<String>,
}

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Args)]
/// Stream logs from a deployed service
pub(crate) struct LogsArgs {
    /// Service to stream logs from (omit to list available services)
    service: Option<String>,

    /// List available services
    #[arg(long)]
    list: bool,

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

// ---------------------------------------------------------------------------
// Service discovery
// ---------------------------------------------------------------------------

fn discover_services(handoff: &WorkloadHandoff) -> Vec<DiscoveredService> {
    let project = post_deploy::sanitize_namespace(&handoff.project_name);
    let mut services = Vec::new();

    // 1. Indexer instances
    match &handoff.services.indexer {
        Some(idx) => {
            if let Some(instances) = &idx.instances {
                if !instances.is_empty() {
                    for inst in instances {
                        services.push(DiscoveredService {
                            short_name: inst.name.clone(),
                            full_name: format!("{project}-{}", inst.name),
                            kind: ServiceKind::Indexer,
                        });
                    }
                } else {
                    // Empty instances array — add default
                    services.push(DiscoveredService {
                        short_name: "indexer".into(),
                        full_name: format!("{project}-indexer"),
                        kind: ServiceKind::Indexer,
                    });
                }
            } else {
                // No instances field — single indexer
                services.push(DiscoveredService {
                    short_name: "indexer".into(),
                    full_name: format!("{project}-indexer"),
                    kind: ServiceKind::Indexer,
                });
            }
        }
        None => {
            // Indexer field absent — indexer is always deployed
            services.push(DiscoveredService {
                short_name: "indexer".into(),
                full_name: format!("{project}-indexer"),
                kind: ServiceKind::Indexer,
            });
        }
    }

    // 2. RPC proxy
    if handoff.services.rpc_proxy.is_some() {
        services.push(DiscoveredService {
            short_name: "erpc".into(),
            full_name: format!("{project}-erpc"),
            kind: ServiceKind::RpcProxy,
        });
    }

    // 3. Custom services
    if let Some(custom) = &handoff.services.custom_services {
        for entry in custom {
            services.push(DiscoveredService {
                short_name: entry.name.clone(),
                full_name: format!("{project}-{}", entry.name),
                kind: ServiceKind::CustomService,
            });
        }
    }

    // 4. Monitoring
    if handoff.services.monitoring.is_some() {
        services.push(DiscoveredService {
            short_name: "grafana".into(),
            full_name: format!("{project}-grafana"),
            kind: ServiceKind::Monitoring,
        });
        services.push(DiscoveredService {
            short_name: "prometheus".into(),
            full_name: format!("{project}-prometheus"),
            kind: ServiceKind::Monitoring,
        });
    }

    // 5. Static infrastructure services (always present)
    services.push(DiscoveredService {
        short_name: "clickhouse".into(),
        full_name: format!("{project}-clickhouse"),
        kind: ServiceKind::Database,
    });
    services.push(DiscoveredService {
        short_name: "postgres".into(),
        full_name: format!("{project}-postgres"),
        kind: ServiceKind::Database,
    });
    services.push(DiscoveredService {
        short_name: "caddy".into(),
        full_name: format!("{project}-caddy"),
        kind: ServiceKind::Ingress,
    });

    services
}

fn fallback_services(project: &str) -> Vec<DiscoveredService> {
    vec![
        DiscoveredService {
            short_name: "indexer".into(),
            full_name: format!("{project}-indexer"),
            kind: ServiceKind::Indexer,
        },
        DiscoveredService {
            short_name: "erpc".into(),
            full_name: format!("{project}-erpc"),
            kind: ServiceKind::RpcProxy,
        },
        DiscoveredService {
            short_name: "clickhouse".into(),
            full_name: format!("{project}-clickhouse"),
            kind: ServiceKind::Database,
        },
        DiscoveredService {
            short_name: "postgres".into(),
            full_name: format!("{project}-postgres"),
            kind: ServiceKind::Database,
        },
        DiscoveredService {
            short_name: "caddy".into(),
            full_name: format!("{project}-caddy"),
            kind: ServiceKind::Ingress,
        },
    ]
}

// ---------------------------------------------------------------------------
// Service resolution
// ---------------------------------------------------------------------------

fn resolve_service(
    input: &str,
    project: &str,
    services: &[DiscoveredService],
) -> Result<ServiceTarget> {
    let needle = input.to_lowercase();

    // 1. Exact match on short_name or full_name
    let matched = services
        .iter()
        .find(|s| s.short_name.to_lowercase() == needle || s.full_name.to_lowercase() == needle);

    // 2. Strip project prefix and retry on short_name
    let matched = matched.or_else(|| {
        let prefix = format!("{}-", project.to_lowercase());
        if let Some(stripped) = needle.strip_prefix(&prefix) {
            services
                .iter()
                .find(|s| s.short_name.to_lowercase() == stripped)
        } else {
            None
        }
    });

    // 3. Add project prefix and match on full_name
    let matched = matched.or_else(|| {
        let prefixed = format!("{}-{}", project.to_lowercase(), needle);
        services
            .iter()
            .find(|s| s.full_name.to_lowercase() == prefixed)
    });

    let svc = match matched {
        Some(s) => s,
        None => {
            let available: Vec<String> = services
                .iter()
                .map(|s| format!("  {} ({})", s.short_name, s.full_name))
                .collect();
            return Err(CliError::InvalidArg {
                arg: input.to_string(),
                details: format!(
                    "unknown service. Available:\n{}",
                    available.join("\n")
                ),
            });
        }
    };

    let target = build_service_target(svc, project);
    Ok(target)
}

fn build_service_target(svc: &DiscoveredService, project: &str) -> ServiceTarget {
    match svc.kind {
        ServiceKind::Indexer => ServiceTarget {
            display_name: svc.full_name.clone(),
            kind: ServiceKind::Indexer,
            label_selector: format!("app.kubernetes.io/instance={}", svc.full_name),
            namespace: project.to_string(),
            compose_name: Some("rindexer".to_string()),
        },
        ServiceKind::RpcProxy => ServiceTarget {
            display_name: svc.full_name.clone(),
            kind: ServiceKind::RpcProxy,
            label_selector: format!("app.kubernetes.io/instance={}", svc.full_name),
            namespace: project.to_string(),
            compose_name: Some("erpc".to_string()),
        },
        ServiceKind::CustomService => ServiceTarget {
            display_name: svc.full_name.clone(),
            kind: ServiceKind::CustomService,
            label_selector: format!("app.kubernetes.io/instance={}", svc.full_name),
            namespace: project.to_string(),
            compose_name: None,
        },
        ServiceKind::Database => ServiceTarget {
            display_name: svc.full_name.clone(),
            kind: ServiceKind::Database,
            label_selector: format!("app={}", svc.short_name),
            namespace: project.to_string(),
            compose_name: Some(svc.short_name.clone()),
        },
        ServiceKind::Monitoring => ServiceTarget {
            display_name: svc.full_name.clone(),
            kind: ServiceKind::Monitoring,
            label_selector: format!("app.kubernetes.io/name={}", svc.short_name),
            namespace: "monitoring".to_string(),
            compose_name: None,
        },
        ServiceKind::Ingress => ServiceTarget {
            display_name: svc.full_name.clone(),
            kind: ServiceKind::Ingress,
            label_selector: format!("app={}", svc.short_name),
            namespace: project.to_string(),
            compose_name: Some("caddy".to_string()),
        },
    }
}

// ---------------------------------------------------------------------------
// Service list display
// ---------------------------------------------------------------------------

fn print_service_list(
    services: &[DiscoveredService],
    project: &str,
    compute_engine: Option<ComputeEngine>,
    _color: ColorMode,
) {
    let is_compose = matches!(
        compute_engine,
        Some(ComputeEngine::Ec2) | Some(ComputeEngine::DockerCompose)
    );

    let engine_label = match compute_engine {
        Some(ComputeEngine::K3s) => "k3s",
        Some(ComputeEngine::Eks) => "eks",
        Some(ComputeEngine::Ec2) => "ec2",
        Some(ComputeEngine::DockerCompose) => "docker_compose",
        None => "unknown",
    };

    println!(
        "evm-cloud logs \u{2014} {} ({engine_label})\n",
        project
    );
    println!(
        "  {:<20} {:<30} {}",
        "Service", "Target", "Engine"
    );
    println!("  {}", "\u{2500}".repeat(62));

    // Filter out services unavailable on the current engine
    let visible: Vec<&DiscoveredService> = if is_compose {
        services
            .iter()
            .filter(|s| {
                // Only show services that have a compose mapping
                matches!(
                    s.kind,
                    ServiceKind::Indexer
                        | ServiceKind::RpcProxy
                        | ServiceKind::Database
                        | ServiceKind::Ingress
                )
            })
            .collect()
    } else {
        services.iter().collect()
    };

    for svc in &visible {
        let (target, engine) = if is_compose {
            let compose_name = match svc.kind {
                ServiceKind::Indexer => "rindexer",
                ServiceKind::RpcProxy => "erpc",
                ServiceKind::Ingress => "caddy",
                ServiceKind::Database => &svc.short_name,
                _ => &svc.full_name,
            };
            (compose_name.to_string(), "compose".to_string())
        } else {
            let engine = if svc.kind == ServiceKind::Monitoring {
                "k8s (monitoring)".to_string()
            } else {
                "k8s".to_string()
            };
            (svc.full_name.clone(), engine)
        };

        println!("  {:<20} {:<30} {}", svc.short_name, target, engine);
    }

    println!("\n  Tip: evm-cloud logs <service> -f");
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub(crate) fn run(args: LogsArgs, color: ColorMode) -> Result<()> {
    let follow = args.follow;

    // 1. Resolve project root and kind
    let preflight = preflight::run_checks(&args.dir, true)?;
    let project_root = &preflight.resolved_root;

    // 2. Resolve env context
    let env_ctx = crate::env::resolve_env(args.env.as_deref(), project_root)?;
    if let Some(ref ctx) = env_ctx {
        output::info(&format!("[env: {}]", ctx.name), color);
    }

    let terraform_dir = match &preflight.project_kind {
        ProjectKind::EasyToml => project_root.join(".evm-cloud"),
        ProjectKind::RawTerraform => project_root.clone(),
    };

    // 3. Init TerraformRunner, try to load handoff
    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    let runner = match env_ctx.as_ref() {
        Some(ctx) => runner.with_env(ctx),
        None => runner,
    };

    let (handoff, services, sanitized_project) =
        match crate::handoff::load_from_state(&runner, &terraform_dir, &args.module_name) {
            Ok(h) => {
                let p = post_deploy::sanitize_namespace(&h.project_name);
                let svcs = discover_services(&h);
                (Some(h), svcs, p)
            }
            Err(_) => {
                let dir_name = project_root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("project");
                let p = post_deploy::sanitize_namespace(dir_name);
                output::warn(
                    "Could not load deployment state. Using default service list.",
                    color,
                );
                (None, fallback_services(&p), p)
            }
        };

    let compute_engine_for_list = handoff.as_ref().map(|h| h.compute_engine);

    // 4. Show service list if --list or no service arg
    if args.list || args.service.is_none() {
        print_service_list(&services, &sanitized_project, compute_engine_for_list, color);
        return Ok(());
    }

    // 5. Resolve service target
    let service_input = args.service.as_deref().unwrap_or("indexer");
    let target = resolve_service(service_input, &sanitized_project, &services)?;

    // 6. Info line
    output::info(&format!("Tailing {} logs...", target.display_name), color);

    // 7. Require handoff for actual log streaming
    let handoff_ref = match &handoff {
        Some(h) => h,
        None => {
            return Err(CliError::HandoffMissing {
                module: args.module_name.clone(),
            });
        }
    };

    let compute_engine = handoff_ref.compute_engine;

    // Warn if --pod on non-K8s engine
    if args.pod.is_some()
        && !matches!(compute_engine, ComputeEngine::K3s | ComputeEngine::Eks)
    {
        output::warn("--pod is only supported on K8s engines, ignoring", color);
    }

    let status = match compute_engine {
        ComputeEngine::Ec2 | ComputeEngine::DockerCompose => {
            // Check for CloudWatch path (EC2 only)
            let cw_group = handoff_ref
                .runtime
                .ec2
                .as_ref()
                .and_then(|rt| post_deploy::non_empty(rt.cloudwatch_log_group.as_deref()));

            if let Some(log_group) = cw_group {
                if compute_engine == ComputeEngine::Ec2 {
                    match which::which("aws") {
                        Ok(_) => {
                            return run_cloudwatch(&log_group, follow, color);
                        }
                        Err(_) => {
                            output::warn("aws CLI not found, falling back to SSH", color);
                        }
                    }
                }
            } else if compute_engine == ComputeEngine::Ec2 {
                output::info("No CloudWatch log group configured, using SSH", color);
            }

            // Check if service is available via compose
            let compose_name = match &target.compose_name {
                Some(name) => name,
                None => {
                    return Err(CliError::InvalidArg {
                        arg: target.display_name.clone(),
                        details: format!(
                            "Service '{}' requires K3s/EKS. Current engine: {:?}.",
                            target.display_name, compute_engine
                        ),
                    });
                }
            };

            // SSH + docker compose logs path
            let ssh_ctx = ssh::resolve(
                handoff_ref,
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
                compose_name, args.tail
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
                handoff_ref,
                project_root,
                &terraform_dir,
                None,
            )?;

            let mut cmd = Command::new("kubectl");

            if let Some(ref pod_name) = args.pod {
                cmd.args([
                    "logs",
                    pod_name,
                    "-n",
                    &target.namespace,
                    "--tail",
                    &args.tail.to_string(),
                    "--all-containers=true",
                ]);
            } else {
                cmd.args([
                    "logs",
                    "-l",
                    &target.label_selector,
                    "-n",
                    &target.namespace,
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
        match compute_engine {
            ComputeEngine::Ec2 | ComputeEngine::DockerCompose => "ssh",
            ComputeEngine::K3s | ComputeEngine::Eks => "kubectl",
        },
    )
}

// ---------------------------------------------------------------------------
// CloudWatch helper (unchanged)
// ---------------------------------------------------------------------------

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
