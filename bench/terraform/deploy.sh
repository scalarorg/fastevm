#!/bin/bash

# Gravity Reth Benchmark Terraform Deployment Script
# This script automates the entire Terraform workflow

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

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

# Show help
show_help() {
    cat << EOF
Usage: $0 [COMMAND] [OPTIONS]

Commands:
  init              Initialize Terraform (download providers)
  plan              Show execution plan
  apply             Apply the Terraform configuration
  destroy           Destroy all resources
  output            Show Terraform outputs
  deploy            Full deployment (init + plan + apply)
  execution         Deploy execution node only
  client            Deploy client node only
  status            Show current status
  ssh-execution     SSH to execution node
  ssh-client        SSH to client node
  logs-execution    Show execution node logs (last 100 lines)
  logs-client       Show client node logs (last 100 lines)
  rerun-execution   Re-run execution node setup and restart (cleanup + start)
  rerun-client      Re-run client node setup script
  all               Run: init, plan, apply, output (default)

Note: For full debugging capabilities, use ./debug.sh instead

Options:
  -h, --help        Show this help message
  -y, --yes         Auto-approve apply/destroy operations
  -v, --verbose     Verbose output

Examples:
  $0                    # Full deployment (init + plan + apply + output)
  $0 init               # Initialize only
  $0 plan               # Show plan only
  $0 apply              # Apply changes
  $0 apply --yes         # Apply without confirmation
  $0 destroy            # Destroy all resources
  $0 output             # Show outputs
  $0 execution         # Deploy execution node only
  $0 client            # Deploy client node only
  $0 ssh-execution      # SSH to execution node
  $0 ssh-client         # SSH to client node
  $0 rerun-execution    # Re-run execution node setup and restart (cleanup + start)
  $0 rerun-client       # Re-run client node setup script

EOF
}

# Check if terraform.tfvars exists
check_tfvars() {
    if [ ! -f "terraform.tfvars" ]; then
        log_warning "terraform.tfvars not found!"
        log_info "Creating terraform.tfvars from example..."
        if [ -f "terraform.tfvars.example" ]; then
            cp terraform.tfvars.example terraform.tfvars
            log_warning "Please edit terraform.tfvars with your GCP project ID before continuing!"
            log_info "Edit: terraform.tfvars"
            exit 1
        else
            log_error "terraform.tfvars.example not found!"
            exit 1
        fi
    fi
}

# Initialize Terraform
cmd_init() {
    log_info "Initializing Terraform..."
    terraform init
    log_success "Terraform initialized!"
}

# Plan Terraform
cmd_plan() {
    # Ensure terraform is initialized
    if [ ! -d ".terraform" ]; then
        log_info "Terraform not initialized, running init..."
        terraform init
    fi
    
    log_info "Creating Terraform plan..."
    terraform plan -out=tfplan
    log_success "Plan created! Review the plan above."
}

# Apply Terraform
cmd_apply() {
    local auto_approve="$1"
    
    # Ensure terraform is initialized
    if [ ! -d ".terraform" ]; then
        log_info "Terraform not initialized, running init..."
        terraform init
    fi
    
    # Check if execution node already exists
    local execution_exists=false
    local output=$(terraform output -json execution_node_info 2>/dev/null || echo "")
    if [ -n "$output" ] && [ "$output" != "null" ]; then
        execution_exists=true
    fi
    
    if [ "$auto_approve" = "true" ]; then
        log_info "Applying Terraform configuration (auto-approve)..."
        terraform apply -auto-approve tfplan 2>/dev/null || terraform apply -auto-approve
    else
        log_info "Applying Terraform configuration..."
        if [ -f "tfplan" ]; then
            terraform apply tfplan
        else
            terraform apply
        fi
    fi
    log_success "Terraform configuration applied!"
    
    # After apply, check if execution node exists and run steps 2-4
    local output_after=$(terraform output -json execution_node_info 2>/dev/null || echo "")
    if [ -n "$output_after" ] && [ "$output_after" != "null" ]; then
        log_info "Execution node exists, running steps 2-4 (copy scripts, setup, start)..."
        # Wait a bit for SSH to be ready if it's a new node
        if [ "$execution_exists" = "false" ]; then
            log_info "Waiting for SSH to be ready..."
            sleep 5
        fi
        cmd_restart_execution_internal
    fi
}

