#!/bin/bash
# Flood Load Testing Script
# This script runs flood load tests against the execution node
# Usage: ./flood-benchmark.sh [OPTIONS]

set -e
set -o pipefail

# Logging function
log() {
    local message="[$(date '+%Y-%m-%d %H:%M:%S')] $1"
    echo "$message"
}

log_error() {
    log "ERROR: $1" >&2
}

log_info() {
    log "INFO: $1"
}

# Default values
FLOOD_RATE="${FLOOD_RATE:-12000}"
FLOOD_SENDERS="${FLOOD_SENDERS:-10000}"
FLOOD_METHOD="${FLOOD_METHOD:-eth-transfer}"
FLOOD_OUTPUT_DIR="${FLOOD_OUTPUT_DIR:-/opt/flood_results}"

# Get execution node RPC endpoint
get_execution_node_rpc() {
    # Try to get from environment variable first
    if [ -n "$EXECUTION_NODE_RPC" ]; then
        echo "$EXECUTION_NODE_RPC"
        return 0
    fi
    
    # Try to get from terraform output (if running on client node with terraform access)
    if command -v terraform &> /dev/null; then
        local terraform_dir=""
        # Check if we're in a terraform directory
        if [ -f "./main.tf" ]; then
            terraform_dir="$(pwd)"
        elif [ -f "../main.tf" ]; then
            terraform_dir="$(cd .. && pwd)"
        elif [ -f "../../main.tf" ]; then
            terraform_dir="$(cd ../.. && pwd)"
        fi
        
        if [ -n "$terraform_dir" ]; then
            log_info "Getting execution node RPC from terraform output..."
            cd "$terraform_dir"
            local output=$(terraform output -json execution_node_info 2>/dev/null || echo "")
            if [ -n "$output" ] && [ "$output" != "null" ]; then
                # Extract http_rpc using jq
                local rpc_url=$(echo "$output" | jq -r '.http_rpc // empty' 2>/dev/null)
                if [ -n "$rpc_url" ] && [ "$rpc_url" != "null" ] && [ "$rpc_url" != "" ]; then
                    echo "$rpc_url"
                    cd - > /dev/null
                    return 0
                fi
                
                # Fallback: construct from internal_ip and http_port
                local internal_ip=$(echo "$output" | jq -r '.internal_ip // empty' 2>/dev/null)
                local http_port=$(terraform output -raw http_port 2>/dev/null || echo "8545")
                if [ -n "$internal_ip" ] && [ "$internal_ip" != "null" ]; then
                    echo "http://${internal_ip}:${http_port}"
                    cd - > /dev/null
                    return 0
                fi
            fi
            cd - > /dev/null
        fi
    fi
    
    # Try to get from environment variable set by deploy script
    if [ -n "${execution_node_internal_ip}" ] && [ -n "${http_port}" ]; then
        echo "http://${execution_node_internal_ip}:${http_port}"
        return 0
    fi
    
    # Default fallback
    log_error "Could not determine execution node RPC endpoint"
    log_info "Please set EXECUTION_NODE_RPC environment variable or ensure terraform outputs are available"
    return 1
}

# Check prerequisites
check_prerequisites() {
    # Check if flood is available
    if ! command -v flood &> /dev/null && ! python3 -m flood --help &> /dev/null 2>&1; then
        log_error "flood is not installed. Please run client-setup.sh first."
        log_info "Install with: pip3 install paradigm-flood"
        return 1
    fi
    
    # Check if vegeta is available
    if ! command -v vegeta &> /dev/null; then
        log_error "vegeta is not installed. Please run client-setup.sh first."
        log_info "Install with: go install github.com/tsenart/vegeta/v12@v12.8.4"
        return 1
    fi
    
    return 0
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --rpc)
                EXECUTION_NODE_RPC="$2"
                shift 2
                ;;
            --rate)
                FLOOD_RATE="$2"
                shift 2
                ;;
            --senders)
                FLOOD_SENDERS="$2"
                shift 2
                ;;
            --method)
                FLOOD_METHOD="$2"
                shift 2
                ;;
            --output)
                FLOOD_OUTPUT_DIR="$2"
                shift 2
                ;;
            --help|-h)
                cat << EOF
