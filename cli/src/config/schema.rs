use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectConfig {
    pub(crate) name: String,
    pub(crate) region: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ComputeConfig {
    pub(crate) engine: ComputeEngine,
    pub(crate) instance_type: String,
}

#[derive(Debug, Clone, Deserialize)]
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DatabaseConfig {
    pub(crate) mode: String,
    pub(crate) provider: String,
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
    pub(crate) mode: String,
    #[serde(default)]
    pub(crate) domain: Option<String>,
    #[serde(default)]
    pub(crate) tls_email: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SecretsConfig {
    pub(crate) mode: String,
}
