# Multi-stage build for consensus client
FROM rust:1.88 AS builder

ARG BUILD_PROFILE=release
ENV BUILD_PROFILE=$BUILD_PROFILE

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
COPY testing/integration ./testing/integration
# COPY testing ./testing

# Build execution client
RUN cargo build --profile $BUILD_PROFILE --bin fastevm-execution --bin cli
RUN cp /app/target/$BUILD_PROFILE/fastevm-execution /app/fastevm-execution
RUN cp /app/target/$BUILD_PROFILE/cli /app/cli

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
COPY --from=builder /app/fastevm-execution /usr/local/bin/fastevm-execution
COPY --from=builder /app/cli /usr/local/bin/cli
RUN ln -sf /usr/local/bin/fastevm-execution /usr/local/bin/reth

# Change ownership
RUN chown -R fastevm:fastevm /app

USER fastevm

ENTRYPOINT ["reth"]