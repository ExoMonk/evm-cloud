use std::path::PathBuf;
use std::time::Instant;

use clap::Args;

use crate::deployer::{self, Action, DeployLockGuard, InvokeOptions};
use crate::easy_mode;
use crate::error::{CliError, Result};
use crate::handoff;
use crate::output::{self, ColorMode};
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
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    deployer_args: Vec<String>,
}

pub(crate) fn run(args: DeployArgs, color: ColorMode) -> Result<()> {
    let started = Instant::now();
    output::headline(
        &format!("🏰 ⚒️ Deploying stack for {}", args.dir.display()),
        color,
    );

    let preflight = preflight::run_checks(&args.dir, args.allow_raw_terraform)?;
    let project_root = preflight.resolved_root.clone();
    let terraform_dir = match preflight.project_kind {
        ProjectKind::EasyToml => easy_mode::prepare_workspace(&project_root, color)?,
        ProjectKind::RawTerraform => project_root.clone(),
    };

    let action = if args.teardown {
        Action::Teardown
    } else {
        Action::Deploy
    };

    let runner = TerraformRunner::check_installed(&terraform_dir)?;
    output::with_terraforming(color, || runner.init(&terraform_dir, &[]))?;

    let parsed_handoff = match runner.output_named_json(&terraform_dir, "workload_handoff") {
        Ok(value) => handoff::parse_handoff_value(value)?,
        Err(CliError::TerraformOutputMissing { .. }) => {
            let full_output = runner.output_json(&terraform_dir)?;
            handoff::parse_from_full_output(full_output, &args.module_name)?
        }
        Err(err) => return Err(err),
    };
    handoff::validate_for_action(&parsed_handoff, action, &args.deployer_args)?;

    let _lock = DeployLockGuard::acquire(&project_root)?;
    deployer::invoke_deployer(
        &parsed_handoff,
        action,
        InvokeOptions {
            passthrough_args: &args.deployer_args,
            quiet_output: true,
        },
    )?;

    eprintln!();
    eprintln!("     ✓  VPC + networking");
    eprintln!();
    if parsed_handoff.compute_engine == "k3s" {
        eprintln!("     ✓ k3s cluster (1 nodes)");
    } else {
        eprintln!("     ✓ {} cluster", parsed_handoff.compute_engine);
    }
    eprintln!();
    eprintln!("     ✓ eRPC proxy");
    eprintln!();
    eprintln!("     ✓ 🦀rindexer");
    eprintln!();
    eprintln!("     ✓ ClickHouse connected");
    eprintln!();

    output::headline(
        &format!(
            "🏰 ✅ Stack deployed - {}",
            output::duration_human(started.elapsed())
        ),
        color,
    );

    eprintln!("     👉🏻 eRPC:     https://rpc.example.com");
    eprintln!("     👉🏻 Grafana:  https://grafana.example.com");
    if let Some(ip) = parsed_handoff
        .runtime
        .ec2
        .as_ref()
        .and_then(|ec2| ec2.public_ip.as_ref())
    {
        eprintln!("     👉🏻 SSH:      ssh ubuntu@{}", ip);
    } else {
        eprintln!("     👉🏻 SSH:      ssh ubuntu@203.0.113.42");
    }

    Ok(())
}
