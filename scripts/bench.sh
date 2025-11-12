#!/bin/bash
# Unified script for managing reth nodes and running benchmarks
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

# Defaults
PROFILE="release" RPC_URL="http://localhost:8545" ENGINE_RPC_URL="http://127.0.0.1:8551"
RETH_CHAIN="mainnet" BENCH_COMMAND="new-payload-fcu" MAX_WAIT_TIME=60
PID_FILE="/tmp/reth-node.pid" LOG_FILE="/tmp/reth-node.log"
RETH_BINARY="" BENCH_BINARY="" JWT_SECRET="" FROM_BLOCK="" TO_BLOCK="" ADVANCE=""
RETH_DATADIR="" RETH_METRICS="" RETH_EXTRA_ARGS="" BENCH_OUTPUT="" WAIT_TIME=""
USE_EXISTING_NODE=false NODE_PID="" PROJECT_NAME=""

# Colors
RED='\033[0;31m' GREEN='\033[0;32m' YELLOW='\033[1;33m' BLUE='\033[0;34m' NC='\033[0m'
log() { echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"; }
log_error() { echo -e "${RED}[$(date +'%Y-%m-%d %H:%M:%S')] ERROR:${NC} $1" >&2; }
log_warn() { echo -e "${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] WARN:${NC} $1"; }
log_info() { echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')] INFO:${NC} $1"; }

# Find binary in PATH or target directory
find_binary() {
    local name="$1" profile="$2" root="$3"
    command -v "$name" >/dev/null 2>&1 && { command -v "$name"; return 0; }
    local path="${root}/target/${profile}/${name}"
    [ -f "$path" ] && [ -x "$path" ] && { echo "$path"; return 0; }
    [ "$name" = "reth-bench" ] && {
        [ -f "${root}/target/${profile}/bin/${name}" ] && [ -x "${root}/target/${profile}/bin/${name}" ] && {
            echo "${root}/target/${profile}/bin/${name}"; return 0; }
    }
    return 1
}

# Resolve binary paths
resolve_binaries() {
    local profile="$1" root="$2"
    # If PROJECT_NAME is set, check ~/{projectName}/target first
    if [ -n "$PROJECT_NAME" ]; then
        local project_root="${HOME}/${PROJECT_NAME}"
        local project_target="${project_root}/target/${profile}"
        if [ -z "$RETH_BINARY" ] || [ "$RETH_BINARY" = "reth" ]; then
            if [ -f "${project_target}/reth" ] && [ -x "${project_target}/reth" ]; then
                RETH_BINARY="${project_target}/reth"
                log_info "Using reth binary from project: $RETH_BINARY"
            fi
        fi
        if [ -z "$BENCH_BINARY" ] || [ "$BENCH_BINARY" = "reth-bench" ]; then
            if [ -f "${project_target}/bin/reth-bench" ] && [ -x "${project_target}/bin/reth-bench" ]; then
                BENCH_BINARY="${project_target}/bin/reth-bench"
                log_info "Using reth-bench binary from project: $BENCH_BINARY"
            elif [ -f "${project_target}/reth-bench" ] && [ -x "${project_target}/reth-bench" ]; then
                BENCH_BINARY="${project_target}/reth-bench"
                log_info "Using reth-bench binary from project: $BENCH_BINARY"
            fi
        fi
    fi
    # Fall back to default resolution if not found in project directory
    if [ -z "$RETH_BINARY" ] || [ "$RETH_BINARY" = "reth" ]; then
        RETH_BINARY=$(find_binary "reth" "$profile" "$root") || {
            log_error "reth binary not found in PATH or target/${profile}/"; return 1; }
        log_info "Using reth binary: $RETH_BINARY"
    fi
    if [ -z "$BENCH_BINARY" ] || [ "$BENCH_BINARY" = "reth-bench" ]; then
        BENCH_BINARY=$(find_binary "reth-bench" "$profile" "$root") || {
            log_error "reth-bench binary not found in PATH or target/${profile}/"; return 1; }
        log_info "Using reth-bench binary: $BENCH_BINARY"
    fi
}

# Generate JWT secret if missing
generate_jwt_secret() {
    local path="$1"
    [ -f "$path" ] && return 0
    local dir=$(dirname "$path")
    [ ! -d "$dir" ] && { log "Creating JWT secret directory: $dir"; mkdir -p "$dir" || { log_error "Failed to create directory: $dir"; return 1; }; }
    log "Generating JWT secret at: $path"
    command -v openssl >/dev/null 2>&1 && openssl rand -hex 32 | tr -d "\n" > "$path" 2>/dev/null && {
        log "JWT secret generated successfully using openssl"; return 0; }
    [ -c /dev/urandom ] && head -c 32 /dev/urandom | xxd -p -c 32 | tr -d "\n" > "$path" 2>/dev/null && {
        log "JWT secret generated successfully using /dev/urandom"; return 0; }
    log_error "Failed to generate JWT secret. Install openssl or generate manually:"
    log_error "openssl rand -hex 32 | tr -d \"\\n\" > $path"
    return 1
}

# Get default JWT secret path based on chain
get_default_jwt_secret() {
    local chain="${1:-mainnet}"
    local datadir="${RETH_DATADIR:-${HOME}/.local/share/reth}"
    echo "${datadir}/${chain}/jwt.hex"
}

# Set default JWT secret if not provided
set_default_jwt_secret() {
    [ -z "$JWT_SECRET" ] && JWT_SECRET=$(get_default_jwt_secret "$RETH_CHAIN")
}

# Presets
preset_mainnet() { RETH_CHAIN="mainnet"; RETH_DATADIR="${RETH_DATADIR:-${HOME}/.local/share/reth}"; log_info "Using mainnet preset"; }
preset_sepolia() { RETH_CHAIN="sepolia"; RETH_DATADIR="${RETH_DATADIR:-${HOME}/.local/share/reth}"; log_info "Using sepolia preset"; }
preset_holesky() { RETH_CHAIN="holesky"; RETH_DATADIR="${RETH_DATADIR:-${HOME}/.local/share/reth}"; log_info "Using holesky preset"; }
preset_quick() { ADVANCE="${ADVANCE:-10}"; log_info "Using quick test preset (10 blocks)"; }
preset_medium() { ADVANCE="${ADVANCE:-100}"; log_info "Using medium test preset (100 blocks)"; }
preset_long() { ADVANCE="${ADVANCE:-1000}"; log_info "Using long test preset (1000 blocks)"; }
preset_with_metrics() { RETH_METRICS="${RETH_METRICS:-localhost:9001}"; log_info "Using metrics preset (localhost:9001)"; }
preset_local() { RPC_URL="${RPC_URL:-http://localhost:8545}"; RETH_DATADIR="${RETH_DATADIR:-${HOME}/.local/share/reth}"; log_info "Using local development preset"; }
preset_production() { preset_mainnet; preset_with_metrics; log_info "Using production-like preset"; }

# Check if engine API is ready
check_engine_api() {
    local url="$1" host_port=$(echo "$url" | sed -E 's|https?://||' | sed 's|/.*||')
    local host=$(echo "$host_port" | cut -d: -f1) port=$(echo "$host_port" | cut -d: -f2)
    command -v nc >/dev/null 2>&1 && nc -z "$host" "$port" 2>/dev/null && return 0
    command -v curl >/dev/null 2>&1 && curl -s --max-time 2 "$url" >/dev/null 2>&1 && return 0
    return 1
}

# Wait for node to be ready
wait_for_node() {
    local url="$1" elapsed=0 interval=2
    log "Waiting for reth node to be ready..."
    while [ $elapsed -lt $MAX_WAIT_TIME ]; do
        check_engine_api "$url" && { log "Reth node is ready!"; return 0; }
        sleep $interval; elapsed=$((elapsed + interval)); echo -n "."
    done
    echo ""; log_error "Reth node did not become ready within ${MAX_WAIT_TIME} seconds"; return 1
}

# Check/get node PID
check_existing_node() {
    [ -f "$PID_FILE" ] && {
        local pid=$(cat "$PID_FILE" 2>/dev/null || echo "")
        [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null && return 0
        rm -f "$PID_FILE"
    }
    return 1
}
get_node_pid() { [ -f "$PID_FILE" ] && cat "$PID_FILE" 2>/dev/null || echo ""; }

# Build reth node command
build_reth_cmd() {
    local cmd="$RETH_BINARY node --chain $RETH_CHAIN --authrpc.jwtsecret $JWT_SECRET"
    [ -n "$RETH_DATADIR" ] && cmd="$cmd --datadir $RETH_DATADIR"
    [ -n "$RETH_METRICS" ] && cmd="$cmd --metrics $RETH_METRICS"
    [ -n "$RETH_EXTRA_ARGS" ] && cmd="$cmd $RETH_EXTRA_ARGS"
    echo "$cmd"
}

# Build benchmark command
build_bench_cmd() {
    local cmd="$BENCH_BINARY $BENCH_COMMAND --rpc-url $RPC_URL --jwt-secret $JWT_SECRET --engine-rpc-url $ENGINE_RPC_URL"
    [ -n "$ADVANCE" ] && cmd="$cmd --advance $ADVANCE" || cmd="$cmd --from $FROM_BLOCK --to $TO_BLOCK"
    [ -n "$BENCH_OUTPUT" ] && cmd="$cmd --output $BENCH_OUTPUT"
    [ -n "$WAIT_TIME" ] && cmd="$cmd --wait-time $WAIT_TIME"
    echo "$cmd"
}

# Start reth node
start_reth_node() {
    local no_wait="${1:-false}"
    log "Starting reth node..."
    local cmd=$(build_reth_cmd)
    log "Command: $cmd"
    nohup $cmd > "$LOG_FILE" 2>&1 & NODE_PID=$!
    echo "$NODE_PID" > "$PID_FILE"
    log "Reth node started with PID: $NODE_PID"
    [ "$no_wait" = "false" ] && wait_for_node "$ENGINE_RPC_URL" || log "Node started in background"
}

# Command: start
cmd_start() {
    local NO_WAIT=false RUN_BENCH=false
    while [[ $# -gt 0 ]]; do
        case $1 in
            --jwt-secret) JWT_SECRET="$2"; shift 2 ;;
            --reth-binary) RETH_BINARY="$2"; shift 2 ;;
            --profile) PROFILE="$2"; shift 2 ;;
            --datadir) RETH_DATADIR="$2"; shift 2 ;;
            --chain) RETH_CHAIN="$2"; shift 2 ;;
            --metrics) RETH_METRICS="$2"; shift 2 ;;
            --engine-rpc-url) ENGINE_RPC_URL="$2"; shift 2 ;;
            --reth-extra-args) RETH_EXTRA_ARGS="$2"; shift 2 ;;
            --pid-file) PID_FILE="$2"; shift 2 ;;
            --log-file) LOG_FILE="$2"; shift 2 ;;
            --max-wait-time) MAX_WAIT_TIME="$2"; shift 2 ;;
            --no-wait) NO_WAIT=true; shift ;;
            --run-bench) RUN_BENCH=true; shift ;;
            --rpc-url) RPC_URL="$2"; shift 2 ;;
            --from) FROM_BLOCK="$2"; shift 2 ;;
            --to) TO_BLOCK="$2"; shift 2 ;;
            --advance) ADVANCE="$2"; shift 2 ;;
            --bench-command) BENCH_COMMAND="$2"; shift 2 ;;
            --bench-output) BENCH_OUTPUT="$2"; shift 2 ;;
            --wait-time) WAIT_TIME="$2"; shift 2 ;;
            --project) PROJECT_NAME="$2"; shift 2 ;;
            quick) preset_quick; shift ;;
            medium) preset_medium; shift ;;
            long) preset_long; shift ;;
            mainnet) preset_mainnet; shift ;;
            sepolia) preset_sepolia; shift ;;
            holesky) preset_holesky; shift ;;
            with-metrics) preset_with_metrics; shift ;;
            production) preset_production; shift ;;
            -h|--help) cat << EOF
