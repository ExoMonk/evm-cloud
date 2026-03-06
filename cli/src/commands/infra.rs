use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use crate::commands::apply::{ensure_non_interactive_terraform, terraform_log_path, terraform_output_path};
use crate::commands::tfvars;
use crate::config::schema::ComputeEngine;
use crate::error::{CliError, Result};
use crate::handoff::{self, WorkloadHandoff};
use crate::output::{self, ColorMode};
use crate::terraform::TerraformRunner;

pub(crate) struct InfraPhaseOpts<'a> {
    pub terraform_dir: &'a Path,
    pub tf_args: &'a [String],
    pub module_name: &'a str,
    pub dry_run: bool,
    pub auto_approve: bool,
    pub json: bool,
    pub color: ColorMode,
}

pub(crate) enum InfraPhaseOutcome {
    DryRun {
        log_path: PathBuf,
        output_path: PathBuf,
    },
    Cancelled,
    Applied {
        handoff: Option<WorkloadHandoff>,
        log_path: PathBuf,
        output_path: PathBuf,
    },
}

pub(crate) fn run_infra_phase(opts: InfraPhaseOpts<'_>) -> Result<InfraPhaseOutcome> {
    let render_color = if opts.json { ColorMode::Never } else { opts.color };

    if !std::io::stdin().is_terminal() && !opts.auto_approve && !opts.dry_run {
        return Err(CliError::FlagConflict {
            message: "non-interactive shell detected: re-run with --auto-approve".to_string(),
        });
    }

    let runner = TerraformRunner::check_installed(opts.terraform_dir)?;

    let mut effective_args: Vec<String> = opts.tf_args.to_vec();
    if let Some(auto_var_file) = tfvars::auto_var_file_arg(opts.terraform_dir, &effective_args)? {
        effective_args.push(auto_var_file);
    }
    let mut plan_args = effective_args.clone();
    ensure_non_interactive_terraform(&mut plan_args);

    runner.init_if_needed(opts.terraform_dir, &[])?;

    let log_path = terraform_log_path(
        opts.terraform_dir,
        if opts.dry_run { "plan" } else { "apply" },
    )?;
    let output_path = terraform_output_path(opts.terraform_dir)?;

    // Plan
    output::with_terraforming(render_color, || {
        runner.plan_with_log(opts.terraform_dir, &plan_args, &log_path)
    })?;

    if opts.dry_run {
        fs::write(
            &output_path,
            "{\n  \"status\": \"dry-run\",\n  \"note\": \"no terraform outputs generated for plan\"\n}\n",
        )
        .map_err(|source| CliError::Io {
            source,
            path: output_path.clone(),
        })?;
        return Ok(InfraPhaseOutcome::DryRun {
            log_path,
            output_path,
        });
    }

    if !opts.json {
        output::checkline("Ran terraform plan", opts.color);
    }

    // Confirmation
    if !opts.auto_approve && !opts.json {
        let confirmed = output::confirmline("Apply these changes?", opts.color)
            .map_err(|err| CliError::PromptFailed(err.to_string()))?;
        if !confirmed {
            return Ok(InfraPhaseOutcome::Cancelled);
        }
    }

    // Apply
    let mut apply_args = effective_args;
    ensure_non_interactive_terraform(&mut apply_args);
    output::with_terraforming(render_color, || {
        runner.apply_captured_with_log(opts.terraform_dir, true, &apply_args, &log_path)
    })?;

    // Parse handoff from terraform output
    let mut parsed_handoff = None;
    if let Ok(outputs) = runner.output_json(opts.terraform_dir) {
        match handoff::parse_from_full_output(outputs.clone(), opts.module_name) {
            Ok(h) => parsed_handoff = Some(h),
            Err(err) => output::warn(&format!("handoff parse failed: {err}"), render_color),
        }
        let rendered =
            serde_json::to_string_pretty(&outputs).map_err(CliError::OutputParseError)?;
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

    Ok(InfraPhaseOutcome::Applied {
        handoff: parsed_handoff,
        log_path,
        output_path,
    })
}

pub(crate) fn print_compute_summary(handoff: &WorkloadHandoff, color: ColorMode) {
    output::checkline("Ran terraform apply", color);
    eprintln!("     ✓ VPC + networking");

    match handoff.compute_engine {
        ComputeEngine::K3s => {
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
                "      ✓ k3s cluster ({} node{})",
                total_nodes,
                if total_nodes == 1 { "" } else { "s" }
            );
        }
        ComputeEngine::DockerCompose => {
            eprintln!("      ✓ ⛴️ Docker container started");
        }
        ComputeEngine::Ec2 => {
            eprintln!("      ✓ EC2 instance");
        }
        ComputeEngine::Eks => {
            eprintln!("      ✓ EKS cluster");
        }
    }
}
