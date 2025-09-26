#!/bin/bash

# FastEVM Remote Deployment Installation Script
# This script sets up the deployment environment

set -e

echo "🚀 FastEVM Remote Deployment Setup"
echo "=================================="

# Check if Taskfile is installed
if ! command -v task &> /dev/null; then
    echo "❌ Taskfile is not installed"
    echo "Please install Taskfile from https://taskfile.dev/"
    echo ""
    echo "Installation commands:"
    echo "  macOS: brew install go-task/tap/go-task"
    echo "  Linux: sh -c \"\$(curl --location https://taskfile.dev/install.sh)\" -- -d -b /usr/local/bin"
    echo "  Windows: scoop install task"
    exit 1
fi

echo "✅ Taskfile is installed"

# Check if we're in the right directory
if [ ! -f "Taskfile.yml" ]; then
    echo "❌ Taskfile.yml not found"
    echo "Please run this script from the deployment directory"
    exit 1
fi

echo "✅ Taskfile.yml found"

# Create necessary directories
echo "📁 Creating directories..."
mkdir -p config
mkdir -p logs/backups
mkdir -p logs/reports
mkdir -p logs/logs

echo "✅ Directories created"

# Create configuration file if it doesn't exist
if [ ! -f "config/nodes.env" ]; then
    echo "📝 Creating configuration file..."
    cp config/nodes.env.example config/nodes.env
    echo "✅ Configuration file created: config/nodes.env"
    echo "⚠️  Please edit config/nodes.env with your remote node details"
else
    echo "✅ Configuration file already exists"
fi

# Set executable permissions
echo "🔧 Setting permissions..."
chmod +x install.sh
find tasks/ -name "*.yml" -exec chmod 644 {} \;

echo "✅ Permissions set"

# Test Taskfile
echo "🧪 Testing Taskfile..."
if task --list > /dev/null 2>&1; then
    echo "✅ Taskfile is working correctly"
else
    echo "❌ Taskfile test failed"
    exit 1
fi

echo ""
echo "🎉 Setup completed successfully!"
echo ""
echo "Next steps:"
echo "1. Edit config/nodes.env with your remote node details"
echo "2. Test SSH connections: task ssh:test-connections"
echo "3. Deploy FastEVM: task deploy"
echo ""
echo "Available commands:"
echo "  task help          - Show help"
echo "  task list          - List all tasks"
echo "  task deploy        - Deploy to all nodes"
echo "  task status        - Check status"
echo ""
echo "For more information, see README.md"