Usage: $0 [OPTIONS]

Options:
  --rpc URL          Execution node RPC endpoint (default: auto-detect from terraform)
  --rate RATE        Request rate per second (default: 12000)
  --senders SENDERS  Number of concurrent senders (default: 10000)
  --method METHOD    RPC method to test (default: eth-transfer)
  --output DIR       Output directory for results (default: /opt/flood_results)
  --help, -h         Show this help message

Environment Variables:
  EXECUTION_NODE_RPC - Execution node RPC endpoint
  FLOOD_RATE         - Request rate per second
  FLOOD_SENDERS      - Number of concurrent senders
  FLOOD_METHOD       - RPC method to test
  FLOOD_OUTPUT_DIR   - Output directory for results

Examples:
  $0 --rpc http://10.0.0.2:8545 --rate 12000 --senders 10000
  $0 --rate 5000 --senders 5000
  EXECUTION_NODE_RPC=http://10.0.0.2:8545 $0
EOF
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                log_info "Use --help for usage information"
                exit 1
                ;;
        esac
    done
}

# Main execution
main() {
    log_info "Starting flood load testing..."
    
    # Parse arguments
    parse_args "$@"
    
    # Check prerequisites
    check_prerequisites || exit 1
    
    # Get RPC endpoint
    RPC_ENDPOINT=$(get_execution_node_rpc)
    if [ $? -ne 0 ] || [ -z "$RPC_ENDPOINT" ]; then
        log_error "Failed to get execution node RPC endpoint"
        exit 1
    fi
    
    log_info "Execution node RPC: $RPC_ENDPOINT"
    log_info "Test method: $FLOOD_METHOD"
    log_info "Rate: $FLOOD_RATE requests/second"
    log_info "Senders: $FLOOD_SENDERS"
    log_info "Output directory: $FLOOD_OUTPUT_DIR"
    
    # Create output directory
    mkdir -p "$FLOOD_OUTPUT_DIR"
    
    # Determine flood command
    FLOOD_CMD=""
    if command -v flood &> /dev/null; then
        FLOOD_CMD="flood"
    elif python3 -m flood --help &> /dev/null 2>&1; then
        FLOOD_CMD="python3 -m flood"
    else
        log_error "flood command not found"
        exit 1
    fi
    
    # Run flood command
    # Note: The exact syntax may vary by flood version
    # Using the syntax provided by the user: flood eth-transfer --rpc URL --rate RATE --senders SENDERS
    log_info "Running flood load test..."
    
    # Try the user's specified syntax first
    if $FLOOD_CMD "$FLOOD_METHOD" --rpc "$RPC_ENDPOINT" --rate "$FLOOD_RATE" --senders "$FLOOD_SENDERS" --output "$FLOOD_OUTPUT_DIR" 2>&1; then
        log_info "Flood test completed successfully!"
        log_info "Results saved to: $FLOOD_OUTPUT_DIR"
    else
        # If that fails, try alternative syntax (flood method NODE_NAME=URL --rates RATE --duration DURATION)
        log_info "Trying alternative flood syntax..."
        local duration=60  # Default duration in seconds
        if $FLOOD_CMD "$FLOOD_METHOD" "test_node=$RPC_ENDPOINT" --rates "$FLOOD_RATE" --duration "$duration" --output "$FLOOD_OUTPUT_DIR" 2>&1; then
            log_info "Flood test completed successfully!"
            log_info "Results saved to: $FLOOD_OUTPUT_DIR"
        else
            log_error "Flood test failed. Check the output above for details."
            log_info "You may need to adjust the command syntax based on your flood version."
            exit 1
        fi
    fi
    
    # Print summary
    if [ -f "$FLOOD_OUTPUT_DIR/results.json" ]; then
        log_info "Test results available in: $FLOOD_OUTPUT_DIR/results.json"
    fi
    if [ -d "$FLOOD_OUTPUT_DIR/figures" ]; then
        log_info "Figures available in: $FLOOD_OUTPUT_DIR/figures/"
    fi
    
    log_info "Flood benchmark completed!"
}

# Run main function
main "$@"

