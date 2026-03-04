use std::collections::BTreeMap;

use crate::init_answers::{DatabaseProfile, IndexerConfigStrategy, InitAnswers};

pub(crate) fn render_evm_cloud_toml(answers: &InitAnswers) -> String {
    let (database_mode, database_provider) = map_database_profile(answers.database_profile);

    let chains = answers
        .chains
        .iter()
        .map(|chain| format!("\"{chain}\""))
        .collect::<Vec<_>>()
        .join(", ");

    let endpoints = render_endpoints(&answers.rpc_endpoints);

    let indexer_config_path = match &answers.indexer_config {
        IndexerConfigStrategy::Generate => "rindexer.yaml".to_string(),
        IndexerConfigStrategy::Existing(path) => path.display().to_string(),
    };

    let erpc_line = if answers.generate_erpc_config {
        "erpc_config_path = \"erpc.yaml\"\n".to_string()
    } else {
        String::new()
    };

    format!(
        "schema_version = 1\n\n[project]\nname = \"{}\"\nregion = \"{}\"\n\n[compute]\nengine = \"{}\"\ninstance_type = \"{}\"\n\n[database]\nmode = \"{}\"\nprovider = \"{}\"\n\n[indexer]\nconfig_path = \"{}\"\n{}chains = [{}]\n\n[rpc]\nendpoints = {{ {} }}\n\n[ingress]\nmode = \"none\"\n\n[secrets]\nmode = \"provider\"\n",
        answers.project_name,
        answers.region,
        answers.compute_engine,
        answers.instance_type,
        database_mode,
        database_provider,
        indexer_config_path,
        erpc_line,
        chains,
        endpoints
    )
}

pub(crate) fn render_rindexer_yaml(answers: &InitAnswers) -> String {
    let first_chain = answers
        .chains
        .first()
        .cloned()
        .unwrap_or_else(|| "ethereum".to_string());

    format!(
        "name: {}\nproject_type: no-code\nnetworks:\n  - name: {}\n    chain_id: 1\n    rpc: ${{RPC_URL}}/main/evm/1\nstorage:\n  clickhouse:\n    enabled: true\ncontracts: []\n",
        answers.project_name, first_chain
    )
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

pub(crate) fn render_main_tf() -> String {
    "module \"evm_cloud\" {\n  source = \"git::https://github.com/ExoMonk/evm-cloud.git?ref=v0.0.1.alpha\"\n\n  project_name            = var.project_name\n  infrastructure_provider = var.infrastructure_provider\n  database_mode           = var.database_mode\n  ingress_mode            = var.ingress_mode\n  erpc_hostname           = var.erpc_hostname\n  ingress_tls_email       = var.ingress_tls_email\n  compute_engine          = var.compute_engine\n  ec2_instance_type       = var.ec2_instance_type\n  aws_region              = var.aws_region\n  secrets_mode            = var.secrets_mode\n\n  rpc_proxy_enabled    = var.rpc_proxy_enabled\n  indexer_enabled      = var.indexer_enabled\n  indexer_rpc_url      = var.indexer_rpc_url\n  erpc_config_yaml     = file(var.erpc_config_yaml)\n  rindexer_config_yaml = file(var.rindexer_config_yaml)\n}\n"
        .to_string()
}

pub(crate) fn render_outputs_tf() -> String {
    "output \"workload_handoff\" {\n  value     = module.evm_cloud.workload_handoff\n  sensitive = true\n}\n"
        .to_string()
}

pub(crate) fn render_variables_tf(answers: &InitAnswers) -> String {
    format!(
        "variable \"project_name\" {{\n  type    = string\n  default = \"{}\"\n}}\n\nvariable \"infrastructure_provider\" {{\n  type    = string\n  default = \"aws\"\n}}\n\nvariable \"database_mode\" {{\n  type    = string\n  default = \"self_hosted\"\n}}\n\nvariable \"ingress_mode\" {{\n  type    = string\n  default = \"none\"\n}}\n\nvariable \"erpc_hostname\" {{\n  type    = string\n  default = \"\"\n}}\n\nvariable \"ingress_tls_email\" {{\n  type    = string\n  default = \"\"\n}}\n\nvariable \"compute_engine\" {{\n  type    = string\n  default = \"{}\"\n}}\n\nvariable \"ec2_instance_type\" {{\n  type    = string\n  default = \"{}\"\n}}\n\nvariable \"aws_region\" {{\n  type    = string\n  default = \"{}\"\n}}\n\nvariable \"secrets_mode\" {{\n  type    = string\n  default = \"provider\"\n}}\n\nvariable \"rpc_proxy_enabled\" {{\n  type    = bool\n  default = true\n}}\n\nvariable \"indexer_enabled\" {{\n  type    = bool\n  default = true\n}}\n\nvariable \"indexer_rpc_url\" {{\n  type    = string\n  default = \"http://erpc:4000\"\n}}\n\nvariable \"erpc_config_yaml\" {{\n  type    = string\n  default = \"erpc.yaml\"\n}}\n\nvariable \"rindexer_config_yaml\" {{\n  type    = string\n  default = \"rindexer.yaml\"\n}}\n",
        answers.project_name, answers.compute_engine, answers.instance_type, answers.region
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

fn render_endpoints(endpoints: &BTreeMap<String, String>) -> String {
    endpoints
        .iter()
        .map(|(chain, endpoint)| format!("{} = \"{}\"", chain, endpoint))
        .collect::<Vec<_>>()
        .join(", ")
}
