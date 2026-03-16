use crate::config::schema::ComputeEngine;
use crate::output::ColorMode;

/// Paint text orange (ANSI yellow/33) when color is enabled.
fn orange(text: &str, mode: ColorMode) -> String {
    if matches!(mode, ColorMode::Never)
        || (matches!(mode, ColorMode::Auto)
            && !std::io::IsTerminal::is_terminal(&std::io::stderr()))
    {
        text.to_string()
    } else {
        format!("\x1b[33m{text}\x1b[0m")
    }
}

/// Map a raw `[evm-cloud] ...` message to a curated CLI status line.
/// Returns `None` for messages that should be suppressed.
pub(super) fn format_deploy_line(
    msg: &str,
    engine: ComputeEngine,
    color: ColorMode,
    rindexer_idx: &mut u32,
) -> Option<String> {
    let icon = match engine {
        ComputeEngine::K3s => "🛟",
        ComputeEngine::Ec2 | ComputeEngine::DockerCompose | ComputeEngine::Eks => "⛴️",
    };

    // --- k3s lines ---
    if msg == "Cluster reachable." {
        return Some("     ✔ k3s cluster reachable".to_string());
    }
    if msg == "ESO is ready." {
        return Some("     ✔ ESO is ready".to_string());
    }
    // ClusterSecretStore: <name> applied.
    if let Some(rest) = msg.strip_prefix("ClusterSecretStore ") {
        if let Some(name) = rest.strip_suffix(" applied.") {
            return Some(format!(
                "     {icon} ClusterSecretStore: {}",
                orange(name, color)
            ));
        }
    }
    if msg.starts_with("Cloudflare origin TLS secret created") {
        return Some("     ✔ Cloudflare origin TLS secret created".to_string());
    }
    // ingress-nginx
    if msg == "ingress-nginx installed." || msg == "ingress-nginx already present." {
        return Some("     ✔ ingress-nginx".to_string());
    }
    // cert-manager
    if msg == "cert-manager installed." || msg == "cert-manager CRDs already present." {
        return Some("     ✔ cert-manager".to_string());
    }
    // kube-prometheus-stack
    if msg == "kube-prometheus-stack installed." || msg == "kube-prometheus-stack already present."
    {
        return Some("     ✔ kube-prometheus-stack".to_string());
    }
    // Loki
    if msg == "Loki installed." || msg == "Loki already present." {
        return Some("     ✔ Loki".to_string());
    }
    // Promtail
    if msg == "Promtail installed." || msg == "Promtail already present." {
        return Some("     ✔ Promtail".to_string());
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
            return Some(format!(
                "     {icon} rindexer #{}: {}",
                rindexer_idx,
                orange(name, color)
            ));
        }
    }
    // Deploying custom service (<name>)...
    if let Some(rest) = msg.strip_prefix("Deploying custom service (") {
        if let Some(name) = rest.strip_suffix(")...") {
            return Some(format!(
                "     {icon} custom: {}",
                orange(name, color)
            ));
        }
    }
    // <name> deployed. (rindexer/custom service completion)
    if msg.ends_with(" deployed.") && !msg.starts_with("eRPC") {
        return None; // suppress completion echoes
    }
    if msg == "All workloads deployed successfully." {
        return None; // the CLI prints its own success banner
    }

    // --- compose/docker lines ---
    if msg == "SSH connectivity verified." {
        return Some("     ✔ SSH connectivity verified".to_string());
    }
    if msg == "Uploaded configs." {
        return Some("     ✔ Configs uploaded".to_string());
    }
    if msg == "Secrets pulled to .env" {
        return Some("     ✔ Secrets pulled".to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::ComputeEngine;
    use crate::output::ColorMode;

    fn fmt(msg: &str, engine: ComputeEngine, idx: &mut u32) -> Option<String> {
        format_deploy_line(msg, engine, ColorMode::Never, idx)
    }

    fn k3s(msg: &str, idx: &mut u32) -> Option<String> {
        fmt(msg, ComputeEngine::K3s, idx)
    }

    fn compose(msg: &str, idx: &mut u32) -> Option<String> {
        fmt(msg, ComputeEngine::DockerCompose, idx)
    }

    // --- K3s lines ---

    #[test]
    fn cluster_reachable() {
        let mut idx = 0;
        assert_eq!(k3s("Cluster reachable.", &mut idx), Some("     ✔ k3s cluster reachable".into()));
    }

    #[test]
    fn eso_is_ready() {
        let mut idx = 0;
        assert_eq!(k3s("ESO is ready.", &mut idx), Some("     ✔ ESO is ready".into()));
    }

    #[test]
    fn cluster_secret_store_applied() {
        let mut idx = 0;
        let out = k3s("ClusterSecretStore my-store applied.", &mut idx).unwrap();
        assert!(out.contains("ClusterSecretStore"), "expected ClusterSecretStore in: {out}");
        assert!(out.contains("my-store"), "expected my-store in: {out}");
    }

    #[test]
    fn cloudflare_origin_tls() {
        let mut idx = 0;
        let out = k3s("Cloudflare origin TLS secret created", &mut idx).unwrap();
        assert!(out.contains("Cloudflare"), "expected Cloudflare in: {out}");
    }

    #[test]
    fn ingress_nginx_installed() {
        let mut idx = 0;
        assert_eq!(k3s("ingress-nginx installed.", &mut idx), Some("     ✔ ingress-nginx".into()));
    }

    #[test]
    fn ingress_nginx_already_present() {
        let mut idx = 0;
        assert_eq!(k3s("ingress-nginx already present.", &mut idx), Some("     ✔ ingress-nginx".into()));
    }

    #[test]
    fn cert_manager_installed() {
        let mut idx = 0;
        assert_eq!(k3s("cert-manager installed.", &mut idx), Some("     ✔ cert-manager".into()));
    }

    #[test]
    fn cert_manager_crds_already_present() {
        let mut idx = 0;
        assert_eq!(k3s("cert-manager CRDs already present.", &mut idx), Some("     ✔ cert-manager".into()));
    }

    #[test]
    fn kube_prometheus_stack_installed() {
        let mut idx = 0;
        assert_eq!(
            k3s("kube-prometheus-stack installed.", &mut idx),
            Some("     ✔ kube-prometheus-stack".into())
        );
    }

    #[test]
    fn loki_installed() {
        let mut idx = 0;
        assert_eq!(k3s("Loki installed.", &mut idx), Some("     ✔ Loki".into()));
    }

    #[test]
    fn promtail_installed() {
        let mut idx = 0;
        assert_eq!(k3s("Promtail installed.", &mut idx), Some("     ✔ Promtail".into()));
    }

    #[test]
    fn dashboards_deployed() {
        let mut idx = 0;
        let out = k3s("Dashboards deployed.", &mut idx).unwrap();
        assert!(out.contains("Dashboards"), "expected Dashboards in: {out}");
    }

    #[test]
    fn deploying_erpc() {
        let mut idx = 0;
        let out = k3s("Deploying eRPC (test-erpc)...", &mut idx).unwrap();
        assert!(out.contains("eRPC"), "expected eRPC in: {out}");
        assert!(out.contains("test-erpc"), "expected test-erpc in: {out}");
    }

    #[test]
    fn erpc_deployed_suppressed() {
        let mut idx = 0;
        assert_eq!(k3s("eRPC deployed.", &mut idx), None);
    }

    #[test]
    fn deploying_rindexer_instance() {
        let mut idx = 0;
        let out = k3s("Deploying rindexer instance (my-indexer)...", &mut idx).unwrap();
        assert!(out.contains("rindexer #1"), "expected rindexer #1 in: {out}");
        assert!(out.contains("my-indexer"), "expected my-indexer in: {out}");
        assert_eq!(idx, 1);
    }

    #[test]
    fn two_rindexer_instances_sequential() {
        let mut idx = 0;
        let first = k3s("Deploying rindexer instance (idx-a)...", &mut idx).unwrap();
        assert!(first.contains("rindexer #1"), "expected #1 in: {first}");
        assert_eq!(idx, 1);

        let second = k3s("Deploying rindexer instance (idx-b)...", &mut idx).unwrap();
        assert!(second.contains("rindexer #2"), "expected #2 in: {second}");
        assert_eq!(idx, 2);
    }

    #[test]
    fn deploying_custom_service() {
        let mut idx = 0;
        let out = k3s("Deploying custom service (my-api)...", &mut idx).unwrap();
        assert!(out.contains("custom"), "expected custom in: {out}");
        assert!(out.contains("my-api"), "expected my-api in: {out}");
    }

    #[test]
    fn generic_deployed_suppressed() {
        let mut idx = 0;
        assert_eq!(k3s("my-indexer deployed.", &mut idx), None);
    }

    #[test]
    fn all_workloads_deployed_suppressed() {
        let mut idx = 0;
        assert_eq!(k3s("All workloads deployed successfully.", &mut idx), None);
    }

    // --- Compose lines ---

    #[test]
    fn ssh_connectivity_verified() {
        let mut idx = 0;
        assert_eq!(
            compose("SSH connectivity verified.", &mut idx),
            Some("     ✔ SSH connectivity verified".into())
        );
    }

    #[test]
    fn uploaded_configs() {
        let mut idx = 0;
        assert_eq!(compose("Uploaded configs.", &mut idx), Some("     ✔ Configs uploaded".into()));
    }

    #[test]
    fn secrets_pulled() {
        let mut idx = 0;
        assert_eq!(compose("Secrets pulled to .env", &mut idx), Some("     ✔ Secrets pulled".into()));
    }

    #[test]
    fn restarting_containers() {
        let mut idx = 0;
        let out = compose("Restarting containers...", &mut idx).unwrap();
        assert!(out.contains("Restarting"), "expected Restarting in: {out}");
    }

    #[test]
    fn verifying_containers() {
        let mut idx = 0;
        let out = compose("Verifying containers...", &mut idx).unwrap();
        assert!(out.contains("Verifying"), "expected Verifying in: {out}");
    }

    #[test]
    fn deploy_complete_suppressed() {
        let mut idx = 0;
        assert_eq!(compose("Deploy complete.", &mut idx), None);
    }

    // --- Suppression ---

    #[test]
    fn random_message_suppressed() {
        let mut idx = 0;
        assert_eq!(k3s("Waiting for pods...", &mut idx), None);
    }

    #[test]
    fn empty_string_suppressed() {
        let mut idx = 0;
        assert_eq!(k3s("", &mut idx), None);
    }
}
