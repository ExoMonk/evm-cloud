use std::collections::BTreeMap;
use std::path::Path;

use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

use crate::config::schema::{ComputeEngine, InfrastructureProvider, IngressMode, WorkloadMode};
use crate::error::{CliError, Result};
use crate::init_answers::{load_from_config, DatabaseProfile, IndexerConfigStrategy, InitAnswers, InitMode};

pub(crate) fn collect_answers(
    config_path: Option<&Path>,
    non_interactive: bool,
    mode_override: Option<InitMode>,
) -> Result<InitAnswers> {
    if non_interactive {
        let path = config_path.ok_or_else(|| CliError::NonInteractiveRequiresConfig)?;
        return load_from_config(path, mode_override);
    }

    if let Some(path) = config_path {
        return load_from_config(path, mode_override);
    }

    interactive_wizard(mode_override)
}

fn interactive_wizard(mode_override: Option<InitMode>) -> Result<InitAnswers> {
    let theme = ColorfulTheme::default();

    let mode = mode_override.unwrap_or_else(|| {
        let options = ["easy", "power"];
        let selection = Select::with_theme(&theme)
            .with_prompt("Integration mode")
            .items(&options)
            .default(0)
            .interact()
            .unwrap_or(0);
        if selection == 0 {
            InitMode::Easy
        } else {
            InitMode::Power
        }
    });

    let project_name: String = Input::with_theme(&theme)
        .with_prompt("Project name")
        .default("evm-cloud-demo".to_string())
        .interact_text()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    let provider_options = [InfrastructureProvider::Aws, InfrastructureProvider::BareMetal];
    let provider_labels: Vec<&str> = provider_options.iter().map(|p| p.as_str()).collect();
    let provider_idx = Select::with_theme(&theme)
        .with_prompt("Infrastructure provider")
        .items(&provider_labels)
        .default(0)
        .interact()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;
    let infrastructure_provider = provider_options[provider_idx];
    let is_aws = infrastructure_provider == InfrastructureProvider::Aws;

    let compute_options: Vec<ComputeEngine> = if is_aws {
        vec![ComputeEngine::Ec2, ComputeEngine::Eks, ComputeEngine::K3s]
    } else {
        vec![ComputeEngine::K3s, ComputeEngine::DockerCompose]
    };
    let compute_labels: Vec<&str> = compute_options.iter().map(|e| e.as_str()).collect();
    let compute_idx = Select::with_theme(&theme)
        .with_prompt("Compute engine")
        .items(&compute_labels)
        .default(0)
        .interact()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;
    let compute_engine = compute_options[compute_idx];

    // k3s/eks always use external deployers; ec2/docker_compose can choose.
    let workload_mode = if matches!(compute_engine, ComputeEngine::Ec2 | ComputeEngine::DockerCompose) {
        let wm_options = [WorkloadMode::Terraform, WorkloadMode::External];
        let wm_labels: Vec<&str> = wm_options.iter().map(|m| m.as_str()).collect();
        let wm_idx = Select::with_theme(&theme)
            .with_prompt("Workload deployment mode (terraform = TF provisioners manage compose; external = CLI deployer)")
            .items(&wm_labels)
            .default(0)
            .interact()
            .map_err(|err| CliError::PromptFailed(err.to_string()))?;
        Some(wm_options[wm_idx])
    } else {
        None // k3s/eks → always "external", inferred downstream
    };

    let region = if is_aws {
        Some(
            Input::with_theme(&theme)
                .with_prompt("AWS region")
                .default("us-east-1".to_string())
                .interact_text()
                .map_err(|err| CliError::PromptFailed(err.to_string()))?,
        )
    } else {
        None
    };

    let db_options = [
        "byodb_clickhouse",
        "byodb_postgres",
        "managed_rds",
        "managed_clickhouse",
    ];
    let db_idx = Select::with_theme(&theme)
        .with_prompt("Database profile")
        .items(&db_options)
        .default(0)
        .interact()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    let database_profile = match db_idx {
        0 => DatabaseProfile::ByodbClickhouse,
        1 => DatabaseProfile::ByodbPostgres,
        2 => DatabaseProfile::ManagedRds,
        _ => DatabaseProfile::ManagedClickhouse,
    };

    let chains = select_chains(&theme)?;

    let mut rpc_endpoints = BTreeMap::new();
    for chain in &chains {
        let endpoint: String = Input::with_theme(&theme)
            .with_prompt(format!("RPC endpoint for {chain}"))
            .interact_text()
            .map_err(|err| CliError::PromptFailed(err.to_string()))?;

        if endpoint.trim().is_empty() {
            return Err(CliError::ConfigValidation {
                field: format!("rpc_endpoints.{chain}"),
                message: "missing endpoint for selected chain".to_string(),
            });
        }

        rpc_endpoints.insert(chain.clone(), endpoint);
    }

    let indexer_options = ["generate starter rindexer.yaml", "use existing rindexer.yaml path"];
    let indexer_idx = Select::with_theme(&theme)
        .with_prompt("Indexer config strategy")
        .items(&indexer_options)
        .default(0)
        .interact()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    let indexer_config = if indexer_idx == 0 {
        IndexerConfigStrategy::Generate
    } else {
        let path: String = Input::with_theme(&theme)
            .with_prompt("Path to existing rindexer.yaml")
            .interact_text()
            .map_err(|err| CliError::PromptFailed(err.to_string()))?;
        IndexerConfigStrategy::Existing(path.into())
    };

    let generate_erpc_config = Confirm::with_theme(&theme)
        .with_prompt("Generate starter erpc.yaml")
        .default(true)
        .interact()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    let needs_instance_type = is_aws && matches!(compute_engine, ComputeEngine::Ec2 | ComputeEngine::Eks | ComputeEngine::K3s);
    let instance_type = if needs_instance_type {
        let default = "t3.small".to_string();

        Some(
            Input::with_theme(&theme)
                .with_prompt("Instance type")
                .default(default)
                .interact_text()
                .map_err(|err| CliError::PromptFailed(err.to_string()))?,
        )
    } else {
        None
    };

    // Ingress / TLS — filter options by compute engine
    let ingress_options = IngressMode::options_for_engine(compute_engine);
    let ingress_labels: Vec<&str> = ingress_options.iter().map(|m| m.as_str()).collect();
    let ingress_idx = Select::with_theme(&theme)
        .with_prompt("Ingress mode (none=no TLS, cloudflare=CF proxy, caddy=Let's Encrypt, ingress_nginx=k8s)")
        .items(&ingress_labels)
        .default(0)
        .interact()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;
    let ingress_mode = ingress_options[ingress_idx];

    let (erpc_hostname, ingress_tls_email) = if !ingress_mode.requires_hostname() {
        (None, None)
    } else {
        let raw_hostname: String = Input::with_theme(&theme)
            .with_prompt("Public hostname for eRPC (e.g. rpc.example.com)")
            .interact_text()
            .map_err(|err| CliError::PromptFailed(err.to_string()))?;
        let hostname = sanitize_hostname(&raw_hostname);

        let email = if ingress_mode.requires_tls_email() {
            Some(
                Input::with_theme(&theme)
                    .with_prompt("Email for Let's Encrypt certificate")
                    .interact_text()
                    .map_err(|err| CliError::PromptFailed(err.to_string()))?,
            )
        } else {
            None
        };

        (Some(hostname), email)
    };

    Ok(InitAnswers {
        mode,
        project_name,
        infrastructure_provider,
        region,
        compute_engine,
        instance_type,
        workload_mode,
        database_profile,
        chains,
        rpc_endpoints,
        indexer_config,
        generate_erpc_config,
        ingress_mode,
        erpc_hostname,
        ingress_tls_email,
    })
}

