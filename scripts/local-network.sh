#!/bin/bash

# FastEVM Local Network Startup Script
# This script starts the FastEVM network locally without Docker
# It manages 4 execution nodes and 4 consensus nodes

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DATA_DIR="$PROJECT_ROOT/.local-data"
LOGS_DIR="$PROJECT_ROOT/.local-logs"
PIDS_DIR="$PROJECT_ROOT/.local-pids"

# WebSocket configuration (can be overridden by command line)
ENABLE_WS=true

# Genesis configuration
GENESIS_FILE=$PROJECT_ROOT/execution-client/shared/genesis.json
GENESIS_OUTPUT_DIR="$DATA_DIR/genesis"
CLI=$PROJECT_ROOT/target/release/fastevm-cli
EXECUTION_CLIENT=$PROJECT_ROOT/target/release/fastevm-execution
# CONSENSUS_CLIENT=$PROJECT_ROOT/modules/mysticeti/target/release/evm-consensus
CONSENSUS_CLIENT=~/workspace/codelight/scalar-consensus/mysticeti/target/release/evm-consensus

GRAVITY_PIPE_BLOCK_GAS_LIMIT=10000000000
GRAVITY_CACHE_MAX_PERSIST_GAP=64
STAKE_AMOUNT=20000
# Default values for account generation
DEFAULT_ACCOUNT_NUMBER=100000
DEFAULT_ACCOUNT_NUMBER=100000
DEFAULT_ACCOUNT_AMOUNT="1000000000000000000000"  # 1000 ETH in wei
DEFAULT_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

# Command line options
ACCOUNT_COUNT="$DEFAULT_ACCOUNT_NUMBER"
ACCOUNT_AMOUNT="$DEFAULT_ACCOUNT_AMOUNT"
MNEMONIC="$DEFAULT_MNEMONIC"

# Execution node configuration (can be overridden by environment variables)
# With consensus producing ~10 subdags/second (min_round_delay: 100ms),
# we should process 1-2 subdags per block to match the rate
# Using 1 subdag per block with 100ms interval = 10 blocks/second = 10 subdags/second
COMMITTED_SUBDAGS_PER_BLOCK="${COMMITTED_SUBDAGS_PER_BLOCK:-30}"
BLOCK_INTERVAL_MS="${BLOCK_INTERVAL_MS:-100}"

# Port configuration
EXECUTION_PORTS=(8545 8544 8543 8542)  # HTTP RPC ports
EXECUTION_PORTS_WS=(8546 8548 8550 8552)  # WebSocket RPC ports
ENGINE_PORTS=(8551 8552 8553 8554)     # Engine API ports
P2P_PORTS=(30303 30304 30305 30306)    # P2P ports
CONSENSUS_PORTS=(26657 26658 26659 26660)  # Consensus ports

# Network configuration
NETWORK_SUBNET="172.20.0.0/16"
#NODE_IPS=("172.20.0.20" "172.20.0.21" "172.20.0.22" "172.20.0.23")
#CONSENSUS_IPS=("172.20.0.10" "172.20.0.11" "172.20.0.12" "172.20.0.13")
NODE_IPS=("127.0.0.1" "127.0.0.1" "127.0.0.1" "127.0.0.1")
CONSENSUS_IPS=("127.0.0.1" "127.0.0.1" "127.0.0.1" "127.0.0.1")

# Logging functions
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

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check if Rust is installed
    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo not found. Please install Rust: https://rustup.rs/"
        exit 1
    fi
    
    # Check if required tools are installed
    local missing_tools=()
    
    if ! command -v openssl &> /dev/null; then
        missing_tools+=("openssl")
    fi
    
    if ! command -v jq &> /dev/null; then
        missing_tools+=("jq")
    fi
    
    if ! command -v curl &> /dev/null; then
        missing_tools+=("curl")
    fi
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        log_info "Please install them using your package manager:"
        log_info "  macOS: brew install ${missing_tools[*]}"
        log_info "  Ubuntu/Debian: sudo apt install ${missing_tools[*]}"
        exit 1
    fi
    
    log_success "All prerequisites found!"
}

# Create necessary directories
setup_directories() {
    log_info "Setting up directories..."
    
    mkdir -p "$DATA_DIR"/{execution1,execution2,execution3,execution4}
    mkdir -p "$DATA_DIR"/{consensus1,consensus2,consensus3,consensus4}
    mkdir -p "$LOGS_DIR"
    mkdir -p "$PIDS_DIR"
    mkdir -p "$GENESIS_OUTPUT_DIR"
    
    log_success "Directories created!"
}


# Build the project
build_project() {
    log_info "Building FastEVM project..."
    
    cd "$PROJECT_ROOT"
    
    if cargo build --release; then
        log_success "Build completed successfully!"
    else
        log_error "Build failed!"
        exit 1
    fi
}

# Generate JWT secret
generate_jwt_secret() {
    local data_dir="$1"
    local jwt_file="$data_dir/jwt.hex"
    
    if [ ! -f "$jwt_file" ]; then
        openssl rand -hex 32 | tr -d '\n' > "$jwt_file"
        log_info "Generated JWT secret: $jwt_file"
    else
        log_info "JWT secret already exists: $jwt_file"
    fi
}

