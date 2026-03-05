use std::path::Path;

use crate::codegen::write_atomic;
use crate::config::schema::EvmCloudConfig;
use crate::error::Result;

const GENERATED_DIR: &str = ".evm-cloud";

pub(crate) fn generate_main_tf(config: &EvmCloudConfig, project_root: &Path) -> Result<()> {
    let module_source = crate::module_source();
    let is_bare_metal = config.database.provider == "bare_metal";
    let is_postgres = config.database.storage_backend == "postgres";
    let engine = config.compute.engine.as_str();

    let mut module_lines = vec![
        format!("  source = \"{module_source}\""),
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
        module_lines.push(String::new());
        module_lines.push("  bare_metal_host                 = var.bare_metal_host".to_string());
        module_lines.push("  bare_metal_ssh_private_key_path = var.bare_metal_ssh_private_key_path".to_string());
        module_lines.push("  bare_metal_ssh_user             = var.bare_metal_ssh_user".to_string());
        module_lines.push("  bare_metal_ssh_port             = var.bare_metal_ssh_port".to_string());
    } else {
        module_lines.push("  networking_enabled      = var.networking_enabled".to_string());
        module_lines.push("  aws_region              = var.aws_region".to_string());
        module_lines.push("  ssh_public_key          = var.ssh_public_key".to_string());
        match engine {
            "ec2" => {
                module_lines.push("  ec2_instance_type       = var.ec2_instance_type".to_string());
                module_lines.push("  ec2_ssh_private_key_path = var.ec2_ssh_private_key_path".to_string());
            }
            "k3s" => {
                module_lines.push("  k3s_instance_type        = var.k3s_instance_type".to_string());
                module_lines.push("  k3s_ssh_private_key_path = var.k3s_ssh_private_key_path".to_string());
                module_lines.push("  k3s_api_allowed_cidrs    = var.k3s_api_allowed_cidrs".to_string());
            }
            _ => {}
        }
    }

    // Database / storage
    module_lines.push(String::new());
    module_lines.push("  indexer_storage_backend = var.indexer_storage_backend".to_string());
    if is_postgres {
        module_lines.push("  postgres_enabled        = var.postgres_enabled".to_string());
        module_lines.push("  indexer_postgres_url    = var.indexer_postgres_url".to_string());
    } else {
        module_lines.push("  indexer_clickhouse_url      = var.indexer_clickhouse_url".to_string());
        module_lines.push("  indexer_clickhouse_user     = var.indexer_clickhouse_user".to_string());
        module_lines.push("  indexer_clickhouse_password = var.indexer_clickhouse_password".to_string());
        module_lines.push("  indexer_clickhouse_db       = var.indexer_clickhouse_db".to_string());
    }

    // Indexer / RPC
    module_lines.push(String::new());
    module_lines.push("  rpc_proxy_enabled   = var.rpc_proxy_enabled".to_string());
    module_lines.push("  indexer_enabled     = var.indexer_enabled".to_string());
    module_lines.push("  indexer_rpc_url     = var.indexer_rpc_url".to_string());
    module_lines.push("  erpc_config_yaml    = var.erpc_config_yaml".to_string());
    module_lines.push("  rindexer_config_yaml = var.rindexer_config_yaml".to_string());
    module_lines.push("  rindexer_abis        = var.rindexer_abis".to_string());

    let module_body = module_lines.join("\n");

    let contents = format!(
        "terraform {{\n  required_version = \">= 1.14.6\"\n}}\n\nmodule \"evm_cloud\" {{\n{module_body}\n}}\n"
    );

    let path = project_root.join(GENERATED_DIR).join("main.tf");
    write_atomic(&path, &contents)
}

