use std::collections::BTreeMap;

use crate::config::schema::ComputeEngine;
use crate::init_answers::{DatabaseProfile, IndexerConfigStrategy, InitAnswers};

pub(crate) fn render_evm_cloud_toml(answers: &InitAnswers) -> String {
    let (database_mode, _) = map_database_profile(answers.database_profile);
    let database_provider = &answers.infrastructure_provider;

    let chains = answers
        .chains
        .iter()
        .map(|chain| format!("\"{chain}\""))
        .collect::<Vec<_>>()
        .join(", ");

    let endpoints = render_endpoints(&answers.rpc_endpoints);

    let indexer_config_path = match &answers.indexer_config {
        IndexerConfigStrategy::Generate => "config/rindexer.yaml".to_string(),
        IndexerConfigStrategy::Existing(path) => path.display().to_string(),
    };

    let erpc_line = if answers.generate_erpc_config {
        "erpc_config_path = \"config/erpc.yaml\"\n".to_string()
    } else {
        String::new()
    };

    let region_line = match &answers.region {
        Some(region) => format!("region = \"{region}\"\n"),
        None => String::new(),
    };

    let instance_type_line = match &answers.instance_type {
        Some(it) => format!("instance_type = \"{it}\"\n"),
        None => String::new(),
    };

    let secrets_mode = infer_secrets_mode(answers);
    let storage_backend = infer_storage_backend(answers);

    let mut ingress_extras = String::new();
    if let Some(hostname) = &answers.erpc_hostname {
        ingress_extras.push_str(&format!("domain = \"{hostname}\"\n"));
    }
    if let Some(email) = &answers.ingress_tls_email {
        ingress_extras.push_str(&format!("tls_email = \"{email}\"\n"));
    }

    format!(
        "schema_version = 1\n\n[project]\nname = \"{}\"\n{}\n[compute]\nengine = \"{}\"\n{}\n[database]\nmode = \"{}\"\nprovider = \"{}\"\nstorage_backend = \"{}\"\n\n[indexer]\nconfig_path = \"{}\"\n{}chains = [{}]\n\n[rpc]\nendpoints = {{ {} }}\n\n[ingress]\nmode = \"{}\"\n{}\n[secrets]\nmode = \"{}\"\n",
        answers.project_name,
        region_line,
        answers.compute_engine,
        instance_type_line,
        database_mode,
        database_provider,
        storage_backend,
        indexer_config_path,
        erpc_line,
        chains,
        endpoints,
        answers.ingress_mode,
        ingress_extras,
        secrets_mode
    )
}

fn map_database_profile(profile: DatabaseProfile) -> (&'static str, &'static str) {
    match profile {
        DatabaseProfile::ByodbClickhouse => ("self_hosted", "aws"),
        DatabaseProfile::ByodbPostgres => ("self_hosted", "aws"),
        DatabaseProfile::ManagedRds => ("managed", "aws"),
        DatabaseProfile::ManagedClickhouse => ("managed", "aws"),
    }
}

fn infer_storage_backend(answers: &InitAnswers) -> &'static str {
    match answers.database_profile {
        DatabaseProfile::ByodbPostgres | DatabaseProfile::ManagedRds => "postgres",
        DatabaseProfile::ByodbClickhouse | DatabaseProfile::ManagedClickhouse => "clickhouse",
    }
}

fn infer_secrets_mode(answers: &InitAnswers) -> String {
    if answers.infrastructure_provider.is_bare_metal() || answers.compute_engine == ComputeEngine::K3s {
        "inline".to_string()
    } else {
        "provider".to_string()
    }
}

fn render_endpoints(endpoints: &BTreeMap<String, String>) -> String {
    endpoints
        .iter()
        .map(|(chain, endpoint)| format!("{} = \"{}\"", chain, endpoint))
        .collect::<Vec<_>>()
        .join(", ")
}
