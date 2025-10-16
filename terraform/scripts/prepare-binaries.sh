#!/bin/bash

# FastEVM Binary Preparation Script
# This script ensures binaries are available on both local machine and build node

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
BINARIES_DIR="$PROJECT_ROOT/binaries"
SSH_KEY_PATH="$PROJECT_ROOT/fastevm-deploy-key"
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

# Required binaries
REQUIRED_BINARIES=("fastevm-execution" "fastevm-consensus" "cli")

log_info "FastEVM Binary Preparation Script"
log_info "================================="

# Create binaries directory if it doesn't exist
mkdir -p "$BINARIES_DIR"

# Check if SSH key exists
if [ ! -f "$SSH_KEY_PATH" ]; then
    log_error "SSH key not found at $SSH_KEY_PATH"
    exit 1
fi

chmod 600 "$SSH_KEY_PATH"

# Get build node IP
get_build_node_ip() {
    # Try to get from deployment info first
    if [ -f "$DEPLOYMENT_INFO_PATH" ]; then
        local node_ips=$(jq -r '.instance_external_ips.value[]' "$DEPLOYMENT_INFO_PATH" 2>/dev/null || jq -r '.instance_ips.value[]' "$DEPLOYMENT_INFO_PATH" 2>/dev/null)
        if [ -n "$node_ips" ]; then
            echo "$node_ips" | head -n 1
            return 0
        fi
    fi
    
    # Try terraform output
    local node_ips=$(cd "$PROJECT_ROOT" && terraform output -json instance_external_ips 2>/dev/null | jq -r '.value[]' 2>/dev/null || terraform output -json instance_ips 2>/dev/null | jq -r '.value[]' 2>/dev/null || echo "")
    if [ -n "$node_ips" ]; then
        echo "$node_ips" | head -n 1
        return 0
    fi
    
    log_error "Could not determine build node IP"
    return 1
}

# Check if all required binaries exist locally
check_local_binaries() {
    for binary in "${REQUIRED_BINARIES[@]}"; do
        if [ ! -f "$BINARIES_DIR/$binary" ]; then
            return 1
        fi
    done
    return 0
}

# Copy binaries from local to build node
restore_binaries() {
    local build_node_ip="$1"
    log_info "Copying binaries from local storage to build node ($build_node_ip)..."
    
    # Ensure directory exists on build node
    ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$build_node_ip "sudo mkdir -p /opt/fastevm-binaries && sudo chown -R ubuntu:ubuntu /opt/fastevm-binaries"
    
    # Copy each binary
    for binary in "${REQUIRED_BINARIES[@]}"; do
        log_info "Copying $binary to build node..."
        scp $SSH_OPTS -i "$SSH_KEY_PATH" "$BINARIES_DIR/$binary" ubuntu@$build_node_ip:/opt/fastevm-binaries/
        ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$build_node_ip "chmod +x /opt/fastevm-binaries/$binary"
    done
    
    log_success "All binaries copied to build node"
}

# Copy binaries from build node to local
backup_binaries() {
    local build_node_ip="$1"
    log_info "Copying binaries from build node ($build_node_ip) to local storage..."
    
    # Copy each binary
    for binary in "${REQUIRED_BINARIES[@]}"; do
        log_info "Copying $binary from build node..."
        scp $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$build_node_ip:/opt/fastevm-binaries/$binary "$BINARIES_DIR/"
        chmod +x "$BINARIES_DIR/$binary"
    done
    
    log_success "All binaries copied to local storage"
}

