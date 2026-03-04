use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Args;

use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

#[derive(Args)]
pub(crate) struct ApplyArgs {
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,
    #[arg(long)]
    auto_approve: bool,
    #[arg(long)]
    allow_raw_terraform: bool,
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    terraform_args: Vec<String>,
}

pub(crate) fn run(args: ApplyArgs, color: ColorMode) -> Result<()> {
    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    match preflight.project_kind {
        ProjectKind::EvmCloudToml => output::info("Detected evm-cloud.toml project", color),
        ProjectKind::RawTerraform => output::info("Detected raw Terraform project (*.tf files)", color),
    }

    if !std::io::stdin().is_terminal() && !args.auto_approve {
        return Err(CliError::Message(
            "non-interactive shell detected: re-run with --auto-approve".to_string(),
        ));
    }

    let runner = TerraformRunner::check_installed(&preflight.resolved_root)?;
    output::info(
        &format!("Using terraform {}", runner.version()),
        color,
    );

    runner.init(&preflight.resolved_root, &[])?;
    runner.apply(&preflight.resolved_root, args.auto_approve, &args.terraform_args)?;
    output::info("Apply complete.", color);
    Ok(())
}
