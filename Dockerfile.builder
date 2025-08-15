# Use the official Rust image as a builder
FROM rust:1.88-slim AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libclang-dev \
    clang \
    make \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy the entire workspace
COPY Cargo.toml Cargo.lock ./
# Copy the execution-client package
COPY execution-client ./execution-client
# Copy the consensus-client package
COPY consensus-client ./consensus-client

# Build all binaries
RUN cargo build --release
