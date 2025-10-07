#!/bin/bash
# FastEVM Local Configuration Preparation Script
# This script prepares all configurations locally before deploying to remote nodes

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CONFIG_DIR="${PROJECT_ROOT}/config"
DEPLOY_DIR="${PROJECT_ROOT}/deploy"
NODE_COUNT=${NODE_COUNT:-4}
BASE_IP=${BASE_IP:-"10.0.0"}
START_IP=${START_IP:-10}
PROJECT_NAME=${PROJECT_NAME:-"fastevm"}
GITHUB_REPO=${GITHUB_REPO:-"https://github.com/scalarorg/fastevm.git"}
GITHUB_BRANCH=${GITHUB_BRANCH:-"main"}

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

# Generate node IPs and ports
log_info "Generating node network configuration..."
declare -a HTTP_PORTS
declare -a WS_PORTS
declare -a ENGINE_PORTS
declare -a CONSENSUS_PORTS
declare -a P2P_PORTS

for i in $(seq 0 $((NODE_COUNT - 1))); do
    HTTP_PORTS[$i]=$((8545 + i * 2))
    WS_PORTS[$i]=$((8546 + i * 2))
    ENGINE_PORTS[$i]=$((8551 + i))
    CONSENSUS_PORTS[$i]=$((26657 + i))
    P2P_PORTS[$i]=$((30303 + i))
done

# Generate JWT secrets
log_info "Generating JWT secrets..."
for i in $(seq 0 $((NODE_COUNT - 1))); do
    JWT_SECRET=$(openssl rand -hex 32 2>/dev/null || echo "placeholder-jwt-secret-$i")
    echo "0x$JWT_SECRET" > "$CONFIG_DIR/jwt$i.hex"
done

# Generate genesis file
log_info "Generating genesis file..."
cat > "$CONFIG_DIR/genesis.json" << EOF
{
  "config": {
    "chainId": 1337,
    "homesteadBlock": 0,
    "eip150Block": 0,
    "eip155Block": 0,
    "eip158Block": 0,
    "byzantiumBlock": 0,
    "constantinopleBlock": 0,
    "petersburgBlock": 0,
    "istanbulBlock": 0,
    "berlinBlock": 0,
    "londonBlock": 0,
    "arrowGlacierBlock": 0,
    "grayGlacierBlock": 0,
    "shanghaiTime": 0,
    "cancunTime": 0
  },
  "difficulty": "0x0",
  "gasLimit": "0x1c9c380",
  "alloc": {
    "0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6": {
      "balance": "0x1000000000000000000000000000000000000000000000000000000000000000"
    }
  }
}
EOF

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
    
    # Create deployment script
    cat > "$NODE_DIR/deploy.sh" << EOF
#!/bin/bash
# FastEVM Node $i Deployment Script

set -e

NODE_INDEX=$i
NODE_COUNT=$NODE_COUNT
PROJECT_NAME="$PROJECT_NAME"
GITHUB_REPO="$GITHUB_REPO"
GITHUB_BRANCH="$GITHUB_BRANCH"

echo "=== Deploying FastEVM Node \$NODE_INDEX ==="

# Update system packages
echo "Updating system packages..."
apt-get update -y
apt-get upgrade -y

# Install required packages
echo "Installing required packages..."
apt-get install -y \\
    curl \\
    wget \\
    git \\
    build-essential \\
    pkg-config \\
    libssl-dev \\
    libclang-dev \\
    cmake \\
    jq \\
    htop \\
    vim \\
    unzip \\
    software-properties-common \\
    apt-transport-https \\
    ca-certificates \\
    gnupg \\
    lsb-release

# Install Docker
echo "Installing Docker..."
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
echo "deb [arch=\$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu \$(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
apt-get update -y
apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

# Install Rust
echo "Installing Rust..."
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
export PATH="$HOME/.cargo/bin:$PATH"
rustup default stable
rustup update

# Add docker group and user
usermod -aG docker ubuntu

# Create FastEVM directory
FASTEVM_DIR="/opt/fastevm"
mkdir -p \$FASTEVM_DIR
cd \$FASTEVM_DIR

# Clone the repository
echo "Cloning FastEVM repository..."
git clone \$GITHUB_REPO .
git checkout \$GITHUB_BRANCH

# Build the project
echo "Building FastEVM..."
export PATH="\$HOME/.cargo/bin:\$PATH"
cargo build --release

# Create data directories
echo "Creating data directories..."
mkdir -p /data/execution
mkdir -p /data/consensus
mkdir -p /data/logs
mkdir -p /data/config

