#!/bin/bash
set -e

# FastEVM Demo Script
# This script demonstrates the full functionality of the FastEVM Engine API Bridge

echo "🚀 FastEVM Engine API Bridge Demo"
echo "=================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
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

# Defaults and CLI parsing
SCENARIO="all"
NON_INTERACTIVE=false
KEEP_CLUSTER=false
EXTRA_ARGS=()

print_usage() {
    cat <<EOF
Usage: $0 [scenario] [options]

Scenarios:
  all            Run full demo (build, tests, single, perf, optional multi)
  build          Build project only
  tests          Run tests only
  single         Run single-node demo only
  perf           Run performance demo only
  multi          Run multi-node (docker-compose) demo only
  exec           Start execution client only (foreground)
  consensus      Start consensus client only (foreground)
  help           Show this help

Options:
  -y, --yes               Non-interactive mode (assume "No" for prompts)
  --keep                  With -y/--yes, keep multi-node cluster running

Notes:
  For 'exec' and 'consensus' scenarios, any additional arguments are passed through
  to the respective binaries.

Examples:
  $0 single
  $0 multi -y                   # run and tear down cluster without prompting
  $0 multi -y --keep            # run and keep cluster running
  $0 exec -- --http.port 8551   # pass flags to execution client
  $0 consensus -- --execution-url http://127.0.0.1:8551
EOF
}

# Parse args: first non-flag is scenario
while [[ $# -gt 0 ]]; do
    case "$1" in
        all|build|tests|single|perf|multi|exec|consensus|help)
            SCENARIO="$1"; shift ;;
        -y|--yes)
            NON_INTERACTIVE=true; shift ;;
        --keep)
            KEEP_CLUSTER=true; shift ;;
        -h|--help)
            SCENARIO="help"; shift ;;
        --)
            shift
            while [[ $# -gt 0 ]]; do EXTRA_ARGS+=("$1"); shift; done ;;
        *)
            # Collect unknowns as extra args for pass-through scenarios
            EXTRA_ARGS+=("$1"); shift ;;
    esac
done

# Check prerequisites
check_prerequisites() {
    # $1: "docker" to also check docker tooling
    log_info "Checking prerequisites..."
    
    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo not found. Please install Rust: https://rustup.rs/"
        exit 1
    fi
    
    if [[ "$1" == "docker" ]]; then
        if ! command -v docker &> /dev/null; then
            log_error "Docker not found. Please install Docker: https://docker.com/"
            exit 1
        fi
        if ! command -v docker-compose &> /dev/null; then
            log_error "Docker Compose not found. Please install Docker Compose"
            exit 1
        fi
    fi
    
    log_success "All required prerequisites found!"
}

# Build the project
build_project() {
    log_info "Building FastEVM project..."
    
    cargo build --release
    if [ $? -eq 0 ]; then
        log_success "Build completed successfully!"
    else
        log_error "Build failed!"
        exit 1
    fi
}

# Run tests
run_tests() {
    log_info "Running comprehensive tests..."
    
    # Unit tests
    log_info "Running unit tests..."
    cargo test --workspace --lib
    
    # Integration tests
    log_info "Running integration tests..."
    cargo test --test integration_test
    
    log_success "All tests passed!"
}

# Demo single node functionality
demo_single_node() {
    log_info "🔧 Demonstrating single node functionality..."
    
    # Start execution client in background
    log_info "Starting execution client on 0.0.0.0:8551..."
    ./target/release/execution-client --port 8551 --http.addr 0.0.0.0 --log-level info &
    EXEC_PID=$!
    
    # Give it time to start
    sleep 3
    
    # Test execution client
    log_info "Testing execution client connectivity..."
    response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"engine_newPayloadV2","params":[{"parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","feeRecipient":"0x0000000000000000000000000000000000000000","stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","prevRandao":"0x0000000000000000000000000000000000000000000000000000000000000000","blockNumber":"0x1","gasLimit":"0x1c9c380","gasUsed":"0x5208","timestamp":"0x499602d2","extraData":"0x","baseFeePerGas":"0x3b9aca00","blockHash":"0x0000000000000000000000000000000000000000000000000000000000000000","transactions":[],"withdrawals":null}],"id":1}' \
        http://localhost:8551)
    
    if echo "$response" | grep -q '"status":"VALID"'; then
        log_success "✅ Execution client responding correctly!"
        echo "Response: $response"
    else
        log_warning "⚠️  Unexpected response: $response"
    fi
    
    # Start consensus client in background
    log_info "Starting consensus client..."
    ./target/release/consensus-client \
        --execution-url http://127.0.0.1:8551 \
        --poll-interval 500 \
        --log-level info &
    CONSENSUS_PID=$!
    
    # Let them run for a demonstration
    log_info "Running integrated system for 15 seconds..."
    sleep 15
    
    log_success "✅ Single node demo completed!"
    
    # Cleanup
    log_info "Cleaning up single node demo..."
    kill $EXEC_PID $CONSENSUS_PID 2>/dev/null || true
    sleep 2
}

