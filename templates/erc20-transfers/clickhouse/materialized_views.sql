CREATE TABLE IF NOT EXISTS transfer_volume_hourly (
    network String,
    hour DateTime,
    contract_address String,
    transfer_count UInt64,
    unique_senders UInt64,
    unique_receivers UInt64,
    total_value String
) ENGINE = SummingMergeTree()
ORDER BY (network, hour, contract_address)
PARTITION BY (network, toYYYYMM(hour));

CREATE MATERIALIZED VIEW IF NOT EXISTS transfer_volume_hourly_mv
TO transfer_volume_hourly
AS SELECT
    network,
    toStartOfHour(block_timestamp) AS hour,
    contract_address,
    count() AS transfer_count,
    uniqExact(from_address) AS unique_senders,
    uniqExact(to_address) AS unique_receivers,
    toString(sum(value)) AS total_value
FROM transfer
GROUP BY network, hour, contract_address;
