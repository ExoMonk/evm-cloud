use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::Args;

use crate::codegen;
use crate::config::loader;
use crate::config::schema::StateConfig;
use crate::error::{CliError, Result};
use crate::init_templates;
use crate::init_wizard;
use crate::output::{self, ColorMode};

#[derive(Args)]
pub(crate) struct BootstrapArgs {
    /// Project directory containing evm-cloud.toml
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,
    /// Print commands without executing (read-only checks still run)
    #[arg(long)]
    dry_run: bool,
    /// Project name (overrides TOML value)
    #[arg(long)]
    name: Option<String>,
    /// State backend: "s3" or "gcs"
    #[arg(long)]
    backend: Option<String>,
    /// S3/GCS bucket name
    #[arg(long)]
    bucket: Option<String>,
    /// AWS region / GCS region
    #[arg(long)]
    region: Option<String>,
    /// DynamoDB lock table (S3 backend only)
    #[arg(long)]
    dynamodb_table: Option<String>,
}

pub(crate) fn run(args: BootstrapArgs, color: ColorMode) -> Result<()> {
    let config_path = args.dir.join("evm-cloud.toml");
    let has_cli_flags = args.backend.is_some()
        || args.bucket.is_some()
        || args.region.is_some()
        || args.dynamodb_table.is_some();

    let (toml_name, toml_state) = if config_path.exists() {
        loader::load_for_bootstrap(&config_path)?
    } else if !has_cli_flags && args.name.is_none() {
        return Err(CliError::ConfigValidation {
            field: "state".into(),
            message: "No evm-cloud.toml found. Run `evm-cloud init` first to create your project, \
                      or pass CLI flags: --name <project> --backend s3 --bucket <bucket> --region <region> --dynamodb-table <table>".into(),
        });
    } else {
        (String::new(), None)
    };

    let toml_name_opt = if toml_name.is_empty() {
        None
    } else {
        Some(toml_name)
    };
    let project_name = args
        .name
        .clone()
        .or(toml_name_opt)
        .ok_or_else(|| CliError::ConfigValidation {
            field: "name".into(),
            message: "No project name: pass --name or provide [project].name in evm-cloud.toml"
                .into(),
        })?;

    let mut state = build_state_from_flags(&args, toml_state, &project_name, &config_path, color)?;

    // Generate .tfbackend in the terraform working directory.
    // Easy mode: .evm-cloud/ is the terraform dir. Power mode: project root is.
    let mut resolved = state.clone();
    resolved.resolve_defaults(&project_name);
    let filename = resolved.tfbackend_filename(&project_name);
    let content = resolved.render_tfbackend();

    let evm_cloud_dir = args.dir.join(".evm-cloud");
    if evm_cloud_dir.is_dir() {
        // Easy mode — write only into .evm-cloud/ (the terraform working dir).
        codegen::write_atomic(&evm_cloud_dir.join(&filename), &content)?;
    } else {
        // Power mode — write at project root (which IS the terraform dir).
        codegen::write_atomic(&args.dir.join(&filename), &content)?;
    }
    output::checkline(&format!("Generated {filename}"), color);

    run_core(&mut state, &project_name, args.dry_run, color)
}

/// Called from `evm-cloud init` when auto-bootstrap is requested.
/// Always runs live (not dry-runnable) — the user explicitly confirmed resource creation.
pub(crate) fn run_inline(dir: &Path, color: ColorMode) -> Result<()> {
    let config_path = dir.join("evm-cloud.toml");
    let config = loader::load(&config_path)?;

    let mut state = config.state.ok_or_else(|| CliError::ConfigValidation {
        field: "state".into(),
        message: "No [state] section found in evm-cloud.toml.".into(),
    })?;

    run_core(&mut state, &config.project.name, false, color)
}

