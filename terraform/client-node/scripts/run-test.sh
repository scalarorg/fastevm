#!/bin/bash
# FastEVM Client Node Test Execution Script
# Handles RPC URL extraction from Terraform and test execution

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLIENT_NODE_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$CLIENT_NODE_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Check if we should disable colors
if [ "$TERM" = "dumb" ] || [ ! -t 1 ] || [ -n "$NO_COLOR" ]; then
    RED=""
    GREEN=""
    YELLOW=""
    BLUE=""
    NC=""
fi

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Function to find terraform command
find_terraform() {
    local terraform_cmd=$(which terraform 2>/dev/null)
    if [ -z "$terraform_cmd" ]; then
        # Try common installation paths
        for path in "/opt/homebrew/bin/terraform" "/usr/local/bin/terraform" "/usr/bin/terraform"; do
            if [ -x "$path" ]; then
                terraform_cmd="$path"
                break
            fi
        done
    fi
    
    if [ -z "$terraform_cmd" ] || [ ! -x "$terraform_cmd" ]; then
        log_error "Terraform not found. Please install terraform."
        exit 1
    fi
    
    echo "$terraform_cmd"
}

# Function to get client IP from Terraform
get_client_ip() {
    local terraform_cmd=$(find_terraform)
    local client_ip=$($terraform_cmd output -json client_node_info | jq -r '.external_ip')
    
    if [ "$client_ip" = "null" ] || [ -z "$client_ip" ]; then
        log_error "Client node not found. Run 'make apply' first."
        exit 1
    fi
    
    echo "$client_ip"
}

# Function to extract RPC URLs from Terraform
extract_rpc_urls() {
    local terraform_cmd=$(find_terraform)
    
    log_info "Extracting RPC URLs from main Terraform deployment..."
    log_info "Terraform directory: $PROJECT_ROOT"
    
    # Change to main terraform directory to get node_endpoints
    if [ ! -d "$PROJECT_ROOT" ]; then
        log_error "Terraform directory not found: $PROJECT_ROOT"
        log_error "Make sure you're running this from the terraform/client-node directory"
        exit 1
    fi
    
    cd "$PROJECT_ROOT"
    
    # Check if terraform state exists
    if [ ! -f "terraform.tfstate" ] && [ ! -f ".terraform/terraform.tfstate" ]; then
        log_error "Terraform state not found in $PROJECT_ROOT"
        log_error "Make sure the main FastEVM deployment is completed first"
        cd "$CLIENT_NODE_DIR"
        exit 1
    fi
    
    # Extract RPC URLs from main deployment
    log_info "Running: $terraform_cmd output -json node_endpoints"
    local node_endpoints_output=$($terraform_cmd output -json node_endpoints 2>&1)
    local terraform_exit_code=$?
    
    if [ $terraform_exit_code -ne 0 ]; then
        log_error "Failed to get terraform output: $node_endpoints_output"
        log_error "Make sure the main FastEVM deployment is completed and terraform is initialized"
        cd "$CLIENT_NODE_DIR"
        exit 1
    fi
    
    # Extract RPC URLs using http_rpc (which uses external/public IPs)
    local rpc_url1=$(echo "$node_endpoints_output" | jq -r '.["node-1"].http_rpc // empty' 2>/dev/null)
    local rpc_url2=$(echo "$node_endpoints_output" | jq -r '.["node-2"].http_rpc // empty' 2>/dev/null)
    local rpc_url3=$(echo "$node_endpoints_output" | jq -r '.["node-3"].http_rpc // empty' 2>/dev/null)
    local rpc_url4=$(echo "$node_endpoints_output" | jq -r '.["node-4"].http_rpc // empty' 2>/dev/null)
    
    # Extract external IPs for logging
    local external_ip1=$(echo "$node_endpoints_output" | jq -r '.["node-1"].external_ip // empty' 2>/dev/null)
    local external_ip2=$(echo "$node_endpoints_output" | jq -r '.["node-2"].external_ip // empty' 2>/dev/null)
    local external_ip3=$(echo "$node_endpoints_output" | jq -r '.["node-3"].external_ip // empty' 2>/dev/null)
    local external_ip4=$(echo "$node_endpoints_output" | jq -r '.["node-4"].external_ip // empty' 2>/dev/null)
    
    # Change back to client-node directory
    cd "$CLIENT_NODE_DIR"
    
    # Validate RPC URLs
    if [ "$rpc_url1" = "null" ] || [ -z "$rpc_url1" ]; then
        log_error "Failed to extract RPC URLs from main Terraform deployment"
        log_error "Terraform output: $node_endpoints_output"
        log_error "Make sure the main FastEVM deployment is completed first"
        log_error "Try running: cd $PROJECT_ROOT && terraform output node_endpoints"
        exit 1
    fi
    
    log_success "RPC URLs extracted successfully (using public IPs):"
    log_info "  RPC_URL1=$rpc_url1 (external IP: $external_ip1)"
    log_info "  RPC_URL2=$rpc_url2 (external IP: $external_ip2)"
    log_info "  RPC_URL3=$rpc_url3 (external IP: $external_ip3)"
    log_info "  RPC_URL4=$rpc_url4 (external IP: $external_ip4)"
    
    # Export RPC URLs
    export RPC_URL1="$rpc_url1"
    export RPC_URL2="$rpc_url2"
    export RPC_URL3="$rpc_url3"
    export RPC_URL4="$rpc_url4"
}

