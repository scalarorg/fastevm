#!/bin/bash
# FastEVM Bootstrap Script
# This script initializes each node with FastEVM software and configuration

set -e

# Configuration variables - read from metadata
NODE_INDEX=$(curl -s "http://metadata.google.internal/computeMetadata/v1/instance/attributes/node-index" -H "Metadata-Flavor: Google")
NODE_COUNT=$(curl -s "http://metadata.google.internal/computeMetadata/v1/instance/attributes/node-count" -H "Metadata-Flavor: Google")
PROJECT_NAME=$(curl -s "http://metadata.google.internal/computeMetadata/v1/instance/attributes/project-name" -H "Metadata-Flavor: Google")
GITHUB_REPO="${github_repo}"
GITHUB_BRANCH="${github_branch}"
SUBNET_CIDR="${subnet_cidr}"

# Set data directory - can be overridden via environment variable
DATA_DIR="$${DATA_DIR:-/data}"

# Logging
LOG_FILE="/var/log/fastevm-bootstrap.log"
exec > >(tee -a $LOG_FILE)
exec 2>&1

echo "=== FastEVM Bootstrap Started at $(date) ==="
echo "Node Index: $NODE_INDEX"
echo "Node Count: $NODE_COUNT"
echo "Project Name: $PROJECT_NAME"
echo "GitHub Repo: $GITHUB_REPO"
echo "GitHub Branch: $GITHUB_BRANCH"

# Update system packages
echo "Updating system packages..."
apt-get update -y

# Fix any broken packages (common issue with google-compute-engine)
echo "Fixing any broken packages..."
dpkg --configure -a || true
apt-get install -f -y || true

apt-get upgrade -y || echo "Package upgrade had some issues, but continuing..."

# Install required packages
echo "Installing required packages..."
# Try to install packages, handling google-compute-engine errors gracefully
if ! apt-get install -y \
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
    lsb-release 2>&1 | tee /tmp/apt-install.log; then
    # Check if the error is related to google-compute-engine
    if grep -q "google-compute-engine" /tmp/apt-install.log; then
        echo "WARNING: google-compute-engine package had issues, attempting to fix..."
        # Try to fix the google-compute-engine package
        dpkg --configure -a || true
        apt-get install -f -y || true
        # Try installing packages again
        apt-get install -y \
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
            lsb-release || {
            echo "ERROR: Failed to install required packages even after fixing google-compute-engine"
            exit 1
        }
    else
        echo "ERROR: Failed to install required packages"
        exit 1
    fi
fi
echo "Required packages installed successfully"

# Install Docker
echo "Installing Docker..."
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
apt-get update -y
apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

# Setup NVMe disk early (before creating data directories)
echo "Setting up NVMe disk if available..."
NVME_DEVICE="/dev/nvme0n1"
MOUNT_POINT="/data"

if [ -b "$NVME_DEVICE" ]; then
    echo "NVMe device $NVME_DEVICE found, setting up..."
    
    # Check if already mounted at /data
    if mountpoint -q "$MOUNT_POINT" 2>/dev/null; then
        echo "$MOUNT_POINT is already mounted, skipping setup"
    else
        # Check if device is mounted elsewhere
        if grep -q "^$NVME_DEVICE " /proc/mounts 2>/dev/null; then
            OTHER_MOUNT=$(grep "^$NVME_DEVICE " /proc/mounts | awk '{print $2}')
            echo "$NVME_DEVICE is already mounted at $OTHER_MOUNT, skipping"
        else
            # Check if device has filesystem
            if ! blkid "$NVME_DEVICE" >/dev/null 2>&1; then
                echo "Creating ext4 filesystem on $NVME_DEVICE..."
                mkfs.ext4 -F "$NVME_DEVICE"
            fi
            
            # Create mount point
            mkdir -p "$MOUNT_POINT"
            
            # Mount the device
            echo "Mounting $NVME_DEVICE to $MOUNT_POINT..."
            if mount "$NVME_DEVICE" "$MOUNT_POINT"; then
                echo "Successfully mounted $NVME_DEVICE to $MOUNT_POINT"
                
                # Add to /etc/fstab for persistent mounting
                if ! grep -q "^$NVME_DEVICE" /etc/fstab 2>/dev/null; then
                    echo "$NVME_DEVICE $MOUNT_POINT ext4 defaults,nofail 0 2" >> /etc/fstab
                    echo "Added $NVME_DEVICE to /etc/fstab"
                fi
            else
                echo "Warning: Failed to mount $NVME_DEVICE, will use boot disk for /data"
            fi
        fi
    fi
    
    # Set ownership
    chown ubuntu:ubuntu "$MOUNT_POINT" 2>/dev/null || true
