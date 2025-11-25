#!/bin/bash

# Gravity Reth Dev Node Management Script
# This script manages node lifecycle: cleanup, data management, startup, and shutdown
# System setup (packages, Rust, building) is handled by execution-node-setup.sh
#
# Dev mode automatically prefunds 20 accounts with 10,000 ETH each
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
# SCRIPT_DIR will be /opt/dev-node.sh
# PROJECT_ROOT will be /opt/gravity-reth or /opt/reth depending on RETH_TYPE
RETH_TYPE="${RETH_TYPE:-gravity}"  # "gravity" or "reth"
DATA_DIR="/opt/bench/.dev-node-data"
LOGS_DIR="/opt/bench/.dev-node-logs"
PIDS_DIR="/opt/bench/.dev-node-pids"
sudo mkdir -p "/opt/bench"
sudo chown -R ubuntu:ubuntu "/opt/bench"
sudo chmod -R 755 "/opt/bench"

# Function to update paths based on RETH_TYPE
update_paths() {
    if [ "$RETH_TYPE" = "reth" ]; then
        PROJECT_ROOT="/opt/reth"
        # Use binary from /usr/local/bin if available, otherwise use build directory
        if [ -f "/usr/local/bin/reth" ]; then
            RETH_BIN="/usr/local/bin/reth"
        else
            RETH_BIN="$PROJECT_ROOT/target/release/reth"
        fi
    else
        PROJECT_ROOT="/opt/gravity-reth"
        # Use binary from /usr/local/bin if available, otherwise use build directory
        if [ -f "/usr/local/bin/gravity-reth" ]; then
            RETH_BIN="/usr/local/bin/gravity-reth"
        else
            RETH_BIN="$PROJECT_ROOT/target/release/reth"
        fi
    fi
}

# Initialize paths
update_paths

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
BUILDER_GAS_LIMIT="${BUILDER_GAS_LIMIT:-240000000}"
LOG_LEVEL="${LOG_LEVEL:-debug}"
DB_SYNC_MODE="${DB_SYNC_MODE:-}"

# Convert log level string to -v format
# trace = -vvvv, debug = -vvv, info = -vv, warn = -v, error = (no flag)
convert_log_level() {
    case "$1" in
        trace)
            echo "-vvvvv"
            ;;
        debug)
            echo "-vvvv"
            ;;
        info)
            echo "-vvv"
            ;;
        warn|warning)
            echo "-vv"
            ;;
        error)
            echo "-v"
            ;;
        *)
            # Default to debug if unknown
            echo "-vvv"
            ;;
    esac
}

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

    # Ensure the bench directory exists with correct permissions
    local bench_dir="$PROJECT_ROOT/bench"
    if [ ! -d "$bench_dir" ]; then
        mkdir -p "$bench_dir"
        # Try to set ownership if we have permissions (may fail if not root/sudo)
        chown "$USER:$USER" "$bench_dir" 2>/dev/null || true
    fi
    
    # Ensure we can write to the bench directory
    if [ ! -w "$bench_dir" ]; then
        log_error "Cannot write to $bench_dir. Please check permissions."
        exit 1
    fi

    mkdir -p "$DATA_DIR"
    mkdir -p "$LOGS_DIR"
    mkdir -p "$PIDS_DIR"

    log_success "Directories created!"
}

# Build the project
build_project() {
    local node_name="Gravity Reth"
    if [ "$RETH_TYPE" = "reth" ]; then
        node_name="Reth"
    fi
    log_info "Building $node_name project..."

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
    local jwt_file="$1/jwt.hex"
    mkdir -p "$(dirname "$jwt_file")"
    if [ ! -f "$jwt_file" ]; then
        log_info "Generating JWT secret at $jwt_file"
        if ! openssl rand -hex 32 | tr -d '\n' > "$jwt_file" 2>/dev/null; then
            log_error "Failed to generate JWT secret"
            exit 1
        fi
        chmod 644 "$jwt_file" 2>/dev/null || true
        if [ ! -f "$jwt_file" ]; then
            log_error "JWT secret file not created at $jwt_file"
            log_info "Directory contents: $(ls -la "$(dirname "$jwt_file")" 2>/dev/null || echo 'cannot list')"
            exit 1
        fi
        log_success "JWT secret created successfully"
    else
        log_info "JWT secret already exists at $jwt_file"
    fi
}

