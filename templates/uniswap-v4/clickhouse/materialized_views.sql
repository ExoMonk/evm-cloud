-- Materialized views for Uniswap V4 analytics.
-- Apply AFTER rindexer has started and created its base tables.
--
-- rindexer schema: uniswap_v4_analytics_pool_manager
-- Base tables:     .initialize, .swap, .modify_liquidity
-- Network column:  `network` (string, e.g. "ethereum", "base")

-- Hourly swap volume per pool
CREATE TABLE IF NOT EXISTS uniswap_v4_analytics_pool_manager.volume_hourly (
    network String,
    hour DateTime,
    pool_id String,
    swap_count UInt64,
    total_amount_0 String,
    total_amount_1 String
) ENGINE = SummingMergeTree()
ORDER BY (network, hour, pool_id)
PARTITION BY (network, toYYYYMM(hour));

CREATE MATERIALIZED VIEW IF NOT EXISTS uniswap_v4_analytics_pool_manager.volume_hourly_mv
TO uniswap_v4_analytics_pool_manager.volume_hourly
AS SELECT
    network,
    toStartOfHour(block_timestamp) AS hour,
    id AS pool_id,
    count() AS swap_count,
    toString(sum(abs(amount_0))) AS total_amount_0,
    toString(sum(abs(amount_1))) AS total_amount_1
FROM uniswap_v4_analytics_pool_manager.swap
GROUP BY network, hour, pool_id;

-- Hook usage analytics
CREATE TABLE IF NOT EXISTS uniswap_v4_analytics_pool_manager.hook_usage (
    network String,
    hooks String,
    pool_count UInt64,
    swap_count UInt64,
    liquidity_change_count UInt64
) ENGINE = SummingMergeTree()
ORDER BY (network, hooks);

CREATE MATERIALIZED VIEW IF NOT EXISTS uniswap_v4_analytics_pool_manager.hook_usage_pools_mv
TO uniswap_v4_analytics_pool_manager.hook_usage
AS SELECT
    network,
    hooks,
    count() AS pool_count,
    toUInt64(0) AS swap_count,
    toUInt64(0) AS liquidity_change_count
FROM uniswap_v4_analytics_pool_manager.initialize
GROUP BY network, hooks;

CREATE MATERIALIZED VIEW IF NOT EXISTS uniswap_v4_analytics_pool_manager.hook_usage_swaps_mv
TO uniswap_v4_analytics_pool_manager.hook_usage
AS SELECT
    s.network AS network,
    p.hooks AS hooks,
    toUInt64(0) AS pool_count,
    count() AS swap_count,
    toUInt64(0) AS liquidity_change_count
FROM uniswap_v4_analytics_pool_manager.swap s
JOIN uniswap_v4_analytics_pool_manager.initialize p ON s.id = p.id AND s.network = p.network
GROUP BY s.network, p.hooks;

CREATE MATERIALIZED VIEW IF NOT EXISTS uniswap_v4_analytics_pool_manager.hook_usage_liq_mv
TO uniswap_v4_analytics_pool_manager.hook_usage
AS SELECT
    l.network AS network,
    p.hooks AS hooks,
    toUInt64(0) AS pool_count,
    toUInt64(0) AS swap_count,
    count() AS liquidity_change_count
FROM uniswap_v4_analytics_pool_manager.modify_liquidity l
JOIN uniswap_v4_analytics_pool_manager.initialize p ON l.id = p.id AND l.network = p.network
GROUP BY l.network, p.hooks;
