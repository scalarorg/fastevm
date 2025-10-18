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

# Load environment variables from node.env file
if [ -f "/tmp/fastevm-config/node.env" ]; then
    log_info "Loading environment variables from node.env file..."
    source /tmp/fastevm-config/node.env
    log_success "Environment variables loaded successfully"
else
    log_error "node.env file not found at /tmp/fastevm-config/node.env"
    exit 1
fi

# Debug information
log_info "Environment variables:"
log_info "  NODE_INDEX: $NODE_INDEX"
log_info "  NODE_COUNT: $NODE_COUNT"
log_info "  PROJECT_NAME: $PROJECT_NAME"
log_info "  NODE_IP: $NODE_IP"
log_info "  HTTP_PORT: $HTTP_PORT"
log_info "  WS_PORT: $WS_PORT"
log_info "  ENGINE_PORT: $ENGINE_PORT"
log_info "  CONSENSUS_PORT: $CONSENSUS_PORT"
log_info "  P2P_PORT: $P2P_PORT"

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

# Create FastEVM directory structure
log_info "Setting up FastEVM directory structure..."
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

# Create data directories
log_info "Creating data directories..."
sudo rm -rf /data/*
sudo mkdir -p /data/execution
sudo mkdir -p /data/execution/p2p
sudo mkdir -p /data/execution/db
sudo mkdir -p /data/consensus
sudo mkdir -p /data/consensus/db
sudo mkdir -p /data/logs
sudo mkdir -p /data/config

# Generate JWT secret
log_info "Generating JWT secret..."
JWT_SECRET=$(openssl rand -hex 32)
echo "$JWT_SECRET" | sudo tee /data/execution/jwt.hex > /dev/null
sudo chown ubuntu:ubuntu /data/execution/jwt.hex
sudo chmod 664 /data/execution/jwt.hex
log_info "JWT secret generated and permissions set"

# Verify JWT secret format
JWT_SECRET_CONTENT=$(cat /data/execution/jwt.hex | tr -d '\n')
JWT_SECRET_LENGTH=${#JWT_SECRET_CONTENT}
log_info "JWT secret length: $JWT_SECRET_LENGTH characters"
if [ "$JWT_SECRET_LENGTH" -ne 64 ]; then
    log_error "JWT secret length is $JWT_SECRET_LENGTH, expected 64 characters"
    exit 1
fi

# Generate P2P secret key and peer ID from environment variables
log_info "Generating P2P secret key and peer ID..."
echo "$P2P_SECRET_KEY" | sudo tee /data/execution/p2p/secret.key > /dev/null
echo "$P2P_PEER_ID" | sudo tee /data/execution/p2p/secret.hex > /dev/null
sudo chown ubuntu:ubuntu /data/execution/p2p/secret.key
sudo chown ubuntu:ubuntu /data/execution/p2p/secret.hex
sudo chmod 664 /data/execution/p2p/secret.key
sudo chmod 664 /data/execution/p2p/secret.hex
log_info "P2P keys generated from environment variables"

# Verify P2P secret key format
P2P_SECRET_CONTENT=$(cat /data/execution/p2p/secret.key | tr -d '\n')
P2P_SECRET_LENGTH=${#P2P_SECRET_CONTENT}
log_info "P2P secret key length: $P2P_SECRET_LENGTH characters"
if [ "$P2P_SECRET_LENGTH" -ne 64 ]; then
    log_error "P2P secret key length is $P2P_SECRET_LENGTH, expected 64 characters"
    exit 1
fi

# Generate configuration files directly using environment variables
log_info "Generating configuration files..."

# Copy genesis.json
log_info "Copying genesis.json..."
sudo cp /tmp/fastevm-config/genesis.json /data/

# Copy configuration files
log_info "Copying configuration files..."
if [ -f "/tmp/fastevm-config/execution.toml" ]; then
    sudo cp /tmp/fastevm-config/execution.toml /data/
    log_success "Copied execution.toml"
else
    log_warning "execution.toml not found in deployment package"
fi

if [ -f "/tmp/fastevm-config/node.yml" ]; then
    sudo cp /tmp/fastevm-config/node.yml /data/
    log_success "Copied node.yml"
else
    log_warning "node.yml not found in deployment package"
fi

if [ -f "/tmp/fastevm-config/node.env" ]; then
    sudo cp /tmp/fastevm-config/node.env /data/
    log_success "Copied node.env"
else
    log_warning "node.env not found in deployment package"
fi

# Install systemd services
log_info "Installing systemd services..."
sudo bash /tmp/fastevm-config/service.sh install

# Ensure database directories have proper permissions
log_info "Setting database permissions..."
sudo chmod -R 755 /data/execution/db
sudo chmod -R 755 /data/consensus/db

# Set proper permissions
log_info "Setting permissions..."
sudo chown -R ubuntu:ubuntu /data

# Create completion marker
echo "FastEVM node $NODE_INDEX deployment completed successfully at $(date)" | sudo tee /var/log/fastevm-deployment-complete

log_success "=== FastEVM Node $NODE_INDEX Deployment Completed Successfully ==="
log_success "Services installed but not started. Use 'make start-services' to start all nodes."
log_info "Use 'fastevm-status' to check status"
log_info "Use 'fastevm-health-check' to verify health"