# Cleanup: Kill processes and remove data directory
cleanup_before_start() {
    log_info "Cleaning up data and processes..."
    
    # Kill processes on ports first (more specific)
    for port in "$HTTP_PORT" "$WS_PORT" "$ENGINE_PORT" "$P2P_PORT"; do
        lsof -ti :$port 2>/dev/null | xargs kill -9 2>/dev/null || true
    done
    
    # Kill reth binary processes (but not scripts containing "reth" in their path)
    # Use pgrep to find reth processes and kill them specifically
    pgrep -f "reth.*node" 2>/dev/null | xargs kill -9 2>/dev/null || true
    pgrep -f "/usr/local/bin/reth" 2>/dev/null | xargs kill -9 2>/dev/null || true
    pgrep -f "/usr/local/bin/gravity-reth" 2>/dev/null | xargs kill -9 2>/dev/null || true
    pgrep -f "target/release/reth" 2>/dev/null | xargs kill -9 2>/dev/null || true
    
    # Wait a bit for processes to fully terminate
    sleep 2
    
    # Try to kill processes using the data directory (with timeout to prevent hanging)
    if command -v timeout &> /dev/null; then
        timeout 5 fuser -k "$DATA_DIR" 2>/dev/null || true
    else
        fuser -k "$DATA_DIR" 2>/dev/null || true
    fi
    
    # Additional wait after fuser
    sleep 1
    
    # Remove data directory and logs
    rm -rf "$DATA_DIR" "$LOGS_DIR/reth-node.log" 2>/dev/null || true
    
    # Recreate directories
    mkdir -p "$LOGS_DIR" "$PIDS_DIR"
    
    log_success "Cleanup completed"
}

# Initialize node data
init_node() {
    log_info "Initializing node data..."
    mkdir -p "$DATA_DIR" "$LOGS_DIR" "$PIDS_DIR"
    generate_jwt_secret "$DATA_DIR"
    log_success "Node data initialized!"
}

