-- Net position by reserve: cumulative supply - withdrawal + borrow - repay (share-based)
CREATE TABLE IF NOT EXISTS aave_v4_net_position_by_reserve (
    chain_id UInt64,
    reserve_id UInt256,
    total_supplied_amount String,
    total_withdrawn_amount String,
    total_borrowed_amount String,
    total_repaid_amount String,
    supply_count UInt64,
    withdraw_count UInt64,
    borrow_count UInt64,
    repay_count UInt64
) ENGINE = SummingMergeTree()
ORDER BY (chain_id, reserve_id);

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_net_position_supply_mv
TO aave_v4_net_position_by_reserve
AS SELECT
    chain_id,
    reserve_id,
    toString(sum(supplied_amount)) AS total_supplied_amount,
    '0' AS total_withdrawn_amount,
    '0' AS total_borrowed_amount,
    '0' AS total_repaid_amount,
    count() AS supply_count,
    toUInt64(0) AS withdraw_count,
    toUInt64(0) AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v4_supplies
GROUP BY chain_id, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_net_position_withdraw_mv
TO aave_v4_net_position_by_reserve
AS SELECT
    chain_id,
    reserve_id,
    '0' AS total_supplied_amount,
    toString(sum(withdrawn_amount)) AS total_withdrawn_amount,
    '0' AS total_borrowed_amount,
    '0' AS total_repaid_amount,
    toUInt64(0) AS supply_count,
    count() AS withdraw_count,
    toUInt64(0) AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v4_withdrawals
GROUP BY chain_id, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_net_position_borrow_mv
TO aave_v4_net_position_by_reserve
AS SELECT
    chain_id,
    reserve_id,
    '0' AS total_supplied_amount,
    '0' AS total_withdrawn_amount,
    toString(sum(drawn_amount)) AS total_borrowed_amount,
    '0' AS total_repaid_amount,
    toUInt64(0) AS supply_count,
    toUInt64(0) AS withdraw_count,
    count() AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v4_borrows
GROUP BY chain_id, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_net_position_repay_mv
TO aave_v4_net_position_by_reserve
AS SELECT
    chain_id,
    reserve_id,
    '0' AS total_supplied_amount,
    '0' AS total_withdrawn_amount,
    '0' AS total_borrowed_amount,
    toString(sum(total_amount_repaid)) AS total_repaid_amount,
    toUInt64(0) AS supply_count,
    toUInt64(0) AS withdraw_count,
    toUInt64(0) AS borrow_count,
    count() AS repay_count
FROM aave_v4_repays
GROUP BY chain_id, reserve_id;

-- Daily liquidation volume (Dutch auction analytics)
CREATE TABLE IF NOT EXISTS aave_v4_liquidation_volume_daily (
    chain_id UInt64,
    day Date,
    collateral_reserve_id UInt256,
    debt_reserve_id UInt256,
    liquidation_count UInt64,
    total_debt_restored String,
    total_collateral_removed String,
    unique_liquidators UInt64,
    unique_users_liquidated UInt64
) ENGINE = SummingMergeTree()
ORDER BY (chain_id, day, collateral_reserve_id, debt_reserve_id)
PARTITION BY (chain_id, toYYYYMM(day));

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_liquidation_volume_daily_mv
TO aave_v4_liquidation_volume_daily
AS SELECT
    chain_id,
    toDate(block_timestamp) AS day,
    collateral_reserve_id,
    debt_reserve_id,
    count() AS liquidation_count,
    toString(sum(debt_amount_restored)) AS total_debt_restored,
    toString(sum(collateral_amount_removed)) AS total_collateral_removed,
    uniqExact(liquidator) AS unique_liquidators,
    uniqExact(user) AS unique_users_liquidated
FROM aave_v4_liquidations
GROUP BY chain_id, day, collateral_reserve_id, debt_reserve_id;

-- Hourly utilization: supply vs borrow activity
CREATE TABLE IF NOT EXISTS aave_v4_utilization_hourly (
    chain_id UInt64,
    hour DateTime,
    reserve_id UInt256,
    supply_volume String,
    withdraw_volume String,
    borrow_volume String,
    repay_volume String,
    supply_count UInt64,
    withdraw_count UInt64,
    borrow_count UInt64,
    repay_count UInt64
) ENGINE = SummingMergeTree()
ORDER BY (chain_id, hour, reserve_id)
PARTITION BY (chain_id, toYYYYMM(hour));

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_utilization_supply_mv
TO aave_v4_utilization_hourly
AS SELECT
    chain_id,
    toStartOfHour(block_timestamp) AS hour,
    reserve_id,
    toString(sum(supplied_amount)) AS supply_volume,
    '0' AS withdraw_volume,
    '0' AS borrow_volume,
    '0' AS repay_volume,
    count() AS supply_count,
    toUInt64(0) AS withdraw_count,
    toUInt64(0) AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v4_supplies
GROUP BY chain_id, hour, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_utilization_withdraw_mv
TO aave_v4_utilization_hourly
AS SELECT
    chain_id,
    toStartOfHour(block_timestamp) AS hour,
    reserve_id,
    '0' AS supply_volume,
    toString(sum(withdrawn_amount)) AS withdraw_volume,
    '0' AS borrow_volume,
    '0' AS repay_volume,
    toUInt64(0) AS supply_count,
    count() AS withdraw_count,
    toUInt64(0) AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v4_withdrawals
GROUP BY chain_id, hour, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_utilization_borrow_mv
TO aave_v4_utilization_hourly
AS SELECT
    chain_id,
    toStartOfHour(block_timestamp) AS hour,
    reserve_id,
    '0' AS supply_volume,
    '0' AS withdraw_volume,
    toString(sum(drawn_amount)) AS borrow_volume,
    '0' AS repay_volume,
    toUInt64(0) AS supply_count,
    toUInt64(0) AS withdraw_count,
    count() AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v4_borrows
GROUP BY chain_id, hour, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_utilization_repay_mv
TO aave_v4_utilization_hourly
AS SELECT
    chain_id,
    toStartOfHour(block_timestamp) AS hour,
    reserve_id,
    '0' AS supply_volume,
    '0' AS withdraw_volume,
    '0' AS borrow_volume,
    toString(sum(total_amount_repaid)) AS repay_volume,
    toUInt64(0) AS supply_count,
    toUInt64(0) AS withdraw_count,
    toUInt64(0) AS borrow_count,
    count() AS repay_count
FROM aave_v4_repays
GROUP BY chain_id, hour, reserve_id;

-- Deficit tracking (bad debt events)
CREATE TABLE IF NOT EXISTS aave_v4_deficit_daily (
    chain_id UInt64,
    day Date,
    reserve_id UInt256,
    deficit_count UInt64,
    unique_users UInt64
) ENGINE = SummingMergeTree()
ORDER BY (chain_id, day, reserve_id);

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_deficit_daily_mv
TO aave_v4_deficit_daily
AS SELECT
    chain_id,
    toDate(block_timestamp) AS day,
    reserve_id,
    count() AS deficit_count,
    uniqExact(user) AS unique_users
FROM aave_v4_deficits
GROUP BY chain_id, day, reserve_id;
