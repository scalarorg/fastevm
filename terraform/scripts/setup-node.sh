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

# Set data directory - can be overridden via environment variable
DATA_DIR="${DATA_DIR:-/data}"

# Setup NVMe disk early (before creating data directories)
log_info "Setting up NVMe disk if available..."
NVME_DEVICE="/dev/nvme0n1"
MOUNT_POINT="/data"

if [ -b "$NVME_DEVICE" ]; then
    log_info "NVMe device $NVME_DEVICE found, setting up..."
    
    # Check if already mounted at /data
    if mountpoint -q "$MOUNT_POINT" 2>/dev/null; then
        log_info "$MOUNT_POINT is already mounted, skipping setup"
    else
        # Check if device is mounted elsewhere
        if grep -q "^$NVME_DEVICE " /proc/mounts 2>/dev/null; then
            OTHER_MOUNT=$(grep "^$NVME_DEVICE " /proc/mounts | awk '{print $2}')
            log_info "$NVME_DEVICE is already mounted at $OTHER_MOUNT, skipping"
        else
            # Check if device has filesystem
            if ! blkid "$NVME_DEVICE" >/dev/null 2>&1; then
                log_info "Creating ext4 filesystem on $NVME_DEVICE..."
                sudo mkfs.ext4 -F "$NVME_DEVICE"
            fi
            
            # Create mount point
            sudo mkdir -p "$MOUNT_POINT"
            
            # Mount the device
            log_info "Mounting $NVME_DEVICE to $MOUNT_POINT..."
            if sudo mount "$NVME_DEVICE" "$MOUNT_POINT"; then
                log_success "Successfully mounted $NVME_DEVICE to $MOUNT_POINT"
                
                # Add to /etc/fstab for persistent mounting
                if ! grep -q "^$NVME_DEVICE" /etc/fstab 2>/dev/null; then
                    echo "$NVME_DEVICE $MOUNT_POINT ext4 defaults,nofail 0 2" | sudo tee -a /etc/fstab > /dev/null
                    log_success "Added $NVME_DEVICE to /etc/fstab"
                fi
            else
                log_warning "Failed to mount $NVME_DEVICE, will use boot disk for /data"
            fi
        fi
    fi
    
    # Set ownership
    sudo chown ubuntu:ubuntu "$MOUNT_POINT" 2>/dev/null || true
else
    log_info "No NVMe device found, will use boot disk for /data"
fi

# Load environment variables from node.env file
if [ -f "/tmp/fastevm-config/node.env" ]; then
    log_info "Loading environment variables from node.env file..."
    source /tmp/fastevm-config/node.env
    cp /tmp/fastevm-config/node.env "$DATA_DIR/node.env"
    log_success "Environment variables loaded successfully"
else
    log_error "node.env file not found at /tmp/fastevm-config/node.env"
    exit 1
fi
ls -la "$DATA_DIR"
log_info "Starting FastEVM node $NODE_INDEX setup..."

# Install required system packages
log_info "Installing required system packages..."
REQUIRED_PACKAGES="curl wget git build-essential pkg-config libssl-dev libclang-dev cmake jq htop vim unzip software-properties-common apt-transport-https ca-certificates gnupg lsb-release"
MISSING_PACKAGES=""

for pkg in $REQUIRED_PACKAGES; do
    if ! dpkg -l | grep -q "^ii  $pkg "; then
        MISSING_PACKAGES="$MISSING_PACKAGES $pkg"
    fi
done

