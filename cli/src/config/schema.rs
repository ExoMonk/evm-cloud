use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvmCloudConfig {
    pub(crate) schema_version: Option<u32>,
    pub(crate) project: ProjectConfig,
    pub(crate) compute: ComputeConfig,
    pub(crate) database: DatabaseConfig,
    pub(crate) indexer: IndexerConfig,
    pub(crate) rpc: RpcConfig,
    pub(crate) ingress: IngressConfig,
    pub(crate) secrets: SecretsConfig,
    #[serde(default)]
    pub(crate) networking: Option<NetworkingConfig>,
    #[serde(default)]
    pub(crate) postgres: Option<PostgresConfig>,
    #[serde(default)]
    pub(crate) containers: Option<ContainerConfig>,
    #[serde(default)]
    pub(crate) monitoring: Option<MonitoringConfig>,
    #[serde(default)]
    pub(crate) bare_metal: Option<BareMetalConfig>,
    #[serde(default)]
    pub(crate) streaming: Option<StreamingConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectConfig {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) region: Option<String>,
    #[serde(default)]
    pub(crate) deployment_target: Option<String>,
    #[serde(default)]
    pub(crate) runtime_arch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ComputeConfig {
    pub(crate) engine: ComputeEngine,
    #[serde(default)]
    pub(crate) instance_type: Option<String>,
    #[serde(default)]
    pub(crate) workload_mode: Option<WorkloadMode>,
    #[serde(default)]
    pub(crate) ec2: Option<Ec2Config>,
    #[serde(default)]
    pub(crate) k3s: Option<K3sConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Ec2Config {
    #[serde(default)]
    pub(crate) rpc_proxy_mem_limit: Option<String>,
    #[serde(default)]
    pub(crate) indexer_mem_limit: Option<String>,
    #[serde(default)]
    pub(crate) secret_recovery_window_in_days: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct K3sConfig {
    #[serde(default)]
    pub(crate) version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ComputeEngine {
    Ec2,
    Eks,
    K3s,
    DockerCompose,
}

impl ComputeEngine {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Ec2 => "ec2",
            Self::Eks => "eks",
            Self::K3s => "k3s",
            Self::DockerCompose => "docker_compose",
        }
    }

    /// Returns the valid compute engines for a given infrastructure provider.
    pub(crate) fn valid_for_provider(provider: InfrastructureProvider) -> &'static [ComputeEngine] {
        match provider {
            InfrastructureProvider::Aws => {
                &[ComputeEngine::Ec2, ComputeEngine::Eks, ComputeEngine::K3s]
            }
            InfrastructureProvider::BareMetal => {
                &[ComputeEngine::K3s, ComputeEngine::DockerCompose]
            }
        }
    }
}

impl std::fmt::Display for ComputeEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IngressMode {
    None,
    Cloudflare,
    Caddy,
    IngressNginx,
}

impl IngressMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Cloudflare => "cloudflare",
            Self::Caddy => "caddy",
            Self::IngressNginx => "ingress_nginx",
        }
    }

    /// Returns the valid ingress modes for a given compute engine.
    pub(crate) fn options_for_engine(engine: ComputeEngine) -> &'static [IngressMode] {
        match engine {
            // k8s engines support all modes
            ComputeEngine::K3s | ComputeEngine::Eks => &[
                IngressMode::None,
                IngressMode::Cloudflare,
                IngressMode::Caddy,
                IngressMode::IngressNginx,
            ],
            // non-k8s: no ingress_nginx (requires k8s ingress controller)
            ComputeEngine::Ec2 | ComputeEngine::DockerCompose => &[
                IngressMode::None,
                IngressMode::Cloudflare,
                IngressMode::Caddy,
            ],
        }
    }

    pub(crate) fn requires_hostname(self) -> bool {
        self != Self::None
    }

    pub(crate) fn requires_tls_email(self) -> bool {
        matches!(self, Self::Caddy | Self::IngressNginx)
    }
}

impl std::fmt::Display for IngressMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InfrastructureProvider {
    Aws,
    BareMetal,
}

impl InfrastructureProvider {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Aws => "aws",
            Self::BareMetal => "bare_metal",
        }
    }

    pub(crate) fn is_bare_metal(self) -> bool {
        self == Self::BareMetal
    }
}

impl std::fmt::Display for InfrastructureProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkloadMode {
    Terraform,
    External,
}

