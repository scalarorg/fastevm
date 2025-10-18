#!/bin/bash
# FastEVM Node Setup Script
# This script initializes the chain, generates peer IDs, and replaces placeholders in config files

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

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Load environment variables from node.env file
if [ -f "/tmp/fastevm-config/node.env" ]; then
    log_info "Loading environment variables from node.env file..."
    source /tmp/fastevm-config/node.env
    log_success "Environment variables loaded successfully"
else
    log_error "node.env file not found at /tmp/fastevm-config/node.env"
    exit 1
fi

log_info "Starting FastEVM node $NODE_INDEX setup..."

# Step 1: Initialize chain with prefunded accounts
log_info "Step 1: Initializing chain with prefunded accounts..."

# Check if CLI is available
if [ ! -f "/usr/local/bin/cli" ]; then
    log_error "CLI not found at /usr/local/bin/cli"
    log_error "Please ensure binaries are installed before running setup-node"
    exit 1
fi

# Check if genesis file exists
if [ ! -f "/tmp/fastevm-config/genesis.json" ]; then
    log_error "Genesis file not found at /tmp/fastevm-config/genesis.json"
    exit 1
fi

# Generate prefunded accounts
log_info "Generating prefunded accounts..."
if /usr/local/bin/cli allocate-funds \
    --input "/tmp/fastevm-config/genesis.json" \
    --count "${PREFUND_ACCOUNT_COUNT:-100000}" \
    --mnemonic "${TEST_MNEMONIC:-abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about}" \
    --amount "${PREFUND_BALANCE:-1000000000000000000000}" \
    --output "/data"; then
    log_success "Generated prefunded accounts successfully"
else
    log_error "Failed to generate prefunded accounts"
    exit 1
fi

# Step 2: Generate peer IDs using CLI
log_info "Step 2: Generating peer IDs using CLI..."

# Create P2P directory if it doesn't exist
mkdir -p /data/execution/p2p

# Generate peer ID for this node
log_info "Generating peer ID for node $NODE_INDEX..."
echo "$P2P_SECRET_KEY" > /data/execution/p2p/secret.key

if /usr/local/bin/cli show-peer-id --file /data/execution/p2p/secret.key --output /data/execution/p2p/secret.hex; then
    # Remove 0x prefix if present
    sed -i 's/^0x//' /data/execution/p2p/secret.hex
    PEER_ID=$(cat /data/execution/p2p/secret.hex)
    log_success "Generated peer ID for node $NODE_INDEX: $PEER_ID"
else
    log_error "Failed to generate peer ID for node $NODE_INDEX"
    exit 1
fi

# Step 3: Generate bootnodes and replace placeholders in configuration files
log_info "Step 3: Generating bootnodes and replacing placeholders in configuration files..."

# Generate peer IDs for all other nodes and store them
log_info "Generating peer IDs for all other nodes..."
declare -A OTHER_PEER_IDS
BOOTNODES=""

for j in $(seq 0 $((NODE_COUNT - 1))); do
    if [ $j -ne $NODE_INDEX ]; then
        # Generate peer ID for other node
        OTHER_NODE_SEED="fastevm-node-$((j+1))-p2p-secret-2025"
        OTHER_SECRET_KEY=$(echo "$OTHER_NODE_SEED" | openssl dgst -sha256 -binary | openssl dgst -sha256 -hex | cut -d' ' -f2 | tr -d '\n')
        
        # Generate peer ID for other node
        TEMP_SECRET_FILE="/tmp/other-secret-$j.key"
        TEMP_PEER_FILE="/tmp/other-peer-$j.hex"
        echo "$OTHER_SECRET_KEY" > "$TEMP_SECRET_FILE"
        
        if /usr/local/bin/cli show-peer-id --file "$TEMP_SECRET_FILE" --output "$TEMP_PEER_FILE"; then
            sed -i 's/^0x//' "$TEMP_PEER_FILE"
            OTHER_PEER_ID=$(cat "$TEMP_PEER_FILE")
            OTHER_PEER_IDS[$j]=$OTHER_PEER_ID
            
            # Get the IP for the other node (assuming sequential IPs starting from NODE_IP)
            OTHER_NODE_IP_PART=$(echo "$NODE_IP" | cut -d'.' -f1-3)
            OTHER_NODE_IP="$OTHER_NODE_IP_PART.$((10 + j))"
            BOOTNODES="$BOOTNODES,enode://$OTHER_PEER_ID@$OTHER_NODE_IP:$P2P_PORT"
            
            rm -f "$TEMP_SECRET_FILE" "$TEMP_PEER_FILE"
        else
            log_warning "Failed to generate peer ID for node $j, using secret key as fallback"
            OTHER_PEER_IDS[$j]=$OTHER_SECRET_KEY
            # Get the IP for the other node
            OTHER_NODE_IP_PART=$(echo "$NODE_IP" | cut -d'.' -f1-3)
            OTHER_NODE_IP="$OTHER_NODE_IP_PART.$((10 + j))"
            BOOTNODES="$BOOTNODES,enode://$OTHER_SECRET_KEY@$OTHER_NODE_IP:$P2P_PORT"
        fi
    fi
