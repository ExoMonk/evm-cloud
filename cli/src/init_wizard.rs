use std::collections::BTreeMap;
use std::path::Path;

use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

use crate::config::schema::{
    ComputeEngine, InfrastructureProvider, IngressMode, StateConfig, WorkloadMode,
};
use crate::error::{CliError, Result};
use crate::init_answers::{
    load_from_config, DatabaseProfile, IndexerConfigStrategy, InitAnswers, InitMode,
};

/// Pre-detected template context to skip redundant wizard questions.
pub(crate) struct TemplateContext {
    pub(crate) chains: Vec<String>,
    pub(crate) rpc_endpoints: BTreeMap<String, String>,
}

pub(crate) fn collect_answers(
    config_path: Option<&Path>,
    non_interactive: bool,
    mode_override: Option<InitMode>,
) -> Result<InitAnswers> {
    collect_answers_with_context(config_path, non_interactive, mode_override, None)
}

pub(crate) fn collect_answers_with_context(
    config_path: Option<&Path>,
    non_interactive: bool,
    mode_override: Option<InitMode>,
    template_ctx: Option<TemplateContext>,
) -> Result<InitAnswers> {
    if non_interactive {
        let path = config_path.ok_or(CliError::NonInteractiveRequiresConfig)?;
        return load_from_config(path, mode_override);
    }

    if let Some(path) = config_path {
        return load_from_config(path, mode_override);
    }

    interactive_wizard(mode_override, template_ctx)
}

