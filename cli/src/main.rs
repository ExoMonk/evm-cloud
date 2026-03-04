mod codegen;
mod commands;
mod config;
mod deployer;
mod easy_mode;
mod error;
mod handoff;
mod init_answers;
mod init_scaffold;
mod init_templates;
mod init_wizard;
mod output;
mod preflight;
mod terraform;
mod version_guard;

use clap::{Parser, Subcommand};

use crate::commands::apply::ApplyArgs;
use crate::commands::deploy::DeployArgs;
use crate::commands::destroy::DestroyArgs;
use crate::commands::init::InitArgs;
use crate::commands::logs::LogsArgs;
use crate::commands::status::StatusArgs;
use crate::error::CliError;
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
    Deploy(DeployArgs),
    Status(StatusArgs),
    Logs(LogsArgs),
    Destroy(DestroyArgs),
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
        Commands::Deploy(args) => commands::deploy::run(args, cli.color),
        Commands::Status(args) => commands::status::run(args, cli.color),
        Commands::Logs(args) => commands::logs::run(args, cli.color),
        Commands::Destroy(args) => commands::destroy::run(args, cli.color),
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
