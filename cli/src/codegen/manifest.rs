//! Single source of truth for all Terraform variables the CLI generates.
//!
//! Both Easy mode (`codegen/scaffold.rs`) and Power mode (`init_templates.rs`)
//! derive their `main.tf` and `variables.tf` output from the manifest defined
//! here, eliminating duplication and preventing drift between the two paths.

use std::collections::HashMap;

use crate::config::schema::{ComputeEngine, EvmCloudConfig, IngressMode, WorkloadMode};
use crate::init_answers::{DatabaseProfile, InitAnswers};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// When a variable should be included in generated output.
#[derive(Debug, Clone)]
pub(crate) enum Condition {
    Always,
    BareMetal,
    Cloud,
    Engine(&'static [&'static str]),
    CloudEngine(&'static [&'static str]),
    Postgres,
    ByodbPostgres,
    Clickhouse,
    ManagedPostgres,
    K8s,
    IngressMode(&'static [&'static str]),
    SecretsMode(&'static [&'static str]),
    Monitoring,
}

/// HCL type for a variable declaration.
#[derive(Debug, Clone, Copy)]
pub(crate) enum HclType {
    String,
    Bool,
    Number,
    List(&'static str),
    Map(&'static str),
}

/// How a variable's value is referenced in the module call inside `main.tf`.
#[derive(Debug, Clone)]
pub(crate) enum PassthroughMode {
    /// `var.X` in both modes.
    Direct,
    /// Easy: `var.X` (content inlined in tfvars.json).
    /// Power: `file(var.X)` (var holds a file path, read at plan time).
    FileContent,
    /// Easy: `var.X` (map inlined in tfvars.json).
    /// Power: literal HCL expression.
    FilesetMap { power_expr: &'static str },
}

/// Default value for a variable declaration.
#[derive(Debug, Clone)]
pub(crate) enum VarDefault {
    Str(&'static str),
    Bool(bool),
    Number(i64),
    EmptyList,
    EmptyMap,
    Null,
    /// Dynamic default computed at generation time from user config/answers.
    /// The string is rendered as-is in HCL (must include quotes for strings).
    Hcl(String),
}

/// A single Terraform variable entry in the manifest.
#[derive(Debug, Clone)]
pub(crate) struct VarEntry {
    pub name: &'static str,
    pub hcl_type: HclType,
    pub sensitive: bool,
    pub condition: Condition,
    pub default: Option<VarDefault>,
    pub passthrough: PassthroughMode,
    /// Insert a blank line before this variable in generated output.
    pub group_break: bool,
    /// Whether Easy mode auto-populates this variable in `terraform.auto.tfvars.json`.
    /// Variables with `false` are declared in `variables.tf` but must be provided by the
    /// user (e.g. via a `.tfvars` file or `-var` flag).
    /// Only read in tests (tfvars consistency check).
    #[allow(dead_code)]
    pub easy_auto_tfvars: bool,
}

/// Whether we are generating for Easy mode or Power mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenerationMode {
    Easy,
    Power,
}

/// Flattened booleans used to evaluate [`Condition`] predicates,
/// plus user-selected defaults derived from config or init wizard answers.
pub(crate) struct ResolvedConfig {
    pub is_bare_metal: bool,
    pub is_postgres: bool,
    pub is_managed_postgres: bool,
    pub is_k8s: bool,
    pub is_monitoring: bool,
    pub engine: ComputeEngine,
    pub ingress_mode: IngressMode,
    pub secrets_mode_val: String,
    /// Defaults derived from the user's config/init answers.
    /// Applied to manifest entries that have `default: None` at render time.
    pub user_defaults: HashMap<String, VarDefault>,
}

// ---------------------------------------------------------------------------
// ResolvedConfig constructors
// ---------------------------------------------------------------------------

impl ResolvedConfig {
    pub(crate) fn from_evm_config(config: &EvmCloudConfig) -> Self {
        let is_bare_metal = config.database.provider.is_bare_metal();
        let is_postgres = config.database.storage_backend == "postgres";
        let engine = config.compute.engine;
        let is_k8s = matches!(engine, ComputeEngine::K3s | ComputeEngine::Eks);
        let is_managed_postgres = is_postgres && config.database.mode == "managed";
        let ingress_mode = config.ingress.mode;
        let secrets_mode_val = config.secrets.mode.clone();
        let is_monitoring = config.monitoring.as_ref().is_some_and(|m| m.enabled);

        let workload_mode = config
            .compute
            .workload_mode
            .unwrap_or_else(|| WorkloadMode::default_for_engine(engine));
        let region = config.project.region.as_deref().unwrap_or("us-east-1");
        let rpc_proxy_enabled = config.indexer.erpc_config_path.is_some();

        let mut d = HashMap::new();
        // Core
        d.insert(
            "infrastructure_provider".into(),
            hcl_str(config.database.provider.as_str()),
        );
        d.insert("database_mode".into(), hcl_str(&config.database.mode));
        d.insert("compute_engine".into(), hcl_str(engine.as_str()));
        d.insert("workload_mode".into(), hcl_str(workload_mode.as_str()));
        d.insert("secrets_mode".into(), hcl_str(&secrets_mode_val));
        d.insert("ingress_mode".into(), hcl_str(ingress_mode.as_str()));
        d.insert(
            "erpc_hostname".into(),
            hcl_str(config.ingress.domain.as_deref().unwrap_or("")),
        );
        d.insert(
            "ingress_tls_email".into(),
            hcl_str(config.ingress.tls_email.as_deref().unwrap_or("")),
        );
        // Deployment
        if let Some(ref dt) = config.project.deployment_target {
            d.insert("deployment_target".into(), hcl_str(dt));
        }
        if let Some(ref ra) = config.project.runtime_arch {
            d.insert("runtime_arch".into(), hcl_str(ra));
        }
        if let Some(st) = config.streaming.as_ref().map(|s| s.mode.as_str()) {
            d.insert("streaming_mode".into(), hcl_str(st));
        }
        // Ingress details
        if let Some(ref ssl) = config.ingress.cloudflare_ssl_mode {
            d.insert("ingress_cloudflare_ssl_mode".into(), hcl_str(ssl));
        }
        if let Some(ref img) = config.ingress.caddy_image {
            d.insert("ingress_caddy_image".into(), hcl_str(img));
        }
        if let Some(ref mem) = config.ingress.caddy_mem_limit {
            d.insert("ingress_caddy_mem_limit".into(), hcl_str(mem));
        }
        if let Some(ref v) = config.ingress.nginx_chart_version {
            d.insert("ingress_nginx_chart_version".into(), hcl_str(v));
        }
        if let Some(ref v) = config.ingress.cert_manager_chart_version {
            d.insert("ingress_cert_manager_chart_version".into(), hcl_str(v));
        }
        if let Some(ref v) = config.ingress.request_body_max_size {
            d.insert("ingress_request_body_max_size".into(), hcl_str(v));
        }
        if let Some(v) = config.ingress.tls_staging {
            d.insert("ingress_tls_staging".into(), VarDefault::Bool(v));
        }
        if let Some(v) = config.ingress.hsts_preload {
            d.insert("ingress_hsts_preload".into(), VarDefault::Bool(v));
        }
        if let Some(ref v) = config.ingress.class_name {
            d.insert("ingress_class_name".into(), hcl_str(v));
        }
        // Cloud-specific
        if !is_bare_metal {
            d.insert("networking_enabled".into(), VarDefault::Bool(true));
            d.insert("aws_region".into(), hcl_str(region));
            d.insert(
                "network_availability_zones".into(),
                VarDefault::Hcl(format!("[\"{region}a\", \"{region}b\"]")),
            );
            d.insert("network_enable_nat_gateway".into(), VarDefault::Bool(true));
            if let Some(ref net) = config.networking {
                if let Some(ref cidr) = net.vpc_cidr {
                    d.insert("network_vpc_cidr".into(), hcl_str(cidr));
                }
                if let Some(v) = net.enable_vpc_endpoints {
                    d.insert("network_enable_vpc_endpoints".into(), VarDefault::Bool(v));
                }
                if let Some(ref env) = net.environment {
                    d.insert("network_environment".into(), hcl_str(env));
                }
            }
            if engine == ComputeEngine::Ec2 {
                d.insert(
                    "ec2_instance_type".into(),
                    hcl_str(
                        config
                            .compute
                            .instance_type
                            .as_deref()
                            .unwrap_or("t3.small"),
                    ),
                );
                if let Some(ref ec2) = config.compute.ec2 {
                    if let Some(ref v) = ec2.rpc_proxy_mem_limit {
                        d.insert("ec2_rpc_proxy_mem_limit".into(), hcl_str(v));
                    }
                    if let Some(ref v) = ec2.indexer_mem_limit {
                        d.insert("ec2_indexer_mem_limit".into(), hcl_str(v));
                    }
                    if let Some(v) = ec2.secret_recovery_window_in_days {
                        d.insert(
                            "ec2_secret_recovery_window_in_days".into(),
                            VarDefault::Number(v),
                        );
                    }
                }
            }
            if engine == ComputeEngine::K3s {
                d.insert(
                    "k3s_instance_type".into(),
                    hcl_str(
                        config
                            .compute
                            .instance_type
                            .as_deref()
                            .unwrap_or("t3.small"),
                    ),
                );
                if let Some(ref k) = config.compute.k3s {
                    if let Some(ref v) = k.version {
                        d.insert("k3s_version".into(), hcl_str(v));
                    }
                }
            }
        } else {
            // Bare metal extras
            if let Some(ref bm) = config.bare_metal {
                if let Some(ref v) = bm.rpc_proxy_mem_limit {
                    d.insert("bare_metal_rpc_proxy_mem_limit".into(), hcl_str(v));
                }
                if let Some(ref v) = bm.indexer_mem_limit {
                    d.insert("bare_metal_indexer_mem_limit".into(), hcl_str(v));
                }
                if let Some(ref v) = bm.secrets_encryption {
                    d.insert("bare_metal_secrets_encryption".into(), hcl_str(v));
                }
            }
        }
        // Database
        d.insert(
            "indexer_storage_backend".into(),
            hcl_str(&config.database.storage_backend),
        );
        if is_postgres {
            d.insert("postgres_enabled".into(), VarDefault::Bool(true));
        }
        if is_managed_postgres {
            if let Some(ref pg) = config.postgres {
                if let Some(ref v) = pg.instance_class {
                    d.insert("postgres_instance_class".into(), hcl_str(v));
                }
                if let Some(ref v) = pg.engine_version {
                    d.insert("postgres_engine_version".into(), hcl_str(v));
                }
                if let Some(ref v) = pg.db_name {
                    d.insert("postgres_db_name".into(), hcl_str(v));
                }
                if let Some(ref v) = pg.db_username {
                    d.insert("postgres_db_username".into(), hcl_str(v));
                }
                if let Some(v) = pg.backup_retention {
                    d.insert("postgres_backup_retention".into(), VarDefault::Number(v));
                }
                if let Some(v) = pg.manage_master_user_password {
                    d.insert(
                        "postgres_manage_master_user_password".into(),
                        VarDefault::Bool(v),
                    );
                }
                if let Some(v) = pg.force_ssl {
                    d.insert("postgres_force_ssl".into(), VarDefault::Bool(v));
                }
            }
        }
        // Container images
        if let Some(ref c) = config.containers {
            if let Some(ref v) = c.rpc_proxy_image {
                d.insert("rpc_proxy_image".into(), hcl_str(v));
            }
            if let Some(ref v) = c.indexer_image {
                d.insert("indexer_image".into(), hcl_str(v));
            }
        }
        // Secrets details
        if let Some(ref v) = config.secrets.kms_key_id {
            d.insert("secrets_manager_kms_key_id".into(), hcl_str(v));
        }
        if let Some(ref v) = config.secrets.external_store_name {
            d.insert("external_secret_store_name".into(), hcl_str(v));
        }
        if let Some(ref v) = config.secrets.external_secret_key {
            d.insert("external_secret_key".into(), hcl_str(v));
        }
        if let Some(ref v) = config.secrets.eso_chart_version {
            d.insert("eso_chart_version".into(), hcl_str(v));
        }
        // Monitoring
        if is_monitoring {
            d.insert("monitoring_enabled".into(), VarDefault::Bool(true));
            if let Some(ref mon) = config.monitoring {
                if let Some(ref v) = mon.kube_prometheus_stack_version {
                    d.insert("kube_prometheus_stack_version".into(), hcl_str(v));
                }
                if let Some(v) = mon.grafana_ingress_enabled {
                    d.insert("grafana_ingress_enabled".into(), VarDefault::Bool(v));
                }
                if let Some(ref v) = mon.grafana_hostname {
                    d.insert("grafana_hostname".into(), hcl_str(v));
                }
                if let Some(ref v) = mon.alertmanager_route_target {
                    d.insert("alertmanager_route_target".into(), hcl_str(v));
                }
                if let Some(ref v) = mon.alertmanager_slack_channel {
                    d.insert("alertmanager_slack_channel".into(), hcl_str(v));
                }
                if let Some(v) = mon.loki_enabled {
                    d.insert("loki_enabled".into(), VarDefault::Bool(v));
                }
                if let Some(ref v) = mon.loki_chart_version {
                    d.insert("loki_chart_version".into(), hcl_str(v));
                }
                if let Some(ref v) = mon.promtail_chart_version {
                    d.insert("promtail_chart_version".into(), hcl_str(v));
                }
                if let Some(v) = mon.loki_persistence_enabled {
                    d.insert("loki_persistence_enabled".into(), VarDefault::Bool(v));
                }
                if let Some(ref v) = mon.clickhouse_metrics_url {
                    d.insert("clickhouse_metrics_url".into(), hcl_str(v));
                }
            }
        }
        // Indexer / RPC
        d.insert(
            "rpc_proxy_enabled".into(),
            VarDefault::Bool(rpc_proxy_enabled),
        );
        d.insert("indexer_enabled".into(), VarDefault::Bool(true));
        let indexer_rpc_url = if rpc_proxy_enabled {
            "http://erpc:4000".to_string()
        } else {
            config
                .rpc
                .endpoints
                .values()
                .next()
                .cloned()
                .unwrap_or_default()
        };
        d.insert("indexer_rpc_url".into(), hcl_str(&indexer_rpc_url));
        // Config content: Easy mode inlines via tfvars.json; empty default prevents TF prompt
        d.insert("erpc_config_yaml".into(), hcl_str(""));
        d.insert("rindexer_config_yaml".into(), hcl_str(""));

        Self {
            is_bare_metal,
            is_postgres,
            is_managed_postgres,
            is_k8s,
            is_monitoring,
            engine,
            ingress_mode,
            secrets_mode_val,
            user_defaults: d,
        }
    }

    pub(crate) fn from_init_answers(answers: &InitAnswers) -> Self {
        let is_bare_metal = answers.infrastructure_provider.is_bare_metal();
        let is_postgres = matches!(
            answers.database_profile,
            DatabaseProfile::ByodbPostgres | DatabaseProfile::ManagedRds
        );
        let engine = answers.compute_engine;
        let is_k8s = matches!(engine, ComputeEngine::K3s | ComputeEngine::Eks);
        let is_managed_postgres = matches!(answers.database_profile, DatabaseProfile::ManagedRds);

        let workload_mode = answers
            .workload_mode
            .unwrap_or_else(|| WorkloadMode::default_for_engine(engine));
        let secrets_mode = if is_bare_metal || engine == ComputeEngine::K3s {
            "inline"
        } else {
            "provider"
        };
        let storage_backend = if is_postgres {
            "postgres"
        } else {
            "clickhouse"
        };
        let database_mode = match answers.database_profile {
            DatabaseProfile::ManagedRds | DatabaseProfile::ManagedClickhouse => "managed",
            _ => "self_hosted",
        };
        let ingress_mode = answers.ingress_mode;

        let mut d = HashMap::new();
        // Core
        d.insert("project_name".into(), hcl_str(&answers.project_name));
        d.insert(
            "infrastructure_provider".into(),
            hcl_str(answers.infrastructure_provider.as_str()),
        );
        d.insert("database_mode".into(), hcl_str(database_mode));
        d.insert("compute_engine".into(), hcl_str(engine.as_str()));
        d.insert("workload_mode".into(), hcl_str(workload_mode.as_str()));
        d.insert("secrets_mode".into(), hcl_str(secrets_mode));
        d.insert("ingress_mode".into(), hcl_str(ingress_mode.as_str()));
        d.insert(
            "erpc_hostname".into(),
            hcl_str(answers.erpc_hostname.as_deref().unwrap_or("")),
        );
        d.insert(
            "ingress_tls_email".into(),
            hcl_str(answers.ingress_tls_email.as_deref().unwrap_or("")),
        );
        // Cloud-specific
        if !is_bare_metal {
            let region = answers.region.as_deref().unwrap_or("us-east-1");
            d.insert("networking_enabled".into(), VarDefault::Bool(true));
            d.insert("aws_region".into(), hcl_str(region));
            d.insert(
                "network_availability_zones".into(),
                VarDefault::Hcl(format!("[\"{region}a\", \"{region}b\"]")),
            );
            d.insert("network_enable_nat_gateway".into(), VarDefault::Bool(true));
            if engine == ComputeEngine::Ec2 {
                d.insert(
                    "ec2_instance_type".into(),
                    hcl_str(answers.instance_type.as_deref().unwrap_or("t3.small")),
                );
            }
            if engine == ComputeEngine::K3s {
                d.insert(
                    "k3s_instance_type".into(),
                    hcl_str(answers.instance_type.as_deref().unwrap_or("t3.small")),
                );
            }
        }
        // Database
        d.insert("indexer_storage_backend".into(), hcl_str(storage_backend));
        if is_postgres {
            d.insert("postgres_enabled".into(), VarDefault::Bool(true));
        }
        // Indexer / RPC
        d.insert(
            "rpc_proxy_enabled".into(),
            VarDefault::Bool(answers.generate_erpc_config),
        );
        d.insert("indexer_enabled".into(), VarDefault::Bool(true));
        let indexer_rpc_url = if answers.generate_erpc_config {
            "http://erpc:4000"
        } else {
            answers
                .rpc_endpoints
                .values()
                .next()
                .map(|s| s.as_str())
                .unwrap_or("")
        };
        d.insert("indexer_rpc_url".into(), hcl_str(indexer_rpc_url));
        // Config paths: Power mode uses file() wrapper, default to standard paths
        d.insert("erpc_config_yaml".into(), hcl_str("config/erpc.yaml"));
        d.insert(
            "rindexer_config_yaml".into(),
            hcl_str("config/rindexer.yaml"),
        );

        Self {
            is_bare_metal,
            is_postgres,
            is_managed_postgres,
            is_k8s,
            is_monitoring: false, // init wizard doesn't configure monitoring
            engine,
            ingress_mode,
            secrets_mode_val: secrets_mode.to_string(),
            user_defaults: d,
        }
    }

    pub(crate) fn matches(&self, condition: &Condition) -> bool {
        match condition {
            Condition::Always => true,
            Condition::BareMetal => self.is_bare_metal,
            Condition::Cloud => !self.is_bare_metal,
            Condition::Engine(engines) => engines.contains(&self.engine.as_str()),
            Condition::CloudEngine(engines) => {
                !self.is_bare_metal && engines.contains(&self.engine.as_str())
            }
            Condition::Postgres => self.is_postgres,
            Condition::ByodbPostgres => self.is_postgres && !self.is_managed_postgres,
            Condition::Clickhouse => !self.is_postgres,
            Condition::ManagedPostgres => self.is_managed_postgres,
            Condition::K8s => self.is_k8s,
            Condition::IngressMode(modes) => modes.contains(&self.ingress_mode.as_str()),
            Condition::SecretsMode(modes) => modes.contains(&self.secrets_mode_val.as_str()),
            Condition::Monitoring => self.is_monitoring,
        }
    }
}

// ---------------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------------

/// Returns all Terraform variables the CLI manages.
pub(crate) fn manifest() -> Vec<VarEntry> {
    vec![
        // ── Core ──
        VarEntry { name: "project_name",            hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "infrastructure_provider",  hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "database_mode",            hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "compute_engine",           hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "workload_mode",            hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "secrets_mode",             hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "ingress_mode",             hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "erpc_hostname",            hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "ingress_tls_email",        hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── SSH ──
        VarEntry { name: "ssh_private_key_path",            hcl_type: HclType::String, sensitive: true,  condition: Condition::Always,    default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: false },
        // ── Bare metal ──
        VarEntry { name: "bare_metal_host",                 hcl_type: HclType::String, sensitive: true,  condition: Condition::BareMetal, default: None,                             passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: false },
        VarEntry { name: "bare_metal_ssh_user",             hcl_type: HclType::String, sensitive: false, condition: Condition::BareMetal, default: Some(VarDefault::Str("ubuntu")),  passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "bare_metal_ssh_port",             hcl_type: HclType::Number, sensitive: false, condition: Condition::BareMetal, default: Some(VarDefault::Number(22)),     passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Cloud (non-bare-metal) ──
        VarEntry { name: "networking_enabled",              hcl_type: HclType::Bool,   sensitive: false, condition: Condition::Cloud,            default: None,                             passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "aws_region",                      hcl_type: HclType::String, sensitive: false, condition: Condition::Cloud,            default: None,                             passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "ssh_public_key",                  hcl_type: HclType::String, sensitive: true,  condition: Condition::Cloud,            default: None,                             passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: false },
        VarEntry { name: "network_availability_zones",      hcl_type: HclType::List("string"), sensitive: false, condition: Condition::Cloud, default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "network_enable_nat_gateway",      hcl_type: HclType::Bool,   sensitive: false, condition: Condition::Cloud,            default: None,                             passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── EC2-specific ──
        VarEntry { name: "ec2_instance_type",               hcl_type: HclType::String, sensitive: false, condition: Condition::Engine(&["ec2"]), default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── K3s-specific ──
        VarEntry { name: "k3s_instance_type",               hcl_type: HclType::String, sensitive: false, condition: Condition::CloudEngine(&["k3s"]), default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "k3s_api_allowed_cidrs",           hcl_type: HclType::List("string"), sensitive: false, condition: Condition::CloudEngine(&["k3s"]), default: Some(VarDefault::EmptyList), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: false },
        // ── Database ──
        VarEntry { name: "indexer_storage_backend",         hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None,                                passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "postgres_enabled",                hcl_type: HclType::Bool,   sensitive: false, condition: Condition::Postgres,   default: None,                                passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "indexer_postgres_url",            hcl_type: HclType::String, sensitive: true,  condition: Condition::ByodbPostgres, default: None,                             passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: false },
        VarEntry { name: "indexer_clickhouse_url",          hcl_type: HclType::String, sensitive: true,  condition: Condition::Clickhouse, default: None,                                passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: false },
        VarEntry { name: "indexer_clickhouse_user",         hcl_type: HclType::String, sensitive: false, condition: Condition::Clickhouse, default: Some(VarDefault::Str("default")),    passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "indexer_clickhouse_password",     hcl_type: HclType::String, sensitive: true,  condition: Condition::Clickhouse, default: None,                                passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: false },
        // NOTE: TF default is "default"; we override to "rindexer" as a better default for rindexer users.
        VarEntry { name: "indexer_clickhouse_db",           hcl_type: HclType::String, sensitive: false, condition: Condition::Clickhouse, default: Some(VarDefault::Str("rindexer")),   passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Indexer / RPC ──
        VarEntry { name: "rpc_proxy_enabled",               hcl_type: HclType::Bool,   sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "indexer_enabled",                 hcl_type: HclType::Bool,   sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "indexer_rpc_url",                 hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "erpc_config_yaml",                hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::FileContent, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "rindexer_config_yaml",            hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: None, passthrough: PassthroughMode::FileContent, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "rindexer_abis",                   hcl_type: HclType::Map("string"), sensitive: false, condition: Condition::Always, default: Some(VarDefault::EmptyMap), passthrough: PassthroughMode::FilesetMap { power_expr: "{ for f in fileset(\"${path.module}/config/abis\", \"*.json\") : f => file(\"${path.module}/config/abis/${f}\") }" }, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "indexer_extra_env",                hcl_type: HclType::Map("string"), sensitive: false, condition: Condition::Always, default: Some(VarDefault::EmptyMap), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "indexer_extra_secret_env",         hcl_type: HclType::Map("string"), sensitive: true,  condition: Condition::Always, default: Some(VarDefault::EmptyMap), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: false },
        // ── Deployment ──
        VarEntry { name: "deployment_target",               hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: Some(VarDefault::Str("managed")),  passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "runtime_arch",                    hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: Some(VarDefault::Str("multi")),    passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "streaming_mode",                  hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: Some(VarDefault::Str("disabled")), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Container images ──
        VarEntry { name: "rpc_proxy_image",                 hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: Some(VarDefault::Str("ghcr.io/erpc/erpc:latest")),                  passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "indexer_image",                   hcl_type: HclType::String, sensitive: false, condition: Condition::Always,     default: Some(VarDefault::Str("ghcr.io/joshstevens19/rindexer:latest")),      passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Ingress: Cloudflare ──
        VarEntry { name: "ingress_cloudflare_origin_cert",  hcl_type: HclType::String, sensitive: true,  condition: Condition::IngressMode(&["cloudflare"]), default: None,                                     passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: false },
        VarEntry { name: "ingress_cloudflare_origin_key",   hcl_type: HclType::String, sensitive: true,  condition: Condition::IngressMode(&["cloudflare"]), default: None,                                     passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: false },
        VarEntry { name: "ingress_cloudflare_ssl_mode",     hcl_type: HclType::String, sensitive: false, condition: Condition::IngressMode(&["cloudflare"]), default: Some(VarDefault::Str("full_strict")),      passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Ingress: Caddy ──
        VarEntry { name: "ingress_caddy_image",             hcl_type: HclType::String, sensitive: false, condition: Condition::IngressMode(&["caddy"]),      default: Some(VarDefault::Str("caddy:2.9.1-alpine")), passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "ingress_caddy_mem_limit",         hcl_type: HclType::String, sensitive: false, condition: Condition::IngressMode(&["caddy"]),      default: Some(VarDefault::Str("128m")),                passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Ingress: nginx/cert-manager ──
        VarEntry { name: "ingress_nginx_chart_version",         hcl_type: HclType::String, sensitive: false, condition: Condition::IngressMode(&["ingress_nginx"]), default: Some(VarDefault::Str("4.11.3")), passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "ingress_cert_manager_chart_version",  hcl_type: HclType::String, sensitive: false, condition: Condition::IngressMode(&["ingress_nginx"]), default: Some(VarDefault::Str("1.16.2")), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Ingress: shared TLS ──
        VarEntry { name: "ingress_request_body_max_size",   hcl_type: HclType::String, sensitive: false, condition: Condition::IngressMode(&["caddy", "ingress_nginx"]), default: Some(VarDefault::Str("1m")),   passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "ingress_tls_staging",             hcl_type: HclType::Bool,   sensitive: false, condition: Condition::IngressMode(&["caddy", "ingress_nginx"]), default: Some(VarDefault::Bool(false)), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "ingress_hsts_preload",            hcl_type: HclType::Bool,   sensitive: false, condition: Condition::IngressMode(&["caddy", "ingress_nginx"]), default: Some(VarDefault::Bool(false)), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "ingress_class_name",              hcl_type: HclType::String, sensitive: false, condition: Condition::K8s,        default: Some(VarDefault::Str("nginx")),    passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── EC2 extras ──
        VarEntry { name: "ec2_rpc_proxy_mem_limit",             hcl_type: HclType::String, sensitive: false, condition: Condition::Engine(&["ec2"]), default: Some(VarDefault::Str("1g")), passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "ec2_indexer_mem_limit",               hcl_type: HclType::String, sensitive: false, condition: Condition::Engine(&["ec2"]), default: Some(VarDefault::Str("2g")), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "ec2_secret_recovery_window_in_days",  hcl_type: HclType::Number, sensitive: false, condition: Condition::Engine(&["ec2"]), default: Some(VarDefault::Number(7)), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Networking extras ──
        VarEntry { name: "network_environment",             hcl_type: HclType::String, sensitive: false, condition: Condition::Cloud, default: Some(VarDefault::Str("dev")),        passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "network_vpc_cidr",                hcl_type: HclType::String, sensitive: false, condition: Condition::Cloud, default: Some(VarDefault::Str("10.42.0.0/16")), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "network_enable_vpc_endpoints",    hcl_type: HclType::Bool,   sensitive: false, condition: Condition::Cloud, default: Some(VarDefault::Bool(false)),       passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Postgres tuning ──
        VarEntry { name: "postgres_instance_class",             hcl_type: HclType::String, sensitive: false, condition: Condition::ManagedPostgres, default: Some(VarDefault::Str("db.t4g.micro")), passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "postgres_engine_version",             hcl_type: HclType::String, sensitive: false, condition: Condition::ManagedPostgres, default: Some(VarDefault::Str("16.4")),         passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "postgres_db_name",                    hcl_type: HclType::String, sensitive: false, condition: Condition::ManagedPostgres, default: Some(VarDefault::Str("rindexer")),     passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "postgres_db_username",                hcl_type: HclType::String, sensitive: false, condition: Condition::ManagedPostgres, default: Some(VarDefault::Str("rindexer")),     passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "postgres_backup_retention",           hcl_type: HclType::Number, sensitive: false, condition: Condition::ManagedPostgres, default: Some(VarDefault::Number(7)),           passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "postgres_manage_master_user_password", hcl_type: HclType::Bool,  sensitive: false, condition: Condition::ManagedPostgres, default: Some(VarDefault::Bool(true)),          passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "postgres_master_password",            hcl_type: HclType::String, sensitive: true,  condition: Condition::ManagedPostgres, default: Some(VarDefault::Null),                passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: false },
        VarEntry { name: "postgres_force_ssl",                  hcl_type: HclType::Bool,   sensitive: false, condition: Condition::ManagedPostgres, default: Some(VarDefault::Bool(false)),         passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Bare metal extras ──
        VarEntry { name: "bare_metal_rpc_proxy_mem_limit",  hcl_type: HclType::String, sensitive: false, condition: Condition::BareMetal, default: Some(VarDefault::Str("1g")), passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "bare_metal_indexer_mem_limit",    hcl_type: HclType::String, sensitive: false, condition: Condition::BareMetal, default: Some(VarDefault::Str("2g")), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "bare_metal_secrets_encryption",   hcl_type: HclType::String, sensitive: false, condition: Condition::BareMetal, default: Some(VarDefault::Str("none")), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── K3s extras ──
        VarEntry { name: "k3s_version",                     hcl_type: HclType::String, sensitive: false, condition: Condition::Engine(&["k3s"]), default: Some(VarDefault::Str("v1.30.4+k3s1")), passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        // ── Secrets management ──
        VarEntry { name: "secrets_manager_secret_arn",      hcl_type: HclType::String, sensitive: true,  condition: Condition::SecretsMode(&["provider"]),            default: Some(VarDefault::Str("")),           passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: false },
        VarEntry { name: "secrets_manager_kms_key_id",      hcl_type: HclType::String, sensitive: false, condition: Condition::SecretsMode(&["provider"]),            default: Some(VarDefault::Str("")),           passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "external_secret_store_name",      hcl_type: HclType::String, sensitive: false, condition: Condition::SecretsMode(&["external"]),            default: Some(VarDefault::Str("")),           passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "external_secret_key",             hcl_type: HclType::String, sensitive: false, condition: Condition::SecretsMode(&["external"]),            default: Some(VarDefault::Str("")),           passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "eso_chart_version",               hcl_type: HclType::String, sensitive: false, condition: Condition::SecretsMode(&["provider", "external"]), default: Some(VarDefault::Str("0.9.13")),    passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        // ── Monitoring ──
        VarEntry { name: "monitoring_enabled",              hcl_type: HclType::Bool,   sensitive: false, condition: Condition::K8s,        default: Some(VarDefault::Bool(false)),       passthrough: PassthroughMode::Direct, group_break: true,  easy_auto_tfvars: true },
        VarEntry { name: "kube_prometheus_stack_version",   hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("72.6.2")),    passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "grafana_admin_password_secret_name", hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("")),       passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "grafana_ingress_enabled",         hcl_type: HclType::Bool,   sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Bool(true)),       passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "grafana_hostname",                hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("")),          passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "grafana_extra_dashboards",       hcl_type: HclType::Map("string"), sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::EmptyMap), passthrough: PassthroughMode::FilesetMap { power_expr: "{ for f in fileset(\"${path.module}/grafana\", \"*.json\") : f => file(\"${path.module}/grafana/${f}\") }" }, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "alertmanager_slack_webhook_secret_name",      hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("")),   passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "alertmanager_sns_topic_arn",                  hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("")),   passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "alertmanager_pagerduty_routing_key_secret_name", hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("")), passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "alertmanager_route_target",       hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("slack")),     passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "alertmanager_slack_channel",      hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("#alerts")),   passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "loki_enabled",                    hcl_type: HclType::Bool,   sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Bool(false)),      passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "loki_chart_version",              hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("6.24.0")),   passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "promtail_chart_version",          hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("6.16.6")),   passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "loki_persistence_enabled",        hcl_type: HclType::Bool,   sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Bool(false)),      passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
        VarEntry { name: "clickhouse_metrics_url",          hcl_type: HclType::String, sensitive: false, condition: Condition::Monitoring, default: Some(VarDefault::Str("")),          passthrough: PassthroughMode::Direct, group_break: false, easy_auto_tfvars: true },
    ]
}

// ---------------------------------------------------------------------------
// HCL rendering helpers
// ---------------------------------------------------------------------------

impl HclType {
    fn as_hcl(self) -> String {
        match self {
            Self::String => "string".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Number => "number".to_string(),
            Self::List(el) => format!("list({el})"),
            Self::Map(el) => format!("map({el})"),
        }
    }
}

impl VarDefault {
    fn as_hcl(&self, _hcl_type: HclType) -> String {
        match self {
            Self::Str(s) => format!("\"{s}\""),
            Self::Bool(b) => b.to_string(),
            Self::Number(n) => n.to_string(),
            Self::EmptyList => "[]".to_string(),
            Self::EmptyMap => "{}".to_string(),
            Self::Null => "null".to_string(),
            Self::Hcl(raw) => raw.clone(),
        }
    }
}

/// Helper to build a quoted HCL string default.
fn hcl_str(s: &str) -> VarDefault {
    VarDefault::Hcl(format!("\"{s}\""))
}

impl VarEntry {
    /// Render a `variable "name" { ... }` HCL block.
    ///
    /// Easy mode: sensitive vars have no default (must come from secrets.auto.tfvars).
    /// Power mode: sensitive vars get `default = ""`.
    ///
    /// The whitespace alignment matches the existing helpers exactly:
    /// - No default, not sensitive: `type = X` (single space)
    /// - Has default or is non-sensitive with default: `type    = X` (4-char pad)
    /// - Sensitive: `type      = string` (6-char pad) with `sensitive = true`
    pub(crate) fn render_variable_block(&self, mode: GenerationMode) -> String {
        let ty = self.hcl_type.as_hcl();

        let has_default = self.default.is_some();
        let has_power_sensitive_default = self.sensitive && mode == GenerationMode::Power;
        let needs_default_line = has_default || has_power_sensitive_default;

        if self.sensitive {
            // Sensitive var: 6-char padded alignment for `type` and `sensitive`
            let mut lines = vec![
                format!("variable \"{}\" {{", self.name),
                format!("  type      = {ty}"),
            ];
            if has_default {
                lines.push(format!(
                    "  default   = {}",
                    self.default.as_ref().unwrap().as_hcl(self.hcl_type)
                ));
            } else if mode == GenerationMode::Power {
                lines.push("  default   = \"\"".to_string());
            }
            lines.push("  sensitive = true".to_string());
            lines.push("}".to_string());
            lines.join("\n")
        } else if needs_default_line {
            // Non-sensitive with default: 4-char padded alignment
            let default_val = self.default.as_ref().unwrap().as_hcl(self.hcl_type);
            format!(
                "variable \"{}\" {{\n  type    = {ty}\n  default = {default_val}\n}}",
                self.name
            )
        } else {
            // No default, not sensitive: minimal
            format!("variable \"{}\" {{\n  type = {ty}\n}}", self.name)
        }
    }

    /// Render the module argument line: `  name = <expr>`.
    pub(crate) fn render_module_arg(&self, mode: GenerationMode) -> String {
        let expr = match (&self.passthrough, mode) {
            (PassthroughMode::Direct | PassthroughMode::FileContent, GenerationMode::Easy) => {
                format!("var.{}", self.name)
            }
            (PassthroughMode::Direct, GenerationMode::Power) => {
                format!("var.{}", self.name)
            }
            (PassthroughMode::FileContent, GenerationMode::Power) => {
                format!("file(var.{})", self.name)
            }
            (PassthroughMode::FilesetMap { .. }, GenerationMode::Easy) => {
                format!("var.{}", self.name)
            }
            (PassthroughMode::FilesetMap { power_expr }, GenerationMode::Power) => {
                power_expr.to_string()
            }
        };
        format!("  {} = {}", self.name, expr)
    }
}

// ---------------------------------------------------------------------------
// Top-level generation entry points
// ---------------------------------------------------------------------------

/// Generate the full `variables.tf` content from the manifest.
pub(crate) fn render_variables_tf(resolved: &ResolvedConfig, mode: GenerationMode) -> String {
    let mut entries = manifest();
    // Apply user-selected defaults from config/init wizard.
    for entry in &mut entries {
        if entry.default.is_none() {
            if let Some(val) = resolved.user_defaults.get(entry.name) {
                entry.default = Some(val.clone());
            }
        }
    }
    let blocks: Vec<String> = entries
        .iter()
        .filter(|v| resolved.matches(&v.condition))
        // Power mode: FilesetMap vars don't get a variable declaration — the
        // expression is used inline in main.tf (e.g. rindexer_abis uses fileset()).
        .filter(|v| {
            !(mode == GenerationMode::Power
                && matches!(v.passthrough, PassthroughMode::FilesetMap { .. }))
        })
        .map(|v| v.render_variable_block(mode))
        .collect();
    format!("{}\n", blocks.join("\n\n"))
}

/// Generate the module argument lines for `module "evm_cloud" { ... }`.
pub(crate) fn render_module_args(
    resolved: &ResolvedConfig,
    mode: GenerationMode,
    module_source: &str,
) -> String {
    let entries = manifest();
    let mut lines = vec![format!("  source = \"{module_source}\""), String::new()];
    for entry in &entries {
        if !resolved.matches(&entry.condition) {
            continue;
        }
        if entry.group_break {
            lines.push(String::new());
        }
        lines.push(entry.render_module_arg(mode));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::InfrastructureProvider;
    use crate::init_answers::InitMode;
    use crate::init_answers::{DatabaseProfile, IndexerConfigStrategy, InitAnswers};
    use std::collections::BTreeMap;

    // -----------------------------------------------------------------------
    // Helpers for building test configs
    // -----------------------------------------------------------------------

    fn easy_config_aws_clickhouse_ec2() -> crate::config::schema::EvmCloudConfig {
        let toml_str = r#"
schema_version = 1

[project]
name = "test"
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
"#;
        toml::from_str(toml_str).unwrap()
    }

    fn easy_config_aws_postgres_k3s() -> crate::config::schema::EvmCloudConfig {
        let toml_str = r#"
schema_version = 1

[project]
name = "test"
region = "us-east-1"

[compute]
engine = "k3s"
instance_type = "t3.medium"

[database]
mode = "managed"
provider = "aws"
storage_backend = "postgres"

[indexer]
config_path = "rindexer.yaml"
chains = ["ethereum"]

[rpc]
endpoints = { ethereum = "https://rpc.example" }

[ingress]
mode = "none"

[secrets]
mode = "inline"
"#;
        toml::from_str(toml_str).unwrap()
    }

    fn easy_config_bare_metal_clickhouse() -> crate::config::schema::EvmCloudConfig {
        let toml_str = r#"
schema_version = 1

[project]
name = "test"

[compute]
engine = "docker_compose"

[database]
mode = "self_hosted"
provider = "bare_metal"

[indexer]
config_path = "rindexer.yaml"
chains = ["polygon"]

[rpc]
endpoints = { polygon = "https://rpc.example" }

[ingress]
mode = "none"

[secrets]
mode = "inline"
"#;
        toml::from_str(toml_str).unwrap()
    }

    fn power_answers_aws_clickhouse_ec2() -> InitAnswers {
        InitAnswers {
            mode: InitMode::Power,
            project_name: "test".to_string(),
            infrastructure_provider: InfrastructureProvider::Aws,
            region: Some("us-east-1".to_string()),
            compute_engine: ComputeEngine::Ec2,
            instance_type: Some("t3.small".to_string()),
            workload_mode: None,
            database_profile: DatabaseProfile::ByodbClickhouse,
            chains: vec!["polygon".to_string()],
            rpc_endpoints: BTreeMap::from([(
                "polygon".to_string(),
                "https://rpc.example".to_string(),
            )]),
            indexer_config: IndexerConfigStrategy::Generate,
            generate_erpc_config: true,
            ingress_mode: IngressMode::None,
            erpc_hostname: None,
            ingress_tls_email: None,
            state_config: None,
            auto_bootstrap: false,
        }
    }

    fn power_answers_aws_postgres_k3s() -> InitAnswers {
        InitAnswers {
            mode: InitMode::Power,
            project_name: "test".to_string(),
            infrastructure_provider: InfrastructureProvider::Aws,
            region: Some("us-east-1".to_string()),
            compute_engine: ComputeEngine::K3s,
            instance_type: Some("t3.medium".to_string()),
            workload_mode: None,
            database_profile: DatabaseProfile::ManagedRds,
            chains: vec!["ethereum".to_string()],
            rpc_endpoints: BTreeMap::from([(
                "ethereum".to_string(),
                "https://rpc.example".to_string(),
            )]),
            indexer_config: IndexerConfigStrategy::Generate,
            generate_erpc_config: true,
            ingress_mode: IngressMode::None,
            erpc_hostname: None,
            ingress_tls_email: None,
            state_config: None,
            auto_bootstrap: false,
        }
    }

    fn power_answers_bare_metal_clickhouse() -> InitAnswers {
        InitAnswers {
            mode: InitMode::Power,
            project_name: "test".to_string(),
            infrastructure_provider: InfrastructureProvider::BareMetal,
            region: None,
            compute_engine: ComputeEngine::DockerCompose,
            instance_type: None,
            workload_mode: None,
            database_profile: DatabaseProfile::ByodbClickhouse,
            chains: vec!["polygon".to_string()],
            rpc_endpoints: BTreeMap::from([(
                "polygon".to_string(),
                "https://rpc.example".to_string(),
            )]),
            indexer_config: IndexerConfigStrategy::Generate,
            generate_erpc_config: true,
            ingress_mode: IngressMode::None,
            erpc_hostname: None,
            ingress_tls_email: None,
            state_config: None,
            auto_bootstrap: false,
        }
    }

    // -----------------------------------------------------------------------
    // Helper: extract variable names from generated variables.tf
    // -----------------------------------------------------------------------

    fn extract_var_names(variables_tf: &str) -> Vec<String> {
        variables_tf
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with("variable \"") {
                    let name = trimmed
                        .strip_prefix("variable \"")
                        .and_then(|s| s.strip_suffix("\" {"))
                        .map(|s| s.to_string());
                    name
                } else {
                    None
                }
            })
            .collect()
    }

    fn extract_module_arg_names(main_tf: &str) -> Vec<String> {
        main_tf
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.contains(" = ")
                    && !trimmed.starts_with("source")
                    && !trimmed.starts_with("module")
                    && !trimmed.starts_with("//")
                    && !trimmed.starts_with("#")
                    && !trimmed.starts_with("required_version")
                {
                    let name = trimmed.split('=').next().map(|s| s.trim().to_string());
                    name.filter(|n| !n.is_empty() && !n.contains('{') && !n.contains('"'))
                } else {
                    None
                }
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Snapshot tests: verify expected variable name sets per config combo
    // -----------------------------------------------------------------------

    // Shared suffixes for new vars per config combo
    // aws+clickhouse+ec2 (secrets=provider): Always + Cloud + Engine(ec2) + Clickhouse + SecretsMode(provider)
    // aws+postgres+k3s (secrets=inline, k8s, managed_postgres): Always + Cloud + Engine(k3s) + Postgres + K8s + ManagedPostgres
    // bare_metal+clickhouse (secrets=inline): Always + BareMetal + Clickhouse

    #[test]
    fn snapshot_easy_aws_clickhouse_ec2_variable_names() {
        let config = easy_config_aws_clickhouse_ec2();
        let resolved = ResolvedConfig::from_evm_config(&config);
        let names = extract_var_names(&render_variables_tf(&resolved, GenerationMode::Easy));

        let expected = vec![
            "project_name",
            "infrastructure_provider",
            "database_mode",
            "compute_engine",
            "workload_mode",
            "secrets_mode",
            "ingress_mode",
            "erpc_hostname",
            "ingress_tls_email",
            "ssh_private_key_path",
            "networking_enabled",
            "aws_region",
            "ssh_public_key",
            "network_availability_zones",
            "network_enable_nat_gateway",
            "ec2_instance_type",
            "indexer_storage_backend",
            "indexer_clickhouse_url",
            "indexer_clickhouse_user",
            "indexer_clickhouse_password",
            "indexer_clickhouse_db",
            "rpc_proxy_enabled",
            "indexer_enabled",
            "indexer_rpc_url",
            "erpc_config_yaml",
            "rindexer_config_yaml",
            "rindexer_abis",
            "indexer_extra_env",
            "indexer_extra_secret_env",
            "deployment_target",
            "runtime_arch",
            "streaming_mode",
            "rpc_proxy_image",
            "indexer_image",
            "ec2_rpc_proxy_mem_limit",
            "ec2_indexer_mem_limit",
            "ec2_secret_recovery_window_in_days",
            "network_environment",
            "network_vpc_cidr",
            "network_enable_vpc_endpoints",
            "secrets_manager_secret_arn",
            "secrets_manager_kms_key_id",
            "eso_chart_version",
        ];
        assert_eq!(names, expected, "aws+clickhouse+ec2 easy");
    }

    #[test]
    fn snapshot_easy_aws_postgres_k3s_variable_names() {
        let config = easy_config_aws_postgres_k3s();
        let resolved = ResolvedConfig::from_evm_config(&config);
        let names = extract_var_names(&render_variables_tf(&resolved, GenerationMode::Easy));

        let expected = vec![
            "project_name",
            "infrastructure_provider",
            "database_mode",
            "compute_engine",
            "workload_mode",
            "secrets_mode",
            "ingress_mode",
            "erpc_hostname",
            "ingress_tls_email",
            "ssh_private_key_path",
            "networking_enabled",
            "aws_region",
            "ssh_public_key",
            "network_availability_zones",
            "network_enable_nat_gateway",
            "k3s_instance_type",
            "k3s_api_allowed_cidrs",
            "indexer_storage_backend",
            "postgres_enabled",
            "rpc_proxy_enabled",
            "indexer_enabled",
            "indexer_rpc_url",
            "erpc_config_yaml",
            "rindexer_config_yaml",
            "rindexer_abis",
            "indexer_extra_env",
            "indexer_extra_secret_env",
            "deployment_target",
            "runtime_arch",
            "streaming_mode",
            "rpc_proxy_image",
            "indexer_image",
            "ingress_class_name",
            "network_environment",
            "network_vpc_cidr",
            "network_enable_vpc_endpoints",
            "postgres_instance_class",
            "postgres_engine_version",
            "postgres_db_name",
            "postgres_db_username",
            "postgres_backup_retention",
            "postgres_manage_master_user_password",
            "postgres_master_password",
            "postgres_force_ssl",
            "k3s_version",
            "monitoring_enabled",
        ];
        assert_eq!(names, expected, "aws+postgres+k3s easy");
    }

    #[test]
    fn snapshot_easy_bare_metal_clickhouse_variable_names() {
        let config = easy_config_bare_metal_clickhouse();
        let resolved = ResolvedConfig::from_evm_config(&config);
        let names = extract_var_names(&render_variables_tf(&resolved, GenerationMode::Easy));

        let expected = vec![
            "project_name",
            "infrastructure_provider",
            "database_mode",
            "compute_engine",
            "workload_mode",
            "secrets_mode",
            "ingress_mode",
            "erpc_hostname",
            "ingress_tls_email",
            "ssh_private_key_path",
            "bare_metal_host",
            "bare_metal_ssh_user",
            "bare_metal_ssh_port",
            "indexer_storage_backend",
            "indexer_clickhouse_url",
            "indexer_clickhouse_user",
            "indexer_clickhouse_password",
            "indexer_clickhouse_db",
            "rpc_proxy_enabled",
            "indexer_enabled",
            "indexer_rpc_url",
            "erpc_config_yaml",
            "rindexer_config_yaml",
            "rindexer_abis",
            "indexer_extra_env",
            "indexer_extra_secret_env",
            "deployment_target",
            "runtime_arch",
            "streaming_mode",
            "rpc_proxy_image",
            "indexer_image",
            "bare_metal_rpc_proxy_mem_limit",
            "bare_metal_indexer_mem_limit",
            "bare_metal_secrets_encryption",
        ];
        assert_eq!(names, expected, "bare_metal+clickhouse easy");
    }

    #[test]
    fn snapshot_power_aws_clickhouse_ec2_variable_names() {
        let answers = power_answers_aws_clickhouse_ec2();
        let resolved = ResolvedConfig::from_init_answers(&answers);
        let names = extract_var_names(&render_variables_tf(&resolved, GenerationMode::Power));

        // Power mode: no rindexer_abis (uses fileset inline)
        // power_answers sets secrets_mode = "provider" (aws + non-k3s)
        let expected = vec![
            "project_name",
            "infrastructure_provider",
            "database_mode",
            "compute_engine",
            "workload_mode",
            "secrets_mode",
            "ingress_mode",
            "erpc_hostname",
            "ingress_tls_email",
            "ssh_private_key_path",
            "networking_enabled",
            "aws_region",
            "ssh_public_key",
            "network_availability_zones",
            "network_enable_nat_gateway",
            "ec2_instance_type",
            "indexer_storage_backend",
            "indexer_clickhouse_url",
            "indexer_clickhouse_user",
            "indexer_clickhouse_password",
            "indexer_clickhouse_db",
            "rpc_proxy_enabled",
            "indexer_enabled",
            "indexer_rpc_url",
            "erpc_config_yaml",
            "rindexer_config_yaml",
            "indexer_extra_env",
            "indexer_extra_secret_env",
            "deployment_target",
            "runtime_arch",
            "streaming_mode",
            "rpc_proxy_image",
            "indexer_image",
            "ec2_rpc_proxy_mem_limit",
            "ec2_indexer_mem_limit",
            "ec2_secret_recovery_window_in_days",
            "network_environment",
            "network_vpc_cidr",
            "network_enable_vpc_endpoints",
            "secrets_manager_secret_arn",
            "secrets_manager_kms_key_id",
            "eso_chart_version",
        ];
        assert_eq!(names, expected, "aws+clickhouse+ec2 power");
    }

    #[test]
    fn snapshot_power_aws_postgres_k3s_variable_names() {
        let answers = power_answers_aws_postgres_k3s();
        let resolved = ResolvedConfig::from_init_answers(&answers);
        let names = extract_var_names(&render_variables_tf(&resolved, GenerationMode::Power));

        // k3s → secrets_mode = "inline", is_k8s = true, ManagedRds → is_managed_postgres = true
        let expected = vec![
            "project_name",
            "infrastructure_provider",
            "database_mode",
            "compute_engine",
            "workload_mode",
            "secrets_mode",
            "ingress_mode",
            "erpc_hostname",
            "ingress_tls_email",
            "ssh_private_key_path",
            "networking_enabled",
            "aws_region",
            "ssh_public_key",
            "network_availability_zones",
            "network_enable_nat_gateway",
            "k3s_instance_type",
            "k3s_api_allowed_cidrs",
            "indexer_storage_backend",
            "postgres_enabled",
            "rpc_proxy_enabled",
            "indexer_enabled",
            "indexer_rpc_url",
            "erpc_config_yaml",
            "rindexer_config_yaml",
            "indexer_extra_env",
            "indexer_extra_secret_env",
            "deployment_target",
            "runtime_arch",
            "streaming_mode",
            "rpc_proxy_image",
            "indexer_image",
            "ingress_class_name",
            "network_environment",
            "network_vpc_cidr",
            "network_enable_vpc_endpoints",
            "postgres_instance_class",
            "postgres_engine_version",
            "postgres_db_name",
            "postgres_db_username",
            "postgres_backup_retention",
            "postgres_manage_master_user_password",
            "postgres_master_password",
            "postgres_force_ssl",
            "k3s_version",
            "monitoring_enabled",
        ];
        assert_eq!(names, expected, "aws+postgres+k3s power");
    }

    #[test]
    fn snapshot_power_bare_metal_clickhouse_variable_names() {
        let answers = power_answers_bare_metal_clickhouse();
        let resolved = ResolvedConfig::from_init_answers(&answers);
        let names = extract_var_names(&render_variables_tf(&resolved, GenerationMode::Power));

        let expected = vec![
            "project_name",
            "infrastructure_provider",
            "database_mode",
            "compute_engine",
            "workload_mode",
            "secrets_mode",
            "ingress_mode",
            "erpc_hostname",
            "ingress_tls_email",
            "ssh_private_key_path",
            "bare_metal_host",
            "bare_metal_ssh_user",
            "bare_metal_ssh_port",
            "indexer_storage_backend",
            "indexer_clickhouse_url",
            "indexer_clickhouse_user",
            "indexer_clickhouse_password",
            "indexer_clickhouse_db",
            "rpc_proxy_enabled",
            "indexer_enabled",
            "indexer_rpc_url",
            "erpc_config_yaml",
            "rindexer_config_yaml",
            "indexer_extra_env",
            "indexer_extra_secret_env",
            "deployment_target",
            "runtime_arch",
            "streaming_mode",
            "rpc_proxy_image",
            "indexer_image",
            "bare_metal_rpc_proxy_mem_limit",
            "bare_metal_indexer_mem_limit",
            "bare_metal_secrets_encryption",
        ];
        assert_eq!(names, expected, "bare_metal+clickhouse power");
    }

    #[test]
    fn snapshot_easy_aws_clickhouse_ec2_module_arg_names() {
        let config = easy_config_aws_clickhouse_ec2();
        let resolved = ResolvedConfig::from_evm_config(&config);
        let names = extract_module_arg_names(&render_module_args(
            &resolved,
            GenerationMode::Easy,
            "test-source",
        ));

        let expected = vec![
            "project_name",
            "infrastructure_provider",
            "database_mode",
            "compute_engine",
            "workload_mode",
            "secrets_mode",
            "ingress_mode",
            "erpc_hostname",
            "ingress_tls_email",
            "ssh_private_key_path",
            "networking_enabled",
            "aws_region",
            "ssh_public_key",
            "network_availability_zones",
            "network_enable_nat_gateway",
            "ec2_instance_type",
            "indexer_storage_backend",
            "indexer_clickhouse_url",
            "indexer_clickhouse_user",
            "indexer_clickhouse_password",
            "indexer_clickhouse_db",
            "rpc_proxy_enabled",
            "indexer_enabled",
            "indexer_rpc_url",
            "erpc_config_yaml",
            "rindexer_config_yaml",
            "rindexer_abis",
            "indexer_extra_env",
            "indexer_extra_secret_env",
            "deployment_target",
            "runtime_arch",
            "streaming_mode",
            "rpc_proxy_image",
            "indexer_image",
            "ec2_rpc_proxy_mem_limit",
            "ec2_indexer_mem_limit",
            "ec2_secret_recovery_window_in_days",
            "network_environment",
            "network_vpc_cidr",
            "network_enable_vpc_endpoints",
            "secrets_manager_secret_arn",
            "secrets_manager_kms_key_id",
            "eso_chart_version",
        ];
        assert_eq!(names, expected, "module args aws+clickhouse+ec2 easy");
    }

    #[test]
    fn snapshot_easy_bare_metal_module_arg_names() {
        let config = easy_config_bare_metal_clickhouse();
        let resolved = ResolvedConfig::from_evm_config(&config);
        let names = extract_module_arg_names(&render_module_args(
            &resolved,
            GenerationMode::Easy,
            "test-source",
        ));

        let expected = vec![
            "project_name",
            "infrastructure_provider",
            "database_mode",
            "compute_engine",
            "workload_mode",
            "secrets_mode",
            "ingress_mode",
            "erpc_hostname",
            "ingress_tls_email",
            "ssh_private_key_path",
            "bare_metal_host",
            "bare_metal_ssh_user",
            "bare_metal_ssh_port",
            "indexer_storage_backend",
            "indexer_clickhouse_url",
            "indexer_clickhouse_user",
            "indexer_clickhouse_password",
            "indexer_clickhouse_db",
            "rpc_proxy_enabled",
            "indexer_enabled",
            "indexer_rpc_url",
            "erpc_config_yaml",
            "rindexer_config_yaml",
            "rindexer_abis",
            "indexer_extra_env",
            "indexer_extra_secret_env",
            "deployment_target",
            "runtime_arch",
            "streaming_mode",
            "rpc_proxy_image",
            "indexer_image",
            "bare_metal_rpc_proxy_mem_limit",
            "bare_metal_indexer_mem_limit",
            "bare_metal_secrets_encryption",
        ];
        assert_eq!(names, expected, "module args bare_metal easy");
    }

    // -----------------------------------------------------------------------
    // Original unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn manifest_has_no_duplicate_names() {
        let entries = manifest();
        let mut seen = std::collections::HashSet::new();
        for e in &entries {
            assert!(seen.insert(e.name), "duplicate manifest entry: {}", e.name);
        }
    }

    fn test_resolved(
        is_bare_metal: bool,
        is_postgres: bool,
        engine: ComputeEngine,
    ) -> ResolvedConfig {
        let is_k8s = matches!(engine, ComputeEngine::K3s | ComputeEngine::Eks);
        ResolvedConfig {
            is_bare_metal,
            is_postgres,
            is_managed_postgres: false,
            is_k8s,
            is_monitoring: false,
            engine,
            ingress_mode: IngressMode::None,
            secrets_mode_val: "inline".to_string(),
            user_defaults: HashMap::new(),
        }
    }

    #[test]
    fn resolved_config_always_matches() {
        let rc = test_resolved(false, false, ComputeEngine::Ec2);
        assert!(rc.matches(&Condition::Always));
    }

    #[test]
    fn resolved_config_bare_metal() {
        let rc = test_resolved(true, false, ComputeEngine::DockerCompose);
        assert!(rc.matches(&Condition::BareMetal));
        assert!(!rc.matches(&Condition::Cloud));
    }

    #[test]
    fn resolved_config_engine_match() {
        let rc = test_resolved(false, false, ComputeEngine::K3s);
        assert!(rc.matches(&Condition::Engine(&["k3s"])));
        assert!(!rc.matches(&Condition::Engine(&["ec2"])));
    }

    #[test]
    fn render_variable_block_easy_simple() {
        let entry = VarEntry {
            name: "project_name",
            hcl_type: HclType::String,
            sensitive: false,
            condition: Condition::Always,
            default: None,
            passthrough: PassthroughMode::Direct,
            group_break: false,
            easy_auto_tfvars: true,
        };
        let block = entry.render_variable_block(GenerationMode::Easy);
        assert_eq!(block, "variable \"project_name\" {\n  type = string\n}");
    }

    #[test]
    fn render_variable_block_easy_with_default() {
        let entry = VarEntry {
            name: "bare_metal_ssh_user",
            hcl_type: HclType::String,
            sensitive: false,
            condition: Condition::BareMetal,
            default: Some(VarDefault::Str("ubuntu")),
            passthrough: PassthroughMode::Direct,
            group_break: false,
            easy_auto_tfvars: true,
        };
        let block = entry.render_variable_block(GenerationMode::Easy);
        assert_eq!(
            block,
            "variable \"bare_metal_ssh_user\" {\n  type    = string\n  default = \"ubuntu\"\n}"
        );
    }

    #[test]
    fn render_variable_block_easy_sensitive() {
        let entry = VarEntry {
            name: "bare_metal_host",
            hcl_type: HclType::String,
            sensitive: true,
            condition: Condition::BareMetal,
            default: None,
            passthrough: PassthroughMode::Direct,
            group_break: false,
            easy_auto_tfvars: false,
        };
        let block = entry.render_variable_block(GenerationMode::Easy);
        assert_eq!(
            block,
            "variable \"bare_metal_host\" {\n  type      = string\n  sensitive = true\n}"
        );
    }

    #[test]
    fn render_variable_block_power_sensitive() {
        let entry = VarEntry {
            name: "bare_metal_host",
            hcl_type: HclType::String,
            sensitive: true,
            condition: Condition::BareMetal,
            default: None,
            passthrough: PassthroughMode::Direct,
            group_break: false,
            easy_auto_tfvars: false,
        };
        let block = entry.render_variable_block(GenerationMode::Power);
        assert_eq!(
            block,
            "variable \"bare_metal_host\" {\n  type      = string\n  default   = \"\"\n  sensitive = true\n}"
        );
    }

    #[test]
    fn render_module_arg_direct() {
        let entry = VarEntry {
            name: "project_name",
            hcl_type: HclType::String,
            sensitive: false,
            condition: Condition::Always,
            default: None,
            passthrough: PassthroughMode::Direct,
            group_break: false,
            easy_auto_tfvars: true,
        };
        assert_eq!(
            entry.render_module_arg(GenerationMode::Easy),
            "  project_name = var.project_name"
        );
    }

    #[test]
    fn render_module_arg_file_content_power() {
        let entry = VarEntry {
            name: "erpc_config_yaml",
            hcl_type: HclType::String,
            sensitive: false,
            condition: Condition::Always,
            default: None,
            passthrough: PassthroughMode::FileContent,
            group_break: false,
            easy_auto_tfvars: true,
        };
        assert_eq!(
            entry.render_module_arg(GenerationMode::Power),
            "  erpc_config_yaml = file(var.erpc_config_yaml)"
        );
    }

    #[test]
    fn render_module_arg_fileset_power() {
        let entry = VarEntry {
            name: "rindexer_abis",
            hcl_type: HclType::Map("string"),
            sensitive: false,
            condition: Condition::Always,
            default: Some(VarDefault::EmptyMap),
            passthrough: PassthroughMode::FilesetMap {
                power_expr: "{ for f in fileset(\"${path.module}/config/abis\", \"*.json\") : f => file(\"${path.module}/config/abis/${f}\") }",
            },
            group_break: false,
            easy_auto_tfvars: true,
        };
        assert!(entry
            .render_module_arg(GenerationMode::Power)
            .contains("fileset"));
        assert_eq!(
            entry.render_module_arg(GenerationMode::Easy),
            "  rindexer_abis = var.rindexer_abis"
        );
    }

    // -----------------------------------------------------------------------
    // HCL integration tests
    // -----------------------------------------------------------------------

    /// Verify generated variables.tf has balanced braces and valid `variable "..." { }` blocks.
    fn assert_valid_hcl_variables(hcl: &str) {
        let mut brace_depth: i32 = 0;
        for ch in hcl.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth -= 1,
                _ => {}
            }
            assert!(brace_depth >= 0, "unbalanced closing brace in variables.tf");
        }
        assert_eq!(brace_depth, 0, "unbalanced braces in variables.tf");

        // Every `variable "..." {` line should have a matching `}`
        for line in hcl.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("variable \"") {
                assert!(
                    trimmed.ends_with('{'),
                    "variable block opening missing brace: {trimmed}"
                );
                assert!(
                    trimmed.contains('"'),
                    "variable block missing quoted name: {trimmed}"
                );
            }
        }
    }

    /// Verify every `var.X` in module args has a matching `variable "X"` declaration.
    fn assert_module_args_have_variables(variables_tf: &str, module_args: &str) {
        let declared: std::collections::HashSet<&str> = variables_tf
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with("variable \"") {
                    let start = trimmed.find('"')? + 1;
                    let end = trimmed[start..].find('"')? + start;
                    Some(&trimmed[start..end])
                } else {
                    None
                }
            })
            .collect();

        for line in module_args.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.split(" = ").nth(1) {
                // Extract var.NAME references
                if let Some(var_name) = rest.strip_prefix("var.") {
                    assert!(
                        declared.contains(var_name),
                        "module arg references var.{var_name} but no variable \"{var_name}\" declared"
                    );
                }
            }
        }
    }

