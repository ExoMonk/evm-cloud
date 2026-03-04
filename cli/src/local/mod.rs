mod cluster;
mod config;
mod deploy;
mod health;
mod manifests;
mod prerequisites;
mod profiles;

use std::path::PathBuf;
use std::time::Instant;

use clap::{Args, Subcommand, ValueEnum};

use crate::error::Result;
use crate::output::{self, ColorMode};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum Profile {
    Default,
    Heavy,
}

#[derive(Subcommand)]
pub(crate) enum LocalCommand {
    /// Start the local dev stack (kind + Anvil + eRPC + ClickHouse + rindexer)
    Up(UpArgs),
    /// Tear down the local dev stack
    Down(DownArgs),
    /// Show status of the local dev stack
    Status(StatusArgs),
    /// Reset: tear down, clean data, and restart
    Reset(ResetArgs),
}

#[derive(Args)]
pub(crate) struct UpArgs {
    /// RPC URL for Anvil to fork from (default: Ethereum mainnet via publicnode)
    #[arg(long)]
    rpc: Option<String>,

    /// Fresh mode: no fork, chain_id=31337
    #[arg(long)]
    fresh: bool,

    /// Persist ClickHouse data across restarts
    #[arg(long)]
    persist: bool,

    /// Force-recreate cluster even if it exists
    #[arg(long)]
    force: bool,

    /// Resource profile
    #[arg(long, value_enum, default_value_t = Profile::Default)]
    profile: Profile,

    /// Path to rindexer.yaml config
    #[arg(long)]
    config: Option<PathBuf>,

    /// Script to run after stack is healthy
    #[arg(long)]
    post_deploy: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct DownArgs;

#[derive(Args)]
pub(crate) struct StatusArgs;

#[derive(Args)]
pub(crate) struct ResetArgs {
    /// RPC URL for Anvil to fork from
    #[arg(long)]
    rpc: Option<String>,

    /// Fresh mode: no fork, chain_id=31337
    #[arg(long)]
    fresh: bool,

    /// Persist ClickHouse data across restarts
    #[arg(long)]
    persist: bool,

    /// Force-recreate cluster
    #[arg(long)]
    force: bool,

    /// Resource profile
    #[arg(long, value_enum, default_value_t = Profile::Default)]
    profile: Profile,

    /// Path to rindexer.yaml config
    #[arg(long)]
    config: Option<PathBuf>,

