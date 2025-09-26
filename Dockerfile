# Multi-stage build for consensus client
FROM rust:1.75 as builder

WORKDIR /app

# Copy workspace Cargo.toml first for better caching
COPY Cargo.toml ./
COPY execution-client/Cargo.toml ./execution-client/
COPY consensus-client/Cargo.toml ./consensus-client/
COPY reth-extension ./reth-extension

# Build dependencies
RUN cargo build --release --bin consensus-client --bin fastevm-execution

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
COPY --from=builder /app/target/release/consensus-client /app/consensus-client
COPY --from=builder /app/target/release/fastevm-execution /app/fastevm-execution

# Change ownership
RUN chown -R fastevm:fastevm /app

USER fastevm

CMD ["./consensus-client"]