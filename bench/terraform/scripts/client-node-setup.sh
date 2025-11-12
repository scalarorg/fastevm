#!/bin/bash
# Client Node Setup Script
# This script sets up all required packages and builds gravity_bench to run directly on the node
# This script is idempotent and can be re-run safely

set -e
set -o pipefail

# Logging function - ensures clean output without prefixes
log() {
    local message="[$(date '+%Y-%m-%d %H:%M:%S')] $1"
    echo "$message"
    # Use sudo to write to log file if running as non-root
    if [ "$EUID" -eq 0 ]; then
        echo "$message" >> /var/log/client-node-setup.log
    else
        echo "$message" | sudo tee -a /var/log/client-node-setup.log > /dev/null
    fi
}

log "Starting client node setup..."

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

# Wait for execution node to be ready
log "Waiting for execution node to be ready..."
EXECUTION_IP="${execution_node_internal_ip}"
MAX_ATTEMPTS=60
ATTEMPT=1

while [ $${ATTEMPT} -le $${MAX_ATTEMPTS} ]; do
    if curl -s "http://$${EXECUTION_IP}:${http_port}" > /dev/null 2>&1; then
        log "Execution node is ready at $${EXECUTION_IP}:${http_port}"
        break
    fi
    log "Waiting for execution node... (attempt $${ATTEMPT}/$${MAX_ATTEMPTS})"
    sleep 10
    ATTEMPT=$((ATTEMPT + 1))
done

if [ $${ATTEMPT} -gt $${MAX_ATTEMPTS} ]; then
    log "WARNING: Execution node may not be ready, but continuing with setup..."
fi

# Clone gravity_bench repository
log "Cloning gravity_bench repository..."
GRAVITY_BENCH_DIR="/opt/gravity_bench"
if [ -d "$GRAVITY_BENCH_DIR" ]; then
    log "Directory $GRAVITY_BENCH_DIR already exists, pulling latest changes..."
    cd "$GRAVITY_BENCH_DIR"
    git pull --quiet > /dev/null 2>&1 || {
        log "WARNING: Failed to pull latest changes, continuing with existing code..."
    }
else
    log "Cloning repository..."
    git clone -b ${gravity_bench_branch} ${gravity_bench_repo} "$GRAVITY_BENCH_DIR" --quiet > /dev/null 2>&1
    cd "$GRAVITY_BENCH_DIR"
    log "Repository cloned successfully"
fi

# Set up Python virtual environment
log "Setting up Python virtual environment..."
VENV_DIR="/opt/gravity_bench/venv"
if [ ! -d "$VENV_DIR" ]; then
    log "Creating Python virtual environment..."
    python3 -m venv "$VENV_DIR"
fi

# Activate virtual environment and add to PATH
export PATH="$VENV_DIR/bin:$PATH"
log "Python virtual environment ready at $VENV_DIR"

# Install Python dependencies
log "Installing Python dependencies..."
if [ -f "$GRAVITY_BENCH_DIR/requirements.txt" ]; then
    "$VENV_DIR/bin/pip" install --upgrade pip > /dev/null 2>&1
    "$VENV_DIR/bin/pip" install --no-cache-dir -r "$GRAVITY_BENCH_DIR/requirements.txt" > /dev/null 2>&1
    log "Python dependencies installed successfully"
else
    log "WARNING: requirements.txt not found, skipping Python dependency installation"
fi

# Run setup.sh to download contracts and install npm packages
log "Running setup.sh to download contracts and install npm packages..."
if [ -f "$GRAVITY_BENCH_DIR/setup.sh" ]; then
    cd "$GRAVITY_BENCH_DIR"
    # Ensure setup.sh is executable
    chmod +x "$GRAVITY_BENCH_DIR/setup.sh"
    # Run setup.sh with the virtual environment's Python in PATH
    sudo -u "$RUST_USER" env PATH="$VENV_DIR/bin:$RUST_HOME/.cargo/bin:$PATH" bash "$GRAVITY_BENCH_DIR/setup.sh" > /var/log/gravity-bench-setup.log 2>&1 || {
        log "WARNING: setup.sh encountered errors, check /var/log/gravity-bench-setup.log"
    }
    log "setup.sh completed"
else
    log "WARNING: setup.sh not found, skipping contract download and npm package installation"
fi

# Run refresh_init_code.py script
log "Running refresh_init_code.py..."
if [ -f "$GRAVITY_BENCH_DIR/scripts/refresh_init_code.py" ]; then
    cd "$GRAVITY_BENCH_DIR"
    sudo -u "$RUST_USER" env PATH="$VENV_DIR/bin:$RUST_HOME/.cargo/bin:$PATH" "$VENV_DIR/bin/python" "$GRAVITY_BENCH_DIR/scripts/refresh_init_code.py" > /var/log/gravity-bench-refresh.log 2>&1 || {
        log "WARNING: refresh_init_code.py encountered errors, check /var/log/gravity-bench-refresh.log"
    }
    log "refresh_init_code.py completed"
else
    log "WARNING: scripts/refresh_init_code.py not found, skipping"
fi

# Build the Rust application
log "Building gravity_bench Rust application..."
cd "$GRAVITY_BENCH_DIR"
# Ensure the directory is owned by the target user
chown -R "$RUST_USER:$RUST_USER" "$GRAVITY_BENCH_DIR" 2>/dev/null || true

