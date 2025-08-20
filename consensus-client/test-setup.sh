#!/bin/bash

# Test script for FastEVM Consensus Client setup
# This script verifies the docker-compose configuration and setup

set -e

echo "Testing FastEVM Consensus Client setup..."
echo "========================================"

# Check if docker compose is available
if ! docker compose version &> /dev/null; then
    echo "❌ docker compose not found. Please install Docker Compose first."
    exit 1
fi

# Check if Docker is running
if ! docker info &> /dev/null; then
    echo "❌ Docker is not running. Please start Docker first."
    exit 1
fi

# Check if config files exist
echo "Checking configuration files..."
if [ ! -f "examples/node0.yml" ]; then
    echo "❌ node0.yml not found in examples directory"
    exit 1
fi

if [ ! -f "examples/node1.yml" ]; then
    echo "❌ node1.yml not found in examples directory"
    exit 1
fi

if [ ! -f "examples/node2.yml" ]; then
    echo "❌ node2.yml not found in examples directory"
    exit 1
fi

if [ ! -f "examples/node3.yml" ]; then
    echo "❌ node3.yml not found in examples directory"
    exit 1
fi

if [ ! -f "examples/committees.yml" ]; then
    echo "❌ committees.yml not found in examples directory"
    exit 1
fi

echo "✅ All configuration files found"

# Check if docker-compose.yml exists and is valid
echo "Checking docker-compose.yml..."
if [ ! -f "docker-compose.yml" ]; then
    echo "❌ docker-compose.yml not found"
    exit 1
fi

# Validate docker-compose.yml
if ! docker compose config &> /dev/null; then
    echo "❌ docker-compose.yml is invalid"
    docker compose config
    exit 1
fi

echo "✅ docker-compose.yml is valid"

# Check if setup script exists and is executable
echo "Checking setup script..."
if [ ! -f "setup-configs.sh" ]; then
    echo "❌ setup-configs.sh not found"
    exit 1
fi

if [ ! -x "setup-configs.sh" ]; then
    echo "❌ setup-configs.sh is not executable"
    exit 1
fi

echo "✅ setup-configs.sh is ready"

# Check if Dockerfile exists
echo "Checking Dockerfile..."
if [ ! -f "Dockerfile" ]; then
    echo "❌ Dockerfile not found"
    exit 1
fi

echo "✅ Dockerfile found"

# Check if external network exists
echo "Checking Docker network..."
if ! docker network ls | grep -q "fastevm-network"; then
    echo "⚠️  fastevm-network not found. You may need to create it first:"
    echo "   docker network create --subnet=172.20.0.0/16 fastevm-network"
else
    echo "✅ fastevm-network found"
fi

echo ""
echo "🎉 Setup verification complete!"
echo ""
echo "Next steps:"
echo "1. Run: ./setup-configs.sh"
echo "2. Run: docker compose up -d"
echo "3. Check logs: docker compose logs -f"
echo ""
echo "The init container will generate committee configuration,"
echo "and all 4 nodes will start using their respective config files."
