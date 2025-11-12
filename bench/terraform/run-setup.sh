#!/bin/bash

# Helper script to manually re-run setup scripts on nodes
# This is useful for debugging and re-executing setup after fixes

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

SSH_KEY="gravity-deploy-key"
SSH_USER="ubuntu"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Get node IP
get_node_ip() {
    local node_type="$1"
    local output=$(terraform output -json ${node_type}_node_info 2>/dev/null || echo "")
    
    if [ -z "$output" ] || [ "$output" = "null" ]; then
        log_error "${node_type} node not found in Terraform state."
        return 1
    fi
    
    echo "$output" | grep -o '"external_ip":"[^"]*"' | cut -d'"' -f4
}

# Run execution node setup
run_execution_setup() {
    local external_ip=$(get_node_ip "execution")
    if [ -z "$external_ip" ]; then
        exit 1
    fi
    
    if [ ! -f "$SSH_KEY" ]; then
        log_error "SSH key not found: $SSH_KEY"
        exit 1
    fi
    
    log_info "Running execution node setup on $external_ip..."
    
    # Get variables from terraform
    local gravity_reth_repo=$(terraform output -raw gravity_reth_repo 2>/dev/null || echo "https://github.com/Galxe/gravity-reth.git")
    local gravity_reth_branch=$(terraform output -raw gravity_reth_branch 2>/dev/null || echo "main")
    local http_port=$(terraform output -raw http_port 2>/dev/null || echo "8545")
    local ws_port=$(terraform output -raw ws_port 2>/dev/null || echo "8546")
    local engine_port=$(terraform output -raw engine_port 2>/dev/null || echo "8551")
    local p2p_port=$(terraform output -raw p2p_port 2>/dev/null || echo "30303")
    
    # Create temporary script with variables substituted
    local temp_script=$(mktemp)
    sed -e "s|\${gravity_reth_repo}|${gravity_reth_repo}|g" \
        -e "s|\${gravity_reth_branch}|${gravity_reth_branch}|g" \
        -e "s|\${http_port}|${http_port}|g" \
        -e "s|\${ws_port}|${ws_port}|g" \
        -e "s|\${engine_port}|${engine_port}|g" \
        -e "s|\${p2p_port}|${p2p_port}|g" \
        "${SCRIPT_DIR}/scripts/execution-node-setup.sh" > "$temp_script"
    
    # Copy script
    scp -i "$SSH_KEY" \
        -o StrictHostKeyChecking=no \
        "$temp_script" \
        "${SSH_USER}@${external_ip}:/tmp/setup-execution-node.sh"
    
    # Execute script
    ssh -i "$SSH_KEY" \
        -o StrictHostKeyChecking=no \
        "${SSH_USER}@${external_ip}" \
        "chmod +x /tmp/setup-execution-node.sh && sudo mv /tmp/setup-execution-node.sh /opt/setup-execution-node.sh && sudo /opt/setup-execution-node.sh"
    
    # Cleanup
    rm -f "$temp_script"
    
    log_success "Execution node setup completed!"
}

# Run client node setup
run_client_setup() {
    local external_ip=$(get_node_ip "client")
    if [ -z "$external_ip" ]; then
        exit 1
    fi
    
    if [ ! -f "$SSH_KEY" ]; then
        log_error "SSH key not found: $SSH_KEY"
        exit 1
    fi
    
    log_info "Running client node setup on $external_ip..."
    
    # Get variables from terraform
    local gravity_bench_repo=$(terraform output -raw gravity_bench_repo 2>/dev/null || echo "https://github.com/Galxe/gravity_bench.git")
    local gravity_bench_branch=$(terraform output -raw gravity_bench_branch 2>/dev/null || echo "main")
    local http_port=$(terraform output -raw http_port 2>/dev/null || echo "8545")
    
    # Get execution node internal IP
    local execution_output=$(terraform output -json execution_node_info 2>/dev/null || echo "")
    local execution_internal_ip=$(echo "$execution_output" | grep -o '"internal_ip":"[^"]*"' | cut -d'"' -f4)
    
    if [ -z "$execution_internal_ip" ]; then
        log_error "Could not get execution node internal IP"
        exit 1
    fi
    
    # Create temporary script with variables substituted
    local temp_script=$(mktemp)
    sed -e "s|\${gravity_bench_repo}|${gravity_bench_repo}|g" \
        -e "s|\${gravity_bench_branch}|${gravity_bench_branch}|g" \
        -e "s|\${execution_node_internal_ip}|${execution_internal_ip}|g" \
        -e "s|\${http_port}|${http_port}|g" \
        "${SCRIPT_DIR}/scripts/client-node-setup.sh" > "$temp_script"
    
    # Copy script
    scp -i "$SSH_KEY" \
        -o StrictHostKeyChecking=no \
        "$temp_script" \
        "${SSH_USER}@${external_ip}:/tmp/setup-client-node.sh"
    
    # Execute script
    ssh -i "$SSH_KEY" \
        -o StrictHostKeyChecking=no \
        "${SSH_USER}@${external_ip}" \
        "chmod +x /tmp/setup-client-node.sh && sudo mv /tmp/setup-client-node.sh /opt/setup-client-node.sh && sudo /opt/setup-client-node.sh"
    
    # Cleanup
    rm -f "$temp_script"
    
    log_success "Client node setup completed!"
}

# Show help
show_help() {
    cat << EOF
Usage: $0 [COMMAND]

Commands:
  execution    Run execution node setup script
  client       Run client node setup script
  both         Run both setup scripts
  help         Show this help message

Examples:
  $0 execution              # Re-run execution node setup
  $0 client                # Re-run client node setup
  $0 both                  # Re-run both setups

Note: Scripts are copied to /opt/ on the remote nodes and can be re-run manually:
  ssh to node and run: sudo /opt/setup-execution-node.sh
  ssh to node and run: sudo /opt/setup-client-node.sh

EOF
}

# Main
case "${1:-help}" in
    execution)
        run_execution_setup
        ;;
    client)
        run_client_setup
        ;;
    both)
        run_execution_setup
        echo
        run_client_setup
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        log_error "Unknown command: $1"
        show_help
        exit 1
        ;;
esac

