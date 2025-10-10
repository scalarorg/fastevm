#!/bin/bash
# FastEVM Client Node Setup Script
# This script sets up a client node for testing FastEVM

set -e

# Configuration
GITHUB_REPO="${github_repo}"
GITHUB_BRANCH="${github_branch}"
SSH_USER="${ssh_user}"
PROJECT_DIR="/home/$SSH_USER/fastevm"
LOG_FILE="/var/log/client-setup.log"

# Logging function
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

log "Starting FastEVM client node setup..."

# Update system packages
log "Updating system packages..."
apt-get update -y
apt-get upgrade -y

# Install essential packages
log "Installing essential packages..."
apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    jq \
    htop \
    tmux \
    vim \
    unzip \
    software-properties-common

# Install Rust
log "Installing Rust..."
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source /home/$SSH_USER/.cargo/env

# Add Rust to PATH for all users
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> /home/$SSH_USER/.bashrc
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> /root/.bashrc

# Install additional Rust components
log "Installing Rust components..."
/home/$SSH_USER/.cargo/bin/rustup component add rustfmt clippy
/home/$SSH_USER/.cargo/bin/rustup update

# Clone the FastEVM repository
log "Cloning FastEVM repository..."
cd /home/$SSH_USER
if [ -d "$PROJECT_DIR" ]; then
    log "Repository already exists, updating..."
    cd "$PROJECT_DIR"
    git fetch origin
    git checkout "$GITHUB_BRANCH"
    git pull origin "$GITHUB_BRANCH"
else
    git clone -b "$GITHUB_BRANCH" "$GITHUB_REPO" "$PROJECT_DIR"
fi

# Set proper ownership
chown -R $SSH_USER:$SSH_USER "$PROJECT_DIR"

# Build the test binary
log "Building FastEVM test binary..."
cd "$PROJECT_DIR/testing/integration"
sudo -u $SSH_USER /home/$SSH_USER/.cargo/bin/cargo build --release --bin fastevm-test

# Verify build
if [ -f "/home/$SSH_USER/fastevm/testing/integration/target/release/fastevm-test" ]; then
    log "✅ FastEVM test binary built successfully"
else
    log "❌ Failed to build FastEVM test binary"
    exit 1
fi

# Create test configuration directory
log "Creating test configuration directory..."
mkdir -p /home/$SSH_USER/test-config
chown -R $SSH_USER:$SSH_USER /home/$SSH_USER/test-config

# Create a sample test configuration
log "Creating sample test configuration..."
cat > /home/$SSH_USER/test-config/test.env.example << 'CONFIG_EOF'
# FastEVM Test Configuration Example
# Copy this file to test.env and update with your actual values

# RPC Endpoints (update with actual node IPs)
RPC_URL1=http://10.0.0.10:8545
RPC_URL2=http://10.0.0.11:8545
RPC_URL3=http://10.0.0.12:8545
RPC_URL4=http://10.0.0.13:8545

# Network Configuration
CHAIN_ID=202501

# Batch Transaction Test Parameters
TEST_SENDER_COUNT=100
TEST_TRANSACTION_COUNT=1
TEST_TRANSACTION_VALUE=1000000000000000
TEST_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

# Test Timing Configuration
TEST_WAITING_TIME_SECONDS=30
TEST_FETCH_NONCE=false
CONFIG_EOF

chown -R $SSH_USER:$SSH_USER /home/$SSH_USER/test-config

log "✅ FastEVM client node setup completed successfully!"
log "📋 Next steps:"
log "   1. SSH into the node: ssh -i <key> $SSH_USER@<node-ip>"
log "   2. Copy test-config/test.env.example to test-config/test.env"
log "   3. Update test-config/test.env with actual node IPs"
log "   4. Run tests: ./fastevm-test scan or ./fastevm-test batch"

# Create a completion marker
touch /var/log/client-setup-complete
