# FastEVM Makefile for Refactored Packages

.PHONY: help build build-release test clean run-execution run-consensus integration-test local-network local-start local-stop local-status local-logs local-cleanup

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
	@echo "Local Network (No Docker):"
	@echo "  local-network  - Start local 4-node network (execution + consensus)"
	@echo "  local-start    - Start local network"
	@echo "  local-stop     - Stop local network"
	@echo "  local-status   - Show local network status"
	@echo "  local-logs     - Show local network logs"
	@echo "  local-cleanup  - Clean up local network data"
	@echo "  local-init     - Initialize local network data only"
	@echo "  local-test-setup - Test local development setup"
	@echo ""
	@echo "Development:"
	@echo "  check          - Check code without building"
	@echo "  fmt            - Format code with rustfmt"
	@echo "  clippy         - Run clippy linter"

# Build all packages in debug mode
build:
	@echo "🔨 Building FastEVM packages in debug mode..."
	cargo build --workspace --bins

# Build all packages in release mode
build-release:
	@echo "🚀 Building FastEVM packages in release mode..."
	cargo build --workspace --release --bins

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

# ===== LOCAL NETWORK TARGETS (No Docker) =====

# Start local network (main target)
local-network: build-release
	@echo "🚀 Starting FastEVM local network (4 execution + 4 consensus nodes)..."
	@echo "This will start the network without Docker for faster development."
	@echo ""
	@./scripts/local-network.sh start

# Start local network
local-start: build-release
	@echo "🚀 Starting local network..."
	@./scripts/local-network.sh start

# Stop local network
local-stop:
	@echo "🛑 Stopping local network..."
	@./scripts/local-network.sh stop

# Show local network status
local-status:
	@echo "📊 Local network status:"
	@./scripts/local-network.sh status

# Show local network logs
local-logs:
	@echo "📋 Local network logs:"
	@./scripts/local-network.sh logs

# Clean up local network data
local-cleanup:
	@echo "🧹 Cleaning up local network data..."
	@./scripts/local-network.sh cleanup

# Initialize local network data only
local-init: build-release
	@echo "🔧 Initializing local network data..."
	@./scripts/local-network.sh init

# Restart local network
local-restart: local-stop
	@echo "🔄 Restarting local network..."
	@sleep 2
	@./scripts/local-network.sh start

# Show logs for specific service
local-logs-execution:
	@echo "📋 Execution node logs:"
	@./scripts/local-network.sh logs execution-node1

local-logs-consensus:
	@echo "📋 Consensus node logs:"
	@./scripts/local-network.sh logs consensus-node1

# Test local network connectivity
local-test:
	@echo "🧪 Testing local network connectivity..."
	@echo "Testing execution nodes..."
	@for port in 8545 8547 8549 8555; do \
		echo -n "  Port $$port: "; \
		if curl -s http://localhost:$$port > /dev/null 2>&1; then \
			echo "✅ OK"; \
		else \
			echo "❌ Failed"; \
		fi; \
	done
	@echo "Testing engine API ports..."
	@for port in 8551 8552 8553 8554; do \
		echo -n "  Port $$port: "; \
		if curl -s http://localhost:$$port > /dev/null 2>&1; then \
			echo "✅ OK"; \
		else \
			echo "❌ Failed"; \
		fi; \
	done

# Development mode - start network and watch for changes
local-dev: local-network
	@echo "👀 Starting development mode..."
	@echo "Network is running. Use Ctrl+C to stop."
	@echo "Logs are available in .local-logs/"
	@echo "Use 'make local-status' to check status"
	@echo "Use 'make local-logs' to view logs"
	@echo ""
	@echo "Press Ctrl+C to stop the network..."
	@trap 'make local-stop' INT; while true; do sleep 1; done

# Test local setup
local-test-setup:
	@echo "🧪 Testing local development setup..."
	@./scripts/test-local-setup.sh