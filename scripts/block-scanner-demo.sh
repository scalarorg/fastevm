#!/usr/bin/env bash

# Block Scanner Demo Script for FastEVM
# This script demonstrates how to use the block scanning functionality

echo "🚀 FastEVM Block Scanner Demo"
echo "=============================="
echo ""

# Set default RPC URL if not provided
RPC_URL=${RPC_URL:-"http://localhost:8545"}

echo "Using RPC URL: $RPC_URL"
echo ""

# Function to run block scan tests
run_tests() {
    echo "🔍 Running block scan tests..."
    echo ""
    
    # Test 1: Scan first 10 blocks
    echo "Test 1: Scanning first 10 blocks"
    echo "--------------------------------"
    cargo test test_block_scan_demo -- --nocapture
    
    echo ""
    echo "Test 2: Scanning with custom RPC URL"
    echo "-----------------------------------"
    RPC_URL1="$RPC_URL" cargo test test_scan_blocks_custom_url -- --nocapture
    
    echo ""
    echo "✅ All tests completed!"
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -h, --help     Show this help message"
    echo "  -t, --test     Run block scan tests"
    echo "  -u, --url URL  Set custom RPC URL (default: http://localhost:8545)"
    echo ""
    echo "Examples:"
    echo "  $0 --test"
    echo "  $0 --url http://localhost:8547 --test"
    echo ""
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_usage
            exit 0
            ;;
        -t|--test)
            run_tests
            exit 0
            ;;
        -u|--url)
            RPC_URL="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# If no arguments provided, show usage
show_usage