# Build binaries on build node
build_binaries() {
    local build_node_ip="$1"
    log_info "Building binaries on build node ($build_node_ip)..."
    
    # Get GitHub configuration from environment variables
    local github_repo="${GITHUB_REPO:-https://github.com/scalarorg/fastevm.git}"
    local github_branch="${GITHUB_BRANCH:-main}"
    
    log_info "Using GitHub repo: $github_repo"
    log_info "Using GitHub branch: $github_branch"
    
    # Run the build process on the build node
    ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$build_node_ip "
        # Install Rust if not already installed
        if ! command -v cargo >/dev/null 2>&1; then
            log_info 'Installing Rust...'
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            export PATH=\"\$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin\"
            source \$HOME/.cargo/env
            rustup default stable
            rustup update
        else
            log_info 'Rust already installed, updating...'
            export PATH=\"\$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin\"
            rustup update
        fi
        
        # Create FastEVM directory if it doesn't exist
        if [ ! -d '/home/ubuntu/fastevm' ]; then
            log_info 'Creating FastEVM directory...'
            mkdir -p /home/ubuntu/fastevm
        fi
        
        cd /home/ubuntu/fastevm
        
        # Clone or update the repository
        if [ -d '.git' ]; then
            log_info 'Repository already exists, pulling latest changes...'
            git fetch origin
            git checkout $github_branch
            git pull origin $github_branch
        else
            log_info 'Cloning FastEVM repository...'
            git clone $github_repo .
            git checkout $github_branch
        fi
        
        # Build the project
        log_info 'Building FastEVM...'
        export PATH=\"\$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin\"
        cargo build --release
        
        # Create binaries directory and copy binaries
        sudo mkdir -p /opt/fastevm-binaries
        sudo cp target/release/fastevm-execution /opt/fastevm-binaries/
        sudo cp target/release/fastevm-consensus /opt/fastevm-binaries/
        sudo cp target/release/cli /opt/fastevm-binaries/
        sudo chmod +x /opt/fastevm-binaries/*
        sudo chown -R ubuntu:ubuntu /opt/fastevm-binaries
        
        log_success 'Build completed successfully!'
    "
    
    log_success "Binaries built on build node"
}

# Get all node IPs
get_all_node_ips() {
    # Try to get from deployment info first
    if [ -f "$DEPLOYMENT_INFO_PATH" ]; then
        local node_ips=$(jq -r '.instance_external_ips.value[]' "$DEPLOYMENT_INFO_PATH" 2>/dev/null || jq -r '.instance_ips.value[]' "$DEPLOYMENT_INFO_PATH" 2>/dev/null)
        if [ -n "$node_ips" ]; then
            echo "$node_ips"
            return 0
        fi
    fi
    
    # Try terraform output
    local node_ips=$(cd "$PROJECT_ROOT" && terraform output -json instance_external_ips 2>/dev/null | jq -r '.value[]' 2>/dev/null || terraform output -json instance_ips 2>/dev/null | jq -r '.value[]' 2>/dev/null || echo "")
    if [ -n "$node_ips" ]; then
        echo "$node_ips"
        return 0
    fi
    
    log_error "Could not determine node IPs"
    return 1
}

# Distribute binaries from build node to all other nodes
distribute_binaries() {
    local build_node_ip="$1"
    local all_nodes="$2"
    
    log_info "Distributing binaries from build node to all other nodes..."
    
    # First, install binaries on build node itself
    log_info "Installing binaries on build node ($build_node_ip)..."
    ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$build_node_ip "
        # Copy to system location
        sudo cp /opt/fastevm-binaries/fastevm-execution /usr/local/bin/
        sudo cp /opt/fastevm-binaries/fastevm-consensus /usr/local/bin/
        sudo cp /opt/fastevm-binaries/cli /usr/local/bin/ 2>/dev/null || true
        
        # Set executable permissions
        sudo chmod +x /usr/local/bin/fastevm-execution
        sudo chmod +x /usr/local/bin/fastevm-consensus
        sudo chmod +x /usr/local/bin/cli 2>/dev/null || true
    "
    log_success "Binaries installed on build node"
    
    # Get other nodes (excluding build node)
    local other_nodes=$(echo "$all_nodes" | tail -n +2)
    
    if [ -z "$other_nodes" ]; then
        log_info "No other nodes to distribute to (only build node exists)"
        return 0
    fi
    
    # Distribute to other nodes
    for node_ip in $other_nodes; do
        log_info "Distributing binaries from $build_node_ip to node $node_ip..."
        
        # Ensure directory exists and has correct permissions on target node
        ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$node_ip "sudo mkdir -p /opt/fastevm-binaries && sudo chown -R ubuntu:ubuntu /opt/fastevm-binaries && sudo chmod -R 755 /opt/fastevm-binaries"
        
        # Copy binaries directly from build node to target node
        ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$build_node_ip "scp $SSH_OPTS -i ~/.ssh/fastevm-deploy-key /opt/fastevm-binaries/fastevm-execution /opt/fastevm-binaries/fastevm-consensus ubuntu@$node_ip:/opt/fastevm-binaries/ && if [ -f /opt/fastevm-binaries/cli ]; then scp $SSH_OPTS -i ~/.ssh/fastevm-deploy-key /opt/fastevm-binaries/cli ubuntu@$node_ip:/opt/fastevm-binaries/; fi"
        
        # Copy binaries to system location and set permissions
        ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$node_ip "
            # Copy to system location
            sudo cp /opt/fastevm-binaries/fastevm-execution /usr/local/bin/
            sudo cp /opt/fastevm-binaries/fastevm-consensus /usr/local/bin/
            sudo cp /opt/fastevm-binaries/cli /usr/local/bin/ 2>/dev/null || true
            
            # Set executable permissions
            sudo chmod +x /usr/local/bin/fastevm-execution
            sudo chmod +x /usr/local/bin/fastevm-consensus
            sudo chmod +x /usr/local/bin/cli 2>/dev/null || true
            chmod +x /opt/fastevm-binaries/*
        "
        
        log_success "Binaries distributed from $build_node_ip to node $node_ip"
    done
}

# Verify binary distribution on all nodes
verify_distribution() {
    local all_nodes="$1"
    
    log_info "Verifying binary distribution..."
    for node_ip in $all_nodes; do
        log_info "Verifying binaries on node $node_ip..."
        if ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$node_ip "ls -la /usr/local/bin/fastevm-execution /usr/local/bin/fastevm-consensus /usr/local/bin/cli" >/dev/null 2>&1; then
            log_success "Binaries verified on node $node_ip"
        else
            log_error "Binaries verification failed on node $node_ip"
            return 1
        fi
    done
}


# Main workflow
main() {
    # Get build node IP
    local build_node_ip
    if ! build_node_ip=$(get_build_node_ip); then
        log_error "Could not determine build node IP"
        exit 1
    fi
    
    # Get all node IPs
    local all_nodes
    if ! all_nodes=$(get_all_node_ips); then
        log_error "Could not determine node IPs"
        exit 1
    fi
    
    local node_count=$(echo "$all_nodes" | wc -l | tr -d ' ')
    log_info "Found $node_count nodes"
    log_info "Build node IP: $build_node_ip"
    
    # Check if we have local binaries
    if check_local_binaries; then
        log_success "Local binaries found, copying to build node..."
        restore_binaries "$build_node_ip"
    else
        log_warning "Local binaries missing, building new ones on build node..."
        build_binaries "$build_node_ip"
        backup_binaries "$build_node_ip"
    fi
    
    # Distribute binaries to all other nodes
    distribute_binaries "$build_node_ip" "$all_nodes"
    
    # Verify distribution
    verify_distribution "$all_nodes"
    
    log_success "Binary preparation and distribution completed successfully!"
    log_info "All nodes now have the required binaries"
}

# Run main function if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi