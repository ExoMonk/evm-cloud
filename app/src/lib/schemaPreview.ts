/**
 * Schema preview generator — derives ClickHouse/Postgres schema from template + state.
 *
 * Tables are derived from event signatures (same logic as rindexer generate.rs).
 * Materialized views and sample queries are static per template.
 */

import type { BuilderState } from "./configSchema.ts";
import { TEMPLATES, type TemplateDef } from "./templateRegistry.ts";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ColumnDef {
  name: string;
  type: string;
  source: "event" | "rindexer";
}

export interface TableDef {
  fullName: string;
  tableName: string;
  description: string;
  columns: ColumnDef[];
  orderBy: string[];
  engine: string;
  columnCount: number;
}

export interface MaterializedViewDef {
  name: string;
  sourceTable: string;
  targetTable: string;
  description: string;
  aggregation: string;
}

export interface SampleQuery {
  title: string;
  sql: string;
}

export interface SchemaPreviewData {
  databaseName: string;
  tables: TableDef[];
  materializedViews: MaterializedViewDef[];
  sampleQueries: SampleQuery[];
}

// ---------------------------------------------------------------------------
// Standard rindexer columns (always present on every event table)
// ---------------------------------------------------------------------------

const STANDARD_COLUMNS_BEFORE: ColumnDef[] = [
  { name: "contract_address", type: "FixedString(42)", source: "rindexer" },
];

const STANDARD_COLUMNS_AFTER: ColumnDef[] = [
  { name: "tx_hash", type: "FixedString(66)", source: "rindexer" },
  { name: "block_number", type: "UInt64", source: "rindexer" },
  { name: "block_timestamp", type: "Nullable(DateTime)", source: "rindexer" },
  { name: "block_hash", type: "FixedString(66)", source: "rindexer" },
  { name: "network", type: "String", source: "rindexer" },
  { name: "tx_index", type: "UInt64", source: "rindexer" },
  { name: "log_index", type: "UInt64", source: "rindexer" },
];

const STANDARD_ORDER_BY = ["network", "block_number", "tx_hash", "log_index"];

// ---------------------------------------------------------------------------
// Solidity type → ClickHouse type (mirrors rindexer generate.rs)
// ---------------------------------------------------------------------------

function solidityToClickhouse(abiType: string): string {
  const isArray = abiType.endsWith("[]");
  const base = abiType.replace("[]", "");

  let chType: string;

  if (base === "address") chType = "FixedString(42)";
  else if (base === "bool") chType = "Bool";
  else if (base === "string") chType = "String";
  else if (base === "bytes") chType = "String";
  else if (base.startsWith("bytes")) chType = "String";
  else if (base === "PoolId" || base === "PoolKey") chType = "String";
  else if (base === "Currency") chType = "FixedString(42)";
  else if (base.startsWith("uint") || base.startsWith("int")) {
    const isUnsigned = base.startsWith("uint");
    const bits = parseInt(base.replace(/^u?int/, "")) || 256;
    const rounded = bits <= 8 ? 8 : bits <= 16 ? 16 : bits <= 32 ? 32
      : bits <= 64 ? 64 : bits <= 128 ? 128 : 256;
    chType = isUnsigned ? `UInt${rounded}` : `Int${rounded}`;
  }
  else chType = "String";

  return isArray ? `Array(${chType})` : chType;
}

// ---------------------------------------------------------------------------
// camelToSnake (mirrors rindexer convention — underscore before digits)
// ---------------------------------------------------------------------------

function camelToSnake(s: string): string {
  return s
    .replace(/([a-z])([A-Z])/g, "$1_$2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1_$2")
    .replace(/([a-zA-Z])(\d)/g, "$1_$2")
    .toLowerCase();
}

// ---------------------------------------------------------------------------
// Event signature parser
// ---------------------------------------------------------------------------

function parseEventSignature(sig: string): { name: string; params: { type: string; name: string }[] } {
  const match = sig.match(/^(\w+)\((.+)\)$/);
  if (!match) return { name: sig, params: [] };

  const [, name, paramsStr] = match;
  const params = paramsStr.split(",").map((p) => {
    const parts = p.trim().split(/\s+/).filter((t) => t !== "indexed");
    return { type: parts[0], name: parts[parts.length - 1] };
  });

  return { name, params };
}

