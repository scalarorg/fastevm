#!/bin/bash

# FastEVM Test Runner Script
# This script provides convenient wrappers around the fastevm-test CLI tool

set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ENV_FILE="$DIR/../.env"
# Load environment variables from .env file if it exists
load_env() {
    local env_file="${ENV_FILE:-.env}"
    if [ -f "$env_file" ]; then
        echo "📋 Loading environment variables from $env_file..."
        # Export variables from .env file, ignoring comments and empty lines
        set -a  # automatically export all variables
        source "$env_file"
        set +a  # stop automatically exporting
        echo "✅ Environment variables loaded"
    else
        echo "⚠️  No $env_file file found. Using default values."
        echo "   Create a $env_file file with your configuration. See examples below."
        echo "   You can also set ENV_FILE environment variable to specify a different file."
    fi
}

# Default values
DEFAULT_SENDER_COUNT=1000
DEFAULT_TRANSACTION_COUNT=1
DEFAULT_SCAN_COUNT=10
DEFAULT_SCAN_START=0

# Load environment variables
load_env

# Batch transaction testing using the new CLI
batch_txs() {
    local sender_count=${1:-$DEFAULT_SENDER_COUNT}
    local transaction_count=${2:-$DEFAULT_TRANSACTION_COUNT}
    
    echo "🚀 Running batch transaction test with $sender_count senders, $transaction_count txs/sender"
    echo "Using fastevm-test CLI..."
    
    # Pass ENV_FILE environment variable to the cargo run command
    cargo run --bin fastevm-test -- batch \
        --sender-count "$sender_count" \
        --transaction-count "$transaction_count"
}

# Block scanning using the new CLI
scan_blocks() {
    local start_block=${1:-$DEFAULT_SCAN_START}
    local count=${2:-$DEFAULT_SCAN_COUNT}
    
    echo "🔍 Scanning $count blocks starting from block $start_block"
    echo "Using fastevm-test CLI..."
    
    # Pass ENV_FILE environment variable to the cargo run command
    ENV_FILE="${ENV_FILE:-.env}" \
    cargo run --bin fastevm-test -- scan \
        --start "$start_block" \
        --count "$count"
}

# Scan all available blocks
scan_all_blocks() {
    echo "🔍 Scanning all available blocks..."
    echo "Using fastevm-test CLI..."
    
    # Pass ENV_FILE environment variable to the cargo run command
    ENV_FILE="${ENV_FILE:-.env}" \
    cargo run --bin fastevm-test -- scan-all
}

# Scan blocks in a specific range
scan_range() {
    local start_block=${1:-0}
    local end_block=${2:-10}
    
    echo "🔍 Scanning blocks from $start_block to $end_block"
    echo "Using fastevm-test CLI..."
    
    # Pass ENV_FILE environment variable to the cargo run command
    ENV_FILE="${ENV_FILE:-.env}" \
    cargo run --bin fastevm-test -- range \
        --start "$start_block" \
        --end "$end_block"
}

# Cross-node consistency tests (still using cargo test for now)
check_block_hash() {
    echo "🔍 Running cross-node block hash consistency test..."
    cargo test --test cross_nodes test_block_hash_consistency -- --nocapture
}

# Create sample .env file
create_env_file() {
    local env_file="${ENV_FILE:-.env}"
    if [ -f "$env_file" ]; then
        echo "⚠️  $env_file file already exists. Backing up to ${env_file}.backup"
        cp "$env_file" "${env_file}.backup"
    fi
    
    echo "📝 Creating sample $env_file file..."
    cat > "$env_file" << 'EOF'
# FastEVM Test Configuration
# Configuration for local testing

# RPC Endpoints for 4 execution nodes
# Update these with actual blockchain node IPs from your deployment
RPC_URL1=http://localhost:8545
RPC_URL2=http://localhost:8544
RPC_URL3=http://localhost:8543
RPC_URL4=http://localhost:8542

# Network Configuration
CHAIN_ID=202501

# Batch Transaction Test Parameters
TEST_SENDER_COUNT=1000
TEST_TRANSACTION_COUNT=1
TEST_TRANSACTION_VALUE=1000000000000000
TEST_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

# Test Configuration
TEST_FETCH_NONCE=false
TEST_WAITING_TIME_SECONDS=30

# Additional Test Parameters
TEST_RPC_TIMEOUT=30
TEST_MAX_RETRIES=3
TEST_LOG_LEVEL=info
EOF
    
    echo "✅ Sample .env file created!"
    echo "   Edit the file to match your network configuration."
}

# Run all cross-node consistency tests
run_cross_node_tests() {
    echo "🔍 Running all cross-node consistency tests..."
    cargo test --test cross_nodes -- --nocapture
}

# Show help
show_help() {
    echo "FastEVM Test Runner"
    echo "==================="
    echo ""
    echo "Available commands:"
    echo "  batch_txs [sender_count] [transaction_count]  - Run batch transaction test"
    echo "  scan_blocks [start_block] [count]              - Scan specific number of blocks"
    echo "  scan_all_blocks                               - Scan all available blocks"
    echo "  scan_range [start_block] [end_block]          - Scan blocks in a range"
    echo "  check_block_hash                              - Check block hash consistency"
    echo "  run_cross_node_tests                          - Run all cross-node tests"
    echo "  create_env_file                               - Create sample .env file"
    echo "  help                                          - Show this help"
    echo ""
    echo "Examples:"
    echo "  $0 batch_txs 500 2                            # 500 senders, 2 txs each"
    echo "  $0 scan_blocks 10 20                          # Scan 20 blocks starting from block 10"
    echo "  $0 scan_range 5 15                           # Scan blocks 5 to 15"
    echo ""
    echo "Environment Configuration:"
    echo "  Create a .env file in the project root with the following variables:"
    echo "  (or set ENV_FILE environment variable to specify a different file)"
    echo ""
    echo "  # RPC Endpoints for 4 execution nodes"
    echo "  RPC_URL1=http://localhost:8545"
    echo "  RPC_URL2=http://localhost:8544"
    echo "  RPC_URL3=http://localhost:8543"
    echo "  RPC_URL4=http://localhost:8542"
    echo ""
    echo "  # Network Configuration"
    echo "  CHAIN_ID=202501"
    echo ""
    echo "  # Test Parameters"
    echo "  TEST_SENDER_COUNT=1000"
    echo "  TEST_TRANSACTION_COUNT=1"
    echo "  TEST_MNEMONIC=\"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\""
    echo ""
    echo "  # Optional Parameters"
    echo "  TEST_FETCH_NONCE=false"
    echo "  TEST_TRANSACTION_VALUE=1000000000000000"
    echo ""
    echo "  If no .env file is found, default values will be used."
    echo "  You can also set ENV_FILE environment variable to specify a different file:"
    echo "  ENV_FILE=my-config.env $0 batch_txs"
}

# Main execution
if [ $# -eq 0 ]; then
    show_help
    exit 0
fi

# Handle special commands
case "$1" in
    "create_env_file")
        create_env_file
        exit 0
        ;;
    "help"|"-h"|"--help")
        show_help
        exit 0
        ;;
esac

# Execute the requested command
$@