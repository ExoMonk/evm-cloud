use std::path::PathBuf;
use std::time::Instant;
use std::{fs, path::Path};

use clap::Args;
use serde_json::Value;

use crate::deployer::{self, Action, DeployLockGuard, InvokeOptions};
use crate::easy_mode;
use crate::error::{CliError, Result};
use crate::handoff;
use crate::output::{self, ColorMode};
use crate::post_deploy;
use crate::preflight::{self, ProjectKind};
use crate::terraform::TerraformRunner;

#[derive(Args)]
pub(crate) struct DeployArgs {
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,
    #[arg(long, default_value = "evm_cloud")]
    module_name: String,
    #[arg(long)]
    teardown: bool,
    #[arg(long)]
    allow_raw_terraform: bool,
    #[arg(long)]
    json: bool,
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    deployer_args: Vec<String>,
}

pub(crate) fn run(args: DeployArgs, color: ColorMode) -> Result<()> {
    let started = Instant::now();
    let _render_color = if args.json { ColorMode::Never } else { color };
    if !args.json {
        output::headline(
            &format!("🏰 ⚒️ Deploying stack for {}", args.dir.display()),
            color,
        );
    }

    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    let project_root = preflight.resolved_root.clone();
    let terraform_dir = match preflight.project_kind {
        ProjectKind::EasyToml => easy_mode::prepare_workspace_quiet(&project_root)?,
        ProjectKind::RawTerraform => project_root.clone(),
    };

    let action = if args.teardown {
        Action::Teardown
    } else {
        Action::Deploy
    };

    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    runner.init_if_needed(&terraform_dir, &[])?;

    let mut parsed_handoff = match runner.output_named_json(&terraform_dir, "workload_handoff") {
        Ok(value) => handoff::parse_handoff_value(value)?,
        Err(CliError::TerraformOutputMissing { .. }) => {
            let full_output = runner.output_json(&terraform_dir)?;
            handoff::parse_from_full_output(full_output, &args.module_name)?
        }
        Err(err) => return Err(err),
    };

    backfill_inline_clickhouse_password(&mut parsed_handoff, &project_root, &preflight.project_kind)?;
    handoff::validate_for_action(&parsed_handoff, action, &args.deployer_args)?;

    let mut effective_deployer_args = args.deployer_args.clone();
    if matches!(action, Action::Deploy)
        && parsed_handoff.compute_engine == "k3s"
        && !has_flag_with_value(&effective_deployer_args, "--config-dir")
    {
        let config_dir = ensure_k3s_config_dir(&project_root)?;
        effective_deployer_args.push("--config-dir".to_string());
        effective_deployer_args.push(config_dir.display().to_string());
    }

    let _lock = DeployLockGuard::acquire(&project_root)?;

    deployer::invoke_deployer(
        &parsed_handoff,
        action,
        InvokeOptions {
            passthrough_args: &effective_deployer_args,
            quiet_output: true,
            color: if args.json { ColorMode::Never } else { color },
            compute_engine: parsed_handoff.compute_engine.clone(),
        },
    )?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&parsed_handoff).map_err(CliError::OutputParseError)?
        );
        return Ok(());
    }

    output::headline(
        &format!(
            "🏰 ✅ Stack deployed - {}",
            output::duration_human(started.elapsed())
        ),
        color,
    );

    post_deploy::print_summary(&parsed_handoff, color);

    Ok(())
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

            Some(value_no_comment.trim_matches('"').trim_matches('\'').to_string())
        })
        .filter(|password| !password.is_empty());

    let Some(password) = fallback_password else {
        return Ok(());
    };

    if !handoff.data.is_object() {
        handoff.data = Value::Object(serde_json::Map::new());
    }

    let data_obj = handoff.data.as_object_mut().expect("data must be an object");
    let clickhouse_obj = data_obj
        .entry("clickhouse")
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| CliError::Message("invalid handoff.data.clickhouse shape".to_string()))?;

    clickhouse_obj.insert("password".to_string(), Value::String(password));
    Ok(())
}

fn has_flag_with_value(args: &[String], flag: &str) -> bool {
    args.iter().enumerate().any(|(index, arg)| {
        if arg == flag {
            return args.get(index + 1).is_some();
        }
        arg.starts_with(&format!("{flag}="))
    })
}

fn ensure_k3s_config_dir(project_root: &Path) -> Result<PathBuf> {
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
    path.join("erpc.yaml").is_file() && path.join("rindexer.yaml").is_file() && path.join("abis").is_dir()
}

fn copy_required_with_fallback(source_root: &Path, destination_root: &Path, file: &str) -> Result<()> {
    let source = [source_root.join("config").join(file), source_root.join(file)]
        .into_iter()
        .find(|candidate| candidate.is_file());

    let Some(source) = source else {
        return Err(CliError::Message(format!(
            "missing required file `{}` for k3s deploy. Provide --config-dir or create `config/{}`",
            file,
            file
        )));
    };

    let destination = destination_root.join(file);
    fs::copy(&source, &destination).map_err(|source_err| CliError::Io {
        source: source_err,
        path: destination,
    })?;

    Ok(())
}