# Destroy resources
cmd_destroy() {
    local auto_approve="${1:-true}"  # Default to auto-approve
    
    # Ensure terraform is initialized
    if [ ! -d ".terraform" ]; then
        log_info "Terraform not initialized, running init..."
        terraform init
    fi
    
    log_warning "This will destroy all resources!"
    log_info "Destroying resources (auto-approve)..."
    terraform destroy -auto-approve
    log_success "Resources destroyed!"
}


# Show outputs
cmd_output() {
    log_info "Terraform outputs:"
    echo
    terraform output
}

# Full deployment
cmd_deploy() {
    local auto_approve="$1"
    
    log_info "Starting full deployment..."
    cmd_init
    cmd_plan
    cmd_apply "$auto_approve"
    cmd_output
    log_success "Deployment completed!"
}

# Clean old data on execution node
cmd_clean_execution_data() {
    log_info "Cleaning old execution node data..."
    
    local ssh_key="gravity-deploy-key"
    local output=$(terraform output -json execution_node_info 2>/dev/null || echo "")
    
    if [ -z "$output" ] || [ "$output" = "null" ]; then
        log_warning "Execution node not found in Terraform state. Skipping data cleanup."
        return 0
    fi
    
    local external_ip=$(echo "$output" | grep -o '"external_ip":"[^"]*"' | cut -d'"' -f4)
    local ssh_user="ubuntu"
    
    if [ -z "$external_ip" ]; then
        log_warning "Could not extract execution node IP. Skipping data cleanup."
        return 0
    fi
    
    if [ ! -f "$ssh_key" ]; then
        log_warning "SSH key not found: $ssh_key. Skipping data cleanup."
        return 0
    fi
    
    log_info "Connecting to execution node at $external_ip to clean old data..."
    
    # Clean old data directories and processes
    if ssh -i "$ssh_key" -o StrictHostKeyChecking=no "$ssh_user@$external_ip" bash << 'EOF'
        echo "[INFO] Stopping any running reth processes..."
        pkill -f "reth.*node" 2>/dev/null || true
        pkill -f "dev-node.sh" 2>/dev/null || true
        sleep 2
        
        echo "[INFO] Cleaning old data directories..."
        # Clean gravity-reth data
        if [ -d "/opt/gravity-reth" ]; then
            rm -rf /opt/gravity-reth/bench/.dev-node-data 2>/dev/null || true
            rm -rf /opt/gravity-reth/bench/.dev-node-logs 2>/dev/null || true
            rm -rf /opt/gravity-reth/bench/.dev-node-pids 2>/dev/null || true
            echo "[INFO] Cleaned /opt/gravity-reth/bench data directories"
        fi
        
        # Clean build artifacts (optional - comment out if you want to keep builds)
        # if [ -d "/opt/gravity-reth/target" ]; then
        #     rm -rf /opt/gravity-reth/target 2>/dev/null || true
        #     echo "[INFO] Cleaned build artifacts"
        # fi
        
        # Clean old logs
        rm -f /var/log/gravity-reth-startup.log 2>/dev/null || true
        rm -f /var/log/cargo-build.log 2>/dev/null || true
        rm -f /var/log/execution-node-setup.log 2>/dev/null || true
        rm -f /var/log/execution-node-setup-complete 2>/dev/null || true
        echo "[INFO] Cleaned log files"
        
        # Clean any stale lock files
        find /opt/gravity-reth -name "*.lock" -type f -delete 2>/dev/null || true
        find /opt/gravity-reth -name "*.pid" -type f -delete 2>/dev/null || true
        echo "[INFO] Cleaned lock and PID files"
        
        echo "[SUCCESS] Old data cleaned successfully"
EOF
    then
        log_success "Old execution node data cleaned!"
    else
        log_warning "Failed to clean old data (node may not be accessible yet or already clean)"
    fi
}

