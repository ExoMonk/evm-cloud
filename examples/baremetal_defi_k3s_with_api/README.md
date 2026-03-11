# Bare Metal k3s: Uniswap V4 Swap Indexer + Custom API

Indexes Uniswap V4 **Swap** events on Ethereum and Base into ClickHouse, with a TypeScript API service deployed alongside via `custom_services`. Runs on any VPS — no AWS account needed.

The indexer streams swaps to the API via rindexer webhook — the API flags whale swaps above a configurable threshold.

## What This Example Demonstrates

- **Multi-chain indexing**: Uniswap V4 PoolManager `Swap` events on Ethereum + Base → ClickHouse
- **Webhook streaming**: rindexer streams decoded swap events to the API in real-time
- **Whale detection**: API flags swaps above `WHALE_THRESHOLD` (configurable)
- **Custom service**: `swap-api` deployed via `custom_services` with auto-injected DB credentials
- **Bare metal k3s**: Runs on any VPS (Hetzner, OVH, DigitalOcean, etc.)

## Architecture

```
Ethereum RPC ─┐                                  ┌──────────────────────┐
              ├→ eRPC → rindexer ─── writes ────→ │ ClickHouse (BYODB)   │
Base RPC ─────┘           │                       └──────────┬───────────┘
                          │                                  │
                   webhook stream                       reads│
                   (Swap events)                             │
                          │         ┌────────────────────────┘
                          ▼         ▼
                      ┌─────────────────┐
                      │    swap-api      │
                      │  GET /swaps      │  ← query indexed data
                      │  GET /stats      │
                      │  GET /alerts     │  ← whale swap alerts
                      │  POST /webhooks  │  ← rindexer stream
                      └─────────────────┘
```

## Uniswap V4 PoolManager Addresses

| Chain | Address |
|-------|---------|
| Ethereum | `0x000000000004444c5dc75cB358380D2e3dE08A90` |
| Base | `0x498581fF718922c3f8e6A244956aF099B2652b2b` |

## Setup

1. **Build the API image:**
   ```bash
   cd api
   docker build -t swap-api:latest .
   # Push to your registry:
   # docker tag swap-api:latest ghcr.io/yourorg/swap-api:latest
   # docker push ghcr.io/yourorg/swap-api:latest
   ```

2. **Configure secrets:**
   ```bash
   cp secrets.auto.tfvars.example secrets.auto.tfvars
   # Edit with your VPS host, SSH key, ClickHouse credentials, and webhook_secret
   ```

3. **Deploy:**
   ```bash
   evm-cloud init
   evm-cloud apply
   evm-cloud deploy
   ```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/swaps?limit=20&network=base` | GET | Recent swaps from ClickHouse (filterable by chain) |
| `/stats` | GET | Per-network aggregates: total swaps, unique senders, latest block |
| `/alerts?network=ethereum` | GET | Recent whale swap alerts (in-memory, filterable by chain) |
| `/webhooks/rindexer` | POST | Rindexer webhook receiver (authenticated via shared secret) |

## How Whale Detection Works

1. rindexer indexes `Swap` events and writes them to ClickHouse
2. Simultaneously, rindexer streams each event via webhook to `swap-api`
3. The API checks if `|amount0|` or `|amount1|` exceeds `WHALE_THRESHOLD`
4. Matches are stored in a ring buffer (last 100) and logged to stdout
5. Query whale alerts via `GET /alerts`

Default threshold: `1e18` (1 token with 18 decimals). Override via `whale_threshold` variable.

## Estimated Cost

Your VPS cost + ClickHouse BYODB. A 4GB VPS (~$5-20/mo on Hetzner/OVH) is sufficient.
