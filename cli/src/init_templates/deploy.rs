use crate::config::schema::{ComputeEngine, IngressMode};
use crate::init_answers::{DatabaseProfile, InitAnswers};

pub(crate) fn render_secrets_example(answers: &InitAnswers) -> String {
    let mut lines = vec![
        "# Required secrets for your configuration.".to_string(),
        "# Fill in the values below — this file is gitignored.".to_string(),
        "# A copy is kept at secrets.auto.tfvars.example for reference.".to_string(),
        String::new(),
    ];

    let is_bare_metal = answers.infrastructure_provider.is_bare_metal();
    let engine = answers.compute_engine;

    // --- SSH / host access ---
    if is_bare_metal {
        lines.push("# Bare metal host access".to_string());
        lines.push(
            r#"bare_metal_host             = ""  # IP or hostname of the target server"#
                .to_string(),
        );
        lines.push(r#"ssh_private_key_path        = "~/.ssh/id_rsa""#.to_string());
        lines.push(r#"bare_metal_ssh_user         = "ubuntu"  # change to root/ec2-user if your host differs"#.to_string());
        lines.push(r#"bare_metal_ssh_port         = 22"#.to_string());
        lines.push(String::new());
    } else {
        match engine {
            ComputeEngine::Ec2 => {
                lines.push("# EC2 SSH access".to_string());
                lines.push(
                    r#"ssh_public_key             = ""  # contents of ~/.ssh/id_rsa.pub"#
                        .to_string(),
                );
                lines.push(r#"ssh_private_key_path       = "~/.ssh/id_rsa""#.to_string());
                lines.push(String::new());
            }
            ComputeEngine::K3s => {
                lines.push("# K3s SSH access".to_string());
                lines.push(
                    r#"ssh_public_key             = ""  # contents of ~/.ssh/id_rsa.pub"#
                        .to_string(),
                );
                lines.push(r#"ssh_private_key_path       = "~/.ssh/id_rsa""#.to_string());
                lines.push(r#"k3s_api_allowed_cidrs      = ["0.0.0.0/0"]  # restrict to your IP in production"#.to_string());
                lines.push(String::new());
            }
            ComputeEngine::Eks => {
                // EKS manages node access — no SSH keys needed
            }
            ComputeEngine::DockerCompose => {}
        }
    }

    // --- Database credentials ---
    match answers.database_profile {
        DatabaseProfile::ByodbClickhouse | DatabaseProfile::ManagedClickhouse => {
            lines.push("# ClickHouse credentials (BYODB)".to_string());
            lines.push(
                r#"indexer_clickhouse_url      = ""  # e.g. clickhouse://host:9000/default"#
                    .to_string(),
            );
            lines.push(r#"indexer_clickhouse_password = """#.to_string());
            lines.push(String::new());
        }
        DatabaseProfile::ByodbPostgres => {
            lines.push("# Postgres credentials (BYODB)".to_string());
            lines.push(
                r#"indexer_postgres_url       = ""  # e.g. postgres://user:pass@host:5432/db"#
                    .to_string(),
            );
            lines.push(String::new());
        }
        DatabaseProfile::ManagedRds => {
            // RDS uses AWS-managed master password by default — no secret needed
        }
    }

    // --- Ingress credentials ---
    if answers.ingress_mode == IngressMode::Cloudflare {
        lines.push("# Cloudflare origin certificates".to_string());
        lines.push(r#"ingress_cloudflare_origin_cert = ""  # PEM cert from Cloudflare dashboard > SSL/TLS > Origin Server"#.to_string());
        lines.push(r#"ingress_cloudflare_origin_key  = ""  # PEM key from Cloudflare dashboard > SSL/TLS > Origin Server"#.to_string());
        lines.push(String::new());
    }

    // Trailing newline
    if !lines.last().is_some_and(|l| l.is_empty()) {
        lines.push(String::new());
    }

    lines.join("\n")
}

pub(crate) fn render_docker_compose_yml(answers: &InitAnswers) -> String {
    let has_erpc = answers.generate_erpc_config;
    let mut lines = vec!["services:".to_string()];

    if has_erpc {
        lines.extend([
            "  erpc:".to_string(),
            "    image: ghcr.io/erpc/erpc:latest".to_string(),
            "    container_name: erpc".to_string(),
            "    restart: unless-stopped".to_string(),
            "    ports:".to_string(),
            "      - \"4000:4000\"".to_string(),
            "    volumes:".to_string(),
            "      - /opt/evm-cloud/config/erpc.yaml:/config/erpc.yaml:ro".to_string(),
            "    command: [\"/erpc-server\", \"--config\", \"/config/erpc.yaml\"]".to_string(),
            "    env_file:".to_string(),
            "      - /opt/evm-cloud/.env".to_string(),
            "    deploy:".to_string(),
            "      resources:".to_string(),
            "        limits:".to_string(),
            "          memory: 1g".to_string(),
            "    networks:".to_string(),
            "      - evm-cloud".to_string(),
            String::new(),
        ]);
    }

    lines.extend([
        "  rindexer:".to_string(),
        "    image: ghcr.io/joshstevens19/rindexer:latest".to_string(),
        "    container_name: rindexer".to_string(),
        "    restart: unless-stopped".to_string(),
        "    volumes:".to_string(),
        "      - /opt/evm-cloud/config:/config:ro".to_string(),
        "    command: [\"start\", \"--path\", \"/config\", \"all\"]".to_string(),
        "    env_file:".to_string(),
        "      - /opt/evm-cloud/.env".to_string(),
        "    deploy:".to_string(),
        "      resources:".to_string(),
        "        limits:".to_string(),
        "          memory: 1g".to_string(),
    ]);

    if has_erpc {
        lines.extend([
            "    depends_on:".to_string(),
            "      erpc:".to_string(),
            "        condition: service_started".to_string(),
        ]);
    }

    lines.extend([
        "    networks:".to_string(),
        "      - evm-cloud".to_string(),
        String::new(),
        "networks:".to_string(),
        "  evm-cloud:".to_string(),
        "    driver: bridge".to_string(),
        String::new(),
    ]);

    lines.join("\n")
}
