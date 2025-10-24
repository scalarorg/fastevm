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
apt-get upgrade -y

# Install required packages
echo "Installing required packages..."
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
    lsb-release

# Install Docker
echo "Installing Docker..."
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
apt-get update -y
apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

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
echo "Creating data directories..."
mkdir -p /data/execution
mkdir -p /data/consensus
mkdir -p /data/logs
mkdir -p /data/config

# Mount the persistent disk
echo "Mounting persistent disk..."
echo "Available block devices:"
lsblk -o NAME,SIZE,TYPE,MOUNTPOINT

# Find the attached disk (should be the second disk after boot disk)
DISK_DEVICE=""
for device in /dev/sdb /dev/nvme1n1 /dev/nvme0n2; do
    if [ -b "$device" ]; then
        DISK_DEVICE="$device"
        echo "Found disk device: $DISK_DEVICE"
        break
    fi
done

# If no device found, try to find any unpartitioned disk
if [ -z "$DISK_DEVICE" ]; then
    echo "No standard device found, searching for unpartitioned disks..."
    for device in $(lsblk -d -n -o NAME | grep -v loop | tail -n +2); do
        if [ -b "/dev/$device" ] && ! lsblk -n -o MOUNTPOINT "/dev/$device" | grep -q "/"; then
            DISK_DEVICE="/dev/$device"
            echo "Found unpartitioned disk: $DISK_DEVICE"
            break
        fi
    done
fi

if [ -n "$DISK_DEVICE" ]; then
    # Check if disk is already formatted
    if ! blkid $DISK_DEVICE >/dev/null 2>&1; then
        echo "Formatting persistent disk $DISK_DEVICE with optimized settings..."
        # Format with optimized settings for database workloads
        mkfs.ext4 -F -O ^has_journal -E lazy_itable_init=0,lazy_journal_init=0 $DISK_DEVICE
    else
        echo "Disk $DISK_DEVICE is already formatted"
    fi
    
    # Ensure /data directory exists
    mkdir -p /data
    
    # Mount the disk
    echo "Mounting persistent disk $DISK_DEVICE to /data..."
    if mount $DISK_DEVICE /data; then
        echo "Successfully mounted $DISK_DEVICE to /data"
    else
        echo "ERROR: Failed to mount $DISK_DEVICE to /data"
        echo "Trying to unmount and remount..."
        umount /data 2>/dev/null || true
        if mount $DISK_DEVICE /data; then
            echo "Successfully mounted $DISK_DEVICE to /data on retry"
        else
            echo "ERROR: Still failed to mount $DISK_DEVICE to /data"
            exit 1
        fi
    fi
    
    # Add to fstab for persistent mounting with optimized options
    if ! grep -q "$DISK_DEVICE.*/data" /etc/fstab; then
        echo "$DISK_DEVICE /data ext4 defaults,nofail,noatime,nodiratime,data=writeback 0 2" >> /etc/fstab
        echo "Added $DISK_DEVICE to /etc/fstab with optimized mount options"
    else
        echo "Mount entry already exists in /etc/fstab"
    fi
    
    # Verify mount
    if mountpoint -q /data; then
        echo "Successfully mounted $DISK_DEVICE to /data"
        df -h /data
        
        # Optimize filesystem for database workloads
        echo "Optimizing filesystem for database workloads..."
        # Increase inode count for large number of small files
        tune2fs -i 0 -c 0 $DISK_DEVICE
        
        # Set optimal I/O scheduler for SSD
        echo mq-deadline > /sys/block/$(basename $DISK_DEVICE)/queue/scheduler 2>/dev/null || true
        
        # Optimize kernel parameters for database workloads
        cat >> /etc/sysctl.conf << EOF
# Database optimization settings
vm.swappiness = 1
vm.dirty_ratio = 15
vm.dirty_background_ratio = 5
vm.dirty_expire_centisecs = 3000
vm.dirty_writeback_centisecs = 500
kernel.sched_rt_runtime_us = -1
EOF
        
        echo "Filesystem optimization completed"
    else
        echo "ERROR: Failed to mount $DISK_DEVICE to /data"
        exit 1
    fi
else
    echo "ERROR: No persistent disk found!"
    echo "Available block devices:"
    lsblk
    exit 1
fi

