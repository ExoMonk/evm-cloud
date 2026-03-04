use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;

use clap::Args;

use crate::easy_mode;
use crate::commands::tfvars;
use crate::error::{CliError, Result};
use crate::handoff;
use crate::output::{self, ColorMode};
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

fn is_kube_destroy_target(runner: &TerraformRunner, terraform_dir: &std::path::Path) -> bool {
    let parsed_handoff = match runner.output_named_json(terraform_dir, "workload_handoff") {
        Ok(value) => handoff::parse_handoff_value(value).ok(),
        Err(CliError::TerraformOutputMissing { .. }) => runner
            .output_json(terraform_dir)
            .ok()
            .and_then(|output| handoff::parse_from_full_output(output, "evm_cloud").ok()),
        Err(_) => None,
    };

    let Some(parsed) = parsed_handoff else {
        return false;
    };

    parsed.compute_engine == "k3s" || parsed.compute_engine == "eks"
}

fn ensure_non_interactive_terraform(args: &mut Vec<String>) {
    if args.iter().any(|arg| arg == "-input=false" || arg == "-input=true") {
        return;
    }
    args.push("-input=false".to_string());
}

#[derive(Args)]
pub(crate) struct DestroyArgs {
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,
    #[arg(long)]
    auto_approve: bool,
    #[arg(long)]
    yes: bool,
    #[arg(long)]
    allow_raw_terraform: bool,
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    terraform_args: Vec<String>,
}

pub(crate) fn run(args: DestroyArgs, color: ColorMode) -> Result<()> {
    let started = Instant::now();
    output::headline_red(
        &format!("🏰 ⚒️ Removing Infra for {}", args.dir.display()),
        color,
    );

    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    let terraform_dir = match preflight.project_kind {
        ProjectKind::EasyToml => easy_mode::prepare_workspace(&preflight.resolved_root, color)?,
        ProjectKind::RawTerraform => preflight.resolved_root.clone(),
    };

    if !args.yes {
        return Err(CliError::Message(
            "destroy requires explicit acknowledgment: pass --yes".to_string(),
        ));
    }

    let non_interactive = !std::io::stdin().is_terminal();
    if non_interactive && !args.auto_approve {
        return Err(CliError::Message(
            "non-interactive shell detected: destroy requires --yes --auto-approve".to_string(),
        ));
    }

    if non_interactive {
        output::warn("running destroy in non-interactive mode", color);
    } else {
        output::warn("running destroy in interactive mode", color);
    }

    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    let kube_target = is_kube_destroy_target(&runner, &terraform_dir);

    let mut effective_args = args.terraform_args.clone();
    if let Some(auto_var_file) = tfvars::auto_var_file_arg(&terraform_dir, &effective_args)? {
        effective_args.push(auto_var_file);
    }

    if !args.auto_approve {
        eprintln!();
        let confirmed = output::confirmline("Destroy infrastructure?", color)
            .map_err(|err| CliError::Other(err.into()))?;

        if !confirmed {
            output::warn("Destroy cancelled", color);
            return Ok(());
        }
    }

    let mut destroy_args = effective_args.clone();
    ensure_non_interactive_terraform(&mut destroy_args);

    output::with_terraforming(color, || {
        runner.destroy_captured(&terraform_dir, true, &destroy_args)
    })?;

    if kube_target {
        output::checkline("Kube Pods tore down", color);
        eprintln!();
    }

    output::checkline("Ran terraform destroy", color);
    eprintln!();
    output::headline_red(
        &format!(
            "🏰 🚀 Destroy complete - {}",
            output::duration_human(started.elapsed())
        ),
        color,
    );
    Ok(())
}