    /// Script to run after stack is healthy
    #[arg(long)]
    post_deploy: Option<PathBuf>,
}

pub(crate) fn run(cmd: LocalCommand, color: ColorMode) -> Result<()> {
    match cmd {
        LocalCommand::Up(args) => run_up(args, color),
        LocalCommand::Down(_) => run_down(color),
        LocalCommand::Status(_) => run_status(color),
        LocalCommand::Reset(args) => run_reset(args, color),
    }
}

fn run_up(args: UpArgs, color: ColorMode) -> Result<()> {
    let started = Instant::now();
    output::headline("🏰 ⚒️ Starting local dev stack", color);

    // Determine fork URL and chain ID
    let (fork_url, chain_id) = resolve_fork_mode(&args.rpc, args.fresh, color)?;

    // Load or generate rindexer config
    let (rindexer_yaml, abis) = resolve_rindexer_config(
        args.config.as_deref(),
        args.fresh,
        chain_id,
        color,
    )?;

    // Prerequisites
    let needs_port_check = !cluster::cluster_exists()?;
    prerequisites::check_all(args.profile, needs_port_check, color)?;
    output::checkline("Prerequisites verified", color);

    // Kind cluster
    let kind_config = config::generate_kind_config(args.persist)?;
    cluster::ensure_cluster(&kind_config, args.force, color)?;
    output::checkline("Kind cluster ready", color);

    if args.persist {
        let data_dir = config::data_dir();
        std::fs::create_dir_all(&data_dir).ok();
        output::subline(&format!("Persistence enabled — data stored in {data_dir}"), color);
    }

    // Deploy services
    let profile_res = profiles::resources(args.profile);

    output::with_spinner("Deploying ClickHouse", color, || {
        deploy::deploy_clickhouse(args.persist, &profile_res.clickhouse, color)?;
        health::wait_for_http("http://localhost:8123/ping", 60)
    })?;
    output::checkline("ClickHouse ready — localhost:8123", color);

    output::with_spinner("Deploying Anvil", color, || {
        deploy::deploy_anvil(fork_url.as_deref(), &profile_res.anvil, color)
    })?;
    output::checkline("Anvil ready — localhost:8545", color);

    let erpc_values = config::generate_erpc_values(chain_id, &profile_res.erpc);
    output::with_spinner("Deploying eRPC", color, || {
        deploy::deploy_erpc(&erpc_values, color)
    })?;
    output::checkline("eRPC ready — localhost:4000", color);

    let indexer_values =
        config::generate_indexer_values(&rindexer_yaml, &abis, chain_id, &profile_res.indexer);
    output::with_spinner("Deploying rindexer", color, || {
        deploy::deploy_rindexer(&indexer_values, color)
    })?;
    output::checkline("🦀rindexer ready — localhost:18080", color);

    // Health checks
    let mut all_healthy = true;
    if health::wait_for_anvil(30).is_err() {
        output::warn("Anvil health check timed out", color);
        all_healthy = false;
    }
    if health::wait_for_http("http://localhost:4000", 30).is_err() {
        output::warn("eRPC health check timed out", color);
        all_healthy = false;
    }
    if health::wait_for_http("http://localhost:8123/ping", 10).is_err() {
        output::warn("ClickHouse health check timed out", color);
        all_healthy = false;
    }
    if all_healthy {
        output::checkline("All health checks passed", color);
    }

    // Post-deploy hook
    if let Some(script) = &args.post_deploy {
        run_post_deploy(script, chain_id, color)?;
        output::checkline("Post-deploy script completed", color);
    }

    // Summary
    output::headline(
        &format!(
            "🏰 ✅ Local stack ready — {}",
            output::duration_human(started.elapsed())
        ),
        color,
    );
    print_summary(fork_url.as_deref(), chain_id, color);

    Ok(())
}

fn run_down(color: ColorMode) -> Result<()> {
    let started = Instant::now();
    output::headline("🏰 ⚒️ Tearing down local stack", color);
    cluster::delete_cluster(color)?;
    output::headline(
        &format!(
            "🏰 ✅ Local stack removed — {}",
            output::duration_human(started.elapsed())
        ),
        color,
    );
    Ok(())
}

fn run_status(color: ColorMode) -> Result<()> {
    if !cluster::cluster_exists()? {
        output::error(&format!("No {} cluster found. Run `evm-cloud local up` first.", cluster::name()), color);
        return Ok(());
    }

    output::headline(&format!("🏰 evm-cloud local stack — {}", cluster::name()), color);

    let anvil_ok = health::wait_for_anvil(4).is_ok();
    let erpc_ok = health::wait_for_http("http://localhost:4000", 4).is_ok();
    let ch_ok = health::wait_for_http("http://localhost:8123/ping", 4).is_ok();
    let idx_ok = health::wait_for_http("http://localhost:18080/health", 4).is_ok();

    eprintln!();
    print_health_line("Anvil", anvil_ok, "http://localhost:8545", color);
    print_health_line("eRPC", erpc_ok, "http://localhost:4000", color);
    print_health_line("ClickHouse", ch_ok, "http://localhost:8123", color);
    print_health_line("rindexer", idx_ok, "http://localhost:18080", color);

    if anvil_ok {
        if let Ok(cid) = health::probe_chain_id("http://localhost:8545") {
            if cid == 31337 {
                output::subline("Chain ID: 31337 (Anvil fresh)", color);
            } else {
                output::subline(&format!("Chain ID: {cid} (fork mode)"), color);
            }
        }
    }

    output::subline(
        "forge create src/MyContract.sol:MyContract --rpc-url http://localhost:8545",
        color,
    );

    Ok(())
}

fn run_reset(args: ResetArgs, color: ColorMode) -> Result<()> {
    let started = Instant::now();
    output::headline("🏰 ⚒️ Resetting local stack", color);

    cluster::delete_cluster(color)?;
    output::checkline("Cluster removed", color);

    let data_dir = config::data_dir();
    if std::path::Path::new(&data_dir).exists() {
        std::fs::remove_dir_all(&data_dir).ok();
        output::checkline("Persistent data cleared", color);
    }

    output::headline(
        &format!(
            "🏰 Restarting — {}",
            output::duration_human(started.elapsed())
        ),
        color,
    );

    run_up(
        UpArgs {
            rpc: args.rpc,
            fresh: args.fresh,
            persist: args.persist,
            force: true,
            profile: args.profile,
            config: args.config,
            post_deploy: args.post_deploy,
        },
        color,
    )
}

fn resolve_fork_mode(
    rpc: &Option<String>,
    fresh: bool,
    color: ColorMode,
) -> Result<(Option<String>, u64)> {
    if fresh {
        output::subline("Fresh mode — no fork, chain_id=31337", color);
        return Ok((None, 31337));
    }

    let fork_url = rpc
        .clone()
        .unwrap_or_else(|| config::DEFAULT_FORK_RPC.to_string());

    output::subline(&format!("Probing chain ID from {fork_url}..."), color);
    let chain_id = health::probe_chain_id(&fork_url)?;
    output::subline(&format!("Fork mode — chain_id={chain_id}"), color);

    Ok((Some(fork_url), chain_id))
}

fn resolve_rindexer_config(
    explicit: Option<&std::path::Path>,
    fresh: bool,
    chain_id: u64,
    color: ColorMode,
) -> Result<(String, Vec<(String, String)>)> {
    if let Some(path) = config::resolve_config_path(explicit) {
        output::subline(&format!("Using rindexer config: {}", path.display()), color);
        return config::load_user_rindexer_config(&path);
    }

    // No config found — generate starter
    let starter = if fresh {
        config::STARTER_RINDEXER_YAML_FRESH
    } else {
        &config::STARTER_RINDEXER_YAML_FORK
            .replace("chain_id: 1", &format!("chain_id: {chain_id}"))
            .replace("evm/{}", &format!("evm/{chain_id}"))
    };

    output::warn("No rindexer.yaml found — using starter config with placeholder contract", color);
    let abis = vec![("placeholder.json".to_string(), config::PLACEHOLDER_ABI.to_string())];
    Ok((starter.to_string(), abis))
}

fn run_post_deploy(script: &std::path::Path, chain_id: u64, color: ColorMode) -> Result<()> {
    output::subline(&format!("Running post-deploy: {}", script.display()), color);

    let status = std::process::Command::new(script)
        .env("ANVIL_RPC_URL", "http://localhost:8545")
        .env("ERPC_URL", "http://localhost:4000")
        .env("CLICKHOUSE_URL", "http://localhost:8123")
        .env("CHAIN_ID", chain_id.to_string())
        .status()
        .map_err(|e| crate::error::CliError::ToolFailed {
            tool: script.display().to_string(),
            details: e.to_string(),
        })?;

    if !status.success() {
        return Err(crate::error::CliError::ToolFailed {
            tool: "post-deploy".into(),
            details: "script exited with non-zero status".into(),
        });
    }

    Ok(())
}

fn print_summary(fork_url: Option<&str>, chain_id: u64, color: ColorMode) {
    eprintln!();
    eprintln!("     👉🏻 Anvil         http://localhost:8545");
    eprintln!("     👉🏻 eRPC          http://localhost:4000");
    eprintln!("     👉🏻 ClickHouse    http://localhost:8123");
    eprintln!("     👉🏻 rindexer      http://localhost:18080");
    eprintln!();
    if let Some(url) = fork_url {
        output::subline(&format!("Chain ID: {chain_id} (fork from {url})"), color);
    } else {
        output::subline(&format!("Chain ID: {chain_id} (Anvil fresh)"), color);
    }
    eprintln!("     👉🏻 Deploy:    forge create src/MyContract.sol:MyContract --rpc-url http://localhost:8545");
    eprintln!("     👉🏻 Query CH:  curl 'http://localhost:8123/?user=default&password=local-dev' -d 'SHOW TABLES'");
    eprintln!("     👉🏻 Status:    evm-cloud local status");
    eprintln!("     👉🏻 Tear down: evm-cloud local down");
}

fn print_health_line(name: &str, ok: bool, url: &str, _color: ColorMode) {
    if ok {
        eprintln!("     👉🏻 {name:<14}{url}");
    } else {
        eprintln!("     ❌ {name:<14}{url} — DOWN");
    }
}
