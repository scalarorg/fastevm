#!/bin/bash
# Client Node Setup - Step 2: Build Client Code and Prepare Config
# This script clones the repository, sets up Python, builds the Rust app, and creates config
# This script is idempotent and can be re-run safely

set -e
set -o pipefail

# Logging function - outputs to stdout (tee in terraform will handle log file)
log() {
    local message="[$(date '+%Y-%m-%d %H:%M:%S')] $1"
    echo "$message"
}

log "Starting client build: Build client code and prepare config..."

# Determine the user (should match step 1)
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

# Ensure cargo is in PATH
export PATH="$RUST_HOME/.cargo/bin:$PATH"
CARGO_BIN="$RUST_HOME/.cargo/bin/cargo"
RUSTC_BIN="$RUST_HOME/.cargo/bin/rustc"

# Wait for execution node to be ready
# log "Waiting for execution node to be ready..."
# EXECUTION_IP="${execution_node_internal_ip}"
# HTTP_PORT="${http_port}"
# MAX_ATTEMPTS=60
# ATTEMPT=1

# while [ $ATTEMPT -le $MAX_ATTEMPTS ]; do
#     if curl -s "http://$${EXECUTION_IP}:$${HTTP_PORT}" > /dev/null 2>&1; then
#         log "Execution node is ready at $${EXECUTION_IP}:$${HTTP_PORT}"
#         break
#     fi
#     log "Waiting for execution node... (attempt $ATTEMPT/$MAX_ATTEMPTS)"
#     sleep 10
#     ATTEMPT=$((ATTEMPT + 1))
# done

# if [ $ATTEMPT -gt $MAX_ATTEMPTS ]; then
#     log "WARNING: Execution node may not be ready, but continuing with setup..."
# fi

# Clone gravity_bench repository
log "Cloning gravity_bench repository..."
GRAVITY_BENCH_DIR="/opt/gravity_bench"
# Set defaults if template variables are not provided
if [ -z "${gravity_bench_repo}" ] || [ "${gravity_bench_repo}" = "" ]; then
    GRAVITY_BENCH_REPO="https://github.com/Galxe/gravity_bench.git"
else
    GRAVITY_BENCH_REPO="${gravity_bench_repo}"
fi
if [ -z "${gravity_bench_branch}" ] || [ "${gravity_bench_branch}" = "" ]; then
    GRAVITY_BENCH_BRANCH="main"
else
    GRAVITY_BENCH_BRANCH="${gravity_bench_branch}"
fi
# Ensure /opt directory exists and has correct permissions
if [ ! -d "/opt" ]; then
    mkdir -p /opt
fi
# If directory exists, ensure it's owned by RUST_USER before operations
if [ -d "$GRAVITY_BENCH_DIR" ]; then
    log "Directory $GRAVITY_BENCH_DIR already exists, ensuring ownership..."
    chown -R "$RUST_USER:$RUST_USER" "$GRAVITY_BENCH_DIR" 2>/dev/null || true
    log "Pulling latest changes..."
    cd "$GRAVITY_BENCH_DIR"
    sudo -u "$RUST_USER" git pull --quiet > /dev/null 2>&1 || {
        log "WARNING: Failed to pull latest changes, continuing with existing code..."
    }