impl WorkloadMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Terraform => "terraform",
            Self::External => "external",
        }
    }

    /// Infer the default workload mode for a given compute engine.
    pub(crate) fn default_for_engine(engine: ComputeEngine) -> Self {
        match engine {
            ComputeEngine::K3s | ComputeEngine::Eks => Self::External,
            ComputeEngine::Ec2 | ComputeEngine::DockerCompose => Self::Terraform,
        }
    }
}

impl std::fmt::Display for WorkloadMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DatabaseConfig {
    pub(crate) mode: String,
    pub(crate) provider: InfrastructureProvider,
    #[serde(default = "default_storage_backend")]
    pub(crate) storage_backend: String,
}

fn default_storage_backend() -> String {
    "clickhouse".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndexerConfig {
    pub(crate) config_path: PathBuf,
    #[serde(default)]
    pub(crate) erpc_config_path: Option<PathBuf>,
    pub(crate) chains: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RpcConfig {
    pub(crate) endpoints: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IngressConfig {
    pub(crate) mode: IngressMode,
    #[serde(default)]
    pub(crate) domain: Option<String>,
    #[serde(default)]
    pub(crate) tls_email: Option<String>,
    #[serde(default)]
    pub(crate) cloudflare_ssl_mode: Option<String>,
    #[serde(default)]
    pub(crate) caddy_image: Option<String>,
    #[serde(default)]
    pub(crate) caddy_mem_limit: Option<String>,
    #[serde(default)]
    pub(crate) nginx_chart_version: Option<String>,
    #[serde(default)]
    pub(crate) cert_manager_chart_version: Option<String>,
    #[serde(default)]
    pub(crate) request_body_max_size: Option<String>,
    #[serde(default)]
    pub(crate) tls_staging: Option<bool>,
    #[serde(default)]
    pub(crate) hsts_preload: Option<bool>,
    #[serde(default)]
    pub(crate) class_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SecretsConfig {
    pub(crate) mode: String,
    #[serde(default)]
    pub(crate) kms_key_id: Option<String>,
    #[serde(default)]
    pub(crate) external_store_name: Option<String>,
    #[serde(default)]
    pub(crate) external_secret_key: Option<String>,
    #[serde(default)]
    pub(crate) eso_chart_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NetworkingConfig {
    #[serde(default)]
    pub(crate) vpc_cidr: Option<String>,
    #[serde(default)]
    pub(crate) enable_vpc_endpoints: Option<bool>,
    #[serde(default)]
    pub(crate) environment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PostgresConfig {
    #[serde(default)]
    pub(crate) instance_class: Option<String>,
    #[serde(default)]
    pub(crate) engine_version: Option<String>,
    #[serde(default)]
    pub(crate) db_name: Option<String>,
    #[serde(default)]
    pub(crate) db_username: Option<String>,
    #[serde(default)]
    pub(crate) backup_retention: Option<i64>,
    #[serde(default)]
    pub(crate) manage_master_user_password: Option<bool>,
    #[serde(default)]
    pub(crate) force_ssl: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContainerConfig {
    #[serde(default)]
    pub(crate) rpc_proxy_image: Option<String>,
    #[serde(default)]
    pub(crate) indexer_image: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MonitoringConfig {
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) kube_prometheus_stack_version: Option<String>,
    #[serde(default)]
    pub(crate) grafana_ingress_enabled: Option<bool>,
    #[serde(default)]
    pub(crate) grafana_hostname: Option<String>,
    #[serde(default)]
    pub(crate) alertmanager_route_target: Option<String>,
    #[serde(default)]
    pub(crate) alertmanager_slack_channel: Option<String>,
    #[serde(default)]
    pub(crate) loki_enabled: Option<bool>,
    #[serde(default)]
    pub(crate) loki_chart_version: Option<String>,
    #[serde(default)]
    pub(crate) promtail_chart_version: Option<String>,
    #[serde(default)]
    pub(crate) loki_persistence_enabled: Option<bool>,
    #[serde(default)]
    pub(crate) clickhouse_metrics_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BareMetalConfig {
    #[serde(default)]
    pub(crate) rpc_proxy_mem_limit: Option<String>,
    #[serde(default)]
    pub(crate) indexer_mem_limit: Option<String>,
    #[serde(default)]
    pub(crate) secrets_encryption: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StreamingConfig {
    #[serde(default = "default_streaming_mode")]
    pub(crate) mode: String,
}

fn default_streaming_mode() -> String {
    "disabled".to_string()
}