# Deploy execution node only
cmd_deploy_execution() {
    local auto_approve="$1"
    
    log_info "Starting execution node deployment..."
    
    # Clean old data before deploying
    cmd_clean_execution_data
    
    cmd_init
    
    # Step 1: Create infrastructure with Terraform only
    log_info "Step 1: Creating execution node infrastructure with Terraform..."
    
    # Resources needed for execution node (infrastructure only, no scripts)
    local targets=(
        "tls_private_key.gravity_ssh"
        "local_file.gravity_private_key"
        "local_file.gravity_public_key"
        "google_compute_network.gravity_network"
        "google_compute_subnetwork.gravity_subnet"
        "google_compute_firewall.gravity_internal"
        "google_compute_firewall.gravity_external"
        "google_service_account.gravity_sa"
        "google_project_iam_binding.gravity_sa_binding"
        "google_compute_instance.execution_node"
        "null_resource.wait_for_execution_ssh"
    )
    
    # Build target flags
    local target_flags=""
    for target in "${targets[@]}"; do
        target_flags="$target_flags -target=$target"
    done
    
    log_info "Creating plan for execution node infrastructure..."
    terraform plan $target_flags -out=tfplan-execution
    
    if [ "$auto_approve" = "true" ]; then
        log_info "Applying execution node infrastructure (auto-approve)..."
        terraform apply -auto-approve tfplan-execution
    else
        log_info "Applying execution node infrastructure..."
        terraform apply tfplan-execution
    fi
    
    log_success "Step 1 completed: Infrastructure created!"
    
    # Wait a bit for SSH to be fully ready
    log_info "Waiting for SSH to be ready..."
    sleep 5
    
    # Steps 2-4: Copy scripts, execute setup, start dev node
    if ! cmd_run_execution_steps; then
        log_error "Failed to complete execution node setup"
        exit 1
    fi
    
    log_success "Execution node deployment completed!"
    log_info "Execution node information:"
    terraform output -json execution_node_info 2>/dev/null || echo "  Run 'terraform output' to see details"
}

# Deploy client node only
cmd_deploy_client() {
    local auto_approve="$1"
    
    log_info "Starting client node deployment..."
    log_warning "Note: Execution node must be deployed first for client node to work properly"
    
    cmd_init
    
    # Resources needed for client node (includes shared resources)
    # Note: execution node script resources removed - handled by deploy.sh now
    local targets=(
        "tls_private_key.gravity_ssh"
        "local_file.gravity_private_key"
        "local_file.gravity_public_key"
        "google_compute_network.gravity_network"
        "google_compute_subnetwork.gravity_subnet"
        "google_compute_firewall.gravity_internal"
        "google_compute_firewall.gravity_external"
        "google_service_account.gravity_sa"
        "google_project_iam_binding.gravity_sa_binding"
        "google_compute_instance.execution_node"
        "null_resource.wait_for_execution_ssh"
        "google_compute_instance.client_node"
        "null_resource.wait_for_client_ssh"
        "null_resource.copy_client_setup_script"
        "null_resource.execute_client_setup"
    )
    
    # Build target flags
    local target_flags=""
    for target in "${targets[@]}"; do
        target_flags="$target_flags -target=$target"
    done
    
    log_info "Creating plan for client node..."
    terraform plan $target_flags -out=tfplan-client
    
    if [ "$auto_approve" = "true" ]; then
        log_info "Applying client node configuration (auto-approve)..."
        terraform apply -auto-approve tfplan-client
    else
        log_info "Applying client node configuration..."
        terraform apply tfplan-client
    fi
    
    log_success "Client node deployment completed!"
    log_info "Client node information:"
    terraform output -json client_node_info 2>/dev/null || echo "  Run 'terraform output' to see details"
}

# Show status
cmd_status() {
    log_info "Checking Terraform status..."
    terraform show 2>/dev/null || {
        log_warning "No Terraform state found. Run 'terraform apply' first."
        return 1
    }
}

