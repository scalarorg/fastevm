#!/bin/bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -z "${GITHUB_BRANCH:-}" ]; then
    GITHUB_BRANCH="gravity"
    echo "GITHUB_BRANCH is undefined, setting to default: $GITHUB_BRANCH"
else
    echo "Using input GITHUB_BRANCH: $GITHUB_BRANCH"
fi

echo "Using GitHub branch: $GITHUB_BRANCH"

sudo apt update
sudo apt install -y build-essential clang gcc pkg-config libclang-dev llvm-dev libssl-dev libfontconfig1-dev

sudo apt install -y make git tmux
curl https://sh.rustup.rs -sSf | sh -s -- -y
. "$HOME/.cargo/env" 
cargo --version
rustup toolchain install 1.88.0
rustup default 1.88.0

REPO_FASTEVM="https://github.com/scalarorg/fastevm.git"
REPO_BRANCH_FASTEVM=$GITHUB_BRANCH
# REPO_GRAVITY_GENESIS_CONTRACT="https://github.com/scalarorg/gravity-genesis-contract.git"
# REPO_BRANCH_GRAVITY_GENESIS_CONTRACT="main"

clone_repository() {
    REPO_URL=$1
    REPO_BRANCH=$2
    # Extract repo_name from repo_url (e.g., https://github.com/user/repo.git -> repo)
    REPO_NAME=$(basename "$REPO_URL" .git)
    if [ -d "$REPO_NAME" ]; then
        echo "$REPO_NAME directory already exists. Updating repository..."
        cd $REPO_NAME
        git config pull.rebase true
        git fetch origin

        if git show-ref --verify --quiet "refs/heads/$REPO_BRANCH"; then
            git checkout "$REPO_BRANCH"
        else
            git checkout -b "$REPO_BRANCH" "origin/$REPO_BRANCH"
        fi

        git pull origin "$REPO_BRANCH"

        echo "Updating all submodules..."
        git submodule sync --recursive
        git submodule update --init --recursive
    else
        echo "Cloning fastevm repository..."
        git clone $REPO_URL
        cd $REPO_NAME

        git checkout "$REPO_BRANCH"

        echo "Initializing and updating all submodules..."
        git submodule update --init --recursive
    fi
}

build_fastevm() {
    cargo build --release
    sudo systemctl stop fastevm-execution || true
    sudo cp target/release/fastevm-execution /usr/local/bin/fastevm-execution
    sudo cp target/release/fastevm-cli /usr/local/bin/fastevm-cli
    # cargo clean
    cd modules/mysticeti
    # cargo build --release
    sudo systemctl stop fastevm-consensus || true
    cargo build --release -p evm-consensus --bin evm-consensus
    sudo cp target/release/evm-consensus /usr/local/bin/evm-consensus
    # cargo clean
}

build_gravity_genesis_contract() {
    cd $SCRIPT_DIR/gravity-genesis-contract
    cargo build --release
    sudo cp target/release/gravity-genesis-contract /usr/local/bin/gravity-genesis-contract
}

clone_repository $REPO_FASTEVM $REPO_BRANCH_FASTEVM
build_fastevm
# clone_repository $REPO_GRAVITY_GENESIS_CONTRACT $REPO_BRANCH_GRAVITY_GENESIS_CONTRACT