Usage: $0 start [OPTIONS] [PRESET]

Start a reth node in the background.

PRESETS: mainnet, sepolia, holesky, with-metrics, production, quick, medium, long

Options:
  --jwt-secret PATH          JWT secret file (default: ~/.local/share/reth/{chain}/jwt.hex, auto-generated if missing)
  --profile PROFILE          Build profile (default: release)
  --project NAME             Project name (reth or gravity-reth) to find binaries in ~/{NAME}/target (default: empty, uses current project)
  --run-bench                Run benchmark after node is ready
  --rpc-url URL              RPC URL for benchmark (default: http://localhost:8545)
  --advance N / --from X --to Y  Block range for benchmark
  --no-wait                  Don't wait for node readiness
  -h, --help                 Show help

Examples:
  $0 start
  $0 start mainnet
  $0 start mainnet quick --run-bench
  $0 start --project reth
  $0 start --project gravity-reth
EOF
                exit 0 ;;
            *) log_error "Unknown option: $1"; exit 1 ;;
        esac
    done
    set_default_jwt_secret
    resolve_binaries "$PROFILE" "$PROJECT_ROOT" || exit 1
    generate_jwt_secret "$JWT_SECRET" || exit 1
    check_existing_node && { log_warn "Node already running (PID: $(get_node_pid))"; exit 1; }
    start_reth_node "$NO_WAIT"
    [ "$NO_WAIT" = false ] && wait_for_node "$ENGINE_RPC_URL" && {
        log "Node is ready for benchmarking!"
        [ "$RUN_BENCH" = true ] && {
            [ -z "$BENCH_BINARY" ] && resolve_binaries "$PROFILE" "$PROJECT_ROOT" || exit 1
            [ -z "$ADVANCE" ] && [ -z "$FROM_BLOCK" ] && [ -z "$TO_BLOCK" ] && {
                log_error "Either --advance or --from/--to required for benchmark"; exit 1; }
            log "Starting benchmark..."
            local bench_cmd=$(build_bench_cmd)
            log "Running: $bench_cmd"
            $bench_cmd && log "Benchmark completed!" || { log_error "Benchmark failed"; exit $?; }
        }
    }
}

