use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;
use std::{fs, path::Path};

use clap::{Args, ValueEnum};
use serde_json::Value;

use crate::commands::apply::{
    ensure_non_interactive_terraform, terraform_log_path, terraform_output_path,
};
use crate::commands::tfvars;
use crate::deployer::{self, Action, DeployLockGuard, InvokeOptions};
use crate::easy_mode;
use crate::error::{CliError, Result};
use crate::handoff;
use crate::output::{self, ColorMode};
use crate::post_deploy;
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

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
        return Err(CliError::DeployFlagConflict {
            message: "--dry-run controls terraform plan; incompatible with --only app".to_string(),
        });
    }
    if is_app_only && args.auto_approve {
        return Err(CliError::DeployFlagConflict {
            message: "--auto-approve controls terraform; incompatible with --only app".to_string(),
        });
    }
    if is_app_only && !args.tf_args.is_empty() {
        return Err(CliError::DeployFlagConflict {
            message: "--tf-args applies to terraform; incompatible with --only app".to_string(),
        });
    }

    Ok(())
}

pub(crate) fn run(args: DeployArgs, color: ColorMode) -> Result<()> {
    validate_flags(&args)?;

    let started = Instant::now();
    let render_color = if args.json { ColorMode::Never } else { color };
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
        ProjectKind::EasyToml => easy_mode::prepare_workspace_quiet(&project_root)?,
        ProjectKind::RawTerraform => project_root.clone(),
    };

    // ── Infra phase (terraform) ─────────────────────────────────────────
    let mut parsed_handoff = None;
    let mut log_path = None;
    let mut output_path = None;
    let is_bundled = run_infra && run_app;

    if run_infra {
        if is_bundled && !args.json {
            output::subline("Phase 1/2 — Deploying Infrastructure", color);
        }
        if !std::io::stdin().is_terminal() && !args.auto_approve && !args.dry_run {
            return Err(CliError::Message(
                "non-interactive shell detected: re-run with --auto-approve".to_string(),
            ));
        }

        let runner = TerraformRunner::check_installed(&terraform_dir)?;

        let mut effective_tf_args = args.tf_args.clone();
        if let Some(auto_var_file) =
            tfvars::auto_var_file_arg(&terraform_dir, &effective_tf_args)?
        {
            effective_tf_args.push(auto_var_file);
        }
        let mut plan_args = effective_tf_args.clone();
        ensure_non_interactive_terraform(&mut plan_args);

        runner.init_if_needed(&terraform_dir, &[])?;

        let lp = terraform_log_path(
            &terraform_dir,
            if args.dry_run { "plan" } else { "apply" },
        )?;
        let op = terraform_output_path(&terraform_dir)?;

        // Plan
        output::with_terraforming(render_color, || {
            runner.plan_with_log(&terraform_dir, &plan_args, &lp)
        })?;

        if args.dry_run {
            fs::write(
                &op,
                "{\n  \"status\": \"dry-run\",\n  \"note\": \"no terraform outputs generated for plan\"\n}\n",
            )
            .map_err(|source| CliError::Io {
                source,
                path: op.clone(),
            })?;

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

        if !args.json {
            output::checkline("Ran terraform plan", color);
        }

        // Confirmation
        if !args.auto_approve && !args.json {
            let confirmed = output::confirmline("Apply these changes?", color)
                .map_err(|err| CliError::Other(err.into()))?;
            if !confirmed {
                output::warn("Deploy cancelled", color);
                return Ok(());
            }
        }

        // Apply
        let mut apply_args = effective_tf_args.clone();
        ensure_non_interactive_terraform(&mut apply_args);
        output::with_terraforming(render_color, || {
            runner.apply_captured_with_log(&terraform_dir, true, &apply_args, &lp)
        })?;

        // Parse handoff from terraform output
        if let Ok(outputs) = runner.output_json(&terraform_dir) {
            parsed_handoff =
                handoff::parse_from_full_output(outputs.clone(), &args.module_name).ok();
            let rendered =
                serde_json::to_string_pretty(&outputs).map_err(CliError::OutputParseError)?;
            fs::write(&op, format!("{rendered}\n")).map_err(|source| CliError::Io {
                source,
                path: op.clone(),
            })?;
        } else {
            fs::write(
                &op,
                "{\n  \"status\": \"apply\",\n  \"note\": \"terraform output unavailable\"\n}\n",
            )
            .map_err(|source| CliError::Io {
                source,
                path: op.clone(),
            })?;
        }

        if !args.json {
            output::checkline("Ran terraform apply", color);
            eprintln!("     ✓  VPC + networking");

            if let Some(handoff) = parsed_handoff.as_ref() {
                if handoff.compute_engine == "k3s" {
                    let control_plane_nodes = handoff
                        .runtime
                        .k3s
                        .as_ref()
                        .and_then(|runtime| runtime.node_name.as_ref())
                        .map(|name| !name.trim().is_empty())
                        .unwrap_or(false) as usize;
                    let worker_nodes = handoff
                        .runtime
                        .k3s
                        .as_ref()
                        .map(|runtime| runtime.worker_nodes.len())
                        .unwrap_or(0);
                    let total_nodes = control_plane_nodes + worker_nodes;
                    eprintln!(
                        "     ✓ k3s cluster ({} node{})",
                        total_nodes,
                        if total_nodes == 1 { "" } else { "s" }
                    );
                } else if handoff.compute_engine == "docker_compose" {
                    eprintln!("     ✓ ⛴️ Docker container started");
                } else {
                    eprintln!("     ✓ {} cluster", handoff.compute_engine);
                }
            }
        }

        log_path = Some(lp);
        output_path = Some(op);
    }

    // ── App phase (deployer) ────────────────────────────────────────────
    if run_app {
        if is_bundled && !args.json {
            output::subline("Phase 2/2 — Deploying Workload", color);
        }

        let runner = TerraformRunner::check_installed(&terraform_dir)?;

        // If we didn't run infra phase, we need to read handoff from existing state.
        if parsed_handoff.is_none() {
            runner.init_if_needed(&terraform_dir, &[])?;
            let mut handoff_result =
                match runner.output_named_json(&terraform_dir, "workload_handoff") {
                    Ok(value) => handoff::parse_handoff_value(value)?,
                    Err(CliError::TerraformOutputMissing { .. }) => {
                        let full_output = runner.output_json(&terraform_dir)?;
                        handoff::parse_from_full_output(full_output, &args.module_name)?
                    }
                    Err(err) => return Err(err),
                };

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
        let resolved_config_dir = if matches!(handoff.compute_engine.as_str(), "k3s" | "ec2" | "docker_compose")
            && !has_flag_with_value(&effective_deployer_args, "--config-dir")
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
        if matches!(handoff.compute_engine.as_str(), "ec2" | "docker_compose") {
            if let Some(ref config_dir) = resolved_config_dir {
                generate_env_file(config_dir, &project_root, &preflight.project_kind, handoff)?;
            }
        }

        // Compose deployer (ec2/docker_compose) needs SSH args for SCP/SSH.
        // Auto-inject from tfvars if not explicitly provided.
        if matches!(handoff.compute_engine.as_str(), "ec2" | "docker_compose") {
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

fn invoke_with_optional_timeout(
    handoff: &handoff::WorkloadHandoff,
    deployer_args: &[String],
    timeout_secs: Option<u64>,
    json: bool,
    color: ColorMode,
) -> Result<()> {
    let invoke = || {
        deployer::invoke_deployer(
            handoff,
            Action::Deploy,
            InvokeOptions {
                passthrough_args: deployer_args,
                quiet_output: true,
                color: if json { ColorMode::Never } else { color },
                compute_engine: handoff.compute_engine.clone(),
            },
        )
    };

    match timeout_secs {
        Some(secs) => {
            // Use a thread + channel for timeout since deployer is sync (spawns child process).
            let (tx, rx) = std::sync::mpsc::channel();
            let handoff_clone = handoff.clone();
            let args_clone = deployer_args.to_vec();
            std::thread::spawn(move || {
                let result = deployer::invoke_deployer(
                    &handoff_clone,
                    Action::Deploy,
                    InvokeOptions {
                        passthrough_args: &args_clone,
                        quiet_output: true,
                        color: if json { ColorMode::Never } else { color },
                        compute_engine: handoff_clone.compute_engine.clone(),
                    },
                );
                let _ = tx.send(result);
            });

            match rx.recv_timeout(std::time::Duration::from_secs(secs)) {
                Ok(result) => result,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    Err(CliError::DeployerTimedOut { seconds: secs })
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    Err(CliError::Message("deployer thread panicked".to_string()))
                }
            }
        }
        None => invoke(),
    }
}

fn backfill_inline_clickhouse_password(
    handoff: &mut handoff::WorkloadHandoff,
    project_root: &Path,
    project_kind: &ProjectKind,
) -> Result<()> {
    if handoff.compute_engine != "k3s" {
        return Ok(());
    }

    let secrets_mode = handoff
        .secrets
        .get("mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("inline");
    if secrets_mode != "inline" {
        return Ok(());
    }

    let backend = handoff
        .data
        .get("backend")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if backend != "clickhouse" {
        return Ok(());
    }

    let has_password = handoff
        .data
        .get("clickhouse")
        .and_then(Value::as_object)
        .and_then(|ch| ch.get("password"))
        .and_then(Value::as_str)
        .map(|password| !password.trim().is_empty())
        .unwrap_or(false);
    if has_password {
        return Ok(());
    }

    let secrets_path = match project_kind {
        ProjectKind::EasyToml => project_root.join(".evm-cloud").join("secrets.auto.tfvars"),
        ProjectKind::RawTerraform => project_root.join("secrets.auto.tfvars"),
    };
    if !secrets_path.is_file() {
        return Ok(());
    }

    let raw = fs::read_to_string(&secrets_path).map_err(|source| CliError::Io {
        source,
        path: secrets_path.clone(),
    })?;

    let fallback_password = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .find_map(|line| {
            if !line.starts_with("indexer_clickhouse_password") {
                return None;
            }

            let (_, rhs) = line.split_once('=')?;
            let value_no_comment = rhs.split('#').next().unwrap_or("").trim();
            if value_no_comment.is_empty() {
                return None;
            }

            Some(
                value_no_comment
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            )
        })
        .filter(|password| !password.is_empty());

    let Some(password) = fallback_password else {
        return Ok(());
    };

    if !handoff.data.is_object() {
        handoff.data = Value::Object(serde_json::Map::new());
    }

    let data_obj = handoff
        .data
        .as_object_mut()
        .expect("data must be an object");
    let clickhouse_obj = data_obj
        .entry("clickhouse")
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| CliError::Message("invalid handoff.data.clickhouse shape".to_string()))?;

    clickhouse_obj.insert("password".to_string(), Value::String(password));
    Ok(())
}

struct SshVars {
    key_path: Option<String>,
    user: Option<String>,
    port: Option<String>,
}

/// Read SSH connection vars from secrets.auto.tfvars or terraform.tfvars.
fn resolve_ssh_vars_from_tfvars(
    project_root: &Path,
    project_kind: &ProjectKind,
) -> Result<SshVars> {
    let candidates = match project_kind {
        ProjectKind::EasyToml => vec![
            project_root.join(".evm-cloud").join("secrets.auto.tfvars"),
            project_root.join("secrets.auto.tfvars"),
            project_root.join(".evm-cloud").join("terraform.tfvars"),
        ],
        ProjectKind::RawTerraform => vec![
            project_root.join("secrets.auto.tfvars"),
            project_root.join("terraform.tfvars"),
        ],
    };

    let mut result = SshVars {
        key_path: None,
        user: None,
        port: None,
    };

    for tfvars_path in &candidates {
        if !tfvars_path.is_file() {
            continue;
        }
        let raw = fs::read_to_string(tfvars_path).map_err(|source| CliError::Io {
            source,
            path: tfvars_path.clone(),
        })?;

        for line in raw.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((lhs, rhs)) = line.split_once('=') else {
                continue;
            };
            let var_name = lhs.trim();
            let value = rhs.split('#').next().unwrap_or("").trim();
            let value = value.trim_matches('"').trim_matches('\'');
            if value.is_empty() {
                continue;
            }

            match var_name {
                "ec2_ssh_private_key_path" | "bare_metal_ssh_private_key_path" => {
                    if result.key_path.is_none() {
                        result.key_path = Some(value.to_string());
                    }
                }
                "bare_metal_ssh_user" => {
                    if result.user.is_none() {
                        result.user = Some(value.to_string());
                    }
                }
                "bare_metal_ssh_port" => {
                    if result.port.is_none() {
                        result.port = Some(value.to_string());
                    }
                }
                _ => {}
            }
        }
    }

    Ok(result)
}

fn has_flag_with_value(args: &[String], flag: &str) -> bool {
    args.iter().enumerate().any(|(index, arg)| {
        if arg == flag {
            return args.get(index + 1).is_some();
        }
        arg.starts_with(&format!("{flag}="))
    })
}

fn ensure_config_dir(project_root: &Path) -> Result<PathBuf> {
    let explicit = project_root.join("config");
    if config_dir_ready(&explicit) {
        return Ok(explicit);
    }

    if config_dir_ready(project_root) {
        return Ok(project_root.to_path_buf());
    }

    let generated = project_root.join(".evm-cloud").join("config-bundle");
    fs::create_dir_all(generated.join("abis")).map_err(|source| CliError::Io {
        source,
        path: generated.join("abis"),
    })?;

    copy_required_with_fallback(project_root, &generated, "erpc.yaml")?;
    copy_required_with_fallback(project_root, &generated, "rindexer.yaml")?;

    Ok(generated)
}

fn config_dir_ready(path: &Path) -> bool {
    path.join("erpc.yaml").is_file()
        && path.join("rindexer.yaml").is_file()
        && path.join("abis").is_dir()
}

fn copy_required_with_fallback(
    source_root: &Path,
    destination_root: &Path,
    file: &str,
) -> Result<()> {
    let source = [source_root.join("config").join(file), source_root.join(file)]
        .into_iter()
        .find(|candidate| candidate.is_file());

    let Some(source) = source else {
        return Err(CliError::Message(format!(
            "missing required file `{}` for deploy. Provide --config-dir or create `config/{}`",
            file, file
        )));
    };

    let destination = destination_root.join(file);
    fs::copy(&source, &destination).map_err(|source_err| CliError::Io {
        source: source_err,
        path: destination,
    })?;

    Ok(())
}

/// Generate a `.env` file in the config directory from tfvars secrets.
/// Maps tfvar names to the env var names that docker-compose services expect
/// (matching the secret_payload shape used by the Terraform modules).
fn generate_env_file(
    config_dir: &Path,
    project_root: &Path,
    project_kind: &ProjectKind,
    handoff: &handoff::WorkloadHandoff,
) -> Result<()> {
    let candidates = match project_kind {
        ProjectKind::EasyToml => vec![
            project_root.join(".evm-cloud").join("secrets.auto.tfvars"),
            project_root.join("secrets.auto.tfvars"),
        ],
        ProjectKind::RawTerraform => vec![
            project_root.join("secrets.auto.tfvars"),
        ],
    };

    // Read all tfvars key=value pairs from the first existing secrets file.
    let mut tfvars: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for tfvars_path in &candidates {
        if !tfvars_path.is_file() {
            continue;
        }
        let raw = fs::read_to_string(tfvars_path).map_err(|source| CliError::Io {
            source,
            path: tfvars_path.clone(),
        })?;
        for line in raw.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((lhs, rhs)) = line.split_once('=') else {
                continue;
            };
            let var_name = lhs.trim().to_string();
            let value = rhs.split('#').next().unwrap_or("").trim();
            let value = value.trim_matches('"').trim_matches('\'').to_string();
            if !value.is_empty() {
                tfvars.insert(var_name, value);
            }
        }
        break; // use first found
    }

    // Determine storage backend from handoff.
    let backend = handoff
        .data
        .get("backend")
        .and_then(Value::as_str)
        .unwrap_or("");

    // Build env lines matching the bare_metal module's secret_payload shape.
    let mut env_lines: Vec<String> = Vec::new();

    // RPC_URL: when eRPC is enabled, containers use the internal docker network URL.
    // Otherwise fall back to the RPC endpoint from tfvars.
    if handoff.services.get("rpc_proxy").is_some() {
        env_lines.push("RPC_URL=http://erpc:4000".to_string());
    } else if let Some(url) = tfvars.get("indexer_rpc_url") {
        env_lines.push(format!("RPC_URL={url}"));
    }

    // DATABASE_URL (postgres)
    if backend == "postgres" || tfvars.contains_key("indexer_postgres_url") {
        if let Some(url) = tfvars.get("indexer_postgres_url") {
            env_lines.push(format!("DATABASE_URL={url}"));
        }
    }

    // ClickHouse — apply same defaults as the TF module variables.
    if backend == "clickhouse" || tfvars.contains_key("indexer_clickhouse_url") {
        if let Some(url) = tfvars.get("indexer_clickhouse_url") {
            env_lines.push(format!("CLICKHOUSE_URL={url}"));
        }
        let user = tfvars.get("indexer_clickhouse_user").map(|s| s.as_str()).unwrap_or("default");
        env_lines.push(format!("CLICKHOUSE_USER={user}"));
        if let Some(password) = tfvars.get("indexer_clickhouse_password") {
            env_lines.push(format!("CLICKHOUSE_PASSWORD={password}"));
        }
        let db = tfvars.get("indexer_clickhouse_db").map(|s| s.as_str()).unwrap_or("rindexer");
        env_lines.push(format!("CLICKHOUSE_DB={db}"));
    }

    if env_lines.is_empty() {
        return Ok(());
    }

    let env_path = config_dir.join(".env");
    let content = format!("{}\n", env_lines.join("\n"));
    fs::write(&env_path, &content).map_err(|source| CliError::Io {
        source,
        path: env_path,
    })?;

    Ok(())
}
