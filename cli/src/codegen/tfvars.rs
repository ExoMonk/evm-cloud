use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::codegen::write_atomic;
use crate::config::schema::{
    ComputeEngine, EvmCloudConfig, InfrastructureProvider, IngressMode, WorkloadMode,
};
use crate::error::{CliError, Result};

const GENERATED_DIR: &str = ".evm-cloud";
const GENERATED_TFVARS: &str = "terraform.auto.tfvars.json";

#[derive(Serialize)]
struct TerraformVars {
    project_name: String,
    infrastructure_provider: InfrastructureProvider,
    database_mode: String,
    compute_engine: ComputeEngine,
    workload_mode: WorkloadMode,
    secrets_mode: String,
    ingress_mode: IngressMode,
    erpc_hostname: String,
    ingress_tls_email: String,
    // Provider-specific infra (AWS)
    #[serde(skip_serializing_if = "Option::is_none")]
    networking_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aws_region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_availability_zones: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_enable_nat_gateway: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ec2_instance_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    k3s_instance_type: Option<String>,
    // NOTE: ssh_public_key, ssh_private_key_path, bare_metal_host are intentionally
    // omitted. They are sensitive and must be provided via secrets.auto.tfvars.
    // Database / storage
    indexer_storage_backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    postgres_enabled: Option<bool>,
    // NOTE: indexer_postgres_url, indexer_clickhouse_url, and indexer_clickhouse_password
    // are intentionally omitted. They are sensitive and must be provided via secrets.auto.tfvars.
    // ClickHouse non-sensitive defaults
    #[serde(skip_serializing_if = "Option::is_none")]
    indexer_clickhouse_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    indexer_clickhouse_db: Option<String>,
    // Indexer / RPC
    indexer_enabled: bool,
    rpc_proxy_enabled: bool,
    indexer_rpc_url: String,
    rindexer_config_yaml: String,
    erpc_config_yaml: String,
    // ABI files: map of filename → JSON content
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    rindexer_abis: BTreeMap<String, String>,
    // User-defined non-sensitive env vars for the indexer container
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    indexer_extra_env: BTreeMap<String, String>,
    // Deployment
    deployment_target: String,
    runtime_arch: String,
    streaming_mode: String,
    // Container images
    rpc_proxy_image: String,
    indexer_image: String,
    // Ingress details (conditional on ingress_mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    ingress_cloudflare_ssl_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ingress_caddy_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ingress_caddy_mem_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ingress_nginx_chart_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ingress_cert_manager_chart_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ingress_request_body_max_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ingress_tls_staging: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ingress_hsts_preload: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ingress_class_name: Option<String>,
    // EC2 extras
    #[serde(skip_serializing_if = "Option::is_none")]
    ec2_rpc_proxy_mem_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ec2_indexer_mem_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ec2_secret_recovery_window_in_days: Option<i64>,
    // Networking extras
    #[serde(skip_serializing_if = "Option::is_none")]
    network_environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_vpc_cidr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_enable_vpc_endpoints: Option<bool>,
    // Postgres tuning
    #[serde(skip_serializing_if = "Option::is_none")]
    postgres_instance_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postgres_engine_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postgres_db_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postgres_db_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postgres_backup_retention: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postgres_manage_master_user_password: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postgres_force_ssl: Option<bool>,
    // Bare metal extras
    #[serde(skip_serializing_if = "Option::is_none")]
    bare_metal_ssh_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bare_metal_ssh_port: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bare_metal_rpc_proxy_mem_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bare_metal_indexer_mem_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bare_metal_secrets_encryption: Option<String>,
    // K3s extras
    #[serde(skip_serializing_if = "Option::is_none")]
    k3s_version: Option<String>,
    // Secrets management
    #[serde(skip_serializing_if = "Option::is_none")]
    secrets_manager_kms_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_secret_store_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_secret_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eso_chart_version: Option<String>,
    // Monitoring
    #[serde(skip_serializing_if = "Option::is_none")]
    monitoring_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kube_prometheus_stack_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    grafana_admin_password_secret_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    grafana_ingress_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    grafana_hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alertmanager_slack_webhook_secret_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alertmanager_sns_topic_arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alertmanager_pagerduty_routing_key_secret_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alertmanager_route_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alertmanager_slack_channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    loki_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    loki_chart_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    promtail_chart_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    loki_persistence_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clickhouse_metrics_url: Option<String>,
}

