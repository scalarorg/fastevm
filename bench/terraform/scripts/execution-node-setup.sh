#!/bin/bash
# Execution Node Setup Script
# This script handles system setup: packages, Rust installation, cloning, and building
# Node management (startup, cleanup, data) is handled by dev-node.sh
# This script is idempotent and can be re-run safely

set -e
set -o pipefail

# Logging function - ensures clean output without prefixes
log() {
    local message="[$(date '+%Y-%m-%d %H:%M:%S')] $1"
    echo "$message"
    # Use sudo to write to log file if running as non-root
    if [ "$EUID" -eq 0 ]; then
        echo "$message" >> /var/log/execution-node-setup.log
    else
        echo "$message" | sudo tee -a /var/log/execution-node-setup.log > /dev/null
    fi
}

log "Starting execution node setup..."

# Ensure log file exists with proper permissions
if [ "$EUID" -eq 0 ]; then
    touch /var/log/execution-node-setup.log
    chmod 644 /var/log/execution-node-setup.log
else
    sudo touch /var/log/execution-node-setup.log
    sudo chmod 644 /var/log/execution-node-setup.log
fi

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
    llvm \
    llvm-dev \
    cmake \
    clang \
    jq \
    htop \
    vim \
    unzip \
    protobuf-compiler jq dos2unix \
    libudev-dev libusb-1.0-0-dev \
    python3 python-is-python3 python3-pip python3-venv \
    nodejs npm \
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
# Ensure default toolchain is set before building
if command -v rustup &> /dev/null; then
    log "Ensuring default Rust toolchain is set..."
    rustup default stable 2>/dev/null || {
        log "Installing stable toolchain..."
        rustup toolchain install stable
        rustup default stable
    }
fi

log "Rust installation verified: rustc $(rustc --version), cargo $(cargo --version)"
# Clone gravity-sdk
log "Cloning gravity-sdk repository..."
gravity_sdk_repo=${gravity_sdk_repo}
gravity_sdk_branch=${gravity_sdk_branch}
GRAVITY_SDK_DIR="/opt/gravity-sdk"
if [ -d "$GRAVITY_SDK_DIR" ]; then
    cd "$GRAVITY_SDK_DIR"
    log "Repository exists, cleaning local changes and updating..."
    # Discard any local changes and reset to remote state
    git reset --hard HEAD > /dev/null 2>&1 || true
    git clean -fd > /dev/null 2>&1 || true
    git fetch --all --tags --quiet > /dev/null 2>&1 || true
    # Checkout the specified branch or tag
    if [ -n "$gravity_sdk_branch" ]; then
        log "Checking out $gravity_sdk_branch..."
        git checkout "$gravity_sdk_branch" > /dev/null 2>&1 || {
            log "Branch/tag $gravity_sdk_branch not found locally, fetching..."
            git fetch origin "$gravity_sdk_branch" --quiet > /dev/null 2>&1 || true
            git checkout "$gravity_sdk_branch" > /dev/null 2>&1 || {
                log_error "Failed to checkout $gravity_sdk_branch"
                exit 1
            }
        }
    fi
    git pull --quiet > /dev/null 2>&1 || true
else
    if [ -n "$gravity_sdk_branch" ]; then
        git clone -b ${gravity_sdk_branch} ${gravity_sdk_repo} "$GRAVITY_SDK_DIR" --quiet > /dev/null 2>&1
    else
        git clone ${gravity_sdk_repo} "$GRAVITY_SDK_DIR" --quiet > /dev/null 2>&1
    fi
    cd "$GRAVITY_SDK_DIR"
    if [ -n "$gravity_sdk_branch" ]; then
        git checkout "$gravity_sdk_branch" > /dev/null 2>&1 || {
            log_error "Failed to checkout $gravity_sdk_branch"
            exit 1
        }
    fi
