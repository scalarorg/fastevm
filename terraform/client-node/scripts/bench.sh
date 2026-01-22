#!/bin/bash
set -euo pipefail

WORKSPACE_DIR="/opt/workspace"
REPO_FASTEVM="https://github.com/scalarorg/fastevm.git"
REPO_GRAVITY_BENCH="https://github.com/scalarorg/gravity_bench.git"
CHAIN_ID=7771625
LOG_LEVEL=info
FILE_STEM="benchmark"
build_gravity_bench() {
    local REPO_NAME=$(basename "$REPO_GRAVITY_BENCH" .git)
    cd $WORKSPACE_DIR/$REPO_NAME
    cargo build --release
}

start_bench() {
    local REPO_NAME=$(basename "$REPO_GRAVITY_BENCH" .git)
    local REPO_NAME_FASTEVM=$(basename "$REPO_FASTEVM" .git)
    
    cd $WORKSPACE_DIR/$REPO_NAME
    
    # Read public IPs from file and replace localhost URLs in bench_config_fastevm.toml
    # Try multiple possible paths for public_ips.txt
    PUBLIC_IPS_FILE="$WORKSPACE_DIR/$REPO_NAME_FASTEVM/terraform/public_ips.txt"
    if [ -f "$PUBLIC_IPS_FILE" ]; then
        echo "Reading public IPs from $PUBLIC_IPS_FILE"
        # Read IPs into array
        mapfile -t IPS < <(cat "$PUBLIC_IPS_FILE" | grep -v '^$')
        
        if [ ${#IPS[@]} -gt 0 ]; then
            echo "Found ${#IPS[@]} IPs, updating bench_config.toml"
            
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
    RUST_LOG=${LOG_LEVEL:-info} cargo run --bin gravity_bench --release -- --config bench_config_fastevm.toml --log-path "./${FILE_STEM}.log" &
}

find_log_file() {
    EXT="log"
    latest_file=$(ls -1 "${FILE_STEM}".*.${EXT} 2>/dev/null | sort | tail -n 1)
    echo "${latest_file}"
}
build_gravity_bench

start_bench
echo "Benchmark started successfully!"

latest_file=$(find_log_file)
echo "Benchmark log file: ${latest_file}"
tail -f "${latest_file}"