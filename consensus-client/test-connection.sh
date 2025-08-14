#!/bin/bash

# Test script to verify execution client connection
# This script helps debug connection issues between consensus and execution clients

echo "Testing execution client connections..."

# Test each execution client endpoint
for i in {0..3}; do
    port=$((8551 + i))
    echo "Testing execution client $i on port $port..."
    
    # Test HTTP connection
    if curl -s "http://localhost:$port" > /dev/null 2>&1; then
        echo "✓ Execution client $i is responding on port $port"
    else
        echo "✗ Execution client $i is not responding on port $port"
    fi
done

echo ""
echo "Testing consensus client configuration..."

# Check if config files exist and are valid
for i in {0..3}; do
    config_file="config/node$i.yml"
    if [ -f "$config_file" ]; then
        echo "✓ Config file $config_file exists"
        
        # Check execution URL
        exec_url=$(grep "execution_url:" "$config_file" | cut -d'"' -f2)
        echo "  Execution URL: $exec_url"
        
        # Check JWT secret format
        jwt_secret=$(grep "jwt_secret:" "$config_file" | cut -d'"' -f2)
        if [[ $jwt_secret =~ ^0x[a-fA-F0-9]{64}$ ]]; then
            echo "  ✓ JWT secret format is valid"
        else
            echo "  ✗ JWT secret format is invalid (should be 0x + 64 hex chars)"
        fi
    else
        echo "✗ Config file $config_file missing"
    fi
done

echo ""
echo "Testing Docker network..."

# Check if containers are running
echo "Checking container status..."
docker ps --filter "name=fastevm" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

echo ""
echo "Testing network connectivity..."

# Test network connectivity between consensus and execution clients
for i in {0..3}; do
    consensus_container="fastevm-consensus$i"
    execution_container="fastevm-execution$i"
    
    if docker exec "$consensus_container" ping -c 1 "$execution_container" > /dev/null 2>&1; then
        echo "✓ $consensus_container can reach $execution_container"
    else
        echo "✗ $consensus_container cannot reach $execution_container"
    fi
done