# Start execution node
start_execution_node() {
    [ ! -f "$RETH_BIN" ] && { log_error "Reth binary not found at $RETH_BIN"; exit 1; }
    
    local jwt_file="$DATA_DIR/jwt.hex"
    generate_jwt_secret "$DATA_DIR"
    [ ! -f "$jwt_file" ] && { log_error "JWT secret file not found"; exit 1; }
    
    local node_name="Gravity Reth"
    if [ "$RETH_TYPE" = "reth" ]; then
        node_name="Reth"
    fi
    log_info "Starting $node_name node in dev mode..."
    log_info "HTTP RPC: http://localhost:$HTTP_PORT"
    [ "$ENABLE_WS" = true ] && log_info "WebSocket RPC: ws://localhost:$WS_PORT"
    log_info "Engine API: http://localhost:$ENGINE_PORT"
    
    # Build command arguments
    local cmd_args=(
        "node" "--dev" "--datadir" "$DATA_DIR"
        "--http" "--http.api" "eth,net,web3,admin,debug,txpool"
        "--http.addr" "0.0.0.0" "--http.port" "$HTTP_PORT" "--http.corsdomain" "*"
    )
    
    [ "$ENABLE_WS" = true ] && cmd_args+=("--ws" "--ws.api" "eth,net,web3,admin,debug,txpool" "--ws.addr" "0.0.0.0" "--ws.port" "$WS_PORT" "--ws.origins" "*")
    [ -n "$DEV_BLOCK_TIME" ] && cmd_args+=("--dev.block-time" "$DEV_BLOCK_TIME")
    [ -n "$DEV_BLOCK_MAX_TXNS" ] && cmd_args+=("--dev.block-max-transactions" "$DEV_BLOCK_MAX_TXNS")
    [ -n "$DB_SYNC_MODE" ] && cmd_args+=("--db.sync-mode" "$DB_SYNC_MODE")
    
    # Convert log level to -v format
    local log_level_flag=$(convert_log_level "$LOG_LEVEL")
    cmd_args+=(
        "--builder.gaslimit" "$BUILDER_GAS_LIMIT"
        "--authrpc.addr" "0.0.0.0" "--authrpc.port" "$ENGINE_PORT" "--authrpc.jwtsecret" "$jwt_file"
        "--rpc.max-connections" "10000" "--addr" "0.0.0.0" "--port" "$P2P_PORT"
        "--txpool.max-new-txns" "102400" "--txpool.max-account-slots" "102400"
        "--txpool.max-pending-txns" "102400" "--txpool.pending-max-count" "102400"
        "--txpool.pending-max-size" "128" "--txpool.max-new-pending-txs-notifications" "102400"
        "--txpool.queued-max-count" "102400" "--txpool.queued-max-size" "128"
    )
    # Only add gravity-specific flags if using gravity-reth
    if [ "$RETH_TYPE" = "gravity" ]; then
        cmd_args+=(
            "--gravity.disable-pipe-execution"
        )
    fi
    # Add log level flag only if not empty
    [ -n "$log_level_flag" ] && cmd_args+=("$log_level_flag")
    
    if [ "$FOREGROUND" = true ]; then
        log_info "Running in foreground mode (Ctrl+C to stop)"
        "$RETH_BIN" "${cmd_args[@]}"
    else
        nohup "$RETH_BIN" "${cmd_args[@]}" > "$LOGS_DIR/reth-node.log" 2>&1 &
        echo $! > "$PIDS_DIR/reth-node.pid"
        log_info "Node started (PID: $(cat "$PIDS_DIR/reth-node.pid"), Log: $LOGS_DIR/reth-node.log)"
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

# Stop the node (used before starting to ensure clean state)
stop_node_clean() {
    [ -f "$PIDS_DIR/reth-node.pid" ] && kill "$(cat "$PIDS_DIR/reth-node.pid")" 2>/dev/null || true
    for port in "$HTTP_PORT" "$WS_PORT" "$ENGINE_PORT" "$P2P_PORT"; do
        lsof -ti :$port 2>/dev/null | xargs kill -9 2>/dev/null || true
    done
    # Kill reth binary processes specifically (avoid killing scripts)
    pgrep -f "reth.*node" 2>/dev/null | xargs kill -9 2>/dev/null || true
    pgrep -f "/usr/local/bin/reth" 2>/dev/null | xargs kill -9 2>/dev/null || true
    pgrep -f "/usr/local/bin/gravity-reth" 2>/dev/null | xargs kill -9 2>/dev/null || true
    pgrep -f "target/release/reth" 2>/dev/null | xargs kill -9 2>/dev/null || true
    rm -f "$PIDS_DIR/reth-node.pid"
    sleep 1
}

# Start the dev node
start_dev_node() {
    local node_name="Gravity Reth"
    if [ "$RETH_TYPE" = "reth" ]; then
        node_name="Reth"
    fi
    log_info "Starting $node_name dev node..."
    log_info "Ensuring clean state (stopping any existing processes)..."
    stop_node_clean
    cleanup_before_start
    init_node
    log_info "Starting new node instance..."
    start_execution_node
    
    [ "$FOREGROUND" = true ] && return 0
    
    wait_for_service "reth-node" "$HTTP_PORT" || {
        log_error "Node failed to start"
        show_logs
        exit 1
    }
    
    fund_deployer_account
    
    log_success "$node_name dev node started successfully!"
    echo
    log_info "Node Information:"
    echo "  HTTP RPC: http://localhost:$HTTP_PORT"
    [ "$ENABLE_WS" = true ] && echo "  WebSocket RPC: ws://localhost:$WS_PORT"
    echo "  Engine API: http://localhost:$ENGINE_PORT"
    echo "  P2P Port: $P2P_PORT"
    echo "  Data Directory: $DATA_DIR"
    echo "  Logs: $LOGS_DIR/reth-node.log"
    echo
    log_info "Dev Mode: 20 accounts prefunded with 10,000 ETH each"
    log_info "Deployer: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
    echo
    log_info "To view logs: $0 logs | To stop: $0 stop"
}

# Stop the node
stop_node() {
    local node_name="Gravity Reth"
    if [ "$RETH_TYPE" = "reth" ]; then
        node_name="Reth"
    fi
    log_info "Stopping $node_name dev node..."
    stop_node_clean
    log_success "Node stopped!"
}

# Show node status
show_status() {
    local node_name="Gravity Reth"
    if [ "$RETH_TYPE" = "reth" ]; then
        node_name="Reth"
    fi
    log_info "$node_name Dev Node Status:"
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
    if [[ $# -gt 0 ]] && [[ "$1" =~ ^(start|stop|restart|reset|status|logs|cleanup|init)$ ]]; then
        COMMAND="$1"
        shift  # Remove the command from arguments
    fi

    # Then parse remaining options
    while [[ $# -gt 0 ]]; do
        case $1 in
            --reth-type)
                RETH_TYPE="$2"
                if [ "$RETH_TYPE" != "gravity" ] && [ "$RETH_TYPE" != "reth" ]; then
                    log_error "Invalid RETH_TYPE: $RETH_TYPE. Must be 'gravity' or 'reth'"
                    exit 1
                fi
                # Update paths based on RETH_TYPE
                update_paths
                shift 2
                ;;
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
            --builder-gas-limit)
                BUILDER_GAS_LIMIT="$2"
                shift 2
                ;;
            --log-level)
                LOG_LEVEL="$2"
                shift 2
                ;;
            --db-sync-mode)
                DB_SYNC_MODE="$2"
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
    echo "  reset              - Clean all data and start fresh dev node"
    echo "  status             - Show node status"
    echo "  logs               - Show logs (follow mode)"
    echo "  cleanup            - Clean up all data and stop node"
    echo "  init               - Initialize node data without starting"
    echo
    echo "Options:"
    echo "  --reth-type TYPE  - Reth type: 'gravity' or 'reth' (default: gravity)"
    echo "  --ws               - Enable WebSocket support (default)"
    echo "  --no-ws            - Disable WebSocket support"
    echo "  --http-port PORT   - HTTP RPC port (default: 8545)"
    echo "  --ws-port PORT     - WebSocket RPC port (default: 8546)"
    echo "  --engine-port PORT - Engine API port (default: 8551)"
    echo "  --p2p-port PORT    - P2P port (default: 30303)"
    echo "  --dev-block-time DURATION - Block time interval (e.g., 12s)"
    echo "  --dev-block-max-txns N    - Max transactions per block"
    echo "  --builder-gas-limit N    - Block gas limit (default: 240000000)"
    echo "  --log-level LEVEL        - Log level: trace (-vvvv), debug (-vvv), info (-vv), warn (-v), error (default: debug)"
    echo "  --db-sync-mode MODE      - Database sync mode: durable, nometasync, safenosync, utterlynosync"
    echo "  --foreground, --fg - Run node in foreground (logs in terminal, blocks)"
    echo "  --no-build         - Skip building the project"
    echo "  --help, -h         - Show this help message"
    echo
    echo "Environment Variables:"
    echo "  RETH_TYPE          - Reth type: 'gravity' or 'reth' (default: gravity)"
    echo "  HTTP_PORT          - HTTP RPC port (overrides --http-port)"
    echo "  WS_PORT            - WebSocket RPC port (overrides --ws-port)"
    echo "  ENGINE_PORT        - Engine API port (overrides --engine-port)"
    echo "  P2P_PORT           - P2P port (overrides --p2p-port)"
    echo "  BUILDER_GAS_LIMIT  - Block gas limit (default: 240000000)"
    echo "  LOG_LEVEL          - Log level: trace (-vvvvv), debug (-vvvv), info (-vvv), warn (-vv), error (-v), ( default: debug)"
    echo "  DEV_BLOCK_TIME     - Block time interval (overrides --dev-block-time)"
    echo "  DEV_BLOCK_MAX_TXNS - Max transactions per block (overrides --dev-block-max-txns)"
    echo "  DB_SYNC_MODE       - Database sync mode: durable, nometasync, safenosync, utterlynosync (overrides --db-sync-mode)"
    echo
    echo "Examples:"
    echo "  $0 start                                    # Start with default settings (background, gravity-reth)"
    echo "  $0 start --reth-type reth                  # Start reth"
    echo "  $0 start --reth-type gravity               # Start gravity-reth (default)"
    echo "  $0 reset                                    # Clean all data and start fresh node"
    echo "  $0 reset --reth-type reth                  # Clean all data and start fresh reth node"
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

# Ensure paths are updated after parsing (in case RETH_TYPE was set via env var)
update_paths

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
    "reset")
        log_info "Resetting dev node (cleaning all data and starting fresh)..."
        cleanup
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