pub(crate) fn generate_tfvars(config: &EvmCloudConfig, project_root: &Path) -> Result<Value> {
    let rindexer_yaml =
        fs::read_to_string(&config.indexer.config_path).map_err(|source| CliError::Io {
            source,
            path: config.indexer.config_path.clone(),
        })?;

    let erpc_yaml = if let Some(path) = &config.indexer.erpc_config_path {
        fs::read_to_string(path).map_err(|source| CliError::Io {
            source,
            path: path.clone(),
        })?
    } else {
        String::new()
    };

    let is_bare_metal = config.database.provider.is_bare_metal();
    let is_postgres = config.database.storage_backend == "postgres";
    let engine = config.compute.engine;
    let workload_mode = config
        .compute
        .workload_mode
        .unwrap_or_else(|| WorkloadMode::default_for_engine(engine));

    let is_k8s = matches!(engine, ComputeEngine::K3s | ComputeEngine::Eks);
    let is_managed_postgres = is_postgres && config.database.mode == "managed";
    let is_monitoring = config.monitoring.as_ref().is_some_and(|m| m.enabled);
    let ingress_mode = config.ingress.mode;
    let secrets_mode = &config.secrets.mode;

    let vars = TerraformVars {
        project_name: config.project.name.clone(),
        infrastructure_provider: config.database.provider,
        database_mode: config.database.mode.clone(),
        compute_engine: engine,
        workload_mode,
        secrets_mode: config.secrets.mode.clone(),
        ingress_mode: config.ingress.mode,
        erpc_hostname: config.ingress.domain.clone().unwrap_or_default(),
        ingress_tls_email: config.ingress.tls_email.clone().unwrap_or_default(),
        // AWS infra
        networking_enabled: if is_bare_metal { None } else { Some(true) },
        aws_region: if is_bare_metal {
            None
        } else {
            Some(
                config
                    .project
                    .region
                    .clone()
                    .unwrap_or_else(|| "us-east-1".to_string()),
            )
        },
        network_availability_zones: if is_bare_metal {
            None
        } else {
            let region = config.project.region.as_deref().unwrap_or("us-east-1");
            Some(vec![format!("{region}a"), format!("{region}b")])
        },
        network_enable_nat_gateway: if is_bare_metal { None } else { Some(true) },
        ec2_instance_type: if !is_bare_metal && engine == ComputeEngine::Ec2 {
            Some(
                config
                    .compute
                    .instance_type
                    .clone()
                    .unwrap_or_else(|| "t3.small".to_string()),
            )
        } else {
            None
        },
        k3s_instance_type: if !is_bare_metal && engine == ComputeEngine::K3s {
            Some(
                config
                    .compute
                    .instance_type
                    .clone()
                    .unwrap_or_else(|| "t3.small".to_string()),
            )
        } else {
            None
        },
        // Database / storage
        indexer_storage_backend: config.database.storage_backend.clone(),
        postgres_enabled: if is_postgres { Some(true) } else { None },
        indexer_clickhouse_user: if !is_postgres {
            Some("default".to_string())
        } else {
            None
        },
        indexer_clickhouse_db: if !is_postgres {
            Some("rindexer".to_string())
        } else {
            None
        },
        // Indexer / RPC
        indexer_enabled: true,
        rpc_proxy_enabled: !erpc_yaml.is_empty(),
        indexer_rpc_url: infer_indexer_rpc_url(config, !erpc_yaml.is_empty())?,
        rindexer_config_yaml: rindexer_yaml,
        erpc_config_yaml: erpc_yaml,
        rindexer_abis: load_abi_files(&config.indexer.config_path)?,
        indexer_extra_env: config.indexer.extra_env.clone(),
        // Deployment
        deployment_target: config
            .project
            .deployment_target
            .clone()
            .unwrap_or_else(|| "managed".to_string()),
        runtime_arch: config
            .project
            .runtime_arch
            .clone()
            .unwrap_or_else(|| "multi".to_string()),
        streaming_mode: config
            .streaming
            .as_ref()
            .map_or_else(|| "disabled".to_string(), |s| s.mode.clone()),
        // Container images
        rpc_proxy_image: config
            .containers
            .as_ref()
            .and_then(|c| c.rpc_proxy_image.clone())
            .unwrap_or_else(|| "ghcr.io/erpc/erpc:latest".to_string()),
        indexer_image: config
            .containers
            .as_ref()
            .and_then(|c| c.indexer_image.clone())
            .unwrap_or_else(|| "ghcr.io/joshstevens19/rindexer:latest".to_string()),
        // Ingress details
        ingress_cloudflare_ssl_mode: if ingress_mode == IngressMode::Cloudflare {
            Some(
                config
                    .ingress
                    .cloudflare_ssl_mode
                    .clone()
                    .unwrap_or_else(|| "full_strict".to_string()),
            )
        } else {
            None
        },
        ingress_caddy_image: if ingress_mode == IngressMode::Caddy {
            Some(
                config
                    .ingress
                    .caddy_image
                    .clone()
                    .unwrap_or_else(|| "caddy:2.9.1-alpine".to_string()),
            )
        } else {
            None
        },
        ingress_caddy_mem_limit: if ingress_mode == IngressMode::Caddy {
            Some(
                config
                    .ingress
                    .caddy_mem_limit
                    .clone()
                    .unwrap_or_else(|| "128m".to_string()),
            )
        } else {
            None
        },
        ingress_nginx_chart_version: if ingress_mode == IngressMode::IngressNginx {
            Some(
                config
                    .ingress
                    .nginx_chart_version
                    .clone()
                    .unwrap_or_else(|| "4.11.3".to_string()),
            )
        } else {
            None
        },
        ingress_cert_manager_chart_version: if ingress_mode == IngressMode::IngressNginx {
            Some(
                config
                    .ingress
                    .cert_manager_chart_version
                    .clone()
                    .unwrap_or_else(|| "1.16.2".to_string()),
            )
        } else {
            None
        },
        ingress_request_body_max_size: if matches!(
            ingress_mode,
            IngressMode::Caddy | IngressMode::IngressNginx
        ) {
            Some(
                config
                    .ingress
                    .request_body_max_size
                    .clone()
                    .unwrap_or_else(|| "1m".to_string()),
            )
        } else {
            None
        },
        ingress_tls_staging: if matches!(
            ingress_mode,
            IngressMode::Caddy | IngressMode::IngressNginx
        ) {
            Some(config.ingress.tls_staging.unwrap_or(false))
        } else {
            None
        },
        ingress_hsts_preload: if matches!(
            ingress_mode,
            IngressMode::Caddy | IngressMode::IngressNginx
        ) {
            Some(config.ingress.hsts_preload.unwrap_or(false))
        } else {
            None
        },
        ingress_class_name: if is_k8s {
            Some(
                config
                    .ingress
                    .class_name
                    .clone()
                    .unwrap_or_else(|| "nginx".to_string()),
            )
        } else {
            None
        },
        // EC2 extras
        ec2_rpc_proxy_mem_limit: if !is_bare_metal && engine == ComputeEngine::Ec2 {
            Some(
                config
                    .compute
                    .ec2
                    .as_ref()
                    .and_then(|e| e.rpc_proxy_mem_limit.clone())
                    .unwrap_or_else(|| "1g".to_string()),
            )
        } else {
            None
        },
        ec2_indexer_mem_limit: if !is_bare_metal && engine == ComputeEngine::Ec2 {
            Some(
                config
                    .compute
                    .ec2
                    .as_ref()
                    .and_then(|e| e.indexer_mem_limit.clone())
                    .unwrap_or_else(|| "2g".to_string()),
            )
        } else {
            None
        },
        ec2_secret_recovery_window_in_days: if !is_bare_metal && engine == ComputeEngine::Ec2 {
            Some(
                config
                    .compute
                    .ec2
                    .as_ref()
                    .and_then(|e| e.secret_recovery_window_in_days)
                    .unwrap_or(7),
            )
        } else {
            None
        },
        // Networking extras
        network_environment: if !is_bare_metal {
            Some(
                config
                    .networking
                    .as_ref()
                    .and_then(|n| n.environment.clone())
                    .unwrap_or_else(|| "dev".to_string()),
            )
        } else {
            None
        },
        network_vpc_cidr: if !is_bare_metal {
            Some(
                config
                    .networking
                    .as_ref()
                    .and_then(|n| n.vpc_cidr.clone())
                    .unwrap_or_else(|| "10.42.0.0/16".to_string()),
            )
        } else {
            None
        },
        network_enable_vpc_endpoints: if !is_bare_metal {
            Some(
                config
                    .networking
                    .as_ref()
                    .and_then(|n| n.enable_vpc_endpoints)
                    .unwrap_or(false),
            )
        } else {
            None
        },
        // Postgres tuning
        postgres_instance_class: if is_managed_postgres {
            Some(
                config
                    .postgres
                    .as_ref()
                    .and_then(|p| p.instance_class.clone())
                    .unwrap_or_else(|| "db.t4g.micro".to_string()),
            )
        } else {
            None
        },
        postgres_engine_version: if is_managed_postgres {
            Some(
                config
                    .postgres
                    .as_ref()
                    .and_then(|p| p.engine_version.clone())
                    .unwrap_or_else(|| "16.4".to_string()),
            )
        } else {
            None
        },
        postgres_db_name: if is_managed_postgres {
            Some(
                config
                    .postgres
                    .as_ref()
                    .and_then(|p| p.db_name.clone())
                    .unwrap_or_else(|| "rindexer".to_string()),
            )
        } else {
            None
        },
        postgres_db_username: if is_managed_postgres {
            Some(
                config
                    .postgres
                    .as_ref()
                    .and_then(|p| p.db_username.clone())
                    .unwrap_or_else(|| "rindexer".to_string()),
            )
        } else {
            None
        },
        postgres_backup_retention: if is_managed_postgres {
            Some(
                config
                    .postgres
                    .as_ref()
                    .and_then(|p| p.backup_retention)
                    .unwrap_or(7),
            )
        } else {
            None
        },
        postgres_manage_master_user_password: if is_managed_postgres {
            Some(
                config
                    .postgres
                    .as_ref()
                    .and_then(|p| p.manage_master_user_password)
                    .unwrap_or(true),
            )
        } else {
            None
        },
        postgres_force_ssl: if is_managed_postgres {
            Some(
                config
                    .postgres
                    .as_ref()
                    .and_then(|p| p.force_ssl)
                    .unwrap_or(false),
            )
        } else {
            None
        },
        // Bare metal extras
        bare_metal_ssh_user: if is_bare_metal {
            Some("ubuntu".to_string())
        } else {
            None
        },
        bare_metal_ssh_port: if is_bare_metal { Some(22) } else { None },
        bare_metal_rpc_proxy_mem_limit: if is_bare_metal {
            Some(
                config
                    .bare_metal
                    .as_ref()
                    .and_then(|b| b.rpc_proxy_mem_limit.clone())
                    .unwrap_or_else(|| "1g".to_string()),
            )
        } else {
            None
        },
        bare_metal_indexer_mem_limit: if is_bare_metal {
            Some(
                config
                    .bare_metal
                    .as_ref()
                    .and_then(|b| b.indexer_mem_limit.clone())
                    .unwrap_or_else(|| "2g".to_string()),
            )
        } else {
            None
        },
        bare_metal_secrets_encryption: if is_bare_metal {
            Some(
                config
                    .bare_metal
                    .as_ref()
                    .and_then(|b| b.secrets_encryption.clone())
                    .unwrap_or_else(|| "none".to_string()),
            )
        } else {
            None
        },
        // K3s extras
        k3s_version: if !is_bare_metal && engine == ComputeEngine::K3s {
            Some(
                config
                    .compute
                    .k3s
                    .as_ref()
                    .and_then(|k| k.version.clone())
                    .unwrap_or_else(|| "v1.30.4+k3s1".to_string()),
            )
        } else {
            None
        },
        // Secrets management
        secrets_manager_kms_key_id: if secrets_mode == "provider" {
            Some(config.secrets.kms_key_id.clone().unwrap_or_default())
        } else {
            None
        },
        external_secret_store_name: if secrets_mode == "external" {
            Some(
                config
                    .secrets
                    .external_store_name
                    .clone()
                    .unwrap_or_default(),
            )
        } else {
            None
        },
        external_secret_key: if secrets_mode == "external" {
            Some(
                config
                    .secrets
                    .external_secret_key
                    .clone()
                    .unwrap_or_default(),
            )
        } else {
            None
        },
        eso_chart_version: if matches!(secrets_mode.as_str(), "provider" | "external") {
            Some(
                config
                    .secrets
                    .eso_chart_version
                    .clone()
                    .unwrap_or_else(|| "0.9.13".to_string()),
            )
        } else {
            None
        },
        // Monitoring
        monitoring_enabled: if is_k8s { Some(is_monitoring) } else { None },
        kube_prometheus_stack_version: if is_monitoring {
            Some(
                config
                    .monitoring
                    .as_ref()
                    .and_then(|m| m.kube_prometheus_stack_version.clone())
                    .unwrap_or_else(|| "72.6.2".to_string()),
            )
        } else {
            None
        },
        grafana_admin_password_secret_name: if is_monitoring {
            Some(String::new())
        } else {
            None
        },
        grafana_ingress_enabled: if is_monitoring {
            Some(
                config
                    .monitoring
                    .as_ref()
                    .and_then(|m| m.grafana_ingress_enabled)
                    .unwrap_or(true),
            )
        } else {
            None
        },
        grafana_hostname: if is_monitoring {
            Some(
                config
                    .monitoring
                    .as_ref()
                    .and_then(|m| m.grafana_hostname.clone())
                    .unwrap_or_default(),
            )
        } else {
            None
        },
        alertmanager_slack_webhook_secret_name: if is_monitoring {
            Some(String::new())
        } else {
            None
        },
        alertmanager_sns_topic_arn: if is_monitoring {
            Some(String::new())
        } else {
            None
        },
        alertmanager_pagerduty_routing_key_secret_name: if is_monitoring {
            Some(String::new())
        } else {
            None
        },
        alertmanager_route_target: if is_monitoring {
            Some(
                config
                    .monitoring
                    .as_ref()
                    .and_then(|m| m.alertmanager_route_target.clone())
                    .unwrap_or_else(|| "slack".to_string()),
            )
        } else {
            None
        },
        alertmanager_slack_channel: if is_monitoring {
            Some(
                config
                    .monitoring
                    .as_ref()
                    .and_then(|m| m.alertmanager_slack_channel.clone())
                    .unwrap_or_else(|| "#alerts".to_string()),
            )
        } else {
            None
        },
        loki_enabled: if is_monitoring {
            Some(
                config
                    .monitoring
                    .as_ref()
                    .and_then(|m| m.loki_enabled)
                    .unwrap_or(false),
            )
        } else {
            None
        },
        loki_chart_version: if is_monitoring {
            Some(
                config
                    .monitoring
                    .as_ref()
                    .and_then(|m| m.loki_chart_version.clone())
                    .unwrap_or_else(|| "6.24.0".to_string()),
            )
        } else {
            None
        },
        promtail_chart_version: if is_monitoring {
            Some(
                config
                    .monitoring
                    .as_ref()
                    .and_then(|m| m.promtail_chart_version.clone())
                    .unwrap_or_else(|| "6.16.6".to_string()),
            )
        } else {
            None
        },
        loki_persistence_enabled: if is_monitoring {
            Some(
                config
                    .monitoring
                    .as_ref()
                    .and_then(|m| m.loki_persistence_enabled)
                    .unwrap_or(false),
            )
        } else {
            None
        },
        clickhouse_metrics_url: if is_monitoring {
            Some(
                config
                    .monitoring
                    .as_ref()
                    .and_then(|m| m.clickhouse_metrics_url.clone())
                    .unwrap_or_default(),
            )
        } else {
            None
        },
    };

    let json_value = serde_json::to_value(&vars).map_err(CliError::OutputParseError)?;
    let rendered = serde_json::to_string_pretty(&vars).map_err(CliError::OutputParseError)?;

    let generated_path = project_root.join(GENERATED_DIR).join(GENERATED_TFVARS);
    write_atomic(&generated_path, &format!("{rendered}\n"))?;

    ensure_gitignore_entry(project_root, &format!("{GENERATED_DIR}/{GENERATED_TFVARS}"))?;
    Ok(json_value)
}

