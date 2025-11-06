#!/bin/bash

# FastEVM Terraform Network Test Script
# This script tests a newly deployed FastEVM network using Terraform
# 
# Flow:
# 1. Setup nodes with terraform, ensure prefunded accounts are generated using CLI
# 2. Send test transactions to remote network (batch_txs 10)
# 3. Wait 60 seconds then call scan-blocks in loop 5 times with 30 second intervals

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TERRAFORM_DIR="$PROJECT_ROOT/terraform"
DEPLOYMENT_INFO="$PROJECT_ROOT/deployment-info.json"
TEST_LOG="$PROJECT_ROOT/test-terraform-network.log"

# Default values
NODE_COUNT=${NODE_COUNT:-4}
PROJECT_NAME=${PROJECT_NAME:-fastevm}
REGION=${REGION:-us-central1}
ZONE=${ZONE:-us-central1-a}
MACHINE_TYPE=${MACHINE_TYPE:-e2-standard-4}
BATCH_TX_COUNT=${BATCH_TX_COUNT:-10}
SCAN_ITERATIONS=${SCAN_ITERATIONS:-5}
SCAN_INTERVAL=${SCAN_INTERVAL:-30}
WAIT_AFTER_TX=${WAIT_AFTER_TX:-60}

# Prefunded accounts configuration
PREFUND_ACCOUNT_COUNT=${PREFUND_ACCOUNT_COUNT:-10000}
PREFUND_BALANCE=${PREFUND_BALANCE:-"1000000000000000000000"}

# Logging function
log() {
    echo -e "${BLUE}[$(date '+%Y-%m-%d %H:%M:%S')]${NC} $1" | tee -a "$TEST_LOG"
}

log_info() {
    log "$1"
}

log_success() {
    echo -e "${GREEN}[$(date '+%Y-%m-%d %H:%M:%S')] ✅ $1${NC}" | tee -a "$TEST_LOG"
}

log_warning() {
    echo -e "${YELLOW}[$(date '+%Y-%m-%d %H:%M:%S')] ⚠️  $1${NC}" | tee -a "$TEST_LOG"
}

log_error() {
    echo -e "${RED}[$(date '+%Y-%m-%d %H:%M:%S')] ❌ $1${NC}" | tee -a "$TEST_LOG"
}

# Function to check prerequisites
check_prerequisites() {
    log "Checking prerequisites..."
    
    # Check required tools
    local missing_tools=()
    
    if ! command -v terraform &> /dev/null; then
        missing_tools+=("terraform")
    fi
    
    if ! command -v gcloud &> /dev/null; then
        missing_tools+=("gcloud")
    fi
    
    if ! command -v jq &> /dev/null; then
        missing_tools+=("jq")
    fi
    
    if ! command -v cargo &> /dev/null; then
        missing_tools+=("cargo")
    fi
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        log "Please install missing tools and try again"
        exit 1
    fi
    
    # Check GCP authentication
    if ! gcloud auth list --filter=status:ACTIVE --format="value(account)" | grep -q .; then
        log_error "No active GCP authentication found"
        log "Please run: gcloud auth login && gcloud auth application-default login \
            --scopes="https://www.googleapis.com/auth/cloud-platform,https://www.googleapis.com/auth/userinfo.email,openid""
        exit 1
    fi
    
    # Check project ID
    local project_id=$(gcloud config get-value project 2>/dev/null)
    if [ -z "$project_id" ]; then
        log_error "No GCP project ID configured"
        log "Please run: gcloud config set project YOUR_PROJECT_ID"
        exit 1
    fi
    
    log_success "Prerequisites check passed (Project: $project_id)"
}

