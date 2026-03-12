CREATE TABLE IF NOT EXISTS erc20_transfer_volume_hourly (
    chain_id UInt64,
    hour DateTime,
    contract_address String,
    transfer_count UInt64,
    unique_senders UInt64,
    unique_receivers UInt64,
    total_value String
) ENGINE = SummingMergeTree()
ORDER BY (chain_id, hour, contract_address)
PARTITION BY (chain_id, toYYYYMM(hour));

CREATE MATERIALIZED VIEW IF NOT EXISTS erc20_transfer_volume_hourly_mv
TO erc20_transfer_volume_hourly
AS SELECT
    chain_id,
    toStartOfHour(block_timestamp) AS hour,
    contract_address,
    count() AS transfer_count,
    uniqExact(from_address) AS unique_senders,
    uniqExact(to_address) AS unique_receivers,
    toString(sum(value)) AS total_value
FROM erc20_transfers
GROUP BY chain_id, hour, contract_address;
