#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Load environment variables from fastevm.env if it exists
ENV_FILE="${SCRIPT_DIR}/fastevm.env"
if [ -f "$ENV_FILE" ]; then
    echo "📄 Loading environment variables from $ENV_FILE"
    set -a  # Automatically export all variables
    source "$ENV_FILE"
    set +a
    echo "✅ Environment variables loaded"
else
    echo "⚠️  fastevm.env not found, using default values"
fi

# Default values (used if not set in fastevm.env)
DATA_DIR="${DATADIR:-${SCRIPT_DIR}/../data}"
BUILDER_GAS_LIMIT="${BUILDER_GAS_LIMIT:-240000000}"
BLOCK_TIME="${BLOCK_TIME:-1s}"
BLOCK_MAX_TRANSACTIONS="${BLOCK_MAX_TRANSACTIONS:-10000}"
BLOCK_INTERVAL_MS="${BLOCK_INTERVAL_MS:-1000}"
COMMITTED_SUBDAGS_PER_BLOCK="${COMMITTED_SUBDAGS_PER_BLOCK:-30}"
GRAVITY_PIPE_BLOCK_GAS_LIMIT="${GRAVITY_PIPE_BLOCK_GAS_LIMIT:-5000000000}"
GRAVITY_CACHE_MAX_PERSIST_GAP="${GRAVITY_CACHE_MAX_PERSIST_GAP:-64}"
ENGINE_PERSISTENCE_THRESHOLD="${ENGINE_PERSISTENCE_THRESHOLD:-0}"

# Clean up any stale lock files in the data directory
rm -rf $DATA_DIR

setup() {
    ${SCRIPT_DIR}/setup.sh
    # Install systemd services
    echo "Installing FastEVM systemd services..."

    if [ -f "$SCRIPT_DIR/service.sh" ]; then
        if sudo bash "$SCRIPT_DIR/service.sh" install; then
            echo "FastEVM services installed successfully!"
        else
            echo "Warning: Service installation failed. You can try installing manually with:"
            echo "  sudo bash $SCRIPT_DIR/service.sh install"
        fi
    else
        echo "Warning: service.sh not found at $SCRIPT_DIR/service.sh"
        echo "Service installation skipped. You can install services manually with:"
        echo "  sudo bash $SCRIPT_DIR/service.sh install"
    fi
}

start() {
    echo "Starting FastEVM services via systemd..."
    if [ -f "$SCRIPT_DIR/service.sh" ]; then
        if sudo bash "$SCRIPT_DIR/service.sh" start; then
            echo "✅ FastEVM services started successfully"
            echo "Check status with: sudo systemctl status fastevm-execution fastevm-consensus"
        else
            echo "❌ Failed to start FastEVM services"
            echo "Check logs with: sudo bash $SCRIPT_DIR/service.sh logs"
            exit 1
        fi
    else
        echo "❌ service.sh not found at $SCRIPT_DIR/service.sh"
        echo "Cannot start services. Please ensure services are installed."
        exit 1
    fi
}

stop() {
    echo "Stopping FastEVM services via systemd..."
    if [ -f "$SCRIPT_DIR/service.sh" ]; then
        if sudo bash "$SCRIPT_DIR/service.sh" stop; then
            echo "✅ FastEVM services stopped successfully"
        else
            echo "⚠️  Warning: Some services may not have stopped cleanly"
        fi
    else
        echo "⚠️  service.sh not found. Attempting to stop processes manually..."
        # Fallback: Stop any existing processes
        if pgrep -f "fastevm-execution" > /dev/null; then
            echo "Stopping fastevm-execution processes..."
            pkill -f "fastevm-execution" || true
            sleep 2
            pkill -9 -f "fastevm-execution" || true
        fi
        if pgrep -f "evm-consensus" > /dev/null; then
            echo "Stopping evm-consensus processes..."
            pkill -f "evm-consensus" || true
            sleep 2
            pkill -9 -f "evm-consensus" || true
        fi
    fi
}

status() {
    echo "Checking FastEVM service status..."
    if [ -f "$SCRIPT_DIR/service.sh" ]; then
        sudo bash "$SCRIPT_DIR/service.sh" status
    else
        echo "⚠️  service.sh not found. Checking processes..."
        if pgrep -f "fastevm-execution" > /dev/null; then
            echo "✅ fastevm-execution is running"
        else
            echo "❌ fastevm-execution is not running"
        fi
        if pgrep -f "evm-consensus" > /dev/null; then
            echo "✅ evm-consensus is running"
        else
            echo "❌ evm-consensus is not running"
        fi
    fi
}

$@