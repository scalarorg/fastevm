#!/bin/bash
set -euo pipefail

WORKSPACE_DIR="/opt/workspace"
REPO_GRAVITY_BENCH="https://github.com/scalarorg/gravity_bench.git"
REPO_NAME=$(basename "$REPO_GRAVITY_BENCH" .git)    
CHAIN_ID=7771625
LOG_LEVEL=info
FILE_STEM="log"
build_gravity_bench() {
    local REPO_NAME=$(basename "$REPO_GRAVITY_BENCH" .git)
    cd $WORKSPACE_DIR/$REPO_NAME
    cargo build --release
}

wait_until_nodes_ready() {
    local ips=("$@")
    local port=8545
    local check_timeout=5
    local retry_interval=3
    local max_attempts=60  # Maximum number of attempts (3 minutes with 3s interval)
    local attempt=0
    
    echo "Waiting for all nodes to be ready on port ${port}..."
    
    while [ $attempt -lt $max_attempts ]; do
        local all_ready=true
        local not_ready_ips=()
        
        for ip in "${ips[@]}"; do
            # Use bash's built-in /dev/tcp for connection test
            if ! timeout ${check_timeout} bash -c "echo > /dev/tcp/${ip}/${port}" 2>/dev/null; then
                all_ready=false
                not_ready_ips+=("${ip}:${port}")
            fi
        done
        
        if [ "$all_ready" = true ]; then
            echo "✅ All nodes are ready!"
            return 0
        fi
        
        attempt=$((attempt + 1))
        local ready_count=$((${#ips[@]} - ${#not_ready_ips[@]}))
        echo "[Attempt ${attempt}/${max_attempts}] ${ready_count}/${#ips[@]} nodes ready. Waiting for: ${not_ready_ips[*]}"
        sleep ${retry_interval}
    done
    
    echo ""
    echo "❌ Timeout: The following nodes are still not ready after ${max_attempts} attempts:"
    for failed in "${not_ready_ips[@]}"; do
        echo "  - ${failed}"
    done
    echo ""
    exit 1
}

start() {
    cd $WORKSPACE_DIR/$REPO_NAME
    # Read public IPs from file and replace localhost URLs in bench_config_fastevm.toml
    # Try multiple possible paths for public_ips.txt
    PUBLIC_IPS_FILE="${WORKSPACE_DIR}/public_ips.txt"
    if [ -f "$PUBLIC_IPS_FILE" ]; then
        echo "Reading public IPs from $PUBLIC_IPS_FILE"
        # Read IPs into array
        mapfile -t IPS < <(cat "$PUBLIC_IPS_FILE" | grep -v '^$')
        
        if [ ${#IPS[@]} -gt 0 ]; then
            echo "Found ${#IPS[@]} IPs, updating bench_config.toml"
            
            # Wait until all nodes are ready before proceeding
            wait_until_nodes_ready "${IPS[@]}"
            
            if [ -f "bench_config.toml" ]; then
                
                # Build the new nodes array content in a temporary file
                TMP_NODES=$(mktemp)
                echo "nodes = [" > "$TMP_NODES"
                for IP in "${IPS[@]}"; do
                    echo "    { rpc_url = \"http://${IP}:8545\", chain_id = ${CHAIN_ID} }," >> "$TMP_NODES"
                done
                # Remove trailing comma from last line and add closing bracket
                sed -i '$ s/,$//' "$TMP_NODES"
                echo "]" >> "$TMP_NODES"
                
                # Use awk to replace the nodes array section
                awk '
                    BEGIN { 
                        in_nodes = 0
                        # Read the new nodes content
                        while ((getline line < "'"$TMP_NODES"'") > 0) {
                            new_nodes = new_nodes line "\n"
                        }
                        close("'"$TMP_NODES"'")
                    }
                    /^nodes = \[/ {
                        in_nodes = 1
                        printf "%s", new_nodes
                        next
                    }
                    in_nodes {
                        # Skip lines until we find the closing bracket
                        if (/^\]/) {
                            in_nodes = 0
                        }
                        next
                    }
                    { print }
                ' bench_config.toml > bench_config_fastevm.toml
                
                # Clean up temporary file
                rm -f "$TMP_NODES"
                
                echo "✓ Updated bench_config_fastevm.toml with real IPs"
            else
                echo "⚠️  bench_config_fastevm.toml not found, skipping IP replacement"
            fi
        else
            echo "⚠️  No IPs found in $PUBLIC_IPS_FILE, using default configuration"
            exit 1
        fi
    else
        echo "⚠️  public_ips.txt not found (checked: $PUBLIC_IPS_FILE), using default configuration"
        exit 1
    fi
    source ./setup.sh
    RUST_LOG=${LOG_LEVEL:-info} gravity_bench --config bench_config_fastevm.toml
    echo "✅ Benchmark started successfully!"
}

show-log() {
    EXT="log"
    cd $WORKSPACE_DIR/$REPO_NAME
    latest_file=$(ls -1 "${FILE_STEM}".*.${EXT} 2>/dev/null | sort | tail -n 1)
    if [ -z "${latest_file}" ]; then
        echo "❌ No log file found"
        exit 1
    fi
    echo "Benchmark log file: ${latest_file}"
    tail -f "${latest_file}"
}

$@