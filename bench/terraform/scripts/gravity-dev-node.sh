#!/bin/bash
# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info(){ echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn(){ echo -e "${YELLOW}[WARN]${NC} $1"; }

NODE="node1"
INSTALL_DIR="/tmp"

export MOCK_CONSENSUS=true
export RETH_TXPOOL_BATCH_INSERT=1
export BATCH_INSERT_TIME=50
export USE_PARALLEL_STATE_ROOT=1
export USE_STORAGE_CACHE=1

log_info "Killing old gravity_node..."
pkill -9 gravity_node 2>/dev/null || log_warn "No running gravity_node found"

log_info "Deploying $NODE..."
bash ./deploy_utils/deploy.sh --mode single --install_dir "$INSTALL_DIR" --node "$NODE" -v release

log_info "Starting $NODE..."
bash "$INSTALL_DIR/$NODE/script/start.sh" --bin_name gravity_node

log_info "Node started"