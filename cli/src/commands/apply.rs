use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use clap::Args;

use crate::commands::infra::{self, InfraPhaseOpts, InfraPhaseOutcome};
use crate::easy_mode;
use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};
use crate::post_deploy;
use crate::preflight::{self, ProjectKind};

pub(crate) fn ensure_non_interactive_terraform(args: &mut Vec<String>) {
    if args
        .iter()
        .any(|arg| arg == "-input=false" || arg == "-input=true")
    {
        return;
    }
    args.push("-input=false".to_string());
}

pub(crate) fn terraform_log_path(terraform_dir: &Path, op: &str) -> Result<PathBuf> {
    let logs_dir = if terraform_dir.file_name().and_then(|v| v.to_str()) == Some(".evm-cloud") {
        terraform_dir.join("logs")
    } else {
        terraform_dir.join(".evm-cloud").join("logs")
    };

    std::fs::create_dir_all(&logs_dir).map_err(|source| CliError::Io {
        source,
        path: logs_dir.clone(),
    })?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| CliError::SystemClock(err.to_string()))?
        .as_secs();

    Ok(logs_dir.join(format!("terraform-{op}-{ts}.log")))
}

pub(crate) fn terraform_output_path(terraform_dir: &Path) -> Result<PathBuf> {
    let logs_dir = if terraform_dir.file_name().and_then(|v| v.to_str()) == Some(".evm-cloud") {
        terraform_dir.join("logs")
    } else {
        terraform_dir.join(".evm-cloud").join("logs")
    };

    fs::create_dir_all(&logs_dir).map_err(|source| CliError::Io {
        source,
        path: logs_dir.clone(),
    })?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| CliError::SystemClock(err.to_string()))?
        .as_secs();

    Ok(logs_dir.join(format!("terraform-output-{ts}.json")))
}

#[derive(Args)]
pub(crate) struct ApplyArgs {
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    auto_approve: bool,
    #[arg(long)]
    allow_raw_terraform: bool,
    #[arg(long)]
    json: bool,
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    terraform_args: Vec<String>,
}

pub(crate) fn run(args: ApplyArgs, color: ColorMode) -> Result<()> {
    let started = Instant::now();
    if !args.json {
        output::headline(
            &format!("🏰 ⚒️ Preparing apply for {}", args.dir.display()),
            color,
        );
    }

    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    let terraform_dir = match preflight.project_kind {
        ProjectKind::EasyToml => {
            let (dir, scaffold) = easy_mode::prepare_workspace_quiet(&preflight.resolved_root)?;
            if scaffold == crate::codegen::ScaffoldResult::BackendChanged {
                return Err(easy_mode::handle_backend_changed(&preflight.resolved_root));
            }
            dir
        }
        ProjectKind::RawTerraform => preflight.resolved_root.clone(),
    };

    match infra::run_infra_phase(InfraPhaseOpts {
        terraform_dir: &terraform_dir,
        tf_args: &args.terraform_args,
        module_name: "evm_cloud",
        dry_run: args.dry_run,
        auto_approve: args.auto_approve,
        json: args.json,
        color,
    })? {
        InfraPhaseOutcome::DryRun {
            log_path,
            output_path,
        } => {
            if !args.json {
                output::checkline("Ran terraform plan", color);
                output::headline(
                    &format!(
                        "🏖️ ✅ Dry run complete - {}",
                        output::duration_human(started.elapsed())
                    ),
                    color,
                );
                eprintln!("      👉🏻 Logs: {}", log_path.display());
                eprintln!("      👉🏻 Output: {}", output_path.display());
            }
            return Ok(());
        }
        InfraPhaseOutcome::Cancelled => {
            output::warn("Apply cancelled", color);
            return Ok(());
        }
        InfraPhaseOutcome::Applied {
            handoff,
            log_path,
            output_path,
        } => {
            let handoff = *handoff;
            if args.json {
                if let Some(ref h) = handoff {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(h).map_err(CliError::OutputParseError)?
                    );
                } else {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "apply",
                            "handoff": null,
                            "note": "workload_handoff unavailable"
                        })
                    );
                }
                return Ok(());
            }

            if let Some(ref h) = handoff {
                infra::print_compute_summary(h, color);
            }

            output::headline(
                &format!(
                    "🏰 ✅ Infrastructure deployed - {}",
                    output::duration_human(started.elapsed())
                ),
                color,
            );

            if let Some(ref h) = handoff {
                post_deploy::print_summary(h, color);
            } else {
                output::warn(
                    "workload_handoff unavailable; skipping rich post-deploy summary",
                    color,
                );
            }

            eprintln!("      👉🏻 Logs: {}", log_path.display());
            eprintln!("      👉🏻 Output: {}", output_path.display());
        }
    }

    Ok(())
}
