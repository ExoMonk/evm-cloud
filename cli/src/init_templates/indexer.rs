use crate::init_answers::{DatabaseProfile, InitAnswers};

pub(crate) fn render_rindexer_yaml(answers: &InitAnswers) -> String {
    let first_chain = answers
        .chains
        .first()
        .cloned()
        .unwrap_or_else(|| "ethereum".to_string());

    let chain_id = chain_id_for(&first_chain).unwrap_or(1);

    let storage_block = if uses_postgres(answers) {
        "  postgres:\n    enabled: true"
    } else {
        "  clickhouse:\n    enabled: true"
    };

    let (contract_name, contract_address) = starter_contract_for(chain_id);

    format!(
        r#"name: {}
project_type: no-code
networks:
  - name: {}
    chain_id: {}
    rpc: ${{RPC_URL}}/main/evm/{}
storage:
{}
contracts:
  - name: {}
    details:
      - network: {}
        address: "{}"
    abi: ./abis/ERC20.json
    include_events:
      - Transfer
"#,
        answers.project_name,
        first_chain,
        chain_id,
        chain_id,
        storage_block,
        contract_name,
        first_chain,
        contract_address,
    )
}

pub(crate) fn render_erpc_yaml(answers: &InitAnswers) -> String {
    let first_chain = answers
        .chains
        .first()
        .cloned()
        .unwrap_or_else(|| "ethereum".to_string());

    let chain_id = chain_id_for(&first_chain).unwrap_or(1);

    let first_endpoint = answers
        .rpc_endpoints
        .get(&first_chain)
        .cloned()
        .unwrap_or_else(|| "https://ethereum-rpc.publicnode.com".to_string());

    format!(
        "logLevel: warn\nprojects:\n  - id: main\n    networks:\n      - architecture: evm\n        evm:\n          chainId: {}\n    upstreams:\n      - id: primary\n        endpoint: {}\n        type: evm\nserver:\n  listenV4: true\n  httpHostV4: 0.0.0.0\n  httpPort: 4000\n",
        chain_id, first_endpoint
    )
}

/// Minimal ERC20 ABI covering the Transfer event (+ Approval for completeness).
pub(crate) fn erc20_abi_json() -> &'static str {
    r#"[
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
]
"#
}

/// Returns (contract_name, contract_address) for a starter ERC20 contract on the given chain.
fn starter_contract_for(chain_id: u64) -> (&'static str, &'static str) {
    match chain_id {
        8453 => ("BaseUSDC", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
        42161 => ("ArbUSDC", "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
        10 => ("OpUSDC", "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"),
        137 => ("PolygonUSDC", "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
        _ => ("USDC", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), // Ethereum mainnet
    }
}

fn chain_id_for(chain: &str) -> Option<u64> {
    let normalized = chain.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "ethereum" | "eth" | "mainnet" => Some(1),
        "polygon" | "matic" => Some(137),
        "arbitrum" | "arbitrum_one" => Some(42161),
        "base" => Some(8453),
        "optimism" | "op" => Some(10),
        "hyperliquid" | "hyperliquid_mainnet" | "hyperevm" => Some(999),
        _ => None,
    }
}

fn uses_postgres(answers: &InitAnswers) -> bool {
    matches!(
        answers.database_profile,
        DatabaseProfile::ByodbPostgres | DatabaseProfile::ManagedRds
    )
}
