use crate::config::schema::ComputeEngine;
use crate::output::ColorMode;

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
pub(super) fn format_deploy_line(msg: &str, engine: ComputeEngine, color: ColorMode, rindexer_idx: &mut u32) -> Option<String> {
    let icon = match engine {
        ComputeEngine::K3s => "🛟",
        ComputeEngine::Ec2
        | ComputeEngine::DockerCompose
        | ComputeEngine::Eks => "⛴️",
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