# Generate P2P secret key
generate_p2p_secret_key() {
    local node_index="$1"
    local data_dir="$2"
    local p2p_dir="$data_dir/p2p"
    
    mkdir -p "$p2p_dir"
    
    local secret_file="$p2p_dir/secret.key"
    local hex_file="$p2p_dir/secret.hex"
    
    if [ ! -f "$secret_file" ]; then
        # Generate a proper 32-byte (64 hex chars) secret key
        local seed="fastevm-node-${node_index}-p2p-secret-2025"
        echo "$seed" | openssl dgst -sha256 -hex | cut -d' ' -f2 | tr -d '\n' > "$secret_file"
        
        # Generate a deterministic peer ID from the secret key for enode URLs
        # We'll use the secret key directly as the peer ID for simplicity
        # This ensures we get a consistent 64-character hex string
        $CLI show-peer-id --file "$secret_file" --output "$hex_file"
        # Remove 0x prefix if present
        if [ -f "$hex_file" ]; then
            sed -i '' 's/^0x//' "$hex_file" 2>/dev/null || sed -i 's/^0x//' "$hex_file"
        fi
        
        log_info "Generated P2P secret key for node $node_index"
    else
        log_info "P2P secret key already exists for node $node_index"
    fi
}

# Initialize execution node data
init_execution_node() {
    local node_index="$1"
    local data_dir="$DATA_DIR/execution$node_index"
    local http_port="${EXECUTION_PORTS[$((node_index-1))]}"
    local engine_port="${ENGINE_PORTS[$((node_index-1))]}"
    local p2p_port="${P2P_PORTS[$((node_index-1))]}"
    local node_ip="${NODE_IPS[$((node_index-1))]}"
    
    log_info "Initializing execution node $node_index..."
    
    # Generate JWT secret
    generate_jwt_secret "$data_dir"
    
    # Generate P2P secret key
    generate_p2p_secret_key "$node_index" "$data_dir"
    
    cp $SCRIPT_DIR/genesis.json $data_dir/genesis.json

    # Copy prefunded genesis.json if it exists, otherwise fall back to original
    # local prefunded_genesis="$GENESIS_OUTPUT_DIR/genesis.json"
    # if [ -f "$prefunded_genesis" ]; then
    #     cp "$prefunded_genesis" "$data_dir/genesis.json"
    #     log_info "Copied prefunded genesis.json to $data_dir"
    # elif [ -f "$GENESIS_FILE" ]; then
    #     cp "$GENESIS_FILE" "$data_dir/genesis.json"
    #     log_info "Copied original genesis.json to $data_dir"
    # else
    #     log_warning "No genesis file found, using default chain"
    # fi
    
    # Initialize the node if genesis exists
    # if [ -f "$data_dir/genesis.json" ]; then
    #     log_info "Initializing node with genesis..."
    #     "$PROJECT_ROOT/target/release/fastevm-execution" init --datadir "$data_dir" --chain "$data_dir/genesis.json" || true
    # fi
}

