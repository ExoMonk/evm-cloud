use std::path::PathBuf;

use clap::Args;

use crate::error::Result;
use crate::output::{self, ColorMode};
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

#[derive(Args)]
pub(crate) struct InitArgs {
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,
    #[arg(long)]
    allow_raw_terraform: bool,
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    terraform_args: Vec<String>,
}

pub(crate) fn run(args: InitArgs, color: ColorMode) -> Result<()> {
    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    match preflight.project_kind {
        ProjectKind::EvmCloudToml => output::info("Detected evm-cloud.toml project", color),
        ProjectKind::RawTerraform => output::info("Detected raw Terraform project (*.tf files)", color),
    }

    let runner = TerraformRunner::check_installed(&preflight.resolved_root)?;
    output::info(
        &format!("Using terraform {}", runner.version()),
        color,
    );

    runner.init(&preflight.resolved_root, &args.terraform_args)
}
