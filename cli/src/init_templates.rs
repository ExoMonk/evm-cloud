use std::collections::BTreeMap;

use crate::init_answers::{DatabaseProfile, IndexerConfigStrategy, InitAnswers};

pub(crate) fn render_evm_cloud_toml(answers: &InitAnswers) -> String {
    let (database_mode, _) = map_database_profile(answers.database_profile);
    let database_provider = &answers.infrastructure_provider;

    let chains = answers
        .chains
        .iter()
        .map(|chain| format!("\"{chain}\""))
        .collect::<Vec<_>>()
        .join(", ");

    let endpoints = render_endpoints(&answers.rpc_endpoints);

    let indexer_config_path = match &answers.indexer_config {
        IndexerConfigStrategy::Generate => "config/rindexer.yaml".to_string(),
        IndexerConfigStrategy::Existing(path) => path.display().to_string(),
    };

    let erpc_line = if answers.generate_erpc_config {
        "erpc_config_path = \"config/erpc.yaml\"\n".to_string()
    } else {
        String::new()
    };

    let region_line = match &answers.region {
        Some(region) => format!("region = \"{region}\"\n"),
        None => String::new(),
    };

    let instance_type_line = match &answers.instance_type {
        Some(it) => format!("instance_type = \"{it}\"\n"),
        None => String::new(),
    };

    let secrets_mode = infer_secrets_mode(answers);
    let storage_backend = infer_storage_backend(answers);

    format!(
        "schema_version = 1\n\n[project]\nname = \"{}\"\n{}\n[compute]\nengine = \"{}\"\n{}\n[database]\nmode = \"{}\"\nprovider = \"{}\"\nstorage_backend = \"{}\"\n\n[indexer]\nconfig_path = \"{}\"\n{}chains = [{}]\n\n[rpc]\nendpoints = {{ {} }}\n\n[ingress]\nmode = \"none\"\n\n[secrets]\nmode = \"{}\"\n",
        answers.project_name,
        region_line,
        answers.compute_engine,
        instance_type_line,
        database_mode,
        database_provider,
        storage_backend,
        indexer_config_path,
        erpc_line,
        chains,
        endpoints,
        secrets_mode
    )
}

pub(crate) fn render_rindexer_yaml(answers: &InitAnswers) -> String {
    let first_chain = answers
        .chains
        .first()
        .cloned()
        .unwrap_or_else(|| "ethereum".to_string());

    let chain_id = chain_id_for(&first_chain).unwrap_or(1);

    let storage_block = if uses_postgres(answers) {
        "  postgres:\n    enabled: true"
    } else {
        "  clickhouse:\n    enabled: true"
    };

    let (contract_name, contract_address) = starter_contract_for(chain_id);

    format!(
        r#"name: {}
project_type: no-code
networks:
  - name: {}
    chain_id: {}
    rpc: ${{RPC_URL}}/main/evm/{}
storage:
{}
contracts:
  - name: {}
    details:
      - network: {}
        address: "{}"
    abi: ./abis/ERC20.json
    include_events:
      - Transfer
"#,
        answers.project_name,
        first_chain,
        chain_id,
        chain_id,
        storage_block,
        contract_name,
        first_chain,
        contract_address,
    )
}

