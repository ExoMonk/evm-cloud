-- Current holder per token_id (latest owner wins via ReplacingMergeTree)
CREATE TABLE IF NOT EXISTS erc721_holders_current (
    chain_id UInt64,
    contract_address String,
    token_id String,
    owner String,
    block_number UInt64,
    block_timestamp DateTime64(3)
) ENGINE = ReplacingMergeTree(block_number)
ORDER BY (chain_id, contract_address, token_id);

CREATE MATERIALIZED VIEW IF NOT EXISTS erc721_holders_current_mv
TO erc721_holders_current
AS SELECT
    chain_id,
    contract_address,
    token_id,
    to_address AS owner,
    block_number,
    block_timestamp
FROM erc721_transfers;

-- Daily activity summary
CREATE TABLE IF NOT EXISTS erc721_activity_daily (
    chain_id UInt64,
    day Date,
    contract_address String,
    transfer_count UInt64,
    mint_count UInt64,
    burn_count UInt64,
    unique_tokens UInt64,
    unique_senders UInt64,
    unique_receivers UInt64
) ENGINE = SummingMergeTree()
ORDER BY (chain_id, day, contract_address)
PARTITION BY (chain_id, toYYYYMM(day));

CREATE MATERIALIZED VIEW IF NOT EXISTS erc721_activity_daily_mv
TO erc721_activity_daily
AS SELECT
    chain_id,
    toDate(block_timestamp) AS day,
    contract_address,
    count() AS transfer_count,
    countIf(from_address = '0x0000000000000000000000000000000000000000') AS mint_count,
    countIf(to_address = '0x0000000000000000000000000000000000000000') AS burn_count,
    uniqExact(token_id) AS unique_tokens,
    uniqExact(from_address) AS unique_senders,
    uniqExact(to_address) AS unique_receivers
FROM erc721_transfers
GROUP BY chain_id, day, contract_address;
