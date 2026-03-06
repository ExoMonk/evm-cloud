use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde::Deserialize;

use crate::config::schema::{
    ComputeEngine, EvmCloudConfig, InfrastructureProvider, IngressMode, WorkloadMode,
};
use crate::error::{CliError, Result};

#[derive(Debug, Clone, Copy, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InitMode {
    Easy,
    Power,
}

impl InitMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Easy => "easy",
            Self::Power => "power",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Easy => "🧪 easy",
            Self::Power => "⚡️ power",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DatabaseProfile {
    ByodbClickhouse,
    ByodbPostgres,
    #[serde(alias = "self_hosted_rds")]
    ManagedRds,
    #[serde(alias = "self_hosted_clickhouse")]
    ManagedClickhouse,
}

#[derive(Debug, Clone)]
pub(crate) enum IndexerConfigStrategy {
    Generate,
    Existing(PathBuf),
}

#[derive(Debug, Clone)]
pub(crate) struct InitAnswers {
    pub(crate) mode: InitMode,
    pub(crate) project_name: String,
    pub(crate) infrastructure_provider: InfrastructureProvider,
    pub(crate) region: Option<String>,
    pub(crate) compute_engine: ComputeEngine,
    pub(crate) instance_type: Option<String>,
    pub(crate) workload_mode: Option<WorkloadMode>,
    pub(crate) database_profile: DatabaseProfile,
    pub(crate) chains: Vec<String>,
    pub(crate) rpc_endpoints: BTreeMap<String, String>,
    pub(crate) indexer_config: IndexerConfigStrategy,
    pub(crate) generate_erpc_config: bool,
    pub(crate) ingress_mode: IngressMode,
    pub(crate) erpc_hostname: Option<String>,
    pub(crate) ingress_tls_email: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnswersFile {
    mode: Option<InitMode>,
    project_name: String,
    infrastructure_provider: Option<InfrastructureProvider>,
    region: Option<String>,
    compute_engine: ComputeEngine,
    instance_type: Option<String>,
    workload_mode: Option<WorkloadMode>,
    database_profile: Option<DatabaseProfile>,
    chains: Vec<String>,
    rpc_endpoints: BTreeMap<String, String>,
    indexer_config: Option<IndexerConfigFile>,
    generate_erpc_config: Option<bool>,
    ingress_mode: Option<IngressMode>,
    erpc_hostname: Option<String>,
    ingress_tls_email: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexerConfigFile {
    strategy: Option<String>,
    path: Option<PathBuf>,
}

pub(crate) fn load_from_config(
    path: &Path,
    mode_override: Option<InitMode>,
) -> Result<InitAnswers> {
    let raw = fs::read_to_string(path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })?;

    if let Ok(config) = toml::from_str::<EvmCloudConfig>(&raw) {
        let mut answers = from_runtime_config(config);
        if let Some(mode) = mode_override {
            answers.mode = mode;
        }
        return Ok(answers);
    }

    let parsed: AnswersFile = toml::from_str(&raw).map_err(|err| CliError::ConfigParse {
        path: path.to_path_buf(),
        details: err.to_string(),
    })?;

    from_answers_file(parsed, mode_override)
}

fn from_runtime_config(config: EvmCloudConfig) -> InitAnswers {
    let database_profile = match (
        config.database.mode.as_str(),
        config.database.storage_backend.as_str(),
    ) {
        ("managed", "postgres") => DatabaseProfile::ManagedRds,
        ("managed", _) => DatabaseProfile::ManagedClickhouse,
        (_, "postgres") => DatabaseProfile::ByodbPostgres,
        _ => DatabaseProfile::ByodbClickhouse,
    };

    let generate_erpc_config = config.indexer.erpc_config_path.is_none();

    InitAnswers {
        mode: InitMode::Easy,
        project_name: config.project.name,
        infrastructure_provider: config.database.provider,
        region: config.project.region,
        compute_engine: config.compute.engine,
        instance_type: config.compute.instance_type,
        workload_mode: config.compute.workload_mode,
        database_profile,
        chains: config.indexer.chains,
        rpc_endpoints: config.rpc.endpoints,
        indexer_config: IndexerConfigStrategy::Existing(config.indexer.config_path),
        generate_erpc_config,
        ingress_mode: config.ingress.mode,
        erpc_hostname: config.ingress.domain,
        ingress_tls_email: config.ingress.tls_email,
    }
}

fn from_answers_file(file: AnswersFile, mode_override: Option<InitMode>) -> Result<InitAnswers> {
    if file.chains.is_empty() {
        return Err(CliError::ConfigValidation {
            field: "chains".to_string(),
            message: "at least one chain is required".to_string(),
        });
    }

    for chain in &file.chains {
        let endpoint = file
            .rpc_endpoints
            .get(chain)
            .map(|value| value.trim())
            .unwrap_or_default();
        if endpoint.is_empty() {
            return Err(CliError::ConfigValidation {
                field: format!("rpc_endpoints.{chain}"),
                message: "missing endpoint for selected chain".to_string(),
            });
        }
    }

    let provider = file
        .infrastructure_provider
        .unwrap_or(InfrastructureProvider::Aws);
    let valid_engines = ComputeEngine::valid_for_provider(provider);
    if !valid_engines.contains(&file.compute_engine) {
        let allowed: Vec<&str> = valid_engines.iter().map(ComputeEngine::as_str).collect();
        return Err(CliError::ConfigValidation {
            field: "compute_engine".to_string(),
            message: format!(
                "`{}` is not valid for provider `{}`. Valid engines: {}",
                file.compute_engine.as_str(),
                provider.as_str(),
                allowed.join(", "),
            ),
        });
    }

    let ingress_mode = file.ingress_mode.unwrap_or(IngressMode::None);
    let valid_ingress = IngressMode::options_for_engine(file.compute_engine);
    if !valid_ingress.contains(&ingress_mode) {
        let allowed: Vec<&str> = valid_ingress.iter().map(IngressMode::as_str).collect();
        return Err(CliError::ConfigValidation {
            field: "ingress_mode".to_string(),
            message: format!(
                "`{}` is not valid for compute engine `{}`. Valid modes: {}",
                ingress_mode.as_str(),
                file.compute_engine.as_str(),
                allowed.join(", "),
            ),
        });
    }

    let indexer_config = match file.indexer_config {
        Some(config) => match config.strategy.as_deref() {
            Some("existing") => {
                let path = config.path.ok_or_else(|| CliError::ConfigValidation {
                    field: "indexer_config.path".to_string(),
                    message: "required when strategy=existing".to_string(),
                })?;
                IndexerConfigStrategy::Existing(path)
            }
            Some("generate") | None => IndexerConfigStrategy::Generate,
            Some(other) => {
                return Err(CliError::ConfigValidation {
                    field: "indexer_config.strategy".to_string(),
                    message: format!("unsupported strategy `{other}`"),
                })
            }
        },
        None => IndexerConfigStrategy::Generate,
    };

    Ok(InitAnswers {
        mode: mode_override.or(file.mode).unwrap_or(InitMode::Easy),
        project_name: file.project_name,
        infrastructure_provider: provider,
        region: file.region,
        compute_engine: file.compute_engine,
        instance_type: file.instance_type,
        workload_mode: file.workload_mode,
        database_profile: file
            .database_profile
            .unwrap_or(DatabaseProfile::ByodbClickhouse),
        chains: file.chains,
        rpc_endpoints: file.rpc_endpoints,
        indexer_config,
        generate_erpc_config: file.generate_erpc_config.unwrap_or(true),
        ingress_mode,
        erpc_hostname: file.erpc_hostname.map(|h| sanitize_hostname(&h)),
        ingress_tls_email: file.ingress_tls_email,
    })
}

fn sanitize_hostname(raw: &str) -> String {
    let s = raw.trim();
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    s.trim_end_matches('/').to_string()
}
