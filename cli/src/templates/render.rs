use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::error::{CliError, Result};
use crate::templates::chains;
use crate::templates::types::TemplateManifest;

/// Text file extensions that receive variable substitution.
const TEXT_EXTENSIONS: &[&str] = &[
    "yaml", "yml", "sql", "json", "toml", "md", "txt", "graphql",
];

/// Files that belong in `config/` when routed (rindexer config + ABIs).
/// Everything else (clickhouse/, grafana/, README.md) goes to project root.
fn is_config_file(rel_path: &Path) -> bool {
    let s = rel_path.to_str().unwrap_or_default();
    let file_name = rel_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    file_name == "rindexer.yaml"
        || file_name == "rindexer.yml"
        || s.starts_with("abis/")
        || s.starts_with("abis\\")
}

/// Render a template package with selective file routing.
///
/// - `output_dir`: where config files (rindexer.yaml, abis/) are written
/// - `root_dir`: if `Some`, non-config files (clickhouse/, grafana/, README.md)
///   go here instead of `output_dir`. If `None`, all files go to `output_dir`.
///
/// Returns the list of relative file paths written (relative to their target dir).
pub(crate) fn render_template(
    template_dir: &Path,
    manifest: &TemplateManifest,
    selected_chains: &[String],
    user_vars: &BTreeMap<String, String>,
    output_dir: &Path,
    root_dir: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    // Validate required variables (those without defaults)
    for (var_name, var_def) in &manifest.variables {
        if var_def.default.is_none() && !user_vars.contains_key(var_name) {
            return Err(CliError::TemplateVariableRequired {
                template: manifest.name.clone(),
                variable: var_name.clone(),
            });
        }
    }

    // Build the base variable map (non-chain-specific)
    let mut base_vars: BTreeMap<String, String> = BTreeMap::new();

    // Add user vars
    for (k, v) in user_vars {
        base_vars.insert(k.clone(), v.clone());
    }

    // Add defaults for any unset variables
    for (var_name, var_def) in &manifest.variables {
        if !base_vars.contains_key(var_name) {
            if let Some(default) = &var_def.default {
                base_vars.insert(var_name.clone(), toml_value_to_string(default));
            }
        }
    }

    // For single chain, set chain vars directly; for multi-chain, we need
    // special handling for rindexer.yaml (multiple network entries).
    let first_chain = selected_chains.first().ok_or_else(|| CliError::FlagConflict {
        message: "--chains is required with --template".to_string(),
    })?;

    // Build per-chain variable maps
    let mut per_chain_vars: Vec<BTreeMap<String, String>> = Vec::new();
    for chain_name in selected_chains {
        let mut vars = base_vars.clone();
        vars.insert("chain_name".to_string(), chain_name.clone());

        if let Some(id) = chains::chain_id(chain_name) {
            vars.insert("chain_id".to_string(), id.to_string());
        }

        if let Some(chain_cfg) = manifest.chains.get(chain_name) {
            for (contract_name, contract_def) in &chain_cfg.contracts {
                vars.insert(
                    format!("contract_address_{contract_name}"),
                    contract_def.address.clone(),
                );
                vars.insert(
                    format!("start_block_{contract_name}"),
                    contract_def.start_block.to_string(),
                );
            }
        }

        per_chain_vars.push(vars);
    }

    // Use first chain's vars as the primary variable map for most files.
    // For rindexer.yaml, we handle multi-chain specially.
    let primary_vars = &per_chain_vars[0];

    let evm_re = Regex::new(r"\{\{evm:(\w+)\}\}").expect("static regex");
    let mut written: Vec<PathBuf> = Vec::new();

    // Walk all files in template_dir
    let template_files = collect_files(template_dir)?;

    for rel_path in &template_files {
        let file_name = rel_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        // Skip template.toml and README.md from the template package
        if file_name == "template.toml" {
            continue;
        }

        let src_path = template_dir.join(rel_path);
        let target_dir = match root_dir {
            Some(root) if !is_config_file(rel_path) => root,
            _ => output_dir,
        };
        let dst_path = target_dir.join(rel_path);

        if let Some(parent) = dst_path.parent() {
            fs::create_dir_all(parent).map_err(|source| CliError::Io {
                source,
                path: parent.to_path_buf(),
            })?;
        }

        let is_text = rel_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| TEXT_EXTENSIONS.contains(&ext))
            .unwrap_or(false);

        if !is_text {
            // Binary file: copy verbatim
            fs::copy(&src_path, &dst_path).map_err(|source| CliError::Io {
                source,
                path: src_path.clone(),
            })?;
            written.push(rel_path.clone());
            continue;
        }

        let content = fs::read_to_string(&src_path).map_err(|source| CliError::Io {
            source,
            path: src_path.clone(),
        })?;

        // Special handling for rindexer.yaml with multiple chains
        let is_rindexer = file_name == "rindexer.yaml" || file_name == "rindexer.yml";

        let rendered = if is_rindexer {
            render_rindexer_multi_chain(&content, &per_chain_vars, &evm_re, first_chain)?
        } else {
            substitute_vars(&content, primary_vars, &evm_re)?
        };

        // Check for unresolved variables
        check_unresolved(&rendered, &evm_re, rel_path)?;

        fs::write(&dst_path, &rendered).map_err(|source| CliError::Io {
            source,
            path: dst_path.clone(),
        })?;

        written.push(rel_path.clone());
    }

    Ok(written)
}

