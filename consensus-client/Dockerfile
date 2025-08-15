# Use the official Rust image as a builder
FROM scalarorg/fastevm-builder:latest AS builder

# Set working directory
WORKDIR /app

# Copy the entire workspace
COPY Cargo.toml Cargo.lock ./
COPY execution-client/Cargo.toml ./execution-client/
# Copy the consensus-client package
COPY consensus-client ./consensus-client

# Build all binaries from consensus-client crate
RUN cargo build --release --bin fastevm-consensus

# Create a minimal runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -m -u 1000 mysticeti

# Set working directory
WORKDIR /app

# Copy all binaries from builder
COPY --from=builder /app/target/release/fastevm-consensus /usr/local/bin/

# Create data directory
RUN mkdir -p /app/data && chown -R mysticeti:mysticeti /app

# Switch to non-root user
USER mysticeti

# Set default command
ENTRYPOINT ["fastevm-consensus"]
CMD ["--help"] 