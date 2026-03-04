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

    #[error("terraform output `{output}` not found in state")]
    TerraformOutputMissing { output: String },

    #[error("no evm-cloud.toml or *.tf files found in {dir}")]
    NoProjectDetected { dir: String },

    #[error("ambiguous project root in {dir}. candidates: {candidates:?}")]
    AmbiguousProjectRoot {
        dir: String,
        candidates: Vec<String>,
    },

    #[error("raw Terraform root detected in {dir}. Pass --allow-raw-terraform to proceed or add evm-cloud.toml")]
    RawTerraformOptInRequired { dir: String },

    #[error("cannot determine project mode in {dir}: both evm-cloud.toml and explicit Terraform root are present. {remediation}")]
    ModeRoutingAmbiguous { dir: String, remediation: String },

    #[error("invalid mode marker at {path}: `{value}` (expected `easy` or `power` and matching project files)")]
    InvalidModeMarker { path: PathBuf, value: String },

    #[error("invalid .evm-cloud-version at {path}: `{value}` (expected semver-like value such as `v0.1.0`)")]
    PinnedVersionInvalid { path: PathBuf, value: String },

    #[error("evm-cloud version mismatch: project requires `{required}` from {path}, current CLI is `{current}`")]
    PinnedVersionMismatch {
        path: PathBuf,
        required: String,
        current: String,
    },

    #[error("failed to probe terraform version: {details}")]
    TerraformVersionProbeFailed { details: String },

    #[error("failed to parse terraform output: {0}")]
    OutputParseError(#[from] serde_json::Error),

    #[error("failed to parse config at {path}: {details}")]
    ConfigParse { path: PathBuf, details: String },

    #[error("--non-interactive requires --config <answers.toml|evm-cloud.toml>")]
    NonInteractiveRequiresConfig,

    #[error("invalid config field `{field}`: {message}")]
    ConfigValidation { field: String, message: String },

    #[error("unsupported schema_version={found}. This CLI supports schema_version=1")]
    UnsupportedSchemaVersion { found: u32 },

    #[error("missing terraform output `workload_handoff` (module: {module}). Try `terraform output -json workload_handoff` or pass --module-name")]
    HandoffMissing { module: String },

    #[error("invalid handoff field `{field}`: {details}")]
    HandoffInvalid { field: String, details: String },

    #[error("unsupported handoff version `{found}`. Expected `{expected}`. Upgrade the CLI or regenerate handoff output")]
    HandoffVersionUnsupported { found: String, expected: String },

    #[error("unsupported compute_engine `{compute_engine}` for deploy orchestration. Supported in CLI1.3: ec2, docker_compose, k3s")]
    DeployerUnsupportedEngine { compute_engine: String },

    #[error("failed to write bundled script at {path}: {source}")]
    BundledScriptWrite {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("bundled script checksum mismatch for {script}")]
    BundledScriptChecksumMismatch { script: String },

    #[error("managed init file already exists: {path}")]
    InitFileExists { path: PathBuf },

    #[error("deploy lock already held at {path}")]
    DeployLockBusy { path: PathBuf },

    #[error("deployer process failed with exit code {code}")]
    DeployerFailed { code: i32 },

    #[error("deployer process terminated by signal {signal:?}")]
    DeployerSignaled { signal: Option<i32> },

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
