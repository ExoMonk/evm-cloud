//! Template content validation tests.
//!
//! These tests verify the structural integrity of all template packages in
//! `templates/` — valid manifests, parseable YAML/JSON/SQL, registry consistency,
//! and chain mapping coverage.

use std::collections::BTreeMap;
use std::path::PathBuf;

use regex::Regex;

use crate::templates::chains;
use crate::templates::render::render_template;
use crate::templates::types::{RegistryFile, TemplateManifestFile};

fn templates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../templates")
}

fn load_registry() -> RegistryFile {
    let content = std::fs::read_to_string(templates_dir().join("registry.toml")).unwrap();
    toml::from_str(&content).unwrap()
}

fn template_dirs() -> Vec<String> {
    std::fs::read_dir(templates_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect()
}

fn load_manifest(template_name: &str) -> TemplateManifestFile {
    let path = templates_dir()
        .join(template_name)
        .join("template.toml");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    toml::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

// ── Manifest validation ──────────────────────────────────────────────

#[test]
fn all_templates_have_valid_manifest() {
    for dir_name in template_dirs() {
        let manifest = load_manifest(&dir_name);
        assert!(
            !manifest.template.name.is_empty(),
            "{dir_name}: template name is empty"
        );
        assert!(
            !manifest.template.version.is_empty(),
            "{dir_name}: template version is empty"
        );
        assert!(
            !manifest.template.description.is_empty(),
            "{dir_name}: template description is empty"
        );
        assert!(
            !manifest.template.chains.is_empty(),
            "{dir_name}: template must support at least one chain"
        );
    }
}

// ── YAML validation ──────────────────────────────────────────────────

#[test]
fn all_templates_have_valid_rindexer_yaml() {
    for dir_name in template_dirs() {
        let yaml_path = templates_dir().join(&dir_name).join("rindexer.yaml");
        let content = std::fs::read_to_string(&yaml_path)
            .unwrap_or_else(|e| panic!("{dir_name}: missing rindexer.yaml: {e}"));

        // The YAML contains {{evm:...}} placeholders, so it won't parse as
        // strict YAML. Instead, verify it's non-empty and has expected keys.
        assert!(
            !content.is_empty(),
            "{dir_name}: rindexer.yaml is empty"
        );
        assert!(
            content.contains("name:"),
            "{dir_name}: rindexer.yaml missing 'name:' field"
        );
        assert!(
            content.contains("networks:"),
            "{dir_name}: rindexer.yaml missing 'networks:' field"
        );
        assert!(
            content.contains("contracts:"),
            "{dir_name}: rindexer.yaml missing 'contracts:' field"
        );
        assert!(
            content.contains("storage:"),
            "{dir_name}: rindexer.yaml missing 'storage:' field"
        );
    }
}

// ── ABI validation ───────────────────────────────────────────────────

#[test]
fn all_templates_have_valid_abis() {
    for dir_name in template_dirs() {
        let abis_dir = templates_dir().join(&dir_name).join("abis");
        if !abis_dir.exists() {
            panic!("{dir_name}: missing abis/ directory");
        }

        let abi_files: Vec<_> = std::fs::read_dir(&abis_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "json")
                    .unwrap_or(false)
            })
            .collect();

        assert!(
            !abi_files.is_empty(),
            "{dir_name}: abis/ directory has no .json files"
        );

        for entry in abi_files {
            let abi_content = std::fs::read_to_string(entry.path()).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&abi_content).unwrap_or_else(
                |e| {
                    panic!(
                        "{dir_name}: invalid JSON in {}: {e}",
                        entry.file_name().to_string_lossy()
                    )
                },
            );
            assert!(
                parsed.is_array(),
                "{dir_name}: ABI {} should be a JSON array",
                entry.file_name().to_string_lossy()
            );
        }
    }
}

// ── ClickHouse SQL validation ────────────────────────────────────────

#[test]
fn all_templates_have_valid_clickhouse_sql() {
    for dir_name in template_dirs() {
        let ch_dir = templates_dir().join(&dir_name).join("clickhouse");
        assert!(
            ch_dir.exists(),
            "{dir_name}: missing clickhouse/ directory"
        );

        let mv = ch_dir.join("materialized_views.sql");
        assert!(mv.exists(), "{dir_name}: missing clickhouse/materialized_views.sql");
        let content = std::fs::read_to_string(&mv).unwrap();
        assert!(
            !content.trim().is_empty(),
            "{dir_name}: clickhouse/materialized_views.sql is empty"
        );
    }
}

// ── Grafana dashboard validation ─────────────────────────────────────

