use std::fs;
use std::path::{Path, PathBuf};

use super::profiles::ResourceSet;
use crate::error::{CliError, Result};

pub(crate) const DEFAULT_FORK_RPC: &str = "https://ethereum-rpc.publicnode.com";

pub(crate) const ERC20_ABI: &str = r#"[
  {
    "anonymous": false,
    "inputs": [
      { "indexed": true, "name": "from", "type": "address" },
      { "indexed": true, "name": "to", "type": "address" },
      { "indexed": false, "name": "value", "type": "uint256" }
    ],
    "name": "Transfer",
    "type": "event"
  },
  {
    "anonymous": false,
    "inputs": [
      { "indexed": true, "name": "owner", "type": "address" },
      { "indexed": true, "name": "spender", "type": "address" },
      { "indexed": false, "name": "value", "type": "uint256" }
    ],
    "name": "Approval",
    "type": "event"
  }
]"#;

fn default_rindexer_yaml(fresh: bool, chain_id: u64) -> String {
    let (network_name, effective_chain_id) = if fresh {
        ("anvil", 31337)
    } else {
        ("ethereum", chain_id)
    };

    format!(
        "name: local-indexer\nproject_type: no-code\nnetworks:\n  - name: {network_name}\n    chain_id: {effective_chain_id}\n    rpc: http://local-erpc:4000/local/evm/{effective_chain_id}\nstorage:\n  clickhouse:\n    enabled: true\ncontracts:\n  - name: USDC\n    details:\n      - network: {network_name}\n        address: \"0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48\"\n        start_block: \"0\"\n    abi: ./abis/ERC20.json\n"
    )
}

pub(crate) fn ensure_default_config_bundle(fresh: bool, chain_id: u64) -> Result<PathBuf> {
    let config_dir = PathBuf::from("config");
    let abis_dir = config_dir.join("abis");
    let rindexer_path = config_dir.join("rindexer.yaml");
    let erc20_abi_path = abis_dir.join("ERC20.json");

    fs::create_dir_all(&abis_dir).map_err(|source| CliError::Io {
        source,
        path: abis_dir.clone(),
    })?;

    if !rindexer_path.is_file() {
        fs::write(&rindexer_path, default_rindexer_yaml(fresh, chain_id)).map_err(|source| {
            CliError::Io {
                source,
                path: rindexer_path.clone(),
            }
        })?;
    }

    if !erc20_abi_path.is_file() {
        fs::write(&erc20_abi_path, ERC20_ABI).map_err(|source| CliError::Io {
            source,
            path: erc20_abi_path.clone(),
        })?;
    }

    Ok(rindexer_path)
}

pub(crate) fn generate_kind_config(persist: bool) -> Result<String> {
    let base_mappings = r#"kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
  - role: control-plane
    extraPortMappings:
      - containerPort: 30545
        hostPort: 8545
        protocol: TCP
      - containerPort: 30400
        hostPort: 4000
        protocol: TCP
      - containerPort: 30123
        hostPort: 8123
        protocol: TCP
      - containerPort: 31808
        hostPort: 18080
        protocol: TCP
      - containerPort: 30300
        hostPort: 3000
        protocol: TCP"#;

    if !persist {
        return Ok(base_mappings.to_string());
    }

    let data_dir = data_dir();
    Ok(format!(
        "{base_mappings}\n    extraMounts:\n      - hostPath: {data_dir}\n        containerPath: /var/local-data"
    ))
}

pub(crate) fn generate_erpc_values(chain_id: u64, res: &ResourceSet) -> String {
    format!(
        r#"fullnameOverride: local-erpc
service:
  type: NodePort
  nodePort: 30400
  port: 4000
resources:
  requests:
    cpu: {cpu_req}
    memory: {mem_req}
  limits:
    cpu: {cpu_lim}
    memory: {mem_lim}
config:
  erpcYaml: |
    logLevel: debug
    server:
      listenV4: true
      httpHostV4: 0.0.0.0
      httpPort: 4000
    projects:
      - id: local
        networks:
          - architecture: evm
            evm:
              chainId: {chain_id}
        networkDefaults:
          failsafe:
            retry:
              maxAttempts: 3
              delay: 500ms
        upstreams:
          - id: anvil
            endpoint: http://local-anvil:8545
            type: evm
"#,
        cpu_req = res.cpu_req,
        mem_req = res.mem_req,
        cpu_lim = res.cpu_lim,
        mem_lim = res.mem_lim,
    )
}

pub(crate) fn generate_erpc_values_mainnet(
    chain_id: u64,
    rpc_url: &str,
    res: &ResourceSet,
) -> String {
    format!(
        r#"fullnameOverride: local-erpc
service:
  type: NodePort
  nodePort: 30400
  port: 4000
resources:
  requests:
    cpu: {cpu_req}
    memory: {mem_req}
  limits:
    cpu: {cpu_lim}
    memory: {mem_lim}
config:
  erpcYaml: |
    logLevel: debug
    server:
      listenV4: true
      httpHostV4: 0.0.0.0
      httpPort: 4000
    projects:
      - id: local
        networks:
          - architecture: evm
            evm:
              chainId: {chain_id}
        networkDefaults:
          failsafe:
            retry:
              maxAttempts: 3
              delay: 500ms
        upstreams:
          - id: mainnet-rpc
            endpoint: {rpc_url}
            type: evm
"#,
        cpu_req = res.cpu_req,
        mem_req = res.mem_req,
        cpu_lim = res.cpu_lim,
        mem_lim = res.mem_lim,
    )
}