done

# Remove leading comma if present
BOOTNODES=$(echo "$BOOTNODES" | sed 's/^,//')
log_info "Generated bootnodes: $BOOTNODES"

# Append bootnodes to node.env file
log_info "Appending bootnodes to node.env file..."
if [ -f "/data/node.env" ]; then
    echo "" >> /data/node.env
    echo "# Generated bootnodes" >> /data/node.env
    echo "BOOTNODES=\"$BOOTNODES\"" >> /data/node.env
    log_success "Bootnodes appended to node.env"
else
    log_warning "node.env file not found at /data/node.env"
fi

# Replace placeholders in execution.toml
if [ -f "/data/execution.toml" ]; then
    log_info "Replacing placeholders in execution.toml..."
    sed -i "s|{{PEER_ID_$NODE_INDEX}}|$PEER_ID|g" /data/execution.toml
    sed -i "s|{{BOOTNODES}}|$BOOTNODES|g" /data/execution.toml
    
    # Replace placeholders for other nodes' peer IDs using stored values
    for j in $(seq 0 $((NODE_COUNT - 1))); do
        if [ $j -ne $NODE_INDEX ] && [ -n "${OTHER_PEER_IDS[$j]}" ]; then
            sed -i "s|{{PEER_ID_$j}}|${OTHER_PEER_IDS[$j]}|g" /data/execution.toml
        fi
    done
    cat /data/execution.toml
    log_success "Replaced placeholders in execution.toml"
else
    log_warning "execution.toml not found, skipping placeholder replacement"
fi

# Replace placeholders in node.yml
if [ -f "/data/node.yml" ]; then
    log_info "Replacing placeholders in node.yml..."
    sed -i "s|{{PEER_ID_$NODE_INDEX}}|$PEER_ID|g" /data/node.yml
    log_success "Replaced placeholders in node.yml"
else
    log_warning "node.yml not found, skipping placeholder replacement"
fi

# Step 4: Initialize execution node with genesis
log_info "Step 4: Initializing execution node with genesis..."
if [ -f "/data/genesis.json" ] && [ -f "/usr/local/bin/fastevm-execution" ]; then
    log_info "Initializing execution node with genesis..."
    if /usr/local/bin/fastevm-execution init --datadir /data/execution --chain /data/genesis.json; then
        log_success "Execution node initialized successfully"
        cat /data/execution.toml
    else
        log_warning "Execution node initialization completed with warnings"
    fi
else
    log_warning "Skipping execution node initialization (genesis or binary not available)"
fi

# Step 5: Display final configuration
log_success "=== FastEVM Node $NODE_INDEX Setup Completed Successfully ==="

echo ""
log_info "=== FINAL NODE CONFIGURATION ==="
echo ""
log_info "Peer ID generated: $PEER_ID"
# Display node environment summary
log_info "Node Environment:"
echo "  NODE_INDEX: $NODE_INDEX"
echo "  NODE_IP: $NODE_IP"
echo "  HTTP_PORT: $HTTP_PORT"
echo "  WS_PORT: $WS_PORT"
echo "  ENGINE_PORT: $ENGINE_PORT"
echo "  CONSENSUS_PORT: $CONSENSUS_PORT"
echo "  P2P_PORT: $P2P_PORT"
echo "  PEER_ID: $PEER_ID"
echo "  BOOTNODES: $BOOTNODES"
echo ""

# Display execution.toml configuration if it exists
if [ -f "/data/execution.toml" ]; then
    log_info "Execution Configuration (execution.toml):"
    echo "----------------------------------------"
    cat /data/execution.toml
    echo "----------------------------------------"
    echo ""
else
    log_warning "execution.toml not found at /data/execution.toml"
fi

# Display node.yml configuration if it exists
if [ -f "/data/node.yml" ]; then
    log_info "Consensus Configuration (node.yml):"
    echo "----------------------------------------"
    cat /data/node.yml
    echo "----------------------------------------"
    echo ""
else
    log_warning "node.yml not found at /data/node.yml"
fi

# Display network configuration
log_info "Network Configuration:"
echo "  RPC Endpoints:"
echo "    HTTP RPC: http://$NODE_IP:$HTTP_PORT"
echo "    WebSocket RPC: ws://$NODE_IP:$WS_PORT"
echo "    Engine API: http://$NODE_IP:$ENGINE_PORT"
echo "  P2P Configuration:"
echo "    Peer ID: $PEER_ID"
echo "    P2P Port: $P2P_PORT"
echo ""

log_success "Node $NODE_INDEX is ready for service startup!"
