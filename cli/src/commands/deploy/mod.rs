mod helpers;

use std::path::PathBuf;
use std::time::Instant;

use clap::{Args, ValueEnum};

use crate::commands::infra::{self, InfraPhaseOpts, InfraPhaseOutcome};
use crate::config::schema::ComputeEngine;
use crate::deployer::{Action, DeployLockGuard};
use crate::easy_mode;
use crate::error::{CliError, Result};
use crate::handoff;
use crate::output::{self, ColorMode};
use crate::post_deploy;
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

use helpers::{
    backfill_inline_clickhouse_password, ensure_config_dir, generate_env_file, has_flag_with_value,
    invoke_with_optional_timeout, resolve_ssh_vars_from_tfvars,
};

/// Phase selector for the deploy pipeline.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum DeployPhase {
    /// Terraform only (provision infrastructure, skip workload deployer)
    Infra,
    /// Deployer only (deploy workloads to existing infrastructure)
    App,
}

#[derive(Args)]
#[command(about = "Provision infrastructure and deploy workloads")]
pub(crate) struct DeployArgs {
    /// Project directory
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,

    /// Terraform module name for handoff extraction (Power mode)
    #[arg(long, default_value = "evm_cloud")]
    module_name: String,

    /// Allow raw Terraform projects (no evm-cloud.toml)
    #[arg(long)]
    allow_raw_terraform: bool,

    /// Output JSON instead of human-readable text
    #[arg(long)]
    json: bool,

    /// Show terraform plan without applying or deploying
    #[arg(long)]
    dry_run: bool,

    /// Auto-approve terraform apply (skip interactive confirmation)
    #[arg(long)]
    auto_approve: bool,

    /// Run only a specific phase instead of the full pipeline
    #[arg(long, value_enum)]
    only: Option<DeployPhase>,

    /// Extra arguments passed to terraform (plan/apply).
    /// Example: --tf-args='-var=foo=bar' --tf-args='-target=module.x'
    #[arg(long, allow_hyphen_values = true)]
    tf_args: Vec<String>,

    /// Timeout for deployer phase in seconds.
    /// Default: none (no timeout). Deployer scripts have their own
    /// internal Helm rollout timeouts (typically 300-600s per component).
    /// Use this as an outer safety net for truly stuck deploys.
    #[arg(long)]
    deploy_timeout: Option<u64>,

    /// Extra arguments passed to the deployer (Helm/Docker Compose).
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    deployer_args: Vec<String>,
}

fn validate_flags(args: &DeployArgs) -> Result<()> {
    let is_app_only = matches!(args.only, Some(DeployPhase::App));

    if is_app_only && args.dry_run {
        return Err(CliError::FlagConflict {
            message: "--dry-run controls terraform plan; incompatible with --only app".to_string(),
        });
    }
    if is_app_only && args.auto_approve {
        return Err(CliError::FlagConflict {
            message: "--auto-approve controls terraform; incompatible with --only app".to_string(),
        });
    }
    if is_app_only && !args.tf_args.is_empty() {
        return Err(CliError::FlagConflict {
            message: "--tf-args applies to terraform; incompatible with --only app".to_string(),
        });
    }

    Ok(())
}

