#!/bin/sh

DIR=$(cd "$(dirname "$0")" && pwd)
# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color
CLI=reth
# Function to print colored output
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to read BOOTNODES value from file
read_bootnodes() {
    local bootnodes_file="/scripts/bootnodes.env"
    if [ -f "$bootnodes_file" ]; then
        source "$bootnodes_file"
        echo "$BOOTNODES"
    else
        print_warning "BOOTNODES file not found: $bootnodes_file"
        echo ""
    fi
}

# Function to test genesis extraction (placeholder)
test_genesis_extraction() {
    print_info "Testing genesis block hash extraction..."
    print_info "This is a placeholder function for testing purposes."
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  -h, --help     Show this help message"
    echo "  -c, --consensus Generate committee config for each validator node"
    echo "  -t, --test     Test genesis block hash extraction with sample output"
    echo "  -b, --bootnodes Show current BOOTNODES value"
    echo
    echo "This script sets up FastEVM nodes by generating JWT secrets and configs"
    echo "for both execution and consensus clients."
    echo
    echo "Prerequisites:"
    echo "  1. Run this script from the project root or scripts directory"
    echo "  2. Ensure consensus-client directory exists for full setup"
    echo
    echo "Examples:"
    echo "  ./scripts/init.sh               # Full setup (default)"
    echo "  ./scripts/init.sh --consensus   # Generate committee config for each validator node"
    echo "  ./scripts/init.sh --test        # Test genesis block hash extraction"
    echo "  ./scripts/init.sh --bootnodes   # Show current BOOTNODES value"
}

# Function to generate enode URL from peer ID
generate_enode_url() {
    local peer_id=$1
    local node_index=$2
    
    if [ -n "$peer_id" ]; then
        # Generate enode URL format: enode://{peerid}@execution{index}:30303
        local enode_url="enode://${peer_id}@execution${node_index}:30303"
        echo "$enode_url"
    else
        echo ""
    fi
}

# Function to generate P2P secret key for a specific node
generate_p2p_secret_key() {
    node_index=$1
    data_dir=$2
    
    # Create p2p directory if it doesn't exist
    p2p_dir="${data_dir}/p2p"
    mkdir -p "$p2p_dir"
    
    # Generate a deterministic P2P secret key based on node index
    # Using a seed that will produce consistent keys for each node
    seed="fastevm-node-${node_index}-p2p-secret-2025"
    p2p_secret_file="${p2p_dir}/secret.key"
    
    if [ ! -f "$p2p_secret_file" ]; then
        # Generate a deterministic secret using the seed
        # This ensures each node gets a unique but predictable P2P secret
        echo "$seed" | openssl dgst -sha256 -binary | openssl dgst -sha256 -hex | cut -d' ' -f2 | tr -d '\n' > "$p2p_secret_file"
        
        if [ $? -eq 0 ]; then
            print_info "Generated P2P secret key for node $node_index: $p2p_secret_file"
            
            # Also create a hex version for easier use
            hex_file="${p2p_dir}/secret.hex"
            cli show-peer-id --file "$p2p_secret_file" --output "$hex_file"
            
            # Remove 0x prefix from the hex file content
            if [ -f "$hex_file" ]; then
                sed -i 's/^0x//' "$hex_file"
            fi
            
            print_info "Created hex version: $hex_file"
            
            return 0
        else
            print_error "Failed to generate P2P secret key for node $node_index"
            return 1
        fi
    else
        print_info "P2P secret key already exists for node $node_index: $p2p_secret_file"
        return 0
    fi
}

generate_secret_key() {
  key_file=$1
  if [ ! -f "$key_file" ]; then
    if openssl rand -hex 32 | tr -d "\n" > "$key_file"; then
      print_info "Generated JWT secret: $key_file"
      return 0
    else
      print_error "Failed to generate JWT secret: $key_file"
      return 1
    fi
  else
    print_info "JWT secret already exists: $key_file"
    return 0
  fi
}

# Function to validate JWT secret format
validate_jwt_secret() {
    jwt_secret="$1"
    
    # Check if it's a valid hex string with 0x prefix
    if echo "$jwt_secret" | grep -qE '^0x[0-9a-fA-F]{64}$'; then
        return 0
    else
        return 1
    fi
}

# Function to read JWT secret and format it properly
read_jwt_secret() {
    jwt_file="$1"
    
    if [ ! -f "$jwt_file" ]; then
        print_error "JWT file not found: $jwt_file"
        return 1
    fi
    
    # Read the JWT secret and ensure it has 0x prefix
    jwt_secret=$(cat "$jwt_file" | tr -d '\n\r')
    
    # Remove any existing 0x prefix and add it back
    jwt_secret=$(echo "$jwt_secret" | sed 's/^0x//')
    jwt_secret="0x$jwt_secret"
    
    # Validate the JWT secret format
    if ! validate_jwt_secret "$jwt_secret"; then
        print_error "Invalid JWT secret format: $jwt_secret"
        return 1
    fi
    
    echo "$jwt_secret"
}