# SSH to execution node
cmd_ssh_execution() {
    local ssh_key="gravity-deploy-key"
    
    # Try to get SSH command from terraform output
    local ssh_cmd=$(terraform output -raw ssh_commands 2>/dev/null | grep execution_node | sed 's/.*execution_node = "\(.*\)"/\1/' || echo "")
    
    if [ -n "$ssh_cmd" ]; then
        log_info "Connecting to execution node..."
        eval "$ssh_cmd"
    else
        # Fallback: try to extract from JSON output
        local output=$(terraform output -json execution_node_info 2>/dev/null || echo "")
        
        if [ -z "$output" ] || [ "$output" = "null" ]; then
            log_error "Execution node not found in Terraform state. Run 'terraform apply' first."
            exit 1
        fi
        
        local external_ip=$(echo "$output" | grep -o '"external_ip":"[^"]*"' | cut -d'"' -f4)
        local ssh_user="ubuntu"
        
        if [ -z "$external_ip" ]; then
            log_error "Could not extract execution node IP"
            exit 1
        fi
        
        if [ ! -f "$ssh_key" ]; then
            log_error "SSH key not found: $ssh_key"
            exit 1
        fi
        
        log_info "Connecting to execution node at $external_ip..."
        ssh -i "$ssh_key" -o StrictHostKeyChecking=no "$ssh_user@$external_ip"
    fi
}

# SSH to client node
cmd_ssh_client() {
    local ssh_key="gravity-deploy-key"
    
    # Try to get SSH command from terraform output
    local ssh_cmd=$(terraform output -raw ssh_commands 2>/dev/null | grep client_node | sed 's/.*client_node = "\(.*\)"/\1/' || echo "")
    
    if [ -n "$ssh_cmd" ]; then
        log_info "Connecting to client node..."
        eval "$ssh_cmd"
    else
        # Fallback: try to extract from JSON output
        local output=$(terraform output -json client_node_info 2>/dev/null || echo "")
        
        if [ -z "$output" ] || [ "$output" = "null" ]; then
            log_error "Client node not found in Terraform state. Run 'terraform apply' first."
            exit 1
        fi
        
        local external_ip=$(echo "$output" | grep -o '"external_ip":"[^"]*"' | cut -d'"' -f4)
        local ssh_user="ubuntu"
        
        if [ -z "$external_ip" ]; then
            log_error "Could not extract client node IP"
            exit 1
        fi
        
        if [ ! -f "$ssh_key" ]; then
            log_error "SSH key not found: $ssh_key"
            exit 1
        fi
        
        log_info "Connecting to client node at $external_ip..."
        ssh -i "$ssh_key" -o StrictHostKeyChecking=no "$ssh_user@$external_ip"
    fi
}

# Re-run execution setup and restart (starts from step 2)
cmd_rerun_execution() {
    log_info "Re-running execution node setup (steps 2-4)..."
    cmd_run_execution_steps || exit 1
    log_success "Execution node rerun completed successfully!"
}

# Re-run client setup
cmd_rerun_client() {
    log_info "Re-running client node setup..."
    if [ -f "run-setup.sh" ]; then
        bash run-setup.sh client
    else
        log_error "run-setup.sh not found"
        exit 1
    fi
}

# Get execution node connection info (returns via global variables)
get_execution_node_info() {
    local ssh_key="gravity-deploy-key"
    local output=$(terraform output -json execution_node_info 2>/dev/null || echo "")
    
    [ -z "$output" ] || [ "$output" = "null" ] && { log_error "Execution node not found. Run 'terraform apply' first."; return 1; }
    
    EXECUTION_NODE_IP=$(echo "$output" | grep -o '"external_ip":"[^"]*"' | cut -d'"' -f4)
    EXECUTION_NODE_USER="ubuntu"
    EXECUTION_NODE_SSH_KEY="$ssh_key"
    
    [ -z "$EXECUTION_NODE_IP" ] && { log_error "Could not extract execution node IP"; return 1; }
    [ ! -f "$ssh_key" ] && { log_error "SSH key not found: $ssh_key"; return 1; }
    
    return 0
}

