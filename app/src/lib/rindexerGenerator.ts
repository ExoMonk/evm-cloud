/**
 * Full rindexer.yaml generator.
 *
 * Generates valid rindexer no-code YAML config from builder state + template data.
 * Matches the format produced by cli/src/templates/render.rs
 */

import type { BuilderState } from "./configSchema.ts";
import { TEMPLATES, type TemplateDef } from "./templateRegistry.ts";
import { getChainId, STARTER_CONTRACTS } from "./chains.ts";

export function generateRindexerYaml(state: BuilderState): string {
  const template = TEMPLATES.find((t) => t.name === state.selectedTemplate);

  if (template) {
    return generateFromTemplate(state, template);
  }
  return generateBlank(state);
}

// ---------------------------------------------------------------------------
// Template-based generation
// ---------------------------------------------------------------------------

function generateFromTemplate(state: BuilderState, template: TemplateDef): string {
  const dbName = state.databaseName || "rindexer";
  const storageBackend = inferStorage(state);
  const lines: string[] = [];

  // Header
  lines.push(`name: ${dbName}`);
  lines.push("project_type: no-code");

  // Networks
  lines.push("networks:");
  for (const chain of state.chains) {
    const chainId = getChainId(chain);
    lines.push(`  - name: ${chain}`);
    lines.push(`    chain_id: ${chainId}`);
    lines.push(`    rpc: \${RPC_URL}/main/evm/${chainId}`);
  }

  // Storage
  lines.push("storage:");
  lines.push(`  ${storageBackend}:`);
  lines.push("    enabled: true");

  // Contracts — derive from template chain configs
  lines.push("contracts:");

  // Group contracts by contract name across chains
  const contractNames = new Set<string>();
  for (const chainConfig of template.chains) {
    for (const contractName of Object.keys(chainConfig.contracts)) {
      contractNames.add(contractName);
    }
  }

  if (contractNames.size > 0) {
    // Protocol templates (uniswap-v4, aave-v3, etc.) — real contracts
    for (const contractName of contractNames) {
      lines.push(`  - name: ${contractName}`);
      lines.push("    details:");

      for (const chain of state.chains) {
        const chainConfig = template.chains.find((c) => c.chain === chain);
        const contract = chainConfig?.contracts[contractName];
        if (contract) {
          lines.push(`      - network: ${chain}`);
          lines.push(`        address: "${contract.address}"`);
          lines.push(`        start_block: "${contract.startBlock}"`);
        }
      }

      // ABI file reference
      lines.push(`    abi: ./abis/${contractName}.json`);

      // Events from template
      if (template.events.length > 0) {
        lines.push("    include_events:");
        for (const event of template.events) {
          // Extract just the event name (before the parenthesis)
          const eventName = event.split("(")[0];
          lines.push(`      - ${eventName}`);
        }
      }
    }
  } else {
    // Token/NFT templates (erc20, erc721) — user-provided address via variables
    const address = resolveVariableAddress(state, template);
    const contractDisplayName = resolveContractName(state, template);

    lines.push(`  - name: ${contractDisplayName}`);
    lines.push("    details:");
    for (const chain of state.chains) {
      lines.push(`      - network: ${chain}`);
      lines.push(`        address: "${address}"`);
      lines.push(`        start_block: "${resolveStartBlock(state, template)}"`);
    }

    const abiName = template.category === "nft" ? "ERC721" : "ERC20";
    lines.push(`    abi: ./abis/${abiName}.json`);

    if (template.events.length > 0) {
      lines.push("    include_events:");
      for (const event of template.events) {
        const eventName = event.split("(")[0];
        lines.push(`      - ${eventName}`);
      }
    }
  }

  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// Blank project (no template)
// ---------------------------------------------------------------------------

function generateBlank(state: BuilderState): string {
  const dbName = state.databaseName || "rindexer";
  const storageBackend = inferStorage(state);
  const lines: string[] = [];

  lines.push(`name: ${dbName}`);
  lines.push("project_type: no-code");

  // Networks
  lines.push("networks:");
  for (const chain of state.chains) {
    const chainId = getChainId(chain);
    lines.push(`  - name: ${chain}`);
    lines.push(`    chain_id: ${chainId}`);
    lines.push(`    rpc: \${RPC_URL}/main/evm/${chainId}`);
  }

  // Storage
  lines.push("storage:");
  lines.push(`  ${storageBackend}:`);
  lines.push("    enabled: true");

  // Starter contract — USDC on first chain
  const firstChain = state.chains[0];
  const starter = firstChain ? STARTER_CONTRACTS[firstChain] : null;

  lines.push("contracts:");
  if (starter) {
    lines.push(`  - name: ${starter.name}`);
    lines.push("    details:");
    for (const chain of state.chains) {
      const sc = STARTER_CONTRACTS[chain];
      if (sc) {
        lines.push(`      - network: ${chain}`);
        lines.push(`        address: "${sc.address}"`);
        lines.push('        start_block: "0"');
      }
    }
    lines.push("    abi: ./abis/ERC20.json");
    lines.push("    include_events:");
    lines.push("      - Transfer");
  } else {
    lines.push("  # Add your contract definitions below");
    lines.push("  # See: https://rindexer.xyz/docs/start/yaml-config");
    lines.push("  []");
  }

  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function inferStorage(state: BuilderState): "clickhouse" | "postgres" {
  if (state.databaseProfile.includes("clickhouse")) return "clickhouse";
  return "postgres";
}

function resolveVariableAddress(state: BuilderState, template: TemplateDef): string {
  // Try template-specific variable names
  const vars = state.templateVariables;
  if (vars.token_address) return vars.token_address;
  if (vars.nft_address) return vars.nft_address;
  if (vars.spoke_address) return vars.spoke_address;

  // Check if template has a default
  if (template.variables) {
    for (const [, def] of Object.entries(template.variables)) {
      if (def.type === "string" && def.default) return def.default;
    }
  }

  return "0x0000000000000000000000000000000000000000";
}

function resolveContractName(state: BuilderState, template: TemplateDef): string {
  const vars = state.templateVariables;
  if (vars.token_symbol) return vars.token_symbol;
  if (vars.collection_name) return vars.collection_name;
  if (template.category === "nft") return "NFT";
  return "Token";
}

function resolveStartBlock(state: BuilderState, template: TemplateDef): string {
  const vars = state.templateVariables;
  if (vars.start_block) return vars.start_block;
  if (vars.spoke_start_block) return vars.spoke_start_block;

  // Check template variable defaults
  if (template.variables?.start_block?.default) return template.variables.start_block.default;

  return "0";
}