/// Returns (contract_name, contract_address) for a starter ERC20 contract on the given chain.
fn starter_contract_for(chain_id: u64) -> (&'static str, &'static str) {
    match chain_id {
        8453 => ("BaseUSDC", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
        42161 => ("ArbUSDC", "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
        10 => ("OpUSDC", "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"),
        137 => ("PolygonUSDC", "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
        _ => ("USDC", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), // Ethereum mainnet
    }
}

/// Minimal ERC20 ABI covering the Transfer event (+ Approval for completeness).
pub(crate) fn erc20_abi_json() -> &'static str {
    r#"[
  {
    "anonymous": false,
    "inputs": [
      { "indexed": true, "name": "from", "type": "address" },
      { "indexed": true, "name": "to", "type": "address" },
      { "indexed": false, "name": "value", "type": "uint256" }
    ],
    "name": "Transfer",
    "type": "event"
  },
  {
    "anonymous": false,
    "inputs": [
      { "indexed": true, "name": "owner", "type": "address" },
      { "indexed": true, "name": "spender", "type": "address" },
      { "indexed": false, "name": "value", "type": "uint256" }
    ],
    "name": "Approval",
    "type": "event"
  }
]
"#
}

pub(crate) fn render_erpc_yaml(answers: &InitAnswers) -> String {
    let first_chain = answers
        .chains
        .first()
        .cloned()
        .unwrap_or_else(|| "ethereum".to_string());

    let chain_id = chain_id_for(&first_chain).unwrap_or(1);

    let first_endpoint = answers
        .rpc_endpoints
        .get(&first_chain)
        .cloned()
        .unwrap_or_else(|| "https://ethereum-rpc.publicnode.com".to_string());

    format!(
        "logLevel: warn\nprojects:\n  - id: main\n    networks:\n      - architecture: evm\n        evm:\n          chainId: {}\n    upstreams:\n      - id: primary\n        endpoint: {}\n        type: evm\nserver:\n  listenV4: true\n  httpHostV4: 0.0.0.0\n  httpPort: 4000\n",
        chain_id, first_endpoint
    )
}

fn chain_id_for(chain: &str) -> Option<u64> {
    let normalized = chain.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "ethereum" | "eth" | "mainnet" => Some(1),
        "polygon" | "matic" => Some(137),
        "arbitrum" | "arbitrum_one" => Some(42161),
        "base" => Some(8453),
        "optimism" | "op" => Some(10),
        "hyperliquid" | "hyperliquid_mainnet" | "hyperevm" => Some(999),
        _ => None,
    }
}

pub(crate) fn render_versions_tf() -> String {
    "terraform {\n  required_version = \">= 1.14.6\"\n}\n".to_string()
}

