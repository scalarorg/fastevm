#!/bin/bash
# =============================================================================
# FastEVM Execution Client Test Runner
# =============================================================================
# Usage: ./scripts/run-tests.sh [command] [options]
#
# Commands:
#   unit        Run unit tests only
#   integration Run integration tests only  
#   bench       Run bench config tests (no node required)
#   bench-full  Run full 5-minute benchmark (builds binaries, starts node)
#   bench-quick Run quick 30-second benchmark (requires running node)
#   build       Build all benchmark binaries only
#   all         Run all tests
#   full        Run all tests including ignored (requires running node)
#
# Options:
#   --verbose   Show test output
#   --debug     Build/run in debug mode (default is release)
#   --help      Show this help message
#
# Examples:
#   ./scripts/run-tests.sh              # Run all quick tests (release mode)
#   ./scripts/run-tests.sh bench        # Run bench config tests
#   ./scripts/run-tests.sh build        # Build benchmark binaries (release)
#   ./scripts/run-tests.sh build --debug  # Build benchmark binaries (debug)
#   ./scripts/run-tests.sh bench-full   # Build + run full benchmark (release)
#   ./scripts/run-tests.sh full         # Run all tests including integration
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default options (release mode by default)
VERBOSE=""
RELEASE="--release"
BUILD_MODE="release"
COMMAND="all"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        unit|integration|bench|bench-full|bench-quick|build|all|full)
            COMMAND="$1"
            shift
            ;;
        --verbose|-v)
            VERBOSE="-- --nocapture"
            shift
            ;;
        --debug|-d)
            RELEASE=""
            BUILD_MODE="debug"
            shift
            ;;
        --help|-h)
            head -30 "$0" | tail -28
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Print header
print_header() {
    echo -e "${BLUE}=============================================${NC}"
    echo -e "${BLUE}  FastEVM Execution Client Tests${NC}"
    echo -e "${BLUE}=============================================${NC}"
    echo ""
}

# Print section
print_section() {
    echo ""
    echo -e "${YELLOW}>>> $1${NC}"
    echo ""
}

# Run unit tests
run_unit_tests() {
    print_section "Running Unit Tests"
    cargo test --package fastevm-execution --lib $RELEASE $VERBOSE
}

# Run integration tests (consensus, payload, rpc)
run_integration_tests() {
    print_section "Running Integration Tests"
    
    echo -e "${GREEN}  • Consensus Pool Tests${NC}"
    cargo test --package fastevm-execution --test consensus_pool_tests $RELEASE $VERBOSE
    
    echo -e "${GREEN}  • Payload Tests${NC}"
    cargo test --package fastevm-execution --test payload_tests $RELEASE $VERBOSE
    
    echo -e "${GREEN}  • RPC Tests${NC}"
    cargo test --package fastevm-execution --test rpc_tests $RELEASE $VERBOSE
}

# Run bench integration tests (config tests only, no node required)
run_bench_tests() {
    print_section "Running Bench Config Tests (no node required)"
    cargo test --package fastevm-execution --test bench_tests $RELEASE $VERBOSE
}

