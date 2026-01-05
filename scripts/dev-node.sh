#!/bin/bash

# FastEVM Dev Node Startup Script
# This script starts a single FastEVM dev node with custom genesis containing prefunded accounts
# It's designed for development and testing with a large number of prefunded accounts

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
DATA_DIR="$PROJECT_ROOT/.dev-node-data"
LOGS_DIR="$PROJECT_ROOT/.dev-node-logs"
PIDS_DIR="$PROJECT_ROOT/.dev-node-pids"

# Genesis configuration
GENESIS_FILE=$PROJECT_ROOT/execution-client/shared/genesis.json
GENESIS_OUTPUT_DIR="$DATA_DIR/genesis"
CLI=$PROJECT_ROOT/target/release/cli
EXECUTION_CLIENT=$PROJECT_ROOT/target/release/fastevm-execution
# Default values for account generation
DEFAULT_ACCOUNT_NUMBER=100000
DEFAULT_ACCOUNT_AMOUNT="1000000000000000000000"  # 1000 ETH in wei
DEFAULT_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

# Command line options (can be overridden by environment variables)
ACCOUNT_COUNT="${ACCOUNT_COUNT:-$DEFAULT_ACCOUNT_NUMBER}"
ACCOUNT_AMOUNT="${ACCOUNT_AMOUNT:-$DEFAULT_ACCOUNT_AMOUNT}"
MNEMONIC="${MNEMONIC:-$DEFAULT_MNEMONIC}"
ENABLE_WS=true
HTTP_PORT="${HTTP_PORT:-8545}"
WS_PORT="${WS_PORT:-8546}"
ENGINE_PORT="${ENGINE_PORT:-8551}"
P2P_PORT="${P2P_PORT:-30303}"
BUILD_PROJECT=true
FOREGROUND=false

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

    # Ensure default toolchain is set
    if command -v rustup &> /dev/null; then
        log_info "Ensuring default Rust toolchain is set..."
        rustup default stable 2>/dev/null || {
            log_info "Installing stable toolchain..."
            rustup toolchain install stable
            rustup default stable
        }
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
    
    mkdir -p "$DATA_DIR"
    mkdir -p "$LOGS_DIR"
    mkdir -p "$PIDS_DIR"
    mkdir -p "$GENESIS_OUTPUT_DIR"
    
    log_success "Directories created!"
}

# Generate and prefund accounts in genesis.json
prefund_genesis() {
    local account_count="${1:-$ACCOUNT_COUNT}"
    local account_amount="${2:-$ACCOUNT_AMOUNT}"
    local mnemonic="${3:-$MNEMONIC}"
    
    log_info "Generating prefunded genesis.json with $account_count accounts..."
    log_info "Each account will be funded with $account_amount wei"
    
    # Check if CLI is built
    if [ ! -f "$CLI" ]; then
        log_error "CLI not found at $CLI. Please build the project first."
        log_info "Building CLI..."
        build_project
    fi
    
    # Check if input genesis exists
    if [ ! -f "$GENESIS_FILE" ]; then
        log_error "Input genesis file not found: $GENESIS_FILE"
        return 1
    fi
    
    # Generate prefunded genesis
    if "$CLI" allocate-funds \
        --input "$GENESIS_FILE" \
        --count "$account_count" \
        --mnemonic "$mnemonic" \
        --amount "$account_amount" \
        --output "$GENESIS_OUTPUT_DIR"; then
        log_success "Generated prefunded genesis.json with $account_count accounts"
        log_info "Genesis file saved to: $GENESIS_OUTPUT_DIR/genesis.json"
        
        # Show some account addresses for reference
        log_info "Sample prefunded accounts:"
        jq -r '.alloc | keys[0:5] | .[]' "$GENESIS_OUTPUT_DIR/genesis.json" | while read -r addr; do
            local balance=$(jq -r ".alloc[\"$addr\"].balance" "$GENESIS_OUTPUT_DIR/genesis.json")
            log_info "  $addr: $balance wei"
        done
        
        # Show total accounts count
        local total_accounts=$(jq -r '.alloc | length' "$GENESIS_OUTPUT_DIR/genesis.json")
        log_info "Total accounts in genesis: $total_accounts"
        
        return 0
    else
        log_error "Failed to generate prefunded genesis.json"
        return 1
    fi
}

