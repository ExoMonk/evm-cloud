mod cluster;
mod config;
mod deploy;
mod health;
mod manifests;
mod prerequisites;
mod profiles;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use clap::{Args, Subcommand, ValueEnum};

use crate::error::{CliError, Result};
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
    /// Stream logs from a local stack service
    Logs(LocalLogsArgs),
}

#[derive(Args)]
pub(crate) struct UpArgs {
    /// RPC URL for Anvil to fork from (default: Ethereum mainnet via publicnode).
    /// With --mainnet: used as the direct RPC upstream (no Anvil).
    #[arg(long)]
    rpc: Option<String>,

    /// Mainnet mode: skip Anvil, point eRPC directly at --rpc URL
    #[arg(long)]
    mainnet: bool,

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

    /// Path to config directory containing rindexer.yaml + abis/, or direct path to rindexer.yaml
    #[arg(long)]
    config_dir: Option<PathBuf>,

    /// Script to run after stack is healthy
    #[arg(long)]
    post_deploy: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct DownArgs;

#[derive(Args)]
pub(crate) struct StatusArgs;

#[derive(Args)]
pub(crate) struct LocalLogsArgs {
    /// Service to stream logs from (rindexer, erpc, clickhouse)
    #[arg(default_value = "rindexer")]
    service: String,

    /// Follow log output (stream continuously)
    #[arg(short = 'f', long)]
    follow: bool,

    /// Number of historical lines to show
    #[arg(long, default_value_t = 100)]
    tail: u32,
}

#[derive(Args)]
pub(crate) struct ResetArgs {
    /// RPC URL for Anvil to fork from.
    /// With --mainnet: used as the direct RPC upstream (no Anvil).
    #[arg(long)]
    rpc: Option<String>,

    /// Mainnet mode: skip Anvil, point eRPC directly at --rpc URL
    #[arg(long)]
    mainnet: bool,

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

    /// Path to config directory containing rindexer.yaml + abis/, or direct path to rindexer.yaml
    #[arg(long)]
    config_dir: Option<PathBuf>,

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
        LocalCommand::Logs(args) => run_logs(args, color),
    }
}

