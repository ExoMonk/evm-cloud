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

- **`initialize`** — Initialize events (currency_0, currency_1, fee, tick_spacing, hooks, initial price)
- **`swap`** — Swap events with amounts, price, liquidity, tick, fee
- **`modify_liquidity`** — ModifyLiquidity events (add/remove, tick range, salt)

### Materialized Views

- **`volume_hourly`** — Hourly swap count and volume per pool
- **`hook_usage`** — Aggregate pool count, swap count, and liquidity changes per hook address

## Sample Queries

### Top pools by swap count (last 24h)
```sql
SELECT
    s.id,
    p.currency_0,
    p.currency_1,
    p.hooks,
    count() AS swap_count
FROM swap s
JOIN initialize p ON s.id = p.id AND s.network = p.network
WHERE s.block_timestamp >= now() - INTERVAL 1 DAY
GROUP BY s.id, p.currency_0, p.currency_1, p.hooks
ORDER BY swap_count DESC
LIMIT 10;
```

### Hook adoption breakdown
```sql
SELECT
    hooks,
    count() AS pool_count,
    round(count() * 100.0 / (SELECT count() FROM initialize), 2) AS pct
FROM initialize
GROUP BY hooks
ORDER BY pool_count DESC;
```

### Daily pool creation rate
```sql
SELECT
    toDate(block_timestamp) AS day,
    count() AS pools_created,
    countIf(hooks != '0x0000000000000000000000000000000000000000') AS pools_with_hooks
FROM initialize
GROUP BY day
ORDER BY day;
```

### Hourly swap volume
```sql
SELECT hour, sum(swap_count) AS total_swaps
FROM volume_hourly
WHERE hour >= now() - INTERVAL 7 DAY
GROUP BY hour
ORDER BY hour;
```

### Most active liquidity providers
```sql
SELECT sender, count() AS modifications
FROM modify_liquidity
WHERE block_timestamp >= now() - INTERVAL 7 DAY
GROUP BY sender
ORDER BY modifications DESC
LIMIT 20;
```