/// For multi-chain rindexer.yaml: expand `{{evm:networks}}` and `{{evm:contract_details}}`
/// markers to produce one entry per chain with chain-specific variables.
fn render_rindexer_multi_chain(
    content: &str,
    per_chain_vars: &[BTreeMap<String, String>],
    evm_re: &Regex,
    _first_chain: &str,
) -> Result<String> {
    let mut result = content.to_string();

    // 1. Expand {{evm:networks}} — one network block per chain
    if result.contains("{{evm:networks}}") {
        let mut networks_block = String::new();
        for (i, vars) in per_chain_vars.iter().enumerate() {
            let chain = vars.get("chain_name").cloned().unwrap_or_default();
            let chain_id = vars.get("chain_id").cloned().unwrap_or_default();
            if i > 0 {
                // Align with the marker's parent indent (2 spaces for `  {{evm:networks}}`)
                networks_block.push_str("  ");
            }
            networks_block.push_str(&format!(
                "- name: {chain}\n    chain_id: {chain_id}\n    rpc: ${{RPC_URL}}/main/evm/{chain_id}\n",
            ));
        }
        result = result.replace("{{evm:networks}}", networks_block.trim_end());
    }

    // 2. Expand {{evm:contract_details}} — one detail entry per chain
    //    Detect contract name from the preceding `name:` line in the YAML block.
    if result.contains("{{evm:contract_details}}") {
        let lines: Vec<&str> = result.lines().collect();
        let mut expanded = String::new();
        let mut contract_name: Option<String> = None;

        for line in &lines {
            if line.trim().starts_with("- name:") {
                // Extract the contract name for the next contract_details marker
                contract_name = line.trim().strip_prefix("- name:").map(|s| s.trim().to_string());
            }

            if line.contains("{{evm:contract_details}}") {
                // Find indentation of the marker
                let indent = &line[..line.len() - line.trim_start().len()];
                let details = render_contract_details(per_chain_vars, contract_name.as_deref(), indent);
                expanded.push_str(&details);
            } else {
                expanded.push_str(line);
                expanded.push('\n');
            }
        }

        result = expanded;
    }

    // 3. Substitute remaining vars using the first chain's vars
    substitute_vars(&result, &per_chain_vars[0], evm_re)
}