# Function to get current block number
get_current_block() {
    local client_ip="$1"
    local rpc_url="$2"
    
    log_info "Getting current block number from $rpc_url..." >&2
    
    local current_block=$(ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -i "$CLIENT_NODE_DIR/client-deploy-key" "ubuntu@$client_ip" "curl -s -X POST -H 'Content-Type: application/json' --data '{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}' $rpc_url | jq -r '.result' | xargs printf '%d\n' 2>/dev/null || echo '0'" 2>/dev/null)
    
    if [ -z "$current_block" ] || [ "$current_block" = "0" ]; then
        log_warning "Current block is empty or 0, using block 1 as fallback" >&2
        current_block=1
    fi
    
    log_success "Current block number: $current_block" >&2
    echo "$current_block"
}

# Function to get balance for an address
get_balance() {
    local client_ip="$1"
    local rpc_url="$2"
    local address="$3"
    
    log_info "Getting balance for address: $address"
    
    local balance=$(ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -i "$CLIENT_NODE_DIR/client-deploy-key" "ubuntu@$client_ip" "curl -s -X POST -H 'Content-Type: application/json' --data '{\"jsonrpc\":\"2.0\",\"method\":\"eth_getBalance\",\"params\":[\"$address\",\"latest\"],\"id\":1}' $rpc_url | jq -r '.result' 2>/dev/null || echo '0x0'" 2>/dev/null)
    
    log_success "Balance: $balance"
    echo "$balance"
}

# Function to run test with RPC URLs
run_test() {
    local client_ip="$1"
    local command="$2"
    local success_msg="$3"
    local error_msg="$4"
    
    log_info "Running test: $command"
    
    # Extract RPC URLs
    extract_rpc_urls
    
    # Run the test
    local exit_code=0
    ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -i "$CLIENT_NODE_DIR/client-deploy-key" "ubuntu@$client_ip" "cd /home/ubuntu && export RPC_URL1='$RPC_URL1' RPC_URL2='$RPC_URL2' RPC_URL3='$RPC_URL3' RPC_URL4='$RPC_URL4' ENV_FILE='/home/ubuntu/fastevm.env' && $command" 2>/dev/null || exit_code=$?
    
    # Check if the command was successful (exit code 0) or timed out (exit code 124)
    if [ $exit_code -eq 0 ] || [ $exit_code -eq 124 ]; then
        log_success "$success_msg"
        return 0
    else
        log_error "$error_msg (exit code: $exit_code)"
        return 1
    fi
}

