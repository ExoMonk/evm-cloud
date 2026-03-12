# Aave V4 Template

Aave V4 Hub & Spoke lending analytics: share-based supply/borrow, Dutch auction liquidations, risk premium tracking, and bad debt monitoring. Covers the new unified liquidity architecture with reserve IDs instead of token addresses.

> **Note**: Aave V4 is launching on Ethereum mainnet. Spoke contract addresses must be provided via `--var` once deployment is finalized.

## Usage

```bash
evm-cloud templates apply aave-v4 --chains ethereum \
  --var spoke_address=0x... \
  --var spoke_start_block=21000000
```

## V4 vs V3 Key Differences

| Feature | V3 | V4 |
|---------|----|----|
| Architecture | Single Pool contract | Hub & Spoke (isolated risk modules) |
| Accounting | Token amounts + aTokens | ERC-4626 shares (no rebasing) |
| Identifiers | Reserve = token address | Reserve = uint256 reserveId |
| Liquidation | Fixed close factor, static bonus | Target health factor, Dutch auction bonus |
| Interest rates | Governance-set curves | Fuzzy-controlled automated adjustment |
| Risk pricing | Same rate for all borrowers | Per-user risk premium based on collateral quality |
| Bad debt | Silent | Explicit `ReportDeficit` events |

## ClickHouse Tables

- **`supply`** — Supply events (reserveId, caller, user, shares, amount)
- **`withdraw`** — Withdraw events (reserveId, caller, user, shares, amount)
- **`borrow`** — Borrow events (reserveId, caller, user, drawnShares, drawnAmount)
- **`repay`** — Repay events (reserveId, caller, user, drawnShares, totalAmountRepaid)
- **`liquidation_call`** — LiquidationCall events (Dutch auction: collateral/debt reserves, amounts, shares)
- **`report_deficit`** — ReportDeficit events (bad debt tracking per reserve)
- **`update_user_risk_premium`** — Latest risk premium per user (ReplacingMergeTree)

### Materialized Views

- **`net_position_by_reserve`** — Cumulative supply/withdraw/borrow/repay per reserveId
- **`liquidation_volume_daily`** — Daily liquidation count and volume per reserve pair
- **`utilization_hourly`** — Hourly supply/withdraw/borrow/repay volume per reserve
- **`deficit_daily`** — Daily bad debt event count per reserve

## Sample Queries

### Net position per reserve (TVL proxy)
```sql
SELECT
    toString(reserve_id) AS reserve,
    total_supplied_amount,
    total_withdrawn_amount,
    total_borrowed_amount,
    total_repaid_amount,
    supply_count + borrow_count AS total_interactions
FROM net_position_by_reserve
ORDER BY supply_count DESC;
```

### Recent liquidations with Dutch auction details
```sql
SELECT
    block_timestamp,
    toString(collateral_reserve_id) AS collateral,
    toString(debt_reserve_id) AS debt,
    user,
    liquidator,
    debt_amount_restored,
    collateral_amount_removed,
    collateral_shares_to_liquidator,
    tx_hash
FROM liquidation_call
ORDER BY block_timestamp DESC
LIMIT 20;
```

### Users with highest risk premiums
```sql
SELECT
    user,
    risk_premium,
    block_timestamp AS last_updated
FROM update_user_risk_premium FINAL
ORDER BY risk_premium DESC
LIMIT 20;
```

### Bad debt events (deficit reports)
```sql
SELECT
    block_timestamp,
    toString(reserve_id) AS reserve,
    user,
    drawn_shares,
    tx_hash
FROM report_deficit
ORDER BY block_timestamp DESC
LIMIT 20;
```

### Hourly utilization per reserve
```sql
SELECT
    hour,
    toString(reserve_id) AS reserve,
    supply_count,
    withdraw_count,
    borrow_count,
    repay_count
FROM utilization_hourly
WHERE hour >= now() - INTERVAL 24 HOUR
ORDER BY hour, reserve;
```
