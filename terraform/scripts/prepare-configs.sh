#!/bin/bash
# FastEVM Local Configuration Preparation Script
# This script prepares all configurations locally before deploying to remote nodes

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FASTEVM_DIR="$(dirname "$PROJECT_ROOT")"
CONFIG_DIR="${PROJECT_ROOT}/config"
DEPLOY_DIR="${PROJECT_ROOT}/deploy"
NODE_COUNT=${NODE_COUNT:-4}
BASE_IP=${BASE_IP:-"10.0.0"}
START_IP=${START_IP:-10}
PROJECT_NAME=${PROJECT_NAME:-"fastevm"}
GITHUB_REPO=${GITHUB_REPO:-"https://github.com/scalarorg/fastevm.git"}
GITHUB_BRANCH=${GITHUB_BRANCH:-"main"}

# Prefunded accounts configuration
PREFUND_ACCOUNT_COUNT=${PREFUND_ACCOUNT_COUNT:-100000}
PREFUND_BALANCE=${PREFUND_BALANCE:-"1000000000000000000000"}
TEST_MNEMONIC=${TEST_MNEMONIC:-"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"}

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

# Function to detect Terraform deployment information
detect_terraform_info() {
    log_info "Detecting Terraform deployment information..."
    
    # Check if we're in a Terraform directory
    if [ ! -f "main.tf" ]; then
        log_warning "No main.tf found. Using default values."
        return 0
    fi
    
    # Check if terraform state exists
    if [ ! -f "terraform.tfstate" ] && [ ! -d ".terraform" ]; then
        log_warning "No Terraform state found. Using default values."
        return 0
    fi
    
    # Try to get instance information from Terraform
    if command -v terraform >/dev/null 2>&1; then
        log_info "Querying Terraform state for instance information..."
        
        # Get instance IPs from Terraform output (prefer external IPs)
        if terraform output -json >/dev/null 2>&1; then
            # Try external IPs first
            INSTANCE_IPS=$(terraform output -json | jq -r '.instance_external_ips.value[]' 2>/dev/null || echo "")
            if [ -z "$INSTANCE_IPS" ]; then
                # Fallback to internal IPs if external not available
                INSTANCE_IPS=$(terraform output -json | jq -r '.instance_ips.value[]' 2>/dev/null || echo "")
            fi
            
            if [ -n "$INSTANCE_IPS" ]; then
                NODE_COUNT=$(echo "$INSTANCE_IPS" | wc -l | tr -d ' ')
                log_success "Detected $NODE_COUNT nodes from Terraform state"
                
                # Convert to array
                NODE_IPS_ARRAY=()
                while IFS= read -r ip; do
                    if [ -n "$ip" ]; then
                        NODE_IPS_ARRAY+=("$ip")
                    fi
                done <<< "$INSTANCE_IPS"
                
                log_info "Using IPs from Terraform state: ${NODE_IPS_ARRAY[*]}"
                
                # Get project name from Terraform
                PROJECT_NAME_TF=$(terraform output -json | jq -r '.project_name.value' 2>/dev/null || echo "")
                if [ -n "$PROJECT_NAME_TF" ] && [ "$PROJECT_NAME_TF" != "null" ]; then
                    PROJECT_NAME="$PROJECT_NAME_TF"
                    log_info "Detected project name: $PROJECT_NAME"
                fi
                
                return 0
            fi
        fi
        
        # Fallback: try to get from terraform.tfvars
        if [ -f "terraform.tfvars" ]; then
            log_info "Reading configuration from terraform.tfvars..."
            NODE_COUNT_TF=$(grep -E '^\s*node_count\s*=' terraform.tfvars | cut -d'=' -f2 | tr -d ' "')
            PROJECT_NAME_TF=$(grep -E '^\s*project_name\s*=' terraform.tfvars | cut -d'=' -f2 | tr -d ' "')
            
            if [ -n "$NODE_COUNT_TF" ]; then
                NODE_COUNT="$NODE_COUNT_TF"
                log_info "Detected node count from terraform.tfvars: $NODE_COUNT"
            fi
            
            if [ -n "$PROJECT_NAME_TF" ]; then
                PROJECT_NAME="$PROJECT_NAME_TF"
                log_info "Detected project name from terraform.tfvars: $PROJECT_NAME"
            fi
        fi
        
        # Fallback: try to get from variables.tf
        if [ -f "variables.tf" ]; then
            log_info "Reading default values from variables.tf..."
            NODE_COUNT_DEFAULT=$(grep -A 5 'variable "node_count"' variables.tf | grep 'default' | cut -d'=' -f2 | tr -d ' "')
            PROJECT_NAME_DEFAULT=$(grep -A 5 'variable "project_name"' variables.tf | grep 'default' | cut -d'=' -f2 | tr -d ' "')
            
            if [ -n "$NODE_COUNT_DEFAULT" ]; then
                NODE_COUNT="$NODE_COUNT_DEFAULT"
                log_info "Using default node count from variables.tf: $NODE_COUNT"
            fi
            
            if [ -n "$PROJECT_NAME_DEFAULT" ]; then
                PROJECT_NAME="$PROJECT_NAME_DEFAULT"
                log_info "Using default project name from variables.tf: $PROJECT_NAME"
            fi
        fi
    else
        log_warning "Terraform command not found. Using default values."
    fi
}