else
    echo "No NVMe device found, will use boot disk for /data"
fi

# Only build on the first node (node 0)
if [ "$NODE_INDEX" = "0" ]; then
    echo "This is the build node (node 0). Building FastEVM..."
    
    # Install Rust
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH" && rustup default stable && rustup update

    # Create FastEVM directory
    FASTEVM_DIR="/opt/fastevm"
    mkdir -p $FASTEVM_DIR
    cd $FASTEVM_DIR

    # Clone the repository
    echo "Cloning FastEVM repository..."
    echo "GitHub Repo: $GITHUB_REPO"
    echo "GitHub Branch: $GITHUB_BRANCH"
    if [ -z "$GITHUB_REPO" ]; then
        echo "ERROR: GITHUB_REPO is not set!"
        exit 1
    fi
    git clone $GITHUB_REPO .
    git checkout $GITHUB_BRANCH

    # Build the project
    echo "Building FastEVM..."
    export PATH="$HOME/.cargo/bin:$PATH" && cargo build --release
    
    # Build the test binary explicitly
    echo "Building fastevm-test binary..."
    export PATH="$HOME/.cargo/bin:$PATH" && cargo build --release --bin fastevm-test
    
    # Create binaries directory for distribution
    mkdir -p /opt/fastevm-binaries
    cp target/release/fastevm-execution /opt/fastevm-binaries/
    cp target/release/fastevm-consensus /opt/fastevm-binaries/
    cp target/release/fastevm-test /opt/fastevm-binaries/ 2>/dev/null || echo "fastevm-test not found, skipping"
    
    echo "Build completed on node 0. Binaries ready for distribution."
else
    echo "This is node $NODE_INDEX. Skipping build process."
    echo "Binaries will be distributed from node 0."
    
    # Create FastEVM directory structure
    FASTEVM_DIR="/opt/fastevm"
    mkdir -p $FASTEVM_DIR
    mkdir -p /opt/fastevm-binaries
fi

# Add docker group and user
usermod -aG docker ubuntu

# Configure passwordless sudo for ubuntu user
echo "ubuntu ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers.d/ubuntu
chmod 440 /etc/sudoers.d/ubuntu

# Create data directories
echo "Creating data directories at $DATA_DIR..."
mkdir -p "$DATA_DIR/execution"
mkdir -p "$DATA_DIR/consensus"
mkdir -p "$DATA_DIR/logs"
mkdir -p "$DATA_DIR/config"

# Use boot disk for data directory (no separate persistent disk)
echo "Using boot disk for $DATA_DIR directory..."
echo "Available disk space:"
df -h /

# Ensure data directory exists on root filesystem
mkdir -p "$DATA_DIR"
if [ -d "$DATA_DIR" ]; then
    echo "Successfully created $DATA_DIR directory on boot disk"
    df -h "$DATA_DIR"
    
    # Optimize kernel parameters for database workloads
    if ! grep -q "Database optimization settings" /etc/sysctl.conf; then
        cat >> /etc/sysctl.conf << EOF
# Database optimization settings
vm.swappiness = 1
vm.dirty_ratio = 15
vm.dirty_background_ratio = 5
vm.dirty_expire_centisecs = 3000
vm.dirty_writeback_centisecs = 500
kernel.sched_rt_runtime_us = -1
EOF
        echo "Added database optimization settings to /etc/sysctl.conf"
    fi
    echo "Filesystem setup completed"