# Copy configuration files
echo "Copying configuration files..."
if [ -d "/tmp/fastevm-config" ]; then
    cp -r /tmp/fastevm-config/* /data/config/
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
    echo "0x$JWT_SECRET" > /data/jwt.hex

    # Generate P2P secret key
    mkdir -p /data/p2p
    openssl rand -hex 32 > /data/p2p/secret.key

    # Create execution client configuration
    cat > /data/config/execution.toml << EOF
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
jwtsecret = "/data/jwt.hex"

[chain]
chain = "/data/config/genesis.json"

[datadir]
path = "/data/execution"

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
    cat > /data/config/node.yml << EOF
# Node $NODE_INDEX configuration for FastEVM Consensus Client
chain: "/data/config/genesis.json"

# Committee configuration
committee_path: "/data/config/committees.yml"
parameters_path: "/data/config/parameters.yml"

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
working_directory: "/data"
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
    cat > /data/config/parameters.yml << EOF
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

# Create systemd service for data disk mounting
cat > /etc/systemd/system/fastevm-data-mount.service << EOF
[Unit]
Description=Mount FastEVM Data Disk
Before=fastevm-execution.service
Before=fastevm-consensus.service
RequiresMountsFor=/data

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/bin/bash -c 'if [ ! -d /data ]; then mkdir -p /data; fi && if ! mountpoint -q /data; then for device in /dev/sdb /dev/nvme1n1 /dev/nvme0n2; do if [ -b "\$device" ] && ! blkid "\$device" >/dev/null 2>&1; then mkfs.ext4 -F -O ^has_journal -E lazy_itable_init=0,lazy_journal_init=0 "\$device"; fi; done && mount -a; fi'
ExecStop=/bin/umount /data
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
ExecStart=/opt/fastevm-binaries/fastevm-execution \\
    --config /data/config/execution.toml \\
    --datadir /data/execution \\
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
ExecStart=/opt/fastevm-binaries/fastevm-consensus \\
    start \\
    --config /data/config/node.yml
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
LOG_FILE="/data/logs/fastevm-monitor.log"

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
/data/logs/*.log {
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
echo "Data mount service will ensure /data is mounted before other services start."

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

echo "Cleaning up data directories..."
rm -rf /data/execution/*
rm -rf /data/consensus/*

echo "Cleanup completed"
EOF

chmod +x /usr/local/bin/fastevm-cleanup.sh

# Set proper permissions
chown -R ubuntu:ubuntu /data
chown -R ubuntu:ubuntu $FASTEVM_DIR

# Create startup script to ensure disk is mounted at boot
cat > /usr/local/bin/fastevm-startup.sh << 'EOF'
#!/bin/bash
# FastEVM startup script to ensure data disk is mounted

LOG_FILE="/var/log/fastevm-startup.log"
exec > >(tee -a $LOG_FILE)
exec 2>&1

echo "=== FastEVM Startup Script - $(date) ==="

# Check if /data is already mounted
if mountpoint -q /data; then
    echo "/data is already mounted"
    df -h /data
    exit 0
fi

echo "/data is not mounted, attempting to mount..."

# Find and mount the data disk
for device in /dev/sdb /dev/nvme1n1 /dev/nvme0n2; do
    if [ -b "$device" ]; then
        echo "Found disk device: $device"
        
        # Check if disk is formatted
        if ! blkid $device >/dev/null 2>&1; then
            echo "Formatting disk $device..."
            mkfs.ext4 -F -O ^has_journal -E lazy_itable_init=0,lazy_journal_init=0 $device
        fi
        
        # Ensure /data directory exists
        mkdir -p /data
        
        # Mount the disk
        if mount $device /data; then
            echo "Successfully mounted $device to /data"
            df -h /data
            
            # Add to fstab if not already there
            if ! grep -q "$device.*/data" /etc/fstab; then
                echo "$device /data ext4 defaults,nofail,noatime,nodiratime,data=writeback 0 2" >> /etc/fstab
                echo "Added $device to /etc/fstab"
            fi
            
            # Set proper ownership
            chown -R ubuntu:ubuntu /data
            
            echo "Data disk mounted successfully"
            exit 0
        else
            echo "Failed to mount $device to /data"
        fi
    fi
done

echo "ERROR: Could not mount data disk"
exit 1
EOF

chmod +x /usr/local/bin/fastevm-startup.sh

# Add startup script to run at boot
cat > /etc/systemd/system/fastevm-startup.service << EOF
[Unit]
Description=FastEVM Startup Script
After=multi-user.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/fastevm-startup.sh
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

systemctl enable fastevm-startup.service

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
echo "Startup script: fastevm-startup (runs at boot to ensure disk mounting)"
echo "Use 'fastevm-status' to check status"
echo "Use 'fastevm-health-check' to verify health"
echo "Logs available in /var/log/fastevm-* and journalctl"