# Helper: Execute SSH command on execution node
ssh_execution() {
    local cmd="$1"
    get_execution_node_info || return 1
    ssh -i "$EXECUTION_NODE_SSH_KEY" -o StrictHostKeyChecking=no "$EXECUTION_NODE_USER@$EXECUTION_NODE_IP" "$cmd"
}

# Helper: Copy file to execution node
scp_execution() {
    local local_file="$1"
    local remote_path="$2"
    get_execution_node_info || return 1
    scp -i "$EXECUTION_NODE_SSH_KEY" -o StrictHostKeyChecking=no "$local_file" "$EXECUTION_NODE_USER@$EXECUTION_NODE_IP:$remote_path" >/dev/null 2>&1
}

# Run steps 2-4: Copy scripts, execute setup, start dev node
cmd_run_execution_steps() {
    log_info "Running execution node steps 2-4..."
    
    # Step 2: Copy scripts
    log_info "Step 2: Copying scripts to remote node..."
    get_execution_node_info || return 1
    
    local dev_node_script="$SCRIPT_DIR/scripts/dev-node.sh"
    local execution_setup_script="$SCRIPT_DIR/scripts/execution-node-setup.sh"
    
    [ ! -f "$dev_node_script" ] && { log_error "dev-node.sh not found"; return 1; }
    [ ! -f "$execution_setup_script" ] && { log_error "execution-node-setup.sh not found"; return 1; }
    
    scp_execution "$dev_node_script" "/tmp/dev-node.sh" || { log_error "Failed to copy dev-node.sh"; return 1; }
    scp_execution "$execution_setup_script" "/tmp/setup-execution-node.sh" || { log_error "Failed to copy execution-node-setup.sh"; return 1; }
    
    # Install scripts step by step
    ssh_execution "mkdir -p /opt/gravity-reth/bench" || { log_error "Failed to create bench directory"; return 1; }
    ssh_execution "cp /tmp/dev-node.sh /opt/gravity-reth/bench/dev-node.sh" || { log_error "Failed to copy dev-node.sh"; return 1; }
    ssh_execution "chmod +x /opt/gravity-reth/bench/dev-node.sh /tmp/setup-execution-node.sh" || { log_error "Failed to set permissions"; return 1; }
    ssh_execution "sudo mv /tmp/setup-execution-node.sh /opt/setup-execution-node.sh" || { log_error "Failed to move setup script"; return 1; }
    log_success "Scripts copied"
    
    # Step 3: Execute setup
    log_info "Step 3: Executing execution-node-setup.sh..."
    ssh_execution "sudo bash /opt/setup-execution-node.sh 2>&1 | sudo tee -a /var/log/execution-node-setup.log" || { log_error "Setup script failed. Check /var/log/execution-node-setup.log"; return 1; }
    ssh_execution "test -f /var/log/execution-node-setup-complete" || { log_error "Setup did not complete successfully"; return 1; }
    log_success "Setup completed"
    
    # Step 4: Start dev node
    log_info "Step 4: Starting dev node..."
    ssh_execution "bash -c 'cd /opt/gravity-reth && bash bench/dev-node.sh start'" || { log_error "Failed to start dev node"; return 1; }
    log_success "Dev node started"
    
    return 0
}

# Internal function to restart execution node (used by cmd_apply)
cmd_restart_execution_internal() {
    cmd_run_execution_steps
}


# Show execution node logs
cmd_logs_execution() {
    local ssh_key="gravity-deploy-key"
    local output=$(terraform output -json execution_node_info 2>/dev/null || echo "")
    
    if [ -z "$output" ] || [ "$output" = "null" ]; then
        log_error "Execution node not found in Terraform state. Run 'terraform apply' first."
        exit 1
    fi
    
    local external_ip=$(echo "$output" | grep -o '"external_ip":"[^"]*"' | cut -d'"' -f4)
    local ssh_user="ubuntu"
    
    if [ -z "$external_ip" ]; then
        log_error "Could not extract execution node IP"
        exit 1
    fi
    
    if [ ! -f "$ssh_key" ]; then
        log_error "SSH key not found: $ssh_key"
        exit 1
    fi
    
    log_info "Fetching execution node logs (last 100 lines)..."
    log_warning "For full logs and better debugging, use: ./debug.sh logs-execution"
    echo
    ssh -i "$ssh_key" -o StrictHostKeyChecking=no "$ssh_user@$external_ip" \
        "tail -100 /var/log/execution-node-setup.log 2>/dev/null || echo 'Setup log not found'; echo; tail -100 /var/log/gravity-reth-startup.log 2>/dev/null || echo 'Startup log not found'; echo; tail -100 /opt/gravity-reth/bench/.dev-node-logs/reth-node.log 2>/dev/null || echo 'Reth log not found'"
}

