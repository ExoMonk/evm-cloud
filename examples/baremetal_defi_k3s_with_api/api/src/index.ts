import { Hono } from "hono";
import { serve } from "@hono/node-server";
import { getClient } from "./db.js";
import type { RindexerWebhookPayload, WhaleAlert } from "./types.js";

const app = new Hono();

const PORT = parseInt(process.env.PORT ?? "3000", 10);
const WEBHOOK_SECRET = process.env.WEBHOOK_SECRET ?? "";

// Threshold: absolute token amount above which a swap is considered a "whale" swap.
// Uniswap V4 amounts are signed int128 in raw token units.
// Default 1e18 = 1 token with 18 decimals — adjust per your use case.
const WHALE_THRESHOLD = BigInt(process.env.WHALE_THRESHOLD ?? "1000000000000000000");

// In-memory ring buffer for whale alerts (last 100)
const MAX_ALERTS = 100;
const whaleAlerts: WhaleAlert[] = [];

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------
app.get("/health", (c) => c.json({ status: "ok", alerts: whaleAlerts.length }));

// ---------------------------------------------------------------------------
// GET /swaps — query ClickHouse for recent indexed swaps
// ---------------------------------------------------------------------------
app.get("/swaps", async (c) => {
  const limit = Math.min(parseInt(c.req.query("limit") ?? "20", 10), 100);
  const network = c.req.query("network");

  const client = getClient();
  let query = `
    SELECT network, tx_hash, block_number, sender,
           amount_0, amount_1, sqrt_price_x96, liquidity, tick, fee
    FROM swap
  `;
  if (network) query += ` WHERE network = {network: String}`;
  query += ` ORDER BY block_number DESC LIMIT {limit: UInt32}`;

  const result = await client.query({
    query,
    query_params: { network: network ?? "", limit },
    format: "JSONEachRow",
  });
  const rows = await result.json();
  return c.json({ count: rows.length, swaps: rows });
});

// ---------------------------------------------------------------------------
// GET /stats — per-network aggregates
// ---------------------------------------------------------------------------
app.get("/stats", async (c) => {
  const client = getClient();
  const result = await client.query({
    query: `
      SELECT network, count() as total_swaps,
             uniq(sender) as unique_senders,
             max(block_number) as latest_block
      FROM swap
      GROUP BY network ORDER BY network
    `,
    format: "JSONEachRow",
  });
  const rows = await result.json();
  return c.json({ networks: rows });
});

// ---------------------------------------------------------------------------
// GET /alerts — recent whale swap alerts
// ---------------------------------------------------------------------------
app.get("/alerts", (c) => {
  const network = c.req.query("network");
  const filtered = network
    ? whaleAlerts.filter((a) => a.network === network)
    : whaleAlerts;
  return c.json({ count: filtered.length, alerts: filtered });
});

// ---------------------------------------------------------------------------
// POST /webhooks/rindexer — rindexer webhook stream receiver
// ---------------------------------------------------------------------------
app.post("/webhooks/rindexer", async (c) => {
  // Verify shared secret
  const secret = c.req.header("x-rindexer-shared-secret");
  if (WEBHOOK_SECRET && secret !== WEBHOOK_SECRET) {
    return c.json({ error: "unauthorized" }, 401);
  }

  const payload: RindexerWebhookPayload = await c.req.json();

  for (const event of payload.event_data) {
    const abs0 = BigInt(event.amount0) < 0n ? -BigInt(event.amount0) : BigInt(event.amount0);
    const abs1 = BigInt(event.amount1) < 0n ? -BigInt(event.amount1) : BigInt(event.amount1);

    if (abs0 >= WHALE_THRESHOLD || abs1 >= WHALE_THRESHOLD) {
      const alert: WhaleAlert = {
        network: event.transaction_information.network,
        pool_id: event.id,
        sender: event.sender,
        amount0: event.amount0,
        amount1: event.amount1,
        tx_hash: event.transaction_information.transaction_hash,
        block_number: event.transaction_information.block_number,
        timestamp: new Date().toISOString(),
      };
      whaleAlerts.unshift(alert);
      if (whaleAlerts.length > MAX_ALERTS) whaleAlerts.pop();

      console.log(
        `[WHALE] ${alert.network} block=${alert.block_number} ` +
        `amount0=${event.amount0} amount1=${event.amount1} tx=${alert.tx_hash}`
      );
    }
  }

  return c.json({ received: payload.event_data.length });
});

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------
serve({ fetch: app.fetch, port: PORT, hostname: "0.0.0.0" }, () => {
  console.log(`swap-api listening on :${PORT}`);
  console.log(`DB_HOST=${process.env.DB_HOST} DB_NAME=${process.env.DB_NAME}`);
  console.log(`WHALE_THRESHOLD=${WHALE_THRESHOLD}`);
});
