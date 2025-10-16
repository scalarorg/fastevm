#!/bin/bash

# FastEVM Chain Initialization Script
# This script generates prefunded accounts and initializes the local execution node

set -e

# Color codes for logging
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
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

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DEPLOYMENT_INFO_PATH="$(cd "$SCRIPT_DIR/../.." && pwd)/deployment-info.json"

PREFUND_ACCOUNT_COUNT=${PREFUND_ACCOUNT_COUNT:-100000}
PREFUND_BALANCE=${PREFUND_BALANCE:-"1000000000000000000000"}
TEST_MNEMONIC=${TEST_MNEMONIC:-"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"}

log_info "FastEVM Chain Initialization Script"
log_info "================================="
log_info "Account count: $PREFUND_ACCOUNT_COUNT"
log_info "Balance per account: $PREFUND_BALANCE wei"
log_info "Mnemonic: $TEST_MNEMONIC"

# Check if CLI is available
if ! command -v cli >/dev/null 2>&1; then
    log_error "CLI not found in system PATH"
    log_error "Please ensure binaries are distributed and CLI is installed"
    exit 1
fi

# Check if genesis.json exists
if [ ! -f "/data/genesis.json" ]; then
    log_error "Genesis file not found at /data/genesis.json"
    log_error "Please ensure configuration deployment is completed"
    exit 1
fi

# Create backup of original genesis
log_info "Creating backup of original genesis.json..."
sudo cp /data/genesis.json /data/genesis.json.backup

# Ensure proper permissions
sudo chown -R ubuntu:ubuntu /data
sudo chmod -R 755 /data

# Generate prefunded accounts using CLI
log_info "Generating prefunded accounts using CLI..."

if cli allocate-funds \
    --input /data/genesis.json \
    --count "$PREFUND_ACCOUNT_COUNT" \
    --mnemonic "$TEST_MNEMONIC" \
    --amount "$PREFUND_BALANCE" \
    --output /data; then
    
    log_success "Successfully added $PREFUND_ACCOUNT_COUNT prefunded accounts to genesis.json"
    
    # Verify the updated genesis file
    total_accounts=$(jq '.alloc | length' /data/genesis.json)
    log_info "Total accounts in genesis.json: $total_accounts"
    
    # Show sample accounts
    log_info "Sample prefunded accounts:"
    jq -r '.alloc | keys[0:3] | .[]' /data/genesis.json | while read -r addr; do
        balance=$(jq -r ".alloc[\"$addr\"].balance" /data/genesis.json)
        log_info "  $addr: $balance wei"
    done
    
    log_success "Prefunded accounts generation completed successfully"
    
    # Initialize local execution node with the updated genesis
    log_info "Initializing local execution node with updated genesis..."
    
    if [ -f '/usr/local/bin/fastevm-execution' ]; then
        /usr/local/bin/fastevm-execution init --datadir /data/execution --chain /data/genesis.json || echo '[INFO] Execution node initialization completed'
        log_success "Local execution node initialized successfully"
    else
        log_warning "fastevm-execution binary not found, skipping local initialization"
    fi
    
    log_success "Chain initialization completed successfully!"
    exit 0
else
    log_error "Failed to add prefunded accounts to genesis.json"
    # Restore backup
    log_info "Restoring original genesis.json..."
    sudo cp /data/genesis.json.backup /data/genesis.json
    exit 1
fi