# Function to update consensus client config file
update_consensus_config() {
    node_index=$1
    jwt_secret="$2"
    genesis_block_hash="$3"
    config_file="/consensus${node_index}/node.yml"
    template_file="$DIR/../consensus-client/examples/node.template.yml"
    
    # Check if consensus client directory exists
    if [ ! -d "$DIR/../consensus-client" ]; then
        print_warning "Consensus client directory not found. Skipping config update."
        return 1
    fi
    
    # Copy genesis.json to consensus volume for custom chain support
    if [ -f "/shared/genesis.json" ]; then
        if cp "/shared/genesis.json" "/consensus${node_index}/genesis.json"; then
            print_info "Copied genesis.json to consensus volume: /consensus${node_index}/genesis.json"
        else
            print_warning "Failed to copy genesis.json to consensus volume"
        fi
    fi
    
    # If config file from template and replace placeholders
     if [ -f "$template_file" ]; then
        print_info "Creating consensus config from template: $config_file"
        cp "$template_file" "$config_file"
        
        # Replace placeholders in the new config file
        sed -i "s/{NODE_INDEX}/$node_index/g" "$config_file"
        sed -i "s/{AUTHORITY_INDEX}/$(($node_index - 1))/g" "$config_file"
        sed -i "s/{JWT_SECRET}/$jwt_secret/g" "$config_file"
        sed -i "s/{GENESIS_BLOCK_HASH}/$genesis_block_hash/g" "$config_file"
        
        print_info "Generated new consensus config: $config_file"
        cat "$config_file"
    else
        print_warning "Template file not found: $template_file"
        return 1
    fi
}

prepare_data_dir() {
  node_index=$1
  data_dir="/execution${node_index}"
  success=true
  
  print_info "Preparing data directory for node $node_index: $data_dir"
  
  # Create data directory
  if ! mkdir -p ${data_dir}; then
    print_error "Failed to create data directory: $data_dir"
    return 1
  fi
  
  # Generate JWT secret
  if ! generate_secret_key ${data_dir}/jwt.hex; then
    print_error "Failed to generate JWT secret for node $node_index"
    success=false
  fi
  
  # Generate P2P secret key
  if ! generate_p2p_secret_key "$node_index" "$data_dir"; then
    print_error "Failed to generate P2P secret key for node $node_index"
    success=false
  fi
  
  # Copy genesis.json from template
  if cp /shared/genesis.json ${data_dir}/genesis.json; then
    print_info "Copied genesis.json to ${data_dir}/genesis.json"      
    # Extract the genesis block hash
    if last_line=$($CLI init --datadir $data_dir --chain $data_dir/genesis.json 2>&1 | tail -n 1); then
      print_info "Genesis output: $last_line"
      genesis_block_hash=$(echo "$last_line" | grep -o '0x[0-9a-fA-F]\{64\}')
      if [ -z "$genesis_block_hash" ]; then
        print_warning "Could not find genesis block hash in output"
      else
        print_info "Extracted genesis block hash: ${genesis_block_hash}"
      fi
    else
      print_warning "Failed to extract genesis block hash"
    fi
  else
    print_error "Failed to copy genesis.json to ${data_dir}/genesis.json"
    success=false
  fi

  # Read the generated JWT secret
  jwt_secret=""
  if jwt_secret=$(read_jwt_secret "${data_dir}/jwt.hex"); then
    print_info "Node $node_index JWT secret: ${jwt_secret%??????????}..."
    
    # Update consensus client config
    if update_consensus_config "$node_index" "$jwt_secret" "$genesis_block_hash"; then
      print_info "Successfully updated consensus config for node $node_index"
    else
      print_warning "Failed to update consensus config for node $node_index"
      # Don't mark as failure since execution client setup succeeded
    fi
  else
    print_error "Failed to read JWT secret for node $node_index"
    success=false
  fi
  
  echo
  
  if [ "$success" = true ]; then
    return 0
  else
    return 1
  fi
}
consensus() {
  print_info "Starting FastEVM consensus node setup process..."
  print_info "This will generate committee config for each validator node"
  
  # Define the number of nodes
  num_nodes=${1:-4}
  for i in $(seq 1 $num_nodes); do
    echo "Generating committee config for node $i"
    # Copy parameters.yml to the consensus client directory
    cp /parameters.yml /consensus${i}/parameters.yml
    fastevm-consensus generate-committee \
      --output "/consensus${i}/committees.yml" \
      --authorities "4" \
      --epoch "0" \
      --stake "1000" \
      --ip-addresses "172.20.0.10,172.20.0.11,172.20.0.12,172.20.0.13" \
      --network-ports "26657,26657,26657,26657"
  done
}

