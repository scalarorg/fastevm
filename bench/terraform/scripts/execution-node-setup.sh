#!/bin/bash
# Execution Node Setup Script
# This script clones gravity-reth, builds it, and runs dev-node.sh
# This script is idempotent and can be re-run safely

set -e
set -o pipefail

# Logging function - ensures clean output without prefixes
log() {
    local message="[$(date '+%Y-%m-%d %H:%M:%S')] $1"
    echo "$message"
    echo "$message" >> /var/log/execution-node-setup.log
}

log "Starting execution node setup..."

# Update system packages
export DEBIAN_FRONTEND=noninteractive
apt-get update -y > /dev/null 2>&1
apt-get upgrade -y > /dev/null 2>&1

# Install required packages
log "Installing system dependencies..."
apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    llvm-dev \
    cmake \
    jq \
    htop \
    vim \
    unzip \
    software-properties-common \
    apt-transport-https \
    ca-certificates \
    gnupg \
    lsb-release \
    openssh-client \
    openssl > /dev/null 2>&1

# Configure file descriptor limits to prevent "too many files open" errors
log "Configuring file descriptor limits..."
# Set limits for current session
ulimit -n 65536 2>/dev/null || true

# Configure system-wide limits
LIMITS_FILE="/etc/security/limits.conf"
if ! grep -q "ubuntu.*nofile" "$LIMITS_FILE" 2>/dev/null; then
    log "Adding file descriptor limits to $LIMITS_FILE..."
    cat >> "$LIMITS_FILE" << 'EOF'
# File descriptor limits for ubuntu user (added by execution-node-setup.sh)
ubuntu soft nofile 65536
ubuntu hard nofile 65536
root soft nofile 65536
root hard nofile 65536
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
DefaultLimitNOFILE=65536
EOF
        log "Systemd limits configured"
    fi
fi

# Set limits for ubuntu user's current session (if running as ubuntu)
if id ubuntu &>/dev/null 2>&1; then
    sudo -u ubuntu bash -c "ulimit -n 65536" 2>/dev/null || true
fi

log "File descriptor limits configured: soft/hard = 65536"

# Install Rust and add to PATH
UBUNTU_HOME="/home/ubuntu"
UBUNTU_CARGO_ENV="$UBUNTU_HOME/.cargo/env"
ROOT_CARGO_ENV="$HOME/.cargo/env"

# Find where Rust is/will be installed
if [ -f "$UBUNTU_CARGO_ENV" ]; then
    CARGO_ENV="$UBUNTU_CARGO_ENV"
    CARGO_BIN="$UBUNTU_HOME/.cargo/bin"
elif [ -f "$ROOT_CARGO_ENV" ]; then
    CARGO_ENV="$ROOT_CARGO_ENV"
    CARGO_BIN="$HOME/.cargo/bin"
else
    CARGO_ENV=""
    CARGO_BIN=""
fi

# Install Rust if not found
if ! command -v rustc &> /dev/null; then
    log "Installing Rust (this may take 5-10 minutes)..."
    if [ -d "$UBUNTU_HOME" ] && id ubuntu &>/dev/null; then
        sudo -u ubuntu bash -c "curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y" || {
            log "Installing for current user instead..."
            curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y
        }
    else
        curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y
    fi
    
    # Update paths after installation
    if [ -f "$UBUNTU_CARGO_ENV" ]; then
        CARGO_ENV="$UBUNTU_CARGO_ENV"
        CARGO_BIN="$UBUNTU_HOME/.cargo/bin"
    elif [ -f "$ROOT_CARGO_ENV" ]; then
        CARGO_ENV="$ROOT_CARGO_ENV"
        CARGO_BIN="$HOME/.cargo/bin"
    fi
fi

