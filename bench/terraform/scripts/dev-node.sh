#!/bin/bash

# Gravity Reth Dev Node Startup Script
# This script starts a single Gravity Reth dev node in dev mode
# Dev mode automatically prefunds 20 accounts with 10,000 ETH each
# 
# The script also automatically funds the deployer account (0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266)
# which is the first account from the Hardhat/Anvil default mnemonic.
# This address corresponds to the private key used in bench_config.template for contract deployment.

# Re-execute as ubuntu user if running as root
if [ "$EUID" -eq 0 ]; then
    if id ubuntu &>/dev/null 2>&1; then
        SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}" 2>/dev/null || realpath "${BASH_SOURCE[0]}" 2>/dev/null || echo "${BASH_SOURCE[0]}")"
        exec sudo -u ubuntu bash "$SCRIPT_PATH" "$@"
    else
        echo "Error: Running as root but ubuntu user not found. Please run as ubuntu user or install ubuntu user."
        exit 1
    fi
fi

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
# When this script is placed at /opt/gravity-reth/bench/dev-node.sh:
# SCRIPT_DIR will be /opt/gravity-reth/bench
# PROJECT_ROOT will be /opt/gravity-reth
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DATA_DIR="$PROJECT_ROOT/bench/.dev-node-data"
LOGS_DIR="$PROJECT_ROOT/bench/.dev-node-logs"
PIDS_DIR="$PROJECT_ROOT/bench/.dev-node-pids"

# Binary paths
RETH_BIN="$PROJECT_ROOT/target/release/reth"

# Command line options (can be overridden by environment variables)
ENABLE_WS=true
HTTP_PORT="${HTTP_PORT:-8545}"
WS_PORT="${WS_PORT:-8546}"
ENGINE_PORT="${ENGINE_PORT:-8551}"
P2P_PORT="${P2P_PORT:-30303}"
BUILD_PROJECT=true
FOREGROUND=false
DEV_BLOCK_TIME="${DEV_BLOCK_TIME:-}"
DEV_BLOCK_MAX_TXNS="${DEV_BLOCK_MAX_TXNS:-}"

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

# Setup environment for cargo/rustup
setup_cargo_env() {
    # Source cargo environment from current user's home
    local cargo_env="$HOME/.cargo/env"
    
    if [ -f "$cargo_env" ]; then
        source "$cargo_env" 2>/dev/null || true
        export PATH="$HOME/.cargo/bin:$PATH"
    fi
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    # Setup cargo environment (handles sudo case)
    setup_cargo_env

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

    log_success "Directories created!"
}