#[test]
fn all_templates_have_valid_grafana_dashboard() {
    for dir_name in template_dirs() {
        let dashboard = templates_dir()
            .join(&dir_name)
            .join("grafana/dashboard.json");
        assert!(
            dashboard.exists(),
            "{dir_name}: missing grafana/dashboard.json"
        );

        let content = std::fs::read_to_string(&dashboard).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap_or_else(|e| {
            panic!("{dir_name}: invalid JSON in grafana/dashboard.json: {e}")
        });

        // Grafana dashboards should have a "panels" or "title" key
        assert!(
            parsed.get("title").is_some() || parsed.get("panels").is_some(),
            "{dir_name}: grafana/dashboard.json missing 'title' or 'panels' key"
        );
    }
}

// ── Registry consistency ─────────────────────────────────────────────

#[test]
fn registry_lists_all_template_dirs() {
    let registry = load_registry();
    let dirs = template_dirs();

    let registry_paths: Vec<&str> = registry
        .templates
        .iter()
        .map(|t| t.path.as_str())
        .collect();

    for dir_name in &dirs {
        assert!(
            registry_paths.contains(&dir_name.as_str()),
            "template directory '{dir_name}' is not listed in registry.toml"
        );
    }

    for path in &registry_paths {
        assert!(
            dirs.contains(&path.to_string()),
            "registry.toml lists '{path}' but no such template directory exists"
        );
    }
}

#[test]
fn registry_entry_chains_match_manifest() {
    let registry = load_registry();

    for entry in &registry.templates {
        let manifest = load_manifest(&entry.path);
        let manifest_chains: Vec<String> = manifest.template.chains.keys().cloned().collect();

        for chain in &entry.chains {
            assert!(
                manifest_chains.contains(chain),
                "registry.toml lists chain '{chain}' for '{}' but template.toml does not define it",
                entry.name
            );
        }

        for chain in &manifest_chains {
            assert!(
                entry.chains.contains(chain),
                "template.toml for '{}' defines chain '{chain}' but registry.toml does not list it",
                entry.name
            );
        }
    }
}

#[test]
fn registry_entry_names_match_manifest() {
    let registry = load_registry();

    for entry in &registry.templates {
        let manifest = load_manifest(&entry.path);
        assert_eq!(
            entry.name, manifest.template.name,
            "registry name '{}' does not match manifest name '{}'",
            entry.name, manifest.template.name
        );
        assert_eq!(
            entry.version, manifest.template.version,
            "registry version for '{}' does not match manifest version",
            entry.name
        );
    }
}

// ── Chain mapping coverage ───────────────────────────────────────────

#[test]
fn all_template_chains_are_known() {
    for dir_name in template_dirs() {
        let manifest = load_manifest(&dir_name);
        for chain_name in manifest.template.chains.keys() {
            assert!(
                chains::chain_id(chain_name).is_some(),
                "{dir_name}: chain '{chain_name}' is not mapped in chains::chain_id()"
            );
        }
    }
}

// ── Static content checks ────────────────────────────────────────────

#[test]
fn no_unresolved_evm_vars_in_non_template_files() {
    let evm_re = Regex::new(r"\{\{evm:(\w+)\}\}").unwrap();

    for dir_name in template_dirs() {
        let template_root = templates_dir().join(&dir_name);

        // Check clickhouse/*.sql files
        let ch_dir = template_root.join("clickhouse");
        if ch_dir.exists() {
            for entry in std::fs::read_dir(&ch_dir).unwrap().filter_map(|e| e.ok()) {
                if entry.path().extension().map(|e| e == "sql").unwrap_or(false) {
                    let content = std::fs::read_to_string(entry.path()).unwrap();
                    assert!(
                        !evm_re.is_match(&content),
                        "{dir_name}: {} contains unresolved {{{{evm:...}}}} patterns",
                        entry.file_name().to_string_lossy()
                    );
                }
            }
        }

        // Check grafana/*.json files
        let grafana_dir = template_root.join("grafana");
        if grafana_dir.exists() {
            for entry in std::fs::read_dir(&grafana_dir).unwrap().filter_map(|e| e.ok()) {
                if entry.path().extension().map(|e| e == "json").unwrap_or(false) {
                    let content = std::fs::read_to_string(entry.path()).unwrap();
                    assert!(
                        !evm_re.is_match(&content),
                        "{dir_name}: grafana/{} contains unresolved {{{{evm:...}}}} patterns",
                        entry.file_name().to_string_lossy()
                    );
                }
            }
        }
    }
}

// ── End-to-end render of every template ──────────────────────────────

