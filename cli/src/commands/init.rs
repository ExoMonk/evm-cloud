use std::path::PathBuf;

use clap::Args;

use crate::easy_mode;
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
    let terraform_dir = match preflight.project_kind {
        ProjectKind::EvmCloudToml => {
            output::info("Detected evm-cloud.toml project", color);
            easy_mode::prepare_workspace(&preflight.resolved_root, color)?
        }
        ProjectKind::RawTerraform => {
            output::info("Detected raw Terraform project (*.tf files)", color);
            preflight.resolved_root.clone()
        }
    };

    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    output::info(
        &format!("Using terraform {}", runner.version()),
        color,
    );

    runner.init(&terraform_dir, &args.terraform_args)
}
