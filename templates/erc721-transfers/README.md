# ERC-721 Transfers Template

Index ERC-721 Transfer events for any NFT collection. Tracks ownership history, mint/burn activity, and provides holder distribution analytics via ClickHouse materialized views.

## Usage

```bash
evm-cloud templates apply erc721-transfers --chains ethereum \
  --var nft_address=0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D \
  --var collection_name=BAYC
```

## Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `nft_address` | Yes | - | Contract address of the ERC-721 collection |
| `collection_name` | No | `NFT` | Name used for the contract in rindexer config |

## Supported Chains

ethereum, polygon, arbitrum, optimism, base

## ClickHouse Tables

- **`erc721_transfers`** — Raw transfer events (from, to, tokenId, tx_hash, block info)
- **`erc721_holders_current`** — ReplacingMergeTree: latest owner per token_id (use `FINAL` for accurate reads)
- **`erc721_activity_daily`** — Daily transfer, mint, and burn counts with unique token/address metrics

## Sample Queries

### Current holder count
```sql
SELECT uniqExact(owner) AS holder_count
FROM erc721_holders_current FINAL
WHERE owner != '0x0000000000000000000000000000000000000000';
```

### Top holders by token count
```sql
SELECT owner, count() AS tokens_held
FROM erc721_holders_current FINAL
WHERE owner != '0x0000000000000000000000000000000000000000'
GROUP BY owner
ORDER BY tokens_held DESC
LIMIT 20;
```

### Daily mint activity over the past 30 days
```sql
SELECT day, mint_count, transfer_count, burn_count
FROM erc721_activity_daily
WHERE day >= today() - 30
ORDER BY day;
```

### Most transferred tokens
```sql
SELECT token_id, count() AS transfer_count
FROM erc721_transfers
GROUP BY token_id
ORDER BY transfer_count DESC
LIMIT 10;
```

### Ownership history of a specific token
```sql
SELECT block_number, block_timestamp, from_address, to_address, tx_hash
FROM erc721_transfers
WHERE token_id = '1234'
ORDER BY block_number;
```