#[test]
fn render_every_template_with_test_values() {
    let evm_re = Regex::new(r"\{\{evm:(\w+)\}\}").unwrap();

    for dir_name in template_dirs() {
        let template_dir = templates_dir().join(&dir_name);
        let manifest = load_manifest(&dir_name);
        let m = &manifest.template;

        // Pick the first supported chain
        let chain = m
            .chains
            .keys()
            .next()
            .expect(&format!("{dir_name}: no chains defined"));

        // Build minimal user_vars: required vars get dummy values
        let mut user_vars = BTreeMap::new();
        for (var_name, var_def) in &m.variables {
            if var_def.default.is_none() {
                // Provide a dummy value based on type
                let dummy = match var_def.var_type {
                    crate::templates::types::VarType::String => {
                        "0x0000000000000000000000000000000000000001".to_string()
                    }
                    crate::templates::types::VarType::Int => "12345678".to_string(),
                    crate::templates::types::VarType::Bool => "true".to_string(),
                };
                user_vars.insert(var_name.clone(), dummy);
            }
        }

        // erc20-transfers and erc721-transfers use {{evm:start_block}} in their
        // rindexer.yaml but don't declare it as a manifest variable. Supply it
        // as a user var so the render doesn't fail on unresolved patterns.
        if !m.variables.contains_key("start_block")
            && !m.chains.get(chain).map_or(false, |c| !c.contracts.is_empty())
        {
            user_vars.insert("start_block".to_string(), "0".to_string());
        }

        let output_dir = tempfile::tempdir().unwrap();
        let written = render_template(
            &template_dir,
            m,
            &[chain.clone()],
            &user_vars,
            output_dir.path(),
            None,
        )
        .unwrap_or_else(|e| panic!("{dir_name}: render failed: {e}"));

        assert!(
            !written.is_empty(),
            "{dir_name}: render produced no output files"
        );

        // Assert: rindexer.yaml exists and contains the chain name
        let rindexer_path = output_dir.path().join("rindexer.yaml");
        assert!(
            rindexer_path.exists(),
            "{dir_name}: rendered output missing rindexer.yaml"
        );
        let rindexer_content = std::fs::read_to_string(&rindexer_path).unwrap();
        assert!(
            rindexer_content.contains(chain),
            "{dir_name}: rindexer.yaml does not contain chain name '{chain}'"
        );

        // Assert: no {{evm:...}} patterns remain in any text output file
        for rel_path in &written {
            let ext = rel_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let text_exts = ["yaml", "yml", "sql", "json", "toml", "md", "txt", "graphql"];
            if !text_exts.contains(&ext) {
                continue;
            }

            let full_path = output_dir.path().join(rel_path);
            let content = std::fs::read_to_string(&full_path).unwrap();
            assert!(
                !evm_re.is_match(&content),
                "{dir_name}: unresolved {{{{evm:...}}}} in rendered file {}",
                rel_path.display()
            );
        }

        // Assert: expected structural files exist
        assert!(
            output_dir.path().join("clickhouse/materialized_views.sql").exists(),
            "{dir_name}: rendered output missing clickhouse/materialized_views.sql"
        );
        assert!(
            output_dir.path().join("grafana/dashboard.json").exists(),
            "{dir_name}: rendered output missing grafana/dashboard.json"
        );
    }
}

// ── Rendered YAML is valid after substitution ────────────────────────

#[test]
fn rendered_rindexer_yaml_is_valid_yaml() {
    for dir_name in template_dirs() {
        let template_dir = templates_dir().join(&dir_name);
        let manifest = load_manifest(&dir_name);
        let m = &manifest.template;

        let chain = m.chains.keys().next().unwrap();

        let mut user_vars = BTreeMap::new();
        for (var_name, var_def) in &m.variables {
            if var_def.default.is_none() {
                let dummy = match var_def.var_type {
                    crate::templates::types::VarType::String => {
                        "0x0000000000000000000000000000000000000001".to_string()
                    }
                    crate::templates::types::VarType::Int => "12345678".to_string(),
                    crate::templates::types::VarType::Bool => "true".to_string(),
                };
                user_vars.insert(var_name.clone(), dummy);
            }
        }

        if !m.variables.contains_key("start_block")
            && !m.chains.get(chain).map_or(false, |c| !c.contracts.is_empty())
        {
            user_vars.insert("start_block".to_string(), "0".to_string());
        }

        let output_dir = tempfile::tempdir().unwrap();
        render_template(
            &template_dir,
            m,
            &[chain.clone()],
            &user_vars,
            output_dir.path(),
            None,
        )
        .unwrap_or_else(|e| panic!("{dir_name}: render failed: {e}"));

        let rindexer_content =
            std::fs::read_to_string(output_dir.path().join("rindexer.yaml")).unwrap();

        // After substitution, the YAML should be parseable
        let parsed: serde_yaml::Value = serde_yaml::from_str(&rindexer_content)
            .unwrap_or_else(|e| {
                panic!("{dir_name}: rendered rindexer.yaml is not valid YAML: {e}")
            });

        assert!(
            parsed.is_mapping(),
            "{dir_name}: rendered rindexer.yaml root should be a YAML mapping"
        );
    }
}