# Source Rust environment for current session
if [ -f "$CARGO_ENV" ]; then
    source "$CARGO_ENV" 2>/dev/null || true
    export PATH="$CARGO_BIN:$PATH"
    
    # Set default toolchain if rustup is available
    if command -v rustup &> /dev/null; then
        log "Setting default Rust toolchain..."
        rustup default stable || {
            log "WARNING: Failed to set default toolchain, trying to install stable..."
            rustup toolchain install stable
            rustup default stable
        }
    fi
fi

# Add Rust to PATH permanently for ubuntu user
if [ -d "$UBUNTU_HOME" ]; then
    chown -R ubuntu:ubuntu "$UBUNTU_HOME/.cargo" 2>/dev/null || true
    
    # Use ubuntu's cargo if available, otherwise use root's
    if [ -f "$UBUNTU_CARGO_ENV" ]; then
        ENV_TO_USE="$UBUNTU_CARGO_ENV"
    elif [ -f "$ROOT_CARGO_ENV" ]; then
        ENV_TO_USE="$ROOT_CARGO_ENV"
    elif [ -d "$UBUNTU_HOME/.cargo/bin" ]; then
        ENV_TO_USE="$UBUNTU_CARGO_ENV"  # Will use bin directory fallback
    elif [ -d "$HOME/.cargo/bin" ]; then
        ENV_TO_USE="$ROOT_CARGO_ENV"  # Will use bin directory fallback
    else
        ENV_TO_USE=""
    fi
    
    # Add to profile files
    for profile in "$UBUNTU_HOME/.bashrc" "$UBUNTU_HOME/.profile"; do
        if [ ! -f "$profile" ]; then
            touch "$profile"
            chown ubuntu:ubuntu "$profile" 2>/dev/null || true
        fi
        
        if ! grep -q "\.cargo" "$profile" 2>/dev/null; then
            if [ -f "$ENV_TO_USE" ]; then
                echo "" >> "$profile"
                echo "# Rust/Cargo environment" >> "$profile"
                echo "if [ -f \"$ENV_TO_USE\" ]; then source \"$ENV_TO_USE\"; fi" >> "$profile"
            elif [ -n "$ENV_TO_USE" ]; then
                bin_dir=$(dirname "$ENV_TO_USE")/bin
                echo "" >> "$profile"
                echo "# Rust/Cargo PATH" >> "$profile"
                echo "export PATH=\"$bin_dir:\$PATH\"" >> "$profile"
            fi
            chown ubuntu:ubuntu "$profile" 2>/dev/null || true
        fi
    done
fi

# Verify Rust installation
log "Verifying Rust installation..."

# Check if rustc and cargo are available
if ! command -v rustc &> /dev/null; then
    log "ERROR: rustc not found in PATH"
    log "PATH: $PATH"
    log "Checking if cargo bin exists: $HOME/.cargo/bin"
    ls -la "$HOME/.cargo/bin" 2>/dev/null || log "Cargo bin directory does not exist"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    log "ERROR: cargo not found in PATH"
    log "PATH: $PATH"
    exit 1
fi

log "Rust installation verified: rustc $(rustc --version), cargo $(cargo --version)"

# Clone gravity-reth repository
log "Cloning gravity-reth repository..."
GRAVITY_RETH_DIR="/opt/gravity-reth"
if [ -d "$GRAVITY_RETH_DIR" ]; then
    cd "$GRAVITY_RETH_DIR"
    git pull --quiet > /dev/null 2>&1
else
    git clone -b ${gravity_reth_branch} ${gravity_reth_repo} "$GRAVITY_RETH_DIR" --quiet > /dev/null 2>&1
    cd "$GRAVITY_RETH_DIR"
fi
log "Repository ready"
sudo chown -R ubuntu:ubuntu "$GRAVITY_RETH_DIR"
sudo chmod -R 755 "$GRAVITY_RETH_DIR"
# Ensure default toolchain is set before building
if command -v rustup &> /dev/null; then
    log "Ensuring default Rust toolchain is set..."
    rustup default stable 2>/dev/null || {
        log "Installing stable toolchain..."
        rustup toolchain install stable
        rustup default stable
    }
