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
ENV_FILE="${DATA_DIR}/node.env"
# Check if running as root or with sudo
check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "This script must be run as root or with sudo"
        exit 1
    fi
}
# Update BOOTNODES in node.env file
update-bootnodes() {
    export BOOTNODES="$1"

    if [ -z "$BOOTNODES" ]; then
        echo "❌ bootnodes: enode URLs are empty"
        exit 1
    fi

    echo "🔧 Updating BOOTNODES in $ENV_FILE"
    cat $ENV_FILE
    # Ensure env file exists
    touch "$ENV_FILE"

    # Remove existing BOOTNODES line (idempotent)
    sed -i '/^BOOTNODES=/d' "$ENV_FILE"

    # Append new BOOTNODES
    echo "BOOTNODES=\"$BOOTNODES\"" >> "$ENV_FILE"

    echo "✅ BOOTNODES updated"
    cat $ENV_FILE
}

# Install systemd services
install_services() {
    log_info "Installing systemd services..."
    sudo cp ./fastevm-*.service /etc/systemd/system/

    log_info "Reloading systemd..."
    sudo systemctl daemon-reload

    log_info "Enabling services..."
    sudo systemctl enable fastevm-execution fastevm-consensus

    log_info "Done"
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
        "update-bootnodes")
            update-bootnodes "$1"
            ;;
        "help"|"--help"|"-h")
            echo "Usage: $0 {install|start|restart|stop|status|logs|follow|rotate|update-bootnodes} [args]"
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
            echo "  update-bootnodes <bootnodes> - Update bootnodes"
            echo "  help                 - Show this help"
            echo ""
            echo "Examples:"
            echo "  $0 logs execution 100    # Show last 100 lines of execution logs"
            echo "  $0 follow consensus      # Follow consensus logs in real-time"
            echo "  $0 rotate all            # Rotate all log files"
            echo "  $0 update-bootnodes <bootnodes>"
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
