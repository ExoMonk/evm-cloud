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
    #[serde(default)]
    pub(crate) state: Option<StateConfig>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IndexerType {
    Rindexer,
    Custom,
}

impl Default for IndexerType {
    fn default() -> Self {
        Self::Rindexer
    }
}

impl IndexerType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Rindexer => "rindexer",
            Self::Custom => "custom",
        }
    }

    pub(crate) fn is_custom(self) -> bool {
        self == Self::Custom
    }
}

impl std::fmt::Display for IndexerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndexerConfig {
    #[serde(default)]
    pub(crate) indexer_type: IndexerType,
    pub(crate) config_path: PathBuf,
    #[serde(default)]
    pub(crate) erpc_config_path: Option<PathBuf>,
    pub(crate) chains: Vec<String>,
    #[serde(default)]
    pub(crate) extra_env: BTreeMap<String, String>,
    /// Custom indexer: override the container command (e.g. `["node", "dist/index.js"]`).
    #[serde(default)]
    pub(crate) custom_command: Option<Vec<String>>,
    /// Custom indexer: override the health endpoint path (e.g. `/healthz`).
    #[serde(default)]
    pub(crate) custom_health_path: Option<String>,
    /// Custom indexer: override the health endpoint port.
    #[serde(default)]
    pub(crate) custom_health_port: Option<u16>,
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

/// Minimal config for bootstrap — only needs project name + state.
///
/// Uses `deny_unknown_fields`: a full TOML (with `[compute]`, `[database]`, etc.)
/// will fail to parse into this struct. This is intentional —
/// `load_for_bootstrap()` tries the full `EvmCloudConfig` parse first, then
/// falls back to this. If you add fields here, verify the invariant holds:
/// a full TOML must still fail `BootstrapConfig` parse so the fallback
/// ordering remains correct.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BootstrapConfig {
    pub(crate) schema_version: Option<u32>,
    pub(crate) project: BootstrapProjectConfig,
    #[serde(default)]
    pub(crate) state: Option<StateConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BootstrapProjectConfig {
    pub(crate) name: String,
}

// State backend is DECOUPLED from infrastructure provider.
// Users can deploy on AWS but store state in GCS, or deploy on
// bare_metal but store state in S3. The backend choice is orthogonal.
//
// NOTE: This enum covers `backend {}` block codegen only.
// Terraform Cloud (`cloud {}` block) would need a separate codegen path.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, tag = "backend")]
pub(crate) enum StateConfig {
    #[serde(rename = "s3")]
    S3 {
        bucket: String,
        dynamodb_table: String,
        region: String,
        #[serde(default)]
        key: Option<String>,
        #[serde(default = "default_encrypt")]
        encrypt: bool,
    },
    #[serde(rename = "gcs")]
    Gcs {
        bucket: String,
        region: String,
        #[serde(default)]
        prefix: Option<String>,
    },
}

fn default_encrypt() -> bool {
    true
}

impl StateConfig {
    pub(crate) fn resolve_defaults(&mut self, project_name: &str) {
        match self {
            StateConfig::S3 { key, .. } => {
                if key.is_none() {
                    *key = Some(format!("{project_name}/terraform.tfstate"));
                }
            }
            StateConfig::Gcs { prefix, .. } => {
                if prefix.is_none() {
                    *prefix = Some(project_name.to_string());
                }
            }
        }
    }

    pub(crate) fn is_encrypt_disabled(&self) -> bool {
        matches!(self, StateConfig::S3 { encrypt: false, .. })
    }

    pub(crate) fn backend_type(&self) -> &'static str {
        match self {
            StateConfig::S3 { .. } => "s3",
            StateConfig::Gcs { .. } => "gcs",
        }
    }

    pub(crate) fn tfbackend_filename(&self, project_name: &str) -> String {
        format!("{}.{}.tfbackend", project_name, self.backend_type())
    }

