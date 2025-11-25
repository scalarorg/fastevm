#!/bin/bash
# Client Node Setup - Step 3: Run Benchmark
# This script runs the gravity_bench benchmark
# This script is optional and can be run separately

set -e
set -o pipefail

# Logging function - outputs to stdout (tee in terraform will handle log file)
log() {
    local message="[$(date '+%Y-%m-%d %H:%M:%S')] $1"
    echo "$message"
}

log "Starting client benchmark: Run benchmark..."

# Determine the user
if [ -n "$SUDO_USER" ]; then
    RUST_USER="$SUDO_USER"
    RUST_HOME=$(getent passwd "$RUST_USER" | cut -d: -f6)
elif id "ubuntu" &>/dev/null; then
    RUST_USER="ubuntu"
    RUST_HOME=$(getent passwd "$RUST_USER" | cut -d: -f6)
else
    RUST_USER="root"
    RUST_HOME="/root"
fi

GRAVITY_BENCH_DIR="/opt/gravity_bench"
BENCH_CONFIG="/opt/bench_config.toml"

# Check if build is complete (either marker file exists or binary exists)
# if [ ! -f /var/log/client-build-complete ] && [ ! -f "$GRAVITY_BENCH_DIR/target/release/gravity_bench" ]; then
#     log "ERROR: Client build (Build client code and prepare config) must be completed first"
#     log "Please run client-build.sh first"
#     exit 1
# fi

# Verify execution node is ready
# log "Verifying execution node is ready..."
# EXECUTION_IP="${execution_node_internal_ip}"
# EXECUTION_PORT="${http_port}"
# MAX_ATTEMPTS=30
# ATTEMPT=1

# while [ $ATTEMPT -le $MAX_ATTEMPTS ]; do
#     if curl -s "http://$${EXECUTION_IP}:$${EXECUTION_PORT}" > /dev/null 2>&1; then
#         log "Execution node is ready at $${EXECUTION_IP}:$${EXECUTION_PORT}"
#         break
#     fi
#     [ $ATTEMPT -eq $MAX_ATTEMPTS ] && {
#         log "ERROR: Execution node not ready after $MAX_ATTEMPTS attempts"
#         exit 1
#     }
#     log "Waiting for execution node... (attempt $ATTEMPT/$MAX_ATTEMPTS)"
#     sleep 5
#     ATTEMPT=$((ATTEMPT + 1))
# done

# Verify binary exists
if [ ! -f "$GRAVITY_BENCH_DIR/target/release/gravity_bench" ]; then
    log "ERROR: gravity_bench binary not found at $GRAVITY_BENCH_DIR/target/release/gravity_bench"
    log "Please run step 2 first to build the binary"
    exit 1
fi

# Verify config exists
if [ ! -f "$BENCH_CONFIG" ]; then
    log "ERROR: bench_config.toml not found at $BENCH_CONFIG"
    log "Please run step 2 first to create the config"
    exit 1
fi

# Create logs directory
mkdir -p "$GRAVITY_BENCH_DIR/logs"
chown -R "$RUST_USER:$RUST_USER" "$GRAVITY_BENCH_DIR/logs" 2>/dev/null || true

# Start benchmark
log "Starting gravity_bench benchmark..."
log "Config file: $BENCH_CONFIG"
log "Log file: $GRAVITY_BENCH_DIR/logs/gravity-bench.log"

# Source environment file if it exists
if [ -f "$GRAVITY_BENCH_DIR/.env" ]; then
    source "$GRAVITY_BENCH_DIR/.env"
fi

# Run benchmark in background with logging
cd "$GRAVITY_BENCH_DIR"
sudo -u "$RUST_USER" env \
    PATH="/opt/gravity_bench/venv/bin:$RUST_HOME/.cargo/bin:$PATH" \
    RUST_LOG=debug \
    nohup "$GRAVITY_BENCH_DIR/target/release/gravity_bench" \
    --config "$BENCH_CONFIG" \
    > "$GRAVITY_BENCH_DIR/logs/gravity-bench.log" 2>&1 &

BENCH_PID=$!
echo $BENCH_PID > "$GRAVITY_BENCH_DIR/logs/gravity-bench.pid"
chown "$RUST_USER:$RUST_USER" "$GRAVITY_BENCH_DIR/logs/gravity-bench.pid" 2>/dev/null || true

log "Benchmark started with PID: $BENCH_PID"
log "Monitor logs with: tail -f $GRAVITY_BENCH_DIR/logs/gravity-bench.log"
log "Check status with: ps -p $BENCH_PID"

# Create a marker file to indicate benchmark is complete
touch /var/log/client-benchmark-complete || true

log "Client benchmark completed successfully!"
log "Benchmark is running in the background"
log "PID: $BENCH_PID"
log "Logs: $GRAVITY_BENCH_DIR/logs/gravity-bench.log"

# Explicitly exit with success status
exit 0