pub(crate) fn run(args: DeployArgs, color: ColorMode) -> Result<()> {
    validate_flags(&args)?;

    let started = Instant::now();
    let is_app_only = matches!(args.only, Some(DeployPhase::App));
    let is_infra_only = matches!(args.only, Some(DeployPhase::Infra));
    let run_infra = !is_app_only;
    let run_app = !is_infra_only;

    if !args.json {
        let phase_hint = match args.only {
            Some(DeployPhase::Infra) => " (infra only)",
            Some(DeployPhase::App) => " (app only)",
            None => "",
        };
        output::headline(
            &format!(
                "🏰 ⚒️ Deploying stack for {}{}",
                args.dir.display(),
                phase_hint
            ),
            color,
        );
    }

    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    let project_root = preflight.resolved_root.clone();
    let terraform_dir = match preflight.project_kind {
        ProjectKind::EasyToml => {
            let (dir, scaffold) = easy_mode::prepare_workspace_quiet(&project_root)?;
            if scaffold == crate::codegen::ScaffoldResult::BackendChanged {
                return Err(easy_mode::handle_backend_changed(&project_root));
            }
            dir
        }
        ProjectKind::RawTerraform => project_root.clone(),
    };

    // ── Infra phase (terraform) ─────────────────────────────────────────
    let mut parsed_handoff = None;
    let mut log_path = None;
    let mut output_path = None;
    let is_bundled = run_infra && run_app;

    if run_infra {
        if is_bundled && !args.json {
            output::subline("🏗️ Phase 1/2 — Deploying Infrastructure", color);
        }

        match infra::run_infra_phase(InfraPhaseOpts {
            terraform_dir: &terraform_dir,
            tf_args: &args.tf_args,
            module_name: &args.module_name,
            dry_run: args.dry_run,
            auto_approve: args.auto_approve,
            json: args.json,
            color,
        })? {
            InfraPhaseOutcome::DryRun {
                log_path: lp,
                output_path: op,
            } => {
                if !args.json {
                    output::checkline("Ran terraform plan", color);
                    output::headline(
                        &format!(
                            "🏖️ ✅ Dry run complete - {}",
                            output::duration_human(started.elapsed())
                        ),
                        color,
                    );
                    eprintln!("      👉🏻 Logs: {}", lp.display());
                    eprintln!("      👉🏻 Output: {}", op.display());
                }
                return Ok(());
            }
            InfraPhaseOutcome::Cancelled => {
                output::warn("Deploy cancelled", color);
                return Ok(());
            }
            InfraPhaseOutcome::Applied {
                handoff,
                log_path: lp,
                output_path: op,
            } => {
                let handoff = *handoff;
                if !args.json {
                    if let Some(h) = handoff.as_ref() {
                        infra::print_compute_summary(h, color);
                    }
                }
                parsed_handoff = handoff;
                log_path = Some(lp);
                output_path = Some(op);
            }
        }
    }

    // ── App phase (deployer) ────────────────────────────────────────────
    if run_app {
        if is_bundled && !args.json {
            output::subline("🧑‍🌾 Phase 2/2 — Deploying Workload", color);
        }

        let runner = TerraformRunner::check_installed(&terraform_dir)?;

        // If we didn't run infra phase, we need to read handoff from existing state.
        if parsed_handoff.is_none() {
            runner.init_if_needed(&terraform_dir, &[])?;
            let mut handoff_result =
                handoff::load_from_state(&runner, &terraform_dir, &args.module_name)?;

            backfill_inline_clickhouse_password(
                &mut handoff_result,
                &project_root,
                &preflight.project_kind,
            )?;
            parsed_handoff = Some(handoff_result);
        }

        let handoff = parsed_handoff.as_mut().unwrap();

        // When workload_mode=terraform, TF provisioners already deployed the
        // workloads during apply. Skip the external deployer unless the user
        // explicitly requested --only app (meaning they want to re-deploy).
        if handoff.mode == "terraform" && !is_app_only {
            if !args.json {
                output::checkline("Workloads deployed by Terraform provisioners", color);
            }
        } else {
            // Backfill for handoffs that came from the infra phase too.
            backfill_inline_clickhouse_password(handoff, &project_root, &preflight.project_kind)?;
            handoff::validate_for_action(handoff, Action::Deploy, &args.deployer_args)?;

            let mut effective_deployer_args = args.deployer_args.clone();
            let resolved_config_dir =
                if matches!(
                    handoff.compute_engine,
                    ComputeEngine::K3s | ComputeEngine::Ec2 | ComputeEngine::DockerCompose
                ) && !has_flag_with_value(&effective_deployer_args, "--config-dir")
                {
                    let config_dir = ensure_config_dir(&project_root)?;
                    effective_deployer_args.push("--config-dir".to_string());
                    effective_deployer_args.push(config_dir.display().to_string());
                    Some(config_dir)
                } else {
                    None
                };

            // Compose deployer (ec2/docker_compose): generate .env from tfvars secrets
            // so the deployer can upload it to the remote host.
            if matches!(
                handoff.compute_engine,
                ComputeEngine::Ec2 | ComputeEngine::DockerCompose
            ) {
                if let Some(ref config_dir) = resolved_config_dir {
                    generate_env_file(config_dir, &project_root, &preflight.project_kind, handoff)?;
                }
            }

            // Compose deployer (ec2/docker_compose) needs SSH args for SCP/SSH.
            // Auto-inject from tfvars if not explicitly provided.
            if matches!(
                handoff.compute_engine,
                ComputeEngine::Ec2 | ComputeEngine::DockerCompose
            ) {
                let ssh_vars =
                    resolve_ssh_vars_from_tfvars(&project_root, &preflight.project_kind)?;

                if !has_flag_with_value(&effective_deployer_args, "--ssh-key") {
                    if let Some(ref key_path) = ssh_vars.key_path {
                        effective_deployer_args.push("--ssh-key".to_string());
                        effective_deployer_args.push(key_path.clone());
                    }
                }
                if !has_flag_with_value(&effective_deployer_args, "--user") {
                    if let Some(ref user) = ssh_vars.user {
                        effective_deployer_args.push("--user".to_string());
                        effective_deployer_args.push(user.clone());
                    }
                }
                if !has_flag_with_value(&effective_deployer_args, "--port") {
                    if let Some(ref port) = ssh_vars.port {
                        effective_deployer_args.push("--port".to_string());
                        effective_deployer_args.push(port.clone());
                    }
                }
            }

            let _lock = DeployLockGuard::acquire(
                &project_root,
                if args.json { ColorMode::Never } else { color },
            )?;

            let deploy_result = invoke_with_optional_timeout(
                handoff,
                &effective_deployer_args,
                args.deploy_timeout,
                args.json,
                color,
            );

            if let Err(ref _err) = deploy_result {
                // Phase-aware recovery hint: infra succeeded, deployer failed.
                if run_infra && !args.json {
                    eprintln!();
                    output::warn(
                        "Terraform applied successfully. Infrastructure is provisioned.",
                        color,
                    );
                    eprintln!(
                        "      Retry deployer only: evm-cloud deploy --only app --dir {}",
                        args.dir.display()
                    );
                }
                return deploy_result;
            }
        } // else (external deployer)
    }

    // ── Summary ─────────────────────────────────────────────────────────
    if args.json {
        if let Some(handoff) = parsed_handoff.as_ref() {
            println!(
                "{}",
                serde_json::to_string_pretty(handoff).map_err(CliError::OutputParseError)?
            );
        } else {
            println!(
                "{}",
                serde_json::json!({
                    "status": "deployed",
                    "handoff": null,
                    "note": "workload_handoff unavailable"
                })
            );
        }
        return Ok(());
    }

    let phase_label = match (run_infra, run_app) {
        (true, true) => "Stack deployed",
        (true, false) => "Infrastructure deployed",
        (false, true) => "App deployed",
        (false, false) => "Complete",
    };

    output::headline(
        &format!(
            "🏰 ✅ {} - {}",
            phase_label,
            output::duration_human(started.elapsed())
        ),
        color,
    );

    if let Some(handoff) = parsed_handoff.as_ref() {
        post_deploy::print_summary(handoff, color);
    } else {
        output::warn(
            "workload_handoff unavailable; skipping rich post-deploy summary",
            color,
        );
    }

    if let Some(lp) = log_path {
        eprintln!("      👉🏻 Logs: {}", lp.display());
    }
    if let Some(op) = output_path {
        eprintln!("      👉🏻 Output: {}", op.display());
    }

    Ok(())
}