// ---------------------------------------------------------------------------
// Template MV + sample query data (static per template)
// ---------------------------------------------------------------------------

interface TemplateSchemaMeta {
  materializedViews: MaterializedViewDef[];
  sampleQueries: (dbName: string) => SampleQuery[];
}

const TEMPLATE_SCHEMA_META: Record<string, TemplateSchemaMeta> = {
  "erc20-transfers": {
    materializedViews: [
      { name: "transfer_volume_hourly_mv", sourceTable: "transfer", targetTable: "transfer_volume_hourly", description: "Hourly transfer volume and unique addresses", aggregation: "count, sum(value), uniqExact(from/to)" },
    ],
    sampleQueries: (db) => [
      { title: "Transfers in last 24h", sql: `SELECT count()\nFROM ${db}.transfer\nWHERE block_timestamp > now() - INTERVAL 1 DAY` },
      { title: "Top senders by count", sql: `SELECT from_address, count() as cnt\nFROM ${db}.transfer\nGROUP BY from_address\nORDER BY cnt DESC\nLIMIT 10` },
      { title: "Hourly volume (last 7d)", sql: `SELECT *\nFROM ${db}.transfer_volume_hourly\nWHERE hour > now() - INTERVAL 7 DAY\nORDER BY hour DESC` },
    ],
  },
  "erc721-transfers": {
    materializedViews: [
      { name: "holders_current_mv", sourceTable: "transfer", targetTable: "holders_current", description: "Current owner per token ID (ReplacingMergeTree)", aggregation: "latest to_address by block_number" },
      { name: "activity_daily_mv", sourceTable: "transfer", targetTable: "activity_daily", description: "Daily mint/burn/transfer counts", aggregation: "count, countIf(mint), countIf(burn), uniqExact" },
    ],
    sampleQueries: (db) => [
      { title: "Current holder count", sql: `SELECT count()\nFROM ${db}.holders_current FINAL` },
      { title: "Top holders", sql: `SELECT owner, count() as tokens\nFROM ${db}.holders_current FINAL\nGROUP BY owner\nORDER BY tokens DESC\nLIMIT 10` },
      { title: "Daily activity", sql: `SELECT *\nFROM ${db}.activity_daily\nORDER BY day DESC\nLIMIT 30` },
    ],
  },
  "uniswap-v4": {
    materializedViews: [
      { name: "volume_hourly_mv", sourceTable: "swap", targetTable: "volume_hourly", description: "Hourly swap volume per pool", aggregation: "count, sum(amount_0), sum(amount_1)" },
      { name: "hook_usage_pools_mv", sourceTable: "initialize", targetTable: "hook_usage", description: "Pool count per hook contract", aggregation: "count() per hooks address" },
      { name: "hook_usage_swaps_mv", sourceTable: "swap", targetTable: "hook_usage", description: "Swap count per hook contract", aggregation: "count() via JOIN with initialize" },
    ],
    sampleQueries: (db) => [
      { title: "Recent swaps", sql: `SELECT *\nFROM ${db}.swap\nORDER BY block_number DESC\nLIMIT 100` },
      { title: "Top pools by 24h volume", sql: `SELECT pool_id, swap_count,\n       total_amount_0, total_amount_1\nFROM ${db}.volume_hourly\nWHERE hour > now() - INTERVAL 1 DAY\nORDER BY swap_count DESC\nLIMIT 10` },
      { title: "Hook adoption", sql: `SELECT hooks, pool_count, swap_count\nFROM ${db}.hook_usage\nORDER BY swap_count DESC` },
    ],
  },
  "aave-v3": {
    materializedViews: [
      { name: "net_position_by_asset_mv", sourceTable: "supply/withdraw/borrow/repay", targetTable: "net_position_by_asset", description: "Cumulative supply/borrow positions per reserve", aggregation: "sum(amount) per action type" },
      { name: "liquidation_volume_daily_mv", sourceTable: "liquidation_call", targetTable: "liquidation_volume_daily", description: "Daily liquidation volume by collateral/debt pair", aggregation: "count, sum(debt/collateral), uniqExact" },
      { name: "utilization_hourly_mv", sourceTable: "supply/withdraw/borrow/repay", targetTable: "utilization_hourly", description: "Hourly supply/borrow volume per reserve", aggregation: "sum(amount), count per action type per hour" },
    ],
    sampleQueries: (db) => [
      { title: "Net position per asset (TVL)", sql: `SELECT reserve,\n       total_supplied, total_borrowed\nFROM ${db}.net_position_by_asset\nORDER BY total_supplied DESC` },
      { title: "Recent liquidations", sql: `SELECT *\nFROM ${db}.liquidation_call\nORDER BY block_number DESC\nLIMIT 20` },
      { title: "Hourly utilization", sql: `SELECT *\nFROM ${db}.utilization_hourly\nWHERE hour > now() - INTERVAL 24 HOUR\nORDER BY hour DESC` },
    ],
  },
  "aave-v4": {
    materializedViews: [
      { name: "net_position_by_reserve_mv", sourceTable: "supply/withdraw/borrow/repay", targetTable: "net_position_by_reserve", description: "Share-based positions per reserve ID", aggregation: "sum(amount) per action type" },
      { name: "liquidation_volume_daily_mv", sourceTable: "liquidation_call", targetTable: "liquidation_volume_daily", description: "Daily Dutch auction liquidation volume", aggregation: "count, sum(debt/collateral), uniqExact" },
      { name: "deficit_daily_mv", sourceTable: "report_deficit", targetTable: "deficit_daily", description: "Daily bad debt (deficit) reports", aggregation: "count, uniqExact(user)" },
    ],
    sampleQueries: (db) => [
      { title: "Net position per reserve", sql: `SELECT reserve_id,\n       total_supplied_amount,\n       total_borrowed_amount\nFROM ${db}.net_position_by_reserve\nORDER BY total_supplied_amount DESC` },
      { title: "Recent liquidations", sql: `SELECT *\nFROM ${db}.liquidation_call\nORDER BY block_number DESC\nLIMIT 20` },
      { title: "Bad debt events", sql: `SELECT *\nFROM ${db}.deficit_daily\nORDER BY day DESC` },
    ],
  },
};

