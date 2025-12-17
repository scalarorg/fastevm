#!/bin/bash
# Script to check balances for faucet and secondary accounts via RPC

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
DEFAULT_GRAVITY_BENCH_DIR="$(cd "$REPO_ROOT/../gravity/gravity_bench" 2>/dev/null && pwd || true)"

# Default values
RPC_HOST="${RPC_HOST:-34.134.212.150}"
RPC_PORT="${RPC_PORT:-8545}"
TEST_ADDRESS="${TEST_ADDRESS:-0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266}"
GRAVITY_BENCH_DIR="${GRAVITY_BENCH_DIR:-$DEFAULT_GRAVITY_BENCH_DIR}"
ACCOUNTS_FILE="${ACCOUNTS_FILE:-}"
MAX_ADDRESSES="${MAX_ADDRESSES:-}"
INCLUDE_DEFAULT="${INCLUDE_DEFAULT:-true}"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

print_help() {
    cat << EOF
Usage: $0 [OPTIONS]

Check balances for faucet and gravity_bench secondary accounts via RPC.

Options:
  --host HOST               RPC host (default: $RPC_HOST)
  --port PORT               RPC port (default: $RPC_PORT)
  --address ADDR            Faucet address to include (default: $TEST_ADDRESS)
  --gravity-bench-dir PATH  Path to gravity_bench project (default: $GRAVITY_BENCH_DIR)
  --accounts-file FILE      Explicit accounts file (defaults to <gravity_bench_dir>/accounts.txt)
  --limit N                 Limit number of secondary accounts checked
  --skip-default            Do not include the faucet/default address
  -h, --help                Show this help message

Environment variables:
  RPC_HOST, RPC_PORT, TEST_ADDRESS
  GRAVITY_BENCH_DIR, ACCOUNTS_FILE
  MAX_ADDRESSES (alias for --limit)
  INCLUDE_DEFAULT=true|false
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --host)
            RPC_HOST="$2"
            shift 2
            ;;
        --port)
            RPC_PORT="$2"
            shift 2
            ;;
        --address)
            TEST_ADDRESS="$2"
            shift 2
            ;;
        --gravity-bench-dir)
            GRAVITY_BENCH_DIR="$2"
            shift 2
            ;;
        --accounts-file)
            ACCOUNTS_FILE="$2"
            shift 2
            ;;
        --limit)
            MAX_ADDRESSES="$2"
            shift 2
            ;;
        --skip-default)
            INCLUDE_DEFAULT="false"
            shift 1
            ;;
        -h|--help)
            print_help
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            print_help
            exit 1
            ;;
    esac
done

RPC_URL="http://${RPC_HOST}:${RPC_PORT}"
DEFAULT_ACCOUNTS_FILE=""
if [ -z "${ACCOUNTS_FILE}" ] && [ -n "${GRAVITY_BENCH_DIR}" ]; then
    DEFAULT_ACCOUNTS_FILE="${GRAVITY_BENCH_DIR}/accounts.txt"
    if [ -f "$DEFAULT_ACCOUNTS_FILE" ]; then
        ACCOUNTS_FILE="$DEFAULT_ACCOUNTS_FILE"
    fi
fi

if [ -n "$MAX_ADDRESSES" ] && ! [[ "$MAX_ADDRESSES" =~ ^[0-9]+$ ]]; then
    echo -e "${RED}--limit must be a positive integer${NC}"
    exit 1
fi

if [ -z "$GRAVITY_BENCH_DIR" ] && [ -z "$ACCOUNTS_FILE" ]; then
    echo -e "${YELLOW}gravity_bench directory not found. Provide --gravity-bench-dir or --accounts-file if you want to check secondary accounts.${NC}"
fi

declare -a ADDRESSES=()

address_exists() {
    local addr="$1"
    for existing in "${ADDRESSES[@]:-}"; do
        if [ "$existing" = "$addr" ]; then
            return 0
        fi
    done
    return 1
}

add_address() {
    local addr="$1"
    addr="$(echo "$addr" | tr '[:upper:]' '[:lower:]')"
    [[ -z "$addr" ]] && return 0
    if [[ ! "$addr" =~ ^0x[0-9a-f]{40}$ ]]; then
        echo -e "${YELLOW}Skipping invalid address: $addr${NC}"
        return 0
    fi
    if ! address_exists "$addr"; then
        ADDRESSES+=("$addr")
    fi
}

if [ "$INCLUDE_DEFAULT" = "true" ]; then
    add_address "$TEST_ADDRESS"
fi