# Start execution client only (foreground)
start_execution_only() {
    log_info "🔧 Starting execution client only..."
    check_prerequisites
    build_project
    log_info "Execution client starting with args: ${EXTRA_ARGS[*]}"
    exec ./target/release/execution-client "${EXTRA_ARGS[@]}"
}

# Start consensus client only (foreground)
start_consensus_only() {
    log_info "🔧 Starting consensus client only..."
    check_prerequisites
    build_project
    log_info "Consensus client starting with args: ${EXTRA_ARGS[*]}"
    exec ./target/release/consensus-client "${EXTRA_ARGS[@]}"
}

# Demo multi-node functionality
demo_multi_node() {
    log_info "🌐 Demonstrating multi-node functionality..."
    
    # Build Docker images
    log_info "Building Docker images..."
    docker-compose build
    
    # Start multi-node cluster
    log_info "Starting 4-node cluster..."
    docker-compose up -d
    
    # Wait for services to be ready
    log_info "Waiting for services to initialize..."
    sleep 20
    
    # Test all nodes
    for port in 8551 8552 8553 8554; do
        log_info "Testing node on port $port..."
        
        response=$(curl -s -X POST \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"engine_newPayloadV2","params":[{"parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","feeRecipient":"0x0000000000000000000000000000000000000000","stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","prevRandao":"0x0000000000000000000000000000000000000000000000000000000000000000","blockNumber":"0x1","gasLimit":"0x1c9c380","gasUsed":"0x5208","timestamp":"0x499602d2","extraData":"0x","baseFeePerGas":"0x3b9aca00","blockHash":"0x0000000000000000000000000000000000000000000000000000000000000000","transactions":[],"withdrawals":null}],"id":1}' \
            http://localhost:$port)
        
        if echo "$response" | grep -q '"status":"VALID"'; then
            log_success "✅ Node $port responding correctly!"
        else
            log_warning "⚠️  Node $port unexpected response: $response"
        fi
    done
    
    # Show cluster status
    log_info "Cluster status:"
    docker-compose ps
    
    # Show some logs
    log_info "Recent logs (last 20 lines):"
    docker-compose logs --tail=20
    
    log_success "✅ Multi-node demo completed!"
    
    # Option to keep running
    if [[ "$NON_INTERACTIVE" == true ]]; then
        if [[ "$KEEP_CLUSTER" == true ]]; then
            log_info "Cluster is still running. Access nodes at:"
            echo "  - Node 0: http://localhost:8551"
            echo "  - Node 1: http://localhost:8552"
            echo "  - Node 2: http://localhost:8553"
            echo "  - Node 3: http://localhost:8554"
            echo "  - Prometheus: http://localhost:9090"
            echo
            echo "To stop the cluster later, run: docker-compose down"
        else
            log_info "Shutting down cluster..."
            docker-compose down
            log_success "Cluster shut down."
        fi
    else
        echo
        read -p "Keep the cluster running for manual testing? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log_info "Shutting down cluster..."
            docker-compose down
            log_success "Cluster shut down."
        else
            log_info "Cluster is still running. Access nodes at:"
            echo "  - Node 0: http://localhost:8551"
            echo "  - Node 1: http://localhost:8552" 
            echo "  - Node 2: http://localhost:8553"
            echo "  - Node 3: http://localhost:8554"
            echo "  - Prometheus: http://localhost:9090"
            echo
            echo "To stop the cluster later, run: docker-compose down"
        fi
    fi
}

