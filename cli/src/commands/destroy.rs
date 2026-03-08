use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;

use clap::Args;

use crate::commands::apply::ensure_non_interactive_terraform;
use crate::commands::tfvars;
use crate::config::schema::ComputeEngine;
use crate::easy_mode;
use crate::error::{CliError, Result};
use crate::handoff;
use crate::output::{self, ColorMode};
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

fn is_kube_destroy_target(runner: &TerraformRunner, terraform_dir: &std::path::Path) -> bool {
    handoff::try_load_from_state(runner, terraform_dir, "evm_cloud")
        .map(|h| matches!(h.compute_engine, ComputeEngine::K3s | ComputeEngine::Eks))
        .unwrap_or(false)
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
    /// Target environment for multi-env projects (envs/<name>/)
    #[arg(long, env = "EVM_CLOUD_ENV")]
    env: Option<String>,
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    terraform_args: Vec<String>,
}

pub(crate) fn run(args: DestroyArgs, color: ColorMode) -> Result<()> {
    let started = Instant::now();

    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    let project_root = preflight.resolved_root.clone();
    let env_ctx = crate::env::resolve_env(args.env.as_deref(), &project_root)?;

    let env_hint = env_ctx
        .as_ref()
        .map(|c| format!(" [env: {}]", c.name))
        .unwrap_or_default();
    output::headline_red(
        &format!("🏰 ⚒️ Removing Infra for {}{}", args.dir.display(), env_hint),
        color,
    );

    let terraform_dir = match preflight.project_kind {
        ProjectKind::EasyToml => {
            // Silently regenerate bridge files — destroy needs matching variable
            // declarations but the user doesn't need to see "Generated" output.
            let (dir, scaffold) = easy_mode::prepare_workspace_quiet(&preflight.resolved_root)?;
            if scaffold == crate::codegen::ScaffoldResult::BackendChanged {
                return Err(easy_mode::handle_backend_changed(&preflight.resolved_root));
            }
            dir
        }
        ProjectKind::RawTerraform => preflight.resolved_root.clone(),
    };

    if !args.yes {
        return Err(CliError::FlagConflict {
            message: "destroy requires explicit acknowledgment: pass --yes".to_string(),
        });
    }

    let non_interactive = !std::io::stdin().is_terminal();
    if non_interactive && !args.auto_approve {
        return Err(CliError::FlagConflict {
            message: "non-interactive shell detected: destroy requires --yes --auto-approve"
                .to_string(),
        });
    }

    if non_interactive {
        output::warn("running destroy in non-interactive mode", color);
    } else {
        output::warn("running destroy in interactive mode", color);
    }

    // Extra safety: require typing the env name for production environments.
    if let Some(ref ctx) = env_ctx {
        let lower = ctx.name.to_lowercase();
        if (lower == "prod" || lower == "production") && !args.auto_approve {
            let prompt = format!(
                "You are about to destroy the '{}' environment. Type '{}' to confirm",
                ctx.name, ctx.name
            );
            let input: String = dialoguer::Input::new()
                .with_prompt(&prompt)
                .interact_text()
                .map_err(|err| CliError::PromptFailed(err.to_string()))?;
            if input != ctx.name {
                output::warn("Destroy cancelled — name did not match", color);
                return Ok(());
            }
        }
    }

    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    let runner = match env_ctx.as_ref() {
        Some(ctx) => runner.with_env(ctx),
        None => runner,
    };
    let kube_target = is_kube_destroy_target(&runner, &terraform_dir);

    let mut effective_args = args.terraform_args.clone();
    if let Some(auto_var_file) = tfvars::auto_var_file_arg(&terraform_dir, &effective_args)? {
        effective_args.push(auto_var_file);
    }

    if !args.auto_approve {
        let confirmed = output::confirmline("Destroy infrastructure?", color)
            .map_err(|err| CliError::PromptFailed(err.to_string()))?;

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
        output::checkline("🛟 Pods tore down", color);
    }

    output::checkline("Ran terraform destroy", color);
    output::headline_red(
        &format!(
            "🏰 🚀 Destroy complete - {}",
            output::duration_human(started.elapsed())
        ),
        color,
    );
    Ok(())
}