fi

# Build gravity-reth
log "Building gravity-reth (this may take 10-20 minutes)..."
# Ensure file descriptor limits are set before building
ulimit -n 65536 2>/dev/null || true
# Build as ubuntu user to ensure proper limits
if id ubuntu &>/dev/null 2>&1; then
    # Determine cargo environment file for ubuntu user
    UBUNTU_CARGO_ENV="/home/ubuntu/.cargo/env"
    ROOT_CARGO_ENV="$HOME/.cargo/env"
    
    # Build the command to source cargo environment
    if [ -f "$UBUNTU_CARGO_ENV" ]; then
        # Use ubuntu's cargo env file
        CARGO_CMD="source $UBUNTU_CARGO_ENV && cargo build --release --bin reth"
    elif [ -f "$ROOT_CARGO_ENV" ]; then
        # Use root's cargo env file (if ubuntu's doesn't exist)
        CARGO_CMD="source $ROOT_CARGO_ENV && cargo build --release --bin reth"
    elif [ -d "/home/ubuntu/.cargo/bin" ]; then
        # Fallback: add ubuntu's cargo bin to PATH
        CARGO_CMD="export PATH=\"/home/ubuntu/.cargo/bin:\$PATH\" && cargo build --release --bin reth"
    elif [ -d "$HOME/.cargo/bin" ]; then
        # Fallback: add root's cargo bin to PATH
        CARGO_CMD="export PATH=\"$HOME/.cargo/bin:\$PATH\" && cargo build --release --bin reth"
    else
        # Last resort: try to find cargo in PATH
        CARGO_CMD="cargo build --release --bin reth"
    fi
    
    sudo -u ubuntu bash -c "cd $GRAVITY_RETH_DIR && ulimit -n 65536 && $CARGO_CMD" > /var/log/cargo-build.log 2>&1 || {
        log "ERROR: Build failed. Check /var/log/cargo-build.log"
        exit 1
    }
else
    cargo build --release --bin reth > /var/log/cargo-build.log 2>&1 || {
        log "ERROR: Build failed. Check /var/log/cargo-build.log"
        exit 1
    }
fi

# Verify binary exists
RETH_BIN="$GRAVITY_RETH_DIR/target/release/reth"
if [ ! -f "$RETH_BIN" ]; then
    log "ERROR: reth binary not found at $RETH_BIN"
    exit 1
fi
log "Build completed successfully"

# Copy dev-node.sh to the bench folder
# The script is copied from terraform/scripts/ by Terraform before this script runs
DEV_NODE_SCRIPT="$GRAVITY_RETH_DIR/bench/dev-node.sh"
mkdir -p "$GRAVITY_RETH_DIR/bench"

# Copy dev-node.sh from /tmp (where Terraform placed it) to the bench directory
if [ -f "/tmp/dev-node.sh" ]; then
    log "Copying dev-node.sh from Terraform scripts..."
    cp /tmp/dev-node.sh "$DEV_NODE_SCRIPT"
    chmod +x "$DEV_NODE_SCRIPT"
    log "dev-node.sh copied successfully"
elif [ -f "$GRAVITY_RETH_DIR/bench/dev-node.sh" ]; then
    log "Found dev-node.sh in repository, using it"
    chmod +x "$DEV_NODE_SCRIPT"
else
    log "WARNING: dev-node.sh not found, creating a simple fallback script"
    # Fallback: create a simple wrapper that uses environment variables
    cat > "$DEV_NODE_SCRIPT" << 'HEREDOC_EOF'
#!/bin/bash
# Simple dev-node.sh wrapper for gravity-reth

set -e

