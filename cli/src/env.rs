use std::fs;
use std::path::{Path, PathBuf};

use crate::config::schema::StateConfig;
use crate::error::{CliError, Result};

/// Reserved directory/file names that cannot be used as environment names.
const RESERVED_NAMES: &[&str] = &[
    "envs",
    "config",
    ".evm-cloud",
    ".terraform",
    "terraform",
    "modules",
    "state",
    "backend",
    "src",
    "scripts",
    "node_modules",
];

/// Context for a resolved multi-environment deployment.
#[derive(Debug, Clone)]
pub(crate) struct EnvContext {
    /// Environment name (e.g. "staging", "prod").
    pub name: String,
    /// Path to `envs/<name>/`.
    #[allow(dead_code)] // Used by future `env add/list/remove` commands
    pub dir: PathBuf,
    /// Path to the `.tfbackend` file inside `envs/<name>/`.
    pub tfbackend: PathBuf,
    /// Optional per-env tfvars file (`envs/<name>/<name>.tfvars`).
    pub tfvars: Option<PathBuf>,
    /// Auto-discovered `*.auto.tfvars` files in `envs/<name>/`.
    /// Terraform only auto-loads these from its cwd (project root), so the CLI
    /// must explicitly pass them as `-var-file` when running with `--env`.
    pub auto_tfvars: Vec<PathBuf>,
    /// Isolated terraform data directory (`envs/<name>/.terraform/`).
    pub tf_data_dir: PathBuf,
}

/// Summary info for listing environments.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Used by future `env list` command
pub(crate) struct EnvInfo {
    pub name: String,
    pub dir: PathBuf,
    pub initialized: bool,
}

/// Resolve the active environment from the `--env` flag, `EVM_CLOUD_ENV` env var,
/// or the presence of an `envs/` directory.
///
/// Returns `Ok(None)` when the project is NOT multi-env (no `envs/` dir).
pub(crate) fn resolve_env(flag: Option<&str>, project_root: &Path) -> Result<Option<EnvContext>> {
    let envs_dir = project_root.join("envs");

    // Determine env name from flag or env var.
    let name_from_source = flag.map(String::from).or_else(|| {
        std::env::var("EVM_CLOUD_ENV")
            .ok()
            .filter(|v| !v.is_empty())
    });

    let envs_exists = envs_dir.is_dir();

    // If a name is provided but envs/ doesn't exist, error.
    if name_from_source.is_some() && !envs_exists {
        return Err(CliError::EnvNotMultiEnv);
    }

    // If envs/ doesn't exist, this is not a multi-env project.
    if !envs_exists {
        return Ok(None);
    }

    // Count subdirectories in envs/.
    let subdirs = list_env_subdirs(&envs_dir)?;

    // If envs/ exists but has no subdirs, treat as non-multi-env —
    // unless the user explicitly specified an env name, in which case error.
    if subdirs.is_empty() {
        if let Some(name) = name_from_source {
            return Err(CliError::EnvNotFound {
                name,
                available: String::new(),
            });
        }
        return Ok(None);
    }

    // envs/ has 1+ subdirs but no name was provided -- error.
    let name = match name_from_source {
        Some(n) => n,
        None => {
            let envs_list = subdirs.join(", ");
            return Err(CliError::EnvRequired { envs: envs_list });
        }
    };

    // Validate and build context.
    validate_env_name(&name)?;
    let ctx = build_env_context(&name, project_root)?;

    // Check TF_DATA_DIR conflict.
    if let Ok(existing) = std::env::var("TF_DATA_DIR") {
        let expected = ctx.tf_data_dir.display().to_string();
        if !existing.is_empty() && existing != expected {
            return Err(CliError::TfDataDirConflict {
                existing,
                env: name,
                expected,
            });
        }
    }

    Ok(Some(ctx))
}

/// Validate that an environment name is well-formed.
pub(crate) fn validate_env_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 32 {
        return Err(CliError::InvalidEnvName {
            name: name.to_string(),
            reason: "must be 1-32 characters".to_string(),
        });
    }

    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(CliError::InvalidEnvName {
            name: name.to_string(),
            reason: "only alphanumeric characters and hyphens are allowed".to_string(),
        });
    }

    if name.starts_with('-') || name.ends_with('-') {
        return Err(CliError::InvalidEnvName {
            name: name.to_string(),
            reason: "must not start or end with a hyphen".to_string(),
        });
    }

    if RESERVED_NAMES.contains(&name) {
        return Err(CliError::InvalidEnvName {
            name: name.to_string(),
            reason: format!("`{name}` is a reserved name"),
        });
    }

    Ok(())
}

