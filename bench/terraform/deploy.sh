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

# Load .env file if it exists
load_env_file() {
    local env_file="$SCRIPT_DIR/.env"
    if [ -f "$env_file" ]; then
        log_info "Loading environment variables from .env file..."
        # Export variables from .env file, ignoring comments and empty lines
        set -a
        source "$env_file"
        set +a
        
        # Export machine-type variables as Terraform variables (TF_VAR_ prefix)
        if [ -n "$EXECUTION_MACHINE_TYPE" ]; then
            export TF_VAR_execution_machine_type="$EXECUTION_MACHINE_TYPE"
            log_info "Using EXECUTION_MACHINE_TYPE from .env: $EXECUTION_MACHINE_TYPE"
        fi
        if [ -n "$CLIENT_MACHINE_TYPE" ]; then
            export TF_VAR_client_machine_type="$CLIENT_MACHINE_TYPE"
            log_info "Using CLIENT_MACHINE_TYPE from .env: $CLIENT_MACHINE_TYPE"
        fi
        if [ -n "$EXECUTION_LOCAL_SSD_COUNT" ]; then
            export TF_VAR_execution_local_ssd_count="$EXECUTION_LOCAL_SSD_COUNT"
            log_info "Using EXECUTION_LOCAL_SSD_COUNT from .env: $EXECUTION_LOCAL_SSD_COUNT"
        fi
        
        log_success "Environment variables loaded from .env"
    else
        log_info ".env file not found (using defaults or command-line options)"
    fi
}

# Build environment variable string for SSH commands
build_env_string() {
    local env_vars=""
    
    # List of environment variables to pass to remote dev-node.sh
    local vars_to_pass=(
        "RETH_TYPE"
        "DB_SYNC_MODE"
        "HTTP_PORT"
        "WS_PORT"
        "ENGINE_PORT"
        "P2P_PORT"
        "DEV_BLOCK_TIME"
        "DEV_BLOCK_MAX_TXNS"
        "BUILDER_GAS_LIMIT"
        "LOG_LEVEL"
    )
    
    for var in "${vars_to_pass[@]}"; do
        local value="${!var}"
        if [ -n "$value" ]; then
            # Escape single quotes in the value for bash -c
            local escaped_value=$(echo "$value" | sed "s/'/'\"'\"'/g")
            env_vars="${env_vars}export ${var}='${escaped_value}' && "
        fi
    done
    
    echo "$env_vars"
}

# Load .env file early (after logging functions are defined)
load_env_file

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
  all               Full deployment (execution + client nodes) (default)
  execution         Deploy execution node only
  client            Deploy client node only
  status            Show current status
  ssh-execution     SSH to execution node
  ssh-client        SSH to client node
  logs-execution    Show execution node logs (last 100 lines)
  logs-client       Show client node logs (last 100 lines)
  copy-logs         Copy gravity-bench.log from client node to local machine
  rerun-execution   Re-run execution node setup and restart (cleanup + start)
  restart-execution Reset execution node (clean all data and start fresh)
  rerun-client      Re-run client node setup script
  start-benchmark   Start benchmark on client node

Note: For full debugging capabilities, use ./debug.sh instead

Options:
  -h, --help        Show this help message
  -y, --yes         Auto-approve apply/destroy operations
  -v, --verbose     Verbose output
  --reth-type TYPE  Reth binary type: 'gravity' or 'reth' (default: reth)
                     Can also be set via RETH_TYPE environment variable or .env file

Environment Configuration:
  The script automatically loads variables from .env file if it exists in the same
  directory as deploy.sh. Copy .env.example to .env and modify as needed.
  
  Supported variables in .env:
    - EXECUTION_MACHINE_TYPE: GCP machine type for execution node (e.g., c4-highcpu-16, e2-standard-8)
    - CLIENT_MACHINE_TYPE: GCP machine type for client node (e.g., e2-standard-8, e2-standard-4)
    - EXECUTION_LOCAL_SSD_COUNT: Number of local SSD (NVMe) disks for execution node (default: 0). Note: Not all machine types support local SSDs (e.g., c4-highcpu-16 does not support them)
    - RETH_TYPE: 'gravity' or 'reth' (default: reth)
    - DB_SYNC_MODE: durable, nometasync, safenosync, utterlynosync
    - HTTP_PORT, WS_PORT, ENGINE_PORT, P2P_PORT: Port numbers
    - DEV_BLOCK_TIME: Block time interval (default: 1s)
    - DEV_BLOCK_MAX_TXNS: Max transactions per block
    - BUILDER_GAS_LIMIT: Block gas limit
    - LOG_LEVEL: trace, debug, info, warn, error

