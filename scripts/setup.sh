#!/bin/bash
sudo apt update
sudo apt install -y build-essential clang gcc pkg-config libclang-dev llvm-dev
sudo apt install -y make git tmux
curl https://sh.rustup.rs -sSf | sh -s -- -y
. "$HOME/.cargo/env" 
cargo --version
rustup toolchain install 1.88.0
rustup default 1.88.0
git clone https://github.com/paradigmxyz/reth.git
cd reth
git checkout tags/v1.8.2
#RUSTFLAGS="-C target-cpu=native" cargo build --profile profiling --features "jemalloc-prof,asm-keccak"
cargo build --profile profiling --features "jemalloc-prof,asm-keccak"
cargo build --release -p reth-bench
cd
git clone https://github.com/Galxe/gravity-reth.git
cd gravity-reth
cargo build --release


