#!/bin/bash
# Script to start fastevm-gravity node for benchmarking
# Usage: ./start-bench-node.sh <binary_path> <port> <data_dir> <log_file>
#
# This script starts a fastevm-gravity node optimized for benchmarking
# with settings similar to node.sh for high-throughput transaction processing

set -e

BINARY_PATH=""
PORT="$2"
DATA_DIR="$3"
LOG_FILE="$4"

if [ -z "$BINARY_PATH" ] || [ -z "$PORT" ] || [ -z "$DATA_DIR" ] || [ -z "$LOG_FILE" ]; then
    echo "Usage: $0 <binary_path> <port> <data_dir> <log_file>"
    exit 1
fi

if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Binary not found at $BINARY_PATH"
    exit 1
fi

# Benchmark-optimized configuration (matching node.sh settings)
BUILDER_GAS_LIMIT="${BUILDER_GAS_LIMIT:-240000000}"
BLOCK_INTERVAL_MS="${BLOCK_INTERVAL_MS:-1000}"
GRAVITY_PIPE_BLOCK_GAS_LIMIT="${GRAVITY_PIPE_BLOCK_GAS_LIMIT:-5000000000}"
GRAVITY_CACHE_MAX_PERSIST_GAP="${GRAVITY_CACHE_MAX_PERSIST_GAP:-64}"
ENGINE_PERSISTENCE_THRESHOLD="${ENGINE_PERSISTENCE_THRESHOLD:-0}"

# Clean up previous data if exists
if [ -d "$DATA_DIR" ]; then
    rm -rf "$DATA_DIR"
fi

mkdir -p "$DATA_DIR"

# Start the node with benchmarking-optimized settings
"$BINARY_PATH" node \
    --datadir "$DATA_DIR" \
    --dev \
    --builder.gaslimit "$BUILDER_GAS_LIMIT" \
    --http \
    --http.addr 0.0.0.0 \
    --http.port "$PORT" \
    --http.api eth,net,web3,debug,trace \
    --engine.persistence-threshold "$ENGINE_PERSISTENCE_THRESHOLD" \
    --gravity.pipe-block-gas-limit "$GRAVITY_PIPE_BLOCK_GAS_LIMIT" \
    --gravity.cache.max-persist-gap "$GRAVITY_CACHE_MAX_PERSIST_GAP" \
    --block-interval-ms "$BLOCK_INTERVAL_MS" \
    --txpool.max-pending-txns 1000000 \
    --txpool.pending-max-count 17592186044415 \
    --txpool.pending-max-size 17592186044415 \
    --txpool.basefee-max-count 17592186044415 \
    --txpool.basefee-max-size 17592186044415 \
    --txpool.queued-max-count 17592186044415 \
    --txpool.queued-max-size 17592186044415 \
    --rpc.max-connections 50000 \
    --rpc.max-subscriptions-per-connection 50000 \
    -vvv > "$LOG_FILE" 2>&1 &

NODE_PID=$!
echo "$NODE_PID"