if [ -n "$MISSING_PACKAGES" ]; then
    log_info "Some packages are missing, installing required packages..."
    
    # Set non-interactive mode for package installation
    export DEBIAN_FRONTEND=noninteractive
    export DEBIAN_PRIORITY=critical
    
    # Update system packages
    log_info "Updating package lists..."
    if ! sudo env DEBIAN_FRONTEND=noninteractive apt-get update -y; then
        log_error "Failed to update package lists"
        exit 1
    fi

    # Fix any broken packages (common issue with google-compute-engine)
    log_info "Fixing any broken packages..."
    sudo env DEBIAN_FRONTEND=noninteractive dpkg --configure -a || true
    sudo env DEBIAN_FRONTEND=noninteractive apt-get install -f -y -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold" || true

    if ! sudo env DEBIAN_FRONTEND=noninteractive apt-get upgrade -y -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold"; then
        log_warning "Package upgrade had some issues, but continuing..."
    fi

    # Install required packages
    log_info "Installing missing packages:$MISSING_PACKAGES"
    # Try to install packages, handling google-compute-engine errors gracefully
    # Use --force-confdef and --force-confold to automatically handle config file conflicts
    if ! sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold" \
        curl \
        wget \
        git \
        build-essential \
        pkg-config \
        libssl-dev \
        libclang-dev \
        cmake \
        jq \
        htop \
        vim \
        unzip \
        software-properties-common \
        apt-transport-https \
        ca-certificates \
        gnupg \
        lsb-release 2>&1 | tee /tmp/apt-install.log; then
        # Check if the error is related to google-compute-engine
        if grep -q "google-compute-engine" /tmp/apt-install.log; then
            log_warning "google-compute-engine package had issues, attempting to fix..."
            # Try to fix the google-compute-engine package
            sudo env DEBIAN_FRONTEND=noninteractive dpkg --configure -a || true
            sudo env DEBIAN_FRONTEND=noninteractive apt-get install -f -y -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold" || true
            # Try installing packages again, excluding google-compute-engine if needed
            sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold" \
                curl \
                wget \
                git \
                build-essential \
                pkg-config \
                libssl-dev \
                libclang-dev \
                cmake \
                jq \
                htop \
                vim \
                unzip \
                software-properties-common \
                apt-transport-https \
                ca-certificates \
                gnupg \
                lsb-release || {
                log_error "Failed to install required packages even after fixing google-compute-engine"
                exit 1
            }
        else
            log_error "Failed to install required packages"
            exit 1
        fi
    fi
    log_success "Required packages installed successfully"
else
    log_info "All required packages are already installed"
fi

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

# Generate prefunded accounts (check if already generated with correct count)
log_info "Generating prefunded accounts..."
PREFUND_ACCOUNT_COUNT="${PREFUND_ACCOUNT_COUNT:-100000}"
PREFUND_BALANCE="${PREFUND_BALANCE:-1000000000000000000000}"
TEST_MNEMONIC="${TEST_MNEMONIC:-abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about}"

# Check if genesis.json exists and has the expected number of prefunded accounts
SHOULD_GENERATE=true
if [ -f "$DATA_DIR/config/genesis.json" ] && [ -s "$DATA_DIR/config/genesis.json" ]; then
    # Check if jq is available to count accounts
    if command -v jq &> /dev/null; then
        CURRENT_ACCOUNT_COUNT=$(jq '.alloc | length' "$DATA_DIR/config/genesis.json" 2>/dev/null || echo "0")
        log_info "Current genesis.json has $CURRENT_ACCOUNT_COUNT accounts in alloc section"
        # Check if we have at least the expected number of prefunded accounts
        # We allow some margin (at least 90% of expected) to account for base accounts
        if [ "$CURRENT_ACCOUNT_COUNT" -ge "$PREFUND_ACCOUNT_COUNT" ]; then
            log_info "Genesis file already has sufficient prefunded accounts ($CURRENT_ACCOUNT_COUNT >= $PREFUND_ACCOUNT_COUNT), skipping regeneration"
            SHOULD_GENERATE=false
        else
            log_warning "Genesis file exists but has insufficient accounts ($CURRENT_ACCOUNT_COUNT < $PREFUND_ACCOUNT_COUNT), regenerating..."
        fi
    else
        log_warning "jq not available, cannot verify account count. Regenerating to be safe..."
    fi
fi

