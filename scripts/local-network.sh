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

# Port configuration
EXECUTION_PORTS=(8545 8547 8549 8555)  # HTTP RPC ports
ENGINE_PORTS=(8551 8552 8553 8554)     # Engine API ports
P2P_PORTS=(30303 30304 30305 30306)    # P2P ports
CONSENSUS_PORTS=(26657 26658 26659 26660)  # Consensus ports

# Network configuration
NETWORK_SUBNET="172.20.0.0/16"
NODE_IPS=("172.20.0.20" "172.20.0.21" "172.20.0.22" "172.20.0.23")
CONSENSUS_IPS=("172.20.0.10" "172.20.0.11" "172.20.0.12" "172.20.0.13")

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
        # Generate deterministic secret based on node index
        local seed="fastevm-node-${node_index}-p2p-secret-2025"
        echo "$seed" | openssl dgst -sha256 -binary | openssl dgst -sha256 -hex | cut -d' ' -f2 | tr -d '\n' > "$secret_file"
        
        # Generate hex version
        if command -v reth &> /dev/null; then
            reth show-peer-id --file "$secret_file" --output "$hex_file" 2>/dev/null || true
        else
            # Fallback: use openssl to generate hex
            cat "$secret_file" > "$hex_file"
        fi
        
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
    
    # Copy genesis.json if it exists
    if [ -f "$PROJECT_ROOT/execution-client/shared/genesis.json" ]; then
        cp "$PROJECT_ROOT/execution-client/shared/genesis.json" "$data_dir/genesis.json"
        log_info "Copied genesis.json to $data_dir"
    else
        log_warning "Genesis file not found, using default chain"
    fi
    
    # Initialize the node if genesis exists
    if [ -f "$data_dir/genesis.json" ]; then
        log_info "Initializing node with genesis..."
        "$PROJECT_ROOT/target/release/fastevm-execution" init --datadir "$data_dir" --chain "$data_dir/genesis.json" || true
    fi
}

# Initialize consensus node data
init_consensus_node() {
    local node_index="$1"
    local data_dir="$DATA_DIR/consensus$node_index"
    local consensus_port="${CONSENSUS_PORTS[$((node_index-1))]}"
    local consensus_ip="${CONSENSUS_IPS[$((node_index-1))]}"
    
    log_info "Initializing consensus node $node_index..."
    
    # Create node configuration
    local config_file="$data_dir/node.yml"
    local template_file="$PROJECT_ROOT/consensus-client/examples/node.template.yml"
    
    if [ -f "$template_file" ]; then
        cp "$template_file" "$config_file"
        
        # Replace placeholders
        sed -i '' "s/{NODE_INDEX}/$node_index/g" "$config_file" 2>/dev/null || sed -i "s/{NODE_INDEX}/$node_index/g" "$config_file"
        sed -i '' "s/{AUTHORITY_INDEX}/$((node_index-1))/g" "$config_file" 2>/dev/null || sed -i "s/{AUTHORITY_INDEX}/$((node_index-1))/g" "$config_file"
        sed -i '' "s/{JWT_SECRET}/0x$(cat "$DATA_DIR/execution$node_index/jwt.hex")/g" "$config_file" 2>/dev/null || sed -i "s/{JWT_SECRET}/0x$(cat "$DATA_DIR/execution$node_index/jwt.hex")/g" "$config_file"
        sed -i '' "s/{GENESIS_BLOCK_HASH}/0x0000000000000000000000000000000000000000000000000000000000000000/g" "$config_file" 2>/dev/null || sed -i "s/{GENESIS_BLOCK_HASH}/0x0000000000000000000000000000000000000000000000000000000000000000/g" "$config_file"
        
        log_info "Created consensus config: $config_file"
    else
        log_warning "Consensus template not found, creating basic config"
        cat > "$config_file" << EOF
node_index: $node_index
authority_index: $((node_index-1))
jwt_secret: "0x$(cat "$DATA_DIR/execution$node_index/jwt.hex")"
genesis_block_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
execution_url: "http://127.0.0.1:${ENGINE_PORTS[$((node_index-1))]}"
consensus_port: $consensus_port
consensus_ip: $consensus_ip
EOF
    fi
}

# Start execution node
start_execution_node() {
    local node_index="$1"
    local data_dir="$DATA_DIR/execution$node_index"
    local http_port="${EXECUTION_PORTS[$((node_index-1))]}"
    local engine_port="${ENGINE_PORTS[$((node_index-1))]}"
    local p2p_port="${P2P_PORTS[$((node_index-1))]}"
    local node_ip="${NODE_IPS[$((node_index-1))]}"
    local log_file="$LOGS_DIR/execution-node$node_index.log"
    local pid_file="$PIDS_DIR/execution-node$node_index.pid"
    
    log_info "Starting execution node $node_index on port $http_port..."
    
    # Build bootnodes string
    local bootnodes=""
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
    
    # Start the node
    nohup "$PROJECT_ROOT/target/release/fastevm-execution" \
        node \
        --chain "$data_dir/genesis.json" \
        --datadir "$data_dir" \
        --http \
        --http.api "eth,net,web3,admin,debug" \
        --http.addr "0.0.0.0" \
        --http.port "$http_port" \
        --http.corsdomain "*" \
        --ws \
        --ws.api "eth,net,web3,admin,debug" \
        --ws.addr "0.0.0.0" \
        --ws.port "$((http_port+1))" \
        --ws.origins "*" \
        --authrpc.addr "0.0.0.0" \
        --authrpc.port "$engine_port" \
        --authrpc.jwtsecret "$data_dir/jwt.hex" \
        --addr "0.0.0.0" \
        --port "$p2p_port" \
        --discovery.addr "0.0.0.0" \
        --discovery.port "$p2p_port" \
        --p2p-secret-key "$data_dir/p2p/secret.key" \
        --bootnodes "$bootnodes" \
        --enable-txpool-listener \
        -vvvv \
        > "$log_file" 2>&1 &
    
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
    export RUST_LOG=info
    export NODE_INDEX=$((node_index-1))
    export NODE_IP="$consensus_ip"
    
    # Start the node
    nohup "$PROJECT_ROOT/target/release/fastevm-consensus" \
        start \
        --config "$data_dir/node.yml" \
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
    
    # Initialize all nodes
    for i in {1..4}; do
        init_execution_node "$i"
        init_consensus_node "$i"
    done
    
    # Start execution nodes
    for i in {1..4}; do
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
    
    # Stop all processes
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
            echo "  ✅ Node $i: http://localhost:$http_port (RPC), http://localhost:$engine_port (Engine API)"
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

# Main script logic
case "${1:-start}" in
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
        for i in {1..4}; do
            init_execution_node "$i"
            init_consensus_node "$i"
        done
        log_success "Initialization complete!"
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|logs|cleanup|init}"
        echo
        echo "Commands:"
        echo "  start        - Start the local network (default)"
        echo "  stop         - Stop the local network"
        echo "  restart      - Restart the local network"
        echo "  status       - Show network status"
        echo "  logs [service] - Show logs (all services or specific service)"
        echo "  cleanup      - Clean up all data and stop network"
        echo "  init         - Initialize node data without starting"
        echo
        echo "Examples:"
        echo "  $0 start"
        echo "  $0 logs execution-node1"
        echo "  $0 logs consensus-node1"
        echo "  $0 status"
        exit 1
        ;;
esac
