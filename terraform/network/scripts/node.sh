#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}


# Load environment variables from fastevm.env if it exists
ENV_FILE="${SCRIPT_DIR}/node.env"
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
DATA_DIR="${DATADIR:-/data}"
JWT_KEY_FILE="${JWT_KEY_FILE:-$DATA_DIR/execution/jwt.hex}"
P2P_DIR="${P2P_KEY_FILE:-$DATA_DIR/execution/p2p}"

BUILDER_GAS_LIMIT="${BUILDER_GAS_LIMIT:-240000000}"
BLOCK_TIME="${BLOCK_TIME:-1s}"
BLOCK_MAX_TRANSACTIONS="${BLOCK_MAX_TRANSACTIONS:-10000}"
BLOCK_INTERVAL_MS="${BLOCK_INTERVAL_MS:-1000}"
COMMITTED_SUBDAGS_PER_BLOCK="${COMMITTED_SUBDAGS_PER_BLOCK:-30}"
GRAVITY_PIPE_BLOCK_GAS_LIMIT="${GRAVITY_PIPE_BLOCK_GAS_LIMIT:-5000000000}"
GRAVITY_CACHE_MAX_PERSIST_GAP="${GRAVITY_CACHE_MAX_PERSIST_GAP:-64}"
ENGINE_PERSISTENCE_THRESHOLD="${ENGINE_PERSISTENCE_THRESHOLD:-0}"


