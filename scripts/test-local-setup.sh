#!/bin/bash

# FastEVM Local Setup Test Script
# This script tests if the local development setup is working correctly

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

# Test counter
TESTS_PASSED=0
TESTS_FAILED=0

# Test function
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    log_info "Testing: $test_name"
    
    if eval "$test_command" > /dev/null 2>&1; then
        log_success "✅ $test_name - PASSED"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        log_error "❌ $test_name - FAILED"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

# Test prerequisites
test_prerequisites() {
    log_info "Testing prerequisites..."
    
    run_test "Rust/Cargo installed" "command -v cargo"
    run_test "OpenSSL installed" "command -v openssl"
    run_test "jq installed" "command -v jq"
    run_test "curl installed" "command -v curl"
    run_test "make installed" "command -v make"
}

# Test project structure
test_project_structure() {
    log_info "Testing project structure..."
    
    run_test "Project root exists" "[ -d '$PROJECT_ROOT' ]"
    run_test "Cargo.toml exists" "[ -f '$PROJECT_ROOT/Cargo.toml' ]"
    run_test "Scripts directory exists" "[ -d '$PROJECT_ROOT/scripts' ]"
    run_test "Execution client exists" "[ -d '$PROJECT_ROOT/execution-client' ]"
    run_test "Consensus client exists" "[ -d '$PROJECT_ROOT/consensus-client' ]"
    run_test "Local dev script exists" "[ -f '$PROJECT_ROOT/scripts/local-dev.sh' ]"
    run_test "Start network script exists" "[ -f '$PROJECT_ROOT/scripts/local-network.sh' ]"
    run_test "Makefile exists" "[ -f '$PROJECT_ROOT/Makefile' ]"
}

# Test script permissions
test_script_permissions() {
    log_info "Testing script permissions..."
    
    run_test "Local dev script executable" "[ -x '$PROJECT_ROOT/scripts/local-dev.sh' ]"
    run_test "Start network script executable" "[ -x '$PROJECT_ROOT/scripts/local-network.sh' ]"
}

# Test build system
test_build_system() {
    log_info "Testing build system..."
    
    cd "$PROJECT_ROOT"
    
    run_test "Cargo check passes" "cargo check --workspace"
    run_test "Makefile help works" "make help > /dev/null"
    run_test "Local network targets exist" "make help | grep -q 'local-network'"
}

# Test network initialization
test_network_initialization() {
    log_info "Testing network initialization..."
    
    cd "$PROJECT_ROOT"
    
    # Clean up any existing data
    make local-cleanup > /dev/null 2>&1 || true
    
    # Test initialization
    run_test "Network initialization works" "make local-init"
    
    # Check if data directories were created
    run_test "Execution data directories created" "[ -d '.local-data/execution1' ]"
    run_test "Consensus data directories created" "[ -d '.local-data/consensus1' ]"
    run_test "Logs directory created" "[ -d '.local-logs' ]"
    run_test "PIDs directory created" "[ -d '.local-pids' ]"
}

# Test network startup (brief)
test_network_startup() {
    log_info "Testing network startup (brief test)..."
    
    cd "$PROJECT_ROOT"
    
    # Start network in background
    log_info "Starting network for 10 seconds..."
    timeout 10s make local-network > /dev/null 2>&1 &
    local network_pid=$!
    
    # Wait a bit for startup
    sleep 5
    
    # Test if processes are running
    run_test "Network processes started" "[ -d '.local-pids' ] && [ \"\$(ls -A .local-pids 2>/dev/null)\" ]"
    
    # Stop the network
    make local-stop > /dev/null 2>&1 || true
    wait $network_pid 2>/dev/null || true
}

# Test cleanup
test_cleanup() {
    log_info "Testing cleanup..."
    
    cd "$PROJECT_ROOT"
    
    run_test "Cleanup works" "make local-cleanup"
    run_test "Data directories removed" "[ ! -d '.local-data' ]"
    run_test "Logs directory removed" "[ ! -d '.local-logs' ]"
    run_test "PIDs directory removed" "[ ! -d '.local-pids' ]"
}

# Main test function
main() {
    echo "🧪 FastEVM Local Setup Test"
    echo "=========================="
    echo
    
    test_prerequisites
    echo
    
    test_project_structure
    echo
    
    test_script_permissions
    echo
    
    test_build_system
    echo
    
    test_network_initialization
    echo
    
    test_network_startup
    echo
    
    test_cleanup
    echo
    
    # Summary
    echo "📊 Test Summary"
    echo "==============="
    echo "Tests passed: $TESTS_PASSED"
    echo "Tests failed: $TESTS_FAILED"
    echo "Total tests: $((TESTS_PASSED + TESTS_FAILED))"
    echo
    
    if [ $TESTS_FAILED -eq 0 ]; then
        log_success "🎉 All tests passed! Local development setup is ready."
        echo
        echo "Next steps:"
        echo "  1. Run 'make local-network' to start the network"
        echo "  2. Run 'make local-status' to check status"
        echo "  3. Run 'make local-logs' to view logs"
        echo "  4. Read LOCAL_DEVELOPMENT.md for more details"
        exit 0
    else
        log_error "❌ Some tests failed. Please check the errors above."
        echo
        echo "Common fixes:"
        echo "  1. Install missing prerequisites"
        echo "  2. Make sure you're in the project root directory"
        echo "  3. Run 'chmod +x scripts/*.sh' to fix permissions"
        echo "  4. Run 'cargo clean && cargo build' to fix build issues"
        exit 1
    fi
}

# Run main function
main "$@"
