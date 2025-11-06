#!/bin/bash
# FastEVM Multi-Node Deployment Script
# This script deploys all prepared node configurations to remote servers

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DEPLOY_DIR="${PROJECT_ROOT}/deploy"
CONFIG_DIR="${PROJECT_ROOT}/config"

# Colors for output
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

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Help function
show_help() {
    cat << EOF
FastEVM Multi-Node Deployment Script

Usage: $0 [OPTIONS]

This script deploys all prepared node configurations to remote servers.

OPTIONS:
    -c, --config-dir DIR     Configuration directory (default: ../config)
    -d, --deploy-dir DIR     Deploy directory (default: ../deploy)
    -k, --ssh-key PATH       SSH key path (default: ../fastevm-deploy-key)
    -u, --user USER          SSH user (default: ubuntu)
    --parallel               Deploy nodes in parallel (default: sequential)
    -h, --help               Show this help message

EXAMPLES:
    $0                                    # Deploy with default settings
    $0 --parallel                         # Deploy all nodes in parallel
    $0 -k /path/to/key -u root            # Use custom SSH key and user
    $0 -c /custom/config -d /custom/deploy # Use custom directories

EOF
}

# Default values
SSH_KEY_PATH="../fastevm-deploy-key"
SSH_USER="ubuntu"
PARALLEL_DEPLOY=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -c|--config-dir)
            CONFIG_DIR="$2"
            shift 2
            ;;
        -d|--deploy-dir)
            DEPLOY_DIR="$2"
            shift 2
            ;;
        -k|--ssh-key)
            SSH_KEY_PATH="$2"
            shift 2
            ;;
        -u|--user)
            SSH_USER="$2"
            shift 2
            ;;
        --parallel)
            PARALLEL_DEPLOY=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# SSH options
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

# Check if SSH key exists
check_ssh_key() {
    if [ ! -f "$SSH_KEY_PATH" ]; then
        log_error "SSH key not found at $SSH_KEY_PATH"
        log_error "Please run 'terraform apply' first to generate SSH keys"
        exit 1
    fi
    
    # Set proper permissions
    chmod 600 "$SSH_KEY_PATH"
    log_info "Using SSH key: $SSH_KEY_PATH"
}

# Function to deploy a single node
deploy_node() {
    local node_index=$1
    local node_dir="$DEPLOY_DIR/node$node_index"
    
    # Check if node directory exists
    if [ ! -d "$node_dir" ]; then
        log_error "Node $node_index deployment package not found at $node_dir"
        log_error "Please run prepare-configs.sh first"
        return 1
    fi
    
    # Load node configuration
    if [ ! -f "$node_dir/node.env" ]; then
        log_error "Node $node_index environment file not found"
        return 1
    fi
    
    source "$node_dir/node.env"
    
    if [ -z "$NODE_IP" ]; then
        log_error "NODE_IP not found in node $node_index configuration"
        return 1
    fi
    
    log_info "Deploying node $node_index ($NODE_IP)..."
    
    # Copy configuration files to node
    scp $SSH_OPTS -i "$SSH_KEY_PATH" -r "$node_dir"/* "$SSH_USER@$NODE_IP:/tmp/fastevm-config/"
    
    # Run deployment script on node
    ssh $SSH_OPTS -i "$SSH_KEY_PATH" "$SSH_USER@$NODE_IP" "bash /tmp/fastevm-config/deploy.sh"
    
    log_success "Node $node_index deployment completed"
}

# Function to test node connectivity
test_node() {
    local node_index=$1
    local node_dir="$DEPLOY_DIR/node$node_index"
    
    source "$node_dir/node.env"
    
    if [ -z "$NODE_IP" ]; then
        log_error "NODE_IP not found in node $node_index configuration"
        return 1
    fi
    
    local http_port=${HTTP_PORT:-8545}
    
    log_info "Testing node $node_index ($NODE_IP:$http_port)..."
    if curl -s -f "http://$NODE_IP:$http_port" > /dev/null; then
        log_success "Node $node_index is responding"
        return 0
    else
        log_error "Node $node_index is not responding"
        return 1
    fi
}

# Main execution
main() {
    log_info "Starting FastEVM multi-node deployment..."
    
    # Check prerequisites
    check_ssh_key
    
    # Check if deploy directory exists
    if [ ! -d "$DEPLOY_DIR" ]; then
        log_error "Deploy directory not found at $DEPLOY_DIR"
        log_error "Please run prepare-configs.sh first"
        exit 1
    fi
    
    # Count available nodes
    NODE_COUNT=$(ls -1 "$DEPLOY_DIR"/node* 2>/dev/null | wc -l)
    if [ "$NODE_COUNT" -eq 0 ]; then
        log_error "No node deployment packages found in $DEPLOY_DIR"
        log_error "Please run prepare-configs.sh first"
        exit 1
    fi
    
    log_info "Found $NODE_COUNT node deployment packages"
    
    # Deploy nodes
    log_info "Starting deployment of $NODE_COUNT nodes..."
    
    if [ "$PARALLEL_DEPLOY" = "true" ]; then
        log_info "Deploying nodes in parallel..."
        for i in $(seq 0 $((NODE_COUNT - 1))); do
            deploy_node $i &
        done
        
        # Wait for all deployments to complete
        wait
    else
        log_info "Deploying nodes sequentially..."
        for i in $(seq 0 $((NODE_COUNT - 1))); do
            deploy_node $i
        done
    fi
    
    log_success "All nodes deployed successfully!"
    
    # Test connectivity
    log_info "Testing node connectivity..."
    FAILED_NODES=0
    for i in $(seq 0 $((NODE_COUNT - 1))); do
        if ! test_node $i; then
            FAILED_NODES=$((FAILED_NODES + 1))
        fi
    done
    
    if [ $FAILED_NODES -eq 0 ]; then
        log_success "All nodes are responding correctly!"
    else
        log_warning "$FAILED_NODES nodes failed connectivity tests"
    fi
    
    log_success "Deployment completed!"
}

# Run main function
main "$@"
