# ERC-20 Transfers Template

Index ERC-20 Transfer events for any token on any supported EVM chain. Tracks sender, receiver, value, and provides hourly volume analytics via ClickHouse materialized views.

## Usage

```bash
evm-cloud templates apply erc20-transfers --chains polygon \
  --var token_address=0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174 \
  --var token_symbol=USDC
```

## Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `token_address` | Yes | - | Contract address of the ERC-20 token |
| `token_symbol` | No | `TOKEN` | Symbol used for naming in rindexer config |

## Supported Chains

ethereum, polygon, arbitrum, optimism, base

## ClickHouse Tables

- **`erc20_transfers`** — Raw transfer events (from, to, value, tx_hash, block info)
- **`erc20_transfer_volume_hourly`** — Materialized view: hourly transfer count, unique senders/receivers, total volume

## Sample Queries

### Total transfers in the last 24 hours
```sql
SELECT count() AS total_transfers
FROM erc20_transfers
WHERE block_timestamp >= now() - INTERVAL 1 DAY;
```

### Top 10 senders by transfer count
```sql
SELECT from_address, count() AS transfers
FROM erc20_transfers
WHERE block_timestamp >= now() - INTERVAL 7 DAY
GROUP BY from_address
ORDER BY transfers DESC
LIMIT 10;
```

### Hourly transfer volume for the past week
```sql
SELECT hour, transfer_count, unique_senders, unique_receivers
FROM erc20_transfer_volume_hourly
WHERE hour >= now() - INTERVAL 7 DAY
ORDER BY hour;
```

### Largest single transfers
```sql
SELECT block_number, tx_hash, from_address, to_address, value
FROM erc20_transfers
ORDER BY value DESC
LIMIT 10;
```

### Daily unique active addresses
```sql
SELECT
    toDate(block_timestamp) AS day,
    uniqExact(from_address) + uniqExact(to_address) AS unique_addresses
FROM erc20_transfers
WHERE block_timestamp >= now() - INTERVAL 30 DAY
GROUP BY day
ORDER BY day;
```
