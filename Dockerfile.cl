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
COPY consensus-client/Cargo.toml ./consensus-client/
COPY execution-client ./execution-client
COPY reth-extension ./reth-extension

# Build all binaries from consensus-client crate
RUN cargo build --release --bin fastevm-consensus

# Build all binaries from orchestrator crate
# RUN cd orchestrator && cargo build --release --bin orchestrator --bin local-network --bin remote-network

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