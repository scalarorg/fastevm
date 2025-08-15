#!/bin/sh
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

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

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  -h, --help     Show this help message"
    echo "  -v, --verify   Verify JWT secrets without updating files"
    echo "  -u, --update   Update JWT secrets in existing consensus configs only"
    echo
    echo "This script sets up FastEVM nodes by generating JWT secrets and configs"
    echo "for both execution and consensus clients."
    echo
    echo "Prerequisites:"
    echo "  1. Run this script from the project root or scripts directory"
    echo "  2. Ensure consensus-client directory exists for full setup"
    echo
    echo "Examples:"
    echo "  ./scripts/init.sh              # Full setup (default)"
    echo "  ./scripts/init.sh --verify     # Verify existing JWT secrets"
    echo "  ./scripts/init.sh --update     # Update JWT secrets in existing configs"
}

generate_secret_key() {
  key_file=$1
  if [ ! -f "$key_file" ]; then
    #openssl rand -hex 32 | tr -d "\n" > "$key_file"
    if dd if=/dev/urandom bs=32 count=1 2>/dev/null | xxd -p -c 64 > "$key_file"; then
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

# Function to clean up old consensus configs if needed
cleanup_old_configs() {
    num_nodes=$1
    
    if [ ! -d "../consensus-client" ]; then
        return 0
    fi
    
    print_info "Checking for old consensus configs..."
    
    # Find all node config files
    config_files=$(find ../consensus-client/examples/ -name "node*.yml" -type f | sort)
    
    for config_file in $config_files; do
        # Extract node index from filename
        filename=$(basename "$config_file")
        node_index=$(echo "$filename" | sed 's/node\([0-9]*\)\.yml/\1/')
        
        # Check if this node index is within our current range
        if [ "$node_index" -ge "$num_nodes" ]; then
            print_warning "Removing old config for node $node_index: $config_file"
            rm -f "$config_file"
        fi
    done
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

# Function to update JWT secret in a config file (standalone function)
update_jwt_secret() {
    config_file="$1"
    new_jwt_secret="$2"
    
    if [ ! -f "$config_file" ]; then
        print_error "Config file not found: $config_file"
        return 1
    fi
    
    # Create backup of the original file
    backup_file="${config_file}.backup.$(date +%Y%m%d_%H%M%S)"
    cp "$config_file" "$backup_file"
    print_info "Created backup: $backup_file"
    
    # Update the JWT secret in the config file
    # Use sed to replace the jwt_secret line while preserving the comment
    sed -i.bak "s/^jwt_secret:.*$/jwt_secret: \"$new_jwt_secret\"  # JWT secret for authentication/" "$config_file"
    
    # Remove the temporary .bak file created by sed
    rm -f "${config_file}.bak"
    
    print_info "Updated JWT secret in: $config_file"
}

# Function to update consensus client config file
update_consensus_config() {
    node_index=$1
    jwt_secret="$2"
    config_file="../consensus-client/examples/node${node_index}.yml"
    template_file="../consensus-client/examples/node.template.yml"
    
    # Check if consensus client directory exists
    if [ ! -d "../consensus-client" ]; then
        print_warning "Consensus client directory not found. Skipping config update."
        return 1
    fi
    
    # If config file doesn't exist, create it from template
    if [ ! -f "$config_file" ]; then
        if [ -f "$template_file" ]; then
            print_info "Creating consensus config from template: $config_file"
            cp "$template_file" "$config_file"
            
            # Replace placeholders in the new config file
            sed -i "s/{NODE_INDEX}/$node_index/g" "$config_file"
            sed -i "s/{AUTHORITY_INDEX}/$(($node_index - 1))/g" "$config_file"
            sed -i "s/{JWT_SECRET}/$jwt_secret/g" "$config_file"
            
            print_info "Generated new consensus config: $config_file"
        else
            print_warning "Template file not found: $template_file"
            return 1
        fi
    else
        # Create backup of the existing file
        backup_file="${config_file}.backup.$(date +%Y%m%d_%H%M%S)"
        cp "$config_file" "$backup_file"
        print_info "Created backup: $backup_file"
        
        # Update the JWT secret in the existing config file
        # Use sed to replace the jwt_secret line while preserving the comment
        sed -i.bak "s/^jwt_secret:.*$/jwt_secret: \"$jwt_secret\"  # JWT secret for authentication/" "$config_file"
        
        # Remove the temporary .bak file created by sed
        rm -f "${config_file}.bak"
        
        print_info "Updated JWT secret in existing consensus config: $config_file"
    fi
}

# Function to verify JWT secrets without updating
verify_jwt_secrets() {
    print_info "Verifying JWT secrets (read-only mode)..."
    
    # POSIX-compatible mapping using parallel arrays
    data_dirs="/data1 /data2 /data3 /data4"
    config_files="consensus-client/examples/node0.yml consensus-client/examples/node1.yml consensus-client/examples/node2.yml consensus-client/examples/node3.yml"
    
    # Convert to arrays
    set -- $data_dirs
    data_dirs_array="$@"
    set -- $config_files
    config_files_array="$@"
    
    # Process each mapping
    i=1
    for data_dir in $data_dirs_array; do
        config_file=$(echo "$config_files_array" | cut -d' ' -f$i)
        jwt_file="${data_dir}/jwt.hex"
        
        echo "Node: $data_dir -> $config_file"
        
        if [ -f "$jwt_file" ]; then
            jwt_secret=$(read_jwt_secret "$jwt_file" 2>/dev/null || echo "ERROR")
            echo "  JWT file: $jwt_file"
            echo "  JWT secret: ${jwt_secret%??????????}..."
        else
            echo "  JWT file: NOT FOUND"
        fi
        
        if [ -f "$config_file" ]; then
            current_jwt=$(grep "^jwt_secret:" "$config_file" | sed 's/.*jwt_secret: *"\([^"]*\)".*/\1/')
            echo "  Config file: $config_file"
            echo "  Current JWT: ${current_jwt%??????????}..."
        else
            echo "  Config file: NOT FOUND"
        fi
        
        echo
        i=$((i + 1))
    done
}

# Function to update JWT secrets in existing consensus configs only
update_existing_jwt_secrets() {
    print_info "Starting JWT secret update process for existing configs..."
    
    # POSIX-compatible mapping using parallel arrays
    data_dirs="/data1 /data2 /data3 /data4"
    config_files="consensus-client/examples/node0.yml consensus-client/examples/node1.yml consensus-client/examples/node2.yml consensus-client/examples/node3.yml"
    
    # Check if we're running in the right context
    if [ ! -d "consensus-client" ] || [ ! -d "execution-client" ]; then
        print_error "This script must be run from the project root directory"
        print_error "Current directory: $(pwd)"
        exit 1
    fi
    
    # Process each node mapping
    i=1
    for data_dir in $data_dirs; do
        config_file=$(echo "$config_files" | cut -d' ' -f$i)
        jwt_file="${data_dir}/jwt.hex"
        
        print_info "Processing node: $data_dir -> $config_file"
        
        # Check if the JWT file exists
        if [ ! -f "$jwt_file" ]; then
            print_warning "JWT file not found: $jwt_file"
            print_warning "Skipping this node. Make sure to run execution-client/shared/init.sh first."
            continue
        fi
        
        # Check if the config file exists
        if [ ! -f "$config_file" ]; then
            print_warning "Config file not found: $config_file"
            print_warning "Skipping this node."
            continue
        fi
        
        # Read the JWT secret
        if jwt_secret=$(read_jwt_secret "$jwt_file"); then
            print_info "Read JWT secret: ${jwt_secret%??????????}..."
            
            # Update the config file
            if update_jwt_secret "$config_file" "$jwt_secret"; then
                print_info "Successfully updated $config_file"
            else
                print_error "Failed to update $config_file"
            fi
        else
            print_error "Failed to read JWT secret from $jwt_file"
        fi
        
        echo
        i=$((i + 1))
    done
    
    print_info "JWT secret update process completed!"
    print_info "Please verify the changes in the config files before running the consensus clients."
}

prepare_data_dir() {
  node_index=$1
  data_dir="/data${node_index}"
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
  
  # Copy genesis.json from template if needed
  if [ ! -f ${data_dir}/genesis.json ]; then
    if cp /shared/genesis.template.json ${data_dir}/genesis.json; then
      print_info "Copied genesis.json to ${data_dir}/genesis.json"
    else
      print_error "Failed to copy genesis.json to ${data_dir}/genesis.json"
      success=false
    fi
  else
    print_info "Genesis file already exists: ${data_dir}/genesis.json"
  fi
  
  # Read the generated JWT secret
  jwt_secret
  if jwt_secret=$(read_jwt_secret "${data_dir}/jwt.hex"); then
    print_info "Node $node_index JWT secret: ${jwt_secret%??????????}..."
    
    # Update consensus client config
    if update_consensus_config "$node_index" "$jwt_secret"; then
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

# Main execution
main() {
    print_info "Starting FastEVM node setup process..."
    print_info "This will generate JWT secrets and configs for both execution and consensus clients"
    
    # Check if we're in the right context
    if [ ! -d "../execution-client" ]; then
        print_error "Execution client directory not found. Please run this script from the project root or scripts directory."
        exit 1
    fi
    
    if [ ! -d "../consensus-client" ]; then
        print_warning "Consensus client directory not found. Running in execution-only mode."
    fi
    
    # Check if we have write permissions
    if [ ! -w ".." ]; then
        print_error "No write permission in parent directory."
        exit 1
    fi
    
    # Define the number of nodes
    num_nodes=4
    
    # Cleanup old consensus configs
    cleanup_old_configs "$num_nodes"
    
    # Track success/failure for each node
    success_count=0
    failure_count=0
    
    # Loop through all nodes
    for i in $(seq 1 $num_nodes); do
        print_info "Processing node $i..."
        if prepare_data_dir $i; then
            success_count=$((success_count + 1))
        else
            failure_count=$((failure_count + 1))
        fi
    done
    
    print_info "✅ FastEVM node setup completed!"
    print_info "   - Execution clients: JWT secrets and genesis files generated"
    print_info "   - Consensus clients: Config files updated with JWT secrets"
    print_info ""
    print_info "Node summary:"
    for i in $(seq 1 $num_nodes); do
        data_dir="/data${i}"
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
    
    if [ -d "../consensus-client" ]; then
        print_info ""
        print_info "Consensus client configs have been updated with the generated JWT secrets."
        print_info "Please verify the changes before running the consensus clients."
    fi
    
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
    -v|--verify)
        verify_jwt_secrets
        exit 0
        ;;
    -u|--update)
        update_existing_jwt_secrets
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
