use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Args;

use crate::easy_mode;
use crate::error::{CliError, Result};
use crate::output::{self, ColorMode};
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

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
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    terraform_args: Vec<String>,
}

pub(crate) fn run(args: ApplyArgs, color: ColorMode) -> Result<()> {
    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    let terraform_dir = match preflight.project_kind {
        ProjectKind::EasyToml => {
            output::info("Detected evm-cloud.toml project", color);
            easy_mode::prepare_workspace(&preflight.resolved_root, color)?
        }
        ProjectKind::RawTerraform => {
            output::info("Detected raw Terraform project (*.tf files)", color);
            preflight.resolved_root.clone()
        }
    };

    if !std::io::stdin().is_terminal() && !args.auto_approve && !args.dry_run {
        return Err(CliError::Message(
            "non-interactive shell detected: re-run with --auto-approve".to_string(),
        ));
    }

    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    output::info(
        &format!("Using terraform {}", runner.version()),
        color,
    );

    runner.init(&terraform_dir, &[])?;

    if args.dry_run {
        runner.plan(&terraform_dir, &args.terraform_args)?;
        output::info("Dry run complete (terraform plan).", color);
        return Ok(());
    }

    runner.apply(&terraform_dir, args.auto_approve, &args.terraform_args)?;
    output::info("Apply complete.", color);
    Ok(())
}