pub(crate) fn render_main_tf(answers: &InitAnswers) -> String {
    let is_bare_metal = answers.infrastructure_provider == "bare_metal";
    let is_postgres = uses_postgres(answers);
    let engine = answers.compute_engine.as_str();

    let mut lines = vec![
        "module \"evm_cloud\" {".to_string(),
        format!("  source = \"{}\"", crate::module_source()),
        String::new(),
        "  project_name            = var.project_name".to_string(),
        "  infrastructure_provider = var.infrastructure_provider".to_string(),
        "  database_mode           = var.database_mode".to_string(),
        "  compute_engine          = var.compute_engine".to_string(),
        "  workload_mode           = var.workload_mode".to_string(),
        "  secrets_mode            = var.secrets_mode".to_string(),
        "  ingress_mode            = var.ingress_mode".to_string(),
        "  erpc_hostname           = var.erpc_hostname".to_string(),
        "  ingress_tls_email       = var.ingress_tls_email".to_string(),
    ];

    // Provider-specific infra
    if is_bare_metal {
        lines.push(String::new());
        lines.push("  bare_metal_host                 = var.bare_metal_host".to_string());
        lines.push("  bare_metal_ssh_user             = var.bare_metal_ssh_user".to_string());
        lines.push("  bare_metal_ssh_private_key_path = var.bare_metal_ssh_private_key_path".to_string());
        lines.push("  bare_metal_ssh_port             = var.bare_metal_ssh_port".to_string());
    } else {
        lines.push("  networking_enabled      = var.networking_enabled".to_string());
        lines.push("  aws_region              = var.aws_region".to_string());
        lines.push("  ssh_public_key          = var.ssh_public_key".to_string());
        match engine {
            "ec2" => {
                lines.push("  ec2_instance_type       = var.ec2_instance_type".to_string());
                lines.push("  ec2_ssh_private_key_path = var.ec2_ssh_private_key_path".to_string());
            }
            "k3s" => {
                lines.push("  k3s_instance_type        = var.k3s_instance_type".to_string());
                lines.push("  k3s_ssh_private_key_path = var.k3s_ssh_private_key_path".to_string());
                lines.push("  k3s_api_allowed_cidrs    = var.k3s_api_allowed_cidrs".to_string());
            }
            _ => {}
        }
    }

    // Database / storage
    lines.push(String::new());
    lines.push("  indexer_storage_backend = var.indexer_storage_backend".to_string());
    if is_postgres {
        lines.push("  postgres_enabled        = var.postgres_enabled".to_string());
        lines.push("  indexer_postgres_url    = var.indexer_postgres_url".to_string());
    } else {
        lines.push("  indexer_clickhouse_url      = var.indexer_clickhouse_url".to_string());
        lines.push("  indexer_clickhouse_user     = var.indexer_clickhouse_user".to_string());
        lines.push("  indexer_clickhouse_password = var.indexer_clickhouse_password".to_string());
        lines.push("  indexer_clickhouse_db       = var.indexer_clickhouse_db".to_string());
    }

    // Indexer / RPC
    lines.push(String::new());
    lines.push("  rpc_proxy_enabled    = var.rpc_proxy_enabled".to_string());
    lines.push("  indexer_enabled      = var.indexer_enabled".to_string());
    lines.push("  indexer_rpc_url      = var.indexer_rpc_url".to_string());
    lines.push("  erpc_config_yaml     = file(var.erpc_config_yaml)".to_string());
    lines.push("  rindexer_config_yaml = file(var.rindexer_config_yaml)".to_string());
    lines.push("  rindexer_abis        = { for f in fileset(\"${path.module}/config/abis\", \"*.json\") : f => file(\"${path.module}/config/abis/${f}\") }".to_string());
    lines.push("}".to_string());

    format!("{}\n", lines.join("\n"))
}

pub(crate) fn render_outputs_tf() -> String {
    "output \"workload_handoff\" {\n  value     = module.evm_cloud.workload_handoff\n  sensitive = true\n}\n"
        .to_string()
}

pub(crate) fn render_variables_tf(answers: &InitAnswers) -> String {
    let is_bare_metal = answers.infrastructure_provider == "bare_metal";
    let is_postgres = uses_postgres(answers);
    let secrets_mode = infer_secrets_mode(answers);
    let workload_mode = infer_workload_mode(answers);
    let storage_backend = infer_storage_backend(answers);
    let engine = answers.compute_engine.as_str();

    let mut blocks: Vec<String> = Vec::new();

    // Core
    blocks.push(tf_var("project_name", "string", &answers.project_name));
    blocks.push(tf_var("infrastructure_provider", "string", &answers.infrastructure_provider));
    blocks.push(tf_var("database_mode", "string", "self_hosted"));
    blocks.push(tf_var("compute_engine", "string", &answers.compute_engine));
    blocks.push(tf_var("workload_mode", "string", &workload_mode));
    blocks.push(tf_var("secrets_mode", "string", &secrets_mode));
    blocks.push(tf_var("ingress_mode", "string", "none"));
    blocks.push(tf_var("erpc_hostname", "string", ""));
    blocks.push(tf_var("ingress_tls_email", "string", ""));

    // Provider-specific infra
    if is_bare_metal {
        blocks.push(tf_var_sensitive("bare_metal_host"));
        blocks.push(tf_var_sensitive("bare_metal_ssh_private_key_path"));
            blocks.push(tf_var("bare_metal_ssh_user", "string", "ubuntu"));
            blocks.push(tf_var_number("bare_metal_ssh_port", 22));
    } else {
        let region = answers.region.as_deref().unwrap_or("us-east-1");
        let instance_type = answers.instance_type.as_deref().unwrap_or("");
        blocks.push(tf_var_bool("networking_enabled", true));
        blocks.push(tf_var("aws_region", "string", region));
        blocks.push(tf_var_sensitive("ssh_public_key"));
        match engine {
            "ec2" => {
                blocks.push(tf_var("ec2_instance_type", "string", instance_type));
                blocks.push(tf_var_sensitive("ec2_ssh_private_key_path"));
            }
            "k3s" => {
                blocks.push(tf_var("k3s_instance_type", "string", instance_type));
                blocks.push(tf_var_sensitive("k3s_ssh_private_key_path"));
                blocks.push(tf_var_list("k3s_api_allowed_cidrs", "string"));
            }
            _ => {}
        }
    }

    // Database / storage
    blocks.push(tf_var("indexer_storage_backend", "string", storage_backend));
    if is_postgres {
        blocks.push(tf_var_bool("postgres_enabled", true));
        blocks.push(tf_var_sensitive("indexer_postgres_url"));
    } else {
        blocks.push(tf_var_sensitive("indexer_clickhouse_url"));
        blocks.push(tf_var("indexer_clickhouse_user", "string", "default"));
        blocks.push(tf_var_sensitive("indexer_clickhouse_password"));
        blocks.push(tf_var("indexer_clickhouse_db", "string", "rindexer"));
    }

    // Indexer / RPC
    blocks.push(tf_var_bool("rpc_proxy_enabled", true));
    blocks.push(tf_var_bool("indexer_enabled", true));
    blocks.push(tf_var("indexer_rpc_url", "string", "http://erpc:4000"));
    blocks.push(tf_var("erpc_config_yaml", "string", "config/erpc.yaml"));
    blocks.push(tf_var("rindexer_config_yaml", "string", "config/rindexer.yaml"));

    format!("{}\n", blocks.join("\n\n"))
}