/// Render per-chain contract detail entries for `{{evm:contract_details}}` expansion.
fn render_contract_details(
    per_chain_vars: &[BTreeMap<String, String>],
    contract_name: Option<&str>,
    indent: &str,
) -> String {
    let mut block = String::new();
    for vars in per_chain_vars.iter() {
        let chain = vars.get("chain_name").cloned().unwrap_or_default();

        // Try contract_address_<Name> first, then fall back to generic address vars
        let address = contract_name
            .and_then(|name| vars.get(&format!("contract_address_{name}")))
            .cloned()
            .unwrap_or_default();

        let start_block = contract_name
            .and_then(|name| vars.get(&format!("start_block_{name}")))
            .cloned()
            .unwrap_or_else(|| "0".to_string());

        block.push_str(indent);
        block.push_str(&format!("- network: {chain}\n"));
        block.push_str(&format!("{indent}  address: \"{address}\"\n"));
        block.push_str(&format!("{indent}  start_block: \"{start_block}\"\n"));
    }
    block
}

/// Generate an eRPC config YAML for the given chain IDs.
///
/// Uses `repository://evm-public-endpoints.erpc.cloud` as the universal upstream,
/// which covers all EVM chains out of the box.
pub(crate) fn render_erpc_from_chains(chain_ids: &[u64]) -> String {
    let mut networks = String::new();
    for chain_id in chain_ids {
        networks.push_str(&format!(
            "      - architecture: evm\n        evm:\n          chainId: {chain_id}\n"
        ));
    }

    format!(
        "logLevel: warn\nprojects:\n  - id: main\n    networks:\n{networks}    upstreams:\n      - endpoint: repository://evm-public-endpoints.erpc.cloud\n        type: evm\nserver:\n  listenV4: true\n  httpHostV4: 0.0.0.0\n  httpPort: 4000\n"
    )
}

fn substitute_vars(
    content: &str,
    vars: &BTreeMap<String, String>,
    evm_re: &Regex,
) -> Result<String> {
    let result = evm_re.replace_all(content, |caps: &regex::Captures| {
        let var_name = &caps[1];
        vars.get(var_name)
            .cloned()
            .unwrap_or_else(|| caps[0].to_string())
    });
    Ok(result.to_string())
}

fn check_unresolved(content: &str, evm_re: &Regex, file_path: &Path) -> Result<()> {
    for (line_idx, line) in content.lines().enumerate() {
        if let Some(mat) = evm_re.find(line) {
            let var = mat.as_str().to_string();
            return Err(CliError::TemplateRenderError {
                variable: var,
                file: file_path.to_path_buf(),
                line: line_idx + 1,
            });
        }
    }
    Ok(())
}

fn toml_value_to_string(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        other => other.to_string(),
    }
}

fn collect_files(dir: &Path) -> Result<Vec<PathBuf>> {
    collect_files_inner(dir, dir)
}

