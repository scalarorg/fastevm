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

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

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
    
    # Create systemd service for execution client
    log_info "Creating fastevm-execution.service..."
    
    # Ensure log file exists with proper permissions
    touch /data/logs/fastevm-execution.log
    chown ubuntu:ubuntu /data/logs/fastevm-execution.log
    chmod 644 /data/logs/fastevm-execution.log
    
    # Load environment variables from node.env file
    log_info "Loading environment variables from node.env file..."
    if [ -f "/data/node.env" ]; then
        source /data/node.env
        # Remove quotes from BOOTNODES if present
        log_success "Environment variables loaded from node.env"
        log_info "Node index: $NODE_INDEX"
    else
        log_warning "node.env file not found at /data/node.env"
        # Set default values
        NODE_INDEX="0"
    fi
    
    tee /etc/systemd/system/fastevm-execution.service > /dev/null << EOF
[Unit]
Description=FastEVM Execution Client
After=network.target

[Service]
Type=simple
User=ubuntu
Group=ubuntu
WorkingDirectory=/data
EnvironmentFile=/data/node.env
Environment=HTTP_PORT=8545
Environment=WS_PORT=8546
Environment=ENGINE_PORT=8551
Environment=P2P_PORT=30303
ExecStart=/usr/local/bin/fastevm-execution node \
    --chain /data/config/genesis.json \
    --datadir /data/execution \
    --engine.always-process-payload-attributes-on-canonical-head \
    --http \
    --http.api eth,net,web3,admin,debug \
    --http.addr 0.0.0.0 \
    --http.port ${HTTP_PORT} \
    --http.corsdomain "*" \
    --ws \
    --ws.api eth,net,web3,admin,debug \
    --ws.addr 0.0.0.0 \
    --ws.port ${WS_PORT} \
    --ws.origins "*" \
    --txpool.max-new-txns 102400 \
    --txpool.max-account-slots 102400 \
    --txpool.max-pending-txns 102400 \
    --txpool.pending-max-count 102400 \
    --txpool.pending-max-size 128 \
    --txpool.max-new-pending-txs-notifications 102400 \
    --txpool.queued-max-count 102400 \
    --txpool.queued-max-size 128 \
    --authrpc.addr 0.0.0.0 \
    --authrpc.port ${ENGINE_PORT} \
    --authrpc.jwtsecret /data/execution/jwt.hex \
    --addr 0.0.0.0 \
    --port ${P2P_PORT} \
    --discovery.addr 0.0.0.0 \
    --discovery.port ${P2P_PORT} \
    --p2p-secret-key /data/execution/p2p/secret.key \
    --bootnodes ${BOOTNODES} \
    --enable-tx-subscription \
    --committed-subdags-per-block 10 \
    --block-build-interval-ms 100 \
    -$LOG_LEVEL
Restart=always
RestartSec=10
StandardOutput=append:/data/logs/fastevm-execution.log
StandardError=append:/data/logs/fastevm-execution.log

[Install]
WantedBy=multi-user.target
EOF

    # Create systemd service for consensus client
    log_info "Creating fastevm-consensus.service..."
    
    # Ensure log file exists with proper permissions
    touch /data/logs/fastevm-consensus.log
    chown ubuntu:ubuntu /data/logs/fastevm-consensus.log
    chmod 644 /data/logs/fastevm-consensus.log
    
    tee /etc/systemd/system/fastevm-consensus.service > /dev/null << 'EOF'
[Unit]
Description=FastEVM Consensus Client
After=network.target fastevm-execution.service

[Service]
Type=simple
User=ubuntu
Group=ubuntu
WorkingDirectory=/data
ExecStart=/bin/bash -c '/usr/local/bin/fastevm-consensus start --config /data/config/node.yml >> /data/logs/fastevm-consensus.log 2>&1'
Restart=always
RestartSec=10
StandardOutput=append:/data/logs/fastevm-consensus.log
StandardError=append:/data/logs/fastevm-consensus.log

[Install]
WantedBy=multi-user.target
EOF

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

