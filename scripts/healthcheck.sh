#!/bin/sh

# Healthcheck script for execution client
# This script checks if the execution client RPC endpoint is responding

set -e

# Test the RPC endpoint with a simple eth_blockNumber call
response=$(curl -s -f -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://localhost:8545)

# Check if we got a valid response
if echo "$response" | grep -q '"jsonrpc":"2.0"'; then
  echo "Healthcheck passed: RPC endpoint responding"
  exit 0
else
  echo "Healthcheck failed: Invalid response from RPC endpoint"
  echo "Response: $response"
  exit 1
fi