# Build the project
build_project() {
    log_info "Building Gravity Reth project..."

    # Setup cargo environment (handles sudo case)
    setup_cargo_env

    cd "$PROJECT_ROOT"

    # Ensure default toolchain is set before building
    if command -v rustup &> /dev/null; then
        rustup default stable 2>/dev/null || {
            log_info "Installing stable toolchain..."
            rustup toolchain install stable
            rustup default stable
        }
    fi

    if cargo build --release --bin reth; then
        log_success "Build completed successfully!"
    else
        log_error "Build failed!"
        exit 1
    fi

    # Verify binary exists
    if [ ! -f "$RETH_BIN" ]; then
        log_error "Binary not found at $RETH_BIN after build"
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

# Initialize node data
init_node() {
    log_info "Initializing node data..."

    # Generate JWT secret
    generate_jwt_secret "$DATA_DIR"

    log_success "Node data initialized!"
}

# Start execution node
start_execution_node() {
    local log_file="$LOGS_DIR/reth-node.log"
    local pid_file="$PIDS_DIR/reth-node.pid"

    # Verify binary exists
    if [ ! -f "$RETH_BIN" ]; then
        log_error "Reth binary not found at $RETH_BIN"
        log_info "Please build the project first or ensure the binary exists"
        exit 1
    fi

    log_info "Starting Gravity Reth node in dev mode..."
    log_info "Dev mode prefunds 20 accounts with 10,000 ETH each"
    log_info "HTTP RPC: http://localhost:$HTTP_PORT"
    if [ "$ENABLE_WS" = true ]; then
        log_info "WebSocket RPC: ws://localhost:$WS_PORT"
    fi
    log_info "Engine API: http://localhost:$ENGINE_PORT"
    log_info "P2P: localhost:$P2P_PORT"

    # Build command arguments
    local cmd_args=(
        "node"
        "--dev"
        "--datadir" "$DATA_DIR"
        "--http"
        "--http.api" "eth,net,web3,admin,debug"
        "--http.addr" "0.0.0.0"
        "--http.port" "$HTTP_PORT"
        "--http.corsdomain" "*"
    )

    # Add WebSocket arguments if enabled
    if [ "$ENABLE_WS" = true ]; then
        cmd_args+=(
            "--ws"
            "--ws.api" "eth,net,web3,admin,debug"
            "--ws.addr" "0.0.0.0"
            "--ws.port" "$WS_PORT"
            "--ws.origins" "*"
        )
    fi

    # Add dev block time if specified
    if [ -n "$DEV_BLOCK_TIME" ]; then
        cmd_args+=(
            "--dev.block-time" "$DEV_BLOCK_TIME"
        )
    fi

    # Add dev block max transactions if specified
    if [ -n "$DEV_BLOCK_MAX_TXNS" ]; then
        cmd_args+=(
            "--dev.block-max-transactions" "$DEV_BLOCK_MAX_TXNS"
        )
    fi

    # Add engine API arguments
    cmd_args+=(
        "--authrpc.addr" "0.0.0.0"
        "--authrpc.port" "$ENGINE_PORT"
        "--authrpc.jwtsecret" "$DATA_DIR/jwt.hex"
        "--rpc.max-connections" "10000"
    )

    # Add network arguments
    cmd_args+=(
        "--addr" "0.0.0.0"
        "--port" "$P2P_PORT"
    )
    
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
    # Disable gravity-specific features for dev mode
    # These features require additional setup that's not available in simple dev mode
    cmd_args+=(
        "--gravity.disable-pipe-execution"
        "--gravity.disable-grevm"
    )

    # Start the node in foreground or background
    if [ "$FOREGROUND" = true ]; then
        log_info "Running in foreground mode (logs will appear in this terminal)"
        log_info "Press Ctrl+C to stop the node"
        echo
        # Run in foreground
        "$RETH_BIN" "${cmd_args[@]}"
    else
        # Start in background
        nohup "$RETH_BIN" "${cmd_args[@]}" > "$log_file" 2>&1 &
        local pid=$!
        echo $pid > "$pid_file"
        log_info "Gravity Reth node started (PID: $pid, Log: $log_file)"
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

# Fund deployer account
# The deployer address 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 is the first account
# from the Hardhat/Anvil default mnemonic (same as reth dev mode mnemonic)
fund_deployer_account() {
    local rpc_url="http://localhost:$HTTP_PORT"
    local deployer_address="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
    local fund_amount="100000000000000000000"  # 100 ETH in wei
    
    log_info "Funding deployer account: $deployer_address"
    
    # Wait a bit more for the node to be fully ready
    sleep 3
    
    # Check current balance
    local balance_response=$(curl -s -X POST "$rpc_url" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"eth_getBalance\",\"params\":[\"$deployer_address\",\"latest\"],\"id\":1}" 2>/dev/null)
    
    if echo "$balance_response" | jq -e '.result' >/dev/null 2>&1; then
        local current_balance=$(echo "$balance_response" | jq -r '.result')
        log_info "Current deployer balance: $current_balance wei"
        
        # Convert hex balance to decimal for comparison (handle large numbers)
        local balance_hex=$(echo "$current_balance" | sed 's/0x//')
        local balance_decimal=$(python3 -c "print(int('$balance_hex', 16))" 2>/dev/null || echo "0")
        local fund_amount_decimal=$(python3 -c "print(int('$fund_amount'))" 2>/dev/null || echo "100000000000000000000")
        local min_balance_decimal=$(python3 -c "print(50 * 10**18)" 2>/dev/null || echo "50000000000000000000")
        
        # Only fund if balance is less than 50 ETH
        if [ "$balance_decimal" -lt "$min_balance_decimal" ] 2>/dev/null || [ -z "$balance_decimal" ]; then
            log_info "Deployer account needs funding. Sending $fund_amount wei (100 ETH)..."
            
            # Use the second account (index 1) from dev mnemonic to fund the deployer
            # Private key for account index 1 from mnemonic "test test test test test test test test test test test junk"
            # This is a well-known dev account that should be prefunded in dev mode
            local funder_private_key="0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
            local funder_address="0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
            
            # Create a Python script to sign and send the transaction
            local python_script=$(cat << 'PYTHON_EOF'
import json
import sys
from eth_account import Account
from web3 import Web3

rpc_url = sys.argv[1]
funder_private_key = sys.argv[2]
deployer_address = sys.argv[3]
fund_amount = int(sys.argv[4])

w3 = Web3(Web3.HTTPProvider(rpc_url))

# Get account address from private key
account = Account.from_key(funder_private_key)
funder_address = account.address

# Get nonce for the funder address
nonce = w3.eth.get_transaction_count(funder_address)

# Get gas price
gas_price = w3.eth.gas_price

# Build transaction
tx = {
    'to': deployer_address,
    'value': fund_amount,
    'gas': 21000,
    'gasPrice': gas_price,
    'nonce': nonce,
    'chainId': 1337  # Dev mode chain ID
}

# Sign transaction
signed_tx = account.sign_transaction(tx)

# Send transaction
tx_hash = w3.eth.send_raw_transaction(signed_tx.rawTransaction)

print(json.dumps({"success": True, "tx_hash": tx_hash.hex()}))
PYTHON_EOF
)
            
            # Try using Python with web3
            if command -v python3 &> /dev/null; then
                # Check if web3 is available
                if python3 -c "import web3, eth_account" 2>/dev/null; then
                    log_info "Using Python web3 to send transaction..."
                    local result=$(python3 -c "$python_script" "$rpc_url" "$funder_private_key" "$deployer_address" "$fund_amount" 2>&1)
                    
                    if echo "$result" | jq -e '.success' >/dev/null 2>&1; then
                        local tx_hash=$(echo "$result" | jq -r '.tx_hash')
                        log_success "Successfully funded deployer account. Transaction: $tx_hash"
                        # Wait for transaction to be mined
                        sleep 2
                        return 0
                    else
                        log_warning "Failed to fund deployer account using Python: $result"
                    fi
                fi
            fi
            
            # Try using cast (from foundry) if available
            if command -v cast &> /dev/null; then
                log_info "Using cast to send transaction..."
                local tx_hash=$(cast send --rpc-url "$rpc_url" \
                    --private-key "$funder_private_key" \
                    --value "$fund_amount" \
                    "$deployer_address" 2>&1)
                
                if [ $? -eq 0 ]; then
                    log_success "Successfully funded deployer account. Transaction: $tx_hash"
                    return 0
                else
                    log_warning "Failed to fund deployer account using cast: $tx_hash"
                fi
            fi
            
            # If all methods failed, log instructions
            log_warning "Could not automatically fund deployer account."
            log_info "Please manually fund the deployer account:"
            log_info "  Deployer address: $deployer_address"
            log_info "  Required amount: $fund_amount wei (100 ETH)"
            log_info "  You can use cast: cast send --rpc-url $rpc_url --private-key <funder_key> --value $fund_amount $deployer_address"
        else
            log_info "Deployer account already has sufficient balance: $current_balance wei"
        fi
    else
        log_warning "Failed to check deployer account balance. Node may not be fully ready yet."
    fi
}

# Start the dev node
start_dev_node() {
    log_info "Starting Gravity Reth dev node..."

    # Initialize node
    init_node

    # Start execution node
    start_execution_node

    # If running in foreground, the function will block and we won't reach here
    if [ "$FOREGROUND" = true ]; then
        return 0
    fi

    # Wait for execution node to be ready (background mode only)
    wait_for_service "reth-node" "$HTTP_PORT" || {
        log_error "Gravity Reth node failed to start"
        show_logs
        exit 1
    }

    # Fund the deployer account after node is ready
    fund_deployer_account

    log_success "Gravity Reth dev node started successfully!"
    echo
    log_info "Node Information:"
    echo "  HTTP RPC: http://localhost:$HTTP_PORT"
    if [ "$ENABLE_WS" = true ]; then
        echo "  WebSocket RPC: ws://localhost:$WS_PORT"
    fi
    echo "  Engine API: http://localhost:$ENGINE_PORT"
    echo "  P2P Port: $P2P_PORT"
    echo "  Data Directory: $DATA_DIR"
    echo "  Logs: $LOGS_DIR/reth-node.log"
    echo
    log_info "Dev Mode: 20 accounts prefunded with 10,000 ETH each"
    log_info "Mnemonic: test test test test test test test test test test test junk"
    log_info "Deployer address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
    echo
    log_info "To view logs: $0 logs"
    log_info "To stop: $0 stop"
}

# Stop the node
stop_node() {
    log_info "Stopping Gravity Reth dev node..."

    # Stop process from PID file
    local pid_file="$PIDS_DIR/reth-node.pid"
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if kill "$pid" 2>/dev/null; then
            log_info "Stopped reth node (PID: $pid)"
        else
            log_warning "Failed to stop reth node (PID: $pid)"
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
    log_info "Gravity Reth Dev Node Status:"
    echo

    local pid_file="$PIDS_DIR/reth-node.pid"

    if [ -f "$pid_file" ] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
        local status_info="http://localhost:$HTTP_PORT (RPC)"
        if [ "$ENABLE_WS" = true ]; then
            status_info="$status_info, ws://localhost:$WS_PORT (WS)"
        fi
        status_info="$status_info, http://localhost:$ENGINE_PORT (Engine API)"
        echo "  ✅ Node: $status_info"
        echo "  Data: $DATA_DIR"
        echo "  Logs: $LOGS_DIR/reth-node.log"
    else
        echo "  ❌ Node: Not running"
    fi
}

# Show logs
show_logs() {
    local log_file="$LOGS_DIR/reth-node.log"
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
    if [[ $# -gt 0 ]] && [[ "$1" =~ ^(start|stop|restart|status|logs|cleanup|init)$ ]]; then
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
            --dev-block-time)
                DEV_BLOCK_TIME="$2"
                shift 2
                ;;
            --dev-block-max-txns)
                DEV_BLOCK_MAX_TXNS="$2"
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
    echo "Usage: $0 {start|stop|restart|status|logs|cleanup|init} [OPTIONS]"
    echo
    echo "Commands:"
    echo "  start              - Start the dev node (default)"
    echo "  stop               - Stop the dev node"
    echo "  restart            - Restart the dev node"
    echo "  status             - Show node status"
    echo "  logs               - Show logs (follow mode)"
    echo "  cleanup            - Clean up all data and stop node"
    echo "  init               - Initialize node data without starting"
    echo
    echo "Options:"
    echo "  --ws               - Enable WebSocket support (default)"
    echo "  --no-ws            - Disable WebSocket support"
    echo "  --http-port PORT   - HTTP RPC port (default: 8545)"
    echo "  --ws-port PORT     - WebSocket RPC port (default: 8546)"
    echo "  --engine-port PORT - Engine API port (default: 8551)"
    echo "  --p2p-port PORT    - P2P port (default: 30303)"
    echo "  --dev-block-time DURATION - Block time interval (e.g., 12s)"
    echo "  --dev-block-max-txns N    - Max transactions per block"
    echo "  --foreground, --fg - Run node in foreground (logs in terminal, blocks)"
    echo "  --no-build         - Skip building the project"
    echo "  --help, -h         - Show this help message"
    echo
    echo "Environment Variables:"
    echo "  HTTP_PORT          - HTTP RPC port (overrides --http-port)"
    echo "  WS_PORT            - WebSocket RPC port (overrides --ws-port)"
    echo "  ENGINE_PORT        - Engine API port (overrides --engine-port)"
    echo "  P2P_PORT           - P2P port (overrides --p2p-port)"
    echo "  DEV_BLOCK_TIME     - Block time interval (overrides --dev-block-time)"
    echo "  DEV_BLOCK_MAX_TXNS - Max transactions per block (overrides --dev-block-max-txns)"
    echo
    echo "Examples:"
    echo "  $0 start                                    # Start with default settings (background)"
    echo "  $0 start --foreground                      # Start in foreground (logs in terminal)"
    echo "  $0 start --dev-block-time 12s              # Start with 12 second block time"
    echo "  $0 start --dev-block-max-txns 100         # Start with max 100 txns per block"
    echo "  $0 start --no-ws                           # Start without WebSocket"
    echo "  $0 logs                                    # Show logs"
    echo "  $0 status                                  # Show node status"
    echo
    echo "Note: Dev mode automatically prefunds 20 accounts with 10,000 ETH each."
    echo "      The script will automatically build the project if binaries are not found."
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
        if [ "$FOREGROUND" != true ]; then
            show_status
        fi
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
        if [ "$FOREGROUND" != true ]; then
            show_status
        fi
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
        init_node
        log_success "Initialization complete!"
        ;;
    *)
        show_help
        exit 1
        ;;
esac

