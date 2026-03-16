/**
 * Chain name → chain ID mapping.
 * Mirrors cli/src/templates/chains.rs
 */

export const CHAIN_IDS: Record<string, number> = {
  ethereum: 1,
  polygon: 137,
  arbitrum: 42161,
  base: 8453,
  optimism: 10,
  hyperliquid: 999,
};

/** Default public RPC endpoints per chain (free tier, rate-limited) */
export const DEFAULT_RPCS: Record<string, string> = {
  ethereum: "https://ethereum-rpc.publicnode.com",
  polygon: "https://polygon-bor-rpc.publicnode.com",
  arbitrum: "https://arbitrum-one-rpc.publicnode.com",
  base: "https://base-rpc.publicnode.com",
  optimism: "https://optimism-rpc.publicnode.com",
};

/** Default starter contracts (USDC per chain) for blank projects */
export const STARTER_CONTRACTS: Record<string, { name: string; address: string }> = {
  ethereum: { name: "USDC", address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48" },
  polygon: { name: "USDC", address: "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359" },
  arbitrum: { name: "USDC", address: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831" },
  base: { name: "USDC", address: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913" },
  optimism: { name: "USDC", address: "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85" },
};

export function getChainId(chain: string): number {
  return CHAIN_IDS[chain] ?? 1;
}