fn run_up(args: UpArgs, color: ColorMode) -> Result<()> {
    let started = Instant::now();
    let mainnet = args.mainnet;

    // Validate flag combos
    if mainnet && args.fresh {
        return Err(CliError::ToolFailed {
            tool: "local".into(),
            details: "--mainnet and --fresh are mutually exclusive".into(),
        });
    }

    output::headline("🏰 ⚒️ Starting local dev stack", color);

    // Load user-provided eRPC config if present (config/erpc.yaml).
    let user_erpc: Option<String> = config::resolve_erpc_config_path(args.config_dir.as_deref())
        .map(|p| config::load_user_erpc_config(&p))
        .transpose()?;

    // Determine RPC mode and chain ID.
    // Optimisation: if mainnet + no --rpc + config/erpc.yaml with parseable chainId,
    // skip the DEFAULT_FORK_RPC network probe entirely.
    let (fork_url, chain_id) = if mainnet && args.rpc.is_none() {
        if let Some(cid) = user_erpc.as_deref().and_then(config::parse_chain_id_from_erpc) {
            output::subline(
                &format!("Mainnet mode — chain_id={cid} (from config/erpc.yaml), no Anvil"),
                color,
            );
            (None, cid)
        } else {
            resolve_fork_mode(&args.rpc, args.fresh, mainnet, color)?
        }
    } else {
        resolve_fork_mode(&args.rpc, args.fresh, mainnet, color)?
    };

    // Load or generate rindexer config
    let (rindexer_yaml, abis) =
        resolve_rindexer_config(args.config_dir.as_deref(), args.fresh, chain_id, color)?;

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
        output::subline(
            &format!("Persistence enabled — data stored in {data_dir}"),
            color,
        );
    }

    // Deploy services
    let profile_res = profiles::resources(args.profile);

    output::with_spinner("Deploying ClickHouse", color, || {
        deploy::deploy_clickhouse(args.persist, &profile_res.clickhouse, color)?;
        health::wait_for_http("http://localhost:8123/ping", 60)
    })?;
    output::checkline("ClickHouse ready — localhost:8123", color);

    if mainnet {
        // Mainnet mode: no Anvil — eRPC points directly at the user's RPC (or custom config).
        let erpc_values = if let Some(ref content) = user_erpc {
            output::subline("Using custom eRPC config: config/erpc.yaml", color);
            config::generate_erpc_values_from_file(content, &profile_res.erpc)
        } else {
            let rpc_url = fork_url.as_deref().expect("mainnet mode requires --rpc");
            config::generate_erpc_values_mainnet(chain_id, rpc_url, &profile_res.erpc)
        };
        output::with_spinner("Deploying eRPC", color, || {
            deploy::deploy_erpc(&erpc_values, color)
        })?;
        output::checkline("eRPC ready — localhost:4000", color);
    } else {
        output::with_spinner("Deploying Anvil", color, || {
            deploy::deploy_anvil(fork_url.as_deref(), &profile_res.anvil, color)
        })?;
        output::checkline("Anvil ready — localhost:8545", color);

        let erpc_values = config::generate_erpc_values(chain_id, &profile_res.erpc);
        output::with_spinner("Deploying eRPC", color, || {
            deploy::deploy_erpc(&erpc_values, color)
        })?;
        output::checkline("eRPC ready — localhost:4000", color);
    }

    let indexer_values =
        config::generate_indexer_values(&rindexer_yaml, &abis, chain_id, &profile_res.indexer, user_erpc.is_some());
    output::with_spinner("Deploying rindexer", color, || {
        deploy::deploy_rindexer(&indexer_values, color)
    })?;
    output::checkline("🦀rindexer ready — localhost:18080", color);

    // Health checks
    let mut all_healthy = true;
    if !mainnet && health::wait_for_anvil(30).is_err() {
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
    print_summary(fork_url.as_deref(), chain_id, mainnet, color);

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
        output::error(
            &format!(
                "No {} cluster found. Run `evm-cloud local up` first.",
                cluster::name()
            ),
            color,
        );
        return Ok(());
    }

    output::headline(
        &format!(
            " evm-cloud local status — {} (kind · local)",
            cluster::name()
        ),
        color,
    );

    // Detect which services are deployed via helm releases.
    let anvil_deployed = helm_release_exists("local-anvil");

    // Probe each service (4s timeout each).
    struct LocalService {
        name: &'static str,
        healthy: bool,
        detail: String,
    }

    let mut services: Vec<LocalService> = Vec::new();

    if anvil_deployed {
        let ok = health::wait_for_anvil(4).is_ok();
        let detail = if ok {
            health::probe_chain_id("http://localhost:8545")
                .ok()
                .map(|cid| {
                    if cid == 31337 {
                        format!("http://localhost:8545 · chain_id={cid} (fresh)")
                    } else {
                        format!("http://localhost:8545 · chain_id={cid} (fork)")
                    }
                })
                .unwrap_or_else(|| "http://localhost:8545".to_string())
        } else {
            "http://localhost:8545".to_string()
        };
        services.push(LocalService {
            name: "Anvil",
            healthy: ok,
            detail,
        });
    }

    let erpc_ok = health::wait_for_http("http://localhost:4000", 4).is_ok();
    services.push(LocalService {
        name: "eRPC",
        healthy: erpc_ok,
        detail: "http://localhost:4000".to_string(),
    });

    let ch_ok = health::wait_for_http("http://localhost:8123/ping", 4).is_ok();
    services.push(LocalService {
        name: "ClickHouse",
        healthy: ch_ok,
        detail: "http://localhost:8123".to_string(),
    });

    let idx_ok = health::wait_for_http("http://localhost:18080/health", 4).is_ok();
    let idx_detail = {
        let replicas = kubectl_local_ready("local-indexer");
        match replicas {
            Some((ready, total)) => format!("http://localhost:18080 · {ready}/{total} replicas"),
            None => "http://localhost:18080".to_string(),
        }
    };
    services.push(LocalService {
        name: "rindexer",
        healthy: idx_ok,
        detail: idx_detail,
    });

    // Render service table.
    output::section_line("🏰 Services", color);
    let mut healthy_count = 0usize;
    for svc in &services {
        let (icon, status_text) = if svc.healthy {
            healthy_count += 1;
            ("🟢", "UP")
        } else {
            ("🔴", "DOWN")
        };
        output::status_line(svc.name, icon, status_text, &svc.detail, color);
    }

    // Connection block.
    output::section_line("🔌 Connection", color);
    output::subline(
        "kubectl   kubectl --context kind-evm-cloud-local get pods -n default",
        color,
    );
    output::subline("logs      evm-cloud local logs rindexer", color);
    if anvil_deployed {
        output::subline(
            "deploy    forge create src/MyContract.sol:MyContract --rpc-url http://localhost:8545",
            color,
        );
    }

    // Overall verdict.
    eprintln!();
    let total = services.len();
    if healthy_count == total {
        output::checkline(&format!("All {total} services healthy"), color);
    } else {
        let down = total - healthy_count;
        output::error(
            &format!("{down} service{} down", if down == 1 { "" } else { "s" }),
            color,
        );
    }

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
            mainnet: args.mainnet,
            fresh: args.fresh,
            persist: args.persist,
            force: true,
            profile: args.profile,
            config_dir: args.config_dir,
            post_deploy: args.post_deploy,
        },
        color,
    )
}

