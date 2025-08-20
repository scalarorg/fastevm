#!/bin/bash

# FastEVM Network Startup Script
# This script manages the deployment of 4 execution nodes and 4 consensus nodes

set -e

echo "🚀 Starting FastEVM Network..."

# Function to check if Docker is running
check_docker() {
    if ! docker info > /dev/null 2>&1; then
        echo "❌ Docker is not running. Please start Docker and try again."
        exit 1
    fi
    
    # Check if the required builder image exists
    check_builder_image
}

# Function to check if the builder image exists and build it if needed
check_builder_image() {
    local image_name="scalarorg/fastevm-builder:latest"
    
    echo "🔍 Checking if builder image exists..."
    if ! docker image inspect "$image_name" > /dev/null 2>&1; then
        echo "📦 Builder image not found. Building it now..."
        echo "   This may take a few minutes on first run..."
        if ! make docker-builder; then
            echo "❌ Failed to build builder image. Please check the build logs."
            exit 1
        fi
        echo "✅ Builder image built successfully!"
    else
        echo "✅ Builder image already exists."
    fi
}

# Function to clean up existing containers
cleanup() {
    echo "🧹 Cleaning up existing containers..."
    docker compose down -v 2>/dev/null || true
    docker system prune -f 2>/dev/null || true
}

# Function to wait for service to be healthy
wait_for_healthy() {
    local service=$1
    local max_attempts=60
    local attempt=1
    
    echo "   Waiting for $service to be healthy..."
    
    while [ $attempt -le $max_attempts ]; do
        local status=$(docker compose ps $service -a --format "table {{.Status}}" | tail -n +2)
        
        if echo "$status" | grep -q "healthy"; then
            echo "   ✅ $service is healthy!"
            return 0
        elif echo "$status" | grep -q "unhealthy"; then
            echo "   ❌ $service is unhealthy: $status"
            return 1
        elif echo "$status" | grep -q "starting"; then
            echo "   ⏳ Attempt $attempt/$max_attempts - $service is starting up (health: starting)..."
        else
            echo "   ⏳ Attempt $attempt/$max_attempts - $service status: $status"
        fi
        
        sleep 5
        attempt=$((attempt + 1))
    done
    
    echo "   ❌ $service failed to become healthy after $max_attempts attempts"
    return 1
}

# Function to wait for init container completion
wait_for_init_completion() {
    local service=$1
    local max_attempts=60
    local attempt=1
    
    echo "   Waiting for $service to complete..."
    
    while [ $attempt -le $max_attempts ]; do
        local status=$(docker compose ps -a $service --format "table {{.Status}}" | tail -n +2)
        
        if echo "$status" | grep -q "Exited (0)"; then
            echo "   ✅ $service completed successfully!"
            return 0
        elif echo "$status" | grep -q "Exited"; then
            echo "   ❌ $service failed with exit code: $status"
            return 1
        fi
        
        echo "   ⏳ Attempt $attempt/$max_attempts - $service still running..."
        sleep 3
        attempt=$((attempt + 1))
    done
    
    echo "   ❌ $service did not complete after $max_attempts attempts"
    return 1
}
extract_bootnode() {
    docker logs fastevm-execution1 \
  | grep "P2P networking initialized" \
  | awk -F'enode://' '{print $2}' \
  | awk -F'@' '{print $1}'
}
# Function to start the network
start_network() {
    echo "📦 Starting init containers..."
    docker compose up -d genesis-init consensus-init
    
    echo "⏳ Waiting for init containers to complete..."
    echo "   Waiting for genesis-init..."
    if ! wait_for_init_completion genesis-init; then
        echo "❌ Genesis init failed. Check logs:"
        docker compose logs genesis-init
        exit 1
    fi
    
    echo "   Waiting for consensus-init..."
    if ! wait_for_init_completion consensus-init; then
        echo "❌ Consensus init failed. Check logs:"
        docker compose logs consensus-init
        exit 1
    fi
    
    echo "🔄 Starting execution nodes..."
    docker compose up -d execution-node1
    
    echo "⏳ Waiting for execution nodes to be healthy..."
    echo "   Waiting for execution-node1..."
    if ! wait_for_healthy execution-node1; then
        echo "❌ Execution node 1 failed to become healthy. Check logs:"
        docker compose logs execution-node1 --tail=50
        exit 1
    fi
    echo " Extract node1 bootnode then start other execution nodes"
    export BOOTNODE=$(extract_bootnode)
    echo " Bootnode: $BOOTNODE"

    docker compose up -d execution-node2 execution-node3 execution-node4
    echo "   Waiting for execution-node2..."
    if ! wait_for_healthy execution-node2; then
        echo "❌ Execution node 2 failed to become healthy. Check logs:"
        docker compose logs execution-node2 --tail=50
        exit 1
    fi
    
    echo "   Waiting for execution-node3..."
    if ! wait_for_healthy execution-node3; then
        echo "❌ Execution node 3 failed to become healthy. Check logs:"
        docker compose logs execution-node3 --tail=50
        exit 1
    fi
    
    echo "   Waiting for execution-node4..."
    if ! wait_for_healthy execution-node4; then
        echo "❌ Execution node 4 failed to become healthy. Check logs:"
        docker compose logs execution-node4 --tail=50
        exit 1
    fi

    echo "🔄 Starting consensus nodes..."
    docker compose up -d consensus-node1 consensus-node2 consensus-node3 consensus-node4
    
    echo "⏳ Waiting for consensus nodes to start..."
    sleep 10
    
    echo "📊 Starting monitoring service..."
 
    docker compose up -d monitoring
    
    echo "✅ Network startup complete!"
}

