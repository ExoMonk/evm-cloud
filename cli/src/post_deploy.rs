use serde_json::Value;

use crate::handoff::WorkloadHandoff;
use crate::output::ColorMode;

struct SummaryRow {
    label: &'static str,
    value: String,
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
}

fn value_str_at(root: &Value, path: &[&str]) -> Option<String> {
    let mut cursor = root;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    non_empty(cursor.as_str())
}

fn build_https_url(raw: String) -> String {
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

fn format_postgres_url(handoff: &WorkloadHandoff) -> Option<String> {
    let host = value_str_at(&handoff.data, &["postgres", "host"])?;
    let port = value_str_at(&handoff.data, &["postgres", "port"]).unwrap_or_else(|| "5432".to_string());
    let db_name = value_str_at(&handoff.data, &["postgres", "db_name"])?;
    Some(format!("postgresql://{host}:{port}/{db_name}"))
}

fn erpc_url(handoff: &WorkloadHandoff) -> Option<String> {
    value_str_at(&handoff.ingress, &["erpc_hostname"])
        .map(build_https_url)
        .or_else(|| value_str_at(&handoff.services, &["rpc_proxy", "internal_url"]))
}

fn grafana_line(handoff: &WorkloadHandoff) -> Option<String> {
    let url = value_str_at(&handoff.services, &["monitoring", "grafana_hostname"])
        .map(build_https_url)?;
    let has_custom_secret =
        value_str_at(&handoff.services, &["monitoring", "grafana_admin_password_secret_name"])
            .map_or(false, |s| !s.is_empty());
    if has_custom_secret {
        Some(format!("{url} (admin/<custom-secret>)"))
    } else {
        Some(format!("{url} (admin/prom-operator)"))
    }
}

fn aws_region(handoff: &WorkloadHandoff) -> Option<String> {
    handoff
        .extra
        .get("aws_region")
        .and_then(|value| value.as_str())
        .and_then(|value| non_empty(Some(value)))
}

fn ssh_user_for(engine: &str) -> &str {
    match engine {
        "ec2" => "ec2-user",
        _ => "ubuntu",
    }
}

fn server_target(handoff: &WorkloadHandoff) -> Option<String> {
    let user = ssh_user_for(&handoff.compute_engine);

    if handoff.compute_engine == "k3s" {
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

fn logs_command(handoff: &WorkloadHandoff) -> String {
    match handoff.compute_engine.as_str() {
        "k3s" => {
            if handoff
                .runtime
                .k3s
                .as_ref()
                .and_then(|k3s| non_empty(k3s.kubeconfig_base64.as_deref()))
                .is_some()
            {
                "kubectl --kubeconfig=kubeconfig.yaml logs -n evm-cloud -l app=rindexer -f".to_string()
            } else {
                "kubectl logs -n evm-cloud -l app=rindexer -f".to_string()
            }
        }
        "eks" => "kubectl logs -n evm-cloud -l app=rindexer -f".to_string(),
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
    (handoff.compute_engine == "k3s" && has_k3s_kubeconfig) || handoff.compute_engine == "eks"
}

fn k3s_total_nodes(handoff: &WorkloadHandoff) -> Option<usize> {
    if handoff.compute_engine != "k3s" {
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
    if handoff.compute_engine != "k3s" {
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
    let mut rows: Vec<SummaryRow> = Vec::new();

    if handoff.compute_engine == "eks" {
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

    if value_str_at(&handoff.data, &["backend"]).as_deref() == Some("clickhouse") {
        push_row(
            &mut rows,
            "ClickHouse",
            value_str_at(&handoff.data, &["clickhouse", "url"]),
        );
    }

    if value_str_at(&handoff.data, &["backend"]).as_deref() == Some("postgres") {
        push_row(&mut rows, "Postgres", format_postgres_url(handoff));
    }

    let has_k3s_kubeconfig = handoff
        .runtime
        .k3s
        .as_ref()
        .and_then(|runtime| non_empty(runtime.kubeconfig_base64.as_deref()))
        .is_some();

    if uses_kubectl_wrapper(handoff, has_k3s_kubeconfig) {
        push_row(
            &mut rows,
            "Kubeconfig commands",
            Some(String::new()),
        );
    }

    if let Some(total_nodes) = k3s_total_nodes(handoff) {
        push_row(&mut rows, "Nodes", Some(format!("{total_nodes} node{}", if total_nodes == 1 { "" } else { "s" })));
        push_row(&mut rows, "Get nodes", get_nodes_command(handoff));
    }

    if handoff.compute_engine == "k3s" && !has_k3s_kubeconfig {
        push_row(
            &mut rows,
            "Pods",
            Some("kubectl get pods -n evm-cloud".to_string()),
        );
    } else if handoff.compute_engine == "eks" && !uses_kubectl_wrapper(handoff, has_k3s_kubeconfig) {
        push_row(
            &mut rows,
            "Pods",
            Some("kubectl get pods -n evm-cloud".to_string()),
        );
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
            eprintln!(
                "         🛟 Pods      evm-cloud kubectl get pods -n evm-cloud"
            );
            eprintln!(
                "         🛟 Logs      evm-cloud kubectl logs -n evm-cloud -l app=rindexer -f"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::handoff::parse_handoff_value;

    use super::{erpc_url, format_postgres_url, get_nodes_command, k3s_total_nodes, logs_command, server_target};

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

        assert_eq!(erpc_url(&handoff).as_deref(), Some("https://rpc.example.com"));
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
            "kubectl --kubeconfig=kubeconfig.yaml logs -n evm-cloud -l app=rindexer -f"
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