fn infer_storage_backend(answers: &InitAnswers) -> &'static str {
    match answers.database_profile {
        DatabaseProfile::ByodbPostgres | DatabaseProfile::ManagedRds => "postgres",
        DatabaseProfile::ByodbClickhouse | DatabaseProfile::ManagedClickhouse => "clickhouse",
    }
}

fn uses_postgres(answers: &InitAnswers) -> bool {
    matches!(
        answers.database_profile,
        DatabaseProfile::ByodbPostgres | DatabaseProfile::ManagedRds
    )
}

fn infer_secrets_mode(answers: &InitAnswers) -> String {
    if answers.infrastructure_provider == "bare_metal" || answers.compute_engine == "k3s" {
        "inline".to_string()
    } else {
        "provider".to_string()
    }
}

fn infer_workload_mode(answers: &InitAnswers) -> String {
    if let Some(ref wm) = answers.workload_mode {
        return wm.clone();
    }
    match answers.compute_engine.as_str() {
        "k3s" | "eks" => "external".to_string(),
        _ => "terraform".to_string(),
    }
}

fn tf_var(name: &str, ty: &str, default: &str) -> String {
    format!(
        "variable \"{}\" {{\n  type    = {}\n  default = \"{}\"\n}}",
        name, ty, default
    )
}

fn tf_var_sensitive(name: &str) -> String {
    format!(
        "variable \"{}\" {{\n  type      = string\n  default   = \"\"\n  sensitive = true\n}}",
        name
    )
}

fn tf_var_bool(name: &str, default: bool) -> String {
    format!(
        "variable \"{}\" {{\n  type    = bool\n  default = {}\n}}",
        name, default
    )
}

fn tf_var_list(name: &str, element_ty: &str) -> String {
    format!(
        "variable \"{}\" {{\n  type    = list({})\n  default = []\n}}",
        name, element_ty
    )
}