fn resolve_fork_mode(
    rpc: &Option<String>,
    fresh: bool,
    mainnet: bool,
    color: ColorMode,
) -> Result<(Option<String>, u64)> {
    if fresh {
        output::subline("Fresh mode — no fork, chain_id=31337", color);
        return Ok((None, 31337));
    }

    let rpc_url = rpc
        .clone()
        .unwrap_or_else(|| config::DEFAULT_FORK_RPC.to_string());

    output::subline(&format!("Probing chain ID from {rpc_url}..."), color);
    let chain_id = health::probe_chain_id(&rpc_url)?;

    if mainnet {
        output::subline(
            &format!("Mainnet mode — chain_id={chain_id}, no Anvil"),
            color,
        );
    } else {
        output::subline(&format!("Fork mode — chain_id={chain_id}"), color);
    }

    Ok((Some(rpc_url), chain_id))
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

    if let Some(path) = explicit {
        return Err(CliError::RindexerConfigNotFound {
            path: path.display().to_string(),
        });
    }

    let default_path = config::ensure_default_config_bundle(fresh, chain_id)?;
    output::warn(
        &format!(
            "No config/rindexer.yaml found — generated default local config at {}",
            default_path.display()
        ),
        color,
    );
    output::subline(
        "Default starter tracks USDC using config/abis/ERC20.json",
        color,
    );
    output::subline(
        &format!("Using rindexer config: {}", default_path.display()),
        color,
    );
    config::load_user_rindexer_config(&default_path)
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

fn print_summary(fork_url: Option<&str>, chain_id: u64, mainnet: bool, color: ColorMode) {
    if !mainnet {
        eprintln!("     👉🏻 Anvil         http://localhost:8545");
    }
    eprintln!("     👉🏻 eRPC          http://localhost:4000");
    eprintln!("     👉🏻 ClickHouse    http://localhost:8123");
    eprintln!("     👉🏻 rindexer      http://localhost:18080");
    if mainnet {
        if let Some(url) = fork_url {
            output::subline(&format!("Chain ID: {chain_id} (mainnet via {url})"), color);
        }
    } else if let Some(url) = fork_url {
        output::subline(&format!("Chain ID: {chain_id} (fork from {url})"), color);
    } else {
        output::subline(&format!("Chain ID: {chain_id} (Anvil fresh)"), color);
    }
    if !mainnet {
        eprintln!("     👉🏻 Deploy:    forge create src/MyContract.sol:MyContract --rpc-url http://localhost:8545");
    }
    eprintln!("     👉🏻 Query CH:  curl 'http://localhost:8123/?user=default&password=local-dev' -d 'SHOW TABLES'");
    eprintln!("     👉🏻 Status:    evm-cloud local status");
    eprintln!("     👉🏻 Tear down: evm-cloud local down");
}

// ── local logs ──────────────────────────────────────────────────────────────

const LOCAL_VALID_SERVICES: &[&str] = &["rindexer", "erpc", "clickhouse"];

fn run_logs(args: LocalLogsArgs, color: ColorMode) -> Result<()> {
    let service = args.service.to_lowercase();
    if !LOCAL_VALID_SERVICES.contains(&service.as_str()) {
        return Err(CliError::InvalidArg {
            arg: service,
            details: format!(
                "unknown service. Valid: {}",
                LOCAL_VALID_SERVICES.join(", ")
            ),
        });
    }

    if !cluster::cluster_exists()? {
        output::error(
            &format!(
                "No {} cluster found. Run `evm-cloud local up` first.",
                cluster::name()
            ),
            color,
        );
        return Ok(());
    }

    let label = match service.as_str() {
        "rindexer" => "app.kubernetes.io/name=indexer",
        "erpc" => "app.kubernetes.io/name=rpc-proxy",
        "clickhouse" => "app=clickhouse",
        _ => unreachable!(),
    };

    output::info(&format!("Tailing {service} logs..."), color);

    let mut cmd = Command::new("kubectl");
    cmd.args([
        "logs",
        "-l",
        label,
        "-n",
        "default",
        "--context",
        "kind-evm-cloud-local",
        "--tail",
        &args.tail.to_string(),
        "--all-containers=true",
        "--prefix",
        "--max-log-requests=20",
    ]);
    if args.follow {
        cmd.arg("-f");
    }
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status().map_err(|err| CliError::CommandSpawn {
        command: "kubectl".to_string(),
        source: err,
    })?;

    crate::error::tool_exit_status(status, "kubectl")
}

// ── Local-stack helpers ──────────────────────────────────────────────────────

/// Returns true if the given Helm release exists in the local kind cluster.
fn helm_release_exists(release: &str) -> bool {
    let Ok(output) = Command::new("helm")
        .args([
            "list",
            "-n",
            "default",
            "--kube-context",
            "kind-evm-cloud-local",
            "-q",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    else {
        return false;
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().any(|l| l.trim() == release)
}

/// Returns (readyReplicas, totalReplicas) for the Helm release's deployment in the local cluster.
/// Best-effort: returns None on any error.
fn kubectl_local_ready(helm_release: &str) -> Option<(u64, u64)> {
    let label = format!("app.kubernetes.io/instance={helm_release}");
    let output = Command::new("kubectl")
        .args([
            "get",
            "deployments",
            "-n",
            "default",
            "--context",
            "kind-evm-cloud-local",
            "-l",
            &label,
            "-o",
            "json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let item = json.get("items")?.as_array()?.first()?;
    let total = item
        .pointer("/spec/replicas")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    let ready = item
        .pointer("/status/readyReplicas")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some((ready, total))
}
