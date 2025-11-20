#!/bin/bash

# FastEVM Client Binary Preparation Script
# This script ensures client binaries are available on both local machine and client node

set -e

# Color codes for logging
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

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CLIENT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINARIES_DIR="$CLIENT_ROOT/binaries"
SSH_KEY_PATH="$PROJECT_ROOT/client-deploy-key"
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

# Required binaries for client node
REQUIRED_BINARIES=("fastevm-test")

log_info "FastEVM Client Binary Preparation Script"
log_info "======================================"

# Create binaries directory if it doesn't exist
mkdir -p "$BINARIES_DIR"

# Check if SSH key exists
if [ ! -f "$SSH_KEY_PATH" ]; then
    log_error "SSH key not found at $SSH_KEY_PATH"
    exit 1
fi

chmod 600 "$SSH_KEY_PATH"

# Get client node IP
get_client_ip() {
    # Try to get from terraform output
    local client_ip=$(cd "$PROJECT_ROOT" && terraform output -json client_node_info 2>/dev/null | jq -r '.external_ip' 2>/dev/null || echo "")
    if [ -n "$client_ip" ] && [ "$client_ip" != "null" ]; then
        echo "$client_ip"
        return 0
    fi
    
    log_error "Could not determine client node IP"
    return 1
}

# Check if all required binaries exist locally
check_local_binaries() {
    for binary in "${REQUIRED_BINARIES[@]}"; do
        if [ ! -f "$BINARIES_DIR/$binary" ]; then
            return 1
        fi
    done
    return 0
}

# Copy binaries from local to client node
restore_binaries() {
    local client_ip="$1"
    log_info "Copying binaries from local storage to client node ($client_ip)..."
    
    # Ensure directory exists on client node
    ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$client_ip "sudo mkdir -p /opt/fastevm-binaries && sudo chown -R ubuntu:ubuntu /opt/fastevm-binaries"
    
    # Copy each binary
    for binary in "${REQUIRED_BINARIES[@]}"; do
        log_info "Copying $binary to client node..."
        scp $SSH_OPTS -i "$SSH_KEY_PATH" "$BINARIES_DIR/$binary" ubuntu@$client_ip:/opt/fastevm-binaries/
        ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$client_ip "chmod +x /opt/fastevm-binaries/$binary"
    done
    
    log_success "All binaries copied to client node"
}

# Copy binaries from client node to local
backup_binaries() {
    local client_ip="$1"
    log_info "Copying binaries from client node ($client_ip) to local storage..."
    
    # Copy each binary
    for binary in "${REQUIRED_BINARIES[@]}"; do
        log_info "Copying $binary from client node..."
        scp $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$client_ip:/opt/fastevm-binaries/$binary "$BINARIES_DIR/"
        chmod +x "$BINARIES_DIR/$binary"
    done
    
    log_success "All binaries copied to local storage"
}

