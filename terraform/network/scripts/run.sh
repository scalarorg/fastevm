#!/bin/bash
set -euo pipefail
DATA_DIR="/data"
execution() {
    echo "🔧 Starting FastEVM Execution Client (debug mode)"

    # --------------------------------------------------
    # Match systemd execution context
    # --------------------------------------------------

    # Load environment file (same as systemd)
    if [[ -f $DATA_DIR/node.env ]]; then
    set -a
    source $DATA_DIR/node.env
    set +a
    else
    echo "❌ Missing $DATA_DIR/node.env"
    exit 1
    fi

    # Explicit Environment= values from service
    export HTTP_PORT=8545
    export WS_PORT=8546
    export ENGINE_PORT=8551
    export P2P_PORT=30303
    export LOG_LEVEL=vvv

    # Defaults for optional vars
    export SUBDAGS_PER_BLOCK="${SUBDAGS_PER_BLOCK:-30}"
    export BLOCK_BUILD_INTERVAL="${BLOCK_BUILD_INTERVAL:-1000}"

    # --------------------------------------------------
    # Sanity checks (important)
    # --------------------------------------------------
    echo "🔍 Checking required files..."

    for f in \
    "$DATA_DIR/config/genesis.json" \
    "$DATA_DIR/execution/jwt.hex" \
    "$DATA_DIR/execution/p2p/secret.key"
    do
    [[ -f "$f" ]] || { echo "❌ Missing file: $f"; exit 2; }
    done

    : "${BOOTNODES:?❌ BOOTNODES is not set}"
    : "${GAS_LIMIT:?❌ GAS_LIMIT is not set}"

    echo "✅ Environment OK"
    echo

    # --------------------------------------------------
    # Run FastEVM Execution Client
    # --------------------------------------------------
    set -x
    exec /usr/local/bin/fastevm-execution node \
    --chain "${DATA_DIR}/config/genesis.json" \
    --datadir "${DATA_DIR}/execution" \
    --engine.always-process-payload-attributes-on-canonical-head \
    --http \
    --http.addr 0.0.0.0 \
    --http.port "${HTTP_PORT}" \
    --http.corsdomain "*" \
    --ws \
    --ws.addr 0.0.0.0 \
    --ws.port "${WS_PORT}" \
    --ws.origins "*" \
    --builder.gaslimit "${GAS_LIMIT}" \
    --txpool.max-new-txns 102400 \
    --gravity.disable-pipe-execution \
    --authrpc.addr 0.0.0.0 \
    --authrpc.port "${ENGINE_PORT}" \
    --authrpc.jwtsecret "${DATA_DIR}/execution/jwt.hex" \
    --addr 0.0.0.0 \
    --port "${P2P_PORT}" \
    --discovery.addr 0.0.0.0 \
    --discovery.port "${P2P_PORT}" \
    --p2p-secret-key "${DATA_DIR}/execution/p2p/secret.key" \
    --bootnodes "${BOOTNODES}" \
    --enable-tx-subscription \
    --committed-subdags-per-block "${SUBDAGS_PER_BLOCK}" \
    --block-build-interval-ms "${BLOCK_BUILD_INTERVAL}" \
    -${LOG_LEVEL}

}

consensus() {
    echo "🔧 Starting FastEVM Consensus Client (debug mode)"
}

$@