# Aave V3 Template

Full Aave V3 lending protocol analytics: supply, withdraw, borrow, repay, and liquidation events. Provides net position tracking per asset, liquidation volume monitoring, and hourly utilization metrics.

## Usage

```bash
evm-cloud templates apply aave-v3 --chains ethereum
```

Multi-chain:
```bash
evm-cloud templates apply aave-v3 --chains ethereum,polygon,arbitrum
```

## Supported Chains

| Chain | Pool Proxy Address | Start Block |
|-------|-------------------|-------------|
| ethereum | `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2` | 16,291,127 |
| polygon | `0x794a61358D6845594F94dc1DB02A252b5b4814aD` | 25,826,028 |
| arbitrum | `0x794a61358D6845594F94dc1DB02A252b5b4814aD` | 7,742,429 |
| optimism | `0x794a61358D6845594F94dc1DB02A252b5b4814aD` | 4,365,693 |
| base | `0xA238Dd80C259a72e81d7e4664a9801593F98d1c5` | 2,357,652 |

## ClickHouse Tables

- **`aave_v3_supplies`** — Supply events (reserve, user, amount, referral)
- **`aave_v3_withdrawals`** — Withdraw events (reserve, user, to, amount)
- **`aave_v3_borrows`** — Borrow events (reserve, user, amount, interest rate mode, borrow rate)
- **`aave_v3_repays`** — Repay events (reserve, user, repayer, amount)
- **`aave_v3_liquidations`** — LiquidationCall events (collateral, debt, liquidator, amounts)

### Materialized Views

- **`aave_v3_net_position_by_asset`** — Cumulative supply/withdraw/borrow/repay per reserve
- **`aave_v3_liquidation_volume_daily`** — Daily liquidation count and volume per asset pair
- **`aave_v3_utilization_hourly`** — Hourly supply/withdraw/borrow/repay volume and count per reserve

## Sample Queries

### Net position per asset (TVL proxy)
```sql
SELECT
    reserve,
    total_supplied,
    total_withdrawn,
    total_borrowed,
    total_repaid,
    supply_count + borrow_count AS total_interactions
FROM aave_v3_net_position_by_asset
ORDER BY supply_count DESC;
```

### Recent liquidations
```sql
SELECT
    block_timestamp,
    collateral_asset,
    debt_asset,
    user,
    liquidator,
    debt_to_cover,
    liquidated_collateral_amount,
    tx_hash
FROM aave_v3_liquidations
ORDER BY block_timestamp DESC
LIMIT 20;
```

### Top borrowers by volume
```sql
SELECT
    on_behalf_of AS borrower,
    reserve,
    count() AS borrow_count,
    toString(sum(amount)) AS total_borrowed
FROM aave_v3_borrows
WHERE block_timestamp >= now() - INTERVAL 7 DAY
GROUP BY on_behalf_of, reserve
ORDER BY borrow_count DESC
LIMIT 20;
```

### Hourly utilization per reserve
```sql
SELECT
    hour,
    reserve,
    supply_count,
    withdraw_count,
    borrow_count,
    repay_count
FROM aave_v3_utilization_hourly
WHERE hour >= now() - INTERVAL 24 HOUR
ORDER BY hour, reserve;
```

### Liquidation volume by day
```sql
SELECT
    day,
    sum(liquidation_count) AS liquidations,
    sum(unique_users_liquidated) AS users_liquidated
FROM aave_v3_liquidation_volume_daily
WHERE day >= today() - 30
GROUP BY day
ORDER BY day;
```
