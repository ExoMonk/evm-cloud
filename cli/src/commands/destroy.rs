use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Args;

use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

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
    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    match preflight.project_kind {
        ProjectKind::EvmCloudToml => output::info("Detected evm-cloud.toml project", color),
        ProjectKind::RawTerraform => output::info("Detected raw Terraform project (*.tf files)", color),
    }

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
        output::warn("Running destroy in CI/non-interactive mode", color);
    } else {
        output::warn("Running destroy in interactive mode", color);
    }

    let runner = TerraformRunner::check_installed(&preflight.resolved_root)?;
    output::info(
        &format!("Using terraform {}", runner.version()),
        color,
    );

    runner.destroy(&preflight.resolved_root, args.auto_approve, &args.terraform_args)
}
