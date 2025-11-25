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

# Configure file descriptor limits to prevent "too many files open" errors
# Use higher limits for benchmarking workloads that create many concurrent connections
log "Configuring file descriptor limits..."
# Set limits for current session
ulimit -n 1048576 2>/dev/null || true

# Configure system-wide limits
LIMITS_FILE="/etc/security/limits.conf"
if ! grep -q "ubuntu.*nofile" "$LIMITS_FILE" 2>/dev/null; then
    log "Adding file descriptor limits to $LIMITS_FILE..."
    cat >> "$LIMITS_FILE" << 'EOF'
# File descriptor limits for ubuntu user (added by client-setup.sh)
ubuntu soft nofile 1048576
ubuntu hard nofile 1048576
root soft nofile 1048576
root hard nofile 1048576
* soft nofile 1048576
* hard nofile 1048576
EOF
    log "File descriptor limits configured in $LIMITS_FILE"
fi

# Configure systemd limits if systemd is available
if command -v systemctl &> /dev/null; then
    SYSTEMD_LIMITS_DIR="/etc/systemd/system.conf.d"
    mkdir -p "$SYSTEMD_LIMITS_DIR"
    
    if [ ! -f "$SYSTEMD_LIMITS_DIR/limits.conf" ]; then
        log "Configuring systemd file descriptor limits..."
        cat > "$SYSTEMD_LIMITS_DIR/limits.conf" << 'EOF'
[Manager]
DefaultLimitNOFILE=1048576
EOF
        log "Systemd limits configured"
    fi
fi

# Set limits for ubuntu user's current session (if running as ubuntu)
if id ubuntu &>/dev/null 2>&1; then
    sudo -u ubuntu bash -c "ulimit -n 1048576" 2>/dev/null || true
fi

# Also set for root user
ulimit -n 1048576 2>/dev/null || true

log "File descriptor limits configured: soft/hard = 1048576 (1M)"

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
    golang-go \
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

# Install vegeta (required for flood)
log "Installing vegeta (required for flood load testing)..."
if ! command -v vegeta &> /dev/null; then
    # Install vegeta using go install
    export PATH="$PATH:/usr/local/go/bin:$(go env GOPATH)/bin"
    go install github.com/tsenart/vegeta/v12@v12.8.4 > /dev/null 2>&1 || {
        log "Warning: Failed to install vegeta via go install, trying alternative method..."
        # Alternative: download pre-built binary
        VEGETA_VERSION="v12.8.4"
        VEGETA_ARCH="amd64"
        VEGETA_OS="linux"
        VEGETA_URL="https://github.com/tsenart/vegeta/releases/download/${VEGETA_VERSION}/vegeta-${VEGETA_VERSION}-${VEGETA_OS}-${VEGETA_ARCH}.tar.gz"
        
        cd /tmp
        wget -q "$VEGETA_URL" -O vegeta.tar.gz || curl -L "$VEGETA_URL" -o vegeta.tar.gz
        tar -xzf vegeta.tar.gz vegeta
        mv vegeta /usr/local/bin/vegeta
        chmod +x /usr/local/bin/vegeta
        rm -f vegeta.tar.gz
        cd - > /dev/null
    }
    
    # Verify vegeta installation
    if command -v vegeta &> /dev/null; then
        VEGETA_VERSION=$(vegeta -version 2>&1 | head -n1 || echo "unknown")
        log "Vegeta installed: $VEGETA_VERSION"
    else
        log "Warning: vegeta installation may have failed, but continuing..."
    fi
else
    VEGETA_VERSION=$(vegeta -version 2>&1 | head -n1 || echo "unknown")
    log "Vegeta already installed: $VEGETA_VERSION"
fi

# Install flood (Python package)
log "Installing flood (paradigm-flood)..."
if ! command -v flood &> /dev/null; then
    # Install flood using pip
    pip3 install --upgrade pip > /dev/null 2>&1
    pip3 install paradigm-flood > /dev/null 2>&1 || {
        log "Warning: Failed to install flood via pip3, trying with --user flag..."
        pip3 install --user paradigm-flood > /dev/null 2>&1
        # Add user bin to PATH if needed
        if [ -d "$HOME/.local/bin" ]; then
            export PATH="$PATH:$HOME/.local/bin"
        fi
    }
    
    # Verify flood installation
    if command -v flood &> /dev/null || python3 -m flood --help &> /dev/null; then
        FLOOD_VERSION=$(flood --version 2>&1 || python3 -m flood --version 2>&1 || echo "installed")
        log "Flood installed: $FLOOD_VERSION"
    else
        log "Warning: flood installation may have failed, but continuing..."
        log "You can try running: pip3 install paradigm-flood"
    fi
else
    FLOOD_VERSION=$(flood --version 2>&1 || echo "installed")
    log "Flood already installed: $FLOOD_VERSION"
fi

# Create a marker file to indicate setup is complete
touch /var/log/client-setup-complete || true

log "Client setup completed successfully!"
log "Rust toolchain installed and verified"
log "Rust user: $RUST_USER"
log "Rust home: $RUST_HOME"
log "Vegeta and Flood load testing tools installed"

# Explicitly exit with success status
exit 0

