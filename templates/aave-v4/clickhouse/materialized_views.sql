-- rindexer schema: aave_v4_analytics_aave_v4_spoke
-- Net position by reserve: cumulative supply - withdrawal + borrow - repay (share-based)
CREATE TABLE IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.net_position_by_reserve (
    network String,
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
ORDER BY (network, reserve_id);

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.net_position_supply_mv
TO aave_v4_analytics_aave_v4_spoke.net_position_by_reserve
AS SELECT
    network,
    reserve_id,
    toString(sum(supplied_amount)) AS total_supplied_amount,
    '0' AS total_withdrawn_amount,
    '0' AS total_borrowed_amount,
    '0' AS total_repaid_amount,
    count() AS supply_count,
    toUInt64(0) AS withdraw_count,
    toUInt64(0) AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v4_analytics_aave_v4_spoke.supply
GROUP BY network, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.net_position_withdraw_mv
TO aave_v4_analytics_aave_v4_spoke.net_position_by_reserve
AS SELECT
    network,
    reserve_id,
    '0' AS total_supplied_amount,
    toString(sum(withdrawn_amount)) AS total_withdrawn_amount,
    '0' AS total_borrowed_amount,
    '0' AS total_repaid_amount,
    toUInt64(0) AS supply_count,
    count() AS withdraw_count,
    toUInt64(0) AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v4_analytics_aave_v4_spoke.withdraw
GROUP BY network, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.net_position_borrow_mv
TO aave_v4_analytics_aave_v4_spoke.net_position_by_reserve
AS SELECT
    network,
    reserve_id,
    '0' AS total_supplied_amount,
    '0' AS total_withdrawn_amount,
    toString(sum(drawn_amount)) AS total_borrowed_amount,
    '0' AS total_repaid_amount,
    toUInt64(0) AS supply_count,
    toUInt64(0) AS withdraw_count,
    count() AS borrow_count,
    toUInt64(0) AS repay_count
FROM aave_v4_analytics_aave_v4_spoke.borrow
GROUP BY network, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.net_position_repay_mv
TO aave_v4_analytics_aave_v4_spoke.net_position_by_reserve
AS SELECT
    network,
    reserve_id,
    '0' AS total_supplied_amount,
    '0' AS total_withdrawn_amount,
    '0' AS total_borrowed_amount,
    toString(sum(total_amount_repaid)) AS total_repaid_amount,
    toUInt64(0) AS supply_count,
    toUInt64(0) AS withdraw_count,
    toUInt64(0) AS borrow_count,
    count() AS repay_count
FROM aave_v4_analytics_aave_v4_spoke.repay
GROUP BY network, reserve_id;

-- Daily liquidation volume (Dutch auction analytics)
CREATE TABLE IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.liquidation_volume_daily (
    network String,
    day Date,
    collateral_reserve_id UInt256,
    debt_reserve_id UInt256,
    liquidation_count UInt64,
    total_debt_restored String,
    total_collateral_removed String,
    unique_liquidators UInt64,
    unique_users_liquidated UInt64
) ENGINE = SummingMergeTree()
ORDER BY (network, day, collateral_reserve_id, debt_reserve_id)
PARTITION BY (network, toYYYYMM(day));

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.liquidation_volume_daily_mv
TO aave_v4_analytics_aave_v4_spoke.liquidation_volume_daily
AS SELECT
    network,
    toDate(block_timestamp) AS day,
    collateral_reserve_id,
    debt_reserve_id,
    count() AS liquidation_count,
    toString(sum(debt_amount_restored)) AS total_debt_restored,
    toString(sum(collateral_amount_removed)) AS total_collateral_removed,
    uniqExact(liquidator) AS unique_liquidators,
    uniqExact(user) AS unique_users_liquidated
FROM aave_v4_analytics_aave_v4_spoke.liquidation_call
GROUP BY network, day, collateral_reserve_id, debt_reserve_id;

-- Hourly utilization: supply vs borrow activity
CREATE TABLE IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.utilization_hourly (
    network String,
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
ORDER BY (network, hour, reserve_id)
PARTITION BY (network, toYYYYMM(hour));

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.utilization_supply_mv
TO aave_v4_analytics_aave_v4_spoke.utilization_hourly
AS SELECT
    network,
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
FROM aave_v4_analytics_aave_v4_spoke.supply
GROUP BY network, hour, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.utilization_withdraw_mv
TO aave_v4_analytics_aave_v4_spoke.utilization_hourly
AS SELECT
    network,
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
FROM aave_v4_analytics_aave_v4_spoke.withdraw
GROUP BY network, hour, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.utilization_borrow_mv
TO aave_v4_analytics_aave_v4_spoke.utilization_hourly
AS SELECT
    network,
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
FROM aave_v4_analytics_aave_v4_spoke.borrow
GROUP BY network, hour, reserve_id;

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.utilization_repay_mv
TO aave_v4_analytics_aave_v4_spoke.utilization_hourly
AS SELECT
    network,
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
FROM aave_v4_analytics_aave_v4_spoke.repay
GROUP BY network, hour, reserve_id;

-- Deficit tracking (bad debt events)
CREATE TABLE IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.deficit_daily (
    network String,
    day Date,
    reserve_id UInt256,
    deficit_count UInt64,
    unique_users UInt64
) ENGINE = SummingMergeTree()
ORDER BY (network, day, reserve_id);

CREATE MATERIALIZED VIEW IF NOT EXISTS aave_v4_analytics_aave_v4_spoke.deficit_daily_mv
TO aave_v4_analytics_aave_v4_spoke.deficit_daily
AS SELECT
    network,
    toDate(block_timestamp) AS day,
    reserve_id,
    count() AS deficit_count,
    uniqExact(user) AS unique_users
FROM aave_v4_analytics_aave_v4_spoke.report_deficit
GROUP BY network, day, reserve_id;
