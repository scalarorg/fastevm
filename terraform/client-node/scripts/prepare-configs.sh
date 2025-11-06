#!/bin/bash
# FastEVM Client Node Configuration Preparation Script

set -e

# Configuration
CLIENT_CONFIG_DIR="/home/ubuntu/client-config"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLIENT_NODE_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$CLIENT_NODE_DIR")"
CONFIG_DIR="${CLIENT_NODE_DIR}/config"

# Load configuration from environment file
CONFIG_ENV_FILE="${PROJECT_ROOT}/fastevm.env"

if [ -f "$CONFIG_ENV_FILE" ]; then
    echo "Loading configuration from $CONFIG_ENV_FILE"
    source "$CONFIG_ENV_FILE"
else
    echo "Warning: Configuration file $CONFIG_ENV_FILE not found, using defaults"
fi

# Default values (fallback if not set in fastevm.env)
GITHUB_REPO=${GITHUB_REPO:-"https://github.com/scalarorg/fastevm.git"}
GITHUB_BRANCH=${GITHUB_BRANCH:-"terraform"}
CHAIN_ID=${CHAIN_ID:-202501}

# Test configuration defaults
TEST_SENDER_COUNT=${TEST_SENDER_COUNT:-10000}
TEST_TRANSACTION_COUNT=${TEST_TRANSACTION_COUNT:-10}
TEST_TRANSACTION_VALUE=${TEST_TRANSACTION_VALUE:-1000000000000000}
TEST_MNEMONIC=${TEST_MNEMONIC:-"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"}
TEST_FETCH_NONCE=${TEST_FETCH_NONCE:-true}
TEST_WAITING_TIME_SECONDS=${TEST_WAITING_TIME_SECONDS:-30}
TEST_RPC_TIMEOUT=${TEST_RPC_TIMEOUT:-30}
TEST_MAX_RETRIES=${TEST_MAX_RETRIES:-3}
TEST_LOG_LEVEL=${TEST_LOG_LEVEL:-info}

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

# Function to detect RPC URLs from main deployment
detect_rpc_urls() {
    log_info "Detecting RPC URLs from main deployment..."
    
    if [ -f "${PROJECT_ROOT}/deployment-info.json" ]; then
        log_info "Using deployment-info.json for RPC URLs"
        # For client node, prioritize external IPs since it's on a separate instance
        RPC_URL1=$(jq -r '.node_endpoints.value["node-1"].external_ip // empty' "${PROJECT_ROOT}/deployment-info.json")
        RPC_URL2=$(jq -r '.node_endpoints.value["node-2"].external_ip // empty' "${PROJECT_ROOT}/deployment-info.json")
        RPC_URL3=$(jq -r '.node_endpoints.value["node-3"].external_ip // empty' "${PROJECT_ROOT}/deployment-info.json")
        RPC_URL4=$(jq -r '.node_endpoints.value["node-4"].external_ip // empty' "${PROJECT_ROOT}/deployment-info.json")
        
        # If external IPs not available, try internal IPs as fallback
        if [ -z "$RPC_URL1" ] || [ "$RPC_URL1" = "null" ]; then
            log_warning "External IPs not found, trying internal IPs as fallback..."
            RPC_URL1=$(jq -r '.node_endpoints.value["node-1"].internal_ip // empty' "${PROJECT_ROOT}/deployment-info.json")
            RPC_URL2=$(jq -r '.node_endpoints.value["node-2"].internal_ip // empty' "${PROJECT_ROOT}/deployment-info.json")
            RPC_URL3=$(jq -r '.node_endpoints.value["node-3"].internal_ip // empty' "${PROJECT_ROOT}/deployment-info.json")
            RPC_URL4=$(jq -r '.node_endpoints.value["node-4"].internal_ip // empty' "${PROJECT_ROOT}/deployment-info.json")
        fi
    elif [ -f "${PROJECT_ROOT}/terraform.tfstate" ]; then
        log_info "Using terraform.tfstate for RPC URLs"
        cd "${PROJECT_ROOT}"
        
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
        
        # For client node, prioritize external IPs since it's on a separate instance
        RPC_URL1=$($TERRAFORM_CMD output -json node_endpoints 2>/dev/null | jq -r '.["node-1"].external_ip // empty' 2>/dev/null || echo "")
        RPC_URL2=$($TERRAFORM_CMD output -json node_endpoints 2>/dev/null | jq -r '.["node-2"].external_ip // empty' 2>/dev/null || echo "")
        RPC_URL3=$($TERRAFORM_CMD output -json node_endpoints 2>/dev/null | jq -r '.["node-3"].external_ip // empty' 2>/dev/null || echo "")
        RPC_URL4=$($TERRAFORM_CMD output -json node_endpoints 2>/dev/null | jq -r '.["node-4"].external_ip // empty' 2>/dev/null || echo "")
        
        # If external IPs not available, try internal IPs as fallback
        if [ -z "$RPC_URL1" ] || [ "$RPC_URL1" = "null" ]; then
            log_warning "External IPs not found, trying internal IPs as fallback..."
            RPC_URL1=$($TERRAFORM_CMD output -json node_endpoints 2>/dev/null | jq -r '.["node-1"].internal_ip // empty' 2>/dev/null || echo "")
            RPC_URL2=$($TERRAFORM_CMD output -json node_endpoints 2>/dev/null | jq -r '.["node-2"].internal_ip // empty' 2>/dev/null || echo "")
            RPC_URL3=$($TERRAFORM_CMD output -json node_endpoints 2>/dev/null | jq -r '.["node-3"].internal_ip // empty' 2>/dev/null || echo "")
            RPC_URL4=$($TERRAFORM_CMD output -json node_endpoints 2>/dev/null | jq -r '.["node-4"].internal_ip // empty' 2>/dev/null || echo "")
        fi
        cd "${CLIENT_NODE_DIR}"
    else
        log_error "No deployment information found!"
        log_error "Please ensure the main FastEVM deployment is completed first."
        exit 1
    fi
    
    # Validate that we have at least one RPC URL
    if [ -z "$RPC_URL1" ] || [ "$RPC_URL1" = "null" ]; then
        log_error "Could not detect RPC URLs from deployment!"
        exit 1
    fi
    
    log_success "Detected RPC URLs: $RPC_URL1, $RPC_URL2, $RPC_URL3, $RPC_URL4"
}

