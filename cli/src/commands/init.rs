use std::path::PathBuf;

use clap::Args;

use crate::commands::bootstrap;
use crate::config::loader;
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
    /// Create state backend resources after scaffolding (for non-interactive mode)
    #[arg(long)]
    bootstrap: bool,
    #[arg(long)]
    skip_terraform_init: bool,
    /// Target environment for multi-env projects (envs/<name>/)
    #[arg(long, env = "EVM_CLOUD_ENV")]
    env: Option<String>,
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    terraform_args: Vec<String>,
}

pub(crate) fn run(args: InitArgs, color: ColorMode) -> Result<()> {
    if args.env.is_some() {
        return Err(CliError::FlagConflict {
            message: "--env is not supported for `init`. Use `evm-cloud env add <name>` to create a new environment.".to_string(),
        });
    }

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
    let mut should_bootstrap = false;
    let mut project_root = args.dir.clone();

    let terraform_dir = match preflight {
        Ok(preflight) => {
            if args.config.is_some() && !args.force {
                return Err(CliError::FlagConflict {
                    message: "`--config` is only applied during scaffolding. Existing project detected; re-run with `--force` to regenerate, or omit `--config` to run terraform init only.".to_string(),
                });
            }

            project_root = preflight.resolved_root.clone();

            if args.force {
                // Extract existing [state] before overwrite (works with both full and minimal TOMLs)
                let toml_path = preflight.resolved_root.join("evm-cloud.toml");
                let existing_state = if toml_path.exists() {
                    loader::load_for_bootstrap(&toml_path)
                        .ok()
                        .and_then(|(_, s)| s)
                } else {
                    None
                };

                // Guard: warn if existing TOML has [state]
                if existing_state.is_some() {
                    if args.non_interactive {
                        output::warn(
                            "Overwriting existing [state] config (remote resources are NOT deleted)",
                            color,
                        );
                    } else {
                        let overwrite = dialoguer::Confirm::new()
                            .with_prompt("Existing [state] config will be overwritten (remote resources are NOT deleted). Continue?")
                            .default(false)
                            .interact()
                            .unwrap_or(false);
                        if !overwrite {
                            return Ok(());
                        }
                    }
                }

                let mut answers = init_wizard::collect_answers(
                    args.config.as_deref(),
                    args.non_interactive,
                    args.mode,
                )?;
                should_bootstrap = answers.auto_bootstrap || args.bootstrap;

                // Preserve existing [state] if wizard didn't collect one
                if answers.state_config.is_none() {
                    if let Some(state) = existing_state {
                        output::checkline("Preserving existing [state] from evm-cloud.toml", color);
                        answers.state_config = Some(state);
                    }
                }

                init_scaffold::scaffold_project(&preflight.resolved_root, &answers, true, color)?;
            }

            match preflight.project_kind {
                ProjectKind::EasyToml => {
                    let (dir, scaffold) =
                        easy_mode::prepare_workspace(&preflight.resolved_root, color)?;
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
        Err(CliError::RawTerraformOptInRequired { .. }) => {
            // Existing .tf files detected but no evm-cloud.toml or mode marker.
            // Create a minimal TOML + power mode marker so the CLI can track
            // the project without overwriting existing Terraform files.
            let default_name = args
                .dir
                .canonicalize()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| "evm-cloud-project".to_string());

            let name = if args.non_interactive {
                default_name
            } else {
                output::info(
                    "Detected existing Terraform files — creating minimal project metadata.",
                    color,
                );
                dialoguer::Input::<String>::new()
                    .with_prompt("Project name")
                    .default(default_name)
                    .interact()
                    .unwrap_or_else(|_| "evm-cloud-project".to_string())
            };

            init_scaffold::scaffold_raw_project(&args.dir, &name, &None, args.force, color)?;
            args.dir.clone()
        }
        Err(CliError::NoProjectDetected { .. }) => {
            let mut answers = init_wizard::collect_answers(
                args.config.as_deref(),
                args.non_interactive,
                args.mode,
            )?;
            should_bootstrap = answers.auto_bootstrap || args.bootstrap;

            // Preserve existing [state] if wizard didn't collect one
            // (e.g. user ran `evm-cloud bootstrap` first with a minimal TOML)
            if answers.state_config.is_none() {
                let toml_path = args.dir.join("evm-cloud.toml");
                if toml_path.exists() {
                    if let Ok((_, Some(state))) = loader::load_for_bootstrap(&toml_path) {
                        output::checkline("Preserving existing [state] from evm-cloud.toml", color);
                        answers.state_config = Some(state);
                    }
                }
            }

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

    // Auto-bootstrap state backend if requested.
    // Use project_root (not terraform_dir) because in Easy mode terraform_dir is .evm-cloud/
    // but evm-cloud.toml lives at the project root.
    if should_bootstrap {
        if let Err(err) = bootstrap::run_inline(&project_root, color) {
            output::warn(
                &format!(
                    "Bootstrap failed: {err}. Run 'evm-cloud bootstrap' to retry after resolving the issue."
                ),
                color,
            );
        }
    }

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
        && !init_args
            .iter()
            .any(|a| a == "-reconfigure" || a == "-migrate-state")
    {
        init_args.push("-reconfigure".to_string());
    }

    // Auto-add -backend-config if a .tfbackend file exists in the terraform dir.
    // NOTE: This calls runner.init() directly (not init_if_needed), so no double injection.
    if let Some(tfbackend) = crate::terraform::find_tfbackend(&terraform_dir) {
        init_args.insert(0, format!("-backend-config={}", tfbackend.display()));
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