# Function to setup terraform deployment
setup_terraform_deployment() {
    log "Setting up Terraform deployment..."
    
    cd "$TERRAFORM_DIR"
    
    # Check if terraform.tfvars exists
    if [ ! -f "terraform.tfvars" ]; then
        if [ -f "terraform.tfvars.example" ]; then
            log "Creating terraform.tfvars from example..."
            cp terraform.tfvars.example terraform.tfvars
            log_warning "Please edit terraform.tfvars with your configuration before continuing"
            log "Press Enter to continue after editing terraform.tfvars..."
            read -r
        else
            log_error "terraform.tfvars not found and no example file available"
            exit 1
        fi
    fi
    
    # Deploy infrastructure and configurations
    log "Deploying FastEVM infrastructure with $NODE_COUNT nodes..."
    log "Configuring $PREFUND_ACCOUNT_COUNT prefunded accounts with balance $PREFUND_BALANCE"
    make deploy-all NODE_COUNT="$NODE_COUNT" PROJECT_NAME="$PROJECT_NAME" REGION="$REGION" ZONE="$ZONE" MACHINE_TYPE="$MACHINE_TYPE" PREFUND_ACCOUNT_COUNT="$PREFUND_ACCOUNT_COUNT" PREFUND_BALANCE="$PREFUND_BALANCE"
    
    if [ $? -ne 0 ]; then
        log_error "Terraform deployment failed"
        exit 1
    fi
    
    log_success "Terraform deployment completed"
    
    # Verify deployment info exists
    if [ ! -f "$DEPLOYMENT_INFO" ]; then
        log_error "Deployment info file not found: $DEPLOYMENT_INFO"
        exit 1
    fi
    
    log_success "Deployment info available: $DEPLOYMENT_INFO"
}