    #[test]
    fn hcl_integration_easy_aws_ec2() {
        let config = easy_config_aws_clickhouse_ec2();
        let resolved = ResolvedConfig::from_evm_config(&config);
        let variables = render_variables_tf(&resolved, GenerationMode::Easy);
        let module = render_module_args(&resolved, GenerationMode::Easy, "test-source");

        assert_valid_hcl_variables(&variables);
        assert_module_args_have_variables(&variables, &module);
    }

    #[test]
    fn hcl_integration_easy_bare_metal() {
        let config = easy_config_bare_metal_clickhouse();
        let resolved = ResolvedConfig::from_evm_config(&config);
        let variables = render_variables_tf(&resolved, GenerationMode::Easy);
        let module = render_module_args(&resolved, GenerationMode::Easy, "test-source");

        assert_valid_hcl_variables(&variables);
        assert_module_args_have_variables(&variables, &module);
    }

    #[test]
    fn hcl_integration_power_aws_k3s() {
        let answers = power_answers_aws_postgres_k3s();
        let resolved = ResolvedConfig::from_init_answers(&answers);
        let variables = render_variables_tf(&resolved, GenerationMode::Power);
        let module = render_module_args(&resolved, GenerationMode::Power, "test-source");

        assert_valid_hcl_variables(&variables);
        // Power mode has fileset() expressions that don't use var.X — skip strict var check
        // but verify no var.X is undeclared
        assert_module_args_have_variables(&variables, &module);
    }

    #[test]
    fn hcl_integration_power_bare_metal() {
        let answers = power_answers_bare_metal_clickhouse();
        let resolved = ResolvedConfig::from_init_answers(&answers);
        let variables = render_variables_tf(&resolved, GenerationMode::Power);
        let module = render_module_args(&resolved, GenerationMode::Power, "test-source");

        assert_valid_hcl_variables(&variables);
        assert_module_args_have_variables(&variables, &module);
    }
}