else
    log "Cloning repository..."
    log "Repository: ${GRAVITY_BENCH_REPO}"
    log "Branch: ${GRAVITY_BENCH_BRANCH}"
    log "Target directory: $GRAVITY_BENCH_DIR"
    # Clone as RUST_USER to ensure correct ownership from the start
    # Use timeout to prevent hanging, and capture output
    log "Starting git clone (with 5 minute timeout)..."
    set +e  # Temporarily disable exit on error to capture output
    CLONE_OUTPUT=$(timeout 300 sudo git clone -b ${GRAVITY_BENCH_BRANCH} ${GRAVITY_BENCH_REPO} "$GRAVITY_BENCH_DIR" 2>&1)
    CLONE_EXIT=$?
    set -e  # Re-enable exit on error
    # Always log the output (handle empty output case)
    if [ -n "$CLONE_OUTPUT" ]; then
        echo "$CLONE_OUTPUT" | while IFS= read -r line; do
            log "$line"
        done
    else
        log "Git clone produced no output (this may be normal if clone was successful)"
    fi
    if [ $CLONE_EXIT -ne 0 ]; then
        log "ERROR: Failed to clone repository ${GRAVITY_BENCH_REPO} (branch: ${GRAVITY_BENCH_BRANCH})"
        log "Exit code: $CLONE_EXIT"
        if [ $CLONE_EXIT -eq 124 ]; then
            log "ERROR: Git clone timed out after 5 minutes"
        fi
        log "Error output: $CLONE_OUTPUT"
        log "Please check:"
        log "  1. Network connectivity"
        log "  2. Repository URL is correct: ${GRAVITY_BENCH_REPO}"
        log "  3. Branch exists: ${GRAVITY_BENCH_BRANCH}"
        log "  4. Repository is accessible (not private or requires authentication)"
        exit 1
    fi
    if [ ! -d "$GRAVITY_BENCH_DIR" ]; then
        log "ERROR: Directory $GRAVITY_BENCH_DIR was not created after clone"
        exit 1
    fi
    cd "$GRAVITY_BENCH_DIR" || {
        log "ERROR: Failed to change directory to $GRAVITY_BENCH_DIR after clone"
        exit 1
    }
    log "Repository cloned successfully to $GRAVITY_BENCH_DIR"
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

# Ensure the directory is owned by the target user (double-check)
log "Verifying ownership of $GRAVITY_BENCH_DIR..."
chown -R "$RUST_USER:$RUST_USER" "$GRAVITY_BENCH_DIR" 2>/dev/null || true

# Set up Python virtual environment
log "Setting up Python virtual environment..."
VENV_DIR="/opt/gravity_bench/venv"
if [ ! -d "$VENV_DIR" ]; then
    log "Creating Python virtual environment..."
    # Create venv as RUST_USER to ensure correct ownership
    sudo -u "$RUST_USER" python3 -m venv "$VENV_DIR"
    chown -R "$RUST_USER:$RUST_USER" "$VENV_DIR" 2>/dev/null || true
else
    # Ensure existing venv has correct ownership
    chown -R "$RUST_USER:$RUST_USER" "$VENV_DIR" 2>/dev/null || true
    # Check if venv is complete (has pip)
    if [ ! -f "$VENV_DIR/bin/pip" ] && [ ! -f "$VENV_DIR/bin/pip3" ]; then
        log "WARNING: Virtual environment exists but pip is missing. Recreating venv..."
        rm -rf "$VENV_DIR"
        sudo -u "$RUST_USER" python3 -m venv "$VENV_DIR"
        chown -R "$RUST_USER:$RUST_USER" "$VENV_DIR" 2>/dev/null || true
    fi
fi

# Ensure venv/bin/python exists and is executable
if [ ! -f "$VENV_DIR/bin/python" ]; then
    if [ -f "$VENV_DIR/bin/python3" ]; then
        log "Creating python symlink in venv..."
        sudo -u "$RUST_USER" ln -sf "$VENV_DIR/bin/python3" "$VENV_DIR/bin/python" 2>/dev/null || true
        chown "$RUST_USER:$RUST_USER" "$VENV_DIR/bin/python" 2>/dev/null || true
    else
        log "ERROR: Neither python nor python3 found in venv at $VENV_DIR/bin/"
        exit 1
    fi
fi
chmod +x "$VENV_DIR/bin/python" 2>/dev/null || true

