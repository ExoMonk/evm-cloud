-- Net position by asset: cumulative supply - withdrawal + borrow - repay
CREATE TABLE IF NOT EXISTS aave_v3_net_position_by_asset (
    chain_id UInt64,
    reserve String,
    total_supplied String,
    total_withdrawn String,
    total_borrowed String,
    total_repaid String,
    supply_count UInt64,
    withdraw_count UInt64,
    borrow_count UInt64,
    repay_count UInt64
) ENGINE = SummingMergeTree()
ORDER BY (chain_id, reserve);

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v3_net_position_supply_mv
TO aave_v3_net_position_by_asset
AS SELECT
    chain_id,
    reserve,
    toString(sum(amount)) AS total_supplied,
    '0' AS total_withdrawn,
    '0' AS total_borrowed,
    '0' AS total_repaid,
    count() AS supply_count,
    toUInt64(0) AS withdraw_count,
    toUInt64(0) AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v3_supplies
GROUP BY chain_id, reserve;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v3_net_position_withdraw_mv
TO aave_v3_net_position_by_asset
AS SELECT
    chain_id,
    reserve,
    '0' AS total_supplied,
    toString(sum(amount)) AS total_withdrawn,
    '0' AS total_borrowed,
    '0' AS total_repaid,
    toUInt64(0) AS supply_count,
    count() AS withdraw_count,
    toUInt64(0) AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v3_withdrawals
GROUP BY chain_id, reserve;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v3_net_position_borrow_mv
TO aave_v3_net_position_by_asset
AS SELECT
    chain_id,
    reserve,
    '0' AS total_supplied,
    '0' AS total_withdrawn,
    toString(sum(amount)) AS total_borrowed,
    '0' AS total_repaid,
    toUInt64(0) AS supply_count,
    toUInt64(0) AS withdraw_count,
    count() AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v3_borrows
GROUP BY chain_id, reserve;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v3_net_position_repay_mv
TO aave_v3_net_position_by_asset
AS SELECT
    chain_id,
    reserve,
    '0' AS total_supplied,
    '0' AS total_withdrawn,
    '0' AS total_borrowed,
    toString(sum(amount)) AS total_repaid,
    toUInt64(0) AS supply_count,
    toUInt64(0) AS withdraw_count,
    toUInt64(0) AS borrow_count,
    count() AS repay_count
FROM aave_v3_repays
GROUP BY chain_id, reserve;

-- Daily liquidation volume
CREATE TABLE IF NOT EXISTS aave_v3_liquidation_volume_daily (
    chain_id UInt64,
    day Date,
    collateral_asset String,
    debt_asset String,
    liquidation_count UInt64,
    total_debt_covered String,
    total_collateral_liquidated String,
    unique_liquidators UInt64,
    unique_users_liquidated UInt64
) ENGINE = SummingMergeTree()
ORDER BY (chain_id, day, collateral_asset, debt_asset)
PARTITION BY (chain_id, toYYYYMM(day));

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v3_liquidation_volume_daily_mv
TO aave_v3_liquidation_volume_daily
AS SELECT
    chain_id,
    toDate(block_timestamp) AS day,
    collateral_asset,
    debt_asset,
    count() AS liquidation_count,
    toString(sum(debt_to_cover)) AS total_debt_covered,
    toString(sum(liquidated_collateral_amount)) AS total_collateral_liquidated,
    uniqExact(liquidator) AS unique_liquidators,
    uniqExact(user) AS unique_users_liquidated
FROM aave_v3_liquidations
GROUP BY chain_id, day, collateral_asset, debt_asset;

-- Hourly utilization: supply vs borrow activity
CREATE TABLE IF NOT EXISTS aave_v3_utilization_hourly (
    chain_id UInt64,
    hour DateTime,
    reserve String,
    supply_volume String,
    withdraw_volume String,
    borrow_volume String,
    repay_volume String,
    supply_count UInt64,
    withdraw_count UInt64,
    borrow_count UInt64,
    repay_count UInt64
) ENGINE = SummingMergeTree()
ORDER BY (chain_id, hour, reserve)
PARTITION BY (chain_id, toYYYYMM(hour));

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v3_utilization_supply_mv
TO aave_v3_utilization_hourly
AS SELECT
    chain_id,
    toStartOfHour(block_timestamp) AS hour,
    reserve,
    toString(sum(amount)) AS supply_volume,
    '0' AS withdraw_volume,
    '0' AS borrow_volume,
    '0' AS repay_volume,
    count() AS supply_count,
    toUInt64(0) AS withdraw_count,
    toUInt64(0) AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v3_supplies
GROUP BY chain_id, hour, reserve;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v3_utilization_withdraw_mv
TO aave_v3_utilization_hourly
AS SELECT
    chain_id,
    toStartOfHour(block_timestamp) AS hour,
    reserve,
    '0' AS supply_volume,
    toString(sum(amount)) AS withdraw_volume,
    '0' AS borrow_volume,
    '0' AS repay_volume,
    toUInt64(0) AS supply_count,
    count() AS withdraw_count,
    toUInt64(0) AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v3_withdrawals
GROUP BY chain_id, hour, reserve;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v3_utilization_borrow_mv
TO aave_v3_utilization_hourly
AS SELECT
    chain_id,
    toStartOfHour(block_timestamp) AS hour,
    reserve,
    '0' AS supply_volume,
    '0' AS withdraw_volume,
    toString(sum(amount)) AS borrow_volume,
    '0' AS repay_volume,
    toUInt64(0) AS supply_count,
    toUInt64(0) AS withdraw_count,
    count() AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v3_borrows
GROUP BY chain_id, hour, reserve;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v3_utilization_repay_mv
TO aave_v3_utilization_hourly
AS SELECT
    chain_id,
    toStartOfHour(block_timestamp) AS hour,
    reserve,
    '0' AS supply_volume,
    '0' AS withdraw_volume,
    '0' AS borrow_volume,
    toString(sum(amount)) AS repay_volume,
    toUInt64(0) AS supply_count,
    toUInt64(0) AS withdraw_count,
    toUInt64(0) AS borrow_count,
    count() AS repay_count
FROM aave_v3_repays
GROUP BY chain_id, hour, reserve;