if [ "$SHOULD_GENERATE" = true ]; then
    log_info "Generating $PREFUND_ACCOUNT_COUNT prefunded accounts with balance $PREFUND_BALANCE wei..."
    if sudo /usr/local/bin/cli allocate-funds \
        --input "/tmp/fastevm-config/genesis.json" \
        --count "$PREFUND_ACCOUNT_COUNT" \
        --mnemonic "$TEST_MNEMONIC" \
        --amount "$PREFUND_BALANCE" \
        --output "$DATA_DIR/config"; then
        sudo chown -R ubuntu:ubuntu "$DATA_DIR/config"
        
        # Verify the generated file
        if [ -f "$DATA_DIR/config/genesis.json" ]; then
            if command -v jq &> /dev/null; then
                FINAL_ACCOUNT_COUNT=$(jq '.alloc | length' "$DATA_DIR/config/genesis.json" 2>/dev/null || echo "0")
                log_success "Generated prefunded accounts successfully (total accounts: $FINAL_ACCOUNT_COUNT)"
            else
                log_success "Generated prefunded accounts successfully"
            fi
        else
            log_error "Genesis file was not created at $DATA_DIR/config/genesis.json"
            exit 1
        fi
    else
        log_error "Failed to generate prefunded accounts"
        exit 1
    fi
fi

# Step 2: Generate peer IDs using CLI
log_info "Step 2: Generating peer IDs using CLI..."

# Create P2P directory if it doesn't exist
sudo mkdir -p "$DATA_DIR/execution/p2p"

# Generate peer ID for this node
log_info "Generating peer ID for node $NODE_INDEX..."
echo -n "$P2P_SECRET_KEY" | sudo tee "$DATA_DIR/execution/p2p/secret.key" > /dev/null
sudo chmod 600 "$DATA_DIR/execution/p2p/secret.key"
sudo chown ubuntu:ubuntu -R "$DATA_DIR/execution"

if /usr/local/bin/cli show-peer-id --file "$DATA_DIR/execution/p2p/secret.key" --output "$DATA_DIR/execution/p2p/secret.hex"; then
    # Remove 0x prefix if present
    sed -i 's/^0x//' "$DATA_DIR/execution/p2p/secret.hex"
    PEER_ID=$(cat "$DATA_DIR/execution/p2p/secret.hex")
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
            
            # Get the actual IP for the other node from ALL_NODE_IPS
            IFS=' ' read -ra NODE_IPS_ARRAY <<< "$ALL_NODE_IPS"
            OTHER_NODE_IP="${NODE_IPS_ARRAY[$j]}"
            BOOTNODES="$BOOTNODES,enode://$OTHER_PEER_ID@$OTHER_NODE_IP:$P2P_PORT"
            
            rm -f "$TEMP_SECRET_FILE" "$TEMP_PEER_FILE"
        else
            log_warning "Failed to generate peer ID for node $j, using secret key as fallback"
            OTHER_PEER_IDS[$j]=$OTHER_SECRET_KEY
            # Get the actual IP for the other node from ALL_NODE_IPS
            IFS=' ' read -ra NODE_IPS_ARRAY <<< "$ALL_NODE_IPS"
            OTHER_NODE_IP="${NODE_IPS_ARRAY[$j]}"
            BOOTNODES="$BOOTNODES,enode://$OTHER_SECRET_KEY@$OTHER_NODE_IP:$P2P_PORT"
        fi
    fi
done

# Remove leading comma if present
BOOTNODES=$(echo "$BOOTNODES" | sed 's/^,//')
log_info "Generated bootnodes: $BOOTNODES"

# Append bootnodes to node.env file (only if not already present)
log_info "Appending bootnodes to node.env file..."
if [ -f "$DATA_DIR/node.env" ]; then
    # Check if bootnodes already exist
    if grep -q "^BOOTNODES=" "$DATA_DIR/node.env" 2>/dev/null; then
        log_info "Bootnodes already exist in node.env, updating..."
        # Remove old bootnodes line and add new one
        sed -i '/^BOOTNODES=/d' "$DATA_DIR/node.env"
        # Remove the comment if it exists
        sed -i '/^# Generated bootnodes$/d' "$DATA_DIR/node.env"
    fi
    echo "" >> "$DATA_DIR/node.env"
    echo "# Generated bootnodes" >> "$DATA_DIR/node.env"
    echo "BOOTNODES=\"$BOOTNODES\"" >> "$DATA_DIR/node.env"
    log_success "Bootnodes updated in node.env"
else
    log_warning "node.env file not found at $DATA_DIR/node.env"
fi

