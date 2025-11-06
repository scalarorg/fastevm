#!/bin/bash
# FastEVM Client Node Configuration Deployment Script

set -e

# Configuration
CLIENT_CONFIG_DIR="/home/ubuntu/client-config"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLIENT_NODE_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$CLIENT_NODE_DIR")"
CONFIG_DIR="${CLIENT_NODE_DIR}/config"
SSH_KEY="${CLIENT_NODE_DIR}/client-deploy-key"
SSH_USER="ubuntu"
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Function to get client IP from Terraform
get_client_ip() {
    log_info "Getting client node IP from Terraform..."
    
    cd "${CLIENT_NODE_DIR}"
    
    if [ ! -f "terraform.tfstate" ]; then
        log_error "No Terraform state found. Run 'make apply' first."
        exit 1
    fi
    
    # Find terraform command with fallback paths
    TERRAFORM_CMD=$(which terraform 2>/dev/null)
    if [ -z "$TERRAFORM_CMD" ]; then
        # Try common installation paths
        for path in "/opt/homebrew/bin/terraform" "/usr/local/bin/terraform" "/usr/bin/terraform"; do
            if [ -x "$path" ]; then
                TERRAFORM_CMD="$path"
                break
            fi
        done
    fi
    
    if [ -z "$TERRAFORM_CMD" ] || [ ! -x "$TERRAFORM_CMD" ]; then
        log_error "Terraform not found. Please install terraform."
        exit 1
    fi
    
    log_info "Using terraform: $TERRAFORM_CMD"
    
    CLIENT_IP=$($TERRAFORM_CMD output -json client_node_info | jq -r '.external_ip')
    
    if [ "$CLIENT_IP" = "null" ] || [ -z "$CLIENT_IP" ]; then
        log_error "Client node not found. Run 'make apply' first."
        exit 1
    fi
    
    log_success "Client IP: $CLIENT_IP"
}

# Function to check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if [ ! -d "$CONFIG_DIR" ]; then
        log_error "Configuration directory not found: $CONFIG_DIR"
        log_error "Run 'make prepare-configs' first."
        exit 1
    fi
    
    if [ ! -f "$SSH_KEY" ]; then
        log_error "SSH key not found: $SSH_KEY"
        log_error "Run 'make apply' first."
        exit 1
    fi
    
    # Check for setup.sh in config directory
    if [ ! -f "${CONFIG_DIR}/setup.sh" ]; then
        log_error "Required configuration file not found: ${CONFIG_DIR}/setup.sh"
        log_error "Run 'make prepare-configs' first."
        exit 1
    fi
    
    # Check for shared fastevm.env in project root
    local shared_env_file="${PROJECT_ROOT}/fastevm.env"
    if [ ! -f "$shared_env_file" ]; then
        log_error "Shared fastevm.env not found: $shared_env_file"
        log_error "Please ensure the main FastEVM deployment is completed first."
        exit 1
    fi
    
    chmod 600 "$SSH_KEY"
    log_success "Prerequisites check passed"
}

# Function to wait for SSH availability
wait_for_ssh() {
    log_info "Waiting for SSH port 22 to be available on $CLIENT_IP..."
    
    local timeout=300
    local elapsed=0
    
    while [ $elapsed -lt $timeout ]; do
        if nc -z "$CLIENT_IP" 22 2>/dev/null; then
            log_success "SSH port 22 is available on $CLIENT_IP"
            return 0
        fi
        
        log_info "Waiting for SSH port 22... ($elapsed/$timeout seconds)"
        sleep 10
        elapsed=$((elapsed + 10))
    done
    
    log_error "Timeout: SSH port 22 not available after $timeout seconds"
    exit 1
}

# Function to manage SSH host keys
manage_ssh_keys() {
    log_info "Managing SSH host keys for $CLIENT_IP..."
    ssh-keygen -R "$CLIENT_IP" 2>/dev/null || true
    ssh-keyscan -H "$CLIENT_IP" >> ~/.ssh/known_hosts 2>/dev/null || true
    log_success "SSH host keys managed"
}

# Function to copy configuration files
copy_config_files() {
    log_info "Copying configuration files to remote client..."
    
    # Create remote config directory
    ssh $SSH_OPTS -i "$SSH_KEY" "$SSH_USER@$CLIENT_IP" "mkdir -p $CLIENT_CONFIG_DIR"
    
    # Copy setup.sh from config directory
    log_info "Copying setup.sh..."
    scp $SSH_OPTS -i "$SSH_KEY" "${CONFIG_DIR}/setup.sh" "$SSH_USER@$CLIENT_IP:$CLIENT_CONFIG_DIR/setup.sh"
    if [ $? -eq 0 ]; then
        log_success "setup.sh copied successfully"
    else
        log_error "Failed to copy setup.sh"
        exit 1
    fi
    
    # Copy shared fastevm.env to client home directory
    log_info "Copying shared fastevm.env..."
    scp $SSH_OPTS -i "$SSH_KEY" "${PROJECT_ROOT}/fastevm.env" "$SSH_USER@$CLIENT_IP:/home/ubuntu/fastevm.env"
    if [ $? -eq 0 ]; then
        log_success "fastevm.env copied successfully"
    else
        log_error "Failed to copy fastevm.env"
        exit 1
    fi
    
    # Set proper permissions
    ssh $SSH_OPTS -i "$SSH_KEY" "$SSH_USER@$CLIENT_IP" "chmod +x $CLIENT_CONFIG_DIR/*.sh"
    
    log_success "All configuration files copied successfully"
}

# Function to verify deployment
verify_deployment() {
    log_info "Verifying configuration deployment..."
    
    # Check for setup.sh in config directory
    local setup_exists=$(ssh $SSH_OPTS -i "$SSH_KEY" "$SSH_USER@$CLIENT_IP" "ls $CLIENT_CONFIG_DIR/setup.sh >/dev/null 2>&1 && echo 'yes' || echo 'no'")
    
    # Check for fastevm.env in home directory
    local env_exists=$(ssh $SSH_OPTS -i "$SSH_KEY" "$SSH_USER@$CLIENT_IP" "ls /home/ubuntu/fastevm.env >/dev/null 2>&1 && echo 'yes' || echo 'no'")
    
    if [ "$setup_exists" = "yes" ] && [ "$env_exists" = "yes" ]; then
        log_success "Configuration files verified on remote client"
    else
        log_error "Configuration files not found on remote client"
        log_error "setup.sh exists: $setup_exists"
        log_error "fastevm.env exists: $env_exists"
        exit 1
    fi
    
    log_info "Remote configuration files:"
    ssh $SSH_OPTS -i "$SSH_KEY" "$SSH_USER@$CLIENT_IP" "ls -la $CLIENT_CONFIG_DIR/ && echo '---' && ls -la /home/ubuntu/fastevm.env"
}

# Main execution
main() {
    log_info "Starting FastEVM client node configuration deployment..."
    
    get_client_ip
    check_prerequisites
    wait_for_ssh
    manage_ssh_keys
    copy_config_files
    verify_deployment
    
    log_success "Client node configuration deployment completed!"
    log_success "Client IP: $CLIENT_IP"
    log_success "Next steps: make setup"
    log_success "SSH access: ssh -i $SSH_KEY $SSH_USER@$CLIENT_IP"
}

main "$@"