# Build all required binaries for benchmarking
build_bench_binaries() {
    print_section "Building Required Binaries"
    
    # Use RELEASE flag for cargo build
    BUILD_FLAG="$RELEASE"
    
    echo -e "${GREEN}Build mode: ${BUILD_MODE}${NC}"
    echo ""
    
    # Build fastevm-gravity node
    echo -e "${BLUE}[1/3] Building fastevm-gravity...${NC}"
    if cargo build --package fastevm-execution --bin fastevm-gravity $BUILD_FLAG; then
        echo -e "${GREEN}  ✓ fastevm-gravity built successfully${NC}"
    else
        echo -e "${RED}  ✗ Failed to build fastevm-gravity${NC}"
        exit 1
    fi
    
    # Build fastevm-bench tool
    echo -e "${BLUE}[2/3] Building fastevm-bench...${NC}"
    if cargo build --package fastevm-execution --bin fastevm-bench $BUILD_FLAG; then
        echo -e "${GREEN}  ✓ fastevm-bench built successfully${NC}"
    else
        echo -e "${RED}  ✗ Failed to build fastevm-bench${NC}"
        exit 1
    fi
    
    # Check for gravity_bench in PATH
    echo -e "${BLUE}[3/3] Checking gravity_bench...${NC}"
    if command -v gravity_bench &> /dev/null; then
        GRAVITY_BENCH_VERSION=$(gravity_bench --version 2>/dev/null || echo "unknown")
        echo -e "${GREEN}  ✓ gravity_bench found: ${GRAVITY_BENCH_VERSION}${NC}"
    else
        echo -e "${YELLOW}  ⚠ gravity_bench not found in PATH${NC}"
        echo -e "${YELLOW}    Install from: https://github.com/Galxe/gravity_bench.git${NC}"
        echo -e "${YELLOW}    Tests will run in simulation mode${NC}"
    fi
    
    echo ""
    echo -e "${GREEN}All binaries ready!${NC}"
    
    # Print binary locations
    BINARY_DIR="target/${BUILD_MODE}"
    
    echo ""
    echo -e "${BLUE}Binary locations:${NC}"
    echo "  fastevm-gravity: ${BINARY_DIR}/fastevm-gravity"
    echo "  fastevm-bench:   ${BINARY_DIR}/fastevm-bench"
    
    # Export for use in tests
    export FASTEVM_BINARY_DIR="${BINARY_DIR}"
}

# Run full 5-minute benchmark (starts node, runs gravity_bench)
run_bench_full() {
    print_section "Running Full 5-Minute Benchmark"
    
    # Build all required binaries first
    build_bench_binaries
    
    print_section "Starting Benchmark Test"
    echo -e "${YELLOW}This test will:${NC}"
    echo "  1. Start fastevm-gravity node"
    echo "  2. Run gravity_bench for 5 minutes"
    echo "  3. Check mined transactions"
    echo "  4. Verify faucet balances"
    echo "  5. Shutdown node"
    echo ""
    
    cargo test --package fastevm-execution --test bench_tests test_full_benchmark_5_minutes $RELEASE -- --ignored --nocapture
}

# Run quick 30-second benchmark (requires running node)
run_bench_quick() {
    print_section "Running Quick 30-Second Benchmark"
    echo -e "${YELLOW}Note: This requires a running FastEVM node on localhost:8545${NC}"
    echo ""
    cargo test --package fastevm-execution --test bench_tests test_quick_benchmark $RELEASE -- --ignored --nocapture
}

# Run all ignored bench tests (requires node)
run_bench_tests_full() {
    print_section "Running All Bench Integration Tests (including ignored)"
    echo -e "${YELLOW}Note: This requires fastevm-gravity binary and gravity_bench${NC}"
    echo ""
    cargo test --package fastevm-execution --test bench_tests $RELEASE -- --ignored --nocapture
}

# Run all tests
run_all_tests() {
    run_unit_tests
    run_integration_tests
    run_bench_tests
}

# Main execution
print_header

case $COMMAND in
    unit)
        run_unit_tests
        ;;
    integration)
        run_integration_tests
        ;;
    bench)
        run_bench_tests
        ;;
    bench-full)
        run_bench_full
        ;;
    bench-quick)
        run_bench_quick
        ;;
    build)
        build_bench_binaries
        ;;
    all)
        run_all_tests
        ;;
    full)
        run_all_tests
        run_bench_tests_full
        ;;
    *)
        echo -e "${RED}Unknown command: $COMMAND${NC}"
        exit 1
        ;;
esac

echo ""
echo -e "${GREEN}=============================================${NC}"
echo -e "${GREEN}  All tests completed successfully!${NC}"
echo -e "${GREEN}=============================================${NC}"

