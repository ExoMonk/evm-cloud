/**
 * Protocol template metadata from templates/registry.toml
 *
 * These are the starting points for the builder.
 * When new templates are added to the registry, add them here.
 */

import type { TemplateCategory } from "./configSchema.ts";

export interface TemplateChainConfig {
  chain: string;
  contracts: Record<string, { address: string; startBlock: number }>;
}

export interface TemplateDef {
  name: string;
  displayName: string;
  description: string;
  category: TemplateCategory;
  chains: TemplateChainConfig[];
  events: string[];
  variables?: Record<string, { type: string; default?: string; description: string }>;
}

export const TEMPLATES: TemplateDef[] = [
  {
    name: "erc20-transfers",
    displayName: "ERC-20 Transfers",
    description: "Track Transfer events for any ERC-20 token across chains.",
    category: "token",
    chains: [
      { chain: "ethereum", contracts: {} },
      { chain: "polygon", contracts: {} },
      { chain: "arbitrum", contracts: {} },
      { chain: "optimism", contracts: {} },
      { chain: "base", contracts: {} },
    ],
    events: ["Transfer(address indexed from, address indexed to, uint256 value)"],
    variables: {
      token_address: {
        type: "string",
        description: "Contract address of the ERC-20 token to index",
      },
    },
  },
  {
    name: "erc721-transfers",
    displayName: "ERC-721 Transfers",
    description: "Track Transfer events for any ERC-721 NFT collection.",
    category: "nft",
    chains: [
      { chain: "ethereum", contracts: {} },
      { chain: "polygon", contracts: {} },
      { chain: "arbitrum", contracts: {} },
      { chain: "optimism", contracts: {} },
      { chain: "base", contracts: {} },
    ],
    events: ["Transfer(address indexed from, address indexed to, uint256 indexed tokenId)"],
    variables: {
      token_address: {
        type: "string",
        description: "Contract address of the ERC-721 collection to index",
      },
    },
  },
  {
    name: "uniswap-v4",
    displayName: "Uniswap V4",
    description: "Swap, liquidity, and pool analytics for Uniswap V4 PoolManager.",
    category: "dex",
    chains: [
      {
        chain: "ethereum",
        contracts: {
          PoolManager: { address: "0x000000000004444c5dc75cB358380D2e3dE08A90", startBlock: 21688329 },
        },
      },
      {
        chain: "arbitrum",
        contracts: {
          PoolManager: { address: "0x000000000004444c5dc75cB358380D2e3dE08A90", startBlock: 299490921 },
        },
      },
      {
        chain: "base",
        contracts: {
          PoolManager: { address: "0x000000000004444c5dc75cB358380D2e3dE08A90", startBlock: 25350988 },
        },
      },
    ],
    events: [
      "Swap(PoolId indexed id, address indexed sender, int128 amount0, int128 amount1, uint160 sqrtPriceX96, uint128 liquidity, int24 tick)",
      "ModifyLiquidity(PoolId indexed id, address indexed sender, int24 tickLower, int24 tickUpper, int256 liquidityDelta, bytes32 salt)",
      "Initialize(PoolId indexed id, Currency indexed currency0, Currency indexed currency1, uint24 fee, int24 tickSpacing, address hooks, uint160 sqrtPriceX96, int24 tick)",
    ],
  },
  {
    name: "aave-v3",
    displayName: "Aave V3",
    description: "Supply, borrow, repay, and liquidation analytics for Aave V3 Pool.",
    category: "lending",
    chains: [
      {
        chain: "ethereum",
        contracts: {
          Pool: { address: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2", startBlock: 16291127 },
        },
      },
      {
        chain: "polygon",
        contracts: {
          Pool: { address: "0x794a61358D6845594F94dc1DB02A252b5b4814aD", startBlock: 25826028 },
        },
      },
      {
        chain: "arbitrum",
        contracts: {
          Pool: { address: "0x794a61358D6845594F94dc1DB02A252b5b4814aD", startBlock: 7742429 },
        },
      },
      {
        chain: "optimism",
        contracts: {
          Pool: { address: "0x794a61358D6845594F94dc1DB02A252b5b4814aD", startBlock: 4365693 },
        },
      },
      {
        chain: "base",
        contracts: {
          Pool: { address: "0xA238Dd80C259a72e81d7e4664a9801593F98d1c5", startBlock: 2357134 },
        },
      },
    ],
    events: [
      "Supply(address indexed reserve, address user, address indexed onBehalfOf, uint256 amount, uint16 indexed referralCode)",
      "Borrow(address indexed reserve, address user, address indexed onBehalfOf, uint256 amount, uint8 interestRateMode, uint256 borrowRate, uint16 indexed referralCode)",
      "Repay(address indexed reserve, address indexed user, address indexed repayer, uint256 amount, bool useATokens)",
      "LiquidationCall(address indexed collateralAsset, address indexed debtAsset, address indexed user, uint256 debtToCover, uint256 liquidatedCollateralAmount, address liquidator, bool receiveAToken)",
    ],
  },
  {
    name: "aave-v4",
    displayName: "Aave V4",
    description: "Hub & Spoke architecture analytics for Aave V4.",
    category: "lending",
    chains: [
      {
        chain: "ethereum",
        contracts: {
          Pool: { address: "0x0000000000000000000000000000000000000000", startBlock: 0 },
        },
      },
    ],
    events: [
      "Supply(address indexed reserve, address user, address indexed onBehalfOf, uint256 amount)",
      "Borrow(address indexed reserve, address user, address indexed onBehalfOf, uint256 amount)",
    ],
  },
];
