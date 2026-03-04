use std::fs;

use crate::config::schema::EvmCloudConfig;
use crate::error::{CliError, Result};

pub(crate) fn validate(config: &EvmCloudConfig) -> Result<()> {
    validate_non_empty("project.name", &config.project.name)?;
    validate_non_empty("project.region", &config.project.region)?;
    validate_non_empty("compute.instance_type", &config.compute.instance_type)?;
    validate_non_empty("database.mode", &config.database.mode)?;
    validate_non_empty("database.provider", &config.database.provider)?;
    validate_non_empty("ingress.mode", &config.ingress.mode)?;
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
            message: "must include at least one endpoint when indexer.erpc_config_path is not provided"
                .to_string(),
        });
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