Examples:
  $0                    # Full deployment (execution + client nodes)
  $0 all                # Full deployment (execution + client nodes)
  $0 init               # Initialize only
  $0 plan               # Show plan only
  $0 apply              # Apply changes
  $0 apply --yes         # Apply without confirmation
  $0 destroy            # Destroy all resources
  $0 output             # Show outputs
  $0 execution         # Deploy execution node only (uses reth by default)
  $0 execution --reth-type reth  # Deploy execution node with reth binary (default)
  $0 execution --reth-type gravity  # Deploy execution node with gravity-reth binary
  $0 client            # Deploy client node only
  $0 ssh-execution      # SSH to execution node
  $0 ssh-client         # SSH to client node
  $0 rerun-execution    # Re-run execution node setup and restart (cleanup + start)
  $0 rerun-execution --reth-type reth  # Re-run with reth binary
  $0 restart-execution  # Reset execution node (clean all data and start fresh)
  $0 restart-execution --reth-type reth  # Reset with reth binary
  $0 rerun-client       # Re-run client node setup script
  $0 start-benchmark    # Start benchmark on client node
  $0 flood-benchmark    # Run flood load testing benchmark on client node
  $0 copy-logs          # Copy gravity-bench.log from client node to local machine

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
    
    # Ensure terraform is initialized (always run init to handle lock file inconsistencies)
    log_info "Initializing Terraform (ensuring providers are up to date)..."
    terraform init
    
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
    
    log_info "Starting full deployment (execution + client)..."
    
    # Deploy execution node first
    log_info "=== Deploying Execution Node ==="
    if ! cmd_deploy_execution "$auto_approve"; then
        log_error "Execution node deployment failed"
        exit 1
    fi
    
    log_info ""
    log_info "=== Deploying Client Node ==="
    # Deploy client node second
    if ! cmd_deploy_client "$auto_approve"; then
        log_error "Client node deployment failed"
        exit 1
    fi
    
    log_success "Full deployment completed (execution + client)!"
    log_info ""
    log_info "Deployment summary:"
    terraform output -json execution_node_info 2>/dev/null | head -5 || echo "  Execution node: See 'terraform output' for details"
    terraform output -json client_node_info 2>/dev/null | head -5 || echo "  Client node: See 'terraform output' for details"
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
        # Clean gravity-reth data from both possible locations (NVMe /data/bench or boot disk /opt/bench)
        # Check NVMe location first
        if [ -d "/data/bench" ]; then
            rm -rf /data/bench/.dev-node-data 2>/dev/null || true
            rm -rf /data/bench/.dev-node-logs 2>/dev/null || true
            rm -rf /data/bench/.dev-node-pids 2>/dev/null || true
            echo "[INFO] Cleaned /data/bench data directories"
        fi
        # Also clean old /opt/bench location (fallback or legacy)
        if [ -d "/opt/gravity-reth/bench" ]; then
            rm -rf /opt/gravity-reth/bench/.dev-node-data 2>/dev/null || true
            rm -rf /opt/gravity-reth/bench/.dev-node-logs 2>/dev/null || true
            rm -rf /opt/gravity-reth/bench/.dev-node-pids 2>/dev/null || true
            echo "[INFO] Cleaned /opt/gravity-reth/bench data directories"
        fi
        # Clean /opt/bench if it exists (legacy location)
        if [ -d "/opt/bench" ]; then
            rm -rf /opt/bench/.dev-node-data 2>/dev/null || true
            rm -rf /opt/bench/.dev-node-logs 2>/dev/null || true
            rm -rf /opt/bench/.dev-node-pids 2>/dev/null || true
            echo "[INFO] Cleaned /opt/bench data directories"
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
    if ! cmd_run_execution; then
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
        "null_resource.copy_client_scripts"
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
    
    log_success "Client node infrastructure created!"
    
    # Get client node info for SSH
    get_client_node_info || return 1
    
    # Execute client scripts in sequence
    log_info "Executing client scripts in sequence..."
    
    # Step 1: Run client-setup.sh
    if cmd_run_client_setup; then
        log_success "Client setup completed"
    else
        log_error "Client setup failed"
        return 1
    fi
    
    # Step 2: Run client-build.sh
    if cmd_run_client_build; then
        log_success "Client build completed"
    else
        log_error "Client build failed"
        return 1
    fi
    
    # Create and copy bench_config.toml after build completes
    log_info "Creating and copying bench_config.toml..."
    if cmd_create_and_copy_bench_config; then
        log_success "bench_config.toml created and deployed"
    else
        log_warning "Failed to create bench_config.toml, but continuing..."
        log_info "You can create it manually using the template at /tmp/bench_config.template"
    fi
    
    # Step 3: Run client-benchmark.sh
    if cmd_run_client_benchmark; then
        log_success "Client benchmark started"
    else
        log_warning "Client benchmark failed or skipped"
        log_info "You can run it manually: ssh to client node and run: sudo bash /opt/client-benchmark.sh"
    fi
    
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
    cmd_run_execution || exit 1
    log_success "Execution node rerun completed successfully!"
}