/// Load ABI JSON files from the `abis/` directory sibling to the rindexer config.
fn load_abi_files(rindexer_config_path: &Path) -> Result<BTreeMap<String, String>> {
    let abis_dir = rindexer_config_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("abis");

    let mut abis = BTreeMap::new();
    if !abis_dir.is_dir() {
        return Ok(abis);
    }

    let entries = fs::read_dir(&abis_dir).map_err(|source| CliError::Io {
        source,
        path: abis_dir.clone(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| CliError::Io {
            source,
            path: abis_dir.clone(),
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let content = fs::read_to_string(&path).map_err(|source| CliError::Io {
                source,
                path: path.clone(),
            })?;
            abis.insert(name, content);
        }
    }

    Ok(abis)
}

pub(crate) fn ensure_gitignore_entry(project_root: &Path, entry: &str) -> Result<()> {
    let gitignore_path = project_root.join(".gitignore");

    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).map_err(|source| CliError::Io {
            source,
            path: gitignore_path.clone(),
        })?
    } else {
        String::new()
    };

    if existing.lines().any(|line| line.trim() == entry) {
        return Ok(());
    }

    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(entry);
    next.push('\n');

    write_atomic(&gitignore_path, &next)
}

fn infer_indexer_rpc_url(config: &EvmCloudConfig, rpc_proxy_enabled: bool) -> Result<String> {
    if rpc_proxy_enabled {
        return Ok("http://erpc:4000".to_string());
    }

    let endpoint =
        config
            .rpc
            .endpoints
            .values()
            .next()
            .ok_or_else(|| CliError::ConfigValidation {
                field: "rpc.endpoints".to_string(),
                message: "at least one RPC endpoint is required when eRPC config is not provided"
                    .to_string(),
            })?;

    Ok(endpoint.clone())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::config::loader;
    use crate::config::schema::{ComputeEngine, InfrastructureProvider, IngressMode, WorkloadMode};

    use super::{ensure_gitignore_entry, generate_tfvars};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "evm-cloud-cli-tests-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    fn write(path: &Path, content: &str) {
        fs::write(path, content).expect("write file")
    }

    fn sample_toml() -> &'static str {
        r#"
