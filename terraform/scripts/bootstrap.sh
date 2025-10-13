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

# Install Rust
echo "Installing Rust..."
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
export PATH="$HOME/.cargo/bin:$PATH" && rustup default stable && rustup update

# Add docker group and user
usermod -aG docker ubuntu

# Configure passwordless sudo for ubuntu user
echo "ubuntu ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers.d/ubuntu
chmod 440 /etc/sudoers.d/ubuntu

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

# Create data directories
echo "Creating data directories..."
mkdir -p /data/execution
mkdir -p /data/consensus
mkdir -p /data/logs
mkdir -p /data/config

# Mount the persistent disk
echo "Mounting persistent disk..."
DISK_DEVICE="/dev/sdb"
if [ -b "$DISK_DEVICE" ]; then
    # Check if disk is already formatted
    if ! blkid $DISK_DEVICE; then
        echo "Formatting persistent disk..."
        mkfs.ext4 $DISK_DEVICE
    fi
    
    # Mount the disk
    echo "Mounting persistent disk to /data..."
    mount $DISK_DEVICE /data
    echo "$DISK_DEVICE /data ext4 defaults 0 0" >> /etc/fstab
fi

# Copy configuration files
echo "Copying configuration files..."
if [ -d "/tmp/fastevm-config" ]; then
    cp -r /tmp/fastevm-config/* /data/
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
    cat > /data/execution.toml << EOF
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
chain = "/data/genesis.json"

[datadir]
path = "/data/execution"

[txpool]
enabled = true
EOF

    # Create genesis file
    cat > /data/genesis.json << EOF
{
  "config": {
    "chainId": 1337,
    "homesteadBlock": 0,
    "eip150Block": 0,
    "eip155Block": 0,
    "eip158Block": 0,
    "byzantiumBlock": 0,
    "constantinopleBlock": 0,
    "petersburgBlock": 0,
    "istanbulBlock": 0,
    "berlinBlock": 0,
    "londonBlock": 0,
    "arrowGlacierBlock": 0,
    "grayGlacierBlock": 0,
    "shanghaiTime": 0,
    "cancunTime": 0
  },
  "difficulty": "0x0",
  "gasLimit": "0x1c9c380",
  "alloc": {
    "0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6": {
      "balance": "0x1000000000000000000000000000000000000000000000000000000000000000"
    }
  }
}
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
    cat > /data/node.yml << EOF
# Node $NODE_INDEX configuration for FastEVM Consensus Client
chain: "/data/genesis.json"

# Committee configuration
committee_path: "/data/committees.yml"
parameters_path: "/data/parameters.yml"

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

    # Create committees configuration
    cat > /data/committees.yml << EOF
epoch: 0
authorities:
EOF

    for i in $(seq 0 $((NODE_COUNT - 1))); do
        PEER_IP="10.0.0.$((10 + i))"
        PEER_PORT=$((26657 + i))
        cat >> /data/committees.yml << EOF
- index: $i
  stake: 1000
  hostname: fastevm-consensus$i
  address: /ip4/$PEER_IP/udp/$PEER_PORT
  authority_key: AuthorityPublicKey(placeholder-$i)
  protocol_key: ProtocolPublicKey(placeholder-$i)
  network_key: NetworkPublicKey(placeholder-$i)
EOF
    done

    cat >> /data/committees.yml << EOF
docker_network:
  base_ip: 10.0.0
  start_ip: 10
  end_ip: $((10 + NODE_COUNT - 1))
  port: 26657
quorum_threshold: $NODE_COUNT
validity_threshold: $NODE_COUNT
EOF

    # Create parameters configuration
    cat > /data/parameters.yml << EOF
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

# Create systemd service for execution client
cat > /etc/systemd/system/fastevm-execution.service << EOF
[Unit]
Description=FastEVM Execution Client
After=network.target

[Service]
Type=simple
User=ubuntu
Group=ubuntu
WorkingDirectory=$FASTEVM_DIR
ExecStart=$FASTEVM_DIR/target/release/fastevm-execution \\
    --config /data/execution.toml \\
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
After=network.target fastevm-execution.service

[Service]
Type=simple
User=ubuntu
Group=ubuntu
WorkingDirectory=$FASTEVM_DIR
ExecStart=$FASTEVM_DIR/target/release/fastevm-consensus \\
    start \\
    --config /data/node.yml
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

# Enable and start services
echo "Enabling and starting services..."
systemctl daemon-reload
systemctl enable fastevm-execution
systemctl enable fastevm-consensus

# Start execution client first
systemctl start fastevm-execution

# Wait for execution client to be ready
echo "Waiting for execution client to be ready..."
sleep 30

# Start consensus client
systemctl start fastevm-consensus

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

# Create completion marker
echo "FastEVM bootstrap completed successfully at $(date)" > /var/log/fastevm-bootstrap-complete

echo "=== FastEVM Bootstrap Completed Successfully at $(date) ==="
echo "Node $NODE_INDEX is ready!"
echo "Services started: fastevm-execution, fastevm-consensus"
echo "Use 'fastevm-status' to check status"
echo "Use 'fastevm-health-check' to verify health"
echo "Logs available in /var/log/fastevm-* and journalctl"
