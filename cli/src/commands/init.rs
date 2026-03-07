use std::path::PathBuf;

use clap::Args;

use crate::easy_mode;
use crate::error::{CliError, Result};
use crate::examples;
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
    #[arg(long, num_args = 0..=1, default_missing_value = "__LIST__")]
    example: Option<String>,
    #[arg(long)]
    list_examples: bool,
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
    let started = std::time::Instant::now();
    let allow_raw_terraform = args.allow_raw_terraform || args.example.is_some();

    output::headline(
        &format!("🏰 ⚒️ Initializing project in {}", args.dir.display()),
        color,
    );

    if args.list_examples {
        let examples = examples::list_examples_from_cwd()?;
        for example in examples {
            if example.aliases.is_empty() {
                println!("{}", example.canonical);
            } else {
                println!(
                    "{}\taliases={}",
                    example.canonical,
                    example.aliases.join(",")
                );
            }
        }
        return Ok(());
    }

    if let Some(example) = &args.example {
        if args.config.is_some() {
            return Err(CliError::FlagConflict {
                message: "`--example` cannot be combined with `--config` in v1".to_string(),
            });
        }
        if args.mode.is_some() {
            return Err(CliError::FlagConflict {
                message: "`--example` cannot be combined with `--mode` in v1".to_string(),
            });
        }

        if example == examples::bare_example_sentinel() {
            output::error("`--example` requires a value. Available examples:", color);
            let available = examples::list_examples_from_cwd()?;
            for item in available {
                if item.aliases.is_empty() {
                    output::info(&format!("- {}", item.canonical), color);
                } else {
                    output::info(
                        &format!(
                            "- {} (aliases: {})",
                            item.canonical,
                            item.aliases.join(", ")
                        ),
                        color,
                    );
                }
            }
            return Err(CliError::FlagConflict {
                message: "missing value for `--example`".to_string(),
            });
        }

        let bootstrap = examples::bootstrap_example_to_dir(example, &args.dir, args.force)?;
        output::subline(
            &format!("📦 Bootstrapped example `{}`", bootstrap.canonical),
            color,
        );
        if bootstrap.wrote_power_metadata {
            output::subline("🎉 Generated evm-cloud.toml project metadata", color);
        }

        if args.dir.join("rindexer.yaml").exists() || args.dir.join("config/rindexer.yaml").exists()
        {
            output::subline("🦀 Rindexer Linked rindexer.yaml", color);
        }
    }

    std::fs::create_dir_all(&args.dir).map_err(|source| CliError::Io {
        source,
        path: args.dir.clone(),
    })?;

    let preflight = preflight::run_checks(&args.dir, allow_raw_terraform);

    let mut needs_reconfigure = false;

    let terraform_dir = match preflight {
        Ok(preflight) => {
            if args.config.is_some() && !args.force {
                return Err(CliError::FlagConflict {
                    message: "`--config` is only applied during scaffolding. Existing project detected; re-run with `--force` to regenerate, or omit `--config` to run terraform init only.".to_string(),
                });
            }

            if args.force {
                let answers = init_wizard::collect_answers(
                    args.config.as_deref(),
                    args.non_interactive,
                    args.mode,
                )?;
                init_scaffold::scaffold_project(&preflight.resolved_root, &answers, true, color)?;
            }

            match preflight.project_kind {
                ProjectKind::EasyToml => {
                    let (dir, scaffold) = easy_mode::prepare_workspace(&preflight.resolved_root, color)?;
                    if scaffold == crate::codegen::ScaffoldResult::BackendChanged {
                        easy_mode::warn_backend_changed(&preflight.resolved_root)?;
                        needs_reconfigure = true;
                    }
                    dir
                }
                ProjectKind::RawTerraform => {
                    output::checkline("Terraform project ready", color);
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
                InitMode::Easy => {
                    let (dir, scaffold) = easy_mode::prepare_workspace(&args.dir, color)?;
                    if scaffold == crate::codegen::ScaffoldResult::BackendChanged {
                        easy_mode::warn_backend_changed(&args.dir)?;
                        needs_reconfigure = true;
                    }
                    dir
                }
                InitMode::Power => {
                    output::checkline("Generated Terraform files (versions.tf, main.tf, variables.tf, outputs.tf)", color);
                    output::checkline("Generated secrets.auto.tfvars.example", color);
                    args.dir.clone()
                }
            }
        }
        Err(other) => return Err(other),
    };

    if args.skip_terraform_init {
        output::headline(
            &format!(
                "🏰 ✅ Project initialized - {}",
                output::duration_human(started.elapsed())
            ),
            color,
        );
        return Ok(());
    }

    let runner = TerraformRunner::check_installed(&terraform_dir)?;

    let mut init_args = args.terraform_args.clone();
    if needs_reconfigure
        && !init_args.iter().any(|a| a == "-reconfigure" || a == "-migrate-state")
    {
        init_args.push("-reconfigure".to_string());
    }

    output::with_terraforming(color, || runner.init(&terraform_dir, &init_args))?;

    if terraform_dir.join("versions.tf").is_file() {
        output::with_terraforming(color, || runner.fmt(&terraform_dir))?;
        output::with_terraforming(color, || runner.validate(&terraform_dir))?;
    }

    output::headline(
        &format!(
            "🏰 ✅ Project initialized - {}",
            output::duration_human(started.elapsed())
        ),
        color,
    );

    Ok(())
}