else
    echo "ERROR: Failed to create $DATA_DIR directory"
    exit 1
fi

# Copy configuration files
echo "Copying configuration files..."
if [ -d "/tmp/fastevm-config" ]; then
    cp -r /tmp/fastevm-config/* "$DATA_DIR/config/"
    echo "Configuration files copied successfully"
else
    echo "No configuration files found in /tmp/fastevm-config"
    echo "Generating default configuration..."
    
    # Generate default node configuration
    echo "Generating node configuration..."
    NODE_IP=$(hostname -I | awk '{print $1}')
    HTTP_PORT=8545
    WS_PORT=8546
    ENGINE_PORT=8551
    CONSENSUS_PORT=26657
    P2P_PORT=30303

    # Generate JWT secret
    JWT_SECRET=$(openssl rand -hex 32)
    echo "0x$JWT_SECRET" > "$DATA_DIR/jwt.hex"

    # Generate P2P secret key
    mkdir -p "$DATA_DIR/p2p"
    openssl rand -hex 32 > "$DATA_DIR/p2p/secret.key"

    # Create execution client configuration
    cat > "$DATA_DIR/config/execution.toml" << EOF
[network]
port = $P2P_PORT
discovery.port = $P2P_PORT
discovery.addr = "0.0.0.0"

[http]
enabled = true
port = $HTTP_PORT
addr = "0.0.0.0"
api = ["eth", "net", "web3", "admin", "debug"]
corsdomain = "*"

[ws]
enabled = true
port = $WS_PORT
addr = "0.0.0.0"
api = ["eth", "net", "web3", "admin", "debug"]
origins = "*"

[authrpc]
enabled = true
port = $ENGINE_PORT
addr = "0.0.0.0"
jwtsecret = "$DATA_DIR/jwt.hex"

[chain]
chain = "$DATA_DIR/config/genesis.json"

[datadir]
path = "$DATA_DIR/execution"

[txpool]
enabled = true
max_new_txns = 102400
max_account_slots = 102400
max_pending_txns = 102400
pending_max_count = 102400
pending_max_size = 128
max_new_pending_txs_notifications = 102400
queued_max_count = 102400
queued_max_size = 128

[engine]
always_process_payload_attributes_on_canonical_head = true

[consensus]
enable_tx_subscription = true
committed_subdags_per_block = 30
block_build_interval_ms = 100

[db]
max_readers = 126
max_tables = 256
max_dbs = 256

EOF

    # Generate peer addresses for consensus
    PEER_ADDRESSES=""
    for i in $(seq 0 $((NODE_COUNT - 1))); do
        if [ $i -ne $NODE_INDEX ]; then
            PEER_IP="10.0.0.$((10 + i))"
            PEER_PORT=$((26657 + i))
            PEER_ADDRESSES="$PEER_ADDRESSES,/ip4/$PEER_IP/udp/$PEER_PORT"
        fi
    done
    PEER_ADDRESSES=$(echo $PEER_ADDRESSES | sed 's/^,//')

    # Create consensus client configuration
    cat > "$DATA_DIR/config/node.yml" << EOF
# Node $NODE_INDEX configuration for FastEVM Consensus Client
chain: "$DATA_DIR/config/genesis.json"

# Committee configuration
committee_path: "$DATA_DIR/config/committees.yml"
parameters_path: "$DATA_DIR/config/parameters.yml"

# Execution client configuration
execution_http_url: "http://127.0.0.1:$HTTP_PORT"
execution_ws_url: "ws://127.0.0.1:$WS_PORT"
jwt_secret: "0x$JWT_SECRET"
genesis_block_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
genesis_time: 1755000000
fee_recipient: "0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6"

# Network configuration
poll_interval: 30000
max_retries: 3
timeout: 30

# Node configuration
working_directory: "$DATA_DIR"
node_index: $NODE_INDEX
log_level: "info"

# Peer addresses
peer_addresses: [$PEER_ADDRESSES]
EOF

    # Skip committees configuration generation
    # The committees.yml file is now properly generated by prepare-configs.sh with correct external IP addresses
    # and will be deployed via the deployment packages
    echo "Skipping committees.yml generation - will be provided by deployment package with correct external IP addresses"

    # Create parameters configuration
    cat > "$DATA_DIR/config/parameters.yml" << EOF
leader_timeout: {
  secs: 0,
  nanos: 200000000
}
min_round_delay: {
  secs: 0,
  nanos: 100000000
}
max_forward_time_drift: {
  secs: 0,
  nanos: 500000000
}
max_blocks_per_sync: 32
max_blocks_per_fetch: 1000
sync_last_known_own_block_timeout: {
  secs: 5,
  nanos: 0
}
round_prober_interval_ms: 5000
round_prober_request_timeout_ms: 4000
propagation_delay_stop_proposal_threshold: 5
dag_state_cached_rounds: 500
commit_sync_parallel_fetches: 8
commit_sync_batch_size: 100
commit_sync_batches_ahead: 32
EOF
fi

# Create systemd service for ensuring data directory exists
cat > /etc/systemd/system/fastevm-data-mount.service << EOF
[Unit]
Description=Ensure FastEVM Data Directory Exists
Before=fastevm-execution.service
Before=fastevm-consensus.service

[Service]
Type=oneshot
RemainAfterExit=yes
Environment=DATA_DIR="$${DATA_DIR:-/data}"
ExecStart=/bin/bash -c "mkdir -p \"$$DATA_DIR/execution\" \"$$DATA_DIR/consensus\" \"$$DATA_DIR/logs\" \"$$DATA_DIR/config\" && chown -R ubuntu:ubuntu \"$$DATA_DIR\""
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# Create systemd service for execution client
cat > /etc/systemd/system/fastevm-execution.service << EOF
[Unit]
Description=FastEVM Execution Client
After=network.target fastevm-data-mount.service
Requires=fastevm-data-mount.service

[Service]
Type=simple
User=ubuntu
Group=ubuntu
WorkingDirectory=/opt/fastevm
Environment=DATA_DIR="$${DATA_DIR:-/data}"
ExecStart=/opt/fastevm-binaries/fastevm-execution \\
    --config $$DATA_DIR/config/execution.toml \\
    --datadir $$DATA_DIR/execution \\
    --log-level info
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# Create systemd service for consensus client
cat > /etc/systemd/system/fastevm-consensus.service << EOF
[Unit]
Description=FastEVM Consensus Client
After=network.target fastevm-execution.service fastevm-data-mount.service
Requires=fastevm-execution.service fastevm-data-mount.service

[Service]
Type=simple
User=ubuntu
Group=ubuntu
WorkingDirectory=/opt/fastevm
Environment=DATA_DIR="$${DATA_DIR:-/data}"
ExecStart=/opt/fastevm-binaries/fastevm-consensus \\
    start \\
    --config $$DATA_DIR/config/node.yml
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# Create health check script
cat > /usr/local/bin/fastevm-health-check.sh << 'EOF'
#!/bin/bash
# Health check script for FastEVM nodes

NODE_INDEX=$${1:-0}
HTTP_PORT=8545
ENGINE_PORT=8551

# Check execution client
if curl -s -f "http://localhost:$HTTP_PORT" > /dev/null; then
    echo "Execution client healthy"
else
    echo "Execution client unhealthy"
    exit 1
fi

# Check engine API
if curl -s -f "http://localhost:$ENGINE_PORT" > /dev/null; then
    echo "Engine API healthy"
else
    echo "Engine API unhealthy"
    exit 1
fi

echo "All services healthy"
exit 0
EOF

chmod +x /usr/local/bin/fastevm-health-check.sh

# Create monitoring script
cat > /usr/local/bin/fastevm-monitor.sh << 'EOF'
#!/bin/bash
# Monitoring script for FastEVM nodes

NODE_INDEX=$${1:-0}
DATA_DIR="$${DATA_DIR:-/data}"
LOG_FILE="$$DATA_DIR/logs/fastevm-monitor.log"

echo "$(date): Node $NODE_INDEX status check" >> $LOG_FILE

# Check system resources
echo "=== System Resources ===" >> $LOG_FILE
free -h >> $LOG_FILE
df -h >> $LOG_FILE
ps aux | grep fastevm >> $LOG_FILE

# Check service status
echo "=== Service Status ===" >> $LOG_FILE
systemctl status fastevm-execution --no-pager >> $LOG_FILE
systemctl status fastevm-consensus --no-pager >> $LOG_FILE

# Check network connectivity
echo "=== Network Connectivity ===" >> $LOG_FILE
netstat -tlnp | grep -E "(8545|8551|26657)" >> $LOG_FILE

echo "=== End Status Check ===" >> $LOG_FILE
EOF

chmod +x /usr/local/bin/fastevm-monitor.sh

# Create log rotation configuration
cat > /etc/logrotate.d/fastevm << EOF
$${DATA_DIR}/logs/*.log {
    daily
    missingok
    rotate 7
    compress
    delaycompress
    notifempty
    create 644 ubuntu ubuntu
    postrotate
        systemctl reload fastevm-execution fastevm-consensus
    endscript
}
EOF

# Set up cron job for monitoring
echo "*/5 * * * * ubuntu /usr/local/bin/fastevm-monitor.sh $NODE_INDEX" >> /etc/crontab

