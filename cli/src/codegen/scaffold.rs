use std::path::Path;

use crate::codegen::write_atomic;
use crate::config::schema::EvmCloudConfig;
use crate::error::Result;

const GENERATED_DIR: &str = ".evm-cloud";

pub(crate) fn generate_main_tf(_config: &EvmCloudConfig, project_root: &Path) -> Result<()> {
  let module_source = "git::https://github.com/ExoMonk/evm-cloud.git?ref=v0.0.1.alpha";

    let contents = format!(
        r#"terraform {{
  required_version = ">= 1.14.6"
}}

module "evm_cloud" {{
  source = "{module_source}"

  project_name            = var.project_name
  infrastructure_provider = var.infrastructure_provider
  database_mode           = var.database_mode
  ingress_mode            = var.ingress_mode
  erpc_hostname           = var.erpc_hostname
  ingress_tls_email       = var.ingress_tls_email
  compute_engine          = var.compute_engine
  ec2_instance_type       = var.ec2_instance_type
  aws_region              = var.aws_region
  secrets_mode            = var.secrets_mode

  rpc_proxy_enabled   = var.rpc_proxy_enabled
  indexer_enabled     = var.indexer_enabled
  indexer_rpc_url     = var.indexer_rpc_url
  erpc_config_yaml    = var.erpc_config_yaml
  rindexer_config_yaml = var.rindexer_config_yaml
}}

variable "project_name" {{
  type = string
}}

variable "infrastructure_provider" {{
  type = string
}}

variable "database_mode" {{
  type = string
}}

variable "ingress_mode" {{
  type = string
}}

variable "erpc_hostname" {{
  type = string
}}

variable "ingress_tls_email" {{
  type = string
}}

variable "compute_engine" {{
  type = string
}}

variable "ec2_instance_type" {{
  type = string
}}

variable "aws_region" {{
  type = string
}}

variable "secrets_mode" {{
  type = string
}}

variable "rpc_proxy_enabled" {{
  type = bool
}}

variable "indexer_enabled" {{
  type = bool
}}

variable "indexer_rpc_url" {{
  type = string
}}

variable "erpc_config_yaml" {{
  type = string
}}

variable "rindexer_config_yaml" {{
  type = string
}}
"#
    );

    let path = project_root.join(GENERATED_DIR).join("main.tf");
    write_atomic(&path, &contents)
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
