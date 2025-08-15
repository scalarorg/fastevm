#!/bin/bash
set -euo pipefail

# Test script for the updated init.sh
# This script demonstrates how the JWT secret generation and config update works

echo "🧪 Testing FastEVM JWT secret generation and config update"
echo "=========================================================="
echo

# Check if we're in the right directory
if [ ! -f "scripts/init.sh" ]; then
    echo "❌ Error: This script must be run from the project root directory"
    echo "Current directory: $(pwd)"
    exit 1
fi

# Create test data directories
echo "📁 Creating test data directories..."
mkdir -p /tmp/test-data1 /tmp/test-data2 /tmp/test-data3 /tmp/test-data4

# Create symbolic links to test directories
echo "🔗 Creating symbolic links to test directories..."
ln -sf /tmp/test-data1 /data1
ln -sf /tmp/test-data2 /data2
ln -sf /tmp/test-data3 /data3
ln -sf /tmp/test-data4 /data4

# Create a test genesis template
echo "📄 Creating test genesis template..."
mkdir -p /tmp/shared
echo '{"test": "genesis"}' > /tmp/shared/genesis.template.json

# Create symbolic link to shared directory
ln -sf /tmp/shared /shared

echo "✅ Test environment prepared"
echo

# Run the init script
echo "🚀 Running init.sh script..."
echo "----------------------------------------"
cd scripts
./init.sh
echo "----------------------------------------"

# Check results
echo
echo "🔍 Checking results..."
echo

for i in {1..4}; do
    data_dir="/data$i"
    jwt_file="${data_dir}/jwt.hex"
    genesis_file="${data_dir}/genesis.json"
    config_file="../consensus-client/examples/node$((i-1)).yml"
    
    echo "Node $((i-1)):"
    
    if [ -f "$jwt_file" ]; then
        jwt_content=$(cat "$jwt_file" | tr -d '\n\r')
        echo "  ✅ JWT secret: ${jwt_content:0:10}..."
    else
        echo "  ❌ JWT secret: NOT FOUND"
    fi
    
    if [ -f "$genesis_file" ]; then
        echo "  ✅ Genesis file: EXISTS"
    else
        echo "  ❌ Genesis file: NOT FOUND"
    fi
    
    if [ -f "$config_file" ]; then
        jwt_in_config=$(grep "^jwt_secret:" "$config_file" | sed 's/.*jwt_secret: *"\([^"]*\)".*/\1/')
        echo "  ✅ Consensus config: EXISTS (JWT: ${jwt_in_config:0:10}...)"
    else
        echo "  ❌ Consensus config: NOT FOUND"
    fi
    
    echo
done

# Cleanup
echo "🧹 Cleaning up test environment..."
rm -f /data1 /data2 /data3 /data4 /shared
rm -rf /tmp/test-data1 /tmp/test-data2 /tmp/test-data3 /tmp/test-data4 /tmp/shared

echo "✅ Test completed!"
echo
echo "💡 To run this in production:"
echo "   1. Make sure you're in the project root directory"
echo "   2. Run: ./execution-client/shared/init.sh"
echo "   3. The script will automatically:"
echo "      - Generate JWT secrets for execution clients"
echo "      - Update consensus client configs with the JWT secrets"
echo "      - Create backups of existing configs"
echo "      - Handle both existing and new config files"
