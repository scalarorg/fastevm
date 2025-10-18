#!/bin/bash
# Template Replacement Script for FastEVM Configuration
# This script replaces placeholders in template files with actual values

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
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

# Function to replace placeholders in a file
replace_placeholders() {
    local template_file="$1"
    local output_file="$2"
    local node_index="$3"
    
    log_info "Replacing placeholders in $template_file for node $node_index..."
    
    # Read the template file
    local content=$(cat "$template_file")
    
    # Replace placeholders with actual values
    content=$(echo "$content" | sed "s/{{NODE_INDEX}}/$node_index/g")
    content=$(echo "$content" | sed "s/{{HTTP_PORT}}/$HTTP_PORT/g")
    content=$(echo "$content" | sed "s/{{WS_PORT}}/$WS_PORT/g")
    content=$(echo "$content" | sed "s/{{ENGINE_PORT}}/$ENGINE_PORT/g")
    content=$(echo "$content" | sed "s/{{P2P_PORT}}/$P2P_PORT/g")
    content=$(echo "$content" | sed "s/{{JWT_SECRET}}/$JWT_SECRET/g")
    content=$(echo "$content" | sed "s/{{BOOTNODES}}/$BOOTNODES/g")
    content=$(echo "$content" | sed "s/{{PEER_ADDRESSES}}/$PEER_ADDRESSES/g")
    content=$(echo "$content" | sed "s/{{BASE_IP}}/$BASE_IP/g")
    content=$(echo "$content" | sed "s/{{START_IP}}/$START_IP/g")
    content=$(echo "$content" | sed "s/{{END_IP}}/$END_IP/g")
    content=$(echo "$content" | sed "s/{{NODE_COUNT}}/$NODE_COUNT/g")
    content=$(echo "$content" | sed "s/{{AUTHORITIES_LIST}}/$AUTHORITIES_LIST/g")
    
    # Write the output file
    echo "$content" > "$output_file"
    
    log_success "Generated $output_file"
}

# Help function
show_help() {
    cat << EOF
FastEVM Template Replacement Script

Usage: $0 [OPTIONS]

This script replaces placeholders in template files with actual values.

OPTIONS:
    -n, --node-index INDEX     Node index for single-node templates (default: 0)
    -t, --template-dir DIR      Template directory (default: /tmp/fastevm-templates)
    -o, --output-dir DIR       Output directory (default: /data)
    -c, --config-dir DIR       Config directory for docker-compose (default: .)
    --docker-compose           Generate docker-compose.yml from template
    -h, --help                 Show this help message

EXAMPLES:
    $0                                    # Replace templates for node 0
    $0 -n 1                              # Replace templates for node 1
    $0 --docker-compose -c ./config      # Generate docker-compose.yml
    $0 -t ./templates -o ./output        # Custom directories

EOF
}

# Main function
main() {
    local node_index=0
    local template_dir="/tmp/fastevm-templates"
    local output_dir="/data"
    local config_dir="."
    local docker_compose=false
    
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -n|--node-index)
                node_index="$2"
                shift 2
                ;;
            -t|--template-dir)
                template_dir="$2"
                shift 2
                ;;
            -o|--output-dir)
                output_dir="$2"
                shift 2
                ;;
            -c|--config-dir)
                config_dir="$2"
                shift 2
                ;;
            --docker-compose)
                docker_compose=true
                shift
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
    
    log_info "Template directory: $template_dir"
    log_info "Output directory: $output_dir"
    
    if [ "$docker_compose" = "true" ]; then
        # Handle docker-compose.yml template
        log_info "Generating docker-compose.yml from template..."
        
        # Load global configuration
        if [ -f "$config_dir/node0.env" ]; then
            source "$config_dir/node0.env"
        else
            log_warning "Node 0 environment file not found, using defaults"
            NODE_COUNT=${NODE_COUNT:-4}
            BASE_IP=${BASE_IP:-"10.0.0"}
            START_IP=${START_IP:-10}
            PROJECT_NAME=${PROJECT_NAME:-"fastevm"}
        fi

    else
        # Handle single-node templates
        log_info "Replacing templates for node $node_index..."
        
        # Replace execution.toml template
        if [ -f "$template_dir/execution.toml.template" ]; then
            replace_placeholders "$template_dir/execution.toml.template" "$output_dir/execution.toml" "$node_index"
        else
            log_error "execution.toml.template not found in $template_dir"
            return 1
        fi
        
        # Replace node.yml template
        if [ -f "$template_dir/node.yml.template" ]; then
            replace_placeholders "$template_dir/node.yml.template" "$output_dir/node.yml" "$node_index"
        else
            log_error "node.yml.template not found in $template_dir"
            return 1
        fi
        
        # Replace committees.yml template (only for node 0)
        if [ "$node_index" = "0" ] && [ -f "$template_dir/committees.yml.template" ]; then
            replace_placeholders "$template_dir/committees.yml.template" "$output_dir/committees.yml" "$node_index"
        fi
        
        # Replace parameters.yml template (only for node 0)
        if [ "$node_index" = "0" ] && [ -f "$template_dir/parameters.yml.template" ]; then
            replace_placeholders "$template_dir/parameters.yml.template" "$output_dir/parameters.yml" "$node_index"
        fi
        
        log_success "Template replacement completed for node $node_index"
    fi
}

# Handle command line arguments
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
