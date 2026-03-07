use std::path::PathBuf;
use std::process::{Command, Stdio};

use clap::Args;

use crate::config::loader;
use crate::config::schema::StateConfig;
use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};

#[derive(Args)]
pub(crate) struct BootstrapArgs {
    /// Project directory containing evm-cloud.toml
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,
    /// Print commands without executing (read-only checks still run)
    #[arg(long)]
    dry_run: bool,
}

pub(crate) fn run(args: BootstrapArgs, color: ColorMode) -> Result<()> {
    let config_path = args.dir.join("evm-cloud.toml");
    let mut config = loader::load(&config_path)?;

    let state = config.state.as_mut().ok_or_else(|| CliError::ConfigValidation {
        field: "state".into(),
        message: "No [state] section found in evm-cloud.toml. Add a [state] section with backend = \"s3\" or backend = \"gcs\".".into(),
    })?;

    state.resolve_defaults(&config.project.name);

    let dry_run = args.dry_run;

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
    output::success("State backend ready. Run `evm-cloud init` next.", color);

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