# Main execution
main() {
    print_info "Starting FastEVM node setup process..."
    print_info "This will generate JWT secrets, genesis files and configs for both execution and consensus clients"
    apt update && apt install -y openssl
    # Define the number of nodes
    num_nodes=${1:-4}
    
    # Track success/failure for each node
    success_count=0
    failure_count=0
    
    # String to store enode URLs
    enode_urls=""
    
    # Loop through all nodes
    for i in $(seq 1 $num_nodes); do
        print_info "Processing node $i..."
        if prepare_data_dir $i; then
            success_count=$((success_count + 1))
            
            # Generate peer ID and enode URL for this node
            local data_dir="/execution${i}"
            local p2p_secret_hex="${data_dir}/p2p/secret.hex"
            local node_name="execution-node${i}"
            
            if [ -f "$p2p_secret_hex" ]; then
                local peer_id=$(cat "$p2p_secret_hex")
                if [ -n "$peer_id" ]; then
                    local enode_url=$(generate_enode_url "$peer_id" "$i")
                    if [ -n "$enode_url" ]; then
                        enode_urls="${enode_urls}${enode_url},"
                        print_info "   ✅ Added enode: $enode_url"
                    fi
                fi
            else
                print_warning "   ⚠️  P2P secret file not found for node $i"
            fi
        else
            failure_count=$((failure_count + 1))
        fi
    done
    
    print_info "✅ FastEVM node setup completed!"
    print_info "   - Execution clients: JWT secrets and genesis files generated"
    print_info "   - Consensus clients: Config files updated with JWT secrets"
    print_info ""
    
    # Generate and display BOOTNODES value
    if [ -n "$enode_urls" ]; then
        # Remove trailing comma
        enode_urls="${enode_urls%,}"
        
        print_info "🌐 P2P Network Configuration:"
        print_info "   Generated enode URLs:"
        
        # # Split the string and display each enode
        # IFS=',' read -ra enode_array <<< "$enode_urls"
        # for enode in "${enode_array[@]}"; do
        #     print_info "   - $enode"
        # done
        
        # Set BOOTNODES value
        BOOTNODES="$enode_urls"
        print_info ""
        print_info "🔗 BOOTNODES value (comma-separated):"
        print_info "   $BOOTNODES"
        print_info ""
        print_info "💡 You can use this BOOTNODES value in your docker-compose.yml or environment variables"
        
        # Export BOOTNODES for use in other scripts
        export BOOTNODES
        echo "export BOOTNODES=\"$BOOTNODES\"" > $DIR/bootnodes.env
        print_info "   📁 BOOTNODES exported to $DIR/bootnodes.env"
    else
        print_warning "⚠️  No enode URLs generated - P2P networking may not work properly"
    fi
    
    print_info ""
    print_info "Node summary:"
    for i in $(seq 1 $num_nodes); do
        data_dir="/execution${i}"
        jwt_file="${data_dir}/jwt.hex"
        genesis_file="${data_dir}/genesis.json"
        
        if [ -f "$jwt_file" ] && [ -f "$genesis_file" ]; then
            print_info "   - Node $i: $data_dir (jwt.hex + genesis.json) ✅"
        else
            print_warning "   - Node $i: $data_dir (incomplete setup) ⚠️"
        fi
    done
    
    print_info ""
    print_info "Setup results: $success_count successful, $failure_count failed"
    # sleep infinity
    # Exit with error if any node failed
    if [ $failure_count -gt 0 ]; then
        print_warning "Some nodes failed to setup properly. Please check the logs above."
        exit 1
    fi
}

# Parse command line arguments
case "${1:-}" in
    -h|--help)
        show_usage
        exit 0
        ;;
    -c|--consensus)
        consensus
        exit 0
        ;;
    -t|--test)
        test_genesis_extraction
        exit 0
        ;;
    -b|--bootnodes)
        print_info "Current BOOTNODES value:"
        bootnodes_value=$(read_bootnodes)
        if [ -n "$bootnodes_value" ]; then
            echo "$bootnodes_value"
        else
            print_warning "No BOOTNODES value found. Run the full setup first."
        fi
        exit 0
        ;;
    "")
        main
        ;;
    *)
        print_error "Unknown option: $1"
        show_usage
        exit 1
        ;;
esac