# Restart execution node (clean all data and start fresh)
cmd_restart_execution() {
    log_info "Restarting execution node (cleaning all data and starting fresh)..."
    get_execution_node_info || return 1
    
    # Copy the latest dev-node.sh script to ensure it has the reset command
    local dev_node_script="$SCRIPT_DIR/scripts/dev-node.sh"
    [ ! -f "$dev_node_script" ] && { log_error "dev-node.sh not found"; return 1; }
    
    log_info "Updating dev-node.sh script on execution node..."
    scp_execution "$dev_node_script" "/tmp/dev-node.sh" || {
        log_error "Failed to copy dev-node.sh"
        return 1
    }
    ssh_execution "chmod +x /tmp/dev-node.sh && sudo mv /tmp/dev-node.sh /opt/dev-node.sh" || {
        log_error "Failed to update dev-node.sh on execution node"
        return 1
    }
    
    log_info "Calling reset command on execution node with RETH_TYPE=$RETH_TYPE..."
    local env_string=$(build_env_string)
    ssh_execution "bash -c '${env_string}/opt/dev-node.sh reset'" || {
        log_error "Failed to restart execution node"
        return 1
    }
    log_success "Execution node restart completed successfully!"
}

# Re-run client setup
cmd_rerun_client() {
    log_info "Re-running client node setup..."
    
    # Create and copy bench_config.toml first
    if cmd_create_and_copy_bench_config; then
        log_success "bench_config.toml updated"
    else
        log_warning "Failed to update bench_config.toml"
    fi
    
    # Re-run setup script if it exists
    if [ -f "run-setup.sh" ]; then
        bash run-setup.sh client
    else
        log_info "run-setup.sh not found, skipping script re-run"
        log_info "bench_config.toml has been updated on the client node"
    fi
}