# Function to get node IPs from Terraform or generate them
get_node_ips() {
    if [ ${#NODE_IPS_ARRAY[@]} -gt 0 ]; then
        # Use detected IPs from Terraform
        NODE_IPS=("${NODE_IPS_ARRAY[@]}")
        log_info "Using IPs from Terraform state: ${NODE_IPS[*]}"
    else
        # Generate IPs based on detected or default values
        log_info "Generating IPs based on detected configuration..."
        NODE_IPS=()
        for i in $(seq 0 $((NODE_COUNT - 1))); do
            NODE_IPS+=("$BASE_IP.$((START_IP + i))")
        done
        log_info "Generated IPs: ${NODE_IPS[*]}"
    fi
}

# Help function
show_help() {
    cat << EOF
FastEVM Local Configuration Preparation

Usage: $0 [OPTIONS]

This script automatically detects Terraform deployment information when possible,
including node count, IP addresses, and project name. Manual overrides are available.

AUTO-DETECTION:
    The script will automatically detect:
    - Node count and IPs from Terraform state (if available)
    - Project name from Terraform outputs
    - Configuration from terraform.tfvars
    - Default values from variables.tf

OPTIONS:
    -n, --nodes COUNT        Number of nodes (overrides auto-detection)
    -b, --base-ip IP        Base IP address (only used when --no-auto-detect)
    -s, --start-ip IP        Starting IP offset (only used when --no-auto-detect)
    -p, --project NAME      Project name (overrides auto-detection)
    -r, --repo URL          GitHub repository URL
    -br, --branch BRANCH    GitHub branch (default: main)
    -o, --output DIR        Output directory (default: ../config)
    -d, --deploy DIR        Deploy directory (default: ../deploy)
    --no-auto-detect        Disable auto-detection, use defaults only
    -h, --help              Show this help message

EXAMPLES:
    $0                                    # Auto-detect everything
    $0 -n 6                              # Override node count only
    $0 --no-auto-detect -n 4 -b 192.168.1 # Disable auto-detection
    $0 --output /custom/path              # Custom output directory

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -n|--nodes)
            NODE_COUNT="$2"
            shift 2
            ;;
        -b|--base-ip)
            BASE_IP="$2"
            shift 2
            ;;
        -s|--start-ip)
            START_IP="$2"
            shift 2
            ;;
        -p|--project)
            PROJECT_NAME="$2"
            shift 2
            ;;
        -r|--repo)
            GITHUB_REPO="$2"
            shift 2
            ;;
        -br|--branch)
            GITHUB_BRANCH="$2"
            shift 2
            ;;
        -o|--output)
            CONFIG_DIR="$2"
            shift 2
            ;;
        -d|--deploy)
            DEPLOY_DIR="$2"
            shift 2
            ;;
        --no-auto-detect)
            NO_AUTO_DETECT=true
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

# Auto-detect Terraform information unless disabled
if [ "$NO_AUTO_DETECT" != "true" ]; then
    detect_terraform_info
fi

# Get node IPs
get_node_ips

# Validate inputs
if ! [[ "$NODE_COUNT" =~ ^[0-9]+$ ]] || [ "$NODE_COUNT" -lt 1 ]; then
    log_error "Node count must be a positive integer"
    exit 1
fi

if ! [[ "$START_IP" =~ ^[0-9]+$ ]] || [ "$START_IP" -lt 1 ]; then
    log_error "Start IP must be a positive integer"
    exit 1
fi

