# Multi-stage build for consensus client
FROM rust:1.75 as builder

WORKDIR /app

# Copy workspace Cargo.toml first for better caching
COPY Cargo.toml ./
COPY fastevm/execution-client/Cargo.toml ./fastevm/execution-client/
COPY fastevm/consensus-client/Cargo.toml ./fastevm/consensus-client/

# Create dummy main files to cache dependencies
RUN mkdir -p fastevm/execution-client/src fastevm/consensus-client/src
RUN echo "fn main() {}" > fastevm/execution-client/src/main.rs
RUN echo "fn main() {}" > fastevm/consensus-client/src/main.rs

# Build dependencies
RUN cargo build --release --bin consensus-client

# Copy actual source code
COPY fastevm/consensus-client/src ./fastevm/consensus-client/src

# Build the actual binary
RUN touch fastevm/consensus-client/src/main.rs
RUN cargo build --release --bin consensus-client

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

# Change ownership
RUN chown -R fastevm:fastevm /app

USER fastevm

CMD ["./consensus-client"]