    /// Render key-value pairs for a `.tfbackend` file.
    /// Call `resolve_defaults()` first to fill in key/prefix.
    pub(crate) fn render_tfbackend(&self) -> String {
        match self {
            StateConfig::S3 {
                bucket,
                dynamodb_table,
                region,
                key,
                encrypt,
            } => {
                let key_str = key.as_deref().unwrap_or("terraform.tfstate");
                format!(
                    "bucket         = \"{bucket}\"\n\
                     key            = \"{key_str}\"\n\
                     region         = \"{region}\"\n\
                     dynamodb_table = \"{dynamodb_table}\"\n\
                     encrypt        = {encrypt}\n"
                )
            }
            // `region` excluded — Terraform's GCS backend does not accept it.
            StateConfig::Gcs { bucket, prefix, .. } => {
                let mut out = format!("bucket = \"{bucket}\"\n");
                if let Some(p) = prefix.as_deref() {
                    if !p.is_empty() {
                        out.push_str(&format!("prefix = \"{p}\"\n"));
                    }
                }
                out
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_s3_state_config_from_toml() {
        let toml_str = r#"
backend = "s3"
bucket = "my-state-bucket"
dynamodb_table = "my-lock-table"
region = "us-east-1"
key = "my-project/terraform.tfstate"
encrypt = true
"#;
        let config: StateConfig = toml::from_str(toml_str).expect("must parse S3 state config");
        match config {
            StateConfig::S3 {
                bucket,
                dynamodb_table,
                region,
                key,
                encrypt,
            } => {
                assert_eq!(bucket, "my-state-bucket");
                assert_eq!(dynamodb_table, "my-lock-table");
                assert_eq!(region, "us-east-1");
                assert_eq!(key.as_deref(), Some("my-project/terraform.tfstate"));
                assert!(encrypt);
            }
            _ => panic!("expected S3 variant"),
        }
    }

    #[test]
    fn parses_gcs_state_config_from_toml() {
        let toml_str = r#"
backend = "gcs"
bucket = "my-state-bucket"
region = "us-central1"
prefix = "my-project"
"#;
        let config: StateConfig = toml::from_str(toml_str).expect("must parse GCS state config");
        match config {
            StateConfig::Gcs {
                bucket,
                region,
                prefix,
            } => {
                assert_eq!(bucket, "my-state-bucket");
                assert_eq!(region, "us-central1");
                assert_eq!(prefix.as_deref(), Some("my-project"));
            }
            _ => panic!("expected Gcs variant"),
        }
    }

    #[test]
    fn s3_defaults_key_and_encrypt() {
        let toml_str = r#"
backend = "s3"
bucket = "my-bucket"
dynamodb_table = "my-lock"
region = "us-east-1"
"#;
        let config: StateConfig = toml::from_str(toml_str).expect("must parse");
        match config {
            StateConfig::S3 { key, encrypt, .. } => {
                assert!(key.is_none());
                assert!(encrypt);
            }
            _ => panic!("expected S3"),
        }
    }

    #[test]
    fn gcs_defaults_prefix() {
        let toml_str = r#"
backend = "gcs"
bucket = "my-bucket"
region = "us-central1"
"#;
        let config: StateConfig = toml::from_str(toml_str).expect("must parse");
        match config {
            StateConfig::Gcs { prefix, .. } => assert!(prefix.is_none()),
            _ => panic!("expected Gcs"),
        }
    }

    #[test]
    fn resolve_defaults_fills_s3_key() {
        let mut config: StateConfig = toml::from_str(
            r#"
backend = "s3"
bucket = "b"
dynamodb_table = "t"
region = "r"
"#,
        )
        .unwrap();
        config.resolve_defaults("demo");
        match config {
            StateConfig::S3 { key, .. } => {
                assert_eq!(key.as_deref(), Some("demo/terraform.tfstate"))
            }
            _ => panic!("expected S3"),
        }
    }

    #[test]
    fn resolve_defaults_fills_gcs_prefix() {
        let mut config: StateConfig = toml::from_str(
            r#"
backend = "gcs"
bucket = "b"
region = "us-central1"
"#,
        )
        .unwrap();
        config.resolve_defaults("demo");
        match config {
            StateConfig::Gcs { prefix, .. } => assert_eq!(prefix.as_deref(), Some("demo")),
            _ => panic!("expected Gcs"),
        }
    }

    #[test]
    fn rejects_unknown_backend() {
        let toml_str = r#"
backend = "azurerm"
bucket = "my-bucket"
"#;
        let err = toml::from_str::<StateConfig>(toml_str).expect_err("must reject unknown backend");
        let msg = err.to_string();
        assert!(
            msg.contains("s3") || msg.contains("gcs"),
            "error should mention valid variants: {msg}"
        );
    }

    #[test]
    fn no_state_section_parses_as_none() {
        // Simulate EvmCloudConfig without [state]
        #[derive(Deserialize)]
        struct Partial {
            #[serde(default)]
            state: Option<StateConfig>,
        }
        let config: Partial = toml::from_str("").expect("must parse empty");
        assert!(config.state.is_none());
    }

    #[test]
    fn backend_type_s3() {
        let config: StateConfig = toml::from_str(
            r#"
backend = "s3"
bucket = "b"
dynamodb_table = "t"
region = "r"
"#,
        )
        .unwrap();
        assert_eq!(config.backend_type(), "s3");
    }

    #[test]
    fn backend_type_gcs() {
        let config: StateConfig = toml::from_str(
            r#"
backend = "gcs"
bucket = "b"
region = "r"
"#,
        )
        .unwrap();
        assert_eq!(config.backend_type(), "gcs");
    }

    #[test]
    fn tfbackend_filename_format() {
        let config: StateConfig = toml::from_str(
            r#"
backend = "s3"
bucket = "b"
dynamodb_table = "t"
region = "r"
"#,
        )
        .unwrap();
        assert_eq!(
            config.tfbackend_filename("myproject"),
            "myproject.s3.tfbackend"
        );
    }

    #[test]
    fn render_tfbackend_s3() {
        let mut config: StateConfig = toml::from_str(
            r#"
backend = "s3"
bucket = "my-bucket"
dynamodb_table = "my-lock"
region = "us-east-1"
"#,
        )
        .unwrap();
        config.resolve_defaults("demo");
        let rendered = config.render_tfbackend();
        assert!(rendered.contains("bucket         = \"my-bucket\""));
        assert!(rendered.contains("key            = \"demo/terraform.tfstate\""));
        assert!(rendered.contains("region         = \"us-east-1\""));
        assert!(rendered.contains("dynamodb_table = \"my-lock\""));
        assert!(rendered.contains("encrypt        = true"));
        assert!(!rendered.contains("backend"));
    }

    #[test]
    fn render_tfbackend_gcs() {
        let mut config: StateConfig = toml::from_str(
            r#"
backend = "gcs"
bucket = "my-bucket"
region = "us-central1"
"#,
        )
        .unwrap();
        config.resolve_defaults("demo");
        let rendered = config.render_tfbackend();
        assert!(rendered.contains("bucket = \"my-bucket\""));
        assert!(rendered.contains("prefix = \"demo\""));
        assert!(!rendered.contains("region"));
    }

    #[test]
    fn s3_encrypt_false_detected() {
        let toml_str = r#"
backend = "s3"
bucket = "b"
dynamodb_table = "t"
region = "r"
encrypt = false
"#;
        let config: StateConfig = toml::from_str(toml_str).unwrap();
        assert!(config.is_encrypt_disabled());
    }
}
