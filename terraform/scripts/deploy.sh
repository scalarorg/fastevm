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
if [ -d "/data" ]; then
    log_info "Directory /data exists, removing subfolders..."
    sudo rm -rf /data/*
else
    log_info "Directory /data does not exist, creating new directory..."
    sudo mkdir -p /data
fi

# Set proper permissions
log_info "Setting permissions..."
sudo chown -R ubuntu:ubuntu /data
mkdir -p /data/execution
mkdir -p /data/execution/p2p
mkdir -p /data/execution/db
mkdir -p /data/consensus
mkdir -p /data/consensus/db
mkdir -p /data/logs
mkdir -p /data/config
# Ensure database directories have proper permissions
log_info "Setting database permissions..."
chmod -R 755 /data/execution/db
chmod -R 755 /data/consensus/db
# Generate JWT secret
log_info "Generating JWT secret..."
JWT_SECRET=$(openssl rand -hex 32)
echo "$JWT_SECRET" | tee /data/execution/jwt.hex > /dev/null
chown ubuntu:ubuntu /data/execution/jwt.hex
chmod 664 /data/execution/jwt.hex
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
# log_info "Generating P2P secret key and peer ID..."
# echo -n "$P2P_SECRET_KEY" | sudo tee /data/execution/p2p/secret.key > /dev/null
# echo -n "$P2P_PEER_ID" | sudo tee /data/execution/p2p/secret.hex > /dev/null
# sudo chown ubuntu:ubuntu /data/execution/p2p/secret.key
# sudo chown ubuntu:ubuntu /data/execution/p2p/secret.hex
# sudo chmod 600 /data/execution/p2p/secret.key
# sudo chmod 644 /data/execution/p2p/secret.hex
# log_info "P2P keys generated from environment variables"


# Generate configuration files directly using environment variables
log_info "Generating configuration files..."

# Copy genesis.json
log_info "Copying genesis.json..."
cp /tmp/fastevm-config/genesis.json /data/config/

# Copy configuration files
log_info "Copying configuration files..."
if [ -f "/tmp/fastevm-config/execution.toml" ]; then
    cp /tmp/fastevm-config/execution.toml /data/config/
    log_success "Copied execution.toml"
else
    log_warning "execution.toml not found in deployment package"
fi

if [ -f "/tmp/fastevm-config/node.yml" ]; then
    cp /tmp/fastevm-config/node.yml /data/config/
    log_success "Copied node.yml"
else
    log_warning "node.yml not found in deployment package"
fi
if [ -f "/tmp/fastevm-config/committees.yml" ]; then
    cp /tmp/fastevm-config/committees.yml /data/config/
    log_success "Copied committees.yml"
else
    log_warning "committees.yml not found in deployment package"
fi

if [ -f "/tmp/fastevm-config/parameters.yml" ]; then
    cp /tmp/fastevm-config/parameters.yml /data/config/
    log_success "Copied parameters.yml"
else
    log_warning "parameters.yml not found in deployment package"
fi

if [ -f "/tmp/fastevm-config/node.env" ]; then
    cp /tmp/fastevm-config/node.env /data/
    log_success "Copied node.env"
else
    log_warning "node.env not found in deployment package"
fi

# Create completion marker
echo "FastEVM node $NODE_INDEX deployment completed successfully at $(date)" | sudo tee /var/log/fastevm-deployment-complete

log_success "=== FastEVM Node $NODE_INDEX Deployment Completed Successfully ==="
