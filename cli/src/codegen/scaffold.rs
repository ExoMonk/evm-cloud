use std::fs;
use std::path::Path;

use crate::codegen::manifest::{GenerationMode, ResolvedConfig};
use crate::codegen::write_atomic;
use crate::config::schema::{EvmCloudConfig, StateConfig};
use crate::error::{CliError, Result};

const GENERATED_DIR: &str = ".evm-cloud";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScaffoldResult {
    Written,
    BackendChanged,
    Unchanged,
}

pub(crate) fn generate_main_tf(config: &EvmCloudConfig, project_root: &Path) -> Result<ScaffoldResult> {
    let contents = render_main_tf(config);
    let path = project_root.join(GENERATED_DIR).join("main.tf");

    let old_contents = fs::read_to_string(&path).ok();
    if old_contents.as_deref() == Some(&contents) {
        return Ok(ScaffoldResult::Unchanged);
    }

    let result = if backend_changed(old_contents.as_deref(), &contents) {
        ScaffoldResult::BackendChanged
    } else {
        ScaffoldResult::Written
    };

    // On BackendChanged, do NOT write the new main.tf. The old file is preserved
    // so callers can inspect the previous backend config before deciding to migrate.
    // Use `commit_main_tf` to write after the caller has handled the transition.
    if result != ScaffoldResult::BackendChanged {
        write_atomic(&path, &contents)?;
    }

    Ok(result)
}

/// Write main.tf unconditionally, skipping change detection.
/// Used by `init` after warning about backend changes to commit the new config
/// before running `terraform init`.
pub(crate) fn commit_main_tf(config: &EvmCloudConfig, project_root: &Path) -> Result<()> {
    let contents = render_main_tf(config);
    let path = project_root.join(GENERATED_DIR).join("main.tf");
    write_atomic(&path, &contents)
}

fn render_main_tf(config: &EvmCloudConfig) -> String {
    let module_source = crate::module_source();
    let resolved = ResolvedConfig::from_evm_config(config);
    let module_body =
        super::manifest::render_module_args(&resolved, GenerationMode::Easy, &module_source);

    let v = crate::terraform::REQUIRED_VERSION_CONSTRAINT;
    let backend_block = config.state.as_ref().map(render_backend_hcl);

    let terraform_block = match backend_block {
        Some(ref backend) => format!(
            "terraform {{\n  required_version = \"{v}\"\n\n{backend}}}\n"
        ),
        None => format!(
            "terraform {{\n  required_version = \"{v}\"\n}}\n"
        ),
    };

    format!(
        "{terraform_block}\nmodule \"evm_cloud\" {{\n{module_body}\n}}\n"
    )
}

pub(crate) fn generate_variables_tf(config: &EvmCloudConfig, project_root: &Path) -> Result<()> {
    let resolved = ResolvedConfig::from_evm_config(config);
    let contents = super::manifest::render_variables_tf(&resolved, GenerationMode::Easy);

    let path = project_root.join(GENERATED_DIR).join("variables.tf");
    write_atomic(&path, &contents)
}

pub(crate) fn generate_outputs_tf(project_root: &Path) -> Result<()> {
    let contents = r#"output "workload_handoff" {
  value     = module.evm_cloud.workload_handoff
  sensitive = true
}
"#;

    let path = project_root.join(GENERATED_DIR).join("outputs.tf");
    write_atomic(&path, contents)
}

/// Backup `.evm-cloud/terraform.tfstate` before a backend transition.
/// Returns the backup path if a backup was created.
pub(crate) fn backup_state_file(project_root: &Path) -> Result<Option<std::path::PathBuf>> {
    let state_path = project_root.join(GENERATED_DIR).join("terraform.tfstate");
    if !state_path.exists() {
        return Ok(None);
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| CliError::SystemClock(err.to_string()))?
        .as_secs();

    let backup_path = project_root
        .join(GENERATED_DIR)
        .join(format!("terraform.tfstate.backup.{ts}"));

    fs::copy(&state_path, &backup_path).map_err(|source| CliError::Io {
        source,
        path: backup_path.clone(),
    })?;

    Ok(Some(backup_path))
}

