# Multi-stage build for execution client
FROM rust:1.75 as builder

WORKDIR /app

# Copy the entire workspace for building
COPY . .

# Build the execution client
RUN cargo build --release --bin execution-client

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
COPY --from=builder /app/target/release/execution-client /app/execution-client

# Change ownership
RUN chown -R fastevm:fastevm /app

USER fastevm

CMD ["./execution-client"]