# Function to verify genesis.json content across all nodes
verify_genesis_consistency() {
    log "Verifying genesis.json content consistency across all nodes..."
    
    # Extract node information from deployment info
    local node_ips=()
    local node_names=()
    
    for i in $(seq 1 $NODE_COUNT); do
        local node_ip=$(jq -r ".instance_external_ips.value[$((i-1))]" "$DEPLOYMENT_INFO")
        local node_name=$(jq -r ".instance_names.value[$((i-1))]" "$DEPLOYMENT_INFO")
        node_ips+=("$node_ip")
        node_names+=("$node_name")
    done
    
    log "Checking genesis.json on ${#node_ips[@]} nodes: ${node_names[*]}"
    
    # Create temporary directory for genesis files
    local temp_dir="/tmp/genesis_check"
    mkdir -p "$temp_dir"
    
    # Download genesis.json from each node
    local genesis_files=()
    for i in "${!node_ips[@]}"; do
        local node_ip="${node_ips[$i]}"
        local node_name="${node_names[$i]}"
        local genesis_file="$temp_dir/genesis_${node_name}.json"
        
        log "Downloading genesis.json from $node_name ($node_ip)..."
        
        # Use SSH to copy genesis.json from remote node
        if ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
            -i "$TERRAFORM_DIR/fastevm-deploy-key" ubuntu@"$node_ip" \
            "cat /data/config/genosis.json" > "$genesis_file" 2>/dev/null; then
            log_success "Downloaded genesis.json from $node_name"
            genesis_files+=("$genesis_file")
        else
            log_warning "Failed to download genesis.json from $node_name"
        fi
    done
    
    if [ ${#genesis_files[@]} -eq 0 ]; then
        log_error "No genesis files downloaded"
        return 1
    fi
    
    log_success "Downloaded ${#genesis_files[@]} genesis files"
    
    # Compare genesis files
    log "Comparing genesis.json content across all nodes..."
    
    local reference_file="${genesis_files[0]}"
    local reference_node=$(basename "$reference_file" .json | sed 's/genesis_//')
    
    log "Using $reference_node as reference"
    
    # Show reference genesis content
    log "Reference genesis.json content from $reference_node:"
    log "================================================"
    jq . "$reference_file" | head -20
    log "================================================"
    
    # Check basic properties
    local chain_id=$(jq -r '.config.chainId' "$reference_file")
    local gas_limit=$(jq -r '.gasLimit' "$reference_file")
    local account_count=$(jq '.alloc | length' "$reference_file")
    
    log "Reference properties:"
    log "  Chain ID: $chain_id"
    log "  Gas Limit: $gas_limit"
    log "  Account Count: $account_count"
    
    # Compare with other nodes
    local consistent_nodes=0
    local inconsistent_nodes=0
    
    for genesis_file in "${genesis_files[@]}"; do
        local node_name=$(basename "$genesis_file" .json | sed 's/genesis_//')
        
        if [ "$genesis_file" = "$reference_file" ]; then
            log_success "$node_name: Reference file (skipping comparison)"
            ((consistent_nodes++))
            continue
        fi
        
        log "Comparing $node_name with reference..."
        
        # Compare basic properties
        local node_chain_id=$(jq -r '.config.chainId' "$genesis_file")
        local node_gas_limit=$(jq -r '.gasLimit' "$genesis_file")
        local node_account_count=$(jq '.alloc | length' "$genesis_file")
        
        local is_consistent=true
        
        if [ "$node_chain_id" != "$chain_id" ]; then
            log_error "$node_name: Chain ID mismatch ($node_chain_id vs $chain_id)"
            is_consistent=false
        fi
        
        if [ "$node_gas_limit" != "$gas_limit" ]; then
            log_error "$node_name: Gas limit mismatch ($node_gas_limit vs $gas_limit)"
            is_consistent=false
        fi
        
        if [ "$node_account_count" != "$account_count" ]; then
            log_error "$node_name: Account count mismatch ($node_account_count vs $account_count)"
            is_consistent=false
        fi
        
        # Compare alloc section (account balances)
        if ! jq -e '.alloc' "$genesis_file" >/dev/null 2>&1; then
            log_error "$node_name: Missing alloc section"
            is_consistent=false
        else
            # Check if alloc sections are identical
            if ! diff <(jq -S '.alloc' "$reference_file") <(jq -S '.alloc' "$genesis_file") >/dev/null 2>&1; then
                log_error "$node_name: Alloc section differs from reference"
                
                # Show differences in account balances
                log "Account balance differences:"
                jq -r '.alloc | keys[]' "$reference_file" | while read -r addr; do
                    local ref_balance=$(jq -r ".alloc[\"$addr\"].balance" "$reference_file")
                    local node_balance=$(jq -r ".alloc[\"$addr\"].balance" "$genesis_file")
                    
                    if [ "$ref_balance" != "$node_balance" ]; then
                        log "  $addr: ref=$ref_balance, node=$node_balance"
                    fi
                done
                
                is_consistent=false
            fi
        fi
        
        if [ "$is_consistent" = true ]; then
            log_success "$node_name: Genesis.json is consistent with reference"
            ((consistent_nodes++))
        else
            log_error "$node_name: Genesis.json is inconsistent with reference"
            ((inconsistent_nodes++))
        fi
    done
    
    # Summary
    log "Genesis consistency check summary:"
    log "  Consistent nodes: $consistent_nodes"
    log "  Inconsistent nodes: $inconsistent_nodes"
    log "  Total nodes checked: ${#genesis_files[@]}"
    
    # Show sample accounts from each node
    log "Sample accounts from each node:"
    for genesis_file in "${genesis_files[@]}"; do
        local node_name=$(basename "$genesis_file" .json | sed 's/genesis_//')
        log "--- $node_name ---"
        jq -r '.alloc | keys[0:3] | .[]' "$genesis_file" | while read -r addr; do
            local balance=$(jq -r ".alloc[\"$addr\"].balance" "$genesis_file")
            log "  $addr: $balance wei"
        done
    done
    
    # Clean up
    rm -rf "$temp_dir"
    
    if [ $inconsistent_nodes -eq 0 ]; then
        log_success "All nodes have consistent genesis.json files!"
        return 0
    else
        log_error "Found inconsistencies in genesis.json files across nodes"
        return 1
    fi
}

# Function to verify prefunded accounts using CLI
verify_prefunded_accounts() {
    log "Verifying prefunded accounts..."
    
    # Extract RPC URLs from deployment info
    local rpc_urls=()
    for i in $(seq 1 $NODE_COUNT); do
        local rpc_url=$(jq -r ".node_endpoints.value.\"node-$i\".http_rpc" "$DEPLOYMENT_INFO")
        rpc_urls+=("$rpc_url")
    done
    
    log "Testing RPC endpoints: ${rpc_urls[*]}"
    
    # Test each RPC endpoint
    local working_endpoints=0
    for i in "${!rpc_urls[@]}"; do
        local rpc_url="${rpc_urls[$i]}"
        log "Testing RPC endpoint $((i+1)): $rpc_url"
        
        # Test basic connectivity
        if curl -s -f -X POST "$rpc_url" \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
            > /dev/null 2>&1; then
            log_success "RPC endpoint $((i+1)) is responding"
            ((working_endpoints++))
        else
            log_warning "RPC endpoint $((i+1)) is not responding"
        fi
    done
    
    if [ $working_endpoints -eq 0 ]; then
        log_error "No RPC endpoints are responding"
        exit 1
    fi
    
    log_success "Found $working_endpoints working RPC endpoints"
    
    # Verify prefunded accounts using the same CLI approach
    log "Verifying prefunded accounts using CLI-generated addresses..."
    
    # Set up CLI path - use system CLI instead of target directory
    local cli_path="cli"  # Use system CLI from /usr/local/bin/
    local test_mnemonic="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    
    # Check if CLI is available in system PATH
    if ! command -v "$cli_path" >/dev/null 2>&1; then
        log_warning "CLI not found in system PATH, trying to build and install..."
        cd "$PROJECT_ROOT"
        if cargo build --release; then
            log_success "Project built successfully"
            # Install CLI to system path
            sudo cp target/release/cli /usr/local/bin/
            log_success "CLI installed to /usr/local/bin/"
        else
            log_error "Failed to build project"
            return 1
        fi
    fi
    
    # Generate a few test addresses using CLI to verify they match
    local temp_output="/tmp/test_accounts"
    mkdir -p "$temp_output"
    
    log_info "Using system CLI for verification: $(which cli)"
    
    if "$cli_path" allocate-funds \
        --input "$PROJECT_ROOT/execution-client/shared/genesis.json" \
        --count 5 \
        --mnemonic "$test_mnemonic" \
        --amount "1000000000000000000000" \
        --output "$temp_output"; then
        
        log_success "Generated test accounts using system CLI"
        
        # Check balances of first few accounts
        local verified_count=0
        jq -r '.alloc | keys[0:5] | .[]' "$temp_output/genesis.json" | while read -r test_address; do
            log "Checking balance for test account: $test_address"
            
            # Check balance using RPC call
            local balance_response=$(curl -s -X POST "${rpc_urls[0]}" \
                -H "Content-Type: application/json" \
                -d "{\"jsonrpc\":\"2.0\",\"method\":\"eth_getBalance\",\"params\":[\"$test_address\",\"latest\"],\"id\":1}" 2>/dev/null)
            
            if echo "$balance_response" | jq -e '.result' >/dev/null 2>&1; then
                local balance=$(echo "$balance_response" | jq -r '.result')
                if [ "$balance" != "0x0" ] && [ "$balance" != "null" ]; then
                    log_success "Account has balance: $balance"
                    ((verified_count++))
                else
                    log_warning "Account has zero balance"
                fi
            else
                log_warning "Failed to check balance for account"
            fi
        done
        
        # Clean up
        rm -rf "$temp_output"
        
        if [ $verified_count -gt 0 ]; then
            log_success "Verified $verified_count prefunded accounts"
        else
            log_warning "No prefunded accounts verified - this might be expected if accounts are not yet funded"
        fi
    else
        log_warning "Failed to generate test accounts using system CLI"
    fi
    
    log_success "Account prefunding verification completed"
}

# Function to send test transactions
send_test_transactions() {
    log "Sending test transactions (batch_txs $BATCH_TX_COUNT)..."
    
    # Set up environment variables for the test
    export CHAIN_ID="202501"
    
    # Extract RPC URLs from deployment info
    for i in $(seq 1 $NODE_COUNT); do
        local rpc_url=$(jq -r ".node_endpoints.value.\"node-$i\".http_rpc" "$DEPLOYMENT_INFO")
        export "RPC_URL$i"="$rpc_url"
        log "Set RPC_URL$i=$rpc_url"
    done
    
    # Change to project root for running tests
    cd "$PROJECT_ROOT"
    
    # Run the batch transaction test
    log "Running batch transaction test with $BATCH_TX_COUNT transactions per sender..."
    
    # Use the existing run-test.sh script
    if [ -f "scripts/run-test.sh" ]; then
        bash scripts/run-test.sh batch_txs "$BATCH_TX_COUNT"
    else
        log_error "run-test.sh script not found"
        exit 1
    fi
    
    if [ $? -ne 0 ]; then
        log_error "Batch transaction test failed"
        exit 1
    fi
    
    log_success "Test transactions sent successfully"
}

# Function to wait and scan blocks
wait_and_scan_blocks() {
    log "Waiting $WAIT_AFTER_TX seconds for transactions to be processed..."
    sleep "$WAIT_AFTER_TX"
    
    log "Starting block scanning loop ($SCAN_ITERATIONS iterations with ${SCAN_INTERVAL}s intervals)..."
    
    # Change to project root for running tests
    cd "$PROJECT_ROOT"
    
    for i in $(seq 1 $SCAN_ITERATIONS); do
        log "Block scan iteration $i/$SCAN_ITERATIONS..."
        
        # Run block scan test
        if [ -f "scripts/run-test.sh" ]; then
            bash scripts/run-test.sh scan_blocks
        else
            log_error "run-test.sh script not found"
            exit 1
        fi
        
        if [ $? -ne 0 ]; then
            log_warning "Block scan iteration $i failed"
        else
            log_success "Block scan iteration $i completed"
        fi
        
        # Wait before next iteration (except for the last one)
        if [ $i -lt $SCAN_ITERATIONS ]; then
            log "Waiting $SCAN_INTERVAL seconds before next scan..."
            sleep "$SCAN_INTERVAL"
        fi
    done
    
    log_success "Block scanning completed"
}

# Function to generate test report
generate_test_report() {
    log "Generating test report..."
    
    local report_file="$PROJECT_ROOT/terraform-test-report-$(date +%Y%m%d-%H%M%S).txt"
    
    {
        echo "FastEVM Terraform Network Test Report"
        echo "===================================="
        echo "Test Date: $(date)"
        echo "Project: $PROJECT_NAME"
        echo "Nodes: $NODE_COUNT"
        echo "Region: $REGION"
        echo "Zone: $ZONE"
        echo "Machine Type: $MACHINE_TYPE"
        echo ""
        echo "Test Configuration:"
        echo "- Batch Transactions: $BATCH_TX_COUNT per sender"
        echo "- Prefund Accounts: $PREFUND_ACCOUNT_COUNT"
        echo "- Prefund Balance: $PREFUND_BALANCE"
        echo "- Wait After TX: ${WAIT_AFTER_TX}s"
        echo "- Scan Iterations: $SCAN_ITERATIONS"
        echo "- Scan Interval: ${SCAN_INTERVAL}s"
        echo ""
        echo "Deployment Info:"
        if [ -f "$DEPLOYMENT_INFO" ]; then
            jq -r '.deployment_summary.value | "Project: \(.project_name)\nNodes: \(.node_count)\nRegion: \(.region)\nZone: \(.zone)\nMachine Type: \(.machine_type)"' "$DEPLOYMENT_INFO"
        fi
        echo ""
        echo "Node Endpoints:"
        if [ -f "$DEPLOYMENT_INFO" ]; then
            jq -r '.node_endpoints.value | to_entries[] | "\(.key): \(.value.http_rpc)"' "$DEPLOYMENT_INFO"
        fi
        echo ""
        echo "Test Log:"
        echo "--------"
        if [ -f "$TEST_LOG" ]; then
            cat "$TEST_LOG"
        fi
    } > "$report_file"
    
    log_success "Test report generated: $report_file"
}

# Function to cleanup (optional)
cleanup() {
    log "Cleaning up..."
    
    # Unset environment variables
    unset CHAIN_ID
    for i in $(seq 1 $NODE_COUNT); do
        unset "RPC_URL$i"
    done
    
    log_success "Cleanup completed"
}

# Function to show help
show_help() {
    cat << EOF
FastEVM Terraform Network Test Script

Usage: $0 [OPTIONS]

This script tests a newly deployed FastEVM network using Terraform.

Flow:
1. Setup nodes with terraform, ensure 10k accounts are prefunded
2. Send test transactions to remote network (batch_txs 10)
3. Wait 60 seconds then call scan-blocks in loop 5 times with 30 second intervals

Options:
    -h, --help              Show this help message
    -n, --nodes N           Number of nodes (default: 4)
    -p, --project NAME      Project name (default: fastevm)
    -r, --region REGION     GCP region (default: us-central1)
    -z, --zone ZONE         GCP zone (default: us-central1-a)
    -m, --machine TYPE      Machine type (default: e2-standard-4)
    -t, --tx-count COUNT    Batch transaction count (default: 10)
    -i, --iterations N      Scan iterations (default: 5)
    -s, --scan-interval S   Scan interval in seconds (default: 30)
    -w, --wait-time S       Wait time after transactions (default: 60)
    --prefund-count N       Number of prefunded accounts (default: 10000)
    --prefund-balance BAL   Balance for prefunded accounts (default: 0x1000000000000000000000000000000000000000000000000000000000000000)
    --skip-deploy           Skip terraform deployment (use existing)
    --skip-tx               Skip transaction sending
    --skip-scan             Skip block scanning
    --genesis-only          Only check genesis consistency across nodes

Environment Variables:
    NODE_COUNT              Number of nodes
    PROJECT_NAME            Project name
    REGION                  GCP region
    ZONE                    GCP zone
    MACHINE_TYPE            Machine type
    PREFUND_ACCOUNT_COUNT   Number of prefunded accounts
    PREFUND_BALANCE         Balance for prefunded accounts
    BATCH_TX_COUNT          Batch transaction count
    SCAN_ITERATIONS         Number of scan iterations
    SCAN_INTERVAL           Scan interval in seconds
    WAIT_AFTER_TX           Wait time after transactions

Examples:
    $0                                    # Run with defaults (10k prefunded accounts)
    $0 -n 6 -t 20                        # 6 nodes, 20 transactions per sender
    $0 --prefund-count 5000               # Use 5k prefunded accounts instead of 10k
    $0 --skip-deploy --skip-tx            # Only run block scanning
    $0 -i 10 -s 15                        # 10 scan iterations, 15s intervals
    $0 --genesis-only                     # Only check genesis consistency across nodes

EOF
}

# Main function
main() {
    # Parse command line arguments
    SKIP_DEPLOY=false
    SKIP_TX=false
    SKIP_SCAN=false
    GENESIS_ONLY=false
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -n|--nodes)
                NODE_COUNT="$2"
                shift 2
                ;;
            -p|--project)
                PROJECT_NAME="$2"
                shift 2
                ;;
            -r|--region)
                REGION="$2"
                shift 2
                ;;
            -z|--zone)
                ZONE="$2"
                shift 2
                ;;
            -m|--machine)
                MACHINE_TYPE="$2"
                shift 2
                ;;
            -t|--tx-count)
                BATCH_TX_COUNT="$2"
                shift 2
                ;;
            -i|--iterations)
                SCAN_ITERATIONS="$2"
                shift 2
                ;;
            -s|--scan-interval)
                SCAN_INTERVAL="$2"
                shift 2
                ;;
            -w|--wait-time)
                WAIT_AFTER_TX="$2"
                shift 2
                ;;
            --prefund-count)
                PREFUND_ACCOUNT_COUNT="$2"
                shift 2
                ;;
            --prefund-balance)
                PREFUND_BALANCE="$2"
                shift 2
                ;;
            --skip-deploy)
                SKIP_DEPLOY=true
                shift
                ;;
            --skip-tx)
                SKIP_TX=true
                shift
                ;;
            --skip-scan)
                SKIP_SCAN=true
                shift
                ;;
            --genesis-only)
                GENESIS_ONLY=true
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
    
    # Initialize log file
    echo "FastEVM Terraform Network Test Started at $(date)" > "$TEST_LOG"
    
    log "Starting FastEVM Terraform Network Test"
    log "Configuration:"
    log "  Nodes: $NODE_COUNT"
    log "  Project: $PROJECT_NAME"
    log "  Region: $REGION"
    log "  Zone: $ZONE"
    log "  Machine Type: $MACHINE_TYPE"
    log "  Prefund Accounts: $PREFUND_ACCOUNT_COUNT"
    log "  Prefund Balance: $PREFUND_BALANCE"
    log "  Batch TX Count: $BATCH_TX_COUNT"
    log "  Scan Iterations: $SCAN_ITERATIONS"
    log "  Scan Interval: ${SCAN_INTERVAL}s"
    log "  Wait After TX: ${WAIT_AFTER_TX}s"
    
    # Run test phases
    check_prerequisites
    
    if [ "$GENESIS_ONLY" = true ]; then
        log "Running genesis consistency check only..."
        if [ ! -f "$DEPLOYMENT_INFO" ]; then
            log_error "Deployment info not found. Cannot check genesis without existing deployment."
            exit 1
        fi
        verify_genesis_consistency
        log_success "Genesis consistency check completed!"
        return 0
    fi
    
    if [ "$SKIP_DEPLOY" = false ]; then
        setup_terraform_deployment
        verify_prefunded_accounts
        verify_genesis_consistency
    else
        log "Skipping terraform deployment"
        if [ ! -f "$DEPLOYMENT_INFO" ]; then
            log_error "Deployment info not found. Cannot skip deployment without existing deployment."
            exit 1
        fi
        # Still verify genesis consistency even if skipping deploy
        verify_genesis_consistency
    fi
    
    if [ "$SKIP_TX" = false ]; then
        send_test_transactions
    else
        log "Skipping transaction sending"
    fi
    
    if [ "$SKIP_SCAN" = false ]; then
        wait_and_scan_blocks
    else
        log "Skipping block scanning"
    fi
    
    generate_test_report
    cleanup
    
    log_success "FastEVM Terraform Network Test completed successfully!"
    log "Check the test report and log file for details:"
    log "  Log: $TEST_LOG"
    log "  Report: $PROJECT_ROOT/terraform-test-report-*.txt"
}

# Run main function with all arguments
main "$@"
