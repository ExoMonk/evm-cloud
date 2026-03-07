mod codegen;
mod commands;
mod config;
mod deployer;
mod easy_mode;
mod error;
mod examples;
mod handoff;
mod init_answers;
mod init_scaffold;
mod init_templates;
mod init_wizard;
mod kubeconfig;
mod local;
mod output;
mod post_deploy;
mod preflight;
mod ssh;
mod terraform;
mod tfvars_parser;
mod version_guard;

use clap::{Parser, Subcommand};

/// Terraform module source pointing at the GitHub tag matching this CLI version.
pub(crate) fn module_source() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!("git::https://github.com/ExoMonk/evm-cloud.git?ref={version}")
}

use crate::commands::apply::ApplyArgs;
use crate::commands::bootstrap::BootstrapArgs;
use crate::commands::deploy::DeployArgs;
use crate::commands::destroy::DestroyArgs;
use crate::commands::init::InitArgs;
use crate::commands::kubectl::KubectlArgs;
use crate::commands::logs::LogsArgs;
use crate::commands::status::StatusArgs;
use crate::error::CliError;
use crate::local::LocalCommand;
use crate::output::ColorMode;

#[derive(Parser)]
#[command(
    name = "evm-cloud",
    version,
    about = "Deploy EVM blockchain data infrastructure on AWS"
)]
struct Cli {
    #[arg(long, value_enum, default_value_t = ColorMode::Auto, global = true)]
    color: ColorMode,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init(InitArgs),
    Apply(ApplyArgs),
    /// Create prerequisite cloud resources for the remote state backend
    Bootstrap(BootstrapArgs),
    Deploy(DeployArgs),
    Kubectl(KubectlArgs),
    Status(StatusArgs),
    Logs(LogsArgs),
    Destroy(DestroyArgs),
    /// Manage the local dev stack (kind + Anvil + eRPC + ClickHouse + rindexer)
    #[command(subcommand)]
    Local(LocalCommand),
}

fn main() {
    let cli = Cli::parse();

    if let Err(err) = version_guard::enforce_pinned_version_from_cwd() {
        output::error(&err.to_string(), cli.color);
        std::process::exit(1);
    }

    let result = match cli.command {
        Commands::Init(args) => commands::init::run(args, cli.color),
        Commands::Apply(args) => commands::apply::run(args, cli.color),
        Commands::Bootstrap(args) => commands::bootstrap::run(args, cli.color),
        Commands::Deploy(args) => commands::deploy::run(args, cli.color),
        Commands::Kubectl(args) => commands::kubectl::run(args),
        Commands::Status(args) => commands::status::run(args, cli.color),
        Commands::Logs(args) => commands::logs::run(args, cli.color),
        Commands::Destroy(args) => commands::destroy::run(args, cli.color),
        Commands::Local(cmd) => local::run(cmd, cli.color),
    };

    if let Err(err) = result {
        match err {
            CliError::TerraformFailed { code } => std::process::exit(code),
            CliError::TerraformSignaled { signal } => {
                let exit_code = match signal {
                    Some(2) => 130,
                    Some(15) => 143,
                    _ => 1,
                };
                output::error(
                    &format!(
                        "terraform terminated by signal {:?}. If state is locked, run `terraform force-unlock <LOCK_ID>`.",
                        signal
                    ),
                    cli.color,
                );
                std::process::exit(exit_code);
            }
            other => {
                output::error(&other.to_string(), cli.color);
                std::process::exit(1);
            }
        }
    }
}
