#!/bin/bash

set -e

echo "🔨 Building FastEVM Smart Contracts with Foundry..."

# Check if Foundry is installed
if ! command -v forge &> /dev/null; then
    echo "❌ Foundry is not installed. Please install Foundry first:"
    echo "   curl -L https://foundry.paradigm.xyz | bash"
    echo "   foundryup"
    exit 1
fi

# Change to contracts directory
cd "$(dirname "$0")"

echo "🔧 Compiling contracts..."
forge build

echo "📄 Generating genesis.json..."
npm run genesis

echo "✅ Build completed successfully!"
echo ""
echo "📁 Generated files:"
echo "   - Contract artifacts: ./out/"
echo "   - Genesis file: ../../execution-client/shared/genesis.json"
echo ""
echo "🚀 You can now use the genesis.json file for your FastEVM node configuration."
echo ""
echo "💡 Additional Foundry commands:"
echo "   forge test          - Run tests"
echo "   forge script script/Deploy.s.sol --rpc-url <RPC_URL> --broadcast --verify - Deploy and verify"
echo "   forge coverage      - Generate coverage report"