# Step 1: Generate validators.yml configuration file (if needed)
generate_validators_config() {
    local validators_output="$DATA_DIR/validators.yml"
    
    # Check if validators.yml already exists
    if [ -f "$validators_output" ]; then
        log_info "validators.yml already exists at: $validators_output"
        return 0
    fi
    
    log_info "Step 1: Generating validators.yml configuration file..."
    
    # Number of authorities
    local authorities_count=${#CONSENSUS_IPS[@]}
    
    # Build IP addresses string from CONSENSUS_IPS array
    local ip_addresses_str=""
    for i in "${!CONSENSUS_IPS[@]}"; do
        if [ -n "$ip_addresses_str" ]; then
            ip_addresses_str="${ip_addresses_str},${CONSENSUS_IPS[$i]}"
        else
            ip_addresses_str="${CONSENSUS_IPS[$i]}"
        fi
    done
    
    # Build network ports string from CONSENSUS_PORTS array
    local network_ports_str=""
    for i in "${!CONSENSUS_PORTS[@]}"; do
        if [ -n "$network_ports_str" ]; then
            network_ports_str="${network_ports_str},${CONSENSUS_PORTS[$i]}"
        else
            network_ports_str="${CONSENSUS_PORTS[$i]}"
        fi
    done
    
    log_info "Generating validators.yml with:"
    log_info "  Authorities: $authorities_count"
    log_info "  IP addresses: $ip_addresses_str"
    log_info "  Network ports: $network_ports_str"

    # Step 1: Generate validators.yml using consensus client generate-validators command
    if command -v "$CONSENSUS_CLIENT" >/dev/null 2>&1; then
        if "$CONSENSUS_CLIENT" generate-validators \
            --output "$validators_output" \
            --authorities "$authorities_count" \
            --epoch "0" \
            --stake "$STAKE_AMOUNT" \
            --ip-addresses "$ip_addresses_str" \
            --network-ports "$network_ports_str" \
            --hostname-prefix "fastevm-consensus"; then
            log_success "Step 1 completed: Generated validators.yml at: $validators_output"
            return 0
        else
            log_error "Failed to generate validators.yml"
            return 1
        fi
    else
        log_error "Consensus client not found: $CONSENSUS_CLIENT"
        log_info "Please build the project first with: make build"
        return 1
    fi
}

# Step 2: Generate genesis_config.json configuration file from validators.yml
generate_genesis_config() {
    local validators_file="$DATA_DIR/validators.yml"
    local genesis_output="$DATA_DIR/genesis_config.json"
    
    # Check if validators.yml exists
    if [ ! -f "$validators_file" ]; then
        log_error "validators.yml not found at: $validators_file"
        log_error "Please generate validators.yml first (Step 1)"
        return 1
    fi
    
    # Check if genesis_config.json already exists
    if [ -f "$genesis_output" ]; then
        log_info "genesis_config.json already exists at: $genesis_output"
        return 0
    fi
    
    log_info "Step 2: Generating genesis_config.json configuration file from validators.yml..."
    log_info "  Using validators from: $validators_file"
    log_info "  Output: $genesis_output"
    
    # Generate genesis_config.json using generate-genesis-config command with --config-path pointing to validators.yml
    if command -v "$CONSENSUS_CLIENT" >/dev/null 2>&1; then
        if "$CONSENSUS_CLIENT" generate-genesis-config \
            --config-path "$validators_file" \
            --genesis-path "$genesis_output"; then
            log_success "Step 2 completed: Generated genesis_config.json at: $genesis_output"
            return 0
        else
            log_error "Failed to generate genesis_config.json from validators.yml"
            return 1
        fi
    else
        log_error "Consensus client not found: $CONSENSUS_CLIENT"
        log_info "Please build the project first with: make build"
        return 1
    fi
}
collect_validators_into_committees() {

    EPOCH="${EPOCH:-0}"
    QUORUM_THRESHOLD="${QUORUM_THRESHOLD:-2667}"
    VALIDITY_THRESHOLD="${VALIDITY_THRESHOLD:-1334}"
    NODE_COUNT="${NODE_COUNT:-4}"
    # ==========================
    # Build committees.yml
    # ==========================
    echo "🔗 Building committees.yml from local authority.yml files"

    TMP_COMMITTEES="$(mktemp)"

    cleanup() {
        rm -f "$TMP_COMMITTEES"
    }
    trap cleanup EXIT

    echo "epoch: $EPOCH" > "$TMP_COMMITTEES"
    echo "authorities:" >> "$TMP_COMMITTEES"

    for ((INDEX=1; INDEX<=NODE_COUNT; INDEX++)); do
        AUTH_FILE="$DATA_DIR/consensus${INDEX}/authority.yml"

        if [[ ! -f "$AUTH_FILE" ]]; then
            echo "❌ Missing authority file: $AUTH_FILE" >&2
            exit 1
        fi

        echo "  - index: $INDEX" >> "$TMP_COMMITTEES"

        # Indent authority.yml and strip index/epoch if present
        sed 's/^/    /' "$AUTH_FILE" \
            | grep -Ev '^(    index:|    epoch:)' \
            >> "$TMP_COMMITTEES"
    done

    {
        echo
        echo "quorum_threshold: $QUORUM_THRESHOLD"
        echo "validity_threshold: $VALIDITY_THRESHOLD"
    } >> "$TMP_COMMITTEES"

    # ==========================
    # Show result
    # ==========================
    echo "📄 Generated committees.yml"
    echo "--------------------------------"
    cat "$TMP_COMMITTEES"
    echo "--------------------------------"

    # ==========================
    # Distribute to nodes
    # ==========================
    log_info "Copying committees.yml to all consensus directories"

    for ((INDEX=1; INDEX<=NODE_COUNT; INDEX++)); do
        DEST="$DATA_DIR/consensus${INDEX}/committees.yml"
        cp "$TMP_COMMITTEES" "$DEST"
    done

    log_success "Committees.yml updated successfully"
}

# Generate genesis.json by collecting authority.yml files and calling gravity-genesis-contract
generate_genesis() {
    log_info "🔗 Collecting authority.yml files and building genesis_config.json"
    
    # Location of gravity-genesis-contract project
    GRAVITY_GENESIS_CONTRACT_DIR="${GRAVITY_GENESIS_CONTRACT_DIR:-${SCRIPT_DIR}/../../gravity-genesis-contract}"
    
    # Check if gravity-genesis-contract directory exists
    if [ ! -d "$GRAVITY_GENESIS_CONTRACT_DIR" ]; then
        log_error "Gravity genesis contract directory not found: $GRAVITY_GENESIS_CONTRACT_DIR"
        log_error "Please set GRAVITY_GENESIS_CONTRACT_DIR environment variable or update the default path"
        return 1
    fi
    
    # Path for genesis_config.json
    GENESIS_CONFIG_JSON="$DATA_DIR/genesis_config.json"
    
    # Initialize genesis_config.json with empty arrays
    jq -n '{validatorAddresses: [], consensusPublicKeys: [], votingPowers: [], validatorNetworkAddresses: [], fullnodeNetworkAddresses: [], aptosAddresses: []}' > "$GENESIS_CONFIG_JSON"
    
    # Collect authority.yml files from all consensus nodes
    for ((INDEX=1; INDEX<=NODE_COUNT; INDEX++)); do
        AUTH_FILE="$DATA_DIR/consensus${INDEX}/authority.yml"
        
        if [[ ! -f "$AUTH_FILE" ]]; then
            log_error "Missing authority file: $AUTH_FILE"
            return 1
        fi
        
        log_info "Processing authority file: $AUTH_FILE"
        
        # Extract values from authority.yml
        VALIDATOR_ADDR=$(sed -n 's/^validator_address:[[:space:]]*//p' "$AUTH_FILE")
        AUTHORITY_KEY=$(sed -n 's/^authority_key:[[:space:]]*//p' "$AUTH_FILE")
        STAKE=$(sed -n 's/^stake:[[:space:]]*//p' "$AUTH_FILE")
        BASE_ADDR=$(sed -n 's/^address:[[:space:]]*//p' "$AUTH_FILE")
        
        # Process the values
        CONS_PUBKEY=$(echo "$AUTHORITY_KEY" | cut -c1-96)
        APTOS_ADDR=$(echo "$VALIDATOR_ADDR" | tr '[:upper:]' '[:lower:]')
        APTOS_ADDR_LAST40=$(echo "$APTOS_ADDR" | rev | cut -c1-40 | rev)
        NET_ADDR="$BASE_ADDR/noise-ik/$APTOS_ADDR/handshake/0"
        
        # Add to genesis_config.json
        jq --arg vaddr "0x$APTOS_ADDR_LAST40" \
           --arg cpk "$CONS_PUBKEY" \
           --arg stake "$STAKE" \
           --arg net "$NET_ADDR" \
           --arg apt "$APTOS_ADDR" \
           '.validatorAddresses += [$vaddr] | .consensusPublicKeys += [$cpk] | .votingPowers += [$stake] | .validatorNetworkAddresses += [$net] | .fullnodeNetworkAddresses += [$net] | .aptosAddresses += [$apt]' \
           "$GENESIS_CONFIG_JSON" > "$GENESIS_CONFIG_JSON.tmp" && mv "$GENESIS_CONFIG_JSON.tmp" "$GENESIS_CONFIG_JSON"
    done
    
    log_success "📄 Generated genesis_config.json:"
    jq . "$GENESIS_CONFIG_JSON"
    
    # Create genesis.json using gravity-genesis-contract
    log_info "🔗 Creating genesis.json using gravity-genesis-contract"
    
    # Ensure generate directory exists in gravity-genesis-contract
    mkdir -p "$GRAVITY_GENESIS_CONTRACT_DIR/generate"
    
    # Copy genesis_config.json to gravity-genesis-contract/generate/
    cp "$GENESIS_CONFIG_JSON" "$GRAVITY_GENESIS_CONTRACT_DIR/generate/genesis_config.json"
    log_info "Copied genesis_config.json to $GRAVITY_GENESIS_CONTRACT_DIR/generate/"
    
    # Save current directory and change to gravity-genesis-contract directory
    local original_dir=$(pwd)
    cd "$GRAVITY_GENESIS_CONTRACT_DIR"
    if bash ./generate_genesis.sh; then
        log_success "✅ genesis.json created in gravity-genesis-contract"
    else
        log_error "Failed to generate genesis.json"
        cd "$original_dir"
        return 1
    fi
    # Restore original directory
    cd "$original_dir"
    
    # Copy genesis.json to all consensus nodes
    GENESIS_JSON_PATH="$GRAVITY_GENESIS_CONTRACT_DIR/genesis.json"
    if [ ! -f "$GENESIS_JSON_PATH" ]; then
        log_error "Generated genesis.json not found at: $GENESIS_JSON_PATH"
        return 1
    fi
    
    log_info "Copying genesis.json to all consensus nodes..."
    for ((INDEX=1; INDEX<=NODE_COUNT; INDEX++)); do
        DEST="$DATA_DIR/consensus${INDEX}/genesis.json"
        cp "$GENESIS_JSON_PATH" "$DEST"
        log_info "Copied genesis.json to consensus node $INDEX"
    done
    
    log_success "✅ genesis.json is copied to all consensus nodes"
}
# Step 3: Generate committees.yml configuration file from validators.yml (shared across all consensus nodes)
generate_committees_config() {
    local validators_file="$DATA_DIR/validators.yml"
    local committees_output="$DATA_DIR/committees.yml"
    
    # Step 1: Check if validators.yml exists, if not generate it first
    if [ ! -f "$validators_file" ]; then
        log_info "validators.yml not found, generating it first (Step 1)..."
        if ! generate_validators_config; then
            log_error "Failed to generate validators.yml"
            return 1
        fi
    else
        log_info "validators.yml found at: $validators_file"
    fi
    
    # Step 2: Generate genesis_config.json from validators.yml
    if ! generate_genesis_config; then
        log_error "Failed to generate genesis_config.json"
        return 1
    fi
    
    log_info "Step 3: Generating committees.yml configuration file from validators.yml..."
    log_info "  Using validators from: $validators_file"
    
    # Generate committees.yml using generate-committee command with --config-path pointing to validators.yml
    if command -v "$CONSENSUS_CLIENT" >/dev/null 2>&1; then
        # Step 3: Use generate-committee with --config-path to read from validators.yml
        if "$CONSENSUS_CLIENT" generate-committee \
            --config-path "$validators_file" \
            --committee-path "$committees_output" \
            --epoch "0"; then
            log_success "Step 3 completed: Generated committees.yml at: $committees_output"
            return 0
        else
            log_error "Failed to generate committees.yml from validators.yml"
            return 1
        fi
    else
        log_error "Consensus client not found: $CONSENSUS_CLIENT"
        log_info "Please build the project first with: make build"
        return 1
    fi
}

# Generate consensus node configuration files
generate_consensus_files() {
    local node_index="$1"
    local data_dir="$DATA_DIR/consensus$node_index"
    
    log_info "Generating consensus node $node_index configuration files..."
    
    # Create data directory if it doesn't exist
    mkdir -p "$data_dir"
    
    # Copy prefunded genesis.json if it exists, otherwise fall back to original
    local prefunded_genesis="$GENESIS_OUTPUT_DIR/genesis.json"
    local shared_genesis="$PROJECT_ROOT/execution-client/shared/genesis.json"
    
    if [ -f "$prefunded_genesis" ]; then
        if cp "$prefunded_genesis" "$data_dir/genesis.json"; then
            log_info "Copied prefunded genesis.json to consensus node $node_index"
        else
            log_error "Failed to copy prefunded genesis.json to consensus node $node_index"
            return 1
        fi
    elif [ -f "$shared_genesis" ]; then
        if cp "$shared_genesis" "$data_dir/genesis.json"; then
            log_info "Copied original genesis.json to consensus node $node_index"
        else
            log_error "Failed to copy genesis.json to consensus node $node_index"
            return 1
        fi
    else
        log_error "No genesis file found: $shared_genesis"
        return 1
    fi
    
    # Copy parameters.yml from examples
    local parameters_template="$PROJECT_ROOT/consensus-client/examples/parameters.yml"
    if [ -f "$parameters_template" ]; then
        if cp "$parameters_template" "$data_dir/parameters.yml"; then
            log_info "Copied parameters.yml to consensus node $node_index"
        else
            log_error "Failed to copy parameters.yml to consensus node $node_index"
            return 1
        fi
    else
        log_error "Parameters template not found: $parameters_template"
        return 1
    fi
    # Copy validator.yml
    "$CONSENSUS_CLIENT" generate-validator \
        --validator-path $data_dir/validator.yml \
        --authority-path $data_dir/authority.yml \
        --stake $STAKE_AMOUNT \
        --hostname "fastevm-consensus$node_index" \
        --ip-address "${CONSENSUS_IPS[$((node_index-1))]}" \
        --port "${CONSENSUS_PORTS[$((node_index-1))]}"

    log_success "Generated consensus node $node_index files: genesis.json, committees.yml, parameters.yml"
}

# Generate consensus node configuration from template
generate_consensus_node_config() {
    local node_index="$1"
    local data_dir="$DATA_DIR/consensus$node_index"
    local jwt_secret="0x$(cat "$DATA_DIR/execution$node_index/jwt.hex")"
    local genesis_block_hash="0x0000000000000000000000000000000000000000000000000000000000000000"
    
    # Get the correct ports for this node
    local http_port="${EXECUTION_PORTS[$((node_index-1))]}"
    local ws_port="${EXECUTION_PORTS_WS[$((node_index-1))]}"
    
    log_info "Generating consensus node $node_index configuration from template..."
    log_info "Using execution HTTP port: $http_port, WS port: $ws_port"
    
    # Create data directory if it doesn't exist
    mkdir -p "$data_dir"
    
    # Template file path
    local template_file="$PROJECT_ROOT/consensus-client/examples/node.local.yml"
    local config_file="$data_dir/node.yml"
    
    if [ -f "$template_file" ]; then
        # Copy template to data directory
        cp "$template_file" "$config_file"
        
        # Replace placeholders in the config file
        sed -i '' "s/{NODE_INDEX}/$node_index/g" "$config_file" 2>/dev/null || sed -i "s/{NODE_INDEX}/$node_index/g" "$config_file"
        sed -i '' "s/{AUTHORITY_INDEX}/$((node_index-1))/g" "$config_file" 2>/dev/null || sed -i "s/{AUTHORITY_INDEX}/$((node_index-1))/g" "$config_file"
        sed -i '' "s/{JWT_SECRET}/$jwt_secret/g" "$config_file" 2>/dev/null || sed -i "s/{JWT_SECRET}/$jwt_secret/g" "$config_file"
        sed -i '' "s/{GENESIS_BLOCK_HASH}/$genesis_block_hash/g" "$config_file" 2>/dev/null || sed -i "s/{GENESIS_BLOCK_HASH}/$genesis_block_hash/g" "$config_file"
        
        # Update execution URLs to use localhost with correct ports
        sed -i '' "s|http://execution$node_index:8545|http://127.0.0.1:$http_port|g" "$config_file" 2>/dev/null
        sed -i '' "s|ws://execution$node_index:8546|ws://127.0.0.1:$ws_port|g" "$config_file" 2>/dev/null
        
        log_success "Generated consensus node config: $config_file"
        log_info "Execution HTTP URL: http://127.0.0.1:$http_port"
        log_info "Execution WS URL: ws://127.0.0.1:$ws_port"
    else
        log_error "Consensus template not found: $template_file"
        return 1
    fi
}

init_consensus_node() {
    local node_index="$1"
    local data_dir="$DATA_DIR/consensus$node_index"
    local consensus_port="${CONSENSUS_PORTS[$((node_index-1))]}"
    local consensus_ip="${CONSENSUS_IPS[$((node_index-1))]}"
    
    log_info "Initializing consensus node $node_index..."
    
    # Generate required files first
    generate_consensus_files "$node_index"
    
    # Generate node configuration from template
    generate_consensus_node_config "$node_index"
}

# Start execution node
start_execution_node() {
    local node_index="$1"
    local data_dir="$DATA_DIR/execution$node_index"
    # local http_port="${EXECUTION_PORTS[$((node_index-1))]}"
    #local engine_port="${ENGINE_PORTS[$((node_index-1))]}"
    local http_port=8545
    local ws_port=8546
    local engine_port=8551
    local p2p_port="${P2P_PORTS[$((node_index-1))]}"
    local node_ip="${NODE_IPS[$((node_index-1))]}"
    local log_file="$LOGS_DIR/execution-node$node_index.log"
    local pid_file="$PIDS_DIR/execution-node$node_index.pid"
    
    # Build port info string
    local port_info="http:$http_port, engine:$engine_port, p2p:$p2p_port"
    # if [ "$ENABLE_WS" = true ]; then
    #     port_info="http:$http_port, ws:$((http_port+1)), engine:$engine_port, p2p:$p2p_port"
    # fi
    log_info "Starting execution node $node_index on port $port_info..."
    
    # Build bootnodes string
    local bootnodes=""
    local debug_level="-vvv"
    if [ "$node_index" = "1" ]; then
        debug_level="-vvvv"
    fi
    local debug_level="-vvv"
    if [ "$node_index" = "1" ]; then
        debug_level="-vvvv"
    fi
    for i in {1..4}; do
        if [ $i -ne $node_index ]; then
            local peer_hex_file="$DATA_DIR/execution$i/p2p/secret.hex"
            if [ -f "$peer_hex_file" ]; then
                local peer_id=$(cat "$peer_hex_file")
                if [ -n "$peer_id" ]; then
                    local enode="enode://${peer_id}@127.0.0.1:${P2P_PORTS[$((i-1))]}"
                    if [ -n "$bootnodes" ]; then
                        bootnodes="${bootnodes},${enode}"
                    else
                        bootnodes="$enode"
                    fi
                fi
            fi
        fi
    done
    
    # Build command arguments
    local cmd_args=(
        "node"
        "--chain" "$data_dir/genesis.json"
        "--datadir" "$data_dir"
        "--instance" "$node_index"
        "--engine.always-process-payload-attributes-on-canonical-head"
        "--http"
        "--http.api" "eth,net,web3,admin,debug,txpool"
        "--http.addr" "0.0.0.0"
        "--http.port" "$http_port"
        "--http.corsdomain" "*"
    )
    
    # Add WebSocket arguments if enabled
    if [ "$ENABLE_WS" = true ]; then
        cmd_args+=(
            "--ws"
            "--ws.api" "eth,net,web3,admin,debug"
            "--ws.addr" "0.0.0.0"
            "--ws.port" "$((http_port+1))"
            "--ws.origins" "*"
        )
    fi

    # Add --txpool.max-account-slots
    cmd_args+=(
        "--txpool.max-new-txns" "102400"
        "--txpool.max-account-slots" "102400"
        "--txpool.max-pending-txns" "102400"
        "--txpool.pending-max-count" "102400"
        "--txpool.pending-max-size" "128"       
        "--txpool.max-new-pending-txs-notifications" "102400"
        "--txpool.queued-max-count" "102400"
        "--txpool.queued-max-size" "128"
    )
    # Builder
    cmd_args+=(
        "--builder.gaslimit" "240000000"
    )
    # gravity parameters
    cmd_args+=(
        "--gravity.pipe-block-gas-limit" "$GRAVITY_PIPE_BLOCK_GAS_LIMIT"
        "--gravity.cache.max-persist-gap" "$GRAVITY_CACHE_MAX_PERSIST_GAP"
    )
    
    # Add remaining arguments
    cmd_args+=(
        "--authrpc.addr" "0.0.0.0"
        "--authrpc.port" "$engine_port"
        "--authrpc.jwtsecret" "$data_dir/jwt.hex"
        "--addr" "0.0.0.0"
        "--port" "$p2p_port"
        "--discovery.addr" "0.0.0.0"
        "--discovery.port" "$p2p_port"
        "--p2p-secret-key" "$data_dir/p2p/secret.key"
        "--bootnodes" "$bootnodes"
        "--committed-subdags-per-block" "$COMMITTED_SUBDAGS_PER_BLOCK"
        "--block-interval-ms" "$BLOCK_INTERVAL_MS"
        "$debug_level"
    )
    
    # Start the node
    nohup "$EXECUTION_CLIENT" "${cmd_args[@]}" > "$log_file" 2>&1 &


    local pid=$!
    echo $pid > "$pid_file"
    
    log_info "Execution node $node_index started (PID: $pid, Log: $log_file)"
}

# Start consensus node
start_consensus_node() {
    local node_index="$1"
    local data_dir="$DATA_DIR/consensus$node_index"
    local consensus_port="${CONSENSUS_PORTS[$((node_index-1))]}"
    local consensus_ip="${CONSENSUS_IPS[$((node_index-1))]}"
    local log_file="$LOGS_DIR/consensus-node$node_index.log"
    local pid_file="$PIDS_DIR/consensus-node$node_index.pid"
    
    log_info "Starting consensus node $node_index..."
    
    # Set environment variables
    export RUST_LOG=debug
    export NODE_INDEX=$((node_index-1))
    export NODE_IP="$consensus_ip"
    
    # Start the node
    cd $data_dir && nohup "$CONSENSUS_CLIENT" \
        start \
        --config node.yml \
        > "$log_file" 2>&1 &
    
    local pid=$!
    echo $pid > "$pid_file"
    
    log_info "Consensus node $node_index started (PID: $pid, Log: $log_file)"
}

# Wait for service to be ready
wait_for_service() {
    local service_name="$1"
    local port="$2"
    local max_attempts=30
    local attempt=1
    
    log_info "Waiting for $service_name to be ready on port $port..."
    
    while [ $attempt -le $max_attempts ]; do
        if curl -s "http://localhost:$port" > /dev/null 2>&1; then
            log_success "$service_name is ready!"
            return 0
        fi
        
        log_info "Attempt $attempt/$max_attempts - waiting for $service_name..."
        sleep 2
        attempt=$((attempt + 1))
    done
    
    log_error "$service_name failed to start after $max_attempts attempts"
    return 1
}

# Start the network
start_network() {
    log_info "Starting FastEVM local network..."
    
    # Clean up any existing processes and ports before starting
    log_info "Cleaning up any existing processes..."
    stop_network
    
    # Generate committees.yml configuration (shared across all consensus nodes)
    # generate_committees_config
    
    # Initialize all nodes
    for i in {1..4}; do
        init_execution_node "$i"
        init_consensus_node "$i"
    done
    # Collect validator.yml into committees.yml
    collect_validators_into_committees
    generate_genesis
    # Start execution nodes
    for i in {1..4}; do
        log_info "Starting execution node $i ..."
        start_execution_node "$i"
    done
    
    # Wait for execution nodes to be ready
    for i in {1..4}; do
        local http_port="${EXECUTION_PORTS[$((i-1))]}"
        wait_for_service "execution-node$i" "$http_port" || {
            log_error "Execution node $i failed to start"
            show_logs "execution-node$i"
            exit 1
        }
    done
    # local sleep_time=30
    # echo " Sleep $sleep_time seconds for all execution nodes to be ready"
    # sleep $sleep_time
    # Start consensus nodes
    for i in {1..4}; do
        start_consensus_node "$i"
    done
    
    # Wait a bit for consensus nodes to start
    sleep 5
    
    log_success "FastEVM local network started successfully!"
}

# Stop the network
stop_network() {
    log_info "Stopping FastEVM local network..."
    
    # Stop all processes by PID files first
    for pid_file in "$PIDS_DIR"/*.pid; do
        if [ -f "$pid_file" ]; then
            local pid=$(cat "$pid_file")
            local service_name=$(basename "$pid_file" .pid)
            
            if kill "$pid" 2>/dev/null; then
                log_info "Stopped $service_name (PID: $pid)"
            else
                log_warning "Failed to stop $service_name (PID: $pid)"
            fi
        fi
    done
    
    # Clean up PID files
    rm -f "$PIDS_DIR"/*.pid
    
    # Kill any fastevm-execution processes
    if pgrep -f "fastevm-execution" > /dev/null 2>&1; then
        log_info "Killing remaining fastevm-execution processes..."
        pkill -9 -f "fastevm-execution" 2>/dev/null || true
    fi
    
    # Kill any evm-consensus processes
    if pgrep -f "evm-consensus" > /dev/null 2>&1; then
        log_info "Killing remaining evm-consensus processes..."
        pkill -9 -f "evm-consensus" 2>/dev/null || true
    fi
    
    # Kill processes on all ports used by the network
    # Consensus ports, HTTP RPC ports, Engine API ports, P2P ports, WebSocket ports
    local all_ports=(
        "${CONSENSUS_PORTS[@]}"      # 26657, 26658, 26659, 26660
        "${EXECUTION_PORTS[@]}"      # 8545, 8544, 8543, 8542
        "${ENGINE_PORTS[@]}"         # 8551, 8552, 8553, 8554
        "${P2P_PORTS[@]}"            # 30303, 30304, 30305, 30306
        "${EXECUTION_PORTS_WS[@]}"   # 8546, 8548, 8550, 8552
    )
    
    for port in "${all_ports[@]}"; do
        local pid=$(lsof -ti :$port 2>/dev/null)
        if [ -n "$pid" ]; then
            log_info "Killing process $pid on port $port"
            kill -9 "$pid" 2>/dev/null || true
        fi
    done
    
    # Wait a moment for processes to fully terminate
    sleep 1

    log_success "Network stopped!"
}

# Show network status
show_status() {
    log_info "FastEVM Local Network Status:"
    echo
    
    # Check execution nodes
    echo "Execution Nodes:"
    for i in {1..4}; do
        local http_port="${EXECUTION_PORTS[$((i-1))]}"
        local engine_port="${ENGINE_PORTS[$((i-1))]}"
        local pid_file="$PIDS_DIR/execution-node$i.pid"
        
        if [ -f "$pid_file" ] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
            local status_info="http://localhost:$http_port (RPC), http://localhost:$engine_port (Engine API)"
            if [ "$ENABLE_WS" = true ]; then
                local ws_port=$((http_port+1))
                status_info="http://localhost:$http_port (RPC), ws://localhost:$ws_port (WS), http://localhost:$engine_port (Engine API)"
            fi
            echo "  ✅ Node $i: $status_info"
        else
            echo "  ❌ Node $i: Not running"
        fi
    done
    
    echo
    echo "Consensus Nodes:"
    for i in {1..4}; do
        local consensus_port="${CONSENSUS_PORTS[$((i-1))]}"
        local consensus_ip="${CONSENSUS_IPS[$((i-1))]}"
        local pid_file="$PIDS_DIR/consensus-node$i.pid"
        
        if [ -f "$pid_file" ] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
            echo "  ✅ Node $i: $consensus_ip:$consensus_port"
        else
            echo "  ❌ Node $i: Not running"
        fi
    done
    
    echo
    echo "Logs directory: $LOGS_DIR"
    echo "Data directory: $DATA_DIR"
}

# Show logs
show_logs() {
    local service="$1"
    
    if [ -z "$service" ]; then
        log_info "Available log files:"
        ls -la "$LOGS_DIR"/*.log 2>/dev/null || log_warning "No log files found"
        return
    fi
    
    local log_file="$LOGS_DIR/${service}.log"
    if [ -f "$log_file" ]; then
        log_info "Showing logs for $service (Ctrl+C to exit):"
        tail -f "$log_file"
    else
        log_error "Log file not found: $log_file"
    fi
}

# Clean up
cleanup() {
    log_info "Cleaning up local network data..."
    
    stop_network
    
    # Remove data and logs
    if [ -d "$DATA_DIR" ]; then
        rm -rf "$DATA_DIR"
        log_info "Removed data directory: $DATA_DIR"
    fi
    
    if [ -d "$LOGS_DIR" ]; then
        rm -rf "$LOGS_DIR"
        log_info "Removed logs directory: $LOGS_DIR"
    fi
    
    if [ -d "$PIDS_DIR" ]; then
        rm -rf "$PIDS_DIR"
        log_info "Removed PIDs directory: $PIDS_DIR"
    fi
    
    log_success "Cleanup complete!"
}

# Parse command line arguments
parse_arguments() {
    COMMAND="start"  # Default command
    
    # First, check if first argument is a command
    if [[ $# -gt 0 ]] && [[ "$1" =~ ^(start|stop|restart|status|logs|cleanup|init|prefund|regenerate-consensus|regenerate-genesis)$ ]]; then
        COMMAND="$1"
        shift  # Remove the command from arguments
    fi
    
    # Then parse remaining options
    while [[ $# -gt 0 ]]; do
        case $1 in
            --no-ws)
                ENABLE_WS=false
                shift
                ;;
            --ws)
                ENABLE_WS=true
                shift
                ;;
            --accounts)
                ACCOUNT_COUNT="$2"
                shift 2
                ;;
            --amount)
                ACCOUNT_AMOUNT="$2"
                shift 2
                ;;
            --mnemonic)
                MNEMONIC="$2"
                shift 2
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            *)
                # Unknown option
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
}

# Show help
show_help() {
    echo "Usage: $0 {start|stop|restart|status|logs|cleanup|init|regenerate-consensus|regenerate-genesis} [OPTIONS]"
    echo
    echo "Commands:"
    echo "  start        - Start the local network (default)"
    echo "  stop         - Stop the local network"
    echo "  restart      - Restart the local network"
    echo "  status       - Show network status"
    echo "  logs [service] - Show logs (all services or specific service)"
    echo "  cleanup      - Clean up all data and stop network"
    echo "  init         - Initialize node data without starting"
    echo "  regenerate-consensus - Regenerate consensus node configuration files"
    echo "  regenerate-genesis - Regenerate prefunded genesis.json with new accounts"
    echo
    echo "Options:"
    echo "  --ws         - Enable WebSocket support (default)"
    echo "  --no-ws      - Disable WebSocket support"
    echo "  --accounts N - Number of accounts to generate (default: $DEFAULT_ACCOUNT_NUMBER)"
    echo "  --amount X   - Amount in wei to fund each account (default: $DEFAULT_ACCOUNT_AMOUNT)"
    echo "  --mnemonic \"...\" - Mnemonic phrase for account generation (default: test mnemonic)"
    echo "  --help, -h   - Show this help message"
    echo
    echo "Examples:"
    echo "  $0 start                                    # Start with default settings"
    echo "  $0 start --accounts 50 --amount 500000000000000000000  # 50 accounts with 500 ETH each"
    echo "  $0 regenerate-genesis --accounts 200       # Generate 200 prefunded accounts"
    echo "  $0 start --no-ws                           # Start without WebSocket"
    echo "  $0 logs execution-node1                    # Show logs for execution node 1"
    echo "  $0 status                                  # Show network status"
}

# Parse arguments first
parse_arguments "$@"

# Main script logic
case "$COMMAND" in
    "start")
        check_prerequisites
        setup_directories
        build_project
        start_network
        show_status
        ;;
    "stop")
        stop_network
        ;;
    "restart")
        stop_network
        sleep 2
        start_network
        show_status
        ;;
    "status")
        show_status
        ;;
    "logs")
        show_logs "$2"
        ;;
    "cleanup")
        cleanup
        ;;
    "init")
        check_prerequisites
        setup_directories
        build_project
        # Generate committees.yml configuration (shared across all consensus nodes)
        generate_committees_config
        for i in {1..4}; do
            init_execution_node "$i"
            init_consensus_node "$i"
        done
        log_success "Initialization complete!"
        ;;
    "regenerate-consensus")
        log_info "Regenerating consensus node configuration files..."
        
        # Generate committees.yml configuration first (shared across all consensus nodes)
        generate_committees_config
        
        # Regenerate files for all consensus nodes
        for i in {1..4}; do
            generate_consensus_files "$i"
            generate_consensus_node_config "$i"
        done
        
        log_success "Consensus configuration files regenerated for all nodes!"
        ;;
    "prefund")
        prefund_genesis
        ;;
    "regenerate-genesis")
        log_info "Regenerating prefunded genesis.json..."
        
        check_prerequisites
        setup_directories
        build_project
        prefund_genesis
        
        log_success "Prefunded genesis.json regenerated!"
        log_info "Genesis file: $GENESIS_OUTPUT_DIR/genesis.json"
        log_info "Account count: $ACCOUNT_COUNT"
        log_info "Amount per account: $ACCOUNT_AMOUNT wei"
        ;;
    *)
        show_help
        exit 1
        ;;
esac