SCRIPT_DIR="$$(cd "$$(dirname "$${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$$(dirname "$$SCRIPT_DIR")"
DATA_DIR="$$PROJECT_ROOT/bench/.dev-node-data"
LOGS_DIR="$$PROJECT_ROOT/bench/.dev-node-logs"
PIDS_DIR="$$PROJECT_ROOT/bench/.dev-node-pids"
RETH_BIN="$$PROJECT_ROOT/target/release/reth"

mkdir -p "$$DATA_DIR"
mkdir -p "$$LOGS_DIR"
mkdir -p "$$PIDS_DIR"

# Set file descriptor limits to prevent "too many files open" errors
ulimit -n 65536 2>/dev/null || true

# Generate JWT secret if not exists
JWT_FILE="$$DATA_DIR/jwt.hex"
if [ ! -f "$$JWT_FILE" ]; then
    openssl rand -hex 32 | tr -d '\n' > "$$JWT_FILE"
fi

# Start reth node in dev mode
"$$RETH_BIN" node \
    --dev \
    --datadir "$$DATA_DIR" \
    --http \
    --http.api "eth,net,web3,admin,debug" \
    --http.addr "0.0.0.0" \
    --http.port "$${HTTP_PORT:-8545}" \
    --http.corsdomain "*" \
    --ws \
    --ws.api "eth,net,web3,admin,debug" \
    --ws.addr "0.0.0.0" \
    --ws.port "$${WS_PORT:-8546}" \
    --ws.origins "*" \
    --authrpc.addr "0.0.0.0" \
    --authrpc.port "$${ENGINE_PORT:-8551}" \
    --authrpc.jwtsecret "$$JWT_FILE" \
    --addr "0.0.0.0" \
    --port "$${P2P_PORT:-30303}" \
    > "$$LOGS_DIR/reth-node.log" 2>&1 &
    
echo $$! > "$$PIDS_DIR/reth-node.pid"
echo "Reth node started with PID: $$(cat $$PIDS_DIR/reth-node.pid)"
HEREDOC_EOF
    chmod +x "$DEV_NODE_SCRIPT"
fi

# Start the dev node in background
log "Starting gravity-reth dev node..."
cd "$GRAVITY_RETH_DIR"
export HTTP_PORT=${http_port}
export WS_PORT=${ws_port}
export ENGINE_PORT=${engine_port}
export P2P_PORT=${p2p_port}
# Set file descriptor limits before starting the node
ulimit -n 65536 2>/dev/null || true
# Start as ubuntu user with proper limits
if id ubuntu &>/dev/null 2>&1; then
    sudo -u ubuntu bash -c "cd $GRAVITY_RETH_DIR && ulimit -n 65536 && bash $DEV_NODE_SCRIPT start" > /var/log/gravity-reth-startup.log 2>&1 &
else
    nohup bash "$DEV_NODE_SCRIPT" > /var/log/gravity-reth-startup.log 2>&1 &
fi

# Wait a bit for the node to start
sleep 10

# Check if the node is running
log "Waiting for reth node to be ready..."
NODE_READY=false
for i in {1..30}; do
    if curl -s "http://localhost:${http_port}" > /dev/null 2>&1; then
        NODE_READY=true
        break
    fi
    # Only log every 5th attempt to reduce verbosity
    if [ $((i % 5)) -eq 0 ] || [ $i -eq 30 ]; then
        log "Waiting for reth node... (attempt $${i}/30)"
    fi
    sleep 5
done

if [ "$NODE_READY" = "true" ]; then
    log "Reth node is ready"
else
    log "WARNING: Reth node may not be fully ready, but setup will continue"
fi

# Get the internal IP address
INTERNAL_IP=$(curl -s http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/ip -H "Metadata-Flavor: Google" 2>/dev/null || echo "unknown")

# Create a marker file to indicate setup is complete
touch /var/log/execution-node-setup-complete || true

# Disable error exit before final log message to ensure we always exit successfully
set +e
set +o pipefail

log "Execution node setup completed successfully!"

# Explicitly exit with success status - use a simple exit
exit 0