fi
log "Repository ready"
sudo chown -R ubuntu:ubuntu "$GRAVITY_SDK_DIR"
sudo chmod -R 755 "$GRAVITY_SDK_DIR"
# Build gravity-sdk
log "Building gravity-sdk ..."
cd "$GRAVITY_SDK_DIR"
log "Building gravity_node..."
make gravity_node

# Clone gravity-reth repository
log "Cloning gravity-reth repository..."

gravity_reth_repo=${gravity_reth_repo}
gravity_reth_branch=${gravity_reth_branch}

GRAVITY_RETH_DIR="/opt/gravity-reth"
if [ -d "$GRAVITY_RETH_DIR" ]; then
    cd "$GRAVITY_RETH_DIR"
    log "Repository exists, cleaning local changes and updating..."
    # Discard any local changes and reset to remote state
    git reset --hard HEAD > /dev/null 2>&1 || true
    git clean -fd > /dev/null 2>&1 || true
    git fetch --all --tags --quiet > /dev/null 2>&1 || true
    # Checkout the specified branch or tag
    if [ -n "$gravity_reth_branch" ]; then
        log "Checking out $gravity_reth_branch..."
        git checkout "$gravity_reth_branch" > /dev/null 2>&1 || {
            log "Branch/tag $gravity_reth_branch not found locally, fetching..."
            git fetch origin "$gravity_reth_branch" --quiet > /dev/null 2>&1 || true
            git checkout "$gravity_reth_branch" > /dev/null 2>&1 || {
                log_error "Failed to checkout $gravity_reth_branch"
                exit 1
            }
        }
    fi
    git pull --quiet > /dev/null 2>&1 || true
else
    if [ -n "$gravity_reth_branch" ]; then
        git clone -b ${gravity_reth_branch} ${gravity_reth_repo} "$GRAVITY_RETH_DIR" --quiet > /dev/null 2>&1
    else
        git clone ${gravity_reth_repo} "$GRAVITY_RETH_DIR" --quiet > /dev/null 2>&1
    fi
    cd "$GRAVITY_RETH_DIR"
    if [ -n "$gravity_reth_branch" ]; then
        git checkout "$gravity_reth_branch" > /dev/null 2>&1 || {
            log_error "Failed to checkout $gravity_reth_branch"
            exit 1
        }
    fi
fi
log "Repository ready"
sudo chown -R ubuntu:ubuntu "$GRAVITY_RETH_DIR"
sudo chmod -R 755 "$GRAVITY_RETH_DIR"

# Clone origin reth repository
log "Cloning origin reth repository..."
reth_repo=${reth_repo:-"https://github.com/paradigmxyz/reth.git"}
reth_branch_or_tag=${reth_branch_or_tag:-"v1.9.3"}
RETH_DIR="/opt/reth"
if [ -d "$RETH_DIR" ]; then
    cd "$RETH_DIR"
    log "Repository exists, cleaning local changes and updating..."
    # Discard any local changes and reset to remote state
    git reset --hard HEAD > /dev/null 2>&1 || true
    git clean -fd > /dev/null 2>&1 || true
    git fetch --all --tags --quiet > /dev/null 2>&1 || true
    # Checkout the specified branch or tag
    if [ -n "$reth_branch_or_tag" ]; then
        log "Checking out $reth_branch_or_tag..."
        git checkout "$reth_branch_or_tag" > /dev/null 2>&1 || {
            log "Branch/tag $reth_branch_or_tag not found locally, fetching..."
            git fetch origin "$reth_branch_or_tag" --quiet > /dev/null 2>&1 || true
            git checkout "$reth_branch_or_tag" > /dev/null 2>&1 || {
                log_error "Failed to checkout $reth_branch_or_tag"
                exit 1
            }
        }
    fi
    git pull --quiet > /dev/null 2>&1 || true
