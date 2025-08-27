# FastEVM Makefile for Refactored Packages

.PHONY: help build build-release test clean run-execution run-consensus integration-test

# Default target
help:
	@echo "FastEVM Refactored Packages - Available Targets:"
	@echo ""
	@echo "Building:"
	@echo "  build          - Build all packages in debug mode"
	@echo "  build-release  - Build all packages in release mode"
	@echo "  clean          - Clean all build artifacts"
	@echo ""
	@echo "Testing:"
	@echo "  test           - Run all tests"
	@echo "  integration-test - Run integration tests"
	@echo ""
	@echo "Running:"
	@echo "  run-execution  - Run execution-client with transaction listener"
	@echo "  run-consensus  - Run consensus-client with transaction subscription"
	@echo ""
	@echo "Development:"
	@echo "  check          - Check code without building"
	@echo "  fmt            - Format code with rustfmt"
	@echo "  clippy         - Run clippy linter"

# Build all packages in debug mode
build:
	@echo "🔨 Building FastEVM packages in debug mode..."
	cargo build --workspace

# Build all packages in release mode
build-release:
	@echo "🚀 Building FastEVM packages in release mode..."
	cargo build --workspace --release

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	@echo "✅ Clean complete"

# Run all tests
test:
	@echo "🧪 Running FastEVM tests..."
	cargo test --workspace

# Check code without building
check:
	@echo "🔍 Checking FastEVM code..."
	cargo check --workspace

# Format code
fmt:
	@echo "✨ Formatting FastEVM code..."
	cargo fmt --all

# Run clippy linter
clippy:
	@echo "🔧 Running clippy linter..."
	cargo clippy --workspace -- -D warnings

# Run execution-client
run-execution: build
	@echo "🚀 Starting execution-client with transaction listener..."
	cd execution-client && cargo run -- node

# Run consensus-client
run-consensus: build
	@echo "🔗 Starting consensus-client with transaction subscription..."
	cd consensus-client && cargo run -- start --config config.yml

# Run integration tests
integration-test: build-release
	@echo "🧪 Running integration tests..."
	./scripts/test-integration.sh

# Install development dependencies
install-dev:
	@echo "📦 Installing development dependencies..."
	cargo install cargo-watch
	cargo install cargo-audit
	@echo "✅ Development dependencies installed"

# Watch mode for development
watch:
	@echo "👀 Starting watch mode for development..."
	cargo watch -x check -x test -x run

# Security audit
audit:
	@echo "🔒 Running security audit..."
	cargo audit

# Generate documentation
doc:
	@echo "📚 Generating documentation..."
	cargo doc --workspace --no-deps --open

# Quick development cycle
dev: fmt clippy test

# Full development cycle
full-dev: clean build test integration-test

# Docker operations
docker-build:
	@echo "🐳 Building Docker images..."
	docker-compose -f execution-client/docker-compose.yml build
	docker-compose -f consensus-client/docker-compose.yml build

docker-up:
	@echo "🚀 Starting Docker services..."
	docker-compose -f execution-client/docker-compose.yml up -d
	docker-compose -f consensus-client/docker-compose.yml up -d

docker-down:
	@echo "🛑 Stopping Docker services..."
	docker-compose -f execution-client/docker-compose.yml down
	docker-compose -f consensus-client/docker-compose.yml down

# Performance testing
bench:
	@echo "⚡ Running benchmarks..."
	cargo bench --workspace

# Coverage report
coverage:
	@echo "📊 Generating coverage report..."
	cargo tarpaulin --workspace --out Html

# Dependency updates
update-deps:
	@echo "🔄 Updating dependencies..."
	cargo update
	@echo "✅ Dependencies updated"

# Check for outdated dependencies
outdated:
	@echo "🔍 Checking for outdated dependencies..."
	cargo outdated

# Help for specific package
help-execution:
	@echo "Execution-Client specific targets:"
	@echo "  run-execution  - Run execution-client"
	@echo "  test-execution - Test execution-client only"
	@echo "  build-execution - Build execution-client only"

help-consensus:
	@echo "Consensus-Client specific targets:"
	@echo "  run-consensus  - Run consensus-client"
	@echo "  test-consensus - Test consensus-client only"
	@echo "  build-consensus - Build consensus-client only"

# Package-specific builds
build-execution:
	@echo "🔨 Building execution-client..."
	cargo build -p fastevm-execution

build-consensus:
	@echo "🔨 Building consensus-client..."
	cargo build -p fastevm-consensus

# Package-specific tests
test-execution:
	@echo "🧪 Testing execution-client..."
	cargo test -p fastevm-execution

test-consensus:
	@echo "🧪 Testing consensus-client..."
	cargo test -p fastevm-consensus

# Quick start for development
quick-start: install-dev build test
	@echo "🎉 Quick start complete! Ready for development."
	@echo "Use 'make run-execution' or 'make run-consensus' to start services."
	@echo "Use 'make watch' for continuous development mode."