# Build binaries on client node
build_binaries() {
    local client_ip="$1"
    log_info "Building binaries on client node ($client_ip)..."
    
    # Get GitHub configuration from environment variables
    local github_repo="${GITHUB_REPO:-https://github.com/scalarorg/fastevm.git}"
    local github_branch="${GITHUB_BRANCH:-main}"
    
    log_info "Using GitHub repo: $github_repo"
    log_info "Using GitHub branch: $github_branch"
    
    # Run the build process on the client node
    ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$client_ip "
        set -e
        
        # Logging function
        log() {
            echo \"[INFO] \$1\"
        }
        
        log \"Starting FastEVM client binary build process...\"
        
        # Refresh environment to ensure compilers are available
        log \"Refreshing environment...\"
        export PATH=\"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin\"
        hash -r
        
        # Verify compilers are available before proceeding
        log \"Verifying compiler availability...\"
        if ! command -v cc &> /dev/null; then
            log \"ERROR: C compiler (cc) not found\"
            exit 1
        fi
        
        if ! command -v gcc &> /dev/null; then
            log \"ERROR: GCC compiler not found\"
            exit 1
        fi
        
        log \"Compilers verified, proceeding with build...\"
        
        # Install Rust if not already installed
        if ! command -v cargo >/dev/null 2>&1; then
            log \"Installing Rust...\"
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            export PATH=\"\$HOME/.cargo/bin:\$PATH\"
            source \$HOME/.cargo/env
            rustup default stable
        fi
        
        # Clone and build FastEVM
        log \"Cloning FastEVM repository...\"
        cd /home/ubuntu
        if [ -d \"fastevm\" ]; then
            cd fastevm
            git fetch origin
            git checkout \"$github_branch\"
            git pull origin \"$github_branch\"
        else
            git clone -b \"$github_branch\" \"$github_repo\" fastevm
            cd fastevm
        fi
        
        # Build test binary with explicit linker configuration
        log \"Building FastEVM test binary...\"
        cd testing/integration
        
        # Set explicit linker environment variables to ensure cc is found
        export CC=cc
        export CXX=g++
        export RUSTFLAGS=\"-C linker=cc\"
        
        # Build with verbose output to help debug any remaining issues
        cargo build --release --bin fastevm-test --verbose
        
        if [ ! -f \"../../target/release/fastevm-test\" ]; then
            log \"ERROR: Build failed - binary not found\"
            exit 1
        fi
        
        # Copy to binaries directory
        log \"Installing binary to binaries directory...\"
        sudo mkdir -p /opt/fastevm-binaries
        sudo cp \"../../target/release/fastevm-test\" /opt/fastevm-binaries/
        sudo chown ubuntu:ubuntu /opt/fastevm-binaries/fastevm-test
        chmod +x /opt/fastevm-binaries/fastevm-test
        
        log \"FastEVM client binary build completed successfully!\"
    "
    
    log_success "Binaries built successfully on client node"
}

# Install binaries on client node
install_binaries() {
    local client_ip="$1"
    log_info "Installing binaries on client node ($client_ip)..."
    
    ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$client_ip "
        # Copy to system location
        sudo cp /opt/fastevm-binaries/fastevm-test /usr/local/bin/
        
        # Set executable permissions
        sudo chmod +x /usr/local/bin/fastevm-test
        
        # Verify installation
        if command -v fastevm-test >/dev/null 2>&1; then
            echo '[SUCCESS] Binary installed and accessible'
        else
            echo '[ERROR] Binary installation failed'
            exit 1
        fi
    "
    
    log_success "Binaries installed on client node"
}

# Verify binary installation on client node
verify_installation() {
    local client_ip="$1"
    
    log_info "Verifying binary installation on client node..."
    if ssh $SSH_OPTS -i "$SSH_KEY_PATH" ubuntu@$client_ip "ls -la /usr/local/bin/fastevm-test" >/dev/null 2>&1; then
        log_success "Binary verified on client node"
    else
        log_error "Binary verification failed on client node"
        return 1
    fi
}

# Main workflow
main() {
    # Get client node IP
    local client_ip
    if ! client_ip=$(get_client_ip); then
        log_error "Could not determine client node IP"
        exit 1
    fi
    
    log_info "Client node IP: $client_ip"
    
    # Check if we have local binaries
    if check_local_binaries; then
        log_success "Local binaries found, copying to client node..."
        restore_binaries "$client_ip"
    else
        log_warning "Local binaries missing, building new ones on client node..."
        build_binaries "$client_ip"
        backup_binaries "$client_ip"
    fi
    
    # Install binaries on client node
    install_binaries "$client_ip"
    
    # Verify installation
    verify_installation "$client_ip"
    
    log_success "Client binary preparation and installation completed successfully!"
    log_info "Client node now has the required binaries"
}

# Handle command line arguments
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    case "${1:-}" in
        "backup")
            if [ -z "$2" ]; then
                log_error "Client IP required for backup command"
                exit 1
            fi
            backup_binaries "$2"
            ;;
        "restore")
            if [ -z "$2" ]; then
                log_error "Client IP required for restore command"
                exit 1
            fi
            restore_binaries "$2"
            ;;
        *)
            main "$@"
            ;;
    esac
fi
