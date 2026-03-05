use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::fs;

use clap::Args;

use crate::easy_mode;
use crate::commands::tfvars;
use crate::error::{CliError, Result};
use crate::handoff;
use crate::output::{self, ColorMode};
use crate::post_deploy;
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

fn ensure_non_interactive_terraform(args: &mut Vec<String>) {
    if args.iter().any(|arg| arg == "-input=false" || arg == "-input=true") {
        return;
    }
    args.push("-input=false".to_string());
}

fn terraform_log_path(terraform_dir: &Path, op: &str) -> Result<PathBuf> {
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
        .map_err(|err| CliError::Message(format!("system clock error: {err}")))?
        .as_secs();

    Ok(logs_dir.join(format!("terraform-{op}-{ts}.log")))
}

fn terraform_output_path(terraform_dir: &Path) -> Result<PathBuf> {
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
        .map_err(|err| CliError::Message(format!("system clock error: {err}")))?
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
    let render_color = if args.json { ColorMode::Never } else { color };
    if !args.json {
        output::headline(
            &format!("🏰 ⚒️ Preparing apply for {}", args.dir.display()),
            color,
        );
    }

    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    let terraform_dir = match preflight.project_kind {
        ProjectKind::EasyToml => easy_mode::prepare_workspace_quiet(&preflight.resolved_root)?,
        ProjectKind::RawTerraform => preflight.resolved_root.clone(),
    };

    if !std::io::stdin().is_terminal() && !args.auto_approve && !args.dry_run {
        return Err(CliError::Message(
            "non-interactive shell detected: re-run with --auto-approve".to_string(),
        ));
    }

    let runner = TerraformRunner::check_installed(&terraform_dir)?;

    let mut effective_args = args.terraform_args.clone();
    if let Some(auto_var_file) = tfvars::auto_var_file_arg(&terraform_dir, &effective_args)? {
        effective_args.push(auto_var_file);
    }
    let mut plan_args = effective_args.clone();
    ensure_non_interactive_terraform(&mut plan_args);

    runner.init_if_needed(&terraform_dir, &[])?;

    let log_path = terraform_log_path(&terraform_dir, if args.dry_run { "plan" } else { "apply" })?;
    let output_path = terraform_output_path(&terraform_dir)?;

    if args.dry_run {
        output::with_terraforming(render_color, || {
            runner.plan_with_log(&terraform_dir, &plan_args, &log_path)
        })?;
        fs::write(
            &output_path,
            "{\n  \"status\": \"dry-run\",\n  \"note\": \"no terraform outputs generated for plan\"\n}\n",
        )
        .map_err(|source| CliError::Io {
            source,
            path: output_path.clone(),
        })?;
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

    output::with_terraforming(render_color, || {
        runner.plan_with_log(&terraform_dir, &plan_args, &log_path)
    })?;
    if !args.json {
        output::checkline("Ran terraform plan", color);
    }

    if !args.auto_approve && !args.json {
        let confirmed = output::confirmline("Apply these changes?", color)
            .map_err(|err| CliError::Other(err.into()))?;

        if !confirmed {
            output::warn("Apply cancelled", color);
            return Ok(());
        }

    }

    let mut apply_args = effective_args.clone();
    ensure_non_interactive_terraform(&mut apply_args);
    output::with_terraforming(render_color, || {
        runner.apply_captured_with_log(&terraform_dir, true, &apply_args, &log_path)
    })?;

    let mut parsed_handoff = None;

    if let Ok(outputs) = runner.output_json(&terraform_dir) {
        parsed_handoff = handoff::parse_from_full_output(outputs.clone(), "evm_cloud").ok();
        let rendered = serde_json::to_string_pretty(&outputs).map_err(CliError::OutputParseError)?;
        fs::write(&output_path, format!("{rendered}\n")).map_err(|source| CliError::Io {
            source,
            path: output_path.clone(),
        })?;
    } else {
        fs::write(
            &output_path,
            "{\n  \"status\": \"apply\",\n  \"note\": \"terraform output unavailable\"\n}\n",
        )
        .map_err(|source| CliError::Io {
            source,
            path: output_path.clone(),
        })?;
    }

    if args.json {
        if let Some(handoff) = parsed_handoff {
            println!(
                "{}",
                serde_json::to_string_pretty(&handoff).map_err(CliError::OutputParseError)?
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

    output::checkline("Ran terraform apply", color);
    eprintln!("     ✓  VPC + networking");

    if let Some(handoff) = parsed_handoff.as_ref() {
        if handoff.compute_engine == "k3s" {
            let control_plane_nodes = handoff
                .runtime
                .k3s
                .as_ref()
                .and_then(|runtime| runtime.node_name.as_ref())
                .map(|name| !name.trim().is_empty())
                .unwrap_or(false) as usize;
            let worker_nodes = handoff
                .runtime
                .k3s
                .as_ref()
                .map(|runtime| runtime.worker_nodes.len())
                .unwrap_or(0);
            let total_nodes = control_plane_nodes + worker_nodes;
            eprintln!(
                "     ✓ k3s cluster ({} node{})",
                total_nodes,
                if total_nodes == 1 { "" } else { "s" }
            );
        } else if handoff.compute_engine == "docker_compose" {
            eprintln!("     ✓ ⛴️ Docker container started");
        } else {
            eprintln!("     ✓ {} cluster", handoff.compute_engine);
        }
    }

    output::headline(
        &format!(
            "🏰 ✅ Infrastructure deployed - {}",
            output::duration_human(started.elapsed())
        ),
        color,
    );

    if let Some(handoff) = parsed_handoff.as_ref() {
        post_deploy::print_summary(handoff, color);
    } else {
        output::warn("workload_handoff unavailable; skipping rich post-deploy summary", color);
    }

    eprintln!("      👉🏻 Logs: {}", log_path.display());
    eprintln!("      👉🏻 Output: {}", output_path.display());
    Ok(())
}
