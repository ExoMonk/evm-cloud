use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::codegen::write_atomic;
use crate::config::schema::EvmCloudConfig;
use crate::error::{CliError, Result};

const GENERATED_DIR: &str = ".evm-cloud";
const GENERATED_TFVARS: &str = "terraform.auto.tfvars.json";

#[derive(Serialize)]
struct TerraformVars {
    project_name: String,
    aws_region: String,
    infrastructure_provider: String,
    database_mode: String,
    compute_engine: String,
    ec2_instance_type: String,
    ingress_mode: String,
    erpc_hostname: String,
    ingress_tls_email: String,
    secrets_mode: String,
    indexer_enabled: bool,
    rpc_proxy_enabled: bool,
    indexer_rpc_url: String,
    rindexer_config_yaml: String,
    erpc_config_yaml: String,
}

pub(crate) fn generate_tfvars(config: &EvmCloudConfig, project_root: &Path) -> Result<Value> {
    let rindexer_yaml = fs::read_to_string(&config.indexer.config_path).map_err(|source| CliError::Io {
        source,
        path: config.indexer.config_path.clone(),
    })?;

    let erpc_yaml = if let Some(path) = &config.indexer.erpc_config_path {
        fs::read_to_string(path).map_err(|source| CliError::Io {
            source,
            path: path.clone(),
        })?
    } else {
        String::new()
    };

    let vars = TerraformVars {
        project_name: config.project.name.clone(),
        aws_region: config.project.region.clone(),
        infrastructure_provider: config.database.provider.clone(),
        database_mode: config.database.mode.clone(),
        compute_engine: config.compute.engine.as_str().to_string(),
        ec2_instance_type: config.compute.instance_type.clone(),
        ingress_mode: config.ingress.mode.clone(),
        erpc_hostname: config.ingress.domain.clone().unwrap_or_default(),
        ingress_tls_email: config.ingress.tls_email.clone().unwrap_or_default(),
        secrets_mode: config.secrets.mode.clone(),
        indexer_enabled: true,
        rpc_proxy_enabled: !erpc_yaml.is_empty(),
        indexer_rpc_url: infer_indexer_rpc_url(config, !erpc_yaml.is_empty())?,
        rindexer_config_yaml: rindexer_yaml,
        erpc_config_yaml: erpc_yaml,
    };

    let json_value = serde_json::to_value(&vars).map_err(CliError::OutputParseError)?;
    let rendered = serde_json::to_string_pretty(&vars).map_err(CliError::OutputParseError)?;

    let generated_path = project_root.join(GENERATED_DIR).join(GENERATED_TFVARS);
    write_atomic(&generated_path, &format!("{rendered}\n"))?;

    ensure_gitignore_entry(project_root, &format!("{GENERATED_DIR}/{GENERATED_TFVARS}"))?;
    Ok(json_value)
}

pub(crate) fn ensure_gitignore_entry(project_root: &Path, entry: &str) -> Result<()> {
    let gitignore_path = project_root.join(".gitignore");

    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).map_err(|source| CliError::Io {
            source,
            path: gitignore_path.clone(),
        })?
    } else {
        String::new()
    };

    if existing.lines().any(|line| line.trim() == entry) {
        return Ok(());
    }

    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(entry);
    next.push('\n');

    write_atomic(&gitignore_path, &next)
}

fn infer_indexer_rpc_url(config: &EvmCloudConfig, rpc_proxy_enabled: bool) -> Result<String> {
    if rpc_proxy_enabled {
        return Ok("http://erpc:4000".to_string());
    }

    let endpoint = config
        .rpc
        .endpoints
        .values()
        .next()
        .ok_or_else(|| CliError::ConfigValidation {
            field: "rpc.endpoints".to_string(),
            message: "at least one RPC endpoint is required when eRPC config is not provided"
                .to_string(),
        })?;

    Ok(endpoint.clone())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::config::loader;

    use super::{ensure_gitignore_entry, generate_tfvars};

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

    fn write(path: &Path, content: &str) {
        fs::write(path, content).expect("write file")
    }

    fn sample_toml() -> &'static str {
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
config_path = "rindexer.yaml"
chains = ["polygon"]

[rpc]
endpoints = { polygon = "https://rpc.example" }

[ingress]
mode = "none"

[secrets]
mode = "provider"
"#
    }

    #[test]
    fn writes_deterministic_tfvars_and_expected_keys() {
        let dir = temp_dir("tfvars");
        write(&dir.join("rindexer.yaml"), "networks: []");
        write(&dir.join("evm-cloud.toml"), sample_toml());

        let config = loader::load(&dir.join("evm-cloud.toml")).expect("load config");
        let json = generate_tfvars(&config, &dir).expect("generate tfvars");

        let object = json.as_object().expect("json object");
        assert!(object.contains_key("project_name"));
        assert!(object.contains_key("aws_region"));
        assert!(object.contains_key("compute_engine"));
        assert!(object.contains_key("database_mode"));
        assert!(object.contains_key("infrastructure_provider"));
        assert!(object.contains_key("rindexer_config_yaml"));

        let first = fs::read_to_string(dir.join(".evm-cloud/terraform.auto.tfvars.json"))
            .expect("read first tfvars");
        let _ = generate_tfvars(&config, &dir).expect("generate tfvars second pass");
        let second = fs::read_to_string(dir.join(".evm-cloud/terraform.auto.tfvars.json"))
            .expect("read second tfvars");

        assert_eq!(first, second);
    }

    #[test]
    fn gitignore_updates_are_idempotent() {
        let dir = temp_dir("gitignore");
        ensure_gitignore_entry(&dir, ".evm-cloud/terraform.auto.tfvars.json")
            .expect("first update");
        ensure_gitignore_entry(&dir, ".evm-cloud/terraform.auto.tfvars.json")
            .expect("second update");

        let content = fs::read_to_string(dir.join(".gitignore")).expect("read gitignore");
        let count = content
            .lines()
            .filter(|line| line.trim() == ".evm-cloud/terraform.auto.tfvars.json")
            .count();
        assert_eq!(count, 1);
    }
}