fn collect_files_inner(root: &Path, cursor: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(cursor)
        .map_err(|source| CliError::Io {
            source,
            path: cursor.to_path_buf(),
        })?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_files_inner(root, &path)?);
        } else if path.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                files.push(rel.to_path_buf());
            }
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::types::TemplateManifestFile;

    fn templates_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../templates")
    }

    #[test]
    fn substitute_simple_vars() {
        let evm_re = Regex::new(r"\{\{evm:(\w+)\}\}").unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("chain_name".to_string(), "polygon".to_string());
        vars.insert("chain_id".to_string(), "137".to_string());

        let input = "network: {{evm:chain_name}}\nid: {{evm:chain_id}}";
        let result = substitute_vars(input, &vars, &evm_re).unwrap();
        assert_eq!(result, "network: polygon\nid: 137");
    }

    #[test]
    fn unresolved_var_detected() {
        let evm_re = Regex::new(r"\{\{evm:(\w+)\}\}").unwrap();
        let content = "address = {{evm:missing_var}}";
        let err = check_unresolved(content, &evm_re, Path::new("test.yaml"));
        assert!(err.is_err());
    }

    #[test]
    fn toml_value_conversions() {
        assert_eq!(toml_value_to_string(&toml::Value::Integer(42)), "42");
        assert_eq!(
            toml_value_to_string(&toml::Value::String("hello".to_string())),
            "hello"
        );
        assert_eq!(toml_value_to_string(&toml::Value::Boolean(true)), "true");
    }

    #[test]
    fn substitute_contract_vars() {
        let evm_re = Regex::new(r"\{\{evm:(\w+)\}\}").unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("contract_address_PoolManager".to_string(), "0xABCD".to_string());
        vars.insert("start_block_PoolManager".to_string(), "12345".to_string());
        vars.insert("chain_name".to_string(), "ethereum".to_string());

        let input = "address: {{evm:contract_address_PoolManager}}\nstart: {{evm:start_block_PoolManager}}\nchain: {{evm:chain_name}}";
        let result = substitute_vars(input, &vars, &evm_re).unwrap();
        assert_eq!(result, "address: 0xABCD\nstart: 12345\nchain: ethereum");
    }

    #[test]
    fn substitute_preserves_non_evm_patterns() {
        let evm_re = Regex::new(r"\{\{evm:(\w+)\}\}").unwrap();
        let vars = BTreeMap::from([("chain_name".to_string(), "polygon".to_string())]);

        // ${RPC_URL} is an env var pattern, not an evm template var
        let input = "rpc: ${RPC_URL}\nchain: {{evm:chain_name}}";
        let result = substitute_vars(input, &vars, &evm_re).unwrap();
        assert_eq!(result, "rpc: ${RPC_URL}\nchain: polygon");
    }

    #[test]
    fn render_uniswap_v4_template() {
        let template_dir = templates_dir().join("uniswap-v4");
        let content = std::fs::read_to_string(template_dir.join("template.toml")).unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let manifest = &manifest_file.template;

        let output_dir = tempfile::tempdir().unwrap();
        let user_vars = BTreeMap::new(); // reorg_safe_distance has a default

        let written = render_template(
            &template_dir,
            manifest,
            &["ethereum".to_string()],
            &user_vars,
            output_dir.path(),
            None,
        )
        .unwrap();

        assert!(!written.is_empty(), "should produce output files");

        // Check rindexer.yaml was rendered
        let rindexer = std::fs::read_to_string(output_dir.path().join("rindexer.yaml")).unwrap();
        assert!(rindexer.contains("ethereum"), "rindexer.yaml should contain chain name");
        assert!(
            rindexer.contains("0x000000000004444c5dc75cB358380D2e3dE08A90"),
            "rindexer.yaml should contain PoolManager address"
        );
        assert!(
            rindexer.contains("21688329"),
            "rindexer.yaml should contain start block"
        );

        // No unresolved {{evm:...}} patterns
        let evm_re = Regex::new(r"\{\{evm:(\w+)\}\}").unwrap();
        assert!(
            !evm_re.is_match(&rindexer),
            "rindexer.yaml should have no unresolved evm vars"
        );

        // Expected output files exist
        assert!(output_dir.path().join("abis/PoolManager.json").exists());
        assert!(output_dir.path().join("clickhouse/materialized_views.sql").exists());
        assert!(output_dir.path().join("grafana/dashboard.json").exists());
    }

    #[test]
    fn render_erc20_transfers_template() {
        let template_dir = templates_dir().join("erc20-transfers");
        let content = std::fs::read_to_string(template_dir.join("template.toml")).unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let manifest = &manifest_file.template;

        let output_dir = tempfile::tempdir().unwrap();
        let user_vars = BTreeMap::from([
            ("token_address".to_string(), "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string()),
            ("token_symbol".to_string(), "USDC".to_string()),
            ("start_block".to_string(), "25000000".to_string()),
        ]);

        let written = render_template(
            &template_dir,
            manifest,
            &["polygon".to_string()],
            &user_vars,
            output_dir.path(),
            None,
        )
        .unwrap();

        assert!(!written.is_empty());

        let rindexer = std::fs::read_to_string(output_dir.path().join("rindexer.yaml")).unwrap();
        assert!(rindexer.contains("polygon"));
        assert!(rindexer.contains("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"));
        assert!(rindexer.contains("USDC"));

        let evm_re = Regex::new(r"\{\{evm:(\w+)\}\}").unwrap();
        assert!(!evm_re.is_match(&rindexer), "no unresolved vars in rindexer.yaml");

        assert!(output_dir.path().join("abis/ERC20.json").exists());
        assert!(output_dir.path().join("clickhouse/materialized_views.sql").exists());
    }

    #[test]
    fn render_missing_required_var_errors() {
        let template_dir = templates_dir().join("erc20-transfers");
        let content = std::fs::read_to_string(template_dir.join("template.toml")).unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let manifest = &manifest_file.template;

        let output_dir = tempfile::tempdir().unwrap();
        // Omit required token_address
        let user_vars = BTreeMap::new();

        let result = render_template(
            &template_dir,
            manifest,
            &["polygon".to_string()],
            &user_vars,
            output_dir.path(),
            None,
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("token_address"),
            "error should mention the missing variable: {err}"
        );
    }

    #[test]
    fn render_default_var_applied() {
        let template_dir = templates_dir().join("erc20-transfers");
        let content = std::fs::read_to_string(template_dir.join("template.toml")).unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let manifest = &manifest_file.template;

        let output_dir = tempfile::tempdir().unwrap();
        // Provide token_address but omit token_symbol — should use default "TOKEN"
        let user_vars = BTreeMap::from([
            ("token_address".to_string(), "0xabc".to_string()),
            ("start_block".to_string(), "100".to_string()),
        ]);

        let written = render_template(
            &template_dir,
            manifest,
            &["ethereum".to_string()],
            &user_vars,
            output_dir.path(),
            None,
        )
        .unwrap();

        assert!(!written.is_empty());
        let rindexer = std::fs::read_to_string(output_dir.path().join("rindexer.yaml")).unwrap();
        assert!(
            rindexer.contains("TOKEN"),
            "default token_symbol 'TOKEN' should be applied"
        );
    }

    #[test]
    fn render_aave_v4_template() {
        let template_dir = templates_dir().join("aave-v4");
        let content = std::fs::read_to_string(template_dir.join("template.toml")).unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let manifest = &manifest_file.template;

        let output_dir = tempfile::tempdir().unwrap();
        let user_vars = BTreeMap::from([
            ("spoke_address".to_string(), "0xdef1234567890abcdef1234567890abcdef12345".to_string()),
            ("spoke_start_block".to_string(), "21000000".to_string()),
        ]);

        let written = render_template(
            &template_dir,
            manifest,
            &["ethereum".to_string()],
            &user_vars,
            output_dir.path(),
            None,
        )
        .unwrap();

        assert!(!written.is_empty());
        let rindexer = std::fs::read_to_string(output_dir.path().join("rindexer.yaml")).unwrap();
        assert!(rindexer.contains("0xdef1234567890abcdef1234567890abcdef12345"));
        assert!(rindexer.contains("21000000"));
        assert!(rindexer.contains("ethereum"));

        let evm_re = Regex::new(r"\{\{evm:(\w+)\}\}").unwrap();
        assert!(!evm_re.is_match(&rindexer), "no unresolved vars");
    }

    #[test]
    fn render_no_chains_errors() {
        let template_dir = templates_dir().join("uniswap-v4");
        let content = std::fs::read_to_string(template_dir.join("template.toml")).unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let manifest = &manifest_file.template;

        let output_dir = tempfile::tempdir().unwrap();
        let result = render_template(
            &template_dir,
            manifest,
            &[],
            &BTreeMap::new(),
            output_dir.path(),
            None,
        );

        assert!(result.is_err(), "empty chains list should error");
    }

    #[test]
    fn render_template_toml_excluded_from_output() {
        let template_dir = templates_dir().join("uniswap-v4");
        let content = std::fs::read_to_string(template_dir.join("template.toml")).unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let manifest = &manifest_file.template;

        let output_dir = tempfile::tempdir().unwrap();
        render_template(
            &template_dir,
            manifest,
            &["ethereum".to_string()],
            &BTreeMap::new(),
            output_dir.path(),
            None,
        )
        .unwrap();

        assert!(
            !output_dir.path().join("template.toml").exists(),
            "template.toml should not be copied to output"
        );
    }

    #[test]
    fn render_contract_details_multi_chain() {
        let template_dir = templates_dir().join("aave-v3");
        let content = std::fs::read_to_string(template_dir.join("template.toml")).unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let manifest = &manifest_file.template;

        let output_dir = tempfile::tempdir().unwrap();
        let written = render_template(
            &template_dir,
            manifest,
            &["ethereum".to_string(), "base".to_string()],
            &BTreeMap::new(),
            output_dir.path(),
            None,
        )
        .unwrap();

        assert!(!written.is_empty());
        let rindexer = std::fs::read_to_string(output_dir.path().join("rindexer.yaml")).unwrap();

        // Should have 2 network blocks
        assert!(rindexer.contains("- name: ethereum"), "missing ethereum network");
        assert!(rindexer.contains("- name: base"), "missing base network");

        // Should have 2 contract detail entries with correct addresses from manifest
        assert!(
            rindexer.contains("0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"),
            "missing ethereum AaveV3Pool address"
        );
        assert!(
            rindexer.contains("0xA238Dd80C259a72e81d7e4664a9801593F98d1c5"),
            "missing base AaveV3Pool address"
        );

        // Should be valid YAML
        let parsed: serde_yaml::Value = serde_yaml::from_str(&rindexer)
            .expect("rendered multi-chain rindexer.yaml should be valid YAML");
        assert!(parsed.is_mapping());
    }

    #[test]
    fn render_erpc_from_chains_generates_valid_config() {
        let erpc = render_erpc_from_chains(&[1, 8453]);

        assert!(erpc.contains("chainId: 1"), "missing ethereum chain");
        assert!(erpc.contains("chainId: 8453"), "missing base chain");
        assert!(
            erpc.contains("repository://evm-public-endpoints.erpc.cloud"),
            "missing eRPC upstream"
        );
        assert!(erpc.contains("id: main"), "missing project id");

        // Should be valid YAML
        let parsed: serde_yaml::Value =
            serde_yaml::from_str(&erpc).expect("eRPC config should be valid YAML");
        assert!(parsed.is_mapping());
    }

    #[test]
    fn render_template_routing_separates_config_and_root() {
        let template_dir = templates_dir().join("uniswap-v4");
        let content = std::fs::read_to_string(template_dir.join("template.toml")).unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let manifest = &manifest_file.template;

        let config_dir = tempfile::tempdir().unwrap();
        let root_dir = tempfile::tempdir().unwrap();

        render_template(
            &template_dir,
            manifest,
            &["ethereum".to_string()],
            &BTreeMap::new(),
            config_dir.path(),
            Some(root_dir.path()),
        )
        .unwrap();

        // Config files should go to config_dir
        assert!(
            config_dir.path().join("rindexer.yaml").exists(),
            "rindexer.yaml should be in config_dir"
        );
        assert!(
            config_dir.path().join("abis/PoolManager.json").exists(),
            "abis/ should be in config_dir"
        );

        // Non-config files should go to root_dir
        assert!(
            root_dir.path().join("clickhouse/materialized_views.sql").exists(),
            "clickhouse/ should be in root_dir"
        );
        assert!(
            root_dir.path().join("grafana/dashboard.json").exists(),
            "grafana/ should be in root_dir"
        );

        // Config files should NOT be in root_dir
        assert!(
            !root_dir.path().join("rindexer.yaml").exists(),
            "rindexer.yaml should NOT be in root_dir"
        );
    }

    #[test]
    fn collect_files_finds_nested_files() {
        let template_dir = templates_dir().join("erc20-transfers");
        let files = collect_files(&template_dir).unwrap();

        // Should find files in subdirectories
        let has_abi = files.iter().any(|p| p.to_str().unwrap().contains("abis/"));
        let has_clickhouse = files.iter().any(|p| p.to_str().unwrap().contains("clickhouse/"));
        let has_grafana = files.iter().any(|p| p.to_str().unwrap().contains("grafana/"));

        assert!(has_abi, "should find ABI files");
        assert!(has_clickhouse, "should find clickhouse files");
        assert!(has_grafana, "should find grafana files");
    }
}