fn build_state_from_flags(
    args: &BootstrapArgs,
    mut toml_state: Option<StateConfig>,
    project_name: &str,
    config_path: &Path,
    color: ColorMode,
) -> Result<StateConfig> {
    let has_flags = args.backend.is_some()
        || args.bucket.is_some()
        || args.region.is_some()
        || args.dynamodb_table.is_some();

    if !has_flags {
        if let Some(state) = toml_state {
            return Ok(state);
        }

        // No TOML state and no CLI flags — offer the interactive wizard.
        if std::io::stdin().is_terminal() {
            output::info("No [state] configured. Starting state setup wizard...", color);
            let theme = dialoguer::theme::ColorfulTheme::default();
            // Best-effort region hint: try to extract project.region from TOML.
            let region = extract_project_region(config_path);
            let (wizard_state, _auto_bootstrap) =
                init_wizard::collect_state_answers(&theme, project_name, &region)?;

            match wizard_state {
                Some(state) => {
                    // Append [state] to existing TOML.
                    if config_path.exists() {
                        append_state_to_toml(config_path, &state)?;
                        output::checkline("Wrote [state] to evm-cloud.toml", color);
                    }
                    toml_state = Some(state);
                }
                None => {
                    return Err(CliError::ConfigValidation {
                        field: "state".into(),
                        message: "State configuration is required for bootstrap.".into(),
                    });
                }
            }
        } else {
            return Err(CliError::ConfigValidation {
                field: "state".into(),
                message: "No [state] in evm-cloud.toml and no CLI flags. Use --backend --bucket --region or run interactively.".into(),
            });
        }

        return toml_state.ok_or_else(|| CliError::ConfigValidation {
            field: "state".into(),
            message: "State configuration is required for bootstrap.".into(),
        });
    }

    // CLI flags are set — build state from flags, falling back to TOML for missing values
    let toml_backend = toml_state.as_ref().map(|s| match s {
        StateConfig::S3 { .. } => "s3",
        StateConfig::Gcs { .. } => "gcs",
    });

    let backend = args
        .backend
        .as_deref()
        .or(toml_backend)
        .ok_or_else(|| CliError::ConfigValidation {
            field: "backend".into(),
            message: "--backend is required (\"s3\" or \"gcs\")".into(),
        })?;

    // Warn when CLI flags diverge from TOML values
    if let Some(ref ts) = toml_state {
        // Warn if backend type itself changed
        if let Some(ref flag_backend) = args.backend {
            let toml_backend = match ts {
                StateConfig::S3 { .. } => "s3",
                StateConfig::Gcs { .. } => "gcs",
            };
            if flag_backend != toml_backend {
                output::warn(
                    &format!(
                        "Backend type changed from '{toml_backend}' (TOML) to '{flag_backend}' (CLI flag)"
                    ),
                    color,
                );
            }
        }
        warn_divergence(args, ts, color);
    }

    match backend {
        "s3" => {
            let (toml_bucket, toml_table, toml_region, toml_key) = match &toml_state {
                Some(StateConfig::S3 {
                    bucket,
                    dynamodb_table,
                    region,
                    key,
                    ..
                }) => (Some(bucket.clone()), Some(dynamodb_table.clone()), Some(region.clone()), key.clone()),
                _ => (None, None, None, None),
            };

            let bucket = args.bucket.clone().or(toml_bucket).ok_or_else(|| {
                CliError::ConfigValidation {
                    field: "bucket".into(),
                    message: "--bucket is required for S3 backend".into(),
                }
            })?;
            let region = args.region.clone().or(toml_region).ok_or_else(|| {
                CliError::ConfigValidation {
                    field: "region".into(),
                    message: "--region is required for S3 backend".into(),
                }
            })?;
            let dynamodb_table =
                args.dynamodb_table
                    .clone()
                    .or(toml_table)
                    .ok_or_else(|| CliError::ConfigValidation {
                        field: "dynamodb_table".into(),
                        message: "--dynamodb-table is required for S3 backend".into(),
                    })?;

            Ok(StateConfig::S3 {
                bucket,
                dynamodb_table,
                region,
                key: toml_key,
                encrypt: true,
            })
        }
        "gcs" => {
            let (toml_bucket, toml_region, toml_prefix) = match &toml_state {
                Some(StateConfig::Gcs { bucket, region, prefix, .. }) => {
                    (Some(bucket.clone()), Some(region.clone()), prefix.clone())
                }
                _ => (None, None, None),
            };

            let bucket = args.bucket.clone().or(toml_bucket).ok_or_else(|| {
                CliError::ConfigValidation {
                    field: "bucket".into(),
                    message: "--bucket is required for GCS backend".into(),
                }
            })?;
            let region = args.region.clone().or(toml_region).ok_or_else(|| {
                CliError::ConfigValidation {
                    field: "region".into(),
                    message: "--region is required for GCS backend".into(),
                }
            })?;

            Ok(StateConfig::Gcs {
                bucket,
                region,
                prefix: toml_prefix,
            })
        }
        other => Err(CliError::ConfigValidation {
            field: "backend".into(),
            message: format!("unsupported backend \"{other}\". Use \"s3\" or \"gcs\"."),
        }),
    }
}