# Function to run scan test with parameters
run_scan_test() {
    local client_ip="$1"
    local start_number="$2"
    local counter="$3"
    local success_msg="$4"
    local error_msg="$5"
    
    local command="fastevm-test scan"
    log_info "Running scan test with START_NUMBER=$start_number COUNTER=$counter"
    log_info "Command: $command"
    
    # Debug: Show the actual values being passed
    log_info "Debug - client_ip: '$client_ip', start_number: '$start_number', counter: '$counter'"

    if [ -z "$start_number" ] || [ "$start_number" = "0" ]; then
        log_error "START_NUMBER is empty or 0, cannot proceed with scan"
        log_error "$error_msg"
        exit 1
    fi
    
    # Extract RPC URLs
    extract_rpc_urls
    
    # Run the scan test
    local exit_code=0
    ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -i "$CLIENT_NODE_DIR/client-deploy-key" "ubuntu@$client_ip" "cd /home/ubuntu && export RPC_URL1='$RPC_URL1' RPC_URL2='$RPC_URL2' RPC_URL3='$RPC_URL3' RPC_URL4='$RPC_URL4' BLOCK_NUMBER=$start_number ENV_FILE='/home/ubuntu/fastevm.env' && $command --start $start_number --count $counter" 2>/dev/null || exit_code=$?
    
    # Check if the command was successful (exit code 0) or timed out (exit code 124)
    if [ $exit_code -eq 0 ] || [ $exit_code -eq 124 ]; then
        log_success "$success_msg"
        return 0
    else
        log_error "$error_msg (exit code: $exit_code)"
        return 1
    fi
}

# Function to check RPC endpoint health
check_rpc_health() {
    local client_ip="$1"
    local rpc_url="$2"
    local endpoint_name="$3"
    
    log_info "Checking health of $endpoint_name: $rpc_url"
    
    local response=$(ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -i "$CLIENT_NODE_DIR/client-deploy-key" "ubuntu@$client_ip" "curl -s -m 5 -X POST -H 'Content-Type: application/json' --data '{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}' $rpc_url 2>/dev/null" 2>/dev/null)
    
    if echo "$response" | jq -e '.result' >/dev/null 2>&1; then
        log_success "$endpoint_name is healthy"
        return 0
    else
        log_error "$endpoint_name is not responding: $response"
        return 1
    fi
}

# Function to verify all RPC endpoints are accessible
verify_rpc_endpoints() {
    local client_ip="$1"
    
    log_info "Verifying all RPC endpoints are accessible..."
    
    local all_healthy=true
    local healthy_count=0
    local failed_endpoints=""
    
    if check_rpc_health "$client_ip" "$RPC_URL1" "RPC_URL1"; then
        healthy_count=$((healthy_count + 1))
    else
        all_healthy=false
        failed_endpoints="$failed_endpoints RPC_URL1"
    fi
    
    if check_rpc_health "$client_ip" "$RPC_URL2" "RPC_URL2"; then
        healthy_count=$((healthy_count + 1))
    else
        all_healthy=false
        failed_endpoints="$failed_endpoints RPC_URL2"
    fi
    
    if check_rpc_health "$client_ip" "$RPC_URL3" "RPC_URL3"; then
        healthy_count=$((healthy_count + 1))
    else
        all_healthy=false
        failed_endpoints="$failed_endpoints RPC_URL3"
    fi
    
    if check_rpc_health "$client_ip" "$RPC_URL4" "RPC_URL4"; then
        healthy_count=$((healthy_count + 1))
    else
        all_healthy=false
        failed_endpoints="$failed_endpoints RPC_URL4"
    fi
    
    if [ $healthy_count -eq 0 ]; then
        log_error "❌ No RPC endpoints are accessible. Cannot proceed with tests."
        log_error "Please verify:"
        log_error "  1. Main FastEVM deployment is running"
        log_error "  2. RPC services are started on all nodes"
        log_error "  3. Firewall rules allow access from client node"
        log_error "  4. RPC URLs are correct:"
        log_error "     RPC_URL1=$RPC_URL1"
        log_error "     RPC_URL2=$RPC_URL2"
        log_error "     RPC_URL3=$RPC_URL3"
        log_error "     RPC_URL4=$RPC_URL4"
        exit 1
    elif [ "$all_healthy" = false ]; then
        log_warning "⚠️  Only $healthy_count out of 4 RPC endpoints are accessible"
        log_warning "Failed endpoints:$failed_endpoints"
        log_warning "Tests may fail or have reduced performance"
        log_warning "Consider checking the unreachable endpoints"
    else
        log_success "✅ All 4 RPC endpoints are accessible"
    fi
}