pub(crate) fn generate_indexer_values(
    rindexer_yaml: &str,
    abis: &[(String, String)],
    chain_id: u64,
    res: &ResourceSet,
    custom_erpc: bool,
) -> String {
    let indented_yaml = rindexer_yaml
        .lines()
        .map(|l| format!("    {l}"))
        .collect::<Vec<_>>()
        .join("\n");

    let abis_yaml = abis
        .iter()
        .map(|(name, content)| format!("    {name}: '{content}'"))
        .collect::<Vec<_>>()
        .join("\n");

    // When using a custom eRPC config, the rindexer YAML handles per-chain
    // routing via ${RPC_URL}/<project>/evm/<chain_id>, so RPC_URL should be
    // just the base URL. With the default local eRPC (single-chain, project
    // "local"), we include the full path.
    let rpc_url = if custom_erpc {
        "http://local-erpc:4000".to_string()
    } else {
        format!("http://local-erpc:4000/local/evm/{chain_id}")
    };

    format!(
        r#"fullnameOverride: local-indexer
storageBackend: clickhouse
secretsMode: inline
rpcUrl: {rpc_url}
clickhouse:
  url: http://clickhouse:8123
  user: default
  db: default
  password: local-dev
service:
  type: NodePort
  nodePort: 31808
  port: 8080
resources:
  requests:
    cpu: {cpu_req}
    memory: {mem_req}
  limits:
    cpu: {cpu_lim}
    memory: {mem_lim}
config:
  rindexerYaml: |
{indented_yaml}
  abis:
{abis_yaml}
"#,
        cpu_req = res.cpu_req,
        mem_req = res.mem_req,
        cpu_lim = res.cpu_lim,
        mem_lim = res.mem_lim,
    )
}

/// Load user's rindexer.yaml and discover ABIs from the sibling abis/ directory.
/// Returns (yaml_content, vec of (filename, json_content)).
pub(crate) fn load_user_rindexer_config(
    config_path: &Path,
) -> Result<(String, Vec<(String, String)>)> {
    let yaml = fs::read_to_string(config_path).map_err(|_| CliError::RindexerConfigNotFound {
        path: config_path.display().to_string(),
    })?;

    let abis_dir = config_path.parent().unwrap_or(Path::new(".")).join("abis");

    let mut abis = Vec::new();
    if abis_dir.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(&abis_dir)
            .map_err(|source| CliError::Io {
                source,
                path: abis_dir.clone(),
            })?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "json")
                    .unwrap_or(false)
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let content = fs::read_to_string(entry.path()).map_err(|source| CliError::Io {
                source,
                path: entry.path(),
            })?;
            abis.push((name, content.trim().to_string()));
        }
    }

    Ok((yaml, abis))
}

pub(crate) fn data_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    format!("{home}/.evm-cloud/local-data")
}

/// Returns the path to `erpc.yaml` if it exists.
/// Checks explicit config dir first, then falls back to `config/erpc.yaml`.
pub(crate) fn resolve_erpc_config_path(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = explicit {
        // Explicit path may be a directory or a file.
        if dir.is_dir() {
            let candidate = dir.join("erpc.yaml");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        // If explicit points to a file, check its sibling erpc.yaml.
        if dir.is_file() {
            if let Some(parent) = dir.parent() {
                let candidate = parent.join("erpc.yaml");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    let p = PathBuf::from("config").join("erpc.yaml");
    if p.is_file() { Some(p) } else { None }
}

/// Read the contents of a user-provided erpc.yaml.
pub(crate) fn load_user_erpc_config(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|source| CliError::Io {
        source,
        path: path.to_path_buf(),
    })
}

/// Parse the first `chainId:` value found in an eRPC yaml config.
/// Best-effort: returns None if not found or unparseable.
pub(crate) fn parse_chain_id_from_erpc(content: &str) -> Option<u64> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("chainId:") {
            if let Ok(id) = rest.trim().parse::<u64>() {
                return Some(id);
            }
        }
    }
    None
}

/// Generate eRPC Helm values using user-provided erpc.yaml content.
/// Wraps the user's yaml in the Helm chart envelope (service, resources, etc.).
pub(crate) fn generate_erpc_values_from_file(content: &str, res: &ResourceSet) -> String {
    let indented = content
        .lines()
        .map(|l| format!("    {l}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"fullnameOverride: local-erpc
service:
  type: NodePort
  nodePort: 30400
  port: 4000
resources:
  requests:
    cpu: {cpu_req}
    memory: {mem_req}
  limits:
    cpu: {cpu_lim}
    memory: {mem_lim}
config:
  erpcYaml: |
{indented}
"#,
        cpu_req = res.cpu_req,
        mem_req = res.mem_req,
        cpu_lim = res.cpu_lim,
        mem_lim = res.mem_lim,
    )
}

pub(crate) fn resolve_config_path(explicit: Option<&Path>) -> Option<std::path::PathBuf> {
    if let Some(p) = explicit {
        if p.is_file() {
            return Some(p.to_path_buf());
        }

        if p.is_dir() {
            let candidate = p.join("rindexer.yaml");
            if candidate.is_file() {
                return Some(candidate);
            }
            return None;
        }

        return None;
    }

    let preferred = std::path::PathBuf::from("config").join("rindexer.yaml");
    if preferred.is_file() {
        return Some(preferred);
    }

    None
}
