use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error(
        "terraform not found on PATH. Install: https://developer.hashicorp.com/terraform/install"
    )]
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

    #[error("could not locate repository root from `{start}` (expected a parent directory containing `examples/`)")]
    ExampleRepoRootNotFound { start: String },

    #[error("unknown example `{requested}`. Available: {available:?}")]
    ExampleNotFound {
        requested: String,
        available: Vec<String>,
    },

    #[error("example alias collision for `{alias}` between `{first}` and `{second}`")]
    ExampleAliasCollision {
        alias: String,
        first: String,
        second: String,
    },

    #[error("symlinks are not supported in example bootstrap: {path}")]
    ExampleSymlinkUnsupported { path: PathBuf },

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

    #[error("deploy timed out after {seconds}s")]
    DeployerTimedOut { seconds: u64 },

    #[error("{tool} not found on PATH. Install it and retry")]
    PrerequisiteNotFound { tool: String },

    #[error("Docker daemon is not running. Start Docker Desktop and retry")]
    DockerNotRunning,

    #[error("port {port} is already in use. Free it before running `evm-cloud local up`")]
    PortConflict { port: u16 },

    #[error("kind cluster `{name}` exists but is unreachable. Run with --force to recreate")]
    ClusterUnreachable { name: String },

    #[error("health check timed out waiting for {service} at {url}")]
    HealthCheckTimeout { service: String, url: String },

    #[error("failed to probe chain ID from {url}: {details}")]
    ChainIdProbeFailed { url: String, details: String },

    #[error("rindexer config not found at {path}. Create one or use --config to specify a path")]
    RindexerConfigNotFound { path: String },

    #[error("{tool} failed: {details}")]
    ToolFailed { tool: String, details: String },

    #[error("system clock error: {0}")]
    SystemClock(String),

    #[error("flag conflict: {message}")]
    FlagConflict { message: String },

    #[error("kubeconfig not found at {path}")]
    KubeconfigNotFound { path: PathBuf },

    #[error("kubeconfig generation is only supported for k3s/eks; current compute_engine is `{compute_engine}`")]
    KubeconfigUnsupportedEngine { compute_engine: String },

    #[error("failed to fetch examples from GitHub: {details}")]
    ExampleFetchFailed { details: String },

    #[error("example archive invalid: {details}")]
    ExampleArchiveInvalid { details: String },

    #[error("example path escapes root: {path}")]
    ExamplePathEscape { path: PathBuf },

    #[error("deployer thread panicked")]
    DeployerThreadPanicked,

    #[error(
        "missing required file `{file}` for deploy. Provide --config-dir or create `config/{file}`"
    )]
    DeployConfigFileMissing { file: String },

    #[error("interactive prompt failed: {0}")]
    PromptFailed(String),

    #[error("failed to spawn `{command}`: {source}")]
    CommandSpawn {
        command: String,
        source: std::io::Error,
    },

    #[error("io error at {path}: {source}")]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },
}

pub(crate) type Result<T> = std::result::Result<T, CliError>;

/// Map a process exit status to a `CliError`.
///
/// `on_code` receives the non-zero exit code.
/// `on_signal` receives the optional signal number (Unix only; `None` on other platforms).
pub(crate) fn map_exit_status(
    status: std::process::ExitStatus,
    on_code: impl FnOnce(i32) -> CliError,
    on_signal: impl FnOnce(Option<i32>) -> CliError,
) -> Result<()> {
    if status.success() {
        return Ok(());
    }

    if let Some(code) = status.code() {
        return Err(on_code(code));
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        Err(on_signal(status.signal()))
    }

    #[cfg(not(unix))]
    {
        Err(on_signal(None))
    }
}

/// Convenience wrapper for external tool exit status → `CliError::ToolFailed`.
pub(crate) fn tool_exit_status(status: std::process::ExitStatus, tool: &str) -> Result<()> {
    let tool_for_code = tool.to_string();
    let tool_for_signal = tool_for_code.clone();
    map_exit_status(
        status,
        |code| CliError::ToolFailed {
            tool: tool_for_code,
            details: format!("exited with status code {code}"),
        },
        |signal| CliError::ToolFailed {
            tool: tool_for_signal,
            details: match signal {
                Some(sig) => format!("terminated by signal {sig}"),
                None => "terminated unexpectedly".to_string(),
            },
        },
    )
}