fn warn_divergence(args: &BootstrapArgs, toml_state: &StateConfig, color: ColorMode) {
    let check = |flag_name: &str, flag_val: &Option<String>, toml_val: &str| {
        if let Some(ref fv) = flag_val {
            if fv != toml_val {
                output::warn(
                    &format!(
                        "Using --{flag_name} from CLI flag; your evm-cloud.toml [state] block differs and was not updated."
                    ),
                    color,
                );
            }
        }
    };

    match toml_state {
        StateConfig::S3 {
            bucket,
            dynamodb_table,
            region,
            ..
        } => {
            check("bucket", &args.bucket, bucket);
            check("region", &args.region, region);
            check("dynamodb-table", &args.dynamodb_table, dynamodb_table);
        }
        StateConfig::Gcs { bucket, region, .. } => {
            check("bucket", &args.bucket, bucket);
            check("region", &args.region, region);
        }
    }
}

/// Best-effort extraction of `project.region` from TOML for wizard defaults.
fn extract_project_region(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let table: toml::Table = toml::from_str(&raw).ok()?;
    table
        .get("project")?
        .as_table()?
        .get("region")?
        .as_str()
        .map(String::from)
}

/// Append a `[state]` section to an existing evm-cloud.toml.
/// PRECONDITION: caller verified `toml_state.is_none()` — the TOML has no `[state]` yet.
fn append_state_to_toml(path: &Path, state: &StateConfig) -> Result<()> {
    let existing = fs::read_to_string(path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })?;
    let section = init_templates::render_state_section(&Some(state.clone()));
    let updated = format!("{existing}{section}");
    codegen::write_atomic(path, &updated)
}

fn run_core(
    state: &mut StateConfig,
    project_name: &str,
    dry_run: bool,
    color: ColorMode,
) -> Result<()> {
    state.resolve_defaults(project_name);

    match state.clone() {
        StateConfig::S3 { bucket, dynamodb_table, region, .. } => {
            check_tool("aws")?;
            output::headline(
                &format!("🏰 ⚒️  Bootstrapping S3 state backend ({region})"),
                color,
            );
            if dry_run {
                output::subline("🏖️  Dry run — mutations will be printed only", color);
            }
            bootstrap_s3(&bucket, &dynamodb_table, &region, dry_run, color)?;
        }
        StateConfig::Gcs { bucket, region, .. } => {
            check_tool("gcloud")?;
            output::headline(
                &format!("🏰 ⚒️  Bootstrapping GCS state backend ({region})"),
                color,
            );
            if dry_run {
                output::subline("🏖️  Dry run — mutations will be printed only", color);
            }
            bootstrap_gcs(&bucket, &region, dry_run, color)?;
        }
    }

    if dry_run {
        output::subline("🏖️  No resources were created", color);
    }
    output::success("State backend ready.", color);

    Ok(())
}

