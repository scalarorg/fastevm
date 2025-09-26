# Use the official Rust image as a builder
FROM rust:1.88-slim AS builder
# Build profile, release by default
ARG BUILD_PROFILE=release
ENV BUILD_PROFILE=$BUILD_PROFILE
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
COPY consensus-client ./consensus-client
COPY execution-client/Cargo.toml ./execution-client/Cargo.toml
COPY reth-extension ./reth-extension
COPY testing/integration ./testing/integration
# COPY testing ./testing

# Build all binaries from consensus-client crate
RUN cargo build --profile $BUILD_PROFILE --bin fastevm-consensus
RUN cp /app/target/$BUILD_PROFILE/fastevm-consensus /app/fastevm-consensus

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
COPY --from=builder /app/fastevm-consensus /usr/local/bin/
RUN ln -sf /usr/local/bin/fastevm-consensus /usr/local/bin/mysticeti
# Create data directory
RUN mkdir -p /app/data && chown -R mysticeti:mysticeti /app

# Switch to non-root user
USER mysticeti

# Set default command
ENTRYPOINT ["mysticeti"]
CMD ["--help"] 