# Show client node logs
cmd_logs_client() {
    local ssh_key="gravity-deploy-key"
    local output=$(terraform output -json client_node_info 2>/dev/null || echo "")
    
    if [ -z "$output" ] || [ "$output" = "null" ]; then
        log_error "Client node not found in Terraform state. Run 'terraform apply' first."
        exit 1
    fi
    
    local external_ip=$(echo "$output" | grep -o '"external_ip":"[^"]*"' | cut -d'"' -f4)
    local ssh_user="ubuntu"
    
    if [ -z "$external_ip" ]; then
        log_error "Could not extract client node IP"
        exit 1
    fi
    
    if [ ! -f "$ssh_key" ]; then
        log_error "SSH key not found: $ssh_key"
        exit 1
    fi
    
    log_info "Fetching client node logs (last 100 lines)..."
    log_warning "For full logs and better debugging, use: ./debug.sh logs-client"
    echo
    ssh -i "$ssh_key" -o StrictHostKeyChecking=no "$ssh_user@$external_ip" \
        "tail -100 /var/log/client-node-setup.log 2>/dev/null || echo 'Setup log not found'; echo; tail -100 /var/log/gravity-bench.log 2>/dev/null || echo 'Bench log not found'"
}

# Parse arguments
AUTO_APPROVE="false"
VERBOSE="false"
COMMAND="all"

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -y|--yes)
            AUTO_APPROVE="true"
            shift
            ;;
        -v|--verbose)
            VERBOSE="true"
            shift
            ;;
        init|plan|apply|destroy|output|deploy|execution|client|status|ssh-execution|ssh-client|logs-execution|logs-client|rerun-execution|rerun-client|all)
            COMMAND="$1"
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Set verbose mode
if [ "$VERBOSE" = "true" ]; then
    set -x
fi

# Check for terraform.tfvars (except for destroy and help)
if [ "$COMMAND" != "destroy" ] && [ "$COMMAND" != "ssh-execution" ] && [ "$COMMAND" != "ssh-client" ] && [ "$COMMAND" != "logs-execution" ] && [ "$COMMAND" != "logs-client" ] && [ "$COMMAND" != "rerun-execution" ] && [ "$COMMAND" != "rerun-client" ] && [ "$COMMAND" != "restart-execution" ]; then
    check_tfvars
fi

# Execute command
case "$COMMAND" in
    init)
        cmd_init
        ;;
    plan)
        cmd_plan
        ;;
    apply)
        cmd_apply "$AUTO_APPROVE"
        ;;
    destroy)
        cmd_destroy "$AUTO_APPROVE"
        ;;
    output)
        cmd_output
        ;;
    deploy)
        cmd_deploy "$AUTO_APPROVE"
        ;;
    execution)
        cmd_deploy_execution "$AUTO_APPROVE"
        ;;
    client)
        cmd_deploy_client "$AUTO_APPROVE"
        ;;
    status)
        cmd_status
        ;;
    ssh-execution)
        cmd_ssh_execution
        ;;
    ssh-client)
        cmd_ssh_client
        ;;
    logs-execution)
        cmd_logs_execution
        ;;
    logs-client)
        cmd_logs_client
        ;;
    rerun-execution|restart-execution)
        cmd_rerun_execution
        ;;
    rerun-client)
        cmd_rerun_client
        ;;
    all|*)
        cmd_deploy "$AUTO_APPROVE"
        ;;
esac