# Ensure pip exists in venv
if [ ! -f "$VENV_DIR/bin/pip" ]; then
    if [ -f "$VENV_DIR/bin/pip3" ]; then
        log "Creating pip symlink in venv..."
        sudo -u "$RUST_USER" ln -sf "$VENV_DIR/bin/pip3" "$VENV_DIR/bin/pip" 2>/dev/null || true
        chown "$RUST_USER:$RUST_USER" "$VENV_DIR/bin/pip" 2>/dev/null || true
    else
        log "WARNING: pip not found in venv. Attempting to install pip using ensurepip..."
        sudo -u "$RUST_USER" "$VENV_DIR/bin/python" -m ensurepip --upgrade --default-pip 2>&1 | while IFS= read -r line; do
            log "ensurepip: $line"
        done || {
            log "ERROR: Failed to install pip in virtual environment"
            log "The virtual environment may be corrupted. Try removing $VENV_DIR and running again."
            exit 1
        }
    fi
fi
chmod +x "$VENV_DIR/bin/pip" 2>/dev/null || true

# Create system-wide python symlink pointing to venv's python (for gravity_bench to find)
if [ -f "$VENV_DIR/bin/python" ]; then
    log "Creating system-wide python symlink to venv's python..."
    # Remove existing symlink or file if it exists, then create new one
    rm -f /usr/local/bin/python 2>/dev/null || true
    ln -sf "$VENV_DIR/bin/python" /usr/local/bin/python || {
        log "WARNING: Failed to create /usr/local/bin/python symlink, trying with sudo..."
        sudo ln -sf "$VENV_DIR/bin/python" /usr/local/bin/python || true
    }
    # Also create python3 symlink for compatibility
    rm -f /usr/local/bin/python3 2>/dev/null || true
    ln -sf "$VENV_DIR/bin/python" /usr/local/bin/python3 || {
        log "WARNING: Failed to create /usr/local/bin/python3 symlink, trying with sudo..."
        sudo ln -sf "$VENV_DIR/bin/python" /usr/local/bin/python3 || true
    }
    # Also ensure /usr/local/bin is in PATH
    if ! echo "$PATH" | grep -q "/usr/local/bin"; then
        export PATH="/usr/local/bin:$PATH"
    fi
    log "Python symlinks created: /usr/local/bin/python -> $VENV_DIR/bin/python"
fi

# Activate virtual environment and add to PATH
export PATH="$VENV_DIR/bin:$PATH"
log "Python virtual environment ready at $VENV_DIR"

# Verify pip is accessible
if [ ! -f "$VENV_DIR/bin/pip" ] || [ ! -x "$VENV_DIR/bin/pip" ]; then
    log "ERROR: pip is not accessible at $VENV_DIR/bin/pip"
    log "Virtual environment setup failed. Please check Python installation."
    exit 1
fi

# Install Python dependencies
log "Installing Python dependencies..."
if [ -f "$GRAVITY_BENCH_DIR/requirements.txt" ]; then
    # Temporarily disable exit on error for pip commands
    set +e
    log "Upgrading pip..."
    "$VENV_DIR/bin/pip" install --upgrade pip 2>&1 | while IFS= read -r line; do
        log "pip: $line"
    done
    PIP_UPGRADE_EXIT=$?
    set -e
    
    if [ $PIP_UPGRADE_EXIT -ne 0 ]; then
        log "WARNING: pip upgrade failed (exit code: $PIP_UPGRADE_EXIT), continuing..."
    fi
    
    set +e
    log "Installing Python dependencies from requirements.txt..."
    "$VENV_DIR/bin/pip" install --no-cache-dir -r "$GRAVITY_BENCH_DIR/requirements.txt" 2>&1 | while IFS= read -r line; do
        log "pip: $line"
    done
    PIP_INSTALL_EXIT=$?
    set -e
    
    if [ $PIP_INSTALL_EXIT -eq 0 ]; then
        log "Python dependencies installed successfully"
    else
        log "WARNING: Python dependency installation failed (exit code: $PIP_INSTALL_EXIT), but continuing..."
        log "Some features may not work correctly without all dependencies"
    fi
else
    log "WARNING: requirements.txt not found, skipping Python dependency installation"
fi

