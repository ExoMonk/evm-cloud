use std::fs;
use std::path::Path;

use crate::config::schema::{BootstrapConfig, EvmCloudConfig, StateConfig};
use crate::config::validation;
use crate::error::{CliError, Result};

pub(crate) fn load(path: &Path) -> Result<EvmCloudConfig> {
    let raw = fs::read_to_string(path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })?;

    let mut config: EvmCloudConfig = toml::from_str(&raw).map_err(|err| CliError::ConfigParse {
        path: path.to_path_buf(),
        details: err.to_string(),
    })?;

    let version = config
        .schema_version
        .ok_or_else(|| CliError::ConfigValidation {
            field: "schema_version".to_string(),
            message: "missing required field; expected schema_version = 1".to_string(),
        })?;

    if version != 1 {
        return Err(CliError::UnsupportedSchemaVersion { found: version });
    }

    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    resolve_relative_paths(&mut config, base_dir);
    validation::validate(&config)?;
    Ok(config)
}

/// Load only the project name + state section for bootstrap.
///
/// Tries full `EvmCloudConfig` first (common case — user has a complete TOML).
/// Falls back to minimal `BootstrapConfig` (just `schema_version` + `[project]` + `[state]`).
///
/// IMPORTANT: Full TOML fails `BootstrapConfig` parse via `deny_unknown_fields`,
/// so try-full-first is the correct ordering. If you add fields to
/// `BootstrapConfig`, verify this invariant holds.
///
/// Skips `resolve_relative_paths` and `validation::validate` — bootstrap doesn't
/// need indexer paths, rpc endpoints, etc.
pub(crate) fn load_for_bootstrap(path: &Path) -> Result<(String, Option<StateConfig>)> {
    let raw = fs::read_to_string(path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })?;

    // Try full config first (covers existing complete TOMLs)
    let full_err = match toml::from_str::<EvmCloudConfig>(&raw) {
        Ok(full) => {
            let version = full
                .schema_version
                .ok_or_else(|| CliError::ConfigValidation {
                    field: "schema_version".to_string(),
                    message: "missing required field; expected schema_version = 1".to_string(),
                })?;
            if version != 1 {
                return Err(CliError::UnsupportedSchemaVersion { found: version });
            }
            return Ok((full.project.name, full.state));
        }
        Err(e) => e,
    };

    // Fall back to minimal bootstrap config.
    // When both fail, surface the most informative error: use the full-parse error
    // if the TOML looks like it was intended to be a complete config (has full-config
    // sections), otherwise use the minimal-parse error.
    let config: BootstrapConfig =
        toml::from_str(&raw).map_err(|minimal_err| {
            let looks_like_full = raw.contains("[compute]") || raw.contains("[database]") || raw.contains("[indexer]");
            CliError::ConfigParse {
                path: path.to_path_buf(),
                details: if looks_like_full {
                    full_err.to_string()
                } else {
                    minimal_err.to_string()
                },
            }
        })?;

    let version = config
        .schema_version
        .ok_or_else(|| CliError::ConfigValidation {
            field: "schema_version".to_string(),
            message: "missing required field; expected schema_version = 1".to_string(),
        })?;

    if version != 1 {
        return Err(CliError::UnsupportedSchemaVersion { found: version });
    }

    Ok((config.project.name, config.state))
}