# Function to run auto-test sequence
run_auto_test() {
    local client_ip="$1"
    local test_sleep_duration="$2"
    
    log_info "Starting automated test sequence on $client_ip..."
    
    # Extract RPC URLs
    extract_rpc_urls
    
    # Verify all RPC endpoints are accessible before proceeding
    verify_rpc_endpoints "$client_ip"
    
    # Get current block number
    local start_block=$(get_current_block "$client_ip" "$RPC_URL1")
    log_info "Initial block number captured: $start_block"
    
    # Get balance
    get_balance "$client_ip" "$RPC_URL1" "0x07076387734b5b0a2c81d3a84a892fa5e89cdc76"
    
    # Run batch transaction test
    run_test "$client_ip" "fastevm-test batch" "Batch transaction test completed" "❌ Batch transaction test failed"
    
    # Wait for transactions to be processed
    log_info "Waiting $test_sleep_duration seconds for transactions to be processed..."
    sleep "$test_sleep_duration"
    
    # Run final block scan test
    run_scan_test "$client_ip" "$start_block" "200" "Final block scan test completed" "⚠️ Final block scan test had issues"
    
    log_success "🎉 Automated test sequence completed!"
}

# Main execution
main() {
    local action="$1"
    shift
    
    case "$action" in
        "get-current-block")
            local client_ip=$(get_client_ip)
            extract_rpc_urls
            get_current_block "$client_ip" "$RPC_URL1"
            ;;
        "get-balance")
            local client_ip=$(get_client_ip)
            local address="$1"
            extract_rpc_urls
            get_balance "$client_ip" "$RPC_URL1" "$address"
            ;;
        "run-test")
            local client_ip=$(get_client_ip)
            local timeout="$1"
            local command="$2"
            local success_msg="$3"
            local error_msg="$4"
            run_test "$client_ip" "$timeout" "$command" "$success_msg" "$error_msg"
            ;;
        "run-scan")
            local client_ip=$(get_client_ip)
            local start_number="$1"
            local counter="$2"
            local command="$3"
            local success_msg="$4"
            local error_msg="$5"
            run_scan_test "$client_ip" "$start_number" "$counter" "$command" "$success_msg" "$error_msg"
            ;;
        "run-scan-direct")
            local client_ip=$(get_client_ip)
            local start_number="$1"
            local counter="$2"
            extract_rpc_urls
            ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -i "$CLIENT_NODE_DIR/client-deploy-key" "ubuntu@$client_ip" "cd /home/ubuntu && export RPC_URL1='$RPC_URL1' RPC_URL2='$RPC_URL2' RPC_URL3='$RPC_URL3' RPC_URL4='$RPC_URL4' ENV_FILE='/home/ubuntu/fastevm.env' && fastevm-test scan $(if [ -n "$start_number" ]; then echo "--start $start_number"; fi) $(if [ -n "$counter" ]; then echo "--count $counter"; fi)" 2>/dev/null
            ;;
        "auto-test")
            local client_ip=$(get_client_ip)
            local test_sleep_duration="${1:-120}"
            run_auto_test "$client_ip" "$test_sleep_duration"
            ;;
        *)
            log_error "Unknown action: $action"
            echo "Usage: $0 <action> [args...]"
            echo "Actions:"
            echo "  get-current-block"
            echo "  get-balance <address>"
            echo "  run-test [timeout] <command> <success_msg> <error_msg>"
            echo "  run-scan <start_number> <counter> <command> <success_msg> <error_msg>"
            echo "  run-scan-direct <start_number> <counter>"
            echo "  auto-test [sleep_duration]"
            exit 1
            ;;
    esac
}

main "$@"