# Performance demonstration
demo_performance() {
    log_info "🏃 Performance demonstration..."
    
    # Start single execution client for perf test
    log_info "Starting execution client for performance test..."
    ./target/release/execution-client --port 8561 --log-level warn &
    PERF_PID=$!
    sleep 3
    
    # Send multiple requests concurrently
    log_info "Sending 100 concurrent requests..."
    
    start_time=$(date +%s.%N)
    
    for i in {1..100}; do
        (curl -s -X POST \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"engine_newPayloadV2","params":[{"parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","feeRecipient":"0x0000000000000000000000000000000000000000","stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","prevRandao":"0x0000000000000000000000000000000000000000000000000000000000000000","blockNumber":"0x'$(printf "%x" $i)'","gasLimit":"0x1c9c380","gasUsed":"0x5208","timestamp":"0x499602d2","extraData":"0x","baseFeePerGas":"0x3b9aca00","blockHash":"0x0000000000000000000000000000000000000000000000000000000000000000","transactions":[],"withdrawals":null}],"id":'$i'}' \
            http://localhost:8561 > /dev/null) &
    done
    
    # Wait for all requests to complete
    wait
    
    end_time=$(date +%s.%N)
    duration=$(echo "$end_time - $start_time" | bc -l)
    rps=$(echo "scale=2; 100 / $duration" | bc -l)
    
    log_success "✅ Completed 100 requests in ${duration}s (${rps} req/s)"
    
    # Cleanup
    kill $PERF_PID 2>/dev/null || true
}

# Main demo flow
main() {
    echo
    log_info "Starting FastEVM demonstration (scenario: ${SCENARIO})..."
    echo

    case "$SCENARIO" in
        help)
            print_usage
            exit 0
            ;;
        build)
            check_prerequisites
            build_project
            ;;
        tests)
            check_prerequisites
            run_tests
            ;;
        single)
            check_prerequisites
            build_project
            demo_single_node
            ;;
        perf)
            check_prerequisites
            build_project
            demo_performance
            ;;
        execution)
            start_execution_only
            ;;
        consensus)
            start_consensus_only
            ;;
        multi)
            check_prerequisites docker
            build_project
            demo_multi_node
            ;;
        all)
            check_prerequisites
            build_project
            echo
            run_tests
            echo
            demo_single_node
            echo
            demo_performance
            echo
            if [[ "$NON_INTERACTIVE" == true ]]; then
                log_info "Skipping multi-node demo in non-interactive mode."
            else
                read -p "Run multi-node Docker demo? This will build Docker images (y/N): " -n 1 -r
                echo
                if [[ $REPLY =~ ^[Yy]$ ]]; then
                    check_prerequisites docker
                    demo_multi_node
                else
                    log_info "Skipping multi-node demo."
                fi
            fi
            ;;
        *)
            log_error "Unknown scenario: ${SCENARIO}"
            print_usage
            exit 1
            ;;
    esac

    echo
    log_success "🎉 FastEVM ${SCENARIO} completed successfully!"
    echo
    log_info "Next steps:"
    echo "  1. Review the generated documentation: cargo doc --open"
    echo "  2. Run benchmarks: cargo bench"
    echo "  3. Explore the source code in fastevm/"
    echo "  4. Check out the README.md for detailed documentation"
    echo
    log_info "Thank you for trying FastEVM! 🚀"
}

# Handle script interruption
trap 'log_warning "Demo interrupted. Cleaning up..."; kill $(jobs -p) 2>/dev/null; docker-compose down 2>/dev/null; exit 1' INT TERM

# Run main function
main "$@"