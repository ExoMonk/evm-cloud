use crate::config::schema::ComputeEngine;
use crate::handoff::WorkloadHandoff;
use crate::output::ColorMode;

/// Sanitize a project name into a valid k8s namespace (DNS-1123 label):
/// lowercase, alphanumeric + hyphens, max 63 chars, no leading/trailing hyphens.
pub(crate) fn sanitize_namespace(project_name: &str) -> String {
    let s: String = project_name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').replace("--", "-");
    if s.len() > 63 {
        s[..63].trim_end_matches('-').to_string()
    } else {
        s
    }
}

struct SummaryRow {
    label: &'static str,
    value: String,
}

pub(crate) fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn build_https_url(raw: String) -> String {
    if raw.starts_with("http://") || raw.starts_with("https://") {
        raw
    } else {
        format!("https://{raw}")
    }
}

fn host_target(host: String, ssh_user: &str) -> String {
    if host.contains('@') {
        host
    } else {
        format!("{ssh_user}@{host}")
    }
}

fn push_row(rows: &mut Vec<SummaryRow>, label: &'static str, value: Option<String>) {
    if let Some(value) = value {
        rows.push(SummaryRow { label, value });
    }
}

pub(crate) fn format_postgres_url(handoff: &WorkloadHandoff) -> Option<String> {
    let pg = handoff.data.postgres.as_ref()?;
    let host = non_empty(pg.host.as_deref())?;
    let port = non_empty(pg.port.as_deref()).unwrap_or_else(|| "5432".to_string());
    let db_name = non_empty(pg.db_name.as_deref())?;
    Some(format!("postgresql://{host}:{port}/{db_name}"))
}

pub(crate) fn erpc_url(handoff: &WorkloadHandoff) -> Option<String> {
    non_empty(handoff.ingress.erpc_hostname.as_deref()).map(build_https_url)
}

pub(crate) fn grafana_line(handoff: &WorkloadHandoff) -> Option<String> {
    let monitoring = handoff.services.monitoring.as_ref()?;
    let url = non_empty(monitoring.grafana_hostname.as_deref()).map(build_https_url)?;
    let has_custom_secret =
        non_empty(monitoring.grafana_admin_password_secret_name.as_deref()).is_some();
    if has_custom_secret {
        Some(format!("{url} (admin/<custom-secret>)"))
    } else {
        Some(format!("{url} (admin/prom-operator)"))
    }
}

pub(crate) fn aws_region(handoff: &WorkloadHandoff) -> Option<String> {
    handoff
        .extra
        .get("aws_region")
        .and_then(|value| value.as_str())
        .and_then(|value| non_empty(Some(value)))
}

pub(crate) fn ssh_user_for(engine: ComputeEngine) -> &'static str {
    match engine {
        ComputeEngine::Ec2 => "ec2-user",
        ComputeEngine::K3s | ComputeEngine::DockerCompose | ComputeEngine::Eks => "ubuntu",
    }
}

pub(crate) fn server_target(handoff: &WorkloadHandoff) -> Option<String> {
    let user = ssh_user_for(handoff.compute_engine);

    if handoff.compute_engine == ComputeEngine::K3s {
        return handoff
            .runtime
            .k3s
            .as_ref()
            .and_then(|runtime| non_empty(runtime.host_ip.as_deref()))
            .map(|host| host_target(host, user));
    }

    handoff
        .runtime
        .ec2
        .as_ref()
        .and_then(|runtime| non_empty(runtime.public_ip.as_deref()))
        .map(|host| host_target(host, user))
        .or_else(|| {
            handoff
                .runtime
                .bare_metal
                .as_ref()
                .and_then(|runtime| non_empty(runtime.host_address.as_deref()))
                .map(|host| host_target(host, user))
        })
}

pub(crate) fn logs_command(handoff: &WorkloadHandoff) -> String {
    let ns = sanitize_namespace(&handoff.project_name);
    match handoff.compute_engine {
        ComputeEngine::K3s => {
            if handoff
                .runtime
                .k3s
                .as_ref()
                .and_then(|k3s| non_empty(k3s.kubeconfig_base64.as_deref()))
                .is_some()
            {
                format!("kubectl --kubeconfig=kubeconfig.yaml logs -n {ns} -l app=rindexer -f")
            } else {
                format!("kubectl logs -n {ns} -l app=rindexer -f")
            }
        }
        ComputeEngine::Eks => format!("kubectl logs -n {ns} -l app=rindexer -f"),
        _ => handoff
            .runtime
            .ec2
            .as_ref()
            .and_then(|runtime| non_empty(runtime.cloudwatch_log_group.as_deref()))
            .map(|group| format!("aws logs tail {group} --follow"))
            .unwrap_or_else(|| "evm-cloud logs".to_string()),
    }
}

