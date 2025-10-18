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
log_info "Loading environment variables from node.env..."
if [ -f "/tmp/fastevm-config/node.env" ]; then
    source /tmp/fastevm-config/node.env
    log_info "Environment variables loaded successfully"
else
    log_error "node.env file not found!"
    exit 1
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

# Generate P2P secret key and peer ID from environment variables
log_info "Generating P2P secret key and peer ID..."
echo "$P2P_SECRET_KEY" | sudo tee /data/execution/p2p/secret.key > /dev/null
echo "$P2P_PEER_ID" | sudo tee /data/execution/p2p/secret.hex > /dev/null
log_info "P2P keys generated from environment variables"

# Replace template files with actual configuration
log_info "Replacing template files with actual configuration..."
if [ -f "/tmp/fastevm-config/replace-templates.sh" ]; then
    chmod +x /tmp/fastevm-config/replace-templates.sh
    /tmp/fastevm-config/replace-templates.sh "$NODE_INDEX" "/tmp/fastevm-config" "/data"
    log_success "Template replacement completed"
else
    log_error "replace-templates.sh not found!"
    exit 1
fi

# Copy genesis.json
log_info "Copying genesis.json..."
sudo cp /tmp/fastevm-config/genesis.json /data/

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
