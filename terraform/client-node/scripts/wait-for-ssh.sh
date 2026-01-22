#!/bin/bash
# Wait for SSH to be available on a host

HOST="$1"
SSH_KEY="${2:-}"
SSH_USER="${3:-ubuntu}"
MAX_WAIT="${4:-300}"
WAIT_INTERVAL="${5:-10}"

[ -z "$HOST" ] && { echo "Usage: $0 <host> [ssh_key] [ssh_user] [max_wait] [wait_interval]" >&2; exit 1; }

SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=5"
SSH_CMD="ssh $SSH_OPTS"
[ -n "$SSH_KEY" ] && SSH_CMD="$SSH_CMD -i $SSH_KEY"

echo "⏳ Waiting for SSH on $HOST..."

elapsed=0
while [ $elapsed -lt $MAX_WAIT ]; do
    $SSH_CMD "$SSH_USER@$HOST" "echo" >/dev/null 2>&1 && {
        echo "✅ SSH is ready"
        exit 0
    }
    echo "  ⏳ Still waiting... (${elapsed}/${MAX_WAIT}s)"
    sleep $WAIT_INTERVAL
    elapsed=$((elapsed + WAIT_INTERVAL))
done

echo "❌ Timeout after ${MAX_WAIT}s" >&2
exit 1