# Function to show status
show_status() {
    echo "📊 Network Status:"
    docker compose ps
    echo ""
    echo "🌐 Network Information:"
    echo "   Execution Nodes:"
    echo "     Node 1: http://localhost:8545 (RPC), http://localhost:8551 (Engine API)"
    echo "     Node 2: http://localhost:8547 (RPC), http://localhost:8552 (Engine API)"
    echo "     Node 3: http://localhost:8549 (RPC), http://localhost:8553 (Engine API)"
    echo "     Node 4: http://localhost:8555 (RPC), http://localhost:8554 (Engine API)"
    echo "   Consensus Nodes: 172.20.0.10-13"
    echo "   Monitoring: http://localhost:8080"
}

# Function to stop the network
stop_network() {
    echo "🛑 Stopping FastEVM Network..."
    docker compose down
    echo "✅ Network stopped."
}

# Function to show logs
show_logs() {
    local service=${1:-""}
    if [ -z "$service" ]; then
        echo "📋 Showing all logs (use Ctrl+C to exit)..."
        docker compose logs -f
    else
        echo "📋 Showing logs for $service (use Ctrl+C to exit)..."
        docker compose logs -f "$service"
    fi
}

# Function to check init container status
check_init_status() {
    echo "📋 Init Container Status:"
    docker compose ps genesis-init consensus-init
    echo ""
    echo "📋 Init Container Logs:"
    echo "=== Genesis Init Logs ==="
    docker compose logs genesis-init
    echo ""
    echo "=== Consensus Init Logs ==="
    docker compose logs consensus-init
}

# Function to test individual components
test_component() {
    local component=$1
    echo "🧪 Testing $component..."
    
    case $component in
        "genesis-init")
            docker compose up -d genesis-init
            sleep 5
            docker compose ps genesis-init
            docker compose logs genesis-init
            ;;
        "consensus-init")
            docker compose up -d consensus-init
            sleep 5
            docker compose ps consensus-init
            docker compose logs consensus-init
            ;;
        "execution-node1")
            docker compose up -d execution-node1
            sleep 10
            docker compose ps execution-node1
            docker compose logs execution-node1 --tail=20
            ;;
        *)
            echo "❌ Unknown component: $component"
            return 1
            ;;
    esac
}

# Main script logic
case "${1:-start}" in
    "start")
        check_docker
        cleanup
        start_network
        show_status
        ;;
    "stop")
        stop_network
        ;;
    "restart")
        stop_network
        sleep 2
        start_network
        show_status
        ;;
    "status")
        show_status
        ;;
    "logs")
        show_logs "$2"
        ;;
    "cleanup")
        cleanup
        ;;
    "init-status")
        check_init_status
        ;;
    "test")
        if [ -z "$2" ]; then
            echo "❌ Please specify a component to test"
            echo "   Available components: genesis-init, consensus-init, execution-node1"
            exit 1
        fi
        test_component "$2"
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|logs|cleanup|init-status|test}"
        echo ""
        echo "Commands:"
        echo "  start        - Start the network (default)"
        echo "  stop         - Stop the network"
        echo "  restart      - Restart the network"
        echo "  status       - Show network status"
        echo "  logs         - Show logs (all services or specific service)"
        echo "  cleanup      - Clean up containers and volumes"
        echo "  init-status  - Check init container status and logs"
        echo "  test         - Test individual component (genesis-init|consensus-init|execution-node1)"
        echo ""
        echo "Examples:"
        echo "  $0 start"
        echo "  $0 logs execution-node1"
        echo "  $0 logs consensus-node1"
        echo "  $0 init-status"
        echo "  $0 test genesis-init"
        exit 1
        ;;
esac
