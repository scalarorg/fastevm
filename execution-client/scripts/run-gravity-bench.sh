#!/bin/bash
# Script to run gravity_bench benchmark
# Usage: ./run-gravity-bench.sh <config_file> <working_dir> [gravity_bench_path] [recovery_mode]

set -e

CONFIG_FILE="$1"
WORKING_DIR="$2"
GRAVITY_BENCH_PATH="${3:-gravity_bench}"
RECOVERY_MODE="$4"

if [ -z "$CONFIG_FILE" ] || [ -z "$WORKING_DIR" ]; then
    echo "Usage: $0 <config_file> <working_dir> [gravity_bench_path] [recovery_mode]"
    exit 1
fi

if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Config file not found at $CONFIG_FILE"
    exit 1
fi

# Check if gravity_bench exists
if ! command -v "$GRAVITY_BENCH_PATH" &> /dev/null; then
    echo "Error: gravity_bench not found. Install from: https://github.com/Galxe/gravity_bench.git"
    exit 1
fi

# Build command arguments
ARGS=("--config" "$CONFIG_FILE")

if [ "$RECOVERY_MODE" = "true" ]; then
    ARGS+=("--recover")
fi

# Run gravity_bench
cd "$WORKING_DIR"
"$GRAVITY_BENCH_PATH" "${ARGS[@]}"

