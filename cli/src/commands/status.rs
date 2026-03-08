use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use crate::config::schema::ComputeEngine;
use crate::error::{CliError, Result};
use crate::handoff::WorkloadHandoff;
use crate::output::{self, ColorMode};
use crate::post_deploy;
use crate::preflight::{self, ProjectKind};
use crate::ssh::{self, SshContext, SshOverrides};
use crate::terraform::TerraformRunner;

#[derive(Args)]
pub(crate) struct StatusArgs {
    #[arg(short = 'd', long, default_value = ".")]
    dir: PathBuf,
    #[arg(long, default_value = "evm_cloud")]
    module_name: String,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    allow_raw_terraform: bool,
    /// Target environment for multi-env projects (envs/<name>/)
    #[arg(long, env = "EVM_CLOUD_ENV")]
    env: Option<String>,
    #[arg(long)]
    ssh_key: Option<PathBuf>,
    #[arg(long)]
    ssh_user: Option<String>,
    #[arg(long)]
    ssh_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StatusReport {
    pub project_name: String,
    pub compute_engine: ComputeEngine,
    pub region: Option<String>,
    pub services: Vec<ServiceProbe>,
    pub connection: ConnectionInfo,
    pub overall: ProbeStatus,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ServiceProbe {
    pub name: String,
    pub status: ProbeStatus,
    pub endpoint: Option<String>,
    pub detail: Option<String>,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ProbeStatus {
    Healthy,
    Degraded,
    Down,
    #[allow(dead_code)] // Used in --json serialization and future partial-deploy scenarios
    Skipped,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct ConnectionInfo {
    pub ssh_command: Option<String>,
    pub kubectl_command: Option<String>,
    pub logs_command: String,
}

pub(crate) fn run(args: StatusArgs, color: ColorMode) -> Result<()> {
    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    let project_root = &preflight.resolved_root;
    let env_ctx = crate::env::resolve_env(args.env.as_deref(), project_root)?;

    let terraform_dir = match &preflight.project_kind {
        ProjectKind::EasyToml => project_root.join(".evm-cloud"),
        ProjectKind::RawTerraform => project_root.clone(),
    };

    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    let runner = match env_ctx.as_ref() {
        Some(ctx) => runner.with_env(ctx),
        None => runner,
    };
    let handoff = output::with_spinner("Loading deployment state", color, || {
        crate::handoff::load_from_state(&runner, &terraform_dir, &args.module_name)
    })?;

    let region = post_deploy::aws_region(&handoff);

    let (services, connection) = match handoff.compute_engine {
        ComputeEngine::Ec2 | ComputeEngine::DockerCompose => {
            let ssh_ctx = ssh::resolve(
                &handoff,
                project_root,
                &preflight.project_kind,
                SshOverrides {
                    key: args.ssh_key,
                    user: args.ssh_user,
                    port: args.ssh_port,
                },
            )?;
            let services = probe_ssh_engine(&ssh_ctx, &handoff, color);
            let connection = build_ssh_connection(&ssh_ctx, &handoff);
            (services, connection)
        }
        ComputeEngine::K3s | ComputeEngine::Eks => {
            let ns = post_deploy::sanitize_namespace(&handoff.project_name);
            let kubeconfig = crate::kubeconfig::resolve_or_generate(
                &handoff,
                project_root,
                &terraform_dir,
                None,
            )?;
            let services = probe_k8s_engine(&kubeconfig, &handoff, &ns, color);
            let connection = build_k8s_connection(&handoff, &ns);

            // Also resolve SSH for K3s (has a host)
            let ssh_cmd = if handoff.compute_engine == ComputeEngine::K3s {
                ssh::resolve(
                    &handoff,
                    project_root,
                    &preflight.project_kind,
                    SshOverrides {
                        key: args.ssh_key,
                        user: args.ssh_user,
                        port: args.ssh_port,
                    },
                )
                .ok()
                .map(|ctx| ssh::command_string(&ctx))
            } else {
                None
            };

            let connection = ConnectionInfo {
                ssh_command: ssh_cmd,
                ..connection
            };

            (services, connection)
        }
    };

    let overall = overall_status(&services);

    let report = StatusReport {
        project_name: handoff.project_name.clone(),
        compute_engine: handoff.compute_engine,
        region,
        services,
        connection,
        overall,
    };

    if args.json {
        let json = serde_json::to_string_pretty(&report).map_err(CliError::from)?;
        println!("{json}");
    } else {
        render_table(&report, &handoff, color);
    }

    match report.overall {
        ProbeStatus::Down => std::process::exit(1),
        ProbeStatus::Degraded => std::process::exit(2),
        _ => Ok(()),
    }
}

// ── SSH engine probing (EC2 / DockerCompose / BareMetal) ──

fn probe_ssh_engine(
    ssh_ctx: &SshContext,
    handoff: &WorkloadHandoff,
    color: ColorMode,
) -> Vec<ServiceProbe> {
    let mut services = Vec::new();

    // SSH connectivity test
    let ssh_ok = output::with_spinner("Checking SSH connectivity", color, || -> Result<bool> {
        match ssh::exec(ssh_ctx, "echo ok", 10) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    })
    .unwrap_or(false);

    if !ssh_ok {
        let hint = format!(
            "Cannot reach host. Verify security group allows SSH (port {}) and instance is running.",
            ssh_ctx.port
        );
        services.push(ServiceProbe {
            name: "SSH".to_string(),
            status: ProbeStatus::Down,
            endpoint: Some(format!("{}:{}", ssh_ctx.host, ssh_ctx.port)),
            detail: Some("unreachable".to_string()),
            hint: Some(hint),
        });
        push_skipped_ssh_services(&mut services, handoff);
        return services;
    }

    // Docker compose status
    let compose_json = ssh::exec(
        ssh_ctx,
        "docker compose -f /opt/evm-cloud/docker-compose.yml ps --format json 2>/dev/null || docker-compose -f /opt/evm-cloud/docker-compose.yml ps --format json 2>/dev/null",
        15,
    );

    let container_states = match compose_json {
        Ok(json_str) => parse_compose_ps(&json_str),
        Err(_) => Vec::new(),
    };

    // rindexer (always present)
    let rindexer_state = find_container(&container_states, "rindexer");
    let rindexer_probe = probe_ssh_http(ssh_ctx, "http://localhost:18080/health", 5);
    let ssh_target = ssh::command_string(ssh_ctx);
    let ssh_target_suffix = ssh_target.strip_prefix("ssh ").unwrap_or("");
    services.push(build_compose_probe(
        "rindexer",
        rindexer_state,
        rindexer_probe,
        "port 18080",
        &format!("Check indexer logs: ssh {ssh_target_suffix} 'docker compose -f /opt/evm-cloud/docker-compose.yml logs rindexer'"),
    ));

    // eRPC (optional)
    if handoff.services.rpc_proxy.is_some() {
        let erpc_state = find_container(&container_states, "erpc");
        let erpc_probe = probe_ssh_http(ssh_ctx, "http://localhost:4000/", 5);
        let erpc_endpoint = post_deploy::erpc_url(handoff);
        services.push(build_compose_probe(
            "eRPC",
            erpc_state,
            erpc_probe,
            erpc_endpoint.as_deref().unwrap_or("port 4000"),
            "eRPC may be misconfigured. Check logs with docker compose logs erpc",
        ));
    }

    // ClickHouse (optional)
    if handoff.data.clickhouse.is_some() || handoff.data.backend.as_deref() == Some("clickhouse") {
        let ch_state = find_container(&container_states, "clickhouse");
        let ch_probe = probe_ssh_http(ssh_ctx, "http://localhost:8123/?query=SELECT+1", 5);
        services.push(build_compose_probe(
            "ClickHouse",
            ch_state,
            ch_probe,
            "port 8123",
            &format!(
                "ClickHouse may have run out of disk. SSH in: {}",
                ssh::command_string(ssh_ctx)
            ),
        ));
    }

    // Postgres (optional)
    if handoff.data.postgres.is_some() || handoff.data.backend.as_deref() == Some("postgres") {
        let pg_state = find_container(&container_states, "postgres");
        services.push(ServiceProbe {
            name: "Postgres".to_string(),
            status: container_to_status(pg_state),
            endpoint: post_deploy::format_postgres_url(handoff),
            detail: pg_state.map(|s| s.to_string()),
            hint: None,
        });
    }

    // Caddy (optional, EC2 only with ingress)
    if handoff.ingress.erpc_hostname.is_some() && handoff.compute_engine == ComputeEngine::Ec2 {
        let caddy_state = find_container(&container_states, "caddy");
        services.push(ServiceProbe {
            name: "Caddy".to_string(),
            status: container_to_status(caddy_state),
            endpoint: Some("port 443".to_string()),
            detail: caddy_state.map(|s| format!("{s} · TLS active")),
            hint: None,
        });
    }

    services
}

fn push_skipped_ssh_services(services: &mut Vec<ServiceProbe>, handoff: &WorkloadHandoff) {
    let hint = "Cannot probe — SSH connectivity failed".to_string();
    services.push(ServiceProbe {
        name: "rindexer".to_string(),
        status: ProbeStatus::Skipped,
        endpoint: None,
        detail: Some("unable to probe — SSH failed".to_string()),
        hint: Some(hint.clone()),
    });
    if handoff.services.rpc_proxy.is_some() {
        services.push(ServiceProbe {
            name: "eRPC".to_string(),
            status: ProbeStatus::Skipped,
            endpoint: None,
            detail: Some("unable to probe — SSH failed".to_string()),
            hint: Some(hint.clone()),
        });
    }
    if handoff.data.clickhouse.is_some() || handoff.data.backend.as_deref() == Some("clickhouse") {
        services.push(ServiceProbe {
            name: "ClickHouse".to_string(),
            status: ProbeStatus::Skipped,
            endpoint: None,
            detail: Some("unable to probe — SSH failed".to_string()),
            hint: Some(hint.clone()),
        });
    }
}

fn probe_ssh_http(ssh_ctx: &SshContext, url: &str, timeout: u32) -> Option<bool> {
    ssh::exec(
        ssh_ctx,
        &format!("curl -sf --max-time {timeout} {url}"),
        timeout + 5,
    )
    .ok()
    .map(|_| true)
}

fn build_compose_probe(
    name: &str,
    container_state: Option<&str>,
    http_probe: Option<bool>,
    detail_ok: &str,
    hint_on_fail: &str,
) -> ServiceProbe {
    let base_status = container_to_status(container_state);

    let status = match (base_status, http_probe) {
        (ProbeStatus::Healthy, Some(true)) => ProbeStatus::Healthy,
        (ProbeStatus::Healthy, Some(false)) => ProbeStatus::Degraded,
        (ProbeStatus::Healthy, None) => ProbeStatus::Healthy,
        (other, _) => other,
    };

    let detail = match status {
        ProbeStatus::Healthy => Some(format!("running · {detail_ok}")),
        ProbeStatus::Down => container_state.map(|s| s.to_string()),
        ProbeStatus::Degraded => Some("running but HTTP probe failed".to_string()),
        ProbeStatus::Skipped => None,
    };

    ServiceProbe {
        name: name.to_string(),
        status,
        endpoint: if status == ProbeStatus::Healthy {
            Some(detail_ok.to_string())
        } else {
            None
        },
        detail,
        hint: if status != ProbeStatus::Healthy {
            Some(hint_on_fail.to_string())
        } else {
            None
        },
    }
}

fn container_to_status(state: Option<&str>) -> ProbeStatus {
    match state {
        None => ProbeStatus::Down,
        Some(s) => {
            let lower = s.to_lowercase();
            if lower.contains("running") || lower.contains("up") {
                ProbeStatus::Healthy
            } else if lower.contains("restarting") {
                ProbeStatus::Degraded
            } else {
                ProbeStatus::Down
            }
        }
    }
}

/// Parse `docker compose ps --format json` output.
/// Each line is a JSON object with "Name", "State", "Health" fields.
pub(crate) fn parse_compose_ps(json_str: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    for line in json_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
            let name = obj
                .get("Name")
                .or_else(|| obj.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let state = obj
                .get("State")
                .or_else(|| obj.get("state"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            if !name.is_empty() {
                results.push((name, state));
            }
        }
    }
    results
}

fn find_container<'a>(states: &'a [(String, String)], keyword: &str) -> Option<&'a str> {
    states
        .iter()
        .find(|(name, _)| name.to_lowercase().contains(keyword))
        .map(|(_, state)| state.as_str())
}

// ── K8s engine probing (K3s / EKS) ──

fn probe_k8s_engine(
    kubeconfig: &std::path::Path,
    handoff: &WorkloadHandoff,
    ns: &str,
    color: ColorMode,
) -> Vec<ServiceProbe> {
    if which::which("kubectl").is_err() {
        return vec![ServiceProbe {
            name: "kubectl".to_string(),
            status: ProbeStatus::Down,
            endpoint: None,
            detail: Some("kubectl not found on PATH".to_string()),
            hint: Some("Install kubectl: https://kubernetes.io/docs/tasks/tools/".to_string()),
        }];
    }

    let mut services = Vec::new();

    let deployments = output::with_spinner(
        "Checking deployments",
        color,
        || -> Result<serde_json::Value> {
            kubectl_json(kubeconfig, &["get", "deployments", "-n", ns, "-o", "json"])
        },
    );

    let pods = kubectl_json(kubeconfig, &["get", "pods", "-n", ns, "-o", "json"]);

    if let Ok(deps) = &deployments {
        let project = &handoff.project_name;
        services.extend(parse_k8s_deployments(deps, project, ns));
    } else if let Err(err) = &deployments {
        services.push(ServiceProbe {
            name: "cluster".to_string(),
            status: ProbeStatus::Down,
            endpoint: None,
            detail: Some(err.to_string()),
            hint: Some(
                "Kubeconfig may be stale. Regenerate: evm-cloud kubectl --generate".to_string(),
            ),
        });
        return services;
    }

    // Enrich with pod details (restart counts, phase)
    if let Ok(pods_value) = &pods {
        enrich_with_pod_details(&mut services, pods_value, ns);
    }

    // Monitoring namespace (optional)
    if handoff.services.monitoring.is_some() {
        let monitoring_pods = kubectl_json(
            kubeconfig,
            &["get", "pods", "-n", "monitoring", "-o", "json"],
        );
        if let Ok(mpods) = monitoring_pods {
            services.extend(parse_monitoring_pods(&mpods, handoff));
        }
    }

    services
}

fn kubectl_json(kubeconfig: &std::path::Path, args: &[&str]) -> Result<serde_json::Value> {
    let output = std::process::Command::new("kubectl")
        .args(args)
        .env("KUBECONFIG", kubeconfig)
        .output()
        .map_err(|err| CliError::ToolFailed {
            tool: "kubectl".to_string(),
            details: err.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::ToolFailed {
            tool: "kubectl".to_string(),
            details: stderr.trim().to_string(),
        });
    }

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    Ok(value)
}

/// Parse k8s deployments JSON into service probes.
pub(crate) fn parse_k8s_deployments(
    deps: &serde_json::Value,
    project: &str,
    ns: &str,
) -> Vec<ServiceProbe> {
    let mut services = Vec::new();
    let items = deps.get("items").and_then(|i| i.as_array());
    let Some(items) = items else { return services };

    for item in items {
        let name = item
            .pointer("/metadata/name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let replicas = item
            .pointer("/spec/replicas")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);
        let ready = item
            .pointer("/status/readyReplicas")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let service_name = deployment_to_service(name, project);
        let status = if ready >= replicas {
            ProbeStatus::Healthy
        } else if ready > 0 {
            ProbeStatus::Degraded
        } else {
            ProbeStatus::Down
        };

        let hint = if status == ProbeStatus::Down {
            Some(format!(
                "Deployment has 0/{replicas} ready replicas. Check: kubectl describe deployment {name} -n {ns}"
            ))
        } else if status == ProbeStatus::Degraded {
            Some(format!(
                "{ready}/{replicas} ready. Check: kubectl describe deployment {name} -n {ns}"
            ))
        } else {
            None
        };

        services.push(ServiceProbe {
            name: service_name,
            status,
            endpoint: None,
            detail: Some(format!("{ready}/{replicas}")),
            hint,
        });
    }

    services
}

fn deployment_to_service(name: &str, _project: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("indexer") || lower.contains("rindexer") {
        "rindexer".to_string()
    } else if lower.contains("erpc") {
        "eRPC".to_string()
    } else if lower.contains("clickhouse") {
        "ClickHouse".to_string()
    } else if lower.contains("postgres") {
        "Postgres".to_string()
    } else {
        name.to_string()
    }
}

fn enrich_with_pod_details(services: &mut [ServiceProbe], pods: &serde_json::Value, ns: &str) {
    let items = match pods.get("items").and_then(|i| i.as_array()) {
        Some(items) => items,
        None => return,
    };

    for item in items {
        let pod_name = item
            .pointer("/metadata/name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let phase = item
            .pointer("/status/phase")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        let restart_count: u64 = item
            .pointer("/status/containerStatuses")
            .and_then(|cs| cs.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c.get("restartCount").and_then(|v| v.as_u64()))
                    .sum()
            })
            .unwrap_or(0);

        for service in services.iter_mut() {
            let matches = pod_name
                .to_lowercase()
                .contains(&service.name.to_lowercase())
                || (service.name == "rindexer" && pod_name.contains("indexer"))
                || (service.name == "eRPC" && pod_name.to_lowercase().contains("erpc"));

            if matches {
                let pods_detail = format!(
                    "{} · {}{}",
                    pod_name,
                    phase,
                    if restart_count > 0 {
                        format!(" · {} restarts", restart_count)
                    } else {
                        String::new()
                    }
                );

                if let Some(existing) = &service.detail {
                    service.detail = Some(format!("{existing}  {pods_detail}"));
                } else {
                    service.detail = Some(pods_detail);
                }

                if restart_count > 5 && service.status != ProbeStatus::Down {
                    service.status = ProbeStatus::Degraded;
                    service.hint = Some(format!(
                        "Container is crash-looping ({restart_count} restarts). Check: kubectl logs {pod_name} -n {ns}"
                    ));
                }

                break;
            }
        }
    }
}

fn parse_monitoring_pods(pods: &serde_json::Value, handoff: &WorkloadHandoff) -> Vec<ServiceProbe> {
    let mut services = Vec::new();
    let items = match pods.get("items").and_then(|i| i.as_array()) {
        Some(items) => items,
        None => return services,
    };

    let mut grafana_found = false;
    let mut prometheus_found = false;
    let mut alertmanager_found = false;

    for item in items {
        let name = item
            .pointer("/metadata/name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let phase = item
            .pointer("/status/phase")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        let lower = name.to_lowercase();

        let status = if phase == "Running" {
            ProbeStatus::Healthy
        } else if phase == "Pending" {
            ProbeStatus::Degraded
        } else {
            ProbeStatus::Down
        };

        if lower.contains("grafana") && !grafana_found {
            grafana_found = true;
            services.push(ServiceProbe {
                name: "Grafana".to_string(),
                status,
                endpoint: post_deploy::grafana_line(handoff),
                detail: Some(format!("{name} · {phase}")),
                hint: None,
            });
        } else if lower.contains("prometheus") && !lower.contains("operator") && !prometheus_found {
            prometheus_found = true;
            services.push(ServiceProbe {
                name: "Prometheus".to_string(),
                status,
                endpoint: None,
                detail: Some(format!("{name} · {phase}")),
                hint: None,
            });
        } else if lower.contains("alertmanager") && !alertmanager_found {
            alertmanager_found = true;
            services.push(ServiceProbe {
                name: "Alertmanager".to_string(),
                status,
                endpoint: None,
                detail: Some(format!("{name} · {phase}")),
                hint: None,
            });
        }
    }

    services
}

// ── Connection info builders ──

fn build_ssh_connection(ssh_ctx: &SshContext, handoff: &WorkloadHandoff) -> ConnectionInfo {
    ConnectionInfo {
        ssh_command: Some(ssh::command_string(ssh_ctx)),
        kubectl_command: None,
        logs_command: post_deploy::logs_command(handoff),
    }
}

fn build_k8s_connection(handoff: &WorkloadHandoff, ns: &str) -> ConnectionInfo {
    ConnectionInfo {
        ssh_command: None,
        kubectl_command: Some(format!("evm-cloud kubectl -- get pods -n {ns}")),
        logs_command: post_deploy::logs_command(handoff),
    }
}

// ── Aggregation ──

pub(crate) fn overall_status(services: &[ServiceProbe]) -> ProbeStatus {
    let mut worst = ProbeStatus::Healthy;
    for s in services {
        match s.status {
            ProbeStatus::Down => return ProbeStatus::Down,
            ProbeStatus::Degraded => worst = ProbeStatus::Degraded,
            ProbeStatus::Healthy | ProbeStatus::Skipped => {}
        }
    }
    worst
}

// ── Rendering ──

fn render_table(report: &StatusReport, handoff: &WorkloadHandoff, color: ColorMode) {
    let engine_label = match report.compute_engine {
        ComputeEngine::Ec2 => "ec2",
        ComputeEngine::DockerCompose => "docker-compose",
        ComputeEngine::K3s => "k3s",
        ComputeEngine::Eks => "eks",
    };

    let region_str = report.region.as_deref().unwrap_or("unknown");
    eprintln!();
    output::headline(
        &format!(
            " evm-cloud status — {} ({} · {})",
            report.project_name, engine_label, region_str
        ),
        color,
    );

    // K8s cluster info
    if matches!(
        report.compute_engine,
        ComputeEngine::K3s | ComputeEngine::Eks
    ) {
        if let Some(total_nodes) = post_deploy::k3s_total_nodes(handoff) {
            eprintln!();
            output::subline(
                &format!(
                    "Cluster        {} node{} · Ready",
                    total_nodes,
                    if total_nodes == 1 { "" } else { "s" }
                ),
                color,
            );
        }
        let ns = post_deploy::sanitize_namespace(&report.project_name);
        output::subline(&format!("Namespace      {ns}"), color);
    }

    // Service table
    output::section_line("🏰 Services", color);

    let has_monitoring = report
        .services
        .iter()
        .any(|s| matches!(s.name.as_str(), "Grafana" | "Prometheus" | "Alertmanager"));

    let mut printed_monitoring_header = false;

    for service in &report.services {
        if matches!(
            service.name.as_str(),
            "Grafana" | "Prometheus" | "Alertmanager"
        ) && !printed_monitoring_header
            && has_monitoring
        {
            output::section_line("Monitoring (namespace: monitoring)", color);
            printed_monitoring_header = true;
        }

        let (icon, status_text) = match service.status {
            ProbeStatus::Healthy => ("🟢", "UP"),
            ProbeStatus::Degraded => ("🟠", "DEGRADED"),
            ProbeStatus::Down => ("🔴", "DOWN"),
            ProbeStatus::Skipped => ("⚫️", "SKIPPED"),
        };

        let detail = service.detail.as_deref().unwrap_or("");
        output::status_line(&service.name, icon, status_text, detail, color);

        if let Some(hint) = &service.hint {
            output::hint_line(hint, color);
        }
    }

    // Connection block
    output::section_line("🔌 Connection", color);
    if let Some(ssh_cmd) = &report.connection.ssh_command {
        output::subline(&format!("SSH       {ssh_cmd}"), color);
    }
    if let Some(kubectl_cmd) = &report.connection.kubectl_command {
        output::subline(&format!("kubectl   {kubectl_cmd}"), color);
    }
    if let Some(grafana) = post_deploy::grafana_line(handoff) {
        output::subline(&format!("Grafana   {grafana}"), color);
    }
    output::subline(
        &format!("Logs      {}", report.connection.logs_command),
        color,
    );

    // Overall verdict
    eprintln!();
    let total = report.services.len();
    match report.overall {
        ProbeStatus::Healthy => output::checkline(&format!("All {total} services healthy"), color),
        ProbeStatus::Degraded => {
            let degraded_count = report
                .services
                .iter()
                .filter(|s| s.status == ProbeStatus::Degraded)
                .count();
            output::warn(
                &format!(
                    "{degraded_count} service{} degraded",
                    if degraded_count == 1 { "" } else { "s" }
                ),
                color,
            );
        }
        ProbeStatus::Down => {
            let down_count = report
                .services
                .iter()
                .filter(|s| s.status == ProbeStatus::Down)
                .count();
            output::error(
                &format!(
                    "{down_count} service{} down",
                    if down_count == 1 { "" } else { "s" }
                ),
                color,
            );
        }
        ProbeStatus::Skipped => output::checkline("No services to probe", color),
    }
    eprintln!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_compose_ps_healthy() {
        let input = r#"{"Name":"my-project-rindexer-1","State":"running","Health":""}
{"Name":"my-project-erpc-1","State":"running","Health":""}
{"Name":"my-project-clickhouse-1","State":"running","Health":""}"#;

        let result = parse_compose_ps(input);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, "my-project-rindexer-1");
        assert_eq!(result[0].1, "running");
    }

    #[test]
    fn parse_compose_ps_exited() {
        let input = r#"{"Name":"my-project-rindexer-1","State":"exited","Health":""}"#;
        let result = parse_compose_ps(input);
        assert_eq!(result[0].1, "exited");
        assert_eq!(container_to_status(Some("exited")), ProbeStatus::Down);
    }

    #[test]
    fn parse_compose_ps_restarting() {
        let input = r#"{"Name":"my-project-rindexer-1","State":"restarting","Health":""}"#;
        let result = parse_compose_ps(input);
        assert_eq!(
            container_to_status(Some(&result[0].1)),
            ProbeStatus::Degraded
        );
    }

    #[test]
    fn parse_k8s_deployments_ready() {
        let deps = serde_json::json!({
            "items": [{
                "metadata": {"name": "demo-indexer"},
                "spec": {"replicas": 1},
                "status": {"readyReplicas": 1}
            }]
        });
        let probes = parse_k8s_deployments(&deps, "demo", "evm-cloud");
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].status, ProbeStatus::Healthy);
        assert_eq!(probes[0].name, "rindexer");
    }

