#!/bin/bash

# FastEVM Local Development Helper Script
# This script provides convenient commands for local development without Docker

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Logging functions
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

# Show usage
show_usage() {
    cat << EOF
FastEVM Local Development Helper

Usage: $0 <command> [options]

Commands:
  start           Start the local network (4 execution + 4 consensus nodes)
  stop            Stop the local network
  restart         Restart the local network
  status          Show network status
  logs [service]  Show logs (all or specific service)
  test            Test network connectivity
  clean           Clean up all local data
  init            Initialize node data only
  dev             Start in development mode (with auto-restart)
  
  build           Build the project
  check           Check code without building
  test-unit       Run unit tests
  test-integration Run integration tests
  fmt             Format code
  clippy          Run clippy linter

Services:
  execution-node1, execution-node2, execution-node3, execution-node4
  consensus-node1, consensus-node2, consensus-node3, consensus-node4

Examples:
  $0 start                    # Start the network
  $0 logs execution-node1     # Show logs for execution node 1
  $0 test                     # Test network connectivity
  $0 dev                      # Start in development mode
  $0 clean                    # Clean up everything

Network Ports:
  Execution RPC:    8545, 8547, 8549, 8555
  Engine API:       8551, 8552, 8553, 8554
  Consensus:        26657, 26658, 26659, 26660
  P2P:              30303, 30304, 30305, 30306

EOF
}

# Check if network is running
is_network_running() {
    local pids_dir="$PROJECT_ROOT/.local-pids"
    if [ -d "$pids_dir" ] && [ "$(ls -A "$pids_dir" 2>/dev/null)" ]; then
        return 0
    else
        return 1
    fi
}

# Start the network
start_network() {
    if is_network_running; then
        log_warning "Network is already running. Use 'restart' to restart it."
        return 1
    fi
    
    log_info "Starting FastEVM local network..."
    cd "$PROJECT_ROOT"
    make local-network
}

# Stop the network
stop_network() {
    if ! is_network_running; then
        log_warning "Network is not running."
        return 1
    fi
    
    log_info "Stopping FastEVM local network..."
    cd "$PROJECT_ROOT"
    make local-stop
}

# Restart the network
restart_network() {
    log_info "Restarting FastEVM local network..."
    stop_network || true
    sleep 2
    start_network
}

# Show network status
show_status() {
    cd "$PROJECT_ROOT"
    make local-status
}

# Show logs
show_logs() {
    local service="$1"
    cd "$PROJECT_ROOT"
    
    if [ -n "$service" ]; then
        make local-logs SERVICE="$service"
    else
        make local-logs
    fi
}

# Test network connectivity
test_network() {
    cd "$PROJECT_ROOT"
    make local-test
}

# Clean up
cleanup() {
    log_info "Cleaning up local network data..."
    cd "$PROJECT_ROOT"
    make local-cleanup
}

# Initialize only
init_network() {
    log_info "Initializing local network data..."
    cd "$PROJECT_ROOT"
    make local-init
}

# Development mode
dev_mode() {
    log_info "Starting development mode..."
    cd "$PROJECT_ROOT"
    make local-dev
}

# Build project
build_project() {
    log_info "Building FastEVM project..."
    cd "$PROJECT_ROOT"
    make build-release
}

# Check code
check_code() {
    log_info "Checking code..."
    cd "$PROJECT_ROOT"
    make check
}

# Run unit tests
run_unit_tests() {
    log_info "Running unit tests..."
    cd "$PROJECT_ROOT"
    make test
}

# Run integration tests
run_integration_tests() {
    log_info "Running integration tests..."
    cd "$PROJECT_ROOT"
    make integration-test
}

# Format code
format_code() {
    log_info "Formatting code..."
    cd "$PROJECT_ROOT"
    make fmt
}

# Run clippy
run_clippy() {
    log_info "Running clippy linter..."
    cd "$PROJECT_ROOT"
    make clippy
}

# Main script logic
case "${1:-help}" in
    "start")
        start_network
        ;;
    "stop")
        stop_network
        ;;
    "restart")
        restart_network
        ;;
    "status")
        show_status
        ;;
    "logs")
        show_logs "$2"
        ;;
    "test")
        test_network
        ;;
    "clean")
        cleanup
        ;;
    "init")
        init_network
        ;;
    "dev")
        dev_mode
        ;;
    "build")
        build_project
        ;;
    "check")
        check_code
        ;;
    "test-unit")
        run_unit_tests
        ;;
    "test-integration")
        run_integration_tests
        ;;
    "fmt")
        format_code
        ;;
    "clippy")
        run_clippy
        ;;
    "help"|"-h"|"--help")
        show_usage
        ;;
    *)
        log_error "Unknown command: $1"
        echo
        show_usage
        exit 1
        ;;
esac