else
    if [ -n "$reth_branch_or_tag" ]; then
        git clone -b ${reth_branch_or_tag} ${reth_repo} "$RETH_DIR" --quiet > /dev/null 2>&1
    else
        git clone ${reth_repo} "$RETH_DIR" --quiet > /dev/null 2>&1
    fi
    cd "$RETH_DIR"
    if [ -n "$reth_branch_or_tag" ]; then
        git checkout "$reth_branch_or_tag" > /dev/null 2>&1 || {
            log_error "Failed to checkout $reth_branch_or_tag"
            exit 1
        }
    fi
fi
log "Origin reth repository ready"
sudo chown -R ubuntu:ubuntu "$RETH_DIR"
sudo chmod -R 755 "$RETH_DIR"

# Build origin reth
log "Building origin reth (this may take 10-20 minutes)..."
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
    
    sudo -u ubuntu bash -c "cd $RETH_DIR && ulimit -n 65536 && $CARGO_CMD" > /var/log/reth-build.log 2>&1 || {
        log "ERROR: Origin reth build failed. Check /var/log/reth-build.log"
        exit 1
    }
else
    cargo build --release --bin reth > /var/log/reth-build.log 2>&1 || {
        log "ERROR: Origin reth build failed. Check /var/log/reth-build.log"
        exit 1
    }
fi

# Verify origin reth binary exists
ORIGIN_RETH_BIN="$RETH_DIR/target/release/reth"
if [ ! -f "$ORIGIN_RETH_BIN" ]; then
    log "ERROR: Origin reth binary not found at $ORIGIN_RETH_BIN"
    exit 1
fi
log "Origin reth build completed successfully"

# Copy origin reth binary to /usr/local/bin/reth
log "Installing origin reth binary to /usr/local/bin/reth..."
sudo cp "$ORIGIN_RETH_BIN" /usr/local/bin/reth
sudo chmod +x /usr/local/bin/reth
sudo chown root:root /usr/local/bin/reth
log "Origin reth installed to /usr/local/bin/reth"

# Verify gravity-reth binary exists (if it was built)
RETH_BIN="$GRAVITY_RETH_DIR/target/release/reth"
if [ ! -f "$RETH_BIN" ]; then
    log "WARNING: gravity-reth binary not found at $RETH_BIN (may be built separately)"
else
    log "Gravity-reth binary verified"
    # Copy gravity-reth binary to /usr/local/bin/gravity-reth
    log "Installing gravity-reth binary to /usr/local/bin/gravity-reth..."
    sudo cp "$RETH_BIN" /usr/local/bin/gravity-reth
    sudo chmod +x /usr/local/bin/gravity-reth
    sudo chown root:root /usr/local/bin/gravity-reth
    log "Gravity-reth installed to /usr/local/bin/gravity-reth"
fi

# Ensure bench directory exists with correct ownership
# Note: dev-node.sh is now copied by deploy.sh before this script runs
mkdir -p "$GRAVITY_RETH_DIR/bench"
chown -R ubuntu:ubuntu "$GRAVITY_RETH_DIR/bench" 2>/dev/null || true
chmod 755 "$GRAVITY_RETH_DIR/bench" 2>/dev/null || true

# Verify dev-node.sh exists (should have been copied by deploy.sh)
DEV_NODE_SCRIPT="/opt/dev-node.sh"
if [ ! -f "$DEV_NODE_SCRIPT" ]; then
    log "ERROR: dev-node.sh not found at $DEV_NODE_SCRIPT"
    log "Please ensure deploy.sh has copied the script before running execution-node-setup.sh"
    exit 1
fi

# Ensure dev-node.sh has correct permissions
chmod +x "$DEV_NODE_SCRIPT"
chown ubuntu:ubuntu "$DEV_NODE_SCRIPT" 2>/dev/null || true
log "dev-node.sh verified and ready"

# Note: Node startup is now handled by deploy.sh (step 4)
# This script only prepares the environment (dependencies, build, etc.)
log "Environment setup complete. Node will be started by deploy.sh"

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

