use std::path::PathBuf;

use clap::Args;

use crate::easy_mode;
use crate::error::{CliError, Result};
use crate::init_answers::InitMode;
use crate::init_scaffold;
use crate::init_wizard;
use crate::output::{self, ColorMode};
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

#[derive(Args)]
pub(crate) struct InitArgs {
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,
    #[arg(long)]
    allow_raw_terraform: bool,
    #[arg(long)]
    non_interactive: bool,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    force: bool,
    #[arg(long, value_enum)]
    mode: Option<InitMode>,
    #[arg(long)]
    skip_terraform_init: bool,
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    terraform_args: Vec<String>,
}

pub(crate) fn run(args: InitArgs, color: ColorMode) -> Result<()> {
    std::fs::create_dir_all(&args.dir).map_err(|source| CliError::Io {
        source,
        path: args.dir.clone(),
    })?;

    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform);

    let terraform_dir = match preflight {
        Ok(preflight) => {
            if args.config.is_some() && !args.force {
                return Err(CliError::Message(
                    "`--config` is only applied during scaffolding. Existing project detected; re-run with `--force` to regenerate managed files, or omit `--config` to run terraform init only.".to_string(),
                ));
            }

            if args.force {
                let answers = init_wizard::collect_answers(
                    args.config.as_deref(),
                    args.non_interactive,
                    args.mode,
                )?;
                init_scaffold::scaffold_project(&preflight.resolved_root, &answers, true, color)?;
            } else {
                output::info("Project already exists; running terraform init without scaffolding.", color);
            }

            match preflight.project_kind {
                ProjectKind::EasyToml => {
                    output::info("Detected evm-cloud.toml project", color);
                    easy_mode::prepare_workspace(&preflight.resolved_root, color)?
                }
                ProjectKind::RawTerraform => {
                    output::info("Detected raw Terraform project (*.tf files)", color);
                    preflight.resolved_root.clone()
                }
            }
        }
        Err(CliError::NoProjectDetected { .. }) => {
            let answers = init_wizard::collect_answers(
                args.config.as_deref(),
                args.non_interactive,
                args.mode,
            )?;
            init_scaffold::scaffold_project(&args.dir, &answers, args.force, color)?;

            match answers.mode {
                InitMode::Easy => easy_mode::prepare_workspace(&args.dir, color)?,
                InitMode::Power => args.dir.clone(),
            }
        }
        Err(other) => return Err(other),
    };

    if args.skip_terraform_init {
        output::info("Scaffold complete; skipping terraform init by request.", color);
        return Ok(());
    }

    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    output::info(&format!("Using terraform {}", runner.version()), color);

    runner.init(&terraform_dir, &args.terraform_args)?;

    if terraform_dir.join("versions.tf").is_file() {
        runner.fmt(&terraform_dir)?;
        runner.validate(&terraform_dir)?;
    }

    Ok(())
}
