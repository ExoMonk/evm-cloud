# Uniswap V4 Template

Uniswap V4 analytics on the singleton PoolManager contract. Tracks pool initialization, swaps, and liquidity modifications with hook usage analytics.

## Usage

```bash
evm-cloud templates apply uniswap-v4 --chains ethereum
```

Multi-chain:
```bash
evm-cloud templates apply uniswap-v4 --chains ethereum,base
```

## Supported Chains

| Chain | PoolManager Address | Start Block |
|-------|---------------------|-------------|
| ethereum | `0x000000000004444c5dc75cB358380D2e3dE08A90` | 21,688,329 |
| arbitrum | `0x000000000004444c5dc75cB358380D2e3dE08A90` | 291,629,680 |
| base | `0x000000000004444c5dc75cB358380D2e3dE08A90` | 25,350,988 |

## ClickHouse Tables

- **`uniswap_v4_pools`** — Initialize events (currency0, currency1, fee, tickSpacing, hooks, initial price)
- **`uniswap_v4_swaps`** — Swap events with amounts, price, liquidity, tick, fee
- **`uniswap_v4_liquidity_changes`** — ModifyLiquidity events (add/remove, tick range, salt)

### Materialized Views

- **`uniswap_v4_volume_hourly`** — Hourly swap count and volume per pool
- **`uniswap_v4_hook_usage`** — Aggregate pool count, swap count, and liquidity changes per hook address

## Sample Queries

### Top pools by swap count (last 24h)
```sql
SELECT
    s.pool_id,
    p.currency0,
    p.currency1,
    p.hooks,
    count() AS swap_count
FROM uniswap_v4_swaps s
JOIN uniswap_v4_pools p ON s.pool_id = p.pool_id AND s.chain_id = p.chain_id
WHERE s.block_timestamp >= now() - INTERVAL 1 DAY
GROUP BY s.pool_id, p.currency0, p.currency1, p.hooks
ORDER BY swap_count DESC
LIMIT 10;
```

### Hook adoption breakdown
```sql
SELECT
    hooks,
    count() AS pool_count,
    round(count() * 100.0 / (SELECT count() FROM uniswap_v4_pools), 2) AS pct
FROM uniswap_v4_pools
GROUP BY hooks
ORDER BY pool_count DESC;
```

### Daily pool creation rate
```sql
SELECT
    toDate(block_timestamp) AS day,
    count() AS pools_created,
    countIf(hooks != '0x0000000000000000000000000000000000000000') AS pools_with_hooks
FROM uniswap_v4_pools
GROUP BY day
ORDER BY day;
```

### Hourly swap volume
```sql
SELECT hour, sum(swap_count) AS total_swaps
FROM uniswap_v4_volume_hourly
WHERE hour >= now() - INTERVAL 7 DAY
GROUP BY hour
ORDER BY hour;
```

### Most active liquidity providers
```sql
SELECT sender, count() AS modifications
FROM uniswap_v4_liquidity_changes
WHERE block_timestamp >= now() - INTERVAL 7 DAY
GROUP BY sender
ORDER BY modifications DESC
LIMIT 20;
```