fn check_tool(name: &str) -> Result<()> {
    if which::which(name).is_err() {
        return Err(CliError::PrerequisiteNotFound {
            tool: name.to_string(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// S3 bootstrap
// ---------------------------------------------------------------------------

fn bootstrap_s3(
    bucket: &str,
    dynamodb_table: &str,
    region: &str,
    dry_run: bool,
    color: ColorMode,
) -> Result<()> {
    // -- S3 bucket --
    // Check exists (no --region: S3 names are globally unique, --region causes 301 redirects)
    let bucket_created = match resource_exists("aws", &["s3api", "head-bucket", "--bucket", bucket])? {
        ResourceStatus::Exists(_) => {
            output::checkline(&format!("S3 bucket '{bucket}' exists"), color);
            false
        }
        ResourceStatus::NotFound => {
            let mut create_args = vec![
                "s3api", "create-bucket", "--bucket", bucket, "--region", region,
            ];
            let constraint = format!("LocationConstraint={region}");
            if region != "us-east-1" {
                create_args.push("--create-bucket-configuration");
                create_args.push(&constraint);
            }
            run_cmd("aws", &create_args, dry_run, color)?;
            true
        }
    };

    // Harden bucket (always, idempotent)
    run_cmd_lenient(
        "aws",
        &[
            "s3api", "put-public-access-block",
            "--bucket", bucket,
            "--region", region,
            "--public-access-block-configuration",
            "BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=true,RestrictPublicBuckets=true",
        ],
        dry_run,
        color,
    )?;
    run_cmd(
        "aws",
        &[
            "s3api", "put-bucket-versioning",
            "--bucket", bucket,
            "--region", region,
            "--versioning-configuration", "Status=Enabled",
        ],
        dry_run,
        color,
    )?;
    run_cmd(
        "aws",
        &[
            "s3api", "put-bucket-encryption",
            "--bucket", bucket,
            "--region", region,
            "--server-side-encryption-configuration",
            r#"{"Rules":[{"ApplyServerSideEncryptionByDefault":{"SSEAlgorithm":"AES256"}}]}"#,
        ],
        dry_run,
        color,
    )?;

    if bucket_created {
        output::checkline(
            &format!("S3 bucket '{bucket}' created (versioning, encryption, public-access-block)"),
            color,
        );
    }

    // -- DynamoDB lock table --
    let table_needs_wait = match resource_exists(
        "aws",
        &["dynamodb", "describe-table", "--table-name", dynamodb_table, "--region", region],
    )? {
        ResourceStatus::Exists(stdout) => {
            output::checkline(&format!("DynamoDB table '{dynamodb_table}' exists"), color);
            !stdout.contains("\"ACTIVE\"")
        }
        ResourceStatus::NotFound => {
            run_cmd(
                "aws",
                &[
                    "dynamodb", "create-table",
                    "--table-name", dynamodb_table,
                    "--attribute-definitions", "AttributeName=LockID,AttributeType=S",
                    "--key-schema", "AttributeName=LockID,KeyType=HASH",
                    "--billing-mode", "PAY_PER_REQUEST",
                    "--region", region,
                ],
                dry_run,
                color,
            )?;
            !dry_run
        }
    };

    if table_needs_wait {
        output::with_spinner("Waiting for DynamoDB table", color, || {
            run_cmd(
                "aws",
                &["dynamodb", "wait", "table-exists", "--table-name", dynamodb_table, "--region", region],
                false, // never dry-run the wait — we only get here when !dry_run
                color,
            )
        })?;
        output::checkline(&format!("DynamoDB table '{dynamodb_table}' created"), color);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// GCS bootstrap
// ---------------------------------------------------------------------------

fn bootstrap_gcs(
    bucket: &str,
    region: &str,
    dry_run: bool,
    color: ColorMode,
) -> Result<()> {
    let uri = format!("gs://{bucket}");

    let bucket_created = match resource_exists("gcloud", &["storage", "buckets", "describe", &uri, "--format=json"])? {
        ResourceStatus::Exists(_) => {
            output::checkline(&format!("GCS bucket '{bucket}' exists"), color);
            false
        }
        ResourceStatus::NotFound => {
            run_cmd(
                "gcloud",
                &[
                    "storage", "buckets", "create", &uri,
                    &format!("--location={region}"),
                    "--uniform-bucket-level-access",
                    "--public-access-prevention=enforced",
                ],
                dry_run,
                color,
            )?;
            true
        }
    };

    // Versioning (always, idempotent — lenient for org policies that enforce it externally)
    run_cmd_lenient(
        "gcloud",
        &["storage", "buckets", "update", &uri, "--versioning"],
        dry_run,
        color,
    )?;

    if bucket_created {
        output::checkline(&format!("GCS bucket '{bucket}' created (versioning, public-access-prevention)"), color);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Command execution helpers
// ---------------------------------------------------------------------------

enum ResourceStatus {
    Exists(String),
    NotFound,
}

/// Check if a cloud resource exists. Returns Exists(stdout) on success,
/// NotFound on 404/ResourceNotFound, or Err on permission denied / other.
///
/// Stderr content is checked BEFORE exit codes to avoid misclassification
/// (e.g. AWS CLI uses exit code 254 for both 403 and 404).
fn resource_exists(tool: &str, args: &[&str]) -> Result<ResourceStatus> {
    let output = Command::new(tool)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| CliError::ToolFailed {
            tool: tool.into(),
            details: e.to_string(),
        })?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        return Ok(ResourceStatus::Exists(stdout));
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Check stderr content FIRST — exit codes are ambiguous across AWS/GCP CLIs.
    // AWS CLI uses exit code 254 for both 403 and 404 responses.

    // Known "not found" patterns (check before forbidden — more specific)
    if stderr.contains("404")
        || stderr.contains("Not Found")
        || stderr.contains("not found")
        || stderr.contains("ResourceNotFoundException")
        || stderr.contains("does not exist")
        || stderr.contains("NoSuchBucket")
    {
        return Ok(ResourceStatus::NotFound);
    }

    // Permission denied / bucket owned by another account
    if stderr.contains("403")
        || stderr.contains("Forbidden")
        || stderr.contains("AccessDenied")
        || stderr.contains("Access Denied")
        || stderr.contains("PERMISSION_DENIED")
    {
        return Err(CliError::ToolFailed {
            tool: tool.into(),
            details: format!(
                "resource name is taken by another account or you lack permissions. \
                 Choose a different name or check your credentials.\n  stderr: {}",
                stderr.trim()
            ),
        });
    }

    // Unknown error — propagate with stderr
    Err(CliError::ToolFailed {
        tool: tool.into(),
        details: stderr.trim().to_string(),
    })
}

/// Run a CLI command. In dry-run mode, prints the command without executing.
fn run_cmd(tool: &str, args: &[&str], dry_run: bool, color: ColorMode) -> Result<()> {
    if dry_run {
        let cmd_str = format!("{} {}", tool, args.join(" "));
        output::info(&format!("     [dry-run] would run: {cmd_str}"), color);
        return Ok(());
    }

    let output = Command::new(tool)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| CliError::ToolFailed {
            tool: tool.into(),
            details: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::ToolFailed {
            tool: tool.into(),
            details: stderr.trim().to_string(),
        });
    }

    Ok(())
}

/// Like run_cmd, but treats permission errors as a non-fatal warning.
/// Used for idempotent settings that may be enforced by org policies (SCPs, GCP constraints).
fn run_cmd_lenient(tool: &str, args: &[&str], dry_run: bool, color: ColorMode) -> Result<()> {
    if dry_run {
        let cmd_str = format!("{} {}", tool, args.join(" "));
        output::info(&format!("     [dry-run] would run: {cmd_str}"), color);
        return Ok(());
    }

    let output = Command::new(tool)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| CliError::ToolFailed {
            tool: tool.into(),
            details: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("AccessDenied")
            || stderr.contains("Access Denied")
            || stderr.contains("OperationNotPermitted")
        {
            output::warn(
                "Setting may already be enforced by organization policy — skipping",
                color,
            );
            return Ok(());
        }
        return Err(CliError::ToolFailed {
            tool: tool.into(),
            details: stderr.trim().to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn build_s3_create_bucket_args_us_east_1_no_constraint() {
        let region = "us-east-1";
        let mut args = vec!["s3api", "create-bucket", "--bucket", "test-bucket", "--region", region];
        let constraint = format!("LocationConstraint={region}");
        if region != "us-east-1" {
            args.push("--create-bucket-configuration");
            args.push(&constraint);
        }
        assert!(!args.contains(&"--create-bucket-configuration"));
    }

    #[test]
    fn build_s3_create_bucket_args_non_us_east_1_has_constraint() {
        let region = "eu-west-1";
        let mut args = vec!["s3api", "create-bucket", "--bucket", "test-bucket", "--region", region];
        let constraint = format!("LocationConstraint={region}");
        if region != "us-east-1" {
            args.push("--create-bucket-configuration");
            args.push(&constraint);
        }
        assert!(args.contains(&"--create-bucket-configuration"));
    }
}
