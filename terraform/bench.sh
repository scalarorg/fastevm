#!/bin/bash
sudo apt update
sudo apt install -y build-essential gcc make git tmux
curl https://sh.rustup.rs -sSf | sh
. "$HOME/.cargo/env" 
cargo --version
git clone https://github.com/paradigmxyz/reth.git
cd reth
tmux
# RUSTFLAGS="-C target-cpu=native" cargo build --profile profiling --features "jemalloc-prof,asm-keccak"
cargo build --profile profiling --features "jemalloc-prof,asm-keccak"

git clone https://github.com/Galxe/gravity-reth.git
cd gravity-reth
cargo build --release

