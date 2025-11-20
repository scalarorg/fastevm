#!/bin/bash
# Client Node Setup - Step 1: Setup VM and Install Rust
# This script installs system packages and Rust toolchain
# This script is idempotent and can be re-run safely

set -e
set -o pipefail

# Logging function - outputs to stdout (tee in terraform will handle log file)
log() {
    local message="[$(date '+%Y-%m-%d %H:%M:%S')] $1"
    echo "$message"
}

log "Starting client setup: VM and Rust installation..."

# Update system packages
log "Updating system packages..."
export DEBIAN_FRONTEND=noninteractive
apt-get update -y > /dev/null 2>&1
apt-get upgrade -y > /dev/null 2>&1

# Install required system packages
log "Installing required system packages..."
apt-get install -y \
    curl \
    wget \
    git \
    jq \
    htop \
    vim \
    unzip \
    software-properties-common \
    apt-transport-https \
    ca-certificates \
    gnupg \
    lsb-release \
    build-essential \
    pkg-config \
    libssl-dev \
    libssl3 \
    python3 \
    python3-pip \
    python3-venv \
    nodejs \
    npm \
    gcc \
    g++ \
    make \
    cmake \
    libboost-all-dev \
    > /dev/null 2>&1

# Install Rust toolchain (version 1.90 as per Dockerfile)
log "Installing Rust toolchain..."
# Determine the user for Rust installation (prefer SUDO_USER, fallback to ubuntu, then root)
if [ -n "$SUDO_USER" ]; then
    RUST_USER="$SUDO_USER"
    RUST_HOME=$(getent passwd "$RUST_USER" | cut -d: -f6)
elif id "ubuntu" &>/dev/null; then
    RUST_USER="ubuntu"
    RUST_HOME=$(getent passwd "$RUST_USER" | cut -d: -f6)
else
    RUST_USER="root"
    RUST_HOME="/root"
fi

log "Installing Rust for user: $RUST_USER"

# Check if Rust is already installed for the target user
if [ -f "$RUST_HOME/.cargo/bin/rustc" ]; then
    log "Rust already installed for $RUST_USER, checking version..."
    RUST_VERSION=$("$RUST_HOME/.cargo/bin/rustc" --version 2>/dev/null || echo "unknown")
    log "Rust: $RUST_VERSION"
    
    # Ensure Rust 1.90 is available
    if ! "$RUST_HOME/.cargo/bin/rustup" toolchain list 2>/dev/null | grep -q "1.90"; then
        log "Installing Rust 1.90..."
        sudo -u "$RUST_USER" "$RUST_HOME/.cargo/bin/rustup" toolchain install 1.90 > /dev/null 2>&1
    fi
    sudo -u "$RUST_USER" "$RUST_HOME/.cargo/bin/rustup" default 1.90 > /dev/null 2>&1
else
    log "Installing rustup for $RUST_USER..."
    # Install rustup as the target user
    sudo -u "$RUST_USER" bash -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y" > /dev/null 2>&1
    
    # Install Rust 1.90 and set as default
    log "Installing Rust 1.90..."
    sudo -u "$RUST_USER" "$RUST_HOME/.cargo/bin/rustup" toolchain install 1.90 > /dev/null 2>&1
    sudo -u "$RUST_USER" "$RUST_HOME/.cargo/bin/rustup" default 1.90 > /dev/null 2>&1
fi

# Ensure cargo is in PATH for subsequent commands
export PATH="$RUST_HOME/.cargo/bin:$PATH"
CARGO_BIN="$RUST_HOME/.cargo/bin/cargo"
RUSTC_BIN="$RUST_HOME/.cargo/bin/rustc"

# Verify Rust installation
log "Verifying Rust installation..."
RUST_VERSION=$("$RUSTC_BIN" --version 2>/dev/null || echo "unknown")
log "Rust: $RUST_VERSION"
CARGO_VERSION=$("$CARGO_BIN" --version 2>/dev/null || echo "unknown")
log "Cargo: $CARGO_VERSION"

# Create a marker file to indicate setup is complete
touch /var/log/client-setup-complete || true

log "Client setup completed successfully!"
log "Rust toolchain installed and verified"
log "Rust user: $RUST_USER"
log "Rust home: $RUST_HOME"

# Explicitly exit with success status
exit 0