fn interactive_wizard(
    mode_override: Option<InitMode>,
    template_ctx: Option<TemplateContext>,
) -> Result<InitAnswers> {
    let theme = ColorfulTheme::default();

    let mode = mode_override.unwrap_or_else(|| {
        let options = [InitMode::Easy.label(), InitMode::Power.label()];
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

    let provider_options = [
        InfrastructureProvider::Aws,
        InfrastructureProvider::BareMetal,
    ];
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
    let workload_mode = if matches!(
        compute_engine,
        ComputeEngine::Ec2 | ComputeEngine::DockerCompose
    ) {
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

    // If template context is available, skip chain/RPC/indexer questions
    let (chains, rpc_endpoints, indexer_config, generate_erpc_config) =
        if let Some(ctx) = template_ctx {
            (
                ctx.chains,
                ctx.rpc_endpoints,
                IndexerConfigStrategy::Existing("config/rindexer.yaml".into()),
                false, // erpc.yaml already generated by templates apply
            )
        } else {
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

            let indexer_options = [
                "generate starter rindexer.yaml",
                "use existing rindexer.yaml path",
            ];
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

            (chains, rpc_endpoints, indexer_config, generate_erpc_config)
        };

    let needs_instance_type = is_aws
        && matches!(
            compute_engine,
            ComputeEngine::Ec2 | ComputeEngine::Eks | ComputeEngine::K3s
        );
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

    // -- Remote state --
    let (state_config, auto_bootstrap) = collect_state_answers(&theme, &project_name, &region)?;

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
        state_config,
        auto_bootstrap,
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
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    s.trim_end_matches('/').to_string()
}

// ---------------------------------------------------------------------------
// Remote state wizard
// ---------------------------------------------------------------------------

pub(crate) fn collect_state_answers(
    theme: &ColorfulTheme,
    project_name: &str,
    region: &Option<String>,
) -> Result<(Option<StateConfig>, bool)> {
    let want_state = Confirm::with_theme(theme)
        .with_prompt("Configure remote Terraform state? (recommended for production)")
        .default(false)
        .interact()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    if !want_state {
        return Ok((None, false));
    }

    let backend_options = ["S3 (AWS)", "GCS (Google Cloud)"];
    let backend_idx = Select::with_theme(theme)
        .with_prompt("State backend")
        .items(&backend_options)
        .default(0)
        .interact()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    let state_config = if backend_idx == 0 {
        collect_s3_state(theme, project_name, region)?
    } else {
        collect_gcs_state(theme, project_name)?
    };

    let resource_desc = match &state_config {
        StateConfig::S3 { .. } => "S3 bucket + DynamoDB table",
        StateConfig::Gcs { .. } => "GCS bucket",
    };

    let auto_bootstrap = Confirm::with_theme(theme)
        .with_prompt(format!(
            "Create the state backend resources now? ({resource_desc})"
        ))
        .default(true)
        .interact()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    Ok((Some(state_config), auto_bootstrap))
}

fn collect_s3_state(
    theme: &ColorfulTheme,
    project_name: &str,
    region: &Option<String>,
) -> Result<StateConfig> {
    let default_bucket = sanitize_bucket_name(&format!("{project_name}-terraform-state"));
    let bucket: String = Input::with_theme(theme)
        .with_prompt("S3 bucket name")
        .default(default_bucket)
        .validate_with(validate_bucket_name)
        .interact_text()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    let default_table = sanitize_bucket_name(&format!("{project_name}-terraform-locks"));
    let dynamodb_table: String = Input::with_theme(theme)
        .with_prompt("DynamoDB lock table name")
        .default(default_table)
        .validate_with(validate_bucket_name)
        .interact_text()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    let state_region: String = if let Some(r) = region {
        Input::with_theme(theme)
            .with_prompt("AWS region for state backend")
            .default(r.clone())
            .interact_text()
            .map_err(|err| CliError::PromptFailed(err.to_string()))?
    } else {
        // BareMetal + S3: no default region — always prompt (constraint #7)
        Input::with_theme(theme)
            .with_prompt("AWS region for state backend")
            .interact_text()
            .map_err(|err| CliError::PromptFailed(err.to_string()))?
    };

    Ok(StateConfig::S3 {
        bucket,
        dynamodb_table,
        region: state_region,
        key: None,
        encrypt: true,
    })
}

fn collect_gcs_state(theme: &ColorfulTheme, project_name: &str) -> Result<StateConfig> {
    let default_bucket = sanitize_bucket_name(&format!("{project_name}-terraform-state"));
    let bucket: String = Input::with_theme(theme)
        .with_prompt("GCS bucket name")
        .default(default_bucket)
        .validate_with(validate_bucket_name)
        .interact_text()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    let gcs_region: String = Input::with_theme(theme)
        .with_prompt("GCS region (e.g. us-central1, EU, US)")
        .default("US".to_string())
        .validate_with(validate_gcs_region)
        .interact_text()
        .map_err(|err| CliError::PromptFailed(err.to_string()))?;

    Ok(StateConfig::Gcs {
        bucket,
        region: gcs_region,
        prefix: None,
    })
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

pub(crate) fn sanitize_bucket_name(raw: &str) -> String {
    let sanitized: String = raw
        .to_lowercase()
        .replace(['_', '.'], "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(63)
        .collect();
    sanitized.trim_matches('-').to_string()
}

#[allow(clippy::ptr_arg)] // dialoguer's InputValidator requires &String
fn validate_bucket_name(input: &String) -> std::result::Result<(), String> {
    let len = input.len();
    if !(3..=63).contains(&len) {
        return Err("Must be 3-63 characters".to_string());
    }
    let valid = input
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !valid {
        return Err("Only lowercase letters, digits, and hyphens allowed".to_string());
    }
    if input.starts_with('-') || input.ends_with('-') {
        return Err("Must not start or end with a hyphen".to_string());
    }
    Ok(())
}

#[allow(clippy::ptr_arg)] // dialoguer's InputValidator requires &String
fn validate_gcs_region(input: &String) -> std::result::Result<(), String> {
    let r = input.trim();
    // Multi-region values
    let multi_regions = ["US", "EU", "ASIA"];
    if multi_regions.contains(&r) {
        return Ok(());
    }
    // Dual-region patterns (e.g. NAM4, EUR4)
    if matches!(r, "NAM4" | "EUR4" | "EUR7" | "EUR12") {
        return Ok(());
    }
    // Standard regions: continent-direction-number (e.g. us-central1, europe-west1, asia-east1)
    let parts: Vec<&str> = r.split('-').collect();
    if parts.len() >= 2 {
        let valid_prefixes = [
            "us",
            "europe",
            "asia",
            "northamerica",
            "southamerica",
            "australia",
            "me",
            "africa",
        ];
        if valid_prefixes.iter().any(|p| parts[0] == *p) {
            return Ok(());
        }
    }
    Err(format!(
        "Unknown GCS region '{r}'. Expected: US, EU, ASIA, or a region like us-central1, europe-west1"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_bucket_name_lowercases_and_replaces() {
        assert_eq!(sanitize_bucket_name("My_Project.Name"), "my-project-name");
    }

    #[test]
    fn sanitize_bucket_name_trims_hyphens() {
        assert_eq!(sanitize_bucket_name("-foo-"), "foo");
    }

    #[test]
    fn sanitize_bucket_name_truncates_to_63() {
        let long = "a".repeat(100);
        assert_eq!(sanitize_bucket_name(&long).len(), 63);
    }

    #[test]
    fn sanitize_bucket_name_strips_invalid_chars() {
        assert_eq!(sanitize_bucket_name("hello@world!"), "helloworld");
    }

    #[test]
    fn validate_bucket_name_accepts_valid() {
        assert!(validate_bucket_name(&"my-bucket-123".to_string()).is_ok());
    }

    #[test]
    fn validate_bucket_name_rejects_too_short() {
        assert!(validate_bucket_name(&"ab".to_string()).is_err());
    }

    #[test]
    fn validate_bucket_name_rejects_uppercase() {
        assert!(validate_bucket_name(&"My-Bucket".to_string()).is_err());
    }

    #[test]
    fn validate_bucket_name_rejects_leading_hyphen() {
        assert!(validate_bucket_name(&"-my-bucket".to_string()).is_err());
    }

    #[test]
    fn validate_gcs_region_accepts_multi_region() {
        assert!(validate_gcs_region(&"US".to_string()).is_ok());
        assert!(validate_gcs_region(&"EU".to_string()).is_ok());
        assert!(validate_gcs_region(&"ASIA".to_string()).is_ok());
    }

    #[test]
    fn validate_gcs_region_accepts_standard() {
        assert!(validate_gcs_region(&"us-central1".to_string()).is_ok());
        assert!(validate_gcs_region(&"europe-west1".to_string()).is_ok());
    }

    #[test]
    fn validate_gcs_region_rejects_unknown() {
        assert!(validate_gcs_region(&"mars-north1".to_string()).is_err());
    }
}
