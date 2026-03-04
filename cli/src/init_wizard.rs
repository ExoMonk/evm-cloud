use std::collections::BTreeMap;
use std::path::Path;

use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

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
        .map_err(|err| CliError::Message(err.to_string()))?;

    let region: String = Input::with_theme(&theme)
        .with_prompt("AWS region")
        .default("us-east-1".to_string())
        .interact_text()
        .map_err(|err| CliError::Message(err.to_string()))?;

    let compute_options = ["ec2", "k3s", "eks"];
    let compute_idx = Select::with_theme(&theme)
        .with_prompt("Compute engine")
        .items(&compute_options)
        .default(0)
        .interact()
        .map_err(|err| CliError::Message(err.to_string()))?;
    let compute_engine = compute_options[compute_idx].to_string();

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
        .map_err(|err| CliError::Message(err.to_string()))?;

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
            .map_err(|err| CliError::Message(err.to_string()))?;

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
        .map_err(|err| CliError::Message(err.to_string()))?;

    let indexer_config = if indexer_idx == 0 {
        IndexerConfigStrategy::Generate
    } else {
        let path: String = Input::with_theme(&theme)
            .with_prompt("Path to existing rindexer.yaml")
            .interact_text()
            .map_err(|err| CliError::Message(err.to_string()))?;
        IndexerConfigStrategy::Existing(path.into())
    };

    let generate_erpc_config = Confirm::with_theme(&theme)
        .with_prompt("Generate starter erpc.yaml")
        .default(true)
        .interact()
        .map_err(|err| CliError::Message(err.to_string()))?;

    let instance_type_default = match mode {
        InitMode::Easy => "t3.micro",
        InitMode::Power => "t3.small",
    }
    .to_string();

    let instance_type: String = Input::with_theme(&theme)
        .with_prompt("EC2 instance type")
        .default(instance_type_default)
        .interact_text()
        .map_err(|err| CliError::Message(err.to_string()))?;

    Ok(InitAnswers {
        mode,
        project_name,
        region,
        compute_engine,
        instance_type,
        database_profile,
        chains,
        rpc_endpoints,
        indexer_config,
        generate_erpc_config,
    })
}

fn select_chains(theme: &ColorfulTheme) -> Result<Vec<String>> {
    let chain_options = ["ethereum", "polygon", "arbitrum", "base", "optimism"];
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
            .map_err(|err| CliError::Message(err.to_string()))?;

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