    #[test]
    fn parse_k8s_deployments_partial() {
        let deps = serde_json::json!({
            "items": [{
                "metadata": {"name": "demo-erpc"},
                "spec": {"replicas": 2},
                "status": {"readyReplicas": 1}
            }]
        });
        let probes = parse_k8s_deployments(&deps, "demo", "evm-cloud");
        assert_eq!(probes[0].status, ProbeStatus::Degraded);
        assert_eq!(probes[0].name, "eRPC");
    }

    #[test]
    fn parse_k8s_deployments_zero() {
        let deps = serde_json::json!({
            "items": [{
                "metadata": {"name": "demo-indexer"},
                "spec": {"replicas": 1},
                "status": {}
            }]
        });
        let probes = parse_k8s_deployments(&deps, "demo", "evm-cloud");
        assert_eq!(probes[0].status, ProbeStatus::Down);
    }

    #[test]
    fn overall_status_all_healthy() {
        let services = vec![
            ServiceProbe {
                name: "a".into(),
                status: ProbeStatus::Healthy,
                endpoint: None,
                detail: None,
                hint: None,
            },
            ServiceProbe {
                name: "b".into(),
                status: ProbeStatus::Healthy,
                endpoint: None,
                detail: None,
                hint: None,
            },
        ];
        assert_eq!(overall_status(&services), ProbeStatus::Healthy);
    }

    #[test]
    fn overall_status_with_down() {
        let services = vec![
            ServiceProbe {
                name: "a".into(),
                status: ProbeStatus::Healthy,
                endpoint: None,
                detail: None,
                hint: None,
            },
            ServiceProbe {
                name: "b".into(),
                status: ProbeStatus::Down,
                endpoint: None,
                detail: None,
                hint: None,
            },
        ];
        assert_eq!(overall_status(&services), ProbeStatus::Down);
    }

    #[test]
    fn overall_status_with_degraded() {
        let services = vec![
            ServiceProbe {
                name: "a".into(),
                status: ProbeStatus::Healthy,
                endpoint: None,
                detail: None,
                hint: None,
            },
            ServiceProbe {
                name: "b".into(),
                status: ProbeStatus::Degraded,
                endpoint: None,
                detail: None,
                hint: None,
            },
        ];
        assert_eq!(overall_status(&services), ProbeStatus::Degraded);
    }

    #[test]
    fn overall_status_skipped_only() {
        let services = vec![ServiceProbe {
            name: "a".into(),
            status: ProbeStatus::Skipped,
            endpoint: None,
            detail: None,
            hint: None,
        }];
        assert_eq!(overall_status(&services), ProbeStatus::Healthy);
    }
}