pub(crate) fn generate_variables_tf(config: &EvmCloudConfig, project_root: &Path) -> Result<()> {
    let is_bare_metal = config.database.provider == "bare_metal";
    let is_postgres = config.database.storage_backend == "postgres";
    let engine = config.compute.engine.as_str();

    let mut var_decls = vec![
        var_decl("project_name", "string"),
        var_decl("infrastructure_provider", "string"),
        var_decl("database_mode", "string"),
        var_decl("compute_engine", "string"),
        var_decl("workload_mode", "string"),
        var_decl("secrets_mode", "string"),
        var_decl("ingress_mode", "string"),
        var_decl("erpc_hostname", "string"),
        var_decl("ingress_tls_email", "string"),
    ];

    if is_bare_metal {
        var_decls.push(var_decl_sensitive("bare_metal_host"));
        var_decls.push(var_decl_sensitive("bare_metal_ssh_private_key_path"));
        var_decls.push(var_decl_default("bare_metal_ssh_user", "string", "ubuntu"));
        var_decls.push(var_decl_default("bare_metal_ssh_port", "number", "22"));
    } else {
        var_decls.push(var_decl("networking_enabled", "bool"));
        var_decls.push(var_decl("aws_region", "string"));
        var_decls.push(var_decl_sensitive("ssh_public_key"));
        match engine {
            "ec2" => {
                var_decls.push(var_decl("ec2_instance_type", "string"));
                var_decls.push(var_decl_sensitive("ec2_ssh_private_key_path"));
            }
            "k3s" => {
                var_decls.push(var_decl("k3s_instance_type", "string"));
                var_decls.push(var_decl_sensitive("k3s_ssh_private_key_path"));
                var_decls.push(var_decl_list("k3s_api_allowed_cidrs", "string"));
            }
            _ => {}
        }
    }

    var_decls.push(var_decl("indexer_storage_backend", "string"));
    if is_postgres {
        var_decls.push(var_decl("postgres_enabled", "bool"));
        var_decls.push(var_decl_sensitive("indexer_postgres_url"));
    } else {
        var_decls.push(var_decl_sensitive("indexer_clickhouse_url"));
        var_decls.push(var_decl_default("indexer_clickhouse_user", "string", "default"));
        var_decls.push(var_decl_sensitive("indexer_clickhouse_password"));
        var_decls.push(var_decl_default("indexer_clickhouse_db", "string", "rindexer"));
    }

    var_decls.push(var_decl("rpc_proxy_enabled", "bool"));
    var_decls.push(var_decl("indexer_enabled", "bool"));
    var_decls.push(var_decl("indexer_rpc_url", "string"));
    var_decls.push(var_decl("erpc_config_yaml", "string"));
    var_decls.push(var_decl("rindexer_config_yaml", "string"));
    var_decls.push(var_decl_map("rindexer_abis", "string"));

    let contents = format!("{}\n", var_decls.join("\n\n"));

    let path = project_root.join(GENERATED_DIR).join("variables.tf");
    write_atomic(&path, &contents)
}

fn var_decl(name: &str, ty: &str) -> String {
    format!("variable \"{name}\" {{\n  type = {ty}\n}}")
}

fn var_decl_default(name: &str, ty: &str, default: &str) -> String {
    // For string types, quote the default; for numbers/bools, emit raw.
    let default_val = if ty == "string" {
        format!("\"{default}\"")
    } else {
        default.to_string()
    };
    format!("variable \"{name}\" {{\n  type    = {ty}\n  default = {default_val}\n}}")
}

fn var_decl_map(name: &str, value_ty: &str) -> String {
    format!("variable \"{name}\" {{\n  type    = map({value_ty})\n  default = {{}}\n}}")
}

fn var_decl_list(name: &str, element_ty: &str) -> String {
    format!("variable \"{name}\" {{\n  type    = list({element_ty})\n  default = []\n}}")
}

fn var_decl_sensitive(name: &str) -> String {
    format!("variable \"{name}\" {{\n  type      = string\n  sensitive = true\n}}")
}

pub(crate) fn generate_outputs_tf(project_root: &Path) -> Result<()> {
    let contents = r#"output "workload_handoff" {
  value     = module.evm_cloud.workload_handoff
  sensitive = true
}
"#;

    let path = project_root.join(GENERATED_DIR).join("outputs.tf");
    write_atomic(&path, contents)
}