# Build the project
build_project() {
    log_info "Building FastEVM project..."
    
    cd "$PROJECT_ROOT"

    # Ensure default toolchain is set before building
    if command -v rustup &> /dev/null; then
        rustup default stable 2>/dev/null || {
            log_info "Installing stable toolchain..."
            rustup toolchain install stable
            rustup default stable
        }
    fi
    
    if cargo build --release; then
        log_success "Build completed successfully!"
    else
        log_error "Build failed!"
        exit 1
    fi

    # Verify binaries exist
    if [ ! -f "$CLI" ]; then
        log_error "CLI binary not found at $CLI after build"
        exit 1
    fi
    if [ ! -f "$EXECUTION_CLIENT" ]; then
        log_error "Execution client binary not found at $EXECUTION_CLIENT after build"
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
    local data_dir="$2"
    local p2p_dir="$data_dir/p2p"
    
    mkdir -p "$p2p_dir"
    
    local secret_file="$p2p_dir/secret.key"
    local hex_file="$p2p_dir/secret.hex"
    
    if [ ! -f "$secret_file" ]; then
        # Generate a proper 32-byte (64 hex chars) secret key
        local seed="fastevm-dev-node-p2p-secret-2025"
        echo "$seed" | openssl dgst -sha256 -hex | cut -d' ' -f2 | tr -d '\n' > "$secret_file"
        
        # Generate peer ID
        if [ -f "$CLI" ]; then
            $CLI show-peer-id --file "$secret_file" --output "$hex_file"
            # Remove 0x prefix if present
            if [ -f "$hex_file" ]; then
                sed -i '' 's/^0x//' "$hex_file" 2>/dev/null || sed -i 's/^0x//' "$hex_file"
            fi
        fi
        
        log_info "Generated P2P secret key"
    else
        log_info "P2P secret key already exists"
    fi
}

# Initialize execution node data
init_execution_node() {
    log_info "Initializing execution node..."
    
    # Generate JWT secret
    generate_jwt_secret "$DATA_DIR"
    
    # Generate P2P secret key
    generate_p2p_secret_key "1" "$DATA_DIR"
    
    # Copy prefunded genesis.json if it exists, otherwise fall back to original
    local prefunded_genesis="$GENESIS_OUTPUT_DIR/genesis.json"
    if [ -f "$prefunded_genesis" ]; then
        cp "$prefunded_genesis" "$DATA_DIR/genesis.json"
        log_info "Copied prefunded genesis.json to $DATA_DIR"
    elif [ -f "$GENESIS_FILE" ]; then
        cp "$GENESIS_FILE" "$DATA_DIR/genesis.json"
        log_info "Copied original genesis.json to $DATA_DIR"
    else
        log_warning "No genesis file found, using default chain"
    fi
    
    # Initialize the node if genesis exists
    if [ -f "$DATA_DIR/genesis.json" ]; then
        log_info "Initializing node with genesis..."
        "$EXECUTION_CLIENT" init --datadir "$DATA_DIR" --chain "$DATA_DIR/genesis.json" || true
    fi
}