# Replace placeholders in execution.toml
if [ -f "$DATA_DIR/config/execution.toml" ]; then
    log_info "Replacing placeholders in execution.toml..."
    sed -i "s|{{PEER_ID_$NODE_INDEX}}|$PEER_ID|g" "$DATA_DIR/config/execution.toml"
    sed -i "s|{{BOOTNODES}}|$BOOTNODES|g" "$DATA_DIR/config/execution.toml"
    
    # Replace placeholders for other nodes' peer IDs using stored values
    for j in $(seq 0 $((NODE_COUNT - 1))); do
        if [ $j -ne $NODE_INDEX ] && [ -n "${OTHER_PEER_IDS[$j]}" ]; then
            sed -i "s|{{PEER_ID_$j}}|${OTHER_PEER_IDS[$j]}|g" "$DATA_DIR/config/execution.toml"
        fi
    done
    log_success "Replaced placeholders in execution.toml"
    cat "$DATA_DIR/config/execution.toml"
else
    log_warning "execution.toml not found, skipping placeholder replacement"
fi

# Replace placeholders in node.yml
if [ -f "$DATA_DIR/config/node.yml" ]; then
    log_info "Replacing placeholders in node.yml..."
    sed -i "s|{{NODE_INDEX}}|$NODE_INDEX|g" "$DATA_DIR/config/node.yml"
    sed -i "s|{{HTTP_PORT}}|$HTTP_PORT|g" "$DATA_DIR/config/node.yml"
    sed -i "s|{{WS_PORT}}|$WS_PORT|g" "$DATA_DIR/config/node.yml"
    sed -i "s|{{JWT_SECRET}}|$JWT_SECRET|g" "$DATA_DIR/config/node.yml"
    sed -i "s|{{PEER_ID_$NODE_INDEX}}|$PEER_ID|g" "$DATA_DIR/config/node.yml"
    log_success "Replaced placeholders in config/node.yml"
    cat "$DATA_DIR/config/node.yml"
else
    log_warning "config/node.yml not found, skipping placeholder replacement"
fi

# Step 4: Initialize execution node with genesis
log_info "Step 4: Initializing execution node with genesis..."
if [ -f "$DATA_DIR/config/genesis.json" ] && [ -f "/usr/local/bin/fastevm-execution" ]; then
    # Check if already initialized (look for chaindata directory)
    if [ -d "$DATA_DIR/execution/chaindata" ] && [ "$(ls -A $DATA_DIR/execution/chaindata 2>/dev/null)" ]; then
        log_info "Execution node appears to be already initialized, skipping init"
    else
        log_info "Initializing execution node with genesis..."
        if /usr/local/bin/fastevm-execution init --datadir "$DATA_DIR/execution" --chain "$DATA_DIR/config/genesis.json" 2>&1; then
            log_success "Execution node initialized successfully"
            cat "$DATA_DIR/config/execution.toml"
        else
            # Check if it failed because it's already initialized
            if [ -d "$DATA_DIR/execution/chaindata" ]; then
                log_info "Execution node was already initialized, continuing..."
            else
                log_warning "Execution node initialization completed with warnings"
            fi
        fi
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

# Install systemd services
log_info "Installing systemd services..."

# Check if service.sh exists
if [ ! -f "/tmp/fastevm-config/service.sh" ]; then
    log_error "service.sh not found at /tmp/fastevm-config/service.sh"
    log_error "Service installation cannot proceed"
    exit 1
fi

# Install services and check for errors
if ! sudo bash /tmp/fastevm-config/service.sh install; then
    log_error "Failed to install systemd services"
    exit 1
fi

# Verify service files were created
if [ ! -f "/etc/systemd/system/fastevm-execution.service" ]; then
    log_error "Service file not created: /etc/systemd/system/fastevm-execution.service"
    exit 1
fi

if [ ! -f "/etc/systemd/system/fastevm-consensus.service" ]; then
    log_error "Service file not created: /etc/systemd/system/fastevm-consensus.service"
    exit 1
fi

# Verify services are enabled
if ! systemctl is-enabled fastevm-execution >/dev/null 2>&1; then
    log_error "fastevm-execution service is not enabled"
    exit 1
fi

if ! systemctl is-enabled fastevm-consensus >/dev/null 2>&1; then
    log_error "fastevm-consensus service is not enabled"
    exit 1
fi

log_success "Node $NODE_INDEX is ready for service startup!"
