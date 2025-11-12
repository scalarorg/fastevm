# Benchmark Scripts

This directory contains helper scripts for running reth benchmarks.

## Unified Script: `reth_bench.sh`

The main script that combines all functionality into a single command with subcommands.

### Commands

#### `start` - Start a reth node in the background
```bash
# Simplest usage (JWT secret auto-generated at default location)
./reth_bench.sh start

# With presets
./reth_bench.sh start mainnet
./reth_bench.sh start sepolia with-metrics

# Start node and run benchmark immediately
./reth_bench.sh start mainnet quick --run-bench
./reth_bench.sh start --run-bench --advance 10

# Custom options
./reth_bench.sh start --profile maxperf --datadir ~/.reth
```

#### `stop` - Stop a running reth node
```bash
./reth_bench.sh stop
```

#### `bench` - Run a benchmark
```bash
# Simplest usage (uses defaults for JWT secret and RPC URL)
./reth_bench.sh bench mainnet quick
./reth_bench.sh bench --advance 10

# With presets
./reth_bench.sh bench production medium
./reth_bench.sh bench local quick

# Use existing node
./reth_bench.sh bench --use-existing-node --advance 10

# Custom block range
./reth_bench.sh bench --from 21000000 --to 21000100

# With custom RPC URL
./reth_bench.sh bench mainnet quick --rpc-url http://your-rpc:8545
```

## Features

### Automatic Binary Detection
The script automatically finds `reth` and `reth-bench` binaries:
1. First checks if they're in your PATH
2. If not found, looks in `target/{profile}/` directory (default profile: `release`)
3. Supports multiple profiles: `release`, `debug`, `maxperf`, `profiling`, etc.

Use `--profile` to specify a different build profile:
```bash
./reth_bench.sh start --profile maxperf
./reth_bench.sh bench --profile profiling mainnet quick
```

### Automatic JWT Secret Generation
- JWT secrets are automatically generated if missing
- Default location: `~/.local/share/reth/{chain}/jwt.hex`
- Can be overridden with `--jwt-secret PATH`
- Directory structure is created automatically

### Default Values
- **JWT Secret**: `~/.local/share/reth/{chain}/jwt.hex` (auto-generated if missing)
- **RPC URL**: `http://localhost:8545`
- **Profile**: `release`
- **Chain**: `mainnet`
- **Engine RPC URL**: `http://127.0.0.1:8551`

## Presets

Presets provide pre-filled defaults for common scenarios. Use them without the `--` prefix:

### Chain Presets
- `mainnet` - Mainnet with default datadir
- `sepolia` - Sepolia testnet
- `holesky` - Holesky testnet

### Test Duration Presets
- `quick` - 10 blocks
- `medium` - 100 blocks
- `long` - 1000 blocks

### Configuration Presets
- `with-metrics` - Enable metrics on localhost:9001
- `local` - Local development (assumes local RPC)
- `production` - Mainnet with metrics enabled

### Combining Presets
You can combine multiple presets:
```bash
./reth_bench.sh bench mainnet quick with-metrics
./reth_bench.sh start production --run-bench
```

## Examples

### Example 1: Quick start (minimal arguments)
```bash
# Start node with defaults
./reth_bench.sh start

# Run quick benchmark
./reth_bench.sh bench mainnet quick
```

### Example 2: Start node and run benchmark
```bash
# Start node and immediately run a 10-block benchmark
./reth_bench.sh start mainnet quick --run-bench
```

### Example 3: Using different profiles
```bash
# Use maxperf profile for performance testing
./reth_bench.sh start --profile maxperf
./reth_bench.sh bench --profile maxperf mainnet medium

# Use profiling profile for analysis
./reth_bench.sh bench --profile profiling mainnet quick
```

### Example 4: Using existing node
```bash
# Terminal 1: Start node
./reth_bench.sh start mainnet

# Terminal 2: Run multiple benchmarks against the same node
./reth_bench.sh bench --use-existing-node --advance 10
./reth_bench.sh bench --use-existing-node --advance 50

# When done, stop the node
./reth_bench.sh stop
```

