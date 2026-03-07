use std::fs;

use crate::config::schema::{ComputeEngine, EvmCloudConfig, InfrastructureProvider, IngressMode, StateConfig};
use crate::error::{CliError, Result};

pub(crate) fn validate(config: &EvmCloudConfig) -> Result<()> {
    validate_non_empty("project.name", &config.project.name)?;

    if config.database.provider == InfrastructureProvider::Aws {
        if let Some(ref region) = config.project.region {
            validate_non_empty("project.region", region)?;
        } else {
            return Err(CliError::ConfigValidation {
                field: "project.region".to_string(),
                message: "required when infrastructure provider is aws".to_string(),
            });
        }
    }

    if let Some(ref instance_type) = config.compute.instance_type {
        validate_non_empty("compute.instance_type", instance_type)?;
    }

    validate_non_empty("database.mode", &config.database.mode)?;
    validate_engine_provider(config.compute.engine, config.database.provider)?;
    validate_ingress_engine(config.ingress.mode, config.compute.engine)?;
    validate_non_empty("secrets.mode", &config.secrets.mode)?;

    if config.indexer.chains.is_empty() {
        return Err(CliError::ConfigValidation {
            field: "indexer.chains".to_string(),
            message: "must contain at least one chain".to_string(),
        });
    }

    validate_existing_file("indexer.config_path", &config.indexer.config_path)?;

    if let Some(path) = &config.indexer.erpc_config_path {
        validate_existing_file("indexer.erpc_config_path", path)?;
    }

    if config.rpc.endpoints.is_empty() && config.indexer.erpc_config_path.is_none() {
        return Err(CliError::ConfigValidation {
            field: "rpc.endpoints".to_string(),
            message:
                "must include at least one endpoint when indexer.erpc_config_path is not provided"
                    .to_string(),
        });
    }

    if let Some(ref state) = config.state {
        validate_state(state)?;
    }

    Ok(())
}

fn validate_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(CliError::ConfigValidation {
            field: field.to_string(),
            message: "must be non-empty".to_string(),
        });
    }
    Ok(())
}

fn validate_engine_provider(engine: ComputeEngine, provider: InfrastructureProvider) -> Result<()> {
    let valid = ComputeEngine::valid_for_provider(provider);
    if !valid.contains(&engine) {
        let allowed: Vec<&str> = valid.iter().map(ComputeEngine::as_str).collect();
        return Err(CliError::ConfigValidation {
            field: "compute.engine".to_string(),
            message: format!(
                "`{}` is not valid for provider `{}`. Valid engines: {}",
                engine.as_str(),
                provider.as_str(),
                allowed.join(", "),
            ),
        });
    }
    Ok(())
}

fn validate_ingress_engine(ingress: IngressMode, engine: ComputeEngine) -> Result<()> {
    let valid = IngressMode::options_for_engine(engine);
    if !valid.contains(&ingress) {
        let allowed: Vec<&str> = valid.iter().map(IngressMode::as_str).collect();
        return Err(CliError::ConfigValidation {
            field: "ingress.mode".to_string(),
            message: format!(
                "`{}` is not valid for compute engine `{}`. Valid modes: {}",
                ingress.as_str(),
                engine.as_str(),
                allowed.join(", "),
            ),
        });
    }
    Ok(())
}

fn validate_state(state: &StateConfig) -> Result<()> {
    match state {
        StateConfig::S3 { bucket, dynamodb_table, region, key, .. } => {
            validate_non_empty("state.bucket", bucket)?;
            validate_non_empty("state.dynamodb_table", dynamodb_table)?;
            validate_non_empty("state.region", region)?;
            validate_hcl_safe("state.bucket", bucket)?;
            validate_hcl_safe("state.dynamodb_table", dynamodb_table)?;
            validate_hcl_safe("state.region", region)?;
            if let Some(ref k) = key {
                validate_hcl_safe("state.key", k)?;
            }
        }
        StateConfig::Gcs { bucket, prefix } => {
            validate_non_empty("state.bucket", bucket)?;
            validate_hcl_safe("state.bucket", bucket)?;
            if let Some(ref p) = prefix {
                validate_hcl_safe("state.prefix", p)?;
            }
        }
    }
    Ok(())
}

/// Reject characters that would break HCL string interpolation.
fn validate_hcl_safe(field: &str, value: &str) -> Result<()> {
    if value.contains('"') || value.contains('\\') || value.contains('\n') || value.contains('\r') {
        return Err(CliError::ConfigValidation {
            field: field.to_string(),
            message: "contains invalid characters (quotes, backslashes, or newlines are not allowed in HCL values)".to_string(),
        });
    }
    Ok(())
}

fn validate_existing_file(field: &str, path: &std::path::Path) -> Result<()> {
    let metadata = fs::metadata(path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })?;

    if !metadata.is_file() {
        return Err(CliError::ConfigValidation {
            field: field.to_string(),
            message: format!("expected file at {}", path.display()),
        });
    }

    Ok(())
}