if [ -f "$GRAVITY_BENCH_DIR/Cargo.toml" ]; then
    # Check if binary already exists and is up to date
    if [ -f "$GRAVITY_BENCH_DIR/target/release/gravity_bench" ]; then
        log "Binary already exists, checking if rebuild is needed..."
        # Compare modification times - if source is newer, rebuild
        if [ "$GRAVITY_BENCH_DIR/Cargo.toml" -nt "$GRAVITY_BENCH_DIR/target/release/gravity_bench" ] || \
           find "$GRAVITY_BENCH_DIR/src" -type f -newer "$GRAVITY_BENCH_DIR/target/release/gravity_bench" 2>/dev/null | grep -q .; then
            log "Source files are newer, rebuilding..."
            sudo -u "$RUST_USER" env PATH="$RUST_HOME/.cargo/bin:$PATH" "$CARGO_BIN" build --release > /var/log/gravity-bench-build.log 2>&1
        else
            log "Binary is up to date, skipping build"
        fi
    else
        log "Building gravity_bench (this may take several minutes)..."
        sudo -u "$RUST_USER" env PATH="$RUST_HOME/.cargo/bin:$PATH" "$CARGO_BIN" build --release > /var/log/gravity-bench-build.log 2>&1
    fi
    
    if [ -f "$GRAVITY_BENCH_DIR/target/release/gravity_bench" ]; then
        log "Build completed successfully"
        # Make binary executable
        chmod +x "$GRAVITY_BENCH_DIR/target/release/gravity_bench"
        # Create a symlink in /usr/local/bin for easy access
        ln -sf "$GRAVITY_BENCH_DIR/target/release/gravity_bench" /usr/local/bin/gravity_bench 2>/dev/null || true
        log "Binary available at $GRAVITY_BENCH_DIR/target/release/gravity_bench"
    else
        log "ERROR: Build failed, check /var/log/gravity-bench-build.log"
        exit 1
    fi
else
    log "ERROR: Cargo.toml not found, cannot build application"
    exit 1
fi

# Create bench_config.toml from template
log "Creating bench_config.toml from template..."
# Use local template file if available, otherwise fall back to repository template
BENCH_CONFIG_TEMPLATE="/tmp/bench_config.template"
if [ ! -f "$BENCH_CONFIG_TEMPLATE" ]; then
    # Fall back to repository template if local one is not available
    BENCH_CONFIG_TEMPLATE="$GRAVITY_BENCH_DIR/bench_config.template"
fi
BENCH_CONFIG="$GRAVITY_BENCH_DIR/bench_config.toml"

if [ -f "$BENCH_CONFIG_TEMPLATE" ]; then
    # Replace EXECUTION_NODE placeholder with full execution node URL
    EXECUTION_NODE_URL="http://$${EXECUTION_IP}:${http_port}"
    sed "s|EXECUTION_NODE|$${EXECUTION_NODE_URL}|g" "$BENCH_CONFIG_TEMPLATE" > "$BENCH_CONFIG"
    log "Created bench_config.toml with execution node URL: $${EXECUTION_NODE_URL} from template: $BENCH_CONFIG_TEMPLATE"
else
    log "Template not found, creating default bench_config.toml..."
    EXECUTION_NODE_URL="http://$${EXECUTION_IP}:${http_port}"
    cat > "$BENCH_CONFIG" << EOF
# Gravity Bench Configuration
nodes = [
    { rpc_url = "$${EXECUTION_NODE_URL}", chain_id = 1337 },
]

[bench]
# Benchmark configuration
EOF
    log "Created default bench_config.toml"
fi

# Set up environment variables for runtime
log "Setting up environment variables..."
ENV_FILE="/opt/gravity_bench/.env"
cat > "$ENV_FILE" << EOF
# Gravity Bench Environment Variables
export PATH="$VENV_DIR/bin:$RUST_HOME/.cargo/bin:\$PATH"
export SOLCX_BINARY_PATH="/tmp/.solcx"
export GRAVITY_BENCH_DIR="$GRAVITY_BENCH_DIR"
EOF
chown "$RUST_USER:$RUST_USER" "$ENV_FILE" 2>/dev/null || true

# Create solcx directory with proper permissions
log "Setting up solcx directory..."
mkdir -p /tmp/.solcx
chmod 777 /tmp/.solcx

# Create a systemd service file for easy management (optional)
log "Creating systemd service file..."
SERVICE_FILE="/etc/systemd/system/gravity-bench.service"
cat > "$SERVICE_FILE" << EOF
[Unit]
Description=Gravity Bench Client
After=network.target

[Service]
Type=simple
User=$RUST_USER
WorkingDirectory=$GRAVITY_BENCH_DIR
EnvironmentFile=$ENV_FILE
ExecStart=$GRAVITY_BENCH_DIR/target/release/gravity_bench
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF
log "Systemd service file created at $SERVICE_FILE (not enabled by default)"

# Create a wrapper script for easy execution
log "Creating wrapper script..."
WRAPPER_SCRIPT="/usr/local/bin/run-gravity-bench"
cat > "$WRAPPER_SCRIPT" << 'WRAPPER_EOF'
#!/bin/bash
# Wrapper script to run gravity_bench with proper environment
source /opt/gravity_bench/.env
cd /opt/gravity_bench
exec /opt/gravity_bench/target/release/gravity_bench "$@"
WRAPPER_EOF
chmod +x "$WRAPPER_SCRIPT"
log "Wrapper script created at $WRAPPER_SCRIPT"

# Create a marker file to indicate setup is complete
touch /var/log/client-node-setup-complete || true

# Disable error exit before final log message to ensure we always exit successfully
set +e
set +o pipefail

log "Client node setup completed successfully!"
log "Gravity bench binary: $GRAVITY_BENCH_DIR/target/release/gravity_bench"
log "Configuration file: $BENCH_CONFIG"
log "To run gravity_bench, use: run-gravity-bench [arguments]"
log "Or directly: $GRAVITY_BENCH_DIR/target/release/gravity_bench [arguments]"

# Explicitly exit with success status
exit 0