schema_version = 1

[project]
name = "demo"
region = "us-east-1"

[compute]
engine = "ec2"
instance_type = "t3.small"

[database]
mode = "managed"
provider = "aws"

[indexer]
config_path = "rindexer.yaml"
chains = ["polygon"]

[rpc]
endpoints = { polygon = "https://rpc.example" }

[ingress]
mode = "none"

[secrets]
mode = "provider"
"#
    }

    #[test]
    fn writes_deterministic_tfvars_and_expected_keys() {
        let dir = temp_dir("tfvars");
        write(&dir.join("rindexer.yaml"), "networks: []");
        write(&dir.join("evm-cloud.toml"), sample_toml());

        let config = loader::load(&dir.join("evm-cloud.toml")).expect("load config");
        let json = generate_tfvars(&config, &dir).expect("generate tfvars");

        let object = json.as_object().expect("json object");
        assert!(object.contains_key("project_name"));
        assert!(object.contains_key("aws_region"));
        assert!(object.contains_key("network_availability_zones"));
        assert!(object.contains_key("network_enable_nat_gateway"));
        // Verify AZs are derived from region
        let azs = object["network_availability_zones"]
            .as_array()
            .expect("azs is array");
        assert_eq!(azs.len(), 2);
        assert_eq!(azs[0].as_str().unwrap(), "us-east-1a");
        assert_eq!(azs[1].as_str().unwrap(), "us-east-1b");
        // Verify NAT gateway defaults to true
        assert_eq!(
            object["network_enable_nat_gateway"].as_bool().unwrap(),
            true
        );
        assert!(object.contains_key("compute_engine"));
        assert!(object.contains_key("database_mode"));
        assert!(object.contains_key("infrastructure_provider"));
        assert!(object.contains_key("rindexer_config_yaml"));

        let first = fs::read_to_string(dir.join(".evm-cloud/terraform.auto.tfvars.json"))
            .expect("read first tfvars");
        let _ = generate_tfvars(&config, &dir).expect("generate tfvars second pass");
        let second = fs::read_to_string(dir.join(".evm-cloud/terraform.auto.tfvars.json"))
            .expect("read second tfvars");

        assert_eq!(first, second);
    }

    #[test]
    fn tfvars_struct_covers_manifest_non_sensitive_vars() {
        use crate::codegen::manifest::{manifest, ResolvedConfig};

        // Build a TerraformVars with dummy values to discover its JSON keys.
        let dummy = super::TerraformVars {
            project_name: String::new(),
            infrastructure_provider: InfrastructureProvider::Aws,
            database_mode: String::new(),
            compute_engine: ComputeEngine::Ec2,
            workload_mode: WorkloadMode::Terraform,
            secrets_mode: String::new(),
            ingress_mode: IngressMode::None,
            erpc_hostname: String::new(),
            ingress_tls_email: String::new(),
            networking_enabled: Some(false),
            aws_region: Some(String::new()),
            network_availability_zones: Some(vec![String::new()]),
            network_enable_nat_gateway: Some(false),
            ec2_instance_type: Some(String::new()),
            k3s_instance_type: Some(String::new()),
            indexer_storage_backend: String::new(),
            postgres_enabled: Some(false),
            indexer_clickhouse_user: Some(String::new()),
            indexer_clickhouse_db: Some(String::new()),
            indexer_enabled: false,
            rpc_proxy_enabled: false,
            indexer_rpc_url: String::new(),
            rindexer_config_yaml: String::new(),
            erpc_config_yaml: String::new(),
            rindexer_abis: std::collections::BTreeMap::from([(
                "dummy".to_string(),
                "{}".to_string(),
            )]),
            indexer_extra_env: std::collections::BTreeMap::from([(
                "DUMMY".to_string(),
                "val".to_string(),
            )]),
            // New fields — all Some to ensure they appear in JSON keys
            deployment_target: String::new(),
            runtime_arch: String::new(),
            streaming_mode: String::new(),
            rpc_proxy_image: String::new(),
            indexer_image: String::new(),
            ingress_cloudflare_ssl_mode: Some(String::new()),
            ingress_caddy_image: Some(String::new()),
            ingress_caddy_mem_limit: Some(String::new()),
            ingress_nginx_chart_version: Some(String::new()),
            ingress_cert_manager_chart_version: Some(String::new()),
            ingress_request_body_max_size: Some(String::new()),
            ingress_tls_staging: Some(false),
            ingress_hsts_preload: Some(false),
            ingress_class_name: Some(String::new()),
            ec2_rpc_proxy_mem_limit: Some(String::new()),
            ec2_indexer_mem_limit: Some(String::new()),
            ec2_secret_recovery_window_in_days: Some(0),
            network_environment: Some(String::new()),
            network_vpc_cidr: Some(String::new()),
            network_enable_vpc_endpoints: Some(false),
            postgres_instance_class: Some(String::new()),
            postgres_engine_version: Some(String::new()),
            postgres_db_name: Some(String::new()),
            postgres_db_username: Some(String::new()),
            postgres_backup_retention: Some(0),
            postgres_manage_master_user_password: Some(false),
            postgres_force_ssl: Some(false),
            bare_metal_ssh_user: Some(String::new()),
            bare_metal_ssh_port: Some(0),
            bare_metal_rpc_proxy_mem_limit: Some(String::new()),
            bare_metal_indexer_mem_limit: Some(String::new()),
            bare_metal_secrets_encryption: Some(String::new()),
            k3s_version: Some(String::new()),
            secrets_manager_kms_key_id: Some(String::new()),
            external_secret_store_name: Some(String::new()),
            external_secret_key: Some(String::new()),
            eso_chart_version: Some(String::new()),
            monitoring_enabled: Some(false),
            kube_prometheus_stack_version: Some(String::new()),
            grafana_admin_password_secret_name: Some(String::new()),
            grafana_ingress_enabled: Some(false),
            grafana_hostname: Some(String::new()),
            alertmanager_slack_webhook_secret_name: Some(String::new()),
            alertmanager_sns_topic_arn: Some(String::new()),
            alertmanager_pagerduty_routing_key_secret_name: Some(String::new()),
            alertmanager_route_target: Some(String::new()),
            alertmanager_slack_channel: Some(String::new()),
            loki_enabled: Some(false),
            loki_chart_version: Some(String::new()),
            promtail_chart_version: Some(String::new()),
            loki_persistence_enabled: Some(false),
            clickhouse_metrics_url: Some(String::new()),
        };
        let json = serde_json::to_value(&dummy).unwrap();
        let tfvars_keys: std::collections::HashSet<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();

        let entries = manifest();

        // Check multiple resolved configs to catch conditional variables.
        let configs = [
            ResolvedConfig {
                is_bare_metal: false,
                is_postgres: false,
                is_managed_postgres: false,
                is_k8s: false,
                is_monitoring: false,
                engine: ComputeEngine::Ec2,
                ingress_mode: IngressMode::None,
                secrets_mode_val: "inline".to_string(),
                user_defaults: std::collections::HashMap::new(),
            },
            ResolvedConfig {
                is_bare_metal: false,
                is_postgres: false,
                is_managed_postgres: false,
                is_k8s: true,
                is_monitoring: false,
                engine: ComputeEngine::K3s,
                ingress_mode: IngressMode::None,
                secrets_mode_val: "inline".to_string(),
                user_defaults: std::collections::HashMap::new(),
            },
            ResolvedConfig {
                is_bare_metal: false,
                is_postgres: true,
                is_managed_postgres: true,
                is_k8s: false,
                is_monitoring: false,
                engine: ComputeEngine::Ec2,
                ingress_mode: IngressMode::None,
                secrets_mode_val: "provider".to_string(),
                user_defaults: std::collections::HashMap::new(),
            },
        ];

        for rc in &configs {
            for entry in &entries {
                if !entry.easy_auto_tfvars {
                    continue; // Only auto-populated vars belong in TerraformVars
                }
                if !rc.matches(&entry.condition) {
                    continue;
                }
                assert!(
                    tfvars_keys.contains(entry.name),
                    "Manifest variable `{}` is missing from TerraformVars struct (config: engine={}, bare_metal={}, postgres={})",
                    entry.name, rc.engine, rc.is_bare_metal, rc.is_postgres
                );
            }
        }
    }

    #[test]
    fn gitignore_updates_are_idempotent() {
        let dir = temp_dir("gitignore");
        ensure_gitignore_entry(&dir, ".evm-cloud/terraform.auto.tfvars.json")
            .expect("first update");
        ensure_gitignore_entry(&dir, ".evm-cloud/terraform.auto.tfvars.json")
            .expect("second update");

        let content = fs::read_to_string(dir.join(".gitignore")).expect("read gitignore");
        let count = content
            .lines()
            .filter(|line| line.trim() == ".evm-cloud/terraform.auto.tfvars.json")
            .count();
        assert_eq!(count, 1);
    }
}