load_accounts_from_file() {
    local file="$1"
    local limit="${2:-}"
    local count=0

    if [ ! -f "$file" ]; then
        echo -e "${YELLOW}Accounts file not found: $file${NC}"
        return
    fi

    echo -e "${GREEN}Loading secondary accounts from${NC} $file"
    while IFS= read -r line || [ -n "$line" ]; do
        line="${line%%#*}"
        line="$(echo "$line" | xargs || true)"
        [ -z "$line" ] && continue
        local addr="$(echo "$line" | cut -d',' -f1 | xargs)"
        add_address "$addr"
        count=$((count + 1))
        if [ -n "$limit" ] && [ "$count" -ge "$limit" ]; then
            break
        fi
    done < "$file"
    echo -e "${GREEN}Loaded $count addresses from accounts file${NC}"
}

if [ -n "$ACCOUNTS_FILE" ]; then
    load_accounts_from_file "$ACCOUNTS_FILE" "$MAX_ADDRESSES"
else
    echo -e "${YELLOW}No accounts file provided/found. Checking faucet/default address only.${NC}"
fi

if [ ${#ADDRESSES[@]} -eq 0 ]; then
    echo -e "${RED}No addresses to check. Exiting.${NC}"
    exit 1
fi

echo -e "${GREEN}Checking balances on dev node${NC}"
echo "RPC URL: $RPC_URL"
if [ -n "$ACCOUNTS_FILE" ]; then
    echo "Accounts file: $ACCOUNTS_FILE"
fi
echo "Total addresses: ${#ADDRESSES[@]}"
echo ""

# Check if jq is available
if ! command -v jq &> /dev/null; then
    echo -e "${YELLOW}Warning: jq not found. Falling back to basic JSON parsing.${NC}"
    USE_JQ=false
else
    USE_JQ=true
fi

print_balance_table_header() {
    printf "%-6s %-44s %-18s %-24s\n" "Index" "Address" "Balance (ETH)" "Balance (wei)"
    printf "%-6s %-44s %-18s %-24s\n" "-----" "--------------------------------------------" "------------------" "------------------------"
}

hex_to_decimal() {
    local hex_value="$1"
    python3 -c "print(int('$hex_value', 16))" 2>/dev/null || echo "$hex_value"
}

wei_to_eth() {
    local hex_value="$1"
    if command -v python3 &> /dev/null; then
        python3 -c "print('{:.6f}'.format(int('$hex_value', 16) / 10**18))" 2>/dev/null || echo "N/A"
    else
        echo "N/A"
    fi
}

fetch_balance() {
    local address="$1"
    local response
    if ! response=$(curl -s -X POST "$RPC_URL" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"eth_getBalance\",\"params\":[\"$address\",\"latest\"],\"id\":1}"); then
        echo "error:curl"
        return 0
    fi

    local error=""
    local balance_hex=""
    if [ "$USE_JQ" = true ]; then
        error=$(echo "$response" | jq -r '.error.message // empty' 2>/dev/null || true)
        balance_hex=$(echo "$response" | jq -r '.result // empty' 2>/dev/null || true)
    else
        if echo "$response" | grep -q '"error"'; then
            error="RPC error"
        else
            balance_hex=$(echo "$response" | grep -o '"result":"[^"]*"' | cut -d'"' -f4)
        fi
    fi

    if [ -n "$error" ]; then
        echo "error:$error"
        return 0
    fi

    if [ -z "$balance_hex" ] || [ "$balance_hex" = "null" ]; then
        echo "error:empty_result"
        return 0
    fi

    echo "$balance_hex"
}

print_balance_table_header
index=1
error_count=0

for address in "${ADDRESSES[@]}"; do
    balance_hex=$(fetch_balance "$address")
    if [[ "$balance_hex" == error:* ]]; then
        printf "%-6s %-44s %-18s %-24s\n" "$index" "$address" "ERROR" "${balance_hex#error:}"
        error_count=$((error_count + 1))
    else
        balance_hex_clean="${balance_hex#0x}"
        balance_wei=$(hex_to_decimal "$balance_hex")
        balance_eth=$(wei_to_eth "$balance_hex")
        printf "%-6s %-44s %-18s %-24s\n" "$index" "$address" "$balance_eth" "$balance_wei"
    fi
    index=$((index + 1))
done

echo ""
echo -e "${GREEN}Completed balance check for ${#ADDRESSES[@]} address(es).${NC}"
if [ "$error_count" -gt 0 ]; then
    echo -e "${YELLOW}Warnings:${NC} Failed to fetch ${error_count} balance(s). See table above."
fi