fn render_backend_hcl(state: &StateConfig) -> String {
    match state {
        StateConfig::S3 {
            bucket,
            dynamodb_table,
            region,
            key,
            encrypt,
        } => {
            let key_str = key.as_deref().unwrap_or("terraform.tfstate");
            format!(
                "  backend \"s3\" {{\n\
                 \x20   bucket         = \"{bucket}\"\n\
                 \x20   key            = \"{key_str}\"\n\
                 \x20   region         = \"{region}\"\n\
                 \x20   dynamodb_table = \"{dynamodb_table}\"\n\
                 \x20   encrypt        = {encrypt}\n\
                 \x20 }}\n"
            )
        }
        // `region` is intentionally excluded — the Terraform GCS backend does not accept it.
        // It's stored in the schema only to drive `gcloud storage buckets create --location`.
        StateConfig::Gcs { bucket, prefix, .. } => {
            let prefix_str = prefix.as_deref().unwrap_or("");
            if prefix_str.is_empty() {
                format!(
                    "  backend \"gcs\" {{\n\
                     \x20   bucket = \"{bucket}\"\n\
                     \x20 }}\n"
                )
            } else {
                format!(
                    "  backend \"gcs\" {{\n\
                     \x20   bucket = \"{bucket}\"\n\
                     \x20   prefix = \"{prefix_str}\"\n\
                     \x20 }}\n"
                )
            }
        }
    }
}

/// Detect whether the backend configuration changed between old and new main.tf content.
/// Returns `false` on first run (no old file) — that's a fresh write, not a migration.
fn backend_changed(old: Option<&str>, new: &str) -> bool {
    let Some(old) = old else {
        return false;
    };
    extract_backend_block(old) != extract_backend_block(new)
}