# Always install essential Python packages needed by gravity_bench scripts
log "Installing essential Python packages (web3, eth-account)..."
set +e
"$VENV_DIR/bin/pip" install --no-cache-dir web3 eth-account 2>&1 | while IFS= read -r line; do
    log "pip: $line"
done
ESSENTIAL_INSTALL_EXIT=$?
set -e

if [ $ESSENTIAL_INSTALL_EXIT -eq 0 ]; then
    log "Essential Python packages installed successfully"
else
    log "ERROR: Failed to install essential Python packages (web3, eth-account)"
    log "This will cause gravity_bench to fail when calling deploy.py"
    exit 1
fi

# Run setup.sh to download contracts and install npm packages
log "Running setup.sh to download contracts and install npm packages..."
if [ -f "$GRAVITY_BENCH_DIR/setup.sh" ]; then
    cd "$GRAVITY_BENCH_DIR"
    # Ensure setup.sh is executable and owned by RUST_USER
    chmod +x "$GRAVITY_BENCH_DIR/setup.sh"
    chown "$RUST_USER:$RUST_USER" "$GRAVITY_BENCH_DIR/setup.sh" 2>/dev/null || true
    # Ensure all directories are owned by RUST_USER before running setup
    chown -R "$RUST_USER:$RUST_USER" "$GRAVITY_BENCH_DIR" 2>/dev/null || true
    # Run setup.sh with the virtual environment's Python in PATH
    sudo -u "$RUST_USER" env PATH="$VENV_DIR/bin:$RUST_HOME/.cargo/bin:$PATH" bash "$GRAVITY_BENCH_DIR/setup.sh" > /var/log/gravity-bench-setup.log 2>&1 || {
        log "WARNING: setup.sh encountered errors, check /var/log/gravity-bench-setup.log"
        log "Checking setup log for details..."
        tail -50 /var/log/gravity-bench-setup.log 2>/dev/null || true
    }
    # Ensure ownership is maintained after setup.sh runs
    chown -R "$RUST_USER:$RUST_USER" "$GRAVITY_BENCH_DIR" 2>/dev/null || true
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

# Set up environment variables for runtime
log "Setting up environment variables..."
ENV_FILE="/opt/gravity_bench/.env"
cat > "$ENV_FILE" << EOF
# Gravity Bench Environment Variables
export PATH="$VENV_DIR/bin:$RUST_HOME/.cargo/bin:\$PATH"
export SOLCX_BINARY_PATH="/tmp/.solcx"
export GRAVITY_BENCH_DIR="$GRAVITY_BENCH_DIR"
# Ensure python command uses venv's python
alias python="$VENV_DIR/bin/python" 2>/dev/null || true
EOF
chown "$RUST_USER:$RUST_USER" "$ENV_FILE" 2>/dev/null || true

# Create a wrapper script to ensure python uses venv
log "Creating python wrapper script..."
PYTHON_WRAPPER="/opt/gravity_bench/venv/bin/python-wrapper"
cat > "$PYTHON_WRAPPER" << 'EOFWRAPPER'
#!/bin/bash
# Wrapper to ensure python uses venv's python
exec "$(dirname "$0")/python" "$@"
EOFWRAPPER
chmod +x "$PYTHON_WRAPPER" 2>/dev/null || true
chown "$RUST_USER:$RUST_USER" "$PYTHON_WRAPPER" 2>/dev/null || true

# Create solcx directory with proper permissions
log "Setting up solcx directory..."
mkdir -p /tmp/.solcx
chmod 777 /tmp/.solcx

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

# Create a marker file to indicate build is complete
touch /var/log/client-build-complete || true

log "Client build completed successfully!"
log "Gravity bench binary: $GRAVITY_BENCH_DIR/target/release/gravity_bench"
log "Configuration template: /tmp/bench_config.template"
log "To run gravity_bench, use: run-gravity-bench [arguments]"
log "Or directly: $GRAVITY_BENCH_DIR/target/release/gravity_bench [arguments]"

# Explicitly exit with success status
exit 0

