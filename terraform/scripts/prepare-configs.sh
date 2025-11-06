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

# Load configuration from fastevm.env file if it exists
FASTEVM_ENV_FILE="${PROJECT_ROOT}/fastevm.env"
if [ -f "$FASTEVM_ENV_FILE" ]; then
    echo "Loading configuration from $FASTEVM_ENV_FILE"
    source "$FASTEVM_ENV_FILE"
fi

LOG_LEVEL=${LOG_LEVEL:-vvv}
NODE_COUNT=${NODE_COUNT:-4}
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
            NODE_IPS+=("10.0.0.$((10 + i))")
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
    -p, --project NAME      Project name (overrides auto-detection)
    -r, --repo URL          GitHub repository URL
    -br, --branch BRANCH    GitHub branch (default: main)
    -o, --output DIR        Output directory (default: ../config)
    -d, --deploy DIR        Deploy directory (default: ../deploy)
    -h, --help              Show this help message

EXAMPLES:
    $0                                    # Auto-detect everything
    $0 -n 6                              # Override node count only
    $0 -n 4 -p myproject                 # Override with custom values
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

# Auto-detect Terraform information
detect_terraform_info

# Get node IPs
get_node_ips

# Validate inputs
if ! [[ "$NODE_COUNT" =~ ^[0-9]+$ ]] || [ "$NODE_COUNT" -lt 1 ]; then
    log_error "Node count must be a positive integer"
    exit 1
fi