# Start execution node
start_execution_node() {
    local log_file="$LOGS_DIR/execution-node.log"
    local pid_file="$PIDS_DIR/execution-node.pid"
    
    # Verify binary exists
    if [ ! -f "$EXECUTION_CLIENT" ]; then
        log_error "Execution client binary not found at $EXECUTION_CLIENT"
        log_info "Please build the project first or ensure the binary exists"
        exit 1
    fi
    
    log_info "Starting execution node..."
    log_info "HTTP RPC: http://localhost:$HTTP_PORT"
    if [ "$ENABLE_WS" = true ]; then
        log_info "WebSocket RPC: ws://localhost:$WS_PORT"
    fi
    log_info "Engine API: http://localhost:$ENGINE_PORT"
    log_info "P2P: localhost:$P2P_PORT"
    
    # Build command arguments
    local cmd_args=(
        "node"
        "--chain" "$DATA_DIR/genesis.json"
        "--datadir" "$DATA_DIR"
        "--instance" "1"
        "--engine.always-process-payload-attributes-on-canonical-head"
        "--http"
        "--http.api" "eth,net,web3,admin,debug,txpool"
        "--http.addr" "0.0.0.0"
        "--http.port" "$HTTP_PORT"
        "--http.corsdomain" "*"
    )
    
    # Add WebSocket arguments if enabled
    if [ "$ENABLE_WS" = true ]; then
        cmd_args+=(
            "--ws"
            "--ws.api" "eth,net,web3,admin,debug,txpool"
            "--ws.addr" "0.0.0.0"
            "--ws.port" "$WS_PORT"
            "--ws.origins" "*"
        )
    fi

    # Add txpool configuration for handling many accounts
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
    
    # Builder configuration
    cmd_args+=(
        "--builder.gaslimit" "240000000"
    )
    
    # Add remaining arguments
    cmd_args+=(
        "--authrpc.addr" "0.0.0.0"
        "--authrpc.port" "$ENGINE_PORT"
        "--authrpc.jwtsecret" "$DATA_DIR/jwt.hex"
        "--addr" "0.0.0.0"
        "--port" "$P2P_PORT"
        "--discovery.addr" "0.0.0.0"
        "--discovery.port" "$P2P_PORT"
        "--p2p-secret-key" "$DATA_DIR/p2p/secret.key"
        "--enable-tx-subscription"
        "--committed-subdags-per-block" "10"
        "--block-build-interval-ms" "100"
        "-vvv"
    )
    
    # Start the node in foreground or background
    if [ "$FOREGROUND" = true ]; then
        log_info "Running in foreground mode (logs will appear in this terminal)"
        log_info "Press Ctrl+C to stop the node"
        echo
        # Run in foreground
        "$EXECUTION_CLIENT" "${cmd_args[@]}"
    else
        # Start in background
        nohup "$EXECUTION_CLIENT" "${cmd_args[@]}" > "$log_file" 2>&1 &
        local pid=$!
        echo $pid > "$pid_file"
        log_info "Execution node started (PID: $pid, Log: $log_file)"
    fi
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

# Start the dev node
start_dev_node() {
    log_info "Starting FastEVM dev node..."
    
    # Generate prefunded genesis.json first
    prefund_genesis
    
    # Initialize node
    init_execution_node
    
    # Start execution node
    start_execution_node
    
    # If running in foreground, the function will block and we won't reach here
    if [ "$FOREGROUND" = true ]; then
        return 0
    fi
    
    # Wait for execution node to be ready (background mode only)
    wait_for_service "execution-node" "$HTTP_PORT" || {
        log_error "Execution node failed to start"
        show_logs
        exit 1
    }
    
    log_success "FastEVM dev node started successfully!"
    echo
    log_info "Node Information:"
    echo "  HTTP RPC: http://localhost:$HTTP_PORT"
    if [ "$ENABLE_WS" = true ]; then
        echo "  WebSocket RPC: ws://localhost:$WS_PORT"
    fi
    echo "  Engine API: http://localhost:$ENGINE_PORT"
    echo "  P2P Port: $P2P_PORT"
    echo "  Data Directory: $DATA_DIR"
    echo "  Logs: $LOGS_DIR/execution-node.log"
    echo
    log_info "To view logs: $0 logs"
    log_info "To stop: $0 stop"
}

# Stop the node
stop_node() {
    log_info "Stopping FastEVM dev node..."
    
    # Stop process from PID file
    local pid_file="$PIDS_DIR/execution-node.pid"
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if kill "$pid" 2>/dev/null; then
            log_info "Stopped execution node (PID: $pid)"
        else
            log_warning "Failed to stop execution node (PID: $pid)"
        fi
        rm -f "$pid_file"
    fi
    
    # Kill any process on the ports
    for PORT in "$HTTP_PORT" "$WS_PORT" "$ENGINE_PORT" "$P2P_PORT"; do
        PID=$(lsof -ti :$PORT 2>/dev/null)
        if [ -n "$PID" ]; then
            log_info "Killing process $PID on port $PORT"
            kill -9 $PID 2>/dev/null || true
        fi
    done

    log_success "Node stopped!"
}

# Show node status
show_status() {
    log_info "FastEVM Dev Node Status:"
    echo
    
    local pid_file="$PIDS_DIR/execution-node.pid"
    
    if [ -f "$pid_file" ] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
        local status_info="http://localhost:$HTTP_PORT (RPC)"
        if [ "$ENABLE_WS" = true ]; then
            status_info="$status_info, ws://localhost:$WS_PORT (WS)"
        fi
        status_info="$status_info, http://localhost:$ENGINE_PORT (Engine API)"
        echo "  ✅ Node: $status_info"
        echo "  Data: $DATA_DIR"
        echo "  Logs: $LOGS_DIR/execution-node.log"
    else
        echo "  ❌ Node: Not running"
    fi
}

# Show logs
show_logs() {
    local log_file="$LOGS_DIR/execution-node.log"
    if [ -f "$log_file" ]; then
        log_info "Showing logs (Ctrl+C to exit):"
        tail -f "$log_file"
    else
        log_error "Log file not found: $log_file"
    fi
}

# Clean up
cleanup() {
    log_info "Cleaning up dev node data..."
    
    stop_node
    
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
    if [[ $# -gt 0 ]] && [[ "$1" =~ ^(start|stop|restart|status|logs|cleanup|init|prefund|regenerate-genesis)$ ]]; then
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
            --http-port)
                HTTP_PORT="$2"
                shift 2
                ;;
            --ws-port)
                WS_PORT="$2"
                shift 2
                ;;
            --engine-port)
                ENGINE_PORT="$2"
                shift 2
                ;;
            --p2p-port)
                P2P_PORT="$2"
                shift 2
                ;;
            --foreground|--fg)
                FOREGROUND=true
                shift
                ;;
            --no-build)
                BUILD_PROJECT=false
                shift
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
    echo "Usage: $0 {start|stop|restart|status|logs|cleanup|init|regenerate-genesis} [OPTIONS]"
    echo
    echo "Commands:"
    echo "  start              - Start the dev node (default)"
    echo "  stop               - Stop the dev node"
    echo "  restart            - Restart the dev node"
    echo "  status             - Show node status"
    echo "  logs               - Show logs (follow mode)"
    echo "  cleanup            - Clean up all data and stop node"
    echo "  init               - Initialize node data without starting"
    echo "  regenerate-genesis - Regenerate prefunded genesis.json with new accounts"
    echo
    echo "Options:"
    echo "  --ws               - Enable WebSocket support (default)"
    echo "  --no-ws            - Disable WebSocket support"
    echo "  --accounts N       - Number of accounts to generate (default: $DEFAULT_ACCOUNT_NUMBER)"
    echo "  --amount X         - Amount in wei to fund each account (default: $DEFAULT_ACCOUNT_AMOUNT)"
    echo "  --mnemonic \"...\"   - Mnemonic phrase for account generation"
    echo "  --http-port PORT   - HTTP RPC port (default: 8545)"
    echo "  --ws-port PORT     - WebSocket RPC port (default: 8546)"
    echo "  --engine-port PORT - Engine API port (default: 8551)"
    echo "  --p2p-port PORT    - P2P port (default: 30303)"
    echo "  --foreground, --fg - Run node in foreground (logs in terminal, blocks)"
    echo "  --no-build         - Skip building the project"
    echo "  --help, -h         - Show this help message"
    echo
    echo "Environment Variables:"
    echo "  ACCOUNT_COUNT      - Number of accounts (overrides --accounts)"
    echo "  ACCOUNT_AMOUNT     - Amount per account (overrides --amount)"
    echo "  MNEMONIC           - Mnemonic phrase (overrides --mnemonic)"
    echo "  HTTP_PORT          - HTTP RPC port (overrides --http-port)"
    echo "  WS_PORT            - WebSocket RPC port (overrides --ws-port)"
    echo "  ENGINE_PORT        - Engine API port (overrides --engine-port)"
    echo "  P2P_PORT           - P2P port (overrides --p2p-port)"
    echo
    echo "Examples:"
    echo "  $0 start                                    # Start with default settings (100k accounts, background)"
    echo "  $0 start --foreground                      # Start in foreground (logs in terminal)"
    echo "  $0 start --accounts 500000                 # Start with 500k prefunded accounts"
    echo "  $0 start --accounts 10000 --amount 500000000000000000000  # 10k accounts with 500 ETH each"
    echo "  $0 regenerate-genesis --accounts 200000    # Generate 200k prefunded accounts"
    echo "  $0 start --no-ws                           # Start without WebSocket"
    echo "  ACCOUNT_COUNT=1000 $0 start                # Use environment variable"
    echo "  $0 logs                                    # Show logs"
    echo "  $0 status                                  # Show node status"
    echo
    echo "Note: The script will automatically build the project if binaries are not found."
}

# Parse arguments first
parse_arguments "$@"

# Main script logic
case "$COMMAND" in
    "start")
        check_prerequisites
        setup_directories
        if [ "$BUILD_PROJECT" = true ]; then
            build_project
        fi
        start_dev_node
        show_status
        ;;
    "stop")
        stop_node
        ;;
    "restart")
        stop_node
        sleep 2
        check_prerequisites
        setup_directories
        if [ "$BUILD_PROJECT" = true ]; then
            build_project
        fi
        start_dev_node
        show_status
        ;;
    "status")
        show_status
        ;;
    "logs")
        show_logs
        ;;
    "cleanup")
        cleanup
        ;;
    "init")
        check_prerequisites
        setup_directories
        if [ "$BUILD_PROJECT" = true ]; then
            build_project
        fi
        prefund_genesis
        init_execution_node
        log_success "Initialization complete!"
        ;;
    "prefund")
        check_prerequisites
        setup_directories
        if [ "$BUILD_PROJECT" = true ]; then
            build_project
        fi
        prefund_genesis
        ;;
    "regenerate-genesis")
        log_info "Regenerating prefunded genesis.json..."
        
        check_prerequisites
        setup_directories
        if [ "$BUILD_PROJECT" = true ]; then
            build_project
        fi
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