fn resolve_relative_paths(config: &mut EvmCloudConfig, base_dir: &Path) {
    if config.indexer.config_path.is_relative() {
        config.indexer.config_path = base_dir.join(&config.indexer.config_path);
    }

    if let Some(path) = &config.indexer.erpc_config_path {
        if path.is_relative() {
            config.indexer.erpc_config_path = Some(base_dir.join(path));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{load, load_for_bootstrap};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "evm-cloud-cli-tests-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    fn write(path: &std::path::Path, content: &str) {
        fs::write(path, content).expect("write file")
    }

    #[test]
    fn rejects_unknown_nested_fields() {
        let dir = temp_dir("unknown-nested");
        write(&dir.join("rindexer.yaml"), "networks: []");

        write(
            &dir.join("evm-cloud.toml"),
            r#"
schema_version = 1

[project]
name = "demo"
region = "us-east-1"
unexpected = "nope"

[compute]
engine = "ec2"
instance_type = "t3.small"

[database]
mode = "managed"
provider = "aws"

[indexer]
config_path = "rindexer.yaml"
chains = ["polygon"]

[rpc]
endpoints = { polygon = "https://rpc.example" }

[ingress]
mode = "none"

[secrets]
mode = "provider"
"#,
        );

        let err = load(&dir.join("evm-cloud.toml")).expect_err("must fail");
        let rendered = err.to_string();
        assert!(rendered.contains("unknown field"));
    }

    #[test]
    fn rejects_non_one_or_missing_schema_version() {
        let dir = temp_dir("schema-version");
        write(&dir.join("rindexer.yaml"), "networks: []");

        write(
            &dir.join("bad.toml"),
            r#"
schema_version = 2

[project]
name = "demo"
region = "us-east-1"

[compute]
engine = "ec2"
instance_type = "t3.small"

[database]
mode = "managed"
provider = "aws"

[indexer]
config_path = "rindexer.yaml"
chains = ["polygon"]

[rpc]
endpoints = { polygon = "https://rpc.example" }

[ingress]
mode = "none"

[secrets]
mode = "provider"
"#,
        );

        let err = load(&dir.join("bad.toml")).expect_err("must fail");
        assert!(err.to_string().contains("unsupported schema_version"));

        write(
            &dir.join("missing.toml"),
            r#"
[project]
name = "demo"
region = "us-east-1"

[compute]
engine = "ec2"
instance_type = "t3.small"

[database]
mode = "managed"
provider = "aws"

[indexer]
config_path = "rindexer.yaml"
chains = ["polygon"]

[rpc]
endpoints = { polygon = "https://rpc.example" }

[ingress]
mode = "none"

[secrets]
mode = "provider"
"#,
        );

        let err = load(&dir.join("missing.toml")).expect_err("must fail");
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn resolves_relative_paths_against_config_dir() {
        let dir = temp_dir("relative-paths");
        let config_dir = dir.join("config");
        fs::create_dir_all(config_dir.join("sub")).expect("create nested dir");
        write(&config_dir.join("sub/rindexer.yaml"), "networks: []");
        write(&config_dir.join("sub/erpc.yaml"), "projects: []");

        write(
            &config_dir.join("evm-cloud.toml"),
            r#"
schema_version = 1

[project]
name = "demo"
region = "us-east-1"

[compute]
engine = "ec2"
instance_type = "t3.small"

[database]
mode = "managed"
provider = "aws"

[indexer]
config_path = "sub/rindexer.yaml"
erpc_config_path = "sub/erpc.yaml"
chains = ["polygon"]

[rpc]
endpoints = { polygon = "https://rpc.example" }

[ingress]
mode = "none"

[secrets]
mode = "provider"
"#,
        );

        let config = load(&config_dir.join("evm-cloud.toml")).expect("must load");
        assert!(config.indexer.config_path.is_absolute());
        assert!(config
            .indexer
            .erpc_config_path
            .expect("erpc path exists")
            .is_absolute());
    }

    #[test]
    fn load_for_bootstrap_with_full_toml() {
        let dir = temp_dir("bootstrap-full");
        write(&dir.join("rindexer.yaml"), "networks: []");
        write(
            &dir.join("evm-cloud.toml"),
            r#"
schema_version = 1

[project]
name = "demo"
region = "us-east-1"

[compute]
engine = "ec2"

[database]
mode = "managed"
provider = "aws"

[indexer]
config_path = "rindexer.yaml"
chains = ["polygon"]

[rpc]
endpoints = { polygon = "https://rpc.example" }

[ingress]
mode = "none"

[secrets]
mode = "provider"

[state]
backend = "s3"
bucket = "my-bucket"
dynamodb_table = "my-locks"
region = "us-east-1"
"#,
        );

        let (name, state) =
            load_for_bootstrap(&dir.join("evm-cloud.toml")).expect("must load full TOML");
        assert_eq!(name, "demo");
        assert!(state.is_some());
    }

    #[test]
    fn load_for_bootstrap_with_minimal_toml() {
        let dir = temp_dir("bootstrap-minimal");
        write(
            &dir.join("evm-cloud.toml"),
            r#"
schema_version = 1

[project]
name = "my-project"

[state]
backend = "s3"
bucket = "my-bucket"
dynamodb_table = "my-locks"
region = "eu-west-1"
"#,
        );

        let (name, state) =
            load_for_bootstrap(&dir.join("evm-cloud.toml")).expect("must load minimal TOML");
        assert_eq!(name, "my-project");
        assert!(state.is_some());

        // Full loader should reject the same minimal TOML
        let err = load(&dir.join("evm-cloud.toml"));
        assert!(err.is_err(), "full loader must reject minimal TOML");
    }

    #[test]
    fn load_for_bootstrap_without_state() {
        let dir = temp_dir("bootstrap-no-state");
        write(
            &dir.join("evm-cloud.toml"),
            r#"
schema_version = 1

[project]
name = "no-state-project"
"#,
        );

        let (name, state) =
            load_for_bootstrap(&dir.join("evm-cloud.toml")).expect("must load");
        assert_eq!(name, "no-state-project");
        assert!(state.is_none());
    }

    #[test]
    fn load_for_bootstrap_rejects_bad_schema_version() {
        let dir = temp_dir("bootstrap-bad-version");
        write(
            &dir.join("evm-cloud.toml"),
            r#"
schema_version = 99

[project]
name = "test"

[state]
backend = "s3"
bucket = "b"
dynamodb_table = "t"
region = "us-east-1"
"#,
        );

        let err = load_for_bootstrap(&dir.join("evm-cloud.toml")).expect_err("must fail");
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn load_for_bootstrap_gcs() {
        let dir = temp_dir("bootstrap-gcs");
        write(
            &dir.join("evm-cloud.toml"),
            r#"
schema_version = 1

[project]
name = "gcs-project"

[state]
backend = "gcs"
bucket = "my-gcs-bucket"
region = "US"
"#,
        );

        let (name, state) =
            load_for_bootstrap(&dir.join("evm-cloud.toml")).expect("must load");
        assert_eq!(name, "gcs-project");
        assert!(state.is_some());
    }
}