### Example 5: Custom configuration
```bash
# Custom datadir, metrics, and profile
./reth_bench.sh start \
  --datadir ~/.reth-custom \
  --metrics localhost:9001 \
  --profile maxperf

# Custom block range with output
./reth_bench.sh bench \
  --from 21000000 \
  --to 21000100 \
  --bench-output ./results
```

## Default Paths

The scripts use these default paths (can be overridden):
- **JWT Secret**: `~/.local/share/reth/{chain}/jwt.hex`
- **Data Directory**: `~/.local/share/reth`
- **PID File**: `/tmp/reth-node.pid`
- **Log File**: `/tmp/reth-node.log`
- **Binaries**: `target/{profile}/` (auto-detected)

## Notes

- The `--rpc-url` should point to a different node than the one being benchmarked (it's used to fetch block data)
- The node being benchmarked runs on `http://127.0.0.1:8551` by default (engine API)
- Make sure to unwind your node before benchmarking if needed: `reth stage unwind to-block <block_number>`
- JWT secrets are automatically generated if missing - no manual setup required
- Binaries are automatically detected from `target/{profile}/` - no need to specify paths

## Dev Node Scripts

FastEVM provides scripts for starting development nodes with custom genesis configurations containing prefunded accounts.

### `dev-node.sh` - Full-Featured Dev Node Manager

A comprehensive script for managing a single FastEVM dev node with background execution, status checking, and log management.

**Features:**
- Background node execution with PID management
- Automatic genesis generation with configurable prefunded accounts
- Status checking and log viewing
- Support for large numbers of prefunded accounts (default: 100,000)
- WebSocket and HTTP RPC support
- Automatic project building

**Commands:**
```bash
# Start dev node with default settings (100k prefunded accounts)
./scripts/dev-node.sh start

# Start with custom number of accounts
./scripts/dev-node.sh start --accounts 500000

# Start with custom account balance
./scripts/dev-node.sh start --accounts 10000 --amount 500000000000000000000

# Check node status
./scripts/dev-node.sh status

# View logs (follow mode)
./scripts/dev-node.sh logs

# Stop the node
./scripts/dev-node.sh stop

# Regenerate genesis with new accounts
./scripts/dev-node.sh regenerate-genesis --accounts 200000

# Clean up all data
./scripts/dev-node.sh cleanup
```

**Options:**
- `--accounts N` - Number of prefunded accounts (default: 100000)
- `--amount X` - Amount in wei per account (default: 1000000000000000000000 = 1000 ETH)
- `--mnemonic "..."` - Mnemonic for account generation
- `--http-port PORT` - HTTP RPC port (default: 8545)
- `--ws-port PORT` - WebSocket RPC port (default: 8546)
- `--engine-port PORT` - Engine API port (default: 8551)
- `--p2p-port PORT` - P2P port (default: 30303)
- `--no-ws` - Disable WebSocket support
- `--no-build` - Skip building the project

**Example:**
```bash
# Start with 500k accounts, each with 2000 ETH
./scripts/dev-node.sh start \
  --accounts 500000 \
  --amount 2000000000000000000000 \
  --http-port 8545
```

**Foreground Mode:**

The script supports running in foreground mode (like the old `dev-start.sh`), which is useful for interactive development and debugging:

```bash
# Start in foreground (logs appear in terminal, blocks)
./scripts/dev-node.sh start --foreground

# Or use the short form
./scripts/dev-node.sh start --fg
```

**When to use which mode:**
- Use **background mode** (default) when you want to run the node in the background and manage it with status/logs commands
- Use **foreground mode** (`--foreground`) when you want to see logs directly in the terminal or for quick testing

## Legacy Scripts

The following scripts are still available but are superseded by `reth_bench.sh`:

- `start_node.sh` - Use `./reth_bench.sh start` instead
- `stop_node.sh` - Use `./reth_bench.sh stop` instead
- `run_bench.sh` - Use `./reth_bench.sh bench` instead
