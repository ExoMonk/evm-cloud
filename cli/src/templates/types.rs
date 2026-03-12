use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Wraps the top-level `[template]` table in `template.toml`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TemplateManifestFile {
    pub(crate) template: TemplateManifest,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TemplateManifest {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: String,
    pub(crate) category: TemplateCategory,
    #[serde(default)]
    pub(crate) min_evm_cloud_version: Option<String>,
    pub(crate) chains: BTreeMap<String, ChainConfig>,
    #[serde(default)]
    pub(crate) variables: BTreeMap<String, VariableDef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TemplateCategory {
    Dex,
    Lending,
    Token,
    Nft,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChainConfig {
    pub(crate) contracts: BTreeMap<String, ContractDef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ContractDef {
    pub(crate) address: String,
    pub(crate) start_block: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct VariableDef {
    #[serde(rename = "type")]
    pub(crate) var_type: VarType,
    #[serde(default)]
    pub(crate) default: Option<toml::Value>,
    pub(crate) description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum VarType {
    String,
    Int,
    Bool,
}

/// Parsed `templates/registry.toml`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RegistryFile {
    pub(crate) registry: RegistryMeta,
    pub(crate) templates: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RegistryMeta {
    #[allow(dead_code)]
    pub(crate) version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RegistryEntry {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) version: String,
    pub(crate) description: String,
    pub(crate) chains: Vec<String>,
    #[allow(dead_code)]
    pub(crate) category: String,
}

/// Written to `.evm-cloud/template-lock.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct TemplateLock {
    pub(crate) templates: Vec<TemplateLockEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct TemplateLockEntry {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) chains: Vec<String>,
    pub(crate) init_date: String,
    pub(crate) variables: BTreeMap<String, toml::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn templates_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../templates")
    }

    #[test]
    fn parse_registry_toml() {
        let content =
            std::fs::read_to_string(templates_dir().join("registry.toml")).unwrap();
        let registry: RegistryFile = toml::from_str(&content).unwrap();

        assert_eq!(registry.registry.version, 1);
        assert_eq!(registry.templates.len(), 5);

        let names: Vec<&str> = registry.templates.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"erc20-transfers"));
        assert!(names.contains(&"erc721-transfers"));
        assert!(names.contains(&"uniswap-v4"));
        assert!(names.contains(&"aave-v3"));
        assert!(names.contains(&"aave-v4"));

        // Verify chain lists
        let uniswap = registry.templates.iter().find(|t| t.name == "uniswap-v4").unwrap();
        assert_eq!(uniswap.chains, vec!["ethereum", "arbitrum", "base"]);

        let aave_v4 = registry.templates.iter().find(|t| t.name == "aave-v4").unwrap();
        assert_eq!(aave_v4.chains, vec!["ethereum"]);
    }

    #[test]
    fn parse_erc20_transfers_manifest() {
        let content = std::fs::read_to_string(
            templates_dir().join("erc20-transfers/template.toml"),
        )
        .unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let m = &manifest_file.template;

        assert_eq!(m.name, "erc20-transfers");
        assert_eq!(m.version, "0.1.0");
        assert!(matches!(m.category, TemplateCategory::Token));
        assert_eq!(m.chains.len(), 5);
        assert!(m.chains.contains_key("ethereum"));
        assert!(m.chains.contains_key("polygon"));

        // token_address is required (no default)
        let token_addr = m.variables.get("token_address").unwrap();
        assert!(token_addr.default.is_none());
        assert!(matches!(token_addr.var_type, VarType::String));

        // token_symbol has a default
        let token_sym = m.variables.get("token_symbol").unwrap();
        assert_eq!(
            token_sym.default.as_ref().unwrap(),
            &toml::Value::String("TOKEN".to_string())
        );
    }

    #[test]
    fn parse_aave_v4_manifest() {
        let content = std::fs::read_to_string(
            templates_dir().join("aave-v4/template.toml"),
        )
        .unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let m = &manifest_file.template;

        assert_eq!(m.name, "aave-v4");
        assert!(matches!(m.category, TemplateCategory::Lending));
        assert_eq!(m.chains.len(), 1);
        assert!(m.chains.contains_key("ethereum"));

        // spoke_address is required
        let spoke = m.variables.get("spoke_address").unwrap();
        assert!(spoke.default.is_none());

        // spoke_start_block has default 0
        let spoke_block = m.variables.get("spoke_start_block").unwrap();
        assert_eq!(
            spoke_block.default.as_ref().unwrap(),
            &toml::Value::Integer(0)
        );
    }

    #[test]
    fn parse_uniswap_v4_manifest() {
        let content = std::fs::read_to_string(
            templates_dir().join("uniswap-v4/template.toml"),
        )
        .unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let m = &manifest_file.template;

        assert_eq!(m.name, "uniswap-v4");
        assert!(matches!(m.category, TemplateCategory::Dex));
        assert_eq!(m.chains.len(), 3);
        assert!(m.chains.contains_key("ethereum"));
        assert!(m.chains.contains_key("arbitrum"));
        assert!(m.chains.contains_key("base"));

        // Each chain has a PoolManager contract
        for (chain_name, chain_cfg) in &m.chains {
            let pm = chain_cfg.contracts.get("PoolManager").unwrap_or_else(|| {
                panic!("{chain_name} missing PoolManager contract")
            });
            assert!(
                pm.address.starts_with("0x"),
                "{chain_name} PoolManager address should start with 0x"
            );
            assert!(pm.start_block > 0, "{chain_name} PoolManager start_block should be > 0");
        }
    }

    #[test]
    fn parse_erc721_transfers_manifest() {
        let content = std::fs::read_to_string(
            templates_dir().join("erc721-transfers/template.toml"),
        )
        .unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let m = &manifest_file.template;

        assert_eq!(m.name, "erc721-transfers");
        assert!(matches!(m.category, TemplateCategory::Nft));
        assert_eq!(m.chains.len(), 5);

        // nft_address is required
        assert!(m.variables.get("nft_address").unwrap().default.is_none());
        // collection_name has default "NFT"
        assert_eq!(
            m.variables.get("collection_name").unwrap().default.as_ref().unwrap(),
            &toml::Value::String("NFT".to_string())
        );
    }

    #[test]
    fn parse_aave_v3_manifest() {
        let content = std::fs::read_to_string(
            templates_dir().join("aave-v3/template.toml"),
        )
        .unwrap();
        let manifest_file: TemplateManifestFile = toml::from_str(&content).unwrap();
        let m = &manifest_file.template;

        assert_eq!(m.name, "aave-v3");
        assert!(matches!(m.category, TemplateCategory::Lending));
        assert_eq!(m.chains.len(), 5);

        // Each chain has an AaveV3Pool contract
        let eth_pool = m.chains.get("ethereum").unwrap()
            .contracts.get("AaveV3Pool").unwrap();
        assert_eq!(eth_pool.address, "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2");
        assert_eq!(eth_pool.start_block, 16291127);
    }

    #[test]
    fn template_lock_round_trip() {
        let lock = TemplateLock {
            templates: vec![TemplateLockEntry {
                name: "erc20-transfers".to_string(),
                version: "0.1.0".to_string(),
                chains: vec!["polygon".to_string()],
                init_date: "2025-01-15".to_string(),
                variables: BTreeMap::from([
                    ("token_address".to_string(), toml::Value::String("0xabc".to_string())),
                    ("token_symbol".to_string(), toml::Value::String("USDC".to_string())),
                ]),
            }],
        };

        let serialized = toml::to_string(&lock).unwrap();
        let deserialized: TemplateLock = toml::from_str(&serialized).unwrap();
        assert_eq!(lock, deserialized);
    }

    #[test]
    fn template_lock_multiple_entries_round_trip() {
        let lock = TemplateLock {
            templates: vec![
                TemplateLockEntry {
                    name: "erc20-transfers".to_string(),
                    version: "0.1.0".to_string(),
                    chains: vec!["polygon".to_string(), "ethereum".to_string()],
                    init_date: "2025-01-15".to_string(),
                    variables: BTreeMap::from([
                        ("token_address".to_string(), toml::Value::String("0xabc".to_string())),
                    ]),
                },
                TemplateLockEntry {
                    name: "uniswap-v4".to_string(),
                    version: "0.1.0".to_string(),
                    chains: vec!["ethereum".to_string()],
                    init_date: "2025-02-01".to_string(),
                    variables: BTreeMap::new(),
                },
            ],
        };

        let serialized = toml::to_string(&lock).unwrap();
        let deserialized: TemplateLock = toml::from_str(&serialized).unwrap();
        assert_eq!(lock, deserialized);
    }
}
