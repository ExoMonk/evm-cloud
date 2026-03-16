/**
 * Full erpc.yaml generator.
 *
 * Generates valid eRPC config matching the CLI's output format.
 * Always includes repository://evm-public-endpoints.erpc.cloud as default upstream.
 * User-provided RPCs are added as additional per-chain upstreams.
 */

import type { BuilderState } from "./configSchema.ts";
import { getChainId, DEFAULT_RPCS } from "./chains.ts";

export function generateErpcYaml(state: BuilderState): string {
  const lines: string[] = [];

  // Server config
  lines.push("logLevel: warn");
  lines.push("server:");
  lines.push("  listenV4: true");
  lines.push("  httpHostV4: 0.0.0.0");
  lines.push("  httpPort: 4000");
  lines.push("");

  // Project
  lines.push("projects:");
  lines.push("  - id: main");

  // Networks — one per chain
  lines.push("    networks:");
  for (const chain of state.chains) {
    const chainId = getChainId(chain);
    lines.push("      - architecture: evm");
    lines.push("        evm:");
    lines.push(`          chainId: ${chainId}`);
  }

  // Upstreams
  lines.push("    upstreams:");

  // Per-chain user-provided RPCs (if any)
  const hasMultipleChains = state.chains.length > 1;
  for (const chain of state.chains) {
    const userRpc = state.rpcEndpoints[chain];
    const rpc = userRpc || DEFAULT_RPCS[chain];
    if (rpc) {
      const chainId = getChainId(chain);
      lines.push(`      - id: ${chain}-primary`);
      lines.push(`        endpoint: ${rpc}`);
      lines.push("        type: evm");
      if (hasMultipleChains) {
        lines.push("        evm:");
        lines.push(`          chainId: ${chainId}`);
      }
    }
  }

  // Universal public endpoint fallback — works for all EVM chains
  lines.push("      - id: evm-public");
  lines.push("        endpoint: repository://evm-public-endpoints.erpc.cloud");
  lines.push("        type: evm");

  return lines.join("\n");
}
