#!/bin/bash
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
    git checkout gravity
    git pull origin gravity
    echo "Updating all submodules..."
    git submodule sync --recursive
    git submodule update --init --recursive
else
    echo "Cloning fastevm repository..."
    git clone https://github.com/scalarorg/fastevm.git 
    cd fastevm
    git checkout gravity
    echo "Initializing and updating all submodules..."
    git submodule update --init --recursive
fi
cargo build --release
cd modules/mysticeti
# cargo build --release
cargo build --release -p evm-consensus --bin evm-consensus
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


