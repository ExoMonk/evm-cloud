/** Rindexer webhook payload shape */
export interface RindexerWebhookPayload {
  event_name: string;
  event_signature_hash: string;
  network: string;
  event_data: SwapEventData[];
}

export interface SwapEventData {
  id: string;         // bytes32 pool id
  sender: string;     // address
  amount0: string;    // int128 (signed)
  amount1: string;    // int128 (signed)
  sqrtPriceX96: string;
  liquidity: string;
  tick: string;
  fee: string;
  transaction_information: {
    address: string;
    block_hash: string;
    block_number: string;
    log_index: string;
    network: string;
    transaction_hash: string;
    transaction_index: string;
  };
}

/** In-memory whale swap alert */
export interface WhaleAlert {
  network: string;
  pool_id: string;
  sender: string;
  amount0: string;
  amount1: string;
  tx_hash: string;
  block_number: string;
  timestamp: string;
}