# Enable services (but don't start them yet)
echo "Enabling services..."
systemctl daemon-reload
systemctl enable fastevm-data-mount
systemctl enable fastevm-execution
systemctl enable fastevm-consensus

echo "Services enabled but not started. They will be started during deployment."
echo "Data directory service will ensure /data exists before other services start."

# Create status script
cat > /usr/local/bin/fastevm-status.sh << 'EOF'
#!/bin/bash
# Status script for FastEVM nodes

echo "=== FastEVM Node Status ==="
echo "Node Index: $NODE_INDEX"
echo "Node IP: $(hostname -I | awk '{print $1}')"
echo ""

echo "=== Service Status ==="
systemctl status fastevm-execution --no-pager -l
echo ""
systemctl status fastevm-consensus --no-pager -l
echo ""

echo "=== Network Ports ==="
netstat -tlnp | grep -E "(8545|8551|26657)"
echo ""

echo "=== Recent Logs ==="
journalctl -u fastevm-execution --no-pager -n 10
echo ""
journalctl -u fastevm-consensus --no-pager -n 10
EOF

chmod +x /usr/local/bin/fastevm-status.sh

# Create cleanup script
cat > /usr/local/bin/fastevm-cleanup.sh << 'EOF'
#!/bin/bash
# Cleanup script for FastEVM nodes