log_info "Preparing FastEVM configurations for $NODE_COUNT nodes"
if [ ${#NODE_IPS_ARRAY[@]} -eq 0 ]; then
    log_info "Using default IP range: 10.0.0.10 - 10.0.0.$((10 + NODE_COUNT - 1))"
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

# Generate configuration variables (no files created yet)
log_info "Generating configuration variables..."

# Generate JWT secrets
declare -a JWT_SECRETS
for i in $(seq 0 $((NODE_COUNT - 1))); do
    JWT_SECRETS[$i]=$(openssl rand -hex 32 2>/dev/null || echo "placeholder-jwt-secret-$i")
done

# Generate P2P secret keys (peer IDs will be generated later when CLI is available)
declare -a P2P_SECRET_KEYS
for i in $(seq 0 $((NODE_COUNT - 1))); do
    # Generate deterministic secret key (same method as init.sh)
    NODE_SEED="fastevm-node-$((i+1))-p2p-secret-2025"
    P2P_SECRET_KEYS[$i]=$(echo "$NODE_SEED" | openssl dgst -sha256 -binary | openssl dgst -sha256 -hex | cut -d' ' -f2 | tr -d '\n')
done

log_success "Generated P2P secret keys for all nodes (peer IDs will be generated during setup-node)"

# Generate environment files for each node
log_info "Generating environment files for each node..."

for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_IP="${NODE_IPS[$i]}"
    HTTP_PORT="${HTTP_PORTS[$i]}"
    WS_PORT="${WS_PORTS[$i]}"
    ENGINE_PORT="${ENGINE_PORTS[$i]}"
    CONSENSUS_PORT="${CONSENSUS_PORTS[$i]}"
    P2P_PORT="${P2P_PORTS[$i]}"
    
    # Generate peer addresses for consensus
    PEER_ADDRESSES=""
    for j in $(seq 0 $((NODE_COUNT - 1))); do
        if [ $j -ne $i ]; then
            PEER_IP="${NODE_IPS[$j]}"
            PEER_PORT="${CONSENSUS_PORTS[$j]}"
            PEER_ADDRESSES="$PEER_ADDRESSES,/ip4/$PEER_IP/udp/$PEER_PORT"
        fi
    done
    PEER_ADDRESSES=$(echo $PEER_ADDRESSES | sed 's/^,//')
    
    # Generate authorities list for committees.yml
    AUTHORITIES_LIST=""
    for j in $(seq 0 $((NODE_COUNT - 1))); do
        AUTHORITY_IP="${NODE_IPS[$j]}"
        AUTHORITY_PORT="${CONSENSUS_PORTS[$j]}"
        AUTHORITIES_LIST="$AUTHORITIES_LIST
- index: $j
  stake: 1000
  hostname: ${PROJECT_NAME}-consensus$j
  address: /ip4/$AUTHORITY_IP/udp/$AUTHORITY_PORT
  authority_key: AuthorityPublicKey(placeholder-authority-$j)
  protocol_key: ProtocolPublicKey(placeholder-protocol-$j)
  network_key: NetworkPublicKey(placeholder-network-$j)"
    done
    
    # Create node-specific .env file
    cat > "$CONFIG_DIR/node$i.env" << EOF
# FastEVM Node $i Environment Configuration
NODE_INDEX=$i
NODE_COUNT=$NODE_COUNT
PROJECT_NAME="$PROJECT_NAME"
GITHUB_REPO="$GITHUB_REPO"
GITHUB_BRANCH="$GITHUB_BRANCH"

# Network configuration
NODE_IP="$NODE_IP"
HTTP_PORT=$HTTP_PORT
WS_PORT=$WS_PORT
ENGINE_PORT=$ENGINE_PORT
CONSENSUS_PORT=$CONSENSUS_PORT
P2P_PORT=$P2P_PORT

# All node IPs for bootnodes generation
ALL_NODE_IPS="${NODE_IPS[*]}"

# Generated secrets
JWT_SECRET="${JWT_SECRETS[$i]}"
P2P_SECRET_KEY="${P2P_SECRET_KEYS[$i]}"
P2P_PEER_ID="{{PEER_ID_$i}}"

PEER_ADDRESSES="$PEER_ADDRESSES"

# Blockchain configuration
GAS_LIMIT="${BLOCK_GAS_LIMIT}"
SUBDAGS_PER_BLOCK="${SUBDAGS_PER_BLOCK}"

# Logging configuration
LOG_LEVEL="${LOG_LEVEL}"
EOF
    
    log_success "Generated .env file for node $i"
done

# Generate committees.yml file (only once, not per node)
log_info "Generating committees.yml file..."
cat > "$CONFIG_DIR/committees.yml" << EOF
epoch: 0
authorities:
$AUTHORITIES_LIST

docker_network:
  base_ip: 10.0.0
  start_ip: 10
  end_ip: $((10 + NODE_COUNT - 1))
  port: 26657
quorum_threshold: $NODE_COUNT
validity_threshold: $NODE_COUNT
EOF

log_success "Generated committees.yml with correct IP addresses"

# Generate parameters.yml file
log_info "Generating parameters.yml file..."
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

log_success "Generated parameters.yml file"

# Generate deployment packages
log_info "Generating deployment packages..."
for i in $(seq 0 $((NODE_COUNT - 1))); do
    NODE_DIR="$DEPLOY_DIR/node$i"
    mkdir -p "$NODE_DIR"
    
    # Copy node.env file to deployment directory
    cp "$CONFIG_DIR/node$i.env" "$NODE_DIR/node.env"
    
    # Copy shared files
    cp "$CONFIG_DIR/genesis.json" "$NODE_DIR/"
    cp "$CONFIG_DIR/committees.yml" "$NODE_DIR/"
    cp "$CONFIG_DIR/parameters.yml" "$NODE_DIR/"
    
    # Copy configuration templates
    TEMPLATES_DIR="${PROJECT_ROOT}/templates"
    if [ -f "$TEMPLATES_DIR/execution.toml.template" ]; then
        cp "$TEMPLATES_DIR/execution.toml.template" "$NODE_DIR/execution.toml"
        log_info "Copied execution.toml template for node $i"
    else
        log_warning "execution.toml.template not found, skipping for node $i"
    fi
    
    if [ -f "$TEMPLATES_DIR/node.yml.template" ]; then
        cp "$TEMPLATES_DIR/node.yml.template" "$NODE_DIR/node.yml"
        log_info "Copied node.yml template for node $i"
    else
        log_warning "node.yml.template not found, skipping for node $i"
    fi
    
    # Copy scripts
    cp "$SCRIPT_DIR/deploy.sh" "$NODE_DIR/"
    cp "$SCRIPT_DIR/service.sh" "$NODE_DIR/"
    cp "$SCRIPT_DIR/setup-node.sh" "$NODE_DIR/"
    
    # Make scripts executable
    chmod +x "$NODE_DIR/deploy.sh"
    chmod +x "$NODE_DIR/service.sh"
    chmod +x "$NODE_DIR/setup-node.sh"
    
    log_success "Generated deployment package for node $i"
done

# Copy Docker Compose template
log_info "Copying Docker Compose template..."
TEMPLATES_DIR="${PROJECT_ROOT}/templates"
if [ -f "$TEMPLATES_DIR/docker-compose.yml" ]; then
    cp "$TEMPLATES_DIR/docker-compose.yml" "$CONFIG_DIR/docker-compose.yml"
    log_success "Docker Compose template copied"
else
    log_error "Docker Compose template not found at $TEMPLATES_DIR/docker-compose.yml"
    exit 1
fi

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
- \`docker-compose.yml.template\` - Docker Compose template (use replace-templates.sh to generate docker-compose.yml)
- \`network-summary.txt\` - Network configuration summary

### Node-Specific Configuration
- \`node0.env\` to \`node$((NODE_COUNT-1)).env\` - Environment files with all node-specific variables

### Deployment Packages
- \`../deploy/node0/\` to \`../deploy/node$((NODE_COUNT-1))/\` - Individual node deployment packages

## Network Configuration

- **Node Count**: $NODE_COUNT
- **Project Name**: $PROJECT_NAME
- **GitHub Repo**: $GITHUB_REPO
- **GitHub Branch**: $GITHUB_BRANCH

## Deployment Process

### 1. Prepare Configurations (This Step)
\`\`\`bash
# Generate all configurations
./scripts/prepare-configs.sh -n $NODE_COUNT
\`\`\`

### 2. Deploy to Remote Nodes
\`\`\`bash
# Deploy individual nodes
cd ../deploy
for i in \$(seq 0 $((NODE_COUNT-1))); do
    scp -r node\$i/* ubuntu@\${NODE_IPS[\$i]}:/tmp/fastevm-config/
    ssh ubuntu@\${NODE_IPS[\$i]} 'bash /tmp/fastevm-config/deploy.sh'
done
\`\`\`

### 3. Deploy with Docker Compose (Alternative)
\`\`\`bash
# Generate docker-compose.yml from template
cd config
../scripts/replace-templates.sh
# Deploy using Docker Compose
docker-compose up -d
\`\`\`

### 4. Verify Deployment
\`\`\`bash
# Test RPC endpoints
for i in \$(seq 0 $((NODE_COUNT-1))); do
    node_ip="\${NODE_IPS[\$i]}"
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
scp -r deploy/node0/* ubuntu@\${NODE_IPS[0]}:/tmp/fastevm-config/
ssh ubuntu@\${NODE_IPS[0]} 'bash /tmp/fastevm-config/deploy.sh'
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

- JWT secrets and P2P keys are generated for each node and stored in .env files
- Each node has isolated data volumes
- Network is isolated to the specified subnet
- Change default secrets in production environments

## Troubleshooting

### Check Node Status
\`\`\`bash
ssh ubuntu@\${NODE_IPS[0]} 'fastevm-status'
\`\`\`

### Check Health
\`\`\`bash
ssh ubuntu@\${NODE_IPS[0]} 'fastevm-health-check'
\`\`\`

### View Logs
\`\`\`bash
ssh ubuntu@\${NODE_IPS[0]} 'journalctl -u fastevm-execution -f'
ssh ubuntu@\${NODE_IPS[0]} 'journalctl -u fastevm-consensus -f'
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
echo "  - IPs: ${NODE_IPS[*]}"
echo "  - All configurations prepared locally"
echo "  - Deployment packages ready"
echo ""
log_info "Next steps:"
echo "  1. Review the generated configuration files"
echo "  2. Deploy individual nodes using the deployment packages"
echo "  3. Test the RPC endpoints"
echo "  4. Monitor the network status"
echo ""
log_info "For more information, see: $CONFIG_DIR/README.md"