/// Extract the backend block from main.tf content as a comparable string.
/// Returns `None` if no backend block is present (local state).
fn extract_backend_block(content: &str) -> Option<String> {
    let start = content.find("backend \"")?;
    // Find the matching closing brace for the backend block.
    let after_start = &content[start..];
    let open = after_start.find('{')?;
    let mut depth = 0;
    let mut end = None;
    for (i, ch) in after_start[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(open + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    Some(after_start[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_s3_backend_hcl() {
        let state = StateConfig::S3 {
            bucket: "my-bucket".to_string(),
            dynamodb_table: "my-lock".to_string(),
            region: "us-east-1".to_string(),
            key: Some("demo/terraform.tfstate".to_string()),
            encrypt: true,
        };
        let hcl = render_backend_hcl(&state);
        assert!(hcl.contains("backend \"s3\""));
        assert!(hcl.contains("bucket         = \"my-bucket\""));
        assert!(hcl.contains("key            = \"demo/terraform.tfstate\""));
        assert!(hcl.contains("region         = \"us-east-1\""));
        assert!(hcl.contains("dynamodb_table = \"my-lock\""));
        assert!(hcl.contains("encrypt        = true"));
    }

    #[test]
    fn renders_gcs_backend_hcl() {
        let state = StateConfig::Gcs {
            bucket: "my-bucket".to_string(),
            region: "us-central1".to_string(),
            prefix: Some("demo".to_string()),
        };
        let hcl = render_backend_hcl(&state);
        assert!(hcl.contains("backend \"gcs\""));
        assert!(hcl.contains("bucket = \"my-bucket\""));
        assert!(hcl.contains("prefix = \"demo\""));
    }

    #[test]
    fn renders_gcs_without_prefix() {
        let state = StateConfig::Gcs {
            bucket: "my-bucket".to_string(),
            region: "us-central1".to_string(),
            prefix: None,
        };
        let hcl = render_backend_hcl(&state);
        assert!(hcl.contains("backend \"gcs\""));
        assert!(hcl.contains("bucket = \"my-bucket\""));
        assert!(!hcl.contains("prefix"));
    }

    #[test]
    fn detects_backend_added() {
        let old = "terraform {\n  required_version = \">= 1.14.6\"\n}\n";
        let new = "terraform {\n  required_version = \">= 1.14.6\"\n\n  backend \"s3\" {\n    bucket = \"b\"\n  }\n}\n";
        assert!(backend_changed(Some(old), new));
    }

    #[test]
    fn detects_backend_removed() {
        let old = "terraform {\n  required_version = \">= 1.14.6\"\n\n  backend \"s3\" {\n    bucket = \"b\"\n  }\n}\n";
        let new = "terraform {\n  required_version = \">= 1.14.6\"\n}\n";
        assert!(backend_changed(Some(old), new));
    }

    #[test]
    fn detects_backend_bucket_change() {
        let old = "terraform {\n  backend \"s3\" {\n    bucket = \"old\"\n  }\n}\n";
        let new = "terraform {\n  backend \"s3\" {\n    bucket = \"new\"\n  }\n}\n";
        assert!(backend_changed(Some(old), new));
    }

    #[test]
    fn detects_backend_type_change() {
        let old = "terraform {\n  backend \"s3\" {\n    bucket = \"b\"\n  }\n}\n";
        let new = "terraform {\n  backend \"gcs\" {\n    bucket = \"b\"\n  }\n}\n";
        assert!(backend_changed(Some(old), new));
    }

    #[test]
    fn no_change_when_backends_identical() {
        let content = "terraform {\n  backend \"s3\" {\n    bucket = \"b\"\n  }\n}\n";
        assert!(!backend_changed(Some(content), content));
    }

    #[test]
    fn no_change_when_both_have_no_backend() {
        let content = "terraform {\n  required_version = \">= 1.14.6\"\n}\n";
        assert!(!backend_changed(Some(content), content));
    }

    #[test]
    fn first_run_no_old_file_is_not_backend_change() {
        let new = "terraform {\n  required_version = \">= 1.14.6\"\n}\n";
        assert!(!backend_changed(None, new));
    }

    #[test]
    fn first_run_with_backend_is_not_backend_change() {
        // First run (no old file) with a backend should be Written, not BackendChanged
        let new = "terraform {\n  backend \"s3\" {\n    bucket = \"b\"\n  }\n}\n";
        assert!(!backend_changed(None, new));
    }

    // ── Integration tests: full TOML → generate_main_tf ───────────────

    fn base_toml() -> &'static str {
        r#"
schema_version = 1

[project]
name = "test"
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

    fn parse_config(toml_str: &str) -> EvmCloudConfig {
        toml::from_str(toml_str).expect("must parse test TOML")
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join("evm-cloud-scaffold-test")
            .join(name)
            .join(format!("{}", std::process::id()));
        std::fs::create_dir_all(dir.join(GENERATED_DIR)).unwrap();
        dir
    }

    #[test]
    fn generates_main_tf_without_state() {
        let config = parse_config(base_toml());
        let dir = temp_dir("no-state");
        let result = generate_main_tf(&config, &dir).expect("must succeed");
        assert_eq!(result, ScaffoldResult::Written);

        let content = std::fs::read_to_string(dir.join(GENERATED_DIR).join("main.tf")).unwrap();
        assert!(content.contains("required_version"));
        assert!(content.contains("module \"evm_cloud\""));
        assert!(!content.contains("backend \""));
    }

    #[test]
    fn generates_main_tf_with_s3_state() {
        let toml_str = format!(
            "{}\n{}",
            base_toml(),
            r#"
[state]
backend = "s3"
bucket = "my-state"
dynamodb_table = "my-lock"
region = "us-east-1"
key = "test/terraform.tfstate"
"#
        );
        let config = parse_config(&toml_str);
        let dir = temp_dir("s3-state");
        let result = generate_main_tf(&config, &dir).expect("must succeed");
        assert_eq!(result, ScaffoldResult::Written);

        let content = std::fs::read_to_string(dir.join(GENERATED_DIR).join("main.tf")).unwrap();
        assert!(content.contains("backend \"s3\""));
        assert!(content.contains("bucket         = \"my-state\""));
        assert!(content.contains("dynamodb_table = \"my-lock\""));
        assert!(content.contains("key            = \"test/terraform.tfstate\""));
        assert!(content.contains("encrypt        = true"));
    }

    #[test]
    fn generates_main_tf_with_gcs_state() {
        let toml_str = format!(
            "{}\n{}",
            base_toml(),
            r#"
[state]
backend = "gcs"
bucket = "my-state"
region = "us-central1"
prefix = "test"
"#
        );
        let config = parse_config(&toml_str);
        let dir = temp_dir("gcs-state");
        let result = generate_main_tf(&config, &dir).expect("must succeed");
        assert_eq!(result, ScaffoldResult::Written);

        let content = std::fs::read_to_string(dir.join(GENERATED_DIR).join("main.tf")).unwrap();
        assert!(content.contains("backend \"gcs\""));
        assert!(content.contains("bucket = \"my-state\""));
        assert!(content.contains("prefix = \"test\""));
    }

    #[test]
    fn same_config_twice_returns_unchanged() {
        let config = parse_config(base_toml());
        let dir = temp_dir("unchanged");
        let r1 = generate_main_tf(&config, &dir).expect("first run");
        assert_eq!(r1, ScaffoldResult::Written);

        let r2 = generate_main_tf(&config, &dir).expect("second run");
        assert_eq!(r2, ScaffoldResult::Unchanged);
    }

    #[test]
    fn adding_backend_returns_backend_changed_and_preserves_old_file() {
        let config_no_state = parse_config(base_toml());
        let dir = temp_dir("add-backend");
        generate_main_tf(&config_no_state, &dir).expect("first run without state");

        let old_content = std::fs::read_to_string(dir.join(GENERATED_DIR).join("main.tf")).unwrap();
        assert!(!old_content.contains("backend \""));

        let toml_with_state = format!(
            "{}\n{}",
            base_toml(),
            r#"
[state]
backend = "s3"
bucket = "b"
dynamodb_table = "t"
region = "r"
"#
        );
        let config_with_state = parse_config(&toml_with_state);
        let r2 = generate_main_tf(&config_with_state, &dir).expect("second run with state");
        assert_eq!(r2, ScaffoldResult::BackendChanged);

        // Old main.tf is preserved (not overwritten) on BackendChanged.
        let after = std::fs::read_to_string(dir.join(GENERATED_DIR).join("main.tf")).unwrap();
        assert_eq!(old_content, after, "main.tf should be unchanged on BackendChanged");
    }

    #[test]
    fn commit_main_tf_writes_unconditionally() {
        let config_no_state = parse_config(base_toml());
        let dir = temp_dir("commit");
        generate_main_tf(&config_no_state, &dir).expect("first run");

        let toml_with_state = format!(
            "{}\n{}",
            base_toml(),
            r#"
[state]
backend = "s3"
bucket = "b"
dynamodb_table = "t"
region = "r"
"#
        );
        let config_with_state = parse_config(&toml_with_state);

        // generate_main_tf returns BackendChanged and does NOT write
        let r = generate_main_tf(&config_with_state, &dir).expect("detect change");
        assert_eq!(r, ScaffoldResult::BackendChanged);

        // commit_main_tf writes regardless
        commit_main_tf(&config_with_state, &dir).expect("commit");
        let content = std::fs::read_to_string(dir.join(GENERATED_DIR).join("main.tf")).unwrap();
        assert!(content.contains("backend \"s3\""));
    }

    #[test]
    fn changing_bucket_returns_backend_changed() {
        let toml1 = format!(
            "{}\n{}",
            base_toml(),
            r#"
[state]
backend = "s3"
bucket = "bucket-a"
dynamodb_table = "t"
region = "r"
"#
        );
        let toml2 = format!(
            "{}\n{}",
            base_toml(),
            r#"
[state]
backend = "s3"
bucket = "bucket-b"
dynamodb_table = "t"
region = "r"
"#
        );
        let dir = temp_dir("change-bucket");
        generate_main_tf(&parse_config(&toml1), &dir).expect("first run");

        let r2 = generate_main_tf(&parse_config(&toml2), &dir).expect("second run");
        assert_eq!(r2, ScaffoldResult::BackendChanged);
    }

    #[test]
    fn removing_backend_returns_backend_changed() {
        let toml_with = format!(
            "{}\n{}",
            base_toml(),
            r#"
[state]
backend = "s3"
bucket = "b"
dynamodb_table = "t"
region = "r"
"#
        );
        let dir = temp_dir("remove-backend");
        generate_main_tf(&parse_config(&toml_with), &dir).expect("first run with state");

        let config_without = parse_config(base_toml());
        let r2 = generate_main_tf(&config_without, &dir).expect("second run without state");
        assert_eq!(r2, ScaffoldResult::BackendChanged);
    }
}