echo "Stopping FastEVM services..."
systemctl stop fastevm-consensus
systemctl stop fastevm-execution

echo "Disabling FastEVM services..."
systemctl disable fastevm-consensus
systemctl disable fastevm-execution

DATA_DIR="$${DATA_DIR:-/data}"
echo "Cleaning up data directories..."
rm -rf "$$DATA_DIR/execution"/*
rm -rf "$$DATA_DIR/consensus"/*

echo "Cleanup completed"
EOF

chmod +x /usr/local/bin/fastevm-cleanup.sh

# Set proper permissions
chown -R ubuntu:ubuntu "$DATA_DIR"
chown -R ubuntu:ubuntu $FASTEVM_DIR

# No separate startup script needed - using boot disk directly

# Create completion marker
echo "FastEVM bootstrap completed successfully at $(date)" > /var/log/fastevm-bootstrap-complete

echo "=== FastEVM Bootstrap Completed Successfully at $(date) ==="
echo "Node $NODE_INDEX is ready!"
if [ "$NODE_INDEX" = "0" ]; then
    echo "Build node: Binaries ready for distribution"
else
    echo "Worker node: Waiting for binaries from node 0"
fi
echo "Services enabled: fastevm-data-mount, fastevm-execution, fastevm-consensus"
echo "Use 'fastevm-status' to check status"
echo "Use 'fastevm-health-check' to verify health"
echo "Logs available in /var/log/fastevm-* and journalctl"