# Get execution node connection info (returns via global variables)
get_execution_node_info() {
    local ssh_key="gravity-deploy-key"
    local output=$(terraform output -json execution_node_info 2>/dev/null || echo "")
    
    [ -z "$output" ] || [ "$output" = "null" ] && { log_error "Execution node not found. Run 'terraform apply' first."; return 1; }
    
    EXECUTION_NODE_IP=$(echo "$output" | grep -o '"external_ip":"[^"]*"' | cut -d'"' -f4)
    EXECUTION_NODE_INTERNAL_IP=$(echo "$output" | grep -o '"internal_ip":"[^"]*"' | cut -d'"' -f4)
    EXECUTION_NODE_USER="ubuntu"
    EXECUTION_NODE_SSH_KEY="$ssh_key"
    
    [ -z "$EXECUTION_NODE_IP" ] && { log_error "Could not extract execution node IP"; return 1; }
    [ -z "$EXECUTION_NODE_INTERNAL_IP" ] && { log_error "Could not extract execution node internal IP"; return 1; }
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

# Get client node connection info (returns via global variables)
get_client_node_info() {
    local ssh_key="gravity-deploy-key"
    local output=$(terraform output -json client_node_info 2>/dev/null || echo "")
    
    [ -z "$output" ] || [ "$output" = "null" ] && { log_error "Client node not found. Run 'terraform apply' first."; return 1; }
    
    CLIENT_NODE_IP=$(echo "$output" | grep -o '"external_ip":"[^"]*"' | cut -d'"' -f4)
    CLIENT_NODE_USER="ubuntu"
    CLIENT_NODE_SSH_KEY="$ssh_key"
    
    [ -z "$CLIENT_NODE_IP" ] && { log_error "Could not extract client node IP"; return 1; }
    [ ! -f "$ssh_key" ] && { log_error "SSH key not found: $ssh_key"; return 1; }
    
    return 0
}

# Create bench_config.toml locally from template and copy to client node
cmd_create_and_copy_bench_config() {
    log_info "Creating bench_config.toml from template..."
    
    # Get execution node internal IP from Terraform
    local execution_output=$(terraform output -json execution_node_info 2>/dev/null || echo "")
    local execution_internal_ip=$(echo "$execution_output" | grep -o '"internal_ip":"[^"]*"' | cut -d'"' -f4)
    local http_port=$(terraform output -raw http_port 2>/dev/null || echo "8545")
    
    [ -z "$execution_internal_ip" ] && { log_error "Could not get execution node internal IP from Terraform"; return 1; }
    
    local template_file="$SCRIPT_DIR/templates/bench_config.template"
    [ ! -f "$template_file" ] && { log_error "Template file not found: $template_file"; return 1; }
    
    # Create bench_config.toml locally
    local local_config="/tmp/bench_config.toml"
    local execution_url="http://${execution_internal_ip}:${http_port}"
    
    log_info "Execution node URL: $execution_url"
    sed "s|EXECUTION_NODE|${execution_url}|g" "$template_file" > "$local_config" || {
        log_error "Failed to create bench_config.toml from template"
        return 1
    }
    
    # Verify the replacement worked
    if grep -q "EXECUTION_NODE" "$local_config"; then
        log_error "Template replacement failed - EXECUTION_NODE still present in config"
        return 1
    fi
    
    # Copy to client node
    get_client_node_info || return 1
    log_info "Copying bench_config.toml to client node..."
    scp_client "$local_config" "/tmp/bench_config.toml" || {
        log_error "Failed to copy bench_config.toml to client node"
        return 1
    }
    
    # Move to final location on client node
    ssh_client "sudo mv /tmp/bench_config.toml /opt/bench_config.toml && sudo chown ubuntu:ubuntu /opt/bench_config.toml" || {
        log_error "Failed to install bench_config.toml on client node"
        return 1
    }
    
    log_success "bench_config.toml created and copied to client node"
    rm -f "$local_config" 2>/dev/null || true
    return 0
}

# Helper: Execute SSH command on client node
ssh_client() {
    local cmd="$1"
    get_client_node_info || return 1
    ssh -i "$CLIENT_NODE_SSH_KEY" -o StrictHostKeyChecking=no "$CLIENT_NODE_USER@$CLIENT_NODE_IP" "$cmd"
}

# Helper: Copy file to client node
scp_client() {
    local local_file="$1"
    local remote_path="$2"
    get_client_node_info || return 1
    scp -i "$CLIENT_NODE_SSH_KEY" -o StrictHostKeyChecking=no "$local_file" "$CLIENT_NODE_USER@$CLIENT_NODE_IP:$remote_path" >/dev/null 2>&1
}

# Run client-setup.sh script
cmd_run_client_setup() {
    log_info "Running client-setup.sh (installing system packages and Rust)..."
    get_client_node_info || return 1
    
    # Check if script exists
    if ! ssh_client "test -f /opt/client-setup.sh"; then
        log_error "Script /opt/client-setup.sh not found. Copy step may have failed."
        return 1
    fi
    
    # Run the script and capture output
    log_info "Executing /opt/client-setup.sh..."
    if ssh_client "sudo bash /opt/client-setup.sh 2>&1 | sudo tee -a /var/log/client-setup.log"; then
        # Check for completion marker
        if ssh_client "test -f /var/log/client-setup-complete"; then
            log_success "Client setup completed successfully"
            return 0
        else
            log_error "Client setup script completed but marker file not found"
            log_info "Last 50 lines of log:"
            ssh_client "sudo tail -50 /var/log/client-setup.log" || true
            return 1
        fi
    else
        log_error "Client setup script failed"
        log_info "Last 50 lines of log:"
        ssh_client "sudo tail -50 /var/log/client-setup.log" || true
        return 1
    fi
}

# Run client-build.sh script
cmd_run_client_build() {
    log_info "Running client-build.sh (building client code and preparing config)..."
    get_client_node_info || return 1
    
    # Check if setup is complete
    if ! ssh_client "test -f /var/log/client-setup-complete"; then
        log_error "Client setup not completed. Please run client-setup.sh first."
        return 1
    fi
    
    # Check if script exists
    if ! ssh_client "test -f /opt/client-build.sh"; then
        log_error "Script /opt/client-build.sh not found. Copy step may have failed."
        return 1
    fi
    
    # Run the script and capture output
    log_info "Executing /opt/client-build.sh..."
    if ssh_client "sudo bash /opt/client-build.sh 2>&1 | sudo tee -a /var/log/client-build.log"; then
        # Check for completion marker
        if ssh_client "test -f /var/log/client-build-complete"; then
            log_success "Client build completed successfully"
            return 0
        else
            log_error "Client build script completed but marker file not found"
            log_info "Last 50 lines of log:"
            ssh_client "sudo tail -50 /var/log/client-build.log" || true
            return 1
        fi
    else
        log_error "Client build script failed"
        log_info "Last 50 lines of log:"
        ssh_client "sudo tail -50 /var/log/client-build.log" || true
        log_info "Checking related logs..."
        ssh_client "sudo tail -50 /var/log/gravity-bench-build.log 2>/dev/null || echo 'Build log not found'" || true
        return 1
    fi
}

# Run client-benchmark.sh script
cmd_run_client_benchmark() {
    log_info "Running client-benchmark.sh (starting benchmark)..."
    get_client_node_info || return 1
    
    # Check if build is complete
    if ! ssh_client "test -f /var/log/client-build-complete"; then
        log_error "Client build not completed. Please run client-build.sh first."
        return 1
    fi
    
    # Check if script exists
    if ! ssh_client "test -f /opt/client-benchmark.sh"; then
        log_error "Script /opt/client-benchmark.sh not found. Copy step may have failed."
        return 1
    fi
    
    # Run the script and capture output
    log_info "Executing /opt/client-benchmark.sh..."
    if ssh_client "sudo bash /opt/client-benchmark.sh 2>&1 | sudo tee -a /var/log/client-benchmark.log"; then
        # Check for completion marker
        if ssh_client "test -f /var/log/client-benchmark-complete"; then
            log_success "Client benchmark started successfully"
            log_info "Benchmark logs: /opt/gravity_bench/logs/gravity-bench.log"
            log_info "To view logs: ssh to client node and run: tail -f /opt/gravity_bench/logs/gravity-bench.log"
            return 0
        else
            log_warning "Client benchmark script completed but marker file not found"
            log_info "Last 50 lines of log:"
            ssh_client "sudo tail -50 /var/log/client-benchmark.log" || true
            return 1
        fi
    else
        log_warning "Client benchmark script failed"
        log_info "Last 50 lines of log:"
        ssh_client "sudo tail -50 /var/log/client-benchmark.log" || true
        return 1
    fi
}

# Start benchmark on client node (legacy function, kept for compatibility)
cmd_start_benchmark() {
    log_warning "cmd_start_benchmark is deprecated. Use cmd_run_client_benchmark instead."
    cmd_run_client_benchmark
}

# Run flood benchmark on client node
cmd_run_flood_benchmark() {
    log_info "Running flood benchmark on client node..."
    get_client_node_info || return 1
    
    # Check if setup is complete
    if ! ssh_client "test -f /var/log/client-setup-complete"; then
        log_error "Client setup not completed. Please run client setup first."
        return 1
    fi
    
    # Copy flood-benchmark.sh to client node if not already there
    local flood_script="$SCRIPT_DIR/scripts/flood-benchmark.sh"
    if [ ! -f "$flood_script" ]; then
        log_error "flood-benchmark.sh not found at $flood_script"
        return 1
    fi
    
    log_info "Copying flood-benchmark.sh to client node..."
    scp_client "$flood_script" "/tmp/flood-benchmark.sh" || {
        log_error "Failed to copy flood-benchmark.sh to client node"
        return 1
    }
    
    ssh_client "chmod +x /tmp/flood-benchmark.sh && sudo mv /tmp/flood-benchmark.sh /opt/flood-benchmark.sh" || {
        log_error "Failed to install flood-benchmark.sh on client node"
        return 1
    }
    
    # Get execution node RPC endpoint from terraform
    local execution_output=$(terraform output -json execution_node_info 2>/dev/null || echo "")
    local execution_internal_ip=$(echo "$execution_output" | grep -o '"internal_ip":"[^"]*"' | cut -d'"' -f4)
    local http_port=$(terraform output -raw http_port 2>/dev/null || echo "8545")
    
    if [ -z "$execution_internal_ip" ]; then
        log_error "Could not get execution node internal IP from Terraform"
        return 1
    fi
    
    local rpc_endpoint="http://${execution_internal_ip}:${http_port}"
    log_info "Execution node RPC endpoint: $rpc_endpoint"
    
    # Run flood benchmark
    log_info "Executing flood benchmark..."
    log_info "Command: /opt/flood-benchmark.sh --rpc $rpc_endpoint --rate ${FLOOD_RATE:-12000} --senders ${FLOOD_SENDERS:-10000}"
    
    # Properly escape variables for SSH command
    local flood_rate="${FLOOD_RATE:-12000}"
    local flood_senders="${FLOOD_SENDERS:-10000}"
    
    # Build the command with proper quoting for SSH
    # Use printf to safely construct the command string
    local flood_cmd=$(printf 'sudo bash /opt/flood-benchmark.sh --rpc %s --rate %s --senders %s 2>&1 | sudo tee -a /var/log/flood-benchmark.log' \
        "$rpc_endpoint" "$flood_rate" "$flood_senders")
    
    if ssh_client "$flood_cmd"; then
        log_success "Flood benchmark completed successfully"
        log_info "Flood benchmark logs: /var/log/flood-benchmark.log"
        log_info "Results directory: /opt/flood_results"
        return 0
    else
        log_error "Flood benchmark failed"
        log_info "Last 50 lines of log:"
        ssh_client "sudo tail -50 /var/log/flood-benchmark.log" || true
        return 1
    fi
}

# Run steps 2-4: Copy scripts, execute setup, start dev node
cmd_run_execution() {
    log_info "Running execution node steps 2-4..."
    
    # Step 2: Copy scripts
    log_info "Step 2: Copying scripts to remote node..."
    get_execution_node_info || return 1
    
    local dev_node_script="$SCRIPT_DIR/scripts/dev-node.sh"
    local execution_setup_script="$SCRIPT_DIR/scripts/execution-node-setup.sh"
    local client_setup_script="$SCRIPT_DIR/scripts/client-setup.sh"
    local client_build_script="$SCRIPT_DIR/scripts/client-build.sh"
    local client_benchmark_script="$SCRIPT_DIR/scripts/client-benchmark.sh"
    local flood_benchmark_script="$SCRIPT_DIR/scripts/flood-benchmark.sh"
    
    [ ! -f "$dev_node_script" ] && { log_error "dev-node.sh not found"; return 1; }
    [ ! -f "$execution_setup_script" ] && { log_error "execution-node-setup.sh not found"; return 1; }
    [ ! -f "$client_setup_script" ] && { log_error "client-setup.sh not found"; return 1; }
    [ ! -f "$client_build_script" ] && { log_error "client-build.sh not found"; return 1; }
    [ ! -f "$client_benchmark_script" ] && { log_error "client-benchmark.sh not found"; return 1; }
    [ ! -f "$flood_benchmark_script" ] && { log_error "flood-benchmark.sh not found"; return 1; }

    scp_execution "$dev_node_script" "/tmp/dev-node.sh" || { log_error "Failed to copy dev-node.sh"; return 1; }
    
    # Process execution-node-setup.sh template variables before copying
    log_info "Processing execution-node-setup.sh template variables..."
    local temp_setup_script="/tmp/execution-node-setup-processed.sh"
    
    # Get template variables from Terraform outputs (with fallbacks)
    cd "$SCRIPT_DIR" || { log_error "Failed to change to script directory"; return 1; }
    local gravity_reth_repo=$(terraform output -raw gravity_reth_repo 2>/dev/null || echo "https://github.com/Galxe/gravity-reth.git")
    local gravity_reth_branch=$(terraform output -raw gravity_reth_branch 2>/dev/null || echo "main")
    local gravity_sdk_repo=$(terraform output -raw gravity_sdk_repo 2>/dev/null || echo "https://github.com/Galxe/gravity-sdk.git")
    local gravity_sdk_branch=$(terraform output -raw gravity_sdk_branch 2>/dev/null || echo "main")
    local reth_repo=$(terraform output -raw reth_repo 2>/dev/null || echo "https://github.com/paradigmxyz/reth.git")
    local reth_branch_or_tag=$(terraform output -raw reth_branch_or_tag 2>/dev/null || echo "v1.9.3")
    local http_port=$(terraform output -raw http_port 2>/dev/null || echo "8545")
    local ws_port=$(terraform output -raw ws_port 2>/dev/null || echo "8546")
    local engine_port=$(terraform output -raw engine_port 2>/dev/null || echo "8551")
    local p2p_port=$(terraform output -raw p2p_port 2>/dev/null || echo "30303")
    
    # Substitute template variables in the script
    sed -e "s|\${gravity_reth_repo}|${gravity_reth_repo}|g" \
        -e "s|\${gravity_reth_branch}|${gravity_reth_branch}|g" \
        -e "s|\${gravity_sdk_repo}|${gravity_sdk_repo}|g" \
        -e "s|\${gravity_sdk_branch}|${gravity_sdk_branch}|g" \
        -e "s|\${reth_repo}|${reth_repo}|g" \
        -e "s|\${reth_branch_or_tag}|${reth_branch_or_tag}|g" \
        -e "s|\${http_port}|${http_port}|g" \
        -e "s|\${ws_port}|${ws_port}|g" \
        -e "s|\${engine_port}|${engine_port}|g" \
        -e "s|\${p2p_port}|${p2p_port}|g" \
        "$execution_setup_script" > "$temp_setup_script" || {
        log_error "Failed to process execution-node-setup.sh template"
        return 1
    }
    
    scp_execution "$temp_setup_script" "/tmp/setup-execution-node.sh" || { log_error "Failed to copy execution-node-setup.sh"; return 1; }
    rm -f "$temp_setup_script" 2>/dev/null || true
    
    scp_execution "$client_setup_script" "/tmp/client-setup.sh" || { log_error "Failed to copy client-setup.sh"; return 1; }
    scp_execution "$client_build_script" "/tmp/client-build.sh" || { log_error "Failed to copy client-build.sh"; return 1; }
    scp_execution "$client_benchmark_script" "/tmp/client-benchmark.sh" || { log_error "Failed to copy client-benchmark.sh"; return 1; }
    scp_execution "$flood_benchmark_script" "/tmp/flood-benchmark.sh" || { log_error "Failed to copy flood-benchmark.sh"; return 1; }
    # Install scripts step by step
    # ssh_execution "sudo mkdir -p /opt/gravity-reth/bench && sudo chown -R ubuntu:ubuntu /opt/gravity-reth/bench" || { log_error "Failed to create bench directory"; return 1; }
    #ssh_execution "cp /tmp/dev-node.sh /opt/gravity-reth/bench/dev-node.sh" || { log_error "Failed to copy dev-node.sh"; return 1; }
    ssh_execution "chmod +x /tmp/dev-node.sh /tmp/setup-execution-node.sh /tmp/client-build.sh /tmp/client-benchmark.sh /tmp/flood-benchmark.sh" || { log_error "Failed to set permissions"; return 1; }
    ssh_execution "sudo mv /tmp/setup-execution-node.sh /opt/setup-execution-node.sh" || { log_error "Failed to move setup script"; return 1; }
    ssh_execution "sudo mv /tmp/dev-node.sh /opt/dev-node.sh" || { log_error "Failed to move setup script"; return 1; }
    ssh_execution "sudo mv /tmp/client-setup.sh /opt/client-setup.sh" || { log_error "Failed to move client-setup.sh"; return 1; }
    ssh_execution "sudo mv /tmp/client-build.sh /opt/client-build.sh" || { log_error "Failed to move client-build.sh"; return 1; }
    ssh_execution "sudo mv /tmp/client-benchmark.sh /opt/client-benchmark.sh" || { log_error "Failed to move client-benchmark.sh"; return 1; }
    ssh_execution "sudo mv /tmp/flood-benchmark.sh /opt/flood-benchmark.sh" || { log_error "Failed to move flood-benchmark.sh"; return 1; }
    
    log_success "Scripts copied"
    
    local bench_config_template="$SCRIPT_DIR/templates/bench_config.template"
    [ ! -f "$bench_config_template" ] && { log_error "bench_config.template not found"; return 1; }
    local http_port=$(terraform output -raw http_port 2>/dev/null || echo "8545")
    local temp_bench_config="/tmp/bench_config.toml"
    sed "s|EXECUTION_NODE|http://localhost:${http_port}|g" "$bench_config_template" > "$temp_bench_config" || {
        log_error "Failed to process bench_config.template"
        return 1
    }
    scp_execution "$temp_bench_config" "/tmp/bench_config.toml" || { log_error "Failed to copy bench_config.toml"; return 1; }
    rm -f "$temp_bench_config" 2>/dev/null || true
    ssh_execution "sudo mv /tmp/bench_config.toml /opt/bench_config.toml" || { log_error "Failed to move bench_config.toml"; return 1; }
    
    # Step 3: Execute setup
    log_info "Step 3: Executing execution-node-setup.sh..."
    ssh_execution "sudo bash /opt/setup-execution-node.sh 2>&1 | sudo tee -a /var/log/execution-node-setup.log" || { log_error "Setup script failed. Check /var/log/execution-node-setup.log"; return 1; }
    ssh_execution "test -f /var/log/execution-node-setup-complete" || { log_error "Setup did not complete successfully"; return 1; }
    log_success "Setup completed"
    
    # Step 4: Start dev node
    log_info "Step 4: Starting dev node with RETH_TYPE=$RETH_TYPE..."
    local env_string=$(build_env_string)
    # Build command arguments for dev-node.sh
    local dev_node_args="--reth-type $RETH_TYPE"
    [ -n "$DB_SYNC_MODE" ] && dev_node_args="$dev_node_args --db-sync-mode $DB_SYNC_MODE"
    [ -n "$HTTP_PORT" ] && dev_node_args="$dev_node_args --http-port $HTTP_PORT"
    [ -n "$WS_PORT" ] && dev_node_args="$dev_node_args --ws-port $WS_PORT"
    [ -n "$ENGINE_PORT" ] && dev_node_args="$dev_node_args --engine-port $ENGINE_PORT"
    [ -n "$P2P_PORT" ] && dev_node_args="$dev_node_args --p2p-port $P2P_PORT"
    [ -n "$DEV_BLOCK_TIME" ] && dev_node_args="$dev_node_args --dev-block-time $DEV_BLOCK_TIME"
    [ -n "$DEV_BLOCK_MAX_TXNS" ] && dev_node_args="$dev_node_args --dev-block-max-txns $DEV_BLOCK_MAX_TXNS"
    [ -n "$BUILDER_GAS_LIMIT" ] && dev_node_args="$dev_node_args --builder-gas-limit $BUILDER_GAS_LIMIT"
    [ -n "$LOG_LEVEL" ] && dev_node_args="$dev_node_args --log-level $LOG_LEVEL"
    
    ssh_execution "bash -c '${env_string}/opt/dev-node.sh start $dev_node_args'" || { log_error "Failed to start dev node"; return 1; }
    log_success "Dev node started with $RETH_TYPE binary"
    # Step 5: Prepare client benchmark
    log_info "Step 5: Prepare client benchmark"
    ssh_execution "sudo bash /opt/client-setup.sh 2>&1 | sudo tee -a /var/log/client-setup.log" || { log_error "Setup script failed. Check /var/log/client-setup.log"; return 1; }
    log_success "Client setup completed"
    ssh_execution "sudo bash /opt/client-build.sh 2>&1 | sudo tee -a /var/log/client-build.log" || { log_error "Setup script failed. Check /var/log/client-build.log"; return 1; }
    log_success "Client build completed"
    # Step 6: Run client benchmark
    log_info "Step 6: Run client benchmark"
    ssh_execution "sudo bash /opt/client-benchmark.sh 2>&1 | sudo tee -a /var/log/client-benchmark.log" || { log_error "Setup script failed. Check /var/log/client-benchmark.log"; return 1; }
    log_success "Client benchmark started"
    return 0
}

# Internal function to restart execution node (used by cmd_apply)
cmd_restart_execution_internal() {
    cmd_run_execution
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
        "tail -100 /var/log/execution-node-setup.log 2>/dev/null || echo 'Setup log not found'; echo; tail -100 /var/log/gravity-reth-startup.log 2>/dev/null || echo 'Startup log not found'; echo; (tail -100 /data/bench/.dev-node-logs/reth-node.log 2>/dev/null || tail -100 /opt/bench/.dev-node-logs/reth-node.log 2>/dev/null || tail -100 /opt/gravity-reth/bench/.dev-node-logs/reth-node.log 2>/dev/null || echo 'Reth log not found')"
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
        "tail -100 /var/log/client-setup.log 2>/dev/null || echo 'Setup log not found'; echo; tail -100 /var/log/client-build.log 2>/dev/null || echo 'Build log not found'; echo; tail -100 /opt/gravity_bench/logs/gravity-bench.log 2>/dev/null || echo 'Bench log not found'"
}

# Copy gravity-bench.log from client node to local machine
cmd_copy_client_logs() {
    log_info "Copying gravity-bench.log from client node..."
    
    get_client_node_info || return 1
    
    # Create local logs directory if it doesn't exist
    local local_logs_dir="logs"
    mkdir -p "$local_logs_dir"
    
    # Generate timestamp for log file
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local local_log_file="$local_logs_dir/gravity-bench-${timestamp}.log"
    
    # Check if log file exists on remote node
    if ! ssh_client "test -f /opt/gravity_bench/logs/gravity-bench.log"; then
        log_error "Log file not found on client node: /opt/gravity_bench/logs/gravity-bench.log"
        log_info "The benchmark may not have started yet, or the log file is in a different location"
        return 1
    fi
    
    # Copy the log file
    log_info "Copying /opt/gravity_bench/logs/gravity-bench.log to $local_log_file..."
    scp -i "$CLIENT_NODE_SSH_KEY" -o StrictHostKeyChecking=no \
        "$CLIENT_NODE_USER@$CLIENT_NODE_IP:/opt/gravity_bench/logs/gravity-bench.log" \
        "$local_log_file" || {
        log_error "Failed to copy log file from client node"
        return 1
    }
    
    log_success "Log file copied successfully to: $local_log_file"
    log_info "File size: $(du -h "$local_log_file" | cut -f1)"
    log_info "Last 10 lines:"
    tail -10 "$local_log_file"
    
    return 0
}

# Parse arguments
AUTO_APPROVE="false"
VERBOSE="false"
COMMAND="all"
# RETH_TYPE can be set via environment variable or command-line option
RETH_TYPE="${RETH_TYPE:-reth}"  # Default to "reth"

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
        --reth-type)
            RETH_TYPE="$2"
            if [ "$RETH_TYPE" != "gravity" ] && [ "$RETH_TYPE" != "reth" ]; then
                log_error "Invalid RETH_TYPE: $RETH_TYPE. Must be 'gravity' or 'reth'"
                exit 1
            fi
            shift 2
            ;;
        init|plan|apply|destroy|output|all|execution|client|status|ssh-execution|ssh-client|logs-execution|logs-client|copy-logs|rerun-execution|restart-execution|rerun-client|start-benchmark|flood-benchmark)
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
if [ "$COMMAND" != "destroy" ] && [ "$COMMAND" != "ssh-execution" ] && [ "$COMMAND" != "ssh-client" ] && [ "$COMMAND" != "logs-execution" ] && [ "$COMMAND" != "logs-client" ] && [ "$COMMAND" != "copy-logs" ] && [ "$COMMAND" != "rerun-execution" ] && [ "$COMMAND" != "restart-execution" ] && [ "$COMMAND" != "rerun-client" ] && [ "$COMMAND" != "start-benchmark" ] && [ "$COMMAND" != "flood-benchmark" ]; then
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
    copy-logs)
        cmd_copy_client_logs
        ;;
    rerun-execution)
        cmd_rerun_execution
        ;;
    restart-execution)
        cmd_restart_execution
        ;;
    rerun-client)
        cmd_rerun_client
        ;;
    start-benchmark)
        cmd_start_benchmark
        ;;
    flood-benchmark)
        cmd_run_flood_benchmark
        ;;
    all|*)
        cmd_deploy "$AUTO_APPROVE"
        ;;
esac