fn uses_kubectl_wrapper(handoff: &WorkloadHandoff, has_k3s_kubeconfig: bool) -> bool {
    (handoff.compute_engine == ComputeEngine::K3s && has_k3s_kubeconfig)
        || handoff.compute_engine == ComputeEngine::Eks
}

pub(crate) fn k3s_total_nodes(handoff: &WorkloadHandoff) -> Option<usize> {
    if handoff.compute_engine != ComputeEngine::K3s {
        return None;
    }

    let runtime = handoff.runtime.k3s.as_ref()?;
    let control_plane_nodes = runtime
        .node_name
        .as_ref()
        .map(|name| !name.trim().is_empty())
        .unwrap_or(false) as usize;

    Some(control_plane_nodes + runtime.worker_nodes.len())
}

fn get_nodes_command(handoff: &WorkloadHandoff) -> Option<String> {
    if handoff.compute_engine != ComputeEngine::K3s {
        return None;
    }

    let uses_local_kubeconfig = handoff
        .runtime
        .k3s
        .as_ref()
        .and_then(|k3s| non_empty(k3s.kubeconfig_base64.as_deref()))
        .is_some();

    if uses_local_kubeconfig {
        Some("evm-cloud kubectl get nodes -o wide".to_string())
    } else {
        Some("kubectl get nodes -o wide".to_string())
    }
}

