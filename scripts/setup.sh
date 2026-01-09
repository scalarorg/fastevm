#!/bin/bash
#!/bin/bash
set -euo pipefail

: "${GITHUB_BRANCH:=gravity}"

echo "Using GitHub branch: $GITHUB_BRANCH"

sudo apt update
sudo apt install -y build-essential clang gcc pkg-config libclang-dev llvm-dev libssl-dev libfontconfig1-dev

sudo apt install -y make git tmux
curl https://sh.rustup.rs -sSf | sh -s -- -y
. "$HOME/.cargo/env" 
cargo --version
rustup toolchain install 1.88.0
rustup default 1.88.0
if [ -d "fastevm" ]; then
    echo "fastevm directory already exists. Updating repository..."
    cd fastevm

    git fetch origin

    if git show-ref --verify --quiet "refs/heads/$GITHUB_BRANCH"; then
        git checkout "$GITHUB_BRANCH"
    else
        git checkout -b "$GITHUB_BRANCH" "origin/$GITHUB_BRANCH"
    fi

    git pull origin "$GITHUB_BRANCH"

    echo "Updating all submodules..."
    git submodule sync --recursive
    git submodule update --init --recursive
else
    echo "Cloning fastevm repository..."
    git clone https://github.com/scalarorg/fastevm.git
    cd fastevm

    git checkout "$GITHUB_BRANCH"

    echo "Initializing and updating all submodules..."
    git submodule update --init --recursive
fi

cargo build --release
sudo cp target/release/fastevm-execution /usr/local/bin/fastevm-execution
sudo cp target/release/fastevm-cli /usr/local/bin/fastevm-cli
cargo clean
cd modules/mysticeti
# cargo build --release
cargo build --release -p evm-consensus --bin evm-consensus
sudo cp target/release/evm-consensus /usr/local/bin/evm-consensus
cargo clean
# git clone https://github.com/paradigmxyz/reth.git
# cd reth
# git checkout tags/v1.8.2
# #RUSTFLAGS="-C target-cpu=native" cargo build --profile profiling --features "jemalloc-prof,asm-keccak"
# cargo build --profile profiling --features "jemalloc-prof,asm-keccak"
# cargo build --release -p reth-bench
# cd
# git clone https://github.com/Galxe/gravity-reth.git
# cd gravity-reth
# cargo build --release