fn select_chains(theme: &ColorfulTheme) -> Result<Vec<String>> {
    let chain_options = [
        "ethereum",
        "polygon",
        "arbitrum",
        "base",
        "optimism",
        "hyperliquid",
    ];
    let done_label = "Done";

    let mut selected = vec![false; chain_options.len()];
    let mut cursor = 0usize;

    loop {
        let mut items = chain_options
            .iter()
            .enumerate()
            .map(|(index, chain)| {
                let marker = if selected[index] { "x" } else { " " };
                format!("[{marker}] {chain}")
            })
            .collect::<Vec<_>>();
        items.push(done_label.to_string());

        let choice = Select::with_theme(theme)
            .with_prompt("Select chains (Enter toggles; select Done to continue)")
            .items(&items)
            .default(cursor)
            .interact()
            .map_err(|err| CliError::PromptFailed(err.to_string()))?;

        let done_index = items.len() - 1;
        if choice == done_index {
            let chains = chain_options
                .iter()
                .enumerate()
                .filter_map(|(index, chain)| selected[index].then_some((*chain).to_string()))
                .collect::<Vec<_>>();

            if chains.is_empty() {
                return Err(CliError::ConfigValidation {
                    field: "chains".to_string(),
                    message: "at least one chain is required".to_string(),
                });
            }

            return Ok(chains);
        }

        selected[choice] = !selected[choice];
        cursor = done_index;
    }
}

/// Strip protocol prefix and trailing slash from a hostname input.
fn sanitize_hostname(raw: &str) -> String {
    let s = raw.trim();
    let s = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s);
    s.trim_end_matches('/').to_string()
}