echo "=== Log Files ==="
if [ -f "/data/logs/fastevm-execution.log" ]; then
    echo "Execution log: /data/logs/fastevm-execution.log ($(wc -l < /data/logs/fastevm-execution.log) lines, $(du -h /data/logs/fastevm-execution.log | cut -f1))"
else
    echo "Execution log: Not found"
fi

if [ -f "/data/logs/fastevm-consensus.log" ]; then
    echo "Consensus log: /data/logs/fastevm-consensus.log ($(wc -l < /data/logs/fastevm-consensus.log) lines, $(du -h /data/logs/fastevm-consensus.log | cut -f1))"
else
    echo "Consensus log: Not found"
fi
echo ""

echo "=== Recent Execution Logs ==="
if [ -f "/data/logs/fastevm-execution.log" ]; then
    tail -n 5 /data/logs/fastevm-execution.log
else
    echo "Execution log file not found"
fi
echo ""

echo "=== Recent Consensus Logs ==="
if [ -f "/data/logs/fastevm-consensus.log" ]; then
    tail -n 5 /data/logs/fastevm-consensus.log
else
    echo "Consensus log file not found"
fi
EOL

    chmod +x /usr/local/bin/fastevm-status.sh

    # Reload systemd daemon
    log_info "Reloading systemd daemon..."
    systemctl daemon-reload

    # Enable services
    log_info "Enabling services..."
    systemctl enable fastevm-execution
    systemctl enable fastevm-consensus

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
            if [ -f "/data/logs/fastevm-execution.log" ]; then
                tail -n "$lines" /data/logs/fastevm-execution.log
            else
                log_error "Execution log file not found: /data/logs/fastevm-execution.log"
            fi
            ;;
        "consensus"|"cons")
            log_info "Showing consensus client logs (last $lines lines)..."
            if [ -f "/data/logs/fastevm-consensus.log" ]; then
                tail -n "$lines" /data/logs/fastevm-consensus.log
            else
                log_error "Consensus log file not found: /data/logs/fastevm-consensus.log"
            fi
            ;;
        "all")
            log_info "Showing all logs (last $lines lines)..."
            echo "=== Execution Client Logs ==="
            if [ -f "/data/logs/fastevm-execution.log" ]; then
                tail -n "$lines" /data/logs/fastevm-execution.log
            else
                echo "Execution log file not found"
            fi
            echo ""
            echo "=== Consensus Client Logs ==="
            if [ -f "/data/logs/fastevm-consensus.log" ]; then
                tail -n "$lines" /data/logs/fastevm-consensus.log
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
            if [ -f "/data/logs/fastevm-execution.log" ]; then
                tail -f /data/logs/fastevm-execution.log
            else
                log_error "Execution log file not found: /data/logs/fastevm-execution.log"
            fi
            ;;
        "consensus"|"cons")
            log_info "Following consensus client logs..."
            if [ -f "/data/logs/fastevm-consensus.log" ]; then
                tail -f /data/logs/fastevm-consensus.log
            else
                log_error "Consensus log file not found: /data/logs/fastevm-consensus.log"
            fi
            ;;
        "all")
            log_info "Following all logs..."
            if [ -f "/data/logs/fastevm-execution.log" ] && [ -f "/data/logs/fastevm-consensus.log" ]; then
                tail -f /data/logs/fastevm-execution.log /data/logs/fastevm-consensus.log
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
            if [ -f "/data/logs/fastevm-execution.log" ]; then
                log_info "Rotating execution client logs..."
                mv /data/logs/fastevm-execution.log /data/logs/fastevm-execution.log.$(date +%Y%m%d_%H%M%S)
                touch /data/logs/fastevm-execution.log
                chown ubuntu:ubuntu /data/logs/fastevm-execution.log
                systemctl reload fastevm-execution
                log_success "Execution logs rotated"
            else
                log_error "Execution log file not found"
            fi
            ;;
        "consensus"|"cons")
            if [ -f "/data/logs/fastevm-consensus.log" ]; then
                log_info "Rotating consensus client logs..."
                mv /data/logs/fastevm-consensus.log /data/logs/fastevm-consensus.log.$(date +%Y%m%d_%H%M%S)
                touch /data/logs/fastevm-consensus.log
                chown ubuntu:ubuntu /data/logs/fastevm-consensus.log
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