setup() {
    # Clean up any stale lock files in the data directory
    local LOCAL_IP="$1"
    local PUBLIC_IP="$2"
    sudo rm -rf "$DATA_DIR"
    ${SCRIPT_DIR}/setup.sh
    init-folders
    init-execution
    init-consensus "$LOCAL_IP" "$PUBLIC_IP"
    # Iinit-consensus is called when update ips
    init-services
}
# Prepare folders, generate keys and create config files
init-folders() {
    sudo mkdir -p $DATA_DIR
    sudo chown -R ubuntu:ubuntu $DATA_DIR
    sudo chmod -R 755 $DATA_DIR
    # Prepare all required directories and files with proper permissions
    log_info "Preparing all required directories and files..."
    
    # Create all data directories
    mkdir -p "$DATA_DIR/execution"
    mkdir -p "$DATA_DIR/consensus"
    mkdir -p "$DATA_DIR/logs"
    mkdir -p "$DATA_DIR/config"
    
    # Create execution subdirectories
    mkdir -p "$DATA_DIR/execution/db"
    mkdir -p "$DATA_DIR/execution/p2p"
    
    # Set ownership for all directories
    chown -R ubuntu:ubuntu "$DATA_DIR/execution"
    chown -R ubuntu:ubuntu "$DATA_DIR/consensus"
    chown -R ubuntu:ubuntu "$DATA_DIR/logs"
    chown -R ubuntu:ubuntu "$DATA_DIR/config"
    
    # Create log files with proper permissions
    touch "$DATA_DIR/logs/fastevm-execution.log"
    touch "$DATA_DIR/logs/fastevm-consensus.log"
    chown ubuntu:ubuntu "$DATA_DIR/logs"/*.log
    chmod 664 "$DATA_DIR/logs"/*.log
    
}
init-execution() {
    # Copy node.env and genesis.json to data directory
    cp $SCRIPT_DIR/node.env $DATA_DIR/node.env
    cp $SCRIPT_DIR/genesis.json $DATA_DIR/config/genesis.json
    chown ubuntu:ubuntu "$DATA_DIR/config/genesis.json"
    chmod 644 "$DATA_DIR/config/genesis.json"
    
    # Ensure JWT secret exists if not already present
    if [ ! -f "$JWT_KEY_FILE" ]; then
        openssl rand -hex 32 | tr -d "\n" > "$JWT_KEY_FILE"
        chown ubuntu:ubuntu "$JWT_KEY_FILE"
        chmod 644 "$JWT_KEY_FILE"
    fi
    
    # Ensure P2P secret key exists if not already present
    if [ ! -f "$P2P_DIR/secret.key" ]; then
        #seed="fastevm-node-${node_index}-p2p-secret-2025"
        #echo "$seed" | openssl dgst -sha256 -hex | cut -d' ' -f2 | tr -d '\n' > "$P2P_DIR/secret.key"
        openssl rand -hex 32 | tr -d "\n" > "$P2P_DIR/secret.key"
        chown ubuntu:ubuntu "$P2P_DIR/secret.key"
        chmod 600 "$P2P_DIR/secret.key"
    fi
    
    # Also create a hex version for easier use
    peer_id_file="${P2P_DIR}/peer-id.hex"
    fastevm-cli show-peer-id --file "$P2P_DIR/secret.key" --output "${P2P_DIR}/peer-id.hex"
    
    # Remove 0x prefix from the hex file content
    if [ -f "$peer_id_file" ]; then
        sed -i 's/^0x//' "$peer_id_file"
    fi
    
    log_success "All directories and files prepared with proper permissions"
}
init-consensus() {
    local LOCAL_IP=$1
    local HOST_NAME=${2:-validator-$LOCAL_IP}
    # Copy node.yml, committees.yml, parameters.yml to data directory
    cp $SCRIPT_DIR/node.yml $DATA_DIR/config/node.yml
    cp $SCRIPT_DIR/parameters.yml $DATA_DIR/config/parameters.yml
    chown ubuntu:ubuntu "$DATA_DIR/config/node.yml"
    chown ubuntu:ubuntu "$DATA_DIR/config/parameters.yml"
    # Generate validator.yml
    evm-consensus generate-validator \
        --validator-path $DATA_DIR/config/validator.yml \
        --authority-path $DATA_DIR/config/authority.yml \
        --stake 1000 \
        --hostname $HOST_NAME \
        --ip-address $LOCAL_IP \
        --port 26657
    # Generate committees.yml
    # /usr/local/bin/evm-consensus generate-committee \
    #     --config-path "$SCRIPT_DIR/validators.yml" \
    #     --committee-path "$DATA_DIR/config/committees.yml" \
    #     --epoch "0"
}
# Initialize services
init-services() {
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

update-ips() {
    export NODE_IPS="$1"
    # Split CSV into array
    IFS=',' read -r -a IP_ARRAY <<< "$NODE_IPS"

    IP_COUNT="${#IP_ARRAY[@]}"
    VALIDATORS_FILE="$SCRIPT_DIR/validators.yml"
    PLACEHOLDER_COUNT=$(grep -o 'CONSENSUS_IP[0-9]\+' "$VALIDATORS_FILE" | wc -l)

    if [ "$IP_COUNT" -ne "$PLACEHOLDER_COUNT" ]; then
    echo "❌ IP count ($IP_COUNT) does not match placeholder count ($PLACEHOLDER_COUNT)"
    exit 1
    fi

    echo "🔁 Updating validators.yml with consensus IPs..."

    for i in "${!IP_ARRAY[@]}"; do
        idx=$((i + 1))
        ip="${IP_ARRAY[$i]}"

        echo "   → CONSENSUS_IP${idx} = ${ip}"

        sed -i \
            "s/CONSENSUS_IP${idx}/${ip}/g" \
            "$VALIDATORS_FILE"
    done
    echo "✅ validators.yml updated"
}

init-db() {

    DATA_DIR="${DATA_DIR:-/data}"

    DB_DIR="$DATA_DIR/execution/db"
    GENESIS="$DATA_DIR/config/genesis.json"

    if [ ! -d "$DB_DIR" ] || [ -z "$(ls -A "$DB_DIR" 2>/dev/null)" ]; then
    echo "Initializing execution DB..."
    /usr/local/bin/fastevm-execution init \
        --datadir "$DATA_DIR/execution" \
        --chain "$GENESIS"
    fi
}
health-check () {
    HTTP_PORT=8545
    ENGINE_PORT=8551

    curl -sf "http://127.0.0.1:${HTTP_PORT}" >/dev/null \
    || { echo "Execution client unhealthy"; exit 1; }

    curl -sf "http://127.0.0.1:${ENGINE_PORT}" >/dev/null \
    || { echo "Engine API unhealthy"; exit 1; }

    echo "All services healthy"
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

status() {

    DATA_DIR="${DATA_DIR:-/data}"

    echo "=== FastEVM Node Status ==="
    systemctl status fastevm-execution --no-pager -l
    echo
    systemctl status fastevm-consensus --no-pager -l
    echo

    echo "=== Ports ==="
    ss -tlnp | grep -E "8545|8546|8551|30303" || true
    echo

    echo "=== Logs ==="
    ls -lh "$DATA_DIR/logs" || true
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