# Command: stop
cmd_stop() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --pid-file) PID_FILE="$2"; shift 2 ;;
            -h|--help) echo "Usage: $0 stop [--pid-file PATH]"; exit 0 ;;
            *) log_error "Unknown option: $1"; exit 1 ;;
        esac
    done
    [ ! -f "$PID_FILE" ] && { log_warn "PID file not found: $PID_FILE"; exit 0; }
    NODE_PID=$(get_node_pid)
    [ -z "$NODE_PID" ] && { rm -f "$PID_FILE"; exit 0; }
    kill -0 "$NODE_PID" 2>/dev/null || { rm -f "$PID_FILE"; exit 0; }
    log "Stopping reth node (PID: $NODE_PID)..."
    kill "$NODE_PID" 2>/dev/null || true
    for i in {1..10}; do kill -0 "$NODE_PID" 2>/dev/null || { log "Node stopped"; rm -f "$PID_FILE"; exit 0; }; sleep 1; done
    kill -9 "$NODE_PID" 2>/dev/null || true; rm -f "$PID_FILE"; log "Node stopped"
}

# Command: bench
cmd_bench() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --rpc-url) RPC_URL="$2"; shift 2 ;;
            --jwt-secret) JWT_SECRET="$2"; shift 2 ;;
            --from) FROM_BLOCK="$2"; shift 2 ;;
            --to) TO_BLOCK="$2"; shift 2 ;;
            --advance) ADVANCE="$2"; shift 2 ;;
            --bench-command) BENCH_COMMAND="$2"; shift 2 ;;
            --bench-output) BENCH_OUTPUT="$2"; shift 2 ;;
            --wait-time) WAIT_TIME="$2"; shift 2 ;;
            --reth-binary) RETH_BINARY="$2"; shift 2 ;;
            --bench-binary) BENCH_BINARY="$2"; shift 2 ;;
            --profile) PROFILE="$2"; shift 2 ;;
            --datadir) RETH_DATADIR="$2"; shift 2 ;;
            --chain) RETH_CHAIN="$2"; shift 2 ;;
            --metrics) RETH_METRICS="$2"; shift 2 ;;
            --engine-rpc-url) ENGINE_RPC_URL="$2"; shift 2 ;;
            --reth-extra-args) RETH_EXTRA_ARGS="$2"; shift 2 ;;
            --max-wait-time) MAX_WAIT_TIME="$2"; shift 2 ;;
            --use-existing-node) USE_EXISTING_NODE=true; shift ;;
            --pid-file) PID_FILE="$2"; shift 2 ;;
            --log-file) LOG_FILE="$2"; shift 2 ;;
            mainnet) preset_mainnet; shift ;;
            sepolia) preset_sepolia; shift ;;
            holesky) preset_holesky; shift ;;
            quick) preset_quick; shift ;;
            medium) preset_medium; shift ;;
            long) preset_long; shift ;;
            with-metrics) preset_with_metrics; shift ;;
            local) preset_local; shift ;;
            production) preset_production; shift ;;
            -h|--help) cat << EOF
Usage: $0 bench [OPTIONS] [PRESET]

Start a reth node and execute a benchmark.

PRESETS: mainnet, sepolia, holesky, quick, medium, long, with-metrics, local, production

Options:
  --jwt-secret PATH          JWT secret file (default: ~/.local/share/reth/{chain}/jwt.hex, auto-generated if missing)
  --rpc-url URL              RPC URL (default: http://localhost:8545)
  --advance N or --from X --to Y  Block range (required)

Examples:
  $0 bench mainnet quick
  $0 bench --advance 10
  $0 bench --jwt-secret ~/.custom/jwt.hex --advance 10
EOF
                exit 0 ;;
            *) log_error "Unknown option or preset: $1"; exit 1 ;;
        esac
    done
    set_default_jwt_secret
    [ -z "$ADVANCE" ] && [ -z "$FROM_BLOCK" ] && [ -z "$TO_BLOCK" ] && {
        log_error "Either --advance or --from/--to required"; exit 1; }
    [ -n "$ADVANCE" ] && [ -n "$FROM_BLOCK" ] && {
        log_error "--advance cannot be used with --from/--to"; exit 1; }
    resolve_binaries "$PROFILE" "$PROJECT_ROOT" || exit 1
    generate_jwt_secret "$JWT_SECRET" || exit 1
    log "Starting benchmark... RPC: $RPC_URL"
    cleanup_bench() {
        [ "$USE_EXISTING_NODE" = false ] && [ -n "$NODE_PID" ] && kill -0 "$NODE_PID" 2>/dev/null && {
            log "Stopping reth node (PID: $NODE_PID)..."; kill "$NODE_PID" 2>/dev/null || true; wait "$NODE_PID" 2>/dev/null || true; }
    }
    trap cleanup_bench EXIT INT TERM
    [ "$USE_EXISTING_NODE" = false ] && {
        start_reth_node "false"
        wait_for_node "$ENGINE_RPC_URL" || { log_error "Failed to start node"; tail -20 "$LOG_FILE" 2>/dev/null; exit 1; }
    } || { log "Using existing node"; wait_for_node "$ENGINE_RPC_URL" || { log_error "Existing node not accessible"; exit 1; }; }
    local bench_cmd=$(build_bench_cmd)
    log "Running benchmark: $bench_cmd"
    $bench_cmd && { log "Benchmark completed!"; exit 0; } || { log_error "Benchmark failed"; exit $?; }
}

# Main dispatcher
main() {
    [ $# -eq 0 ] && {
        cat << EOF
Usage: $0 <COMMAND> [OPTIONS]

Commands:
  start    Start a reth node in the background
  stop     Stop a running reth node
  bench    Start a reth node and execute a benchmark

Examples:
  $0 start
  $0 start mainnet
  $0 bench mainnet quick
  $0 stop

Run '$0 <COMMAND> --help' for detailed help.
EOF
        exit 1; }
    COMMAND="$1"; shift
    case "$COMMAND" in
        start) cmd_start "$@" ;;
        stop) cmd_stop "$@" ;;
        bench) cmd_bench "$@" ;;
        -h|--help) main ;;
        *) log_error "Unknown command: $COMMAND"; exit 1 ;;
    esac
}

main "$@"
