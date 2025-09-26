# Multi-stage build for execution client
FROM rust:1.75 as builder

WORKDIR /app

# Copy workspace Cargo.toml first for better caching
COPY Cargo.toml ./
COPY Cargo.lock ./

# Create dummy main files to cache dependencies
RUN mkdir -p src
RUN echo "fn main() {}" > src/main.rs
RUN echo "fn main() {}" > src/cli.rs

# Build dependencies
RUN cargo build --release --bin fastevm-execution --bin fastevm-cli

# Copy actual source code
COPY src ./src

# Build the actual binaries
RUN touch src/main.rs src/cli.rs
RUN cargo build --release --bin fastevm-execution --bin fastevm-cli

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

# Copy binaries from builder stage
COPY --from=builder /app/target/release/fastevm-execution /app/fastevm-execution
COPY --from=builder /app/target/release/fastevm-cli /app/fastevm-cli

# Change ownership
RUN chown -R fastevm:fastevm /app

USER fastevm

# Default to running the execution client
CMD ["./fastevm-execution"]
