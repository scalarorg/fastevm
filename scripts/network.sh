#!/bin/bash

NETWORK_NAME="fastevm-network"
SUBNET_RANGE="172.20.0.0/16"
GATEWAY_IP="172.20.0.1"

# Check if network exists
if ! docker network inspect "$NETWORK_NAME" >/dev/null 2>&1; then
    echo "Network '$NETWORK_NAME' does not exist. Creating..."
    docker network create \
        --driver=bridge \
        --subnet="$SUBNET_RANGE" \
        --gateway="$GATEWAY_IP" \
        "$NETWORK_NAME"
    echo "Network '$NETWORK_NAME' created with subnet $SUBNET_RANGE"
else
    echo "Network '$NETWORK_NAME' already exists."
fi