pub(crate) fn render_secrets_example(answers: &InitAnswers) -> String {
    let mut lines = Vec::new();
    lines.push("# Required secrets for your configuration.".to_string());
    lines.push("# Fill in the values below — this file is gitignored.".to_string());
    lines.push("# A copy is kept at secrets.auto.tfvars.example for reference.".to_string());
    lines.push(String::new());

    let is_bare_metal = answers.infrastructure_provider == "bare_metal";
    let engine = answers.compute_engine.as_str();

    // --- SSH / host access ---
    if is_bare_metal {
        lines.push("# Bare metal host access".to_string());
        lines.push(r#"bare_metal_host             = ""  # IP or hostname of the target server"#.to_string());
        lines.push(r#"bare_metal_ssh_private_key_path = "~/.ssh/id_rsa""#.to_string());
        lines.push(r#"bare_metal_ssh_user         = "ubuntu"  # change to root/ec2-user if your host differs"#.to_string());
        lines.push(r#"bare_metal_ssh_port         = 22"#.to_string());
        lines.push(String::new());
    } else {
        match engine {
            "ec2" => {
                lines.push("# EC2 SSH access".to_string());
                lines.push(r#"ssh_public_key             = ""  # contents of ~/.ssh/id_rsa.pub"#.to_string());
                lines.push(r#"ec2_ssh_private_key_path   = "~/.ssh/id_rsa""#.to_string());
                lines.push(String::new());
            }
            "k3s" => {
                lines.push("# K3s SSH access".to_string());
                lines.push(r#"ssh_public_key             = ""  # contents of ~/.ssh/id_rsa.pub"#.to_string());
                lines.push(r#"k3s_ssh_private_key_path   = "~/.ssh/id_rsa""#.to_string());
                lines.push(r#"k3s_api_allowed_cidrs      = ["0.0.0.0/0"]  # restrict to your IP in production"#.to_string());
                lines.push(String::new());
            }
            "eks" => {
                // EKS manages node access — no SSH keys needed
            }
            _ => {}
        }
    }

    // --- Database credentials ---
    match answers.database_profile {
        DatabaseProfile::ByodbClickhouse | DatabaseProfile::ManagedClickhouse => {
            lines.push("# ClickHouse credentials (BYODB)".to_string());
            lines.push(r#"indexer_clickhouse_url      = ""  # e.g. clickhouse://host:9000/default"#.to_string());
            lines.push(r#"indexer_clickhouse_password = """#.to_string());
            lines.push(String::new());
        }
        DatabaseProfile::ByodbPostgres => {
            if is_bare_metal {
                lines.push("# Postgres credentials (BYODB)".to_string());
                lines.push(r#"indexer_postgres_url       = ""  # e.g. postgres://user:pass@host:5432/db"#.to_string());
                lines.push(String::new());
            } else {
                lines.push("# Postgres credentials (BYODB)".to_string());
                lines.push(r#"indexer_postgres_url       = ""  # e.g. postgres://user:pass@host:5432/db"#.to_string());
                lines.push(String::new());
            }
        }
        DatabaseProfile::ManagedRds => {
            // RDS uses AWS-managed master password by default — no secret needed
        }
    }

    // Trailing newline
    if !lines.last().map_or(false, |l| l.is_empty()) {
        lines.push(String::new());
    }

    lines.join("\n")
}

fn tf_var_number(name: &str, default: i64) -> String {
    format!(
        "variable \"{}\" {{\n  type    = number\n  default = {}\n}}",
        name, default
    )
}
fn map_database_profile(profile: DatabaseProfile) -> (&'static str, &'static str) {
    match profile {
        DatabaseProfile::ByodbClickhouse => ("self_hosted", "aws"),
        DatabaseProfile::ByodbPostgres => ("self_hosted", "aws"),
        DatabaseProfile::ManagedRds => ("managed", "aws"),
        DatabaseProfile::ManagedClickhouse => ("managed", "aws"),
    }
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

fn render_endpoints(endpoints: &BTreeMap<String, String>) -> String {
    endpoints
        .iter()
        .map(|(chain, endpoint)| format!("{} = \"{}\"", chain, endpoint))
        .collect::<Vec<_>>()
        .join(", ")
}