/// List all environments under `envs/`.
#[allow(dead_code)] // Used by future `env list` command
pub(crate) fn list_envs(project_root: &Path) -> Result<Vec<EnvInfo>> {
    let envs_dir = project_root.join("envs");
    if !envs_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut envs = Vec::new();
    let subdirs = list_env_subdirs(&envs_dir)?;
    for name in subdirs {
        let dir = envs_dir.join(&name);
        let initialized = dir.join(".terraform").is_dir();
        envs.push(EnvInfo {
            name,
            dir,
            initialized,
        });
    }
    envs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(envs)
}

/// Clone a `StateConfig` and override the S3 key or GCS prefix with the env name.
pub(crate) fn resolve_env_state(
    base: &StateConfig,
    project_name: &str,
    env_name: &str,
) -> StateConfig {
    match base.clone() {
        StateConfig::S3 {
            bucket,
            dynamodb_table,
            region,
            encrypt,
            ..
        } => StateConfig::S3 {
            bucket,
            dynamodb_table,
            region,
            key: Some(format!("{project_name}/{env_name}/terraform.tfstate")),
            encrypt,
        },
        StateConfig::Gcs { bucket, region, .. } => StateConfig::Gcs {
            bucket,
            region,
            prefix: Some(format!("{project_name}/{env_name}")),
        },
    }
}

/// Build an `EnvContext` from an environment name and project root.
pub(crate) fn build_env_context(name: &str, project_root: &Path) -> Result<EnvContext> {
    let envs_dir = project_root.join("envs");
    let env_dir = envs_dir.join(name);

    if !env_dir.is_dir() {
        let available = list_env_subdirs(&envs_dir).unwrap_or_default().join(", ");
        return Err(CliError::EnvNotFound {
            name: name.to_string(),
            available,
        });
    }

    // Find .tfbackend file in the env directory.
    let tfbackend = find_env_tfbackend(&env_dir, name)?;

    // Check for optional tfvars.
    let tfvars_path = env_dir.join(format!("{name}.tfvars"));
    let tfvars = if tfvars_path.is_file() {
        Some(tfvars_path)
    } else {
        None
    };

    // Collect *.auto.tfvars files — Terraform only auto-loads these from its
    // cwd (the project root), so we must explicitly inject them when the env
    // directory differs from the project root.
    let mut auto_tfvars = collect_auto_tfvars(&env_dir)?;
    auto_tfvars.sort();

    let tf_data_dir = env_dir.join(".terraform");

    Ok(EnvContext {
        name: name.to_string(),
        dir: env_dir,
        tfbackend,
        tfvars,
        auto_tfvars,
        tf_data_dir,
    })
}

/// Collect `*.auto.tfvars` files from a directory.
fn collect_auto_tfvars(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = fs::read_dir(dir).map_err(|source| CliError::Io {
        source,
        path: dir.to_path_buf(),
    })?;

    let mut paths = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".auto.tfvars") {
                    paths.push(path);
                }
            }
        }
    }
    Ok(paths)
}