# Mount the persistent disk
echo "Mounting persistent disk..."
DISK_DEVICE="/dev/sdb"
if [ -b "\$DISK_DEVICE" ]; then
    # Check if disk is already formatted
    if ! blkid \$DISK_DEVICE; then
        echo "Formatting persistent disk..."
        mkfs.ext4 \$DISK_DEVICE
    fi
    
    # Mount the disk
    echo "Mounting persistent disk to /data..."
    mount \$DISK_DEVICE /data
    echo "\$DISK_DEVICE /data ext4 defaults 0 0" >> /etc/fstab
fi

# Copy configuration files
echo "Copying configuration files..."
cp /tmp/fastevm-config/* /data/

# Create systemd service for execution client
cat > /etc/systemd/system/fastevm-execution.service << 'EOL'
[Unit]
Description=FastEVM Execution Client
After=network.target

[Service]
Type=simple
User=ubuntu
Group=ubuntu
WorkingDirectory=\$FASTEVM_DIR
ExecStart=\$FASTEVM_DIR/target/release/fastevm-execution \\
    --config /data/execution.toml \\
    --datadir /data/execution \\
    --log-level info
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOL

# Create systemd service for consensus client
cat > /etc/systemd/system/fastevm-consensus.service << 'EOL'
[Unit]
Description=FastEVM Consensus Client
After=network.target fastevm-execution.service

[Service]
Type=simple
User=ubuntu
Group=ubuntu
WorkingDirectory=\$FASTEVM_DIR
ExecStart=\$FASTEVM_DIR/target/release/fastevm-consensus \\
    start \\
    --config /data/node.yml
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOL

# Create health check script
cat > /usr/local/bin/fastevm-health-check.sh << 'EOL'
#!/bin/bash
# Health check script for FastEVM nodes

NODE_INDEX=\${1:-0}
HTTP_PORT=\$((8545 + NODE_INDEX * 2))
ENGINE_PORT=\$((8551 + NODE_INDEX))

# Check execution client
if curl -s -f "http://localhost:\$HTTP_PORT" > /dev/null; then
    echo "Execution client healthy"
else
    echo "Execution client unhealthy"
    exit 1
fi

# Check engine API
if curl -s -f "http://localhost:\$ENGINE_PORT" > /dev/null; then
    echo "Engine API healthy"
else
    echo "Engine API unhealthy"
    exit 1
fi

echo "All services healthy"
exit 0
EOL

chmod +x /usr/local/bin/fastevm-health-check.sh

# Create status script
cat > /usr/local/bin/fastevm-status.sh << 'EOL'
#!/bin/bash
# Status script for FastEVM nodes

echo "=== FastEVM Node Status ==="
echo "Node Index: \$NODE_INDEX"
echo "Node IP: \$(hostname -I | awk '{print \$1}')"
echo ""

echo "=== Service Status ==="
systemctl status fastevm-execution --no-pager -l
echo ""
systemctl status fastevm-consensus --no-pager -l
echo ""

echo "=== Network Ports ==="
netstat -tlnp | grep -E "(8545|8551|26657)"
echo ""

echo "=== Recent Logs ==="
journalctl -u fastevm-execution --no-pager -n 10
echo ""
journalctl -u fastevm-consensus --no-pager -n 10
EOL

chmod +x /usr/local/bin/fastevm-status.sh

# Set proper permissions
chown -R ubuntu:ubuntu /data
chown -R ubuntu:ubuntu \$FASTEVM_DIR

# Enable and start services
echo "Enabling and starting services..."
systemctl daemon-reload
systemctl enable fastevm-execution
systemctl enable fastevm-consensus

# Start execution client first
systemctl start fastevm-execution

# Wait for execution client to be ready
echo "Waiting for execution client to be ready..."
sleep 30

# Start consensus client
systemctl start fastevm-consensus

# Create completion marker
echo "FastEVM node \$NODE_INDEX deployment completed successfully at \$(date)" > /var/log/fastevm-deployment-complete

echo "=== FastEVM Node \$NODE_INDEX Deployment Completed Successfully ==="
echo "Services started: fastevm-execution, fastevm-consensus"
echo "Use 'fastevm-status' to check status"
echo "Use 'fastevm-health-check' to verify health"
EOF
    
    chmod +x "$NODE_DIR/deploy.sh"
    
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
      - "$CONSENSUS_PORT:26657"
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
    
    # Run deployment script on node
    ssh \$SSH_OPTS -i "\$SSH_KEY_PATH" ubuntu@\$node_ip 'bash /tmp/fastevm-config/deploy.sh'
    
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
    http_port=\$((8545 + i * 2))
    
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
    http_port=\$((8545 + i * 2))
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