log_info "Preparing FastEVM configurations for $NODE_COUNT nodes"
if [ "$NO_AUTO_DETECT" = "true" ] || [ ${#NODE_IPS_ARRAY[@]} -eq 0 ]; then
    log_info "Base IP: $BASE_IP"
    log_info "Start IP: $START_IP"
fi
log_info "Project Name: $PROJECT_NAME"
log_info "GitHub Repo: $GITHUB_REPO"
log_info "GitHub Branch: $GITHUB_BRANCH"
log_info "Config Directory: $CONFIG_DIR"
log_info "Deploy Directory: $DEPLOY_DIR"
log_info "Node IPs: ${NODE_IPS[*]}"

# Create directories
mkdir -p "$CONFIG_DIR"
mkdir -p "$DEPLOY_DIR"

# Copy genesis.json from execution-client/shared directory
log_info "Copying genesis.json from execution-client/shared..."
if [ -f "$FASTEVM_DIR/execution-client/shared/genesis.json" ]; then
    cp "$FASTEVM_DIR/execution-client/shared/genesis.json" "$CONFIG_DIR/"
    log_success "Genesis file copied successfully"
else
    log_error "Genesis file not found at $FASTEVM_DIR/execution-client/shared/genesis.json"
    log_error "Please ensure the execution-client/shared/genesis.json file exists"
    exit 1
fi

# Generate node IPs and ports
log_info "Generating node network configuration..."
declare -a HTTP_PORTS
declare -a WS_PORTS
declare -a ENGINE_PORTS
declare -a CONSENSUS_PORTS
declare -a P2P_PORTS

for i in $(seq 0 $((NODE_COUNT - 1))); do
    HTTP_PORTS[$i]=8545
    WS_PORTS[$i]=8546
    ENGINE_PORTS[$i]=8551
    CONSENSUS_PORTS[$i]=26657
    P2P_PORTS[$i]=30303
done

# Generate JWT secrets
log_info "Generating JWT secrets..."
for i in $(seq 0 $((NODE_COUNT - 1))); do
    JWT_SECRET=$(openssl rand -hex 32 2>/dev/null || echo "placeholder-jwt-secret-$i")
    echo "0x$JWT_SECRET" > "$CONFIG_DIR/jwt$i.hex"
done
log_success "Generated JWT secrets for all nodes"

# Generate committees configuration
log_info "Generating committees configuration..."
cat > "$CONFIG_DIR/committees.yml" << EOF
epoch: 0
authorities:
EOF

for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_IP="${NODE_IPS[$i]}"
    CONSENSUS_PORT="${CONSENSUS_PORTS[$i]}"
    
    cat >> "$CONFIG_DIR/committees.yml" << EOF
- index: $i
  stake: 1000
  hostname: ${PROJECT_NAME}-consensus$i
  address: /ip4/$NODE_IP/udp/$CONSENSUS_PORT
  authority_key: AuthorityPublicKey(placeholder-authority-$i)
  protocol_key: ProtocolPublicKey(placeholder-protocol-$i)
  network_key: NetworkPublicKey(placeholder-network-$i)
EOF
done

cat >> "$CONFIG_DIR/committees.yml" << EOF
docker_network:
  base_ip: $BASE_IP
  start_ip: $START_IP
  end_ip: $((START_IP + NODE_COUNT - 1))
  port: 26657
quorum_threshold: $NODE_COUNT
validity_threshold: $NODE_COUNT
EOF
log_success "Generated committees configuration"

# Generate parameters configuration
log_info "Generating parameters configuration..."
cat > "$CONFIG_DIR/parameters.yml" << EOF
leader_timeout: {
  secs: 0,
  nanos: 200000000
}
min_round_delay: {
  secs: 0,
  nanos: 100000000
}
max_forward_time_drift: {
  secs: 0,
  nanos: 500000000
}
max_blocks_per_sync: 32
max_blocks_per_fetch: 1000
sync_last_known_own_block_timeout: {
  secs: 5,
  nanos: 0
}
round_prober_interval_ms: 5000
round_prober_request_timeout_ms: 4000
propagation_delay_stop_proposal_threshold: 5
dag_state_cached_rounds: 500
commit_sync_parallel_fetches: 8
commit_sync_batch_size: 100
commit_sync_batches_ahead: 32
EOF
log_success "Generated parameters configuration"

# Generate individual node configurations
log_info "Generating individual node configurations..."
for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_IP="${NODE_IPS[$i]}"
    HTTP_PORT="${HTTP_PORTS[$i]}"
    WS_PORT="${WS_PORTS[$i]}"
    ENGINE_PORT="${ENGINE_PORTS[$i]}"
    CONSENSUS_PORT="${CONSENSUS_PORTS[$i]}"
    P2P_PORT="${P2P_PORTS[$i]}"
    
    # Generate peer addresses (exclude current node)
    PEER_ADDRESSES=""
    for j in $(seq 0 $((NODE_COUNT - 1))); do
        if [ $j -ne $i ]; then
            PEER_IP="${NODE_IPS[$j]}"
            PEER_PORT="${CONSENSUS_PORTS[$j]}"
            PEER_ADDRESSES="$PEER_ADDRESSES,/ip4/$PEER_IP/udp/$PEER_PORT"
        fi
    done
    PEER_ADDRESSES=$(echo $PEER_ADDRESSES | sed 's/^,//')
    
    # Read JWT secret
    JWT_SECRET=$(cat "$CONFIG_DIR/jwt$i.hex")
    
    # Create consensus client configuration
    cat > "$CONFIG_DIR/node$i.yml" << EOF
# Node $i configuration for FastEVM Consensus Client
chain: "./genesis.json"

# Committee configuration
committee_path: "./committees.yml"
parameters_path: "./parameters.yml"

# Execution client configuration
execution_http_url: "http://127.0.0.1:$HTTP_PORT"
execution_ws_url: "ws://127.0.0.1:$WS_PORT"
jwt_secret: "$JWT_SECRET"
genesis_block_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
genesis_time: 1755000000
fee_recipient: "0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6"

# Database configuration
database_path: "./consensus/db"

# Network configuration
poll_interval: 30000
max_retries: 3
timeout: 30

# Node configuration
working_directory: "."
node_index: $i
log_level: "info"

# Peer addresses
peer_addresses: [$PEER_ADDRESSES]
EOF
    
    # Create execution client configuration
    BOOTNODES=""
    for j in $(seq 0 $((NODE_COUNT - 1))); do
        if [ $j -ne $i ]; then
            BOOTNODE_IP="${NODE_IPS[$j]}"
            BOOTNODE_PORT="${P2P_PORTS[$j]}"
            BOOTNODES="$BOOTNODES,enode://$BOOTNODE_IP:$BOOTNODE_PORT"
        fi
    done
    BOOTNODES=$(echo $BOOTNODES | sed 's/^,//')
    
    cat > "$CONFIG_DIR/execution$i.toml" << EOF
[network]
port = $P2P_PORT
discovery.port = $P2P_PORT
discovery.addr = "0.0.0.0"
bootnodes = [$BOOTNODES]

[http]
enabled = true
port = $HTTP_PORT
addr = "0.0.0.0"
api = ["eth", "net", "web3", "admin", "debug"]
corsdomain = "*"

[ws]
enabled = true
port = $WS_PORT
addr = "0.0.0.0"
api = ["eth", "net", "web3", "admin", "debug"]
origins = "*"

[authrpc]
enabled = true
port = $ENGINE_PORT
addr = "0.0.0.0"
jwtsecret = "./jwt.hex"

[chain]
chain = "./genesis.json"

[datadir]
path = "./data"

[txpool]
enabled = true
EOF
    
    log_success "Generated configuration for node $i"
done

# Generate deployment packages
log_info "Generating deployment packages..."
for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_DIR="$DEPLOY_DIR/node$i"
    mkdir -p "$NODE_DIR"
    
    # Copy node-specific files
    cp "$CONFIG_DIR/node$i.yml" "$NODE_DIR/node.yml"
    cp "$CONFIG_DIR/execution$i.toml" "$NODE_DIR/execution.toml"
    cp "$CONFIG_DIR/jwt$i.hex" "$NODE_DIR/jwt.hex"
    
    # Copy shared files
    cp "$CONFIG_DIR/genesis.json" "$NODE_DIR/"
    cp "$CONFIG_DIR/committees.yml" "$NODE_DIR/"
    cp "$CONFIG_DIR/parameters.yml" "$NODE_DIR/"
    
    # Generate node-specific environment file
    cat > "$NODE_DIR/node.env" << EOF
# FastEVM Node $i Environment Configuration
NODE_INDEX=$i
NODE_COUNT=$NODE_COUNT
PROJECT_NAME="$PROJECT_NAME"
GITHUB_REPO="$GITHUB_REPO"
GITHUB_BRANCH="$GITHUB_BRANCH"

# Prefunded accounts configuration
PREFUND_ACCOUNT_COUNT=$PREFUND_ACCOUNT_COUNT
PREFUND_BALANCE="$PREFUND_BALANCE"
TEST_MNEMONIC="$TEST_MNEMONIC"

# Network configuration
NODE_IP="$NODE_IP"
HTTP_PORT=$HTTP_PORT
WS_PORT=$WS_PORT
ENGINE_PORT=$ENGINE_PORT
CONSENSUS_PORT=$CONSENSUS_PORT
P2P_PORT=$P2P_PORT
EOF
    
    # Copy service.sh to each node directory
    cp "$SCRIPT_DIR/service.sh" "$NODE_DIR/"
    
    # Create deploy.sh script that uses service.sh
cat > "$NODE_DIR/deploy.sh" << 'EOF'
#!/bin/bash
# FastEVM Node Deployment Script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Get node index from environment or default to 0
NODE_INDEX=${NODE_INDEX:-0}

# Set environment variables
NODE_COUNT=$NODE_COUNT
PROJECT_NAME="$PROJECT_NAME"
GITHUB_REPO="${GITHUB_REPO:-https://github.com/scalarorg/fastevm.git}"
GITHUB_BRANCH="${GITHUB_BRANCH:-main}"

# Debug information
log_info "Environment variables:"
log_info "  NODE_INDEX: $NODE_INDEX"
log_info "  NODE_COUNT: $NODE_COUNT"
log_info "  PROJECT_NAME: $PROJECT_NAME"
log_info "  GITHUB_REPO: $GITHUB_REPO"
log_info "  GITHUB_BRANCH: $GITHUB_BRANCH"

# Load environment variables from node.env file
if [ -f "/tmp/fastevm-config/node.env" ]; then
    log_info "Loading environment variables from node.env..."
    source /tmp/fastevm-config/node.env
    log_info "Environment variables loaded successfully"
else
    log_warning "node.env file not found, using defaults"
fi

log_info "Starting FastEVM node $NODE_INDEX deployment..."

# Update system packages
log_info "Updating system packages..."
sudo apt-get update -y
sudo apt-get upgrade -y

# Install required packages
log_info "Installing required packages..."
sudo apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    cmake \
    jq \
    htop \
    vim \
    unzip \
    software-properties-common \
    apt-transport-https \
    ca-certificates \
    gnupg \
    lsb-release

# Install Docker
log_info "Installing Docker..."
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --batch --yes --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
sudo apt-get update -y
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

# Binary building is now handled by prepare-binaries.sh
# Just create the necessary directory structure
log_info "Setting up FastEVM directory structure..."

# Create FastEVM directory structure
FASTEVM_DIR="/opt/fastevm"
sudo mkdir -p $FASTEVM_DIR
sudo mkdir -p /opt/fastevm-binaries
sudo chown -R ubuntu:ubuntu $FASTEVM_DIR
sudo chown -R ubuntu:ubuntu /opt/fastevm-binaries

log_info "Directory structure created. Binaries will be prepared by prepare-binaries.sh"

# Add docker group and user
sudo usermod -aG docker ubuntu

# Stop services before updating binaries
log_info "Stopping services before updating binaries..."
if systemctl is-active --quiet fastevm-execution; then
    log_info "Stopping execution client..."
    sudo systemctl stop fastevm-execution
fi

if systemctl is-active --quiet fastevm-consensus; then
    log_info "Stopping consensus client..."
    sudo systemctl stop fastevm-consensus
fi

# Wait a moment for services to stop
sleep 5

# Binary installation is now handled by prepare-binaries.sh
log_info "Skipping binary installation during deployment..."
log_info "Binaries will be installed by prepare-binaries.sh"

# Create data directories
log_info "Creating data directories..."
sudo mkdir -p /data/execution
sudo mkdir -p /data/execution/p2p
sudo mkdir -p /data/execution/db
sudo mkdir -p /data/consensus
sudo mkdir -p /data/consensus/db
sudo mkdir -p /data/logs
sudo mkdir -p /data/config

# Set proper ownership for database directories
sudo chown -R ubuntu:ubuntu /data/execution
sudo chown -R ubuntu:ubuntu /data/consensus

# Generate JWT secret
log_info "Generating JWT secret..."
openssl rand -hex 32 | tr -d '\n' | sudo tee /data/execution/jwt.hex > /dev/null

# Generate P2P secret key
log_info "Generating P2P secret key..."
NODE_SEED="fastevm-node-${NODE_INDEX}-p2p-secret-2025"
echo "$NODE_SEED" | openssl dgst -sha256 -hex | cut -d' ' -f2 | tr -d '\n' | sudo tee /data/execution/p2p/secret.key > /dev/null

# Generate peer ID from secret key
log_info "Generating peer ID..."
if [ -f "/usr/local/bin/cli" ]; then
    # Set proper permissions for CLI to write
    sudo chown -R ubuntu:ubuntu /data/execution/p2p
    sudo -u ubuntu /usr/local/bin/cli show-peer-id --file /data/execution/p2p/secret.key --output /data/execution/p2p/secret.hex
    # Remove 0x prefix if present
    sudo sed -i 's/^0x//' /data/execution/p2p/secret.hex
else
    # Fallback: use the secret key directly as peer ID
    sudo cp /data/execution/p2p/secret.key /data/execution/p2p/secret.hex
fi

# Copy configuration files
log_info "Copying configuration files..."
sudo cp /tmp/fastevm-config/* /data/

# Initialize execution node with genesis (only if binary exists)
# log_info "Checking for execution node initialization..."
# if [ -f "/data/genesis.json" ] && [ -f "/usr/local/bin/fastevm-execution" ]; then
#     log_info "Initializing execution node with genesis..."
#     /usr/local/bin/fastevm-execution init --datadir /data/execution --chain /data/genesis.json || log_info "Execution node initialization completed"
# else
#     log_info "Skipping execution node initialization (binary not available yet)"
#     log_info "Initialization will be done after binary installation"
# fi

# Install systemd services
log_info "Installing systemd services..."
sudo bash /tmp/fastevm-config/service.sh install

# No need to set NODE_INDEX since we're using fixed ports

# Set proper permissions
log_info "Setting permissions..."
sudo chown -R ubuntu:ubuntu /data

# Ensure database directories have proper permissions
log_info "Setting database permissions..."
sudo chmod -R 755 /data/execution/db
sudo chmod -R 755 /data/consensus/db

# Create completion marker
echo "FastEVM node $NODE_INDEX deployment completed successfully at $(date)" | sudo tee /var/log/fastevm-deployment-complete

log_success "=== FastEVM Node $NODE_INDEX Deployment Completed Successfully ==="
log_success "Services installed but not started. Use 'make start-services' to start all nodes."
log_info "Use 'fastevm-status' to check status"
log_info "Use 'fastevm-health-check' to verify health"
EOF
    
    # Make scripts executable
    chmod +x "$NODE_DIR/deploy.sh"
    chmod +x "$NODE_DIR/service.sh"
    
    log_success "Generated deployment package for node $i"
done

# Generate Docker Compose configuration
log_info "Generating Docker Compose configuration..."
cat > "$CONFIG_DIR/docker-compose.yml" << EOF
version: '3.8'

services:
EOF

for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_IP="${NODE_IPS[$i]}"
    HTTP_PORT="${HTTP_PORTS[$i]}"
    WS_PORT="${WS_PORTS[$i]}"
    ENGINE_PORT="${ENGINE_PORTS[$i]}"
    CONSENSUS_PORT="${CONSENSUS_PORTS[$i]}"
    P2P_PORT="${P2P_PORTS[$i]}"
    
    cat >> "$CONFIG_DIR/docker-compose.yml" << EOF
  execution-node$i:
    image: scalarorg/fastevm-execution:latest
    container_name: ${PROJECT_NAME}-execution$i
    hostname: execution$i
    ports:
      - "$ENGINE_PORT:8551"
      - "$HTTP_PORT:8545"
      - "$WS_PORT:8546"
      - "$P2P_PORT:30303/tcp"
      - "$P2P_PORT:30303/udp"
    volumes:
      - execution-data$i:/data
      - ./execution$i.toml:/data/execution.toml
      - ./genesis.json:/data/genesis.json
      - ./jwt$i.hex:/data/jwt.hex
    networks:
      fastevm-network:
        ipv4_address: $NODE_IP
    restart: unless-stopped

  consensus-node$i:
    image: scalarorg/fastevm-consensus:latest
    container_name: ${PROJECT_NAME}-consensus$i
    hostname: consensus$i
    ports:
      - "26657:26657"
    volumes:
      - consensus-data$i:/app/data
      - ./node$i.yml:/app/data/node.yml
      - ./committees.yml:/app/data/committees.yml
      - ./parameters.yml:/app/data/parameters.yml
      - ./genesis.json:/app/data/genesis.json
    networks:
      fastevm-network:
        ipv4_address: $NODE_IP
    depends_on:
      - execution-node$i
    restart: unless-stopped
EOF
done

cat >> "$CONFIG_DIR/docker-compose.yml" << EOF

networks:
  fastevm-network:
    driver: bridge
    ipam:
      config:
        - subnet: $BASE_IP.0/24

volumes:
EOF

for i in $(seq 0 $((NODE_COUNT - 1))); do
    cat >> "$CONFIG_DIR/docker-compose.yml" << EOF
  execution-data$i:
  consensus-data$i:
EOF
done

log_success "Generated docker-compose.yml"

# Generate network summary
log_info "Generating network summary..."
cat > "$CONFIG_DIR/network-summary.txt" << EOF
FastEVM Network Configuration Summary
=====================================

Node Configuration:
EOF

for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_IP="${NODE_IPS[$i]}"
    HTTP_PORT="${HTTP_PORTS[$i]}"
    WS_PORT="${WS_PORTS[$i]}"
    ENGINE_PORT="${ENGINE_PORTS[$i]}"
    CONSENSUS_PORT="${CONSENSUS_PORTS[$i]}"
    P2P_PORT="${P2P_PORTS[$i]}"
    
    cat >> "$CONFIG_DIR/network-summary.txt" << EOF
Node $i:
  IP: $NODE_IP
  HTTP RPC: $HTTP_PORT
  WebSocket RPC: $WS_PORT
  Engine API: $ENGINE_PORT
  Consensus API: $CONSENSUS_PORT
  P2P Port: $P2P_PORT
EOF
done

cat >> "$CONFIG_DIR/network-summary.txt" << EOF

RPC Endpoints:
EOF

for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_IP="${NODE_IPS[$i]}"
    HTTP_PORT="${HTTP_PORTS[$i]}"
    ENGINE_PORT="${ENGINE_PORTS[$i]}"
    CONSENSUS_PORT="${CONSENSUS_PORTS[$i]}"
    
    cat >> "$CONFIG_DIR/network-summary.txt" << EOF
Node $i:
  HTTP RPC: http://$NODE_IP:$HTTP_PORT
  Engine API: http://$NODE_IP:$ENGINE_PORT
  Consensus API: http://$NODE_IP:$CONSENSUS_PORT
EOF
done
log_success "Generated network summary"

# Generate deployment script
log_info "Generating deployment script..."
cat > "$DEPLOY_DIR/deploy-all.sh" << EOF
#!/bin/bash
# FastEVM Multi-Node Deployment Script

set -e

NODE_COUNT=$NODE_COUNT
PROJECT_NAME="$PROJECT_NAME"

# Node IPs array
NODE_IPS=($(printf '"%s" ' "${NODE_IPS[@]}"))

# SSH key configuration
SSH_KEY_PATH="../fastevm-deploy-key"
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "\${BLUE}[INFO]\${NC} \$1"
}

log_success() {
    echo -e "\${GREEN}[SUCCESS]\${NC} \$1"
}

log_error() {
    echo -e "\${RED}[ERROR]\${NC} \$1"
}

# Check if SSH key exists
check_ssh_key() {
    if [ ! -f "\$SSH_KEY_PATH" ]; then
        log_error "SSH key not found at \$SSH_KEY_PATH"
        log_error "Please run 'terraform apply' first to generate SSH keys"
        exit 1
    fi
    
    # Set proper permissions
    chmod 600 "\$SSH_KEY_PATH"
    log_info "Using SSH key: \$SSH_KEY_PATH"
}

# Function to deploy a single node
deploy_node() {
    local node_index=\$1
    local node_ip="\${NODE_IPS[\$node_index]}"
    
    log_info "Deploying node \$node_index (\$node_ip)..."
    
    # Copy configuration files to node
    scp \$SSH_OPTS -i "\$SSH_KEY_PATH" -r node\$node_index/* ubuntu@\$node_ip:/tmp/fastevm-config/
    
    # Run deployment script on node (environment variables loaded from node.env)
    ssh \$SSH_OPTS -i "\$SSH_KEY_PATH" ubuntu@\$node_ip "bash /tmp/fastevm-config/deploy.sh"
    
    log_success "Node \$node_index deployment completed"
}

# Check SSH key first
check_ssh_key

# Deploy all nodes
log_info "Starting deployment of \$NODE_COUNT nodes..."

for i in \$(seq 0 \$((NODE_COUNT - 1))); do
    deploy_node \$i &
done

# Wait for all deployments to complete
wait

log_success "All nodes deployed successfully!"

# Test connectivity
log_info "Testing node connectivity..."
for i in \$(seq 0 \$((NODE_COUNT - 1))); do
    node_ip="\${NODE_IPS[\$i]}"
    http_port=8545
    
    log_info "Testing node \$i (\$node_ip:$http_port)..."
    if curl -s -f "http://\$node_ip:\$http_port" > /dev/null; then
        log_success "Node \$i is responding"
    else
        log_error "Node \$i is not responding"
    fi
done

log_success "Deployment completed!"
EOF

chmod +x "$DEPLOY_DIR/deploy-all.sh"

# Generate README
log_info "Generating README..."
cat > "$CONFIG_DIR/README.md" << EOF
# FastEVM Network Configuration

This directory contains the prepared configuration files for a FastEVM network with $NODE_COUNT nodes.

## Files Generated

### Shared Configuration
- \`genesis.json\` - Genesis block configuration
- \`committees.yml\` - Consensus committee configuration
- \`parameters.yml\` - Consensus parameters
- \`docker-compose.yml\` - Docker Compose configuration
- \`network-summary.txt\` - Network configuration summary

### Node-Specific Configuration
- \`node0.yml\` to \`node$((NODE_COUNT-1)).yml\` - Individual consensus node configurations
- \`execution0.toml\` to \`execution$((NODE_COUNT-1)).toml\` - Individual execution node configurations
- \`jwt0.hex\` to \`jwt$((NODE_COUNT-1)).hex\` - JWT secrets for each node

### Deployment Packages
- \`../deploy/node0/\` to \`../deploy/node$((NODE_COUNT-1))/\` - Individual node deployment packages
- \`../deploy/deploy-all.sh\` - Script to deploy all nodes

## Network Configuration

- **Node Count**: $NODE_COUNT
- **Base IP**: $BASE_IP
- **IP Range**: $BASE_IP.$START_IP - $BASE_IP.$((START_IP + NODE_COUNT - 1))
- **Project Name**: $PROJECT_NAME
- **GitHub Repo**: $GITHUB_REPO
- **GitHub Branch**: $GITHUB_BRANCH

## Deployment Process

### 1. Prepare Configurations (This Step)
\`\`\`bash
# Generate all configurations
./scripts/prepare-configs.sh -n $NODE_COUNT -b $BASE_IP -s $START_IP
\`\`\`

### 2. Deploy to Remote Nodes
\`\`\`bash
# Deploy all nodes
cd ../deploy
./deploy-all.sh
\`\`\`

### 3. Deploy with Docker Compose (Alternative)
\`\`\`bash
# Deploy using Docker Compose
cd config
docker-compose up -d
\`\`\`

### 4. Verify Deployment
\`\`\`bash
# Test RPC endpoints
for i in \$(seq 0 $((NODE_COUNT-1))); do
    node_ip="$BASE_IP.$((START_IP + i))"
    http_port=8545
    echo "Testing node \$i: http://\$node_ip:\$http_port"
    curl -X POST http://\$node_ip:\$http_port \\
      -H "Content-Type: application/json" \\
      -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
done
\`\`\`

## Manual Deployment

If you prefer to deploy nodes individually:

\`\`\`bash
# Deploy specific node
scp -r deploy/node0/* ubuntu@$BASE_IP.$START_IP:/tmp/fastevm-config/
ssh ubuntu@$BASE_IP.$START_IP 'bash /tmp/fastevm-config/deploy.sh'
\`\`\`

## RPC Endpoints

EOF

for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_IP="${NODE_IPS[$i]}"
    HTTP_PORT="${HTTP_PORTS[$i]}"
    ENGINE_PORT="${ENGINE_PORTS[$i]}"
    CONSENSUS_PORT="${CONSENSUS_PORTS[$i]}"
    
    cat >> "$CONFIG_DIR/README.md" << EOF
### Node $i
- **HTTP RPC**: http://$NODE_IP:$HTTP_PORT
- **Engine API**: http://$NODE_IP:$ENGINE_PORT
- **Consensus API**: http://$NODE_IP:$CONSENSUS_PORT

EOF
done

cat >> "$CONFIG_DIR/README.md" << EOF
## Security Notes

- JWT secrets are generated for each node
- Each node has isolated data volumes
- Network is isolated to the specified subnet
- Change default JWT secrets in production environments

## Troubleshooting

### Check Node Status
\`\`\`bash
ssh ubuntu@$BASE_IP.$START_IP 'fastevm-status'
\`\`\`

### Check Health
\`\`\`bash
ssh ubuntu@$BASE_IP.$START_IP 'fastevm-health-check'
\`\`\`

### View Logs
\`\`\`bash
ssh ubuntu@$BASE_IP.$START_IP 'journalctl -u fastevm-execution -f'
ssh ubuntu@$BASE_IP.$START_IP 'journalctl -u fastevm-consensus -f'
\`\`\`
EOF
log_success "Generated README documentation"

log_success "Configuration preparation completed!"
echo ""
log_info "Generated files in: $CONFIG_DIR"
log_info "Deployment packages in: $DEPLOY_DIR"
echo ""
log_info "Summary:"
echo "  - $NODE_COUNT nodes configured"
echo "  - IP range: $BASE_IP.$START_IP - $BASE_IP.$((START_IP + NODE_COUNT - 1))"
echo "  - All configurations prepared locally"
echo "  - Deployment packages ready"
echo ""
log_info "Next steps:"
echo "  1. Review the generated configuration files"
echo "  2. Deploy using: cd $DEPLOY_DIR && ./deploy-all.sh"
echo "  3. Test the RPC endpoints"
echo "  4. Monitor the network status"
echo ""
log_info "For more information, see: $CONFIG_DIR/README.md"