/// List subdirectory names inside `envs/`.
fn list_env_subdirs(envs_dir: &Path) -> Result<Vec<String>> {
    let entries = fs::read_dir(envs_dir).map_err(|source| CliError::Io {
        source,
        path: envs_dir.to_path_buf(),
    })?;

    let mut names = Vec::new();
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                // Skip hidden directories.
                if !name.starts_with('.') {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Find a single `.tfbackend` file in an env directory (absolute path).
/// Returns a descriptive error if zero or multiple files are found.
fn find_env_tfbackend(env_dir: &Path, env_name: &str) -> Result<PathBuf> {
    let entries = fs::read_dir(env_dir).map_err(|source| CliError::Io {
        source,
        path: env_dir.to_path_buf(),
    })?;
    let mut found: Option<PathBuf> = None;
    let mut count = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("tfbackend") && path.is_file() {
            count += 1;
            if found.is_none() {
                found = Some(path);
            }
        }
    }

    match count {
        0 => Err(CliError::EnvMissingTfbackend {
            name: env_name.to_string(),
        }),
        1 => Ok(found.unwrap()),
        _ => Err(CliError::ConfigValidation {
            field: "env".to_string(),
            message: format!(
                "environment \"{env_name}\" has {count} .tfbackend files in envs/{env_name}/. Expected exactly one."
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("evm-cloud-env-test")
            .join(name)
            .join(format!("{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn validate_env_name_valid() {
        assert!(validate_env_name("staging").is_ok());
        assert!(validate_env_name("prod").is_ok());
        assert!(validate_env_name("us-east-1").is_ok());
        assert!(validate_env_name("a").is_ok());
        assert!(validate_env_name("a1b2c3").is_ok());
    }

    #[test]
    fn validate_env_name_empty() {
        assert!(validate_env_name("").is_err());
    }

    #[test]
    fn validate_env_name_too_long() {
        let long = "a".repeat(33);
        assert!(validate_env_name(&long).is_err());
    }

    #[test]
    fn validate_env_name_invalid_chars() {
        assert!(validate_env_name("my_env").is_err());
        assert!(validate_env_name("my env").is_err());
        assert!(validate_env_name("my.env").is_err());
    }

    #[test]
    fn validate_env_name_leading_trailing_hyphen() {
        assert!(validate_env_name("-staging").is_err());
        assert!(validate_env_name("staging-").is_err());
    }

    #[test]
    fn validate_env_name_reserved() {
        assert!(validate_env_name("envs").is_err());
        assert!(validate_env_name("config").is_err());
        assert!(validate_env_name(".terraform").is_err());
        assert!(validate_env_name("modules").is_err());
    }

    #[test]
    fn resolve_env_no_envs_dir() {
        let root = test_dir("no-envs");
        let result = resolve_env(None, &root).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_env_empty_envs_dir() {
        let root = test_dir("empty-envs");
        std::fs::create_dir_all(root.join("envs")).unwrap();
        let result = resolve_env(None, &root).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_env_name_but_no_envs_dir() {
        let root = test_dir("name-no-envs");
        let result = resolve_env(Some("staging"), &root);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_env_envs_exist_but_no_name() {
        let root = test_dir("envs-no-name");
        let envs_dir = root.join("envs").join("staging");
        std::fs::create_dir_all(&envs_dir).unwrap();
        let result = resolve_env(None, &root);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_env_success() {
        let root = test_dir("resolve-ok");
        let env_dir = root.join("envs").join("staging");
        std::fs::create_dir_all(&env_dir).unwrap();
        std::fs::write(env_dir.join("staging.s3.tfbackend"), "bucket = \"b\"\n").unwrap();
        let ctx = resolve_env(Some("staging"), &root).unwrap().unwrap();
        assert_eq!(ctx.name, "staging");
        assert!(ctx.tfbackend.exists());
        assert!(ctx.tfvars.is_none());
    }

    #[test]
    fn resolve_env_with_tfvars() {
        let root = test_dir("resolve-tfvars");
        let env_dir = root.join("envs").join("prod");
        std::fs::create_dir_all(&env_dir).unwrap();
        std::fs::write(env_dir.join("prod.s3.tfbackend"), "bucket = \"b\"\n").unwrap();
        std::fs::write(env_dir.join("prod.tfvars"), "foo = \"bar\"\n").unwrap();
        let ctx = resolve_env(Some("prod"), &root).unwrap().unwrap();
        assert!(ctx.tfvars.is_some());
    }

    #[test]
    fn resolve_env_not_found() {
        let root = test_dir("resolve-not-found");
        let env_dir = root.join("envs").join("staging");
        std::fs::create_dir_all(&env_dir).unwrap();
        std::fs::write(env_dir.join("staging.s3.tfbackend"), "bucket = \"b\"\n").unwrap();
        let result = resolve_env(Some("prod"), &root);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_env_missing_tfbackend() {
        let root = test_dir("resolve-no-tfbackend");
        let env_dir = root.join("envs").join("staging");
        std::fs::create_dir_all(&env_dir).unwrap();
        let result = resolve_env(Some("staging"), &root);
        assert!(result.is_err());
    }

    #[test]
    fn list_envs_empty() {
        let root = test_dir("list-empty");
        let result = list_envs(&root).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn list_envs_multiple() {
        let root = test_dir("list-multiple");
        let envs_dir = root.join("envs");
        std::fs::create_dir_all(envs_dir.join("prod")).unwrap();
        std::fs::create_dir_all(envs_dir.join("staging")).unwrap();
        std::fs::create_dir_all(envs_dir.join("staging").join(".terraform")).unwrap();
        let result = list_envs(&root).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "prod");
        assert!(!result[0].initialized);
        assert_eq!(result[1].name, "staging");
        assert!(result[1].initialized);
    }

    #[test]
    fn resolve_env_state_s3() {
        let base = StateConfig::S3 {
            bucket: "my-bucket".to_string(),
            dynamodb_table: "my-table".to_string(),
            region: "us-east-1".to_string(),
            key: Some("myproject/terraform.tfstate".to_string()),
            encrypt: true,
        };
        let resolved = resolve_env_state(&base, "myproject", "staging");
        match resolved {
            StateConfig::S3 { key, .. } => {
                assert_eq!(key.unwrap(), "myproject/staging/terraform.tfstate");
            }
            _ => panic!("expected S3"),
        }
    }

    #[test]
    fn resolve_env_state_gcs() {
        let base = StateConfig::Gcs {
            bucket: "my-bucket".to_string(),
            region: "us-central1".to_string(),
            prefix: Some("myproject".to_string()),
        };
        let resolved = resolve_env_state(&base, "myproject", "prod");
        match resolved {
            StateConfig::Gcs { prefix, .. } => {
                assert_eq!(prefix.unwrap(), "myproject/prod");
            }
            _ => panic!("expected Gcs"),
        }
    }
}
