use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error("terraform not found on PATH. Install: https://developer.hashicorp.com/terraform/install")]
    TerraformNotFound,

    #[error("terraform version {found} is below minimum required {minimum}")]
    TerraformVersionTooOld { found: String, minimum: String },

    #[error("terraform command failed with exit code {code}")]
    TerraformFailed { code: i32 },

    #[error("terraform process terminated by signal {signal:?}")]
    TerraformSignaled { signal: Option<i32> },

    #[error("no evm-cloud.toml or *.tf files found in {dir}")]
    NoProjectDetected { dir: String },

    #[error("ambiguous project root in {dir}. candidates: {candidates:?}")]
    AmbiguousProjectRoot {
        dir: String,
        candidates: Vec<String>,
    },

    #[error("raw Terraform root detected in {dir}. Pass --allow-raw-terraform to proceed or add evm-cloud.toml")]
    RawTerraformOptInRequired { dir: String },

    #[error("failed to probe terraform version: {details}")]
    TerraformVersionProbeFailed { details: String },

    #[error("failed to parse terraform output: {0}")]
    OutputParseError(#[from] serde_json::Error),

    #[error("failed to parse config at {path}: {details}")]
    ConfigParse { path: PathBuf, details: String },

    #[error("invalid config field `{field}`: {message}")]
    ConfigValidation { field: String, message: String },

    #[error("unsupported schema_version={found}. This CLI supports schema_version=1")]
    UnsupportedSchemaVersion { found: u32 },

    #[error("{0}")]
    Message(String),

    #[error("io error at {path}: {source}")]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub(crate) type Result<T> = std::result::Result<T, CliError>;
