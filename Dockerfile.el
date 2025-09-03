# Multi-stage build for consensus client
FROM rust:1.88 AS builder

# Install build dependencies including libclang
RUN apt-get update && apt-get install -y \
    clang \
    libclang-dev \
    libssl-dev \
    pkg-config \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace Cargo.toml first for better caching
COPY Cargo.toml ./
COPY consensus-client/Cargo.toml ./consensus-client/
COPY execution-client ./execution-client
COPY reth-extension ./reth-extension

# Build execution client
RUN cargo build --release --bin fastevm-execution

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1001 fastevm

WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/fastevm-execution /usr/local/bin/fastevm-execution

# Change ownership
RUN chown -R fastevm:fastevm /app

USER fastevm

ENTRYPOINT ["fastevm-execution"]