# Function to create configuration directory structure
create_config_structure() {
    log_info "Creating configuration directory structure..."
    mkdir -p "${CONFIG_DIR}"
    log_success "Configuration directories created"
}

# Function to update shared environment configuration
update_shared_env() {
    log_info "Updating shared fastevm.env with RPC URLs..."
    
    local shared_env_file="${PROJECT_ROOT}/fastevm.env"
    
    if [ ! -f "$shared_env_file" ]; then
        log_error "Shared fastevm.env not found at: $shared_env_file"
        log_error "Please ensure the main FastEVM deployment is completed first."
        exit 1
    fi
    
    # Create backup of original file
    cp "$shared_env_file" "${shared_env_file}.backup"
    log_info "Created backup: ${shared_env_file}.backup"
    
    # Update RPC URLs in the shared file
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS sed
        sed -i "" "s|# RPC_URL1=.*|RPC_URL1=http://${RPC_URL1}:8545|g" "$shared_env_file"
        sed -i "" "s|# RPC_URL2=.*|RPC_URL2=http://${RPC_URL2}:8545|g" "$shared_env_file"
        sed -i "" "s|# RPC_URL3=.*|RPC_URL3=http://${RPC_URL3}:8545|g" "$shared_env_file"
        sed -i "" "s|# RPC_URL4=.*|RPC_URL4=http://${RPC_URL4}:8545|g" "$shared_env_file"
    else
        # Linux sed
        sed -i "s|# RPC_URL1=.*|RPC_URL1=http://${RPC_URL1}:8545|g" "$shared_env_file"
        sed -i "s|# RPC_URL2=.*|RPC_URL2=http://${RPC_URL2}:8545|g" "$shared_env_file"
        sed -i "s|# RPC_URL3=.*|RPC_URL3=http://${RPC_URL3}:8545|g" "$shared_env_file"
        sed -i "s|# RPC_URL4=.*|RPC_URL4=http://${RPC_URL4}:8545|g" "$shared_env_file"
    fi
    
    log_success "Shared fastevm.env updated with RPC URLs: $shared_env_file"
}

# Function to generate combined setup script
generate_setup_script() {
    log_info "Generating combined setup script..."
    
    cat > "${CONFIG_DIR}/setup.sh" << 'EOF'
#!/bin/bash
# FastEVM Client Node Setup Script (Bootstrap + Configuration)

set -e

PROJECT_DIR="/home/ubuntu/fastevm"
CONFIG_DIR="/home/ubuntu/client-config"
LOG_FILE="/home/ubuntu/client-setup.log"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

log "Starting FastEVM client node setup..."

# =============================================================================
# BOOTSTRAP PHASE
# =============================================================================
log "=== BOOTSTRAP PHASE ==="

# Check if binaries are available locally, otherwise build
log "Checking for pre-built binaries..."
if [ -f "/opt/fastevm-binaries/fastevm-test" ]; then
    log "Pre-built binary found, installing..."
    sudo cp /opt/fastevm-binaries/fastevm-test /usr/local/bin/
    sudo chmod +x /usr/local/bin/fastevm-test
    log "Binary installed from pre-built source"
else
    log "No pre-built binary found, building from source..."
    
    # Install Rust
    log "Installing Rust system-wide..."
    export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$HOME/.cargo/bin"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
    source $HOME/.cargo/env
    rustup default stable

    # Clone and build FastEVM
    log "Cloning FastEVM repository..."
    cd /home/ubuntu
    if [ -d "$PROJECT_DIR" ]; then
        cd "$PROJECT_DIR"
        git fetch origin
        git checkout "GITHUB_BRANCH_PLACEHOLDER"
        git pull origin "GITHUB_BRANCH_PLACEHOLDER"
    else
        git clone -b "GITHUB_BRANCH_PLACEHOLDER" "GITHUB_REPO_PLACEHOLDER" "$PROJECT_DIR"
    fi

    # Build test binary
    log "Building FastEVM test binary..."
    cd "$PROJECT_DIR/testing/integration"
    cargo build --release --bin fastevm-test

    if [ ! -f "$PROJECT_DIR/target/release/fastevm-test" ]; then
        log "ERROR: Build failed - binary not found"
        exit 1
    fi

    # Install binary
    log "Installing test binary..."
    sudo cp "$PROJECT_DIR/target/release/fastevm-test" /usr/local/bin/
    sudo chmod +x /usr/local/bin/fastevm-test
    
    # Backup the binary for future use
    log "Backing up binary for future deployments..."
    sudo mkdir -p /opt/fastevm-binaries
    sudo cp "$PROJECT_DIR/target/release/fastevm-test" /opt/fastevm-binaries/
    sudo chown ubuntu:ubuntu /opt/fastevm-binaries/fastevm-test
    chmod +x /opt/fastevm-binaries/fastevm-test
fi

# Create completion markers
touch /home/ubuntu/client-bootstrap-complete
touch /home/ubuntu/client-config-complete

log "FastEVM client node setup completed successfully!"
log "Bootstrap and configuration phases completed"
EOF

    # Replace placeholders
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        sed -i "" "s|GITHUB_REPO_PLACEHOLDER|${GITHUB_REPO}|g" "${CONFIG_DIR}/setup.sh"
        sed -i "" "s|GITHUB_BRANCH_PLACEHOLDER|${GITHUB_BRANCH}|g" "${CONFIG_DIR}/setup.sh"
    else
        # Linux
        sed -i "s|GITHUB_REPO_PLACEHOLDER|${GITHUB_REPO}|g" "${CONFIG_DIR}/setup.sh"
        sed -i "s|GITHUB_BRANCH_PLACEHOLDER|${GITHUB_BRANCH}|g" "${CONFIG_DIR}/setup.sh"
    fi
    chmod +x "${CONFIG_DIR}/setup.sh"
    
    log_success "Combined setup script generated: ${CONFIG_DIR}/setup.sh"
}

# Main execution
main() {
    log_info "Starting FastEVM client node configuration preparation..."
    
    detect_rpc_urls
    create_config_structure
    update_shared_env
    generate_setup_script
    
    log_success "Client node configuration preparation completed!"
    log_success "Next steps: make deploy-configs → make setup"
}

main "$@"