// ---------------------------------------------------------------------------
// Main generator
// ---------------------------------------------------------------------------

export function generateSchemaPreview(state: BuilderState): SchemaPreviewData | null {
  const template = TEMPLATES.find((t) => t.name === state.selectedTemplate);
  if (!template) return null;

  const indexerName = state.databaseName || "rindexer";
  const contractName = deriveContractName(template, state);
  const dbName = `${camelToSnake(indexerName)}_${camelToSnake(contractName)}`;

  // Derive tables from event signatures
  const tables: TableDef[] = template.events.map((sig) => {
    const parsed = parseEventSignature(sig);
    const tableName = camelToSnake(parsed.name);

    const eventColumns: ColumnDef[] = parsed.params.map((p) => ({
      name: camelToSnake(p.name),
      type: solidityToClickhouse(p.type),
      source: "event" as const,
    }));

    const allColumns = [
      ...STANDARD_COLUMNS_BEFORE,
      ...eventColumns,
      ...STANDARD_COLUMNS_AFTER,
    ];

    return {
      fullName: `${dbName}.${tableName}`,
      tableName,
      description: `Raw ${parsed.name} events`,
      columns: allColumns,
      orderBy: STANDARD_ORDER_BY,
      engine: "ReplacingMergeTree",
      columnCount: allColumns.length,
    };
  });

  // Get template-specific MV and query data
  const meta = TEMPLATE_SCHEMA_META[template.name];
  const materializedViews = meta?.materializedViews ?? [];
  const sampleQueries = meta?.sampleQueries(dbName) ?? [];

  return { databaseName: dbName, tables, materializedViews, sampleQueries };
}

function deriveContractName(template: TemplateDef, state: BuilderState): string {
  // Protocol templates have named contracts
  const firstChain = template.chains[0];
  if (firstChain) {
    const contractNames = Object.keys(firstChain.contracts);
    if (contractNames.length > 0) return contractNames[0];
  }
  // Token/NFT templates use the user's variable
  if (state.templateVariables.token_symbol) return state.templateVariables.token_symbol;
  if (state.templateVariables.collection_name) return state.templateVariables.collection_name;
  if (template.category === "nft") return "NFT";
  return "Token";
}
