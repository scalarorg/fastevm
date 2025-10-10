#!/bin/bash
# Compare genesis.json files across all nodes

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

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Configuration
PROJECT_NAME="fastevm"
ZONE="us-central1-a"
NODE_COUNT=4

# Check if deployment info exists
if [ ! -f "../deployment-info.json" ]; then
    log_error "No deployment info found. Run 'make apply' first."
    exit 1
fi

# Get node IPs
log_info "Getting node IPs from deployment info..."
NODE_IPS=($(jq -r '.instance_external_ips.value[]' ../deployment-info.json 2>/dev/null || jq -r '.instance_ips.value[]' ../deployment-info.json 2>/dev/null))

if [ -z "$NODE_IPS" ]; then
    log_error "Could not get node IPs from deployment info"
    exit 1
fi

log_info "Found ${#NODE_IPS[@]} nodes: ${NODE_IPS[*]}"

# Check if SSH key exists
SSH_KEY_PATH="fastevm-deploy-key"
if [ ! -f "$SSH_KEY_PATH" ]; then
    log_error "SSH key not found at $SSH_KEY_PATH"
    exit 1
fi

chmod 600 "$SSH_KEY_PATH"
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

# Function to get genesis.json from a node
get_genesis_from_node() {
    local node_index=$1
    local node_ip=$2
    local temp_file="/tmp/genesis_node_${node_index}.json"
    
    log_info "Fetching genesis.json from node $node_index ($node_ip)..."
    
    if ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$node_ip "cat /data/genesis.json" > "$temp_file" 2>/dev/null; then
        log_success "Retrieved genesis.json from node $node_index"
        echo "$temp_file"
    else
        log_error "Failed to retrieve genesis.json from node $node_index"
        return 1
    fi
}

# Function to calculate file hash
calculate_hash() {
    local file=$1
    if [ -f "$file" ]; then
        sha256sum "$file" | cut -d' ' -f1
    else
        echo "FILE_NOT_FOUND"
    fi
}

# Function to compare two JSON files (normalized)
compare_json_files() {
    local file1=$1
    local file2=$2
    
    if [ ! -f "$file1" ] || [ ! -f "$file2" ]; then
        return 1
    fi
    
    # Normalize JSON files (sort keys, remove whitespace differences)
    local normalized1="/tmp/normalized1.json"
    local normalized2="/tmp/normalized2.json"
    
    jq -S . "$file1" > "$normalized1" 2>/dev/null
    jq -S . "$file2" > "$normalized2" 2>/dev/null
    
    if diff -q "$normalized1" "$normalized2" >/dev/null 2>&1; then
        return 0  # Files are identical
    else
        return 1  # Files are different
    fi
}

# Main comparison logic
main() {
    log_info "=== Genesis.json Comparison Across All Nodes ==="
    
    # Array to store temp files
    declare -a temp_files=()
    declare -a node_hashes=()
    
    # Fetch genesis.json from all nodes
    for i in $(seq 0 $((${#NODE_IPS[@]} - 1))); do
        node_ip="${NODE_IPS[$i]}"
        temp_file=$(get_genesis_from_node $i $node_ip)
        if [ $? -eq 0 ]; then
            temp_files+=($temp_file)
            hash=$(calculate_hash "$temp_file")
            node_hashes+=($hash)
            log_info "Node $i hash: $hash"
        else
            log_error "Failed to get genesis.json from node $i"
            temp_files+=("")
            node_hashes+=("FAILED")
        fi
    done
    
    echo ""
    log_info "=== Hash Comparison Results ==="
    
    # Compare all hashes
    all_identical=true
    reference_hash=""
    
    for i in $(seq 0 $((${#node_hashes[@]} - 1))); do
        hash="${node_hashes[$i]}"
        if [ "$hash" = "FAILED" ]; then
            log_error "Node $i: Failed to retrieve genesis.json"
            all_identical=false
        elif [ -z "$reference_hash" ]; then
            reference_hash="$hash"
            log_success "Node $i: $hash (reference)"
        elif [ "$hash" = "$reference_hash" ]; then
            log_success "Node $i: $hash (matches reference)"
        else
            log_error "Node $i: $hash (DIFFERENT from reference)"
            all_identical=false
        fi
    done
    
    echo ""
    if [ "$all_identical" = true ]; then
        log_success "✓ All genesis.json files are identical across all nodes!"
    else
        log_error "✗ Genesis.json files are NOT identical across all nodes!"
        echo ""
        log_info "=== Detailed File Comparison ==="
        
        # Find the reference file (first successful one)
        reference_file=""
        for i in $(seq 0 $((${#temp_files[@]} - 1))); do
            if [ -n "${temp_files[$i]}" ] && [ -f "${temp_files[$i]}" ]; then
                reference_file="${temp_files[$i]}"
                log_info "Using node $i as reference for detailed comparison"
                break
            fi
        done
        
        if [ -n "$reference_file" ]; then
            for i in $(seq 0 $((${#temp_files[@]} - 1))); do
                if [ -n "${temp_files[$i]}" ] && [ -f "${temp_files[$i]}" ]; then
                    if compare_json_files "$reference_file" "${temp_files[$i]}"; then
                        log_success "Node $i: Content matches reference"
                    else
                        log_error "Node $i: Content differs from reference"
                        log_info "Differences:"
                        diff -u "$reference_file" "${temp_files[$i]}" | head -20 || true
                    fi
                fi
            done
        fi
    fi
    
    # Cleanup temp files
    log_info "Cleaning up temporary files..."
    for temp_file in "${temp_files[@]}"; do
        if [ -n "$temp_file" ] && [ -f "$temp_file" ]; then
            rm -f "$temp_file"
        fi
    done
    rm -f /tmp/normalized1.json /tmp/normalized2.json
    
    echo ""
    if [ "$all_identical" = true ]; then
        log_info "Recommendation: Genesis files are consistent. The error might be due to:"
        log_info "1. Database was initialized with a different genesis"
        log_info "2. Need to clean and reinitialize the database"
        log_info "3. Run: make clean-data && make deploy-configs"
    else
        log_error "Recommendation: Fix genesis.json inconsistencies first:"
        log_error "1. Check why genesis files differ across nodes"
        log_error "2. Ensure all nodes use the same genesis.json"
        log_error "3. Redeploy configurations: make deploy-configs"
    fi
}

# Run the comparison
main "$@"
