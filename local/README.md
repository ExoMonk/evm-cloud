# Local Dev Stack

Boot a complete evm-cloud pipeline on your laptop: Anvil (EVM simulator) → eRPC (RPC proxy) → rindexer (indexer) → ClickHouse (database).

## Prerequisites

- [Docker Desktop](https://www.docker.com/products/docker-desktop/) (4GB+ memory allocated)
- [kind](https://kind.sigs.k8s.io/docs/user/quick-start/#installation)
- [kubectl](https://kubernetes.io/docs/tasks/tools/)
- [Helm](https://helm.sh/docs/intro/install/)

## Quick Start

```bash
# Boot the stack
./local/up.sh

# Check status and endpoints
./local/status.sh

# Tear down
./local/down.sh
```

Or via Make:

```bash
make local-up
make local-status
make local-down
```

## Endpoints

| Service    | URL                        | Purpose                     |
|------------|----------------------------|-----------------------------|
| Anvil      | http://localhost:8545      | EVM simulator (JSON-RPC)    |
| eRPC       | http://localhost:4000      | RPC proxy (same as prod)    |
| ClickHouse | http://localhost:8123      | Query indexed data (HTTP)   |
| rindexer   | http://localhost:18080     | Indexer metrics             |

## Options

```bash
./local/up.sh [OPTIONS]

  --profile <name>     Resource profile: default, heavy (default: default)
  --persist            Persistent ClickHouse data across restarts
  --force              Force-recreate cluster even if it exists
  --with-monitoring    Deploy Prometheus + Grafana (needs more memory)
  --anvil-fork <url>   Fork mainnet/testnet state from RPC URL
  --post-deploy <sh>   Script to run after stack is healthy
```

## Fork Mode

Index real mainnet contracts locally:

```bash
./local/up.sh --anvil-fork https://eth.llamarpc.com
```

**Important**: In fork mode, set `start_block` in `local/config/rindexer.yaml` close to the fork block number. Historical `eth_getLogs` are proxied to the fork RPC and may rate-limit on large ranges.

## Contract Deploy Hook

Run a script after the stack boots:

```bash
./local/up.sh --post-deploy ./my-deploy.sh
```

The script receives these environment variables:

```
ANVIL_RPC_URL=http://localhost:8545
ERPC_URL=http://localhost:4000
CLICKHOUSE_URL=http://localhost:8123
CHAIN_ID=31337
```

Example:

```bash
#!/usr/bin/env bash
forge script script/Deploy.s.sol --rpc-url $ANVIL_RPC_URL --broadcast
# Update local/config/rindexer.yaml with deployed contract addresses
```

## Resource Profiles

### `default` (laptop-friendly, ~2.5Gi)

For basic contract development. Fits on 8GB+ laptops.

### `heavy` (fork mode, ~8Gi)

For fork-mode Anvil with large state. Requires 16GB+ or Docker Desktop with 8GB+ allocated.

```bash
./local/up.sh --profile heavy --anvil-fork https://eth.llamarpc.com
```

## Persistence

By default, data is ephemeral — lost on `./local/down.sh`. For persistent sessions:

```bash
./local/up.sh --persist
```

Data is stored in `~/.evm-cloud/local-data/`. Note: data does not survive Docker Desktop VM resets on macOS.

To clean and restart:

```bash
./local/reset.sh
```

## Config Changes

Edit files in `local/config/` and re-run `./local/up.sh` — Helm performs an idempotent upgrade. No need to tear down first.

## Troubleshooting

**Port conflict**: Check if 8545/4000/8123/18080 are already bound. `up.sh` reports which port is taken.

**Docker memory**: Increase in Docker Desktop → Settings → Resources → Memory (4GB+ for default, 8GB+ for heavy).

**ClickHouse timeout**: Try `--profile heavy` for more resources. Check `kubectl logs statefulset/clickhouse`.

**rindexer CrashLoop**: Usually means it can't connect to ClickHouse. Check `kubectl logs deploy/local-indexer`.

**Cluster unreachable after sleep**: NodePort mappings persist through sleep. If the cluster is gone, run `./local/up.sh` to recreate.
