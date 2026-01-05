#!/bin/bash
# FastEVM Service Management Script
# This script provides functions to manage FastEVM systemd services

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Set data directory - can be overridden via environment variable
DATA_DIR="${DATA_DIR:-/data}"

# Check if running as root or with sudo
check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "This script must be run as root or with sudo"
        exit 1
    fi
}

# Install systemd services
install_services() {
    log_info "Installing FastEVM systemd services..."
    
    # Prepare all required directories and files with proper permissions
    log_info "Preparing all required directories and files..."
    
    # Create all data directories
    mkdir -p "$DATA_DIR/execution"
    mkdir -p "$DATA_DIR/consensus"
    mkdir -p "$DATA_DIR/logs"
    mkdir -p "$DATA_DIR/config"
    
    # Create execution subdirectories
    mkdir -p "$DATA_DIR/execution/db"
    mkdir -p "$DATA_DIR/execution/p2p"
    
    # Set ownership for all directories
    chown -R ubuntu:ubuntu "$DATA_DIR/execution"
    chown -R ubuntu:ubuntu "$DATA_DIR/consensus"
    chown -R ubuntu:ubuntu "$DATA_DIR/logs"
    chown -R ubuntu:ubuntu "$DATA_DIR/config"
    
    # Create log files with proper permissions
    touch "$DATA_DIR/logs/fastevm-execution.log"
    touch "$DATA_DIR/logs/fastevm-consensus.log"
    chown ubuntu:ubuntu "$DATA_DIR/logs"/*.log
    chmod 664 "$DATA_DIR/logs"/*.log
    
    # Ensure JWT secret exists if not already present
    if [ ! -f "$DATA_DIR/execution/jwt.hex" ]; then
        openssl rand -hex 32 > "$DATA_DIR/execution/jwt.hex"
        chown ubuntu:ubuntu "$DATA_DIR/execution/jwt.hex"
        chmod 644 "$DATA_DIR/execution/jwt.hex"
    fi
    
    # Ensure P2P secret key exists if not already present
    if [ ! -f "$DATA_DIR/execution/p2p/secret.key" ]; then
        openssl rand -hex 32 > "$DATA_DIR/execution/p2p/secret.key"
        chown ubuntu:ubuntu "$DATA_DIR/execution/p2p/secret.key"
        chmod 600 "$DATA_DIR/execution/p2p/secret.key"
    fi
    
    log_success "All directories and files prepared with proper permissions"
    
    # Create systemd service for execution client
    log_info "Creating fastevm-execution.service..."
    
    # Create database initialization script
    log_info "Creating database initialization script..."
    tee /usr/local/bin/fastevm-init-db.sh > /dev/null << INITSCRIPT
#!/bin/bash
# FastEVM Database Initialization Script
# This script ensures the database is initialized before starting the service

DATA_DIR="\${DATA_DIR:-/data}"

if [ ! -d "\$DATA_DIR/execution/db" ] || [ -z "\$(ls -A \$DATA_DIR/execution/db 2>/dev/null)" ]; then
    if [ -f "\$DATA_DIR/config/genesis.json" ] && [ -f /usr/local/bin/fastevm-execution ]; then
        /usr/local/bin/fastevm-execution init --datadir "\$DATA_DIR/execution" --chain "\$DATA_DIR/config/genesis.json" || true
    fi
fi
INITSCRIPT
    chmod +x /usr/local/bin/fastevm-init-db.sh
    chown ubuntu:ubuntu /usr/local/bin/fastevm-init-db.sh
    
    # Prepare all required directories and files with proper permissions
    log_info "Preparing all required directories and files..."
    
    # Create all data directories
    mkdir -p "$DATA_DIR/logs"
    mkdir -p "$DATA_DIR/config"
    
    # Create execution subdirectories
    mkdir -p "$DATA_DIR/execution/db"
    mkdir -p "$DATA_DIR/execution/p2p"
    
    # Set ownership for all directories
    chown -R ubuntu:ubuntu "$DATA_DIR"
    
    # Create log files with proper permissions
    touch "$DATA_DIR/logs/fastevm-execution.log"
    touch "$DATA_DIR/logs/fastevm-consensus.log"
    chown ubuntu:ubuntu "$DATA_DIR/logs"/*.log
    chmod 664 "$DATA_DIR/logs"/*.log
    
    # Ensure JWT secret exists if not already present
    if [ ! -f "$DATA_DIR/execution/jwt.hex" ]; then
        openssl rand -hex 32 > "$DATA_DIR/execution/jwt.hex"
        chown ubuntu:ubuntu "$DATA_DIR/execution/jwt.hex"
        chmod 644 "$DATA_DIR/execution/jwt.hex"
    fi
    
    # Ensure P2P secret key exists if not already present
    if [ ! -f "$DATA_DIR/execution/p2p/secret.key" ]; then
        openssl rand -hex 32 > "$DATA_DIR/execution/p2p/secret.key"
        chown ubuntu:ubuntu "$DATA_DIR/execution/p2p/secret.key"
        chmod 600 "$DATA_DIR/execution/p2p/secret.key"
    fi
    
    log_success "All directories and files prepared with proper permissions"
    
    # Load environment variables from node.env file
    log_info "Loading environment variables from node.env file..."
    if [ -f "$DATA_DIR/node.env" ]; then
        source "$DATA_DIR/node.env"
        # Remove quotes from BOOTNODES if present
        log_success "Environment variables loaded from node.env"
        log_info "Node index: $NODE_INDEX"
    else
        echo -e "${YELLOW}[WARNING]${NC} node.env file not found at $DATA_DIR/node.env"
        # Set default values
        NODE_INDEX="0"
    fi
    
    if ! tee /etc/systemd/system/fastevm-execution.service > /dev/null << EOF
[Unit]
Description=FastEVM Execution Client
After=network.target

[Service]
Type=simple
User=ubuntu
Group=ubuntu
WorkingDirectory=$DATA_DIR
EnvironmentFile=$DATA_DIR/node.env
Environment=HTTP_PORT=8545
Environment=WS_PORT=8546
Environment=ENGINE_PORT=8551
Environment=P2P_PORT=30303
Environment=DATA_DIR=$DATA_DIR
# Ensure database is initialized before starting
ExecStartPre=/usr/local/bin/fastevm-init-db.sh
ExecStart=/usr/local/bin/fastevm-execution node \
    --chain $DATA_DIR/config/genesis.json \
    --datadir $DATA_DIR/execution \
    --engine.always-process-payload-attributes-on-canonical-head \
    --http \
    --http.api eth,net,web3,admin,debug,txpool \
    --http.addr 0.0.0.0 \
    --http.port ${HTTP_PORT} \
    --http.corsdomain "*" \
    --ws \
    --ws.api eth,net,web3,admin,debug,txpool \
    --ws.addr 0.0.0.0 \
    --ws.port ${WS_PORT} \
    --ws.origins "*" \
    --builder.gaslimit ${GAS_LIMIT} \
    --txpool.max-new-txns 102400 \
    --txpool.max-account-slots 102400 \
    --txpool.max-pending-txns 102400 \
    --txpool.pending-max-count 102400 \
    --txpool.pending-max-size 128 \
    --txpool.max-new-pending-txs-notifications 102400 \
    --txpool.queued-max-count 102400 \
    --txpool.queued-max-size 128 \
    --gravity.disable-pipe-execution \
    --authrpc.addr 0.0.0.0 \
    --authrpc.port ${ENGINE_PORT} \
    --authrpc.jwtsecret $DATA_DIR/execution/jwt.hex \
    --addr 0.0.0.0 \
    --port ${P2P_PORT} \
    --discovery.addr 0.0.0.0 \
    --discovery.port ${P2P_PORT} \
    --p2p-secret-key $DATA_DIR/execution/p2p/secret.key \
    --bootnodes ${BOOTNODES} \
    --enable-tx-subscription \
    --committed-subdags-per-block ${SUBDAGS_PER_BLOCK:-30} \
    --block-build-interval-ms ${BLOCK_BUILD_INTERVAL:-1000} \
    -$LOG_LEVEL
Restart=always
RestartSec=10
StandardOutput=append:$DATA_DIR/logs/fastevm-execution.log
StandardError=append:$DATA_DIR/logs/fastevm-execution.log

[Install]
WantedBy=multi-user.target
EOF
    then
        log_error "Failed to create fastevm-execution.service file"
        exit 1
    fi
    
    # Verify service file was created
    if [ ! -f "/etc/systemd/system/fastevm-execution.service" ]; then
        log_error "Service file was not created: /etc/systemd/system/fastevm-execution.service"
        exit 1
    fi

    # Create systemd service for consensus client
    log_info "Creating fastevm-consensus.service..."
    
    if ! tee /etc/systemd/system/fastevm-consensus.service > /dev/null << EOF
[Unit]
Description=FastEVM Consensus Client
After=network.target fastevm-execution.service

[Service]
Type=simple
User=ubuntu
Group=ubuntu
WorkingDirectory=$DATA_DIR
Environment=DATA_DIR=$DATA_DIR
ExecStart=/usr/local/bin/fastevm-consensus start --config $DATA_DIR/config/node.yml
Restart=always
RestartSec=10
StandardOutput=append:$DATA_DIR/logs/fastevm-consensus.log
StandardError=append:$DATA_DIR/logs/fastevm-consensus.log

[Install]
WantedBy=multi-user.target
EOF
    then
        log_error "Failed to create fastevm-consensus.service file"
        exit 1
    fi
    
    # Verify service file was created
    if [ ! -f "/etc/systemd/system/fastevm-consensus.service" ]; then
        log_error "Service file was not created: /etc/systemd/system/fastevm-consensus.service"
        exit 1
    fi

    # Create health check script
    log_info "Creating fastevm-health-check.sh..."
    tee /usr/local/bin/fastevm-health-check.sh > /dev/null << 'EOL'
#!/bin/bash
# Health check script for FastEVM nodes

# Fixed ports for all nodes
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
EOL

    chmod +x /usr/local/bin/fastevm-health-check.sh

    # Create status script
    log_info "Creating fastevm-status.sh..."
    tee /usr/local/bin/fastevm-status.sh > /dev/null << 'EOL'
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
netstat -tlnp | grep -E "(8545|8546|8551|30303|26657)"
echo ""

DATA_DIR="\${DATA_DIR:-/data}"
echo "=== Log Files ==="
if [ -f "\$DATA_DIR/logs/fastevm-execution.log" ]; then
    echo "Execution log: \$DATA_DIR/logs/fastevm-execution.log (\$(wc -l < \$DATA_DIR/logs/fastevm-execution.log) lines, \$(du -h \$DATA_DIR/logs/fastevm-execution.log | cut -f1))"
else
    echo "Execution log: Not found"
fi

if [ -f "\$DATA_DIR/logs/fastevm-consensus.log" ]; then
    echo "Consensus log: \$DATA_DIR/logs/fastevm-consensus.log (\$(wc -l < \$DATA_DIR/logs/fastevm-consensus.log) lines, \$(du -h \$DATA_DIR/logs/fastevm-consensus.log | cut -f1))"
else
    echo "Consensus log: Not found"
fi
echo ""

echo "=== Recent Execution Logs ==="
if [ -f "\$DATA_DIR/logs/fastevm-execution.log" ]; then
    tail -n 5 "\$DATA_DIR/logs/fastevm-execution.log"
else
    echo "Execution log file not found"
fi
echo ""

echo "=== Recent Consensus Logs ==="
if [ -f "\$DATA_DIR/logs/fastevm-consensus.log" ]; then
    tail -n 5 "\$DATA_DIR/logs/fastevm-consensus.log"
else
    echo "Consensus log file not found"
fi
EOL

    chmod +x /usr/local/bin/fastevm-status.sh

    # Reload systemd daemon
    log_info "Reloading systemd daemon..."
    if ! systemctl daemon-reload; then
        log_error "Failed to reload systemd daemon"
        exit 1
    fi

    # Enable services
    log_info "Enabling services..."
    if ! systemctl enable fastevm-execution; then
        log_error "Failed to enable fastevm-execution service"
        exit 1
    fi
    
    if ! systemctl enable fastevm-consensus; then
        log_error "Failed to enable fastevm-consensus service"
        exit 1
    fi

    log_success "FastEVM services installed successfully!"
}


# Start services
start_services() {
    log_info "Starting FastEVM services..."
    
    # Start execution client first
    log_info "Starting execution client..."
    systemctl start fastevm-execution
    
    # Wait for execution client to be ready
    log_info "Waiting for execution client to be ready..."
    sleep 30
    
    # Start consensus client
    log_info "Starting consensus client..."
    systemctl start fastevm-consensus
    
    log_success "Services started successfully!"
}

# Restart services
restart_services() {
    log_info "Restarting FastEVM services..."
    
    # Restart execution client first
    log_info "Restarting execution client..."
    systemctl restart fastevm-execution
    
    # Wait for execution client to be ready
    log_info "Waiting for execution client to be ready..."
    sleep 30
    
    # Restart consensus client
    log_info "Restarting consensus client..."
    systemctl restart fastevm-consensus
    
    log_success "Services restarted successfully!"
}

# Stop services
stop_services() {
    log_info "Stopping FastEVM services..."
    
    systemctl stop fastevm-consensus
    systemctl stop fastevm-execution
    
    log_success "Services stopped successfully!"
}

# Check service status
check_status() {
    log_info "Checking FastEVM service status..."
    
    echo "=== Execution Client Status ==="
    systemctl status fastevm-execution --no-pager -l
    echo ""
    
    echo "=== Consensus Client Status ==="
    systemctl status fastevm-consensus --no-pager -l
    echo ""
    
    echo "=== Network Ports ==="
    netstat -tlnp | grep -E "(8545|8546|8551|30303|26657)" || echo "No FastEVM ports found"
}

# View logs
view_logs() {
    local service="$1"
    local lines="${2:-50}"
    
    case "$service" in
        "execution"|"exec")
            log_info "Showing execution client logs (last $lines lines)..."
            if [ -f "$DATA_DIR/logs/fastevm-execution.log" ]; then
                tail -n "$lines" "$DATA_DIR/logs/fastevm-execution.log"
            else
                log_error "Execution log file not found: $DATA_DIR/logs/fastevm-execution.log"
            fi
            ;;
        "consensus"|"cons")
            log_info "Showing consensus client logs (last $lines lines)..."
            if [ -f "$DATA_DIR/logs/fastevm-consensus.log" ]; then
                tail -n "$lines" "$DATA_DIR/logs/fastevm-consensus.log"
            else
                log_error "Consensus log file not found: $DATA_DIR/logs/fastevm-consensus.log"
            fi
            ;;
        "all")
            log_info "Showing all logs (last $lines lines)..."
            echo "=== Execution Client Logs ==="
            if [ -f "$DATA_DIR/logs/fastevm-execution.log" ]; then
                tail -n "$lines" "$DATA_DIR/logs/fastevm-execution.log"
            else
                echo "Execution log file not found"
            fi
            echo ""
            echo "=== Consensus Client Logs ==="
            if [ -f "$DATA_DIR/logs/fastevm-consensus.log" ]; then
                tail -n "$lines" "$DATA_DIR/logs/fastevm-consensus.log"
            else
                echo "Consensus log file not found"
            fi
            ;;
        *)
            log_error "Unknown service: $service"
            echo "Usage: $0 logs {execution|consensus|all} [lines]"
            return 1
            ;;
    esac
}

# Follow logs
follow_logs() {
    local service="$1"
    
    case "$service" in
        "execution"|"exec")
            log_info "Following execution client logs..."
            if [ -f "$DATA_DIR/logs/fastevm-execution.log" ]; then
                tail -f "$DATA_DIR/logs/fastevm-execution.log"
            else
                log_error "Execution log file not found: $DATA_DIR/logs/fastevm-execution.log"
            fi
            ;;
        "consensus"|"cons")
            log_info "Following consensus client logs..."
            if [ -f "$DATA_DIR/logs/fastevm-consensus.log" ]; then
                tail -f "$DATA_DIR/logs/fastevm-consensus.log"
            else
                log_error "Consensus log file not found: $DATA_DIR/logs/fastevm-consensus.log"
            fi
            ;;
        "all")
            log_info "Following all logs..."
            if [ -f "$DATA_DIR/logs/fastevm-execution.log" ] && [ -f "$DATA_DIR/logs/fastevm-consensus.log" ]; then
                tail -f "$DATA_DIR/logs/fastevm-execution.log" "$DATA_DIR/logs/fastevm-consensus.log"
            else
                log_error "One or more log files not found"
            fi
            ;;
        *)
            log_error "Unknown service: $service"
            echo "Usage: $0 follow {execution|consensus|all}"
            return 1
            ;;
    esac
}

# Update binaries
update_binaries() {
    local execution_binary="$1"
    local consensus_binary="$2"
    local cli_binary="$3"
    
    if [ -z "$execution_binary" ] || [ -z "$consensus_binary" ] || [ -z "$cli_binary" ]; then
        log_error "All three binary paths are required"
        echo "Usage: $0 update-binaries <execution> <consensus> <cli>"
        return 1
    fi
    
    log_info "Updating FastEVM binaries..."
    
    # Stop services first
    log_info "Stopping services..."
    if systemctl is-active --quiet fastevm-execution; then
        systemctl stop fastevm-execution
    fi
    
    if systemctl is-active --quiet fastevm-consensus; then
        systemctl stop fastevm-consensus
    fi
    
    # Wait for services to stop
    sleep 5
    
    # Copy binaries directly to final location
    log_info "Installing new binaries..."
    cp "$execution_binary" /usr/local/bin/fastevm-execution
    cp "$consensus_binary" /usr/local/bin/fastevm-consensus
    cp "$cli_binary" /usr/local/bin/cli
    
    # Set permissions
    chmod +x /usr/local/bin/fastevm-execution
    chmod +x /usr/local/bin/fastevm-consensus
    chmod +x /usr/local/bin/cli
    
    log_success "Binaries updated successfully!"
    
    # Restart services
    log_info "Restarting services..."
    start_services
}

# Rotate logs
rotate_logs() {
    local service="$1"
    
    case "$service" in
        "execution"|"exec")
            if [ -f "$DATA_DIR/logs/fastevm-execution.log" ]; then
                log_info "Rotating execution client logs..."
                mv "$DATA_DIR/logs/fastevm-execution.log" "$DATA_DIR/logs/fastevm-execution.log.$(date +%Y%m%d_%H%M%S)"
                touch "$DATA_DIR/logs/fastevm-execution.log"
                chown ubuntu:ubuntu "$DATA_DIR/logs/fastevm-execution.log"
                systemctl reload fastevm-execution
                log_success "Execution logs rotated"
            else
                log_error "Execution log file not found"
            fi
            ;;
        "consensus"|"cons")
            if [ -f "$DATA_DIR/logs/fastevm-consensus.log" ]; then
                log_info "Rotating consensus client logs..."
                mv "$DATA_DIR/logs/fastevm-consensus.log" "$DATA_DIR/logs/fastevm-consensus.log.$(date +%Y%m%d_%H%M%S)"
                touch "$DATA_DIR/logs/fastevm-consensus.log"
                chown ubuntu:ubuntu "$DATA_DIR/logs/fastevm-consensus.log"
                systemctl reload fastevm-consensus
                log_success "Consensus logs rotated"
            else
                log_error "Consensus log file not found"
            fi
            ;;
        "all")
            rotate_logs "execution"
            rotate_logs "consensus"
            ;;
        *)
            log_error "Unknown service: $service"
            echo "Usage: $0 rotate {execution|consensus|all}"
            return 1
            ;;
    esac
}

# Main function to handle command line arguments
main() {
    local command="$1"
    shift
    
    case "$command" in
        "install")
            check_root
            install_services
            ;;
        "start")
            check_root
            start_services
            ;;
        "restart")
            check_root
            restart_services
            ;;
        "stop")
            check_root
            stop_services
            ;;
        "status")
            check_status
            ;;
        "logs")
            view_logs "$1" "$2"
            ;;
        "follow")
            follow_logs "$1"
            ;;
        "rotate")
            check_root
            rotate_logs "$1"
            ;;
        "update-binaries")
            check_root
            update_binaries "$1" "$2" "$3"
            ;;
        "help"|"--help"|"-h")
            echo "Usage: $0 {install|start|restart|stop|status|logs|follow|rotate|update-binaries} [args]"
            echo ""
            echo "Commands:"
            echo "  install              - Install systemd services"
            echo "  start                - Start services"
            echo "  restart              - Restart services"
            echo "  stop                 - Stop services"
            echo "  status               - Check service status"
            echo "  logs <service> [n]   - View logs (execution|consensus|all) [lines]"
            echo "  follow <service>     - Follow logs (execution|consensus|all)"
            echo "  rotate <service>     - Rotate logs (execution|consensus|all)"
            echo "  update-binaries <exec> <cons> <cli> - Update binaries safely"
            echo "  help                 - Show this help"
            echo ""
            echo "Examples:"
            echo "  $0 logs execution 100    # Show last 100 lines of execution logs"
            echo "  $0 follow consensus      # Follow consensus logs in real-time"
            echo "  $0 rotate all            # Rotate all log files"
            echo "  $0 update-binaries /path/to/exec /path/to/cons /path/to/cli"
            ;;
        *)
            log_error "Unknown command: $command"
            echo "Use '$0 help' for usage information"
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
