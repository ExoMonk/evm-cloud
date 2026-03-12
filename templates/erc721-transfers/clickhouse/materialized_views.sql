-- Current holder per token_id (latest owner wins via ReplacingMergeTree)
CREATE TABLE IF NOT EXISTS holders_current (
    network String,
    contract_address String,
    token_id String,
    owner String,
    block_number UInt64,
    block_timestamp DateTime64(3)
) ENGINE = ReplacingMergeTree(block_number)
ORDER BY (network, contract_address, token_id);

CREATE MATERIALIZED VIEW IF NOT EXISTS holders_current_mv
TO holders_current
AS SELECT
    network,
    contract_address,
    token_id,
    to_address AS owner,
    block_number,
    block_timestamp
FROM transfer;

-- Daily activity summary
CREATE TABLE IF NOT EXISTS activity_daily (
    network String,
    day Date,
    contract_address String,
    transfer_count UInt64,
    mint_count UInt64,
    burn_count UInt64,
    unique_tokens UInt64,
    unique_senders UInt64,
    unique_receivers UInt64
) ENGINE = SummingMergeTree()
ORDER BY (network, day, contract_address)
PARTITION BY (network, toYYYYMM(day));

CREATE MATERIALIZED VIEW IF NOT EXISTS activity_daily_mv
TO activity_daily
AS SELECT
    network,
    toDate(block_timestamp) AS day,
    contract_address,
    count() AS transfer_count,
    countIf(from_address = '0x0000000000000000000000000000000000000000') AS mint_count,
    countIf(to_address = '0x0000000000000000000000000000000000000000') AS burn_count,
    uniqExact(token_id) AS unique_tokens,
    uniqExact(from_address) AS unique_senders,
    uniqExact(to_address) AS unique_receivers
FROM transfer
GROUP BY network, day, contract_address;