pub(crate) fn print_summary(handoff: &WorkloadHandoff, _mode: ColorMode) {
    let ns = sanitize_namespace(&handoff.project_name);
    let mut rows: Vec<SummaryRow> = Vec::new();

    if handoff.compute_engine == ComputeEngine::Eks {
        let cluster_name = handoff
            .runtime
            .eks
            .as_ref()
            .and_then(|runtime| non_empty(runtime.cluster_name.as_deref()));
        let region = aws_region(handoff);
        let cluster_cmd = cluster_name.map(|cluster_name| {
            if let Some(region) = region {
                format!("aws eks update-kubeconfig --name {cluster_name} --region {region}")
            } else {
                format!("aws eks update-kubeconfig --name {cluster_name}")
            }
        });
        push_row(&mut rows, "Cluster", cluster_cmd);
    } else {
        push_row(&mut rows, "Server", server_target(handoff));
    }

    push_row(&mut rows, "eRPC", erpc_url(handoff));
    push_row(&mut rows, "Grafana", grafana_line(handoff));

    if handoff.data.backend.as_deref() == Some("clickhouse") {
        push_row(
            &mut rows,
            "ClickHouse",
            handoff
                .data
                .clickhouse
                .as_ref()
                .and_then(|ch| non_empty(ch.url.as_deref())),
        );
    }

    if handoff.data.backend.as_deref() == Some("postgres") {
        push_row(&mut rows, "Postgres", format_postgres_url(handoff));
    }

    let has_k3s_kubeconfig = handoff
        .runtime
        .k3s
        .as_ref()
        .and_then(|runtime| non_empty(runtime.kubeconfig_base64.as_deref()))
        .is_some();

    if uses_kubectl_wrapper(handoff, has_k3s_kubeconfig) {
        push_row(&mut rows, "Kubeconfig commands", Some(String::new()));
    }

    if let Some(total_nodes) = k3s_total_nodes(handoff) {
        push_row(
            &mut rows,
            "Nodes",
            Some(format!(
                "{total_nodes} node{}",
                if total_nodes == 1 { "" } else { "s" }
            )),
        );
        push_row(&mut rows, "Get nodes", get_nodes_command(handoff));
    }

    if (handoff.compute_engine == ComputeEngine::K3s && !has_k3s_kubeconfig)
        || (handoff.compute_engine == ComputeEngine::Eks
            && !uses_kubectl_wrapper(handoff, has_k3s_kubeconfig))
    {
        push_row(&mut rows, "Pods", Some(format!("kubectl get pods -n {ns}")));
    }

    if !uses_kubectl_wrapper(handoff, has_k3s_kubeconfig) {
        push_row(&mut rows, "Logs", Some(logs_command(handoff)));
    }
    push_row(&mut rows, "Status", Some("evm-cloud status".to_string()));

    for row in rows {
        if row.label == "Kubeconfig commands" {
            eprintln!("     👉🏻 Kubeconfig commands");
        } else {
            eprintln!("     👉🏻 {:<12} {}", row.label, row.value);
        }

        if row.label == "Kubeconfig commands" {
            eprintln!("         🛟 Pods      evm-cloud kubectl get pods -n {ns}");
            eprintln!("         🛟 Logs      evm-cloud kubectl logs -n {ns} -l app=rindexer -f");
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::handoff::parse_handoff_value;

    use super::{
        erpc_url, format_postgres_url, get_nodes_command, k3s_total_nodes, logs_command,
        sanitize_namespace, server_target,
    };

    #[test]
    fn sanitizes_project_name_to_namespace() {
        assert_eq!(sanitize_namespace("my-project"), "my-project");
        assert_eq!(sanitize_namespace("My_Project"), "my-project");
        assert_eq!(sanitize_namespace("--leading--"), "leading");
        assert_eq!(sanitize_namespace("UPPER.CASE"), "upper-case");
        assert_eq!(sanitize_namespace("a".repeat(100).as_str()), "a".repeat(63));
    }

    #[test]
    fn picks_k3s_server_target() {
        let handoff = parse_handoff_value(json!({
            "version":"v1",
            "mode":"external",
            "compute_engine":"k3s",
            "project_name":"demo",
            "runtime": {"ec2":null,"bare_metal":null,"eks":null,"k3s":{"host_ip":"54.1.2.3","kubeconfig_base64":"abc"}},
            "services": {},
            "data": {},
            "secrets": {},
            "ingress": {}
        }))
        .expect("handoff must parse");

        assert_eq!(server_target(&handoff).as_deref(), Some("ubuntu@54.1.2.3"));
    }

    #[test]
    fn prefers_ingress_erpc_url() {
        let handoff = parse_handoff_value(json!({
            "version":"v1",
            "mode":"external",
            "compute_engine":"ec2",
            "project_name":"demo",
            "runtime": {"ec2": {"public_ip":"54.1.2.3"},"bare_metal":null,"eks":null,"k3s":null},
            "services": {"rpc_proxy": {"internal_url":"http://erpc:4000"}},
            "data": {},
            "secrets": {},
            "ingress": {"erpc_hostname":"rpc.example.com"}
        }))
        .expect("handoff must parse");

        assert_eq!(
            erpc_url(&handoff).as_deref(),
            Some("https://rpc.example.com")
        );
    }

    #[test]
    fn formats_postgres_connection_line() {
        let handoff = parse_handoff_value(json!({
            "version":"v1",
            "mode":"external",
            "compute_engine":"eks",
            "project_name":"demo",
            "runtime": {"ec2":null,"bare_metal":null,"eks":{"cluster_name":"demo"},"k3s":null},
            "services": {},
            "data": {"postgres":{"host":"db.example.com","port":5432,"db_name":"indexer"}},
            "secrets": {},
            "ingress": {}
        }))
        .expect("handoff must parse");

        assert_eq!(
            format_postgres_url(&handoff).as_deref(),
            Some("postgresql://db.example.com:5432/indexer")
        );
    }

    #[test]
    fn derives_k3s_logs_command_with_kubeconfig() {
        let handoff = parse_handoff_value(json!({
            "version":"v1",
            "mode":"external",
            "compute_engine":"k3s",
            "project_name":"demo",
            "runtime": {"ec2":null,"bare_metal":null,"eks":null,"k3s":{"host_ip":"54.1.2.3","kubeconfig_base64":"abc"}},
            "services": {},
            "data": {},
            "secrets": {},
            "ingress": {}
        }))
        .expect("handoff must parse");

        assert_eq!(
            logs_command(&handoff),
            "kubectl --kubeconfig=kubeconfig.yaml logs -n demo -l app=rindexer -f"
        );
    }

    #[test]
    fn computes_k3s_total_nodes_from_control_plane_and_workers() {
        let handoff = parse_handoff_value(json!({
            "version":"v1",
            "mode":"external",
            "compute_engine":"k3s",
            "project_name":"demo",
            "runtime": {
                "ec2": null,
                "bare_metal": null,
                "eks": null,
                "k3s": {
                    "host_ip":"54.1.2.3",
                    "kubeconfig_base64":"abc",
                    "node_name":"server-0",
                    "worker_nodes":[{"name":"worker-1"},{"name":"worker-2"}]
                }
            },
            "services": {},
            "data": {},
            "secrets": {},
            "ingress": {}
        }))
        .expect("handoff must parse");

        assert_eq!(k3s_total_nodes(&handoff), Some(3));
        assert_eq!(
            get_nodes_command(&handoff).as_deref(),
            Some("evm-cloud kubectl get nodes -o wide")
        );
    }
}
