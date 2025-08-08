# FastEVM - Engine API Bridge Makefile

.PHONY: build test clean docker-build docker-up docker-down integration-test benchmark lint format check-all

# Default target
all: check-all

# Build all targets
build:
	@echo "Building all crates..."
	cargo build --release

# Run all tests
test:
	@echo "Running unit tests..."
	cargo test --workspace
	@echo "Running integration tests..."
	cargo test --test integration_tests

# Run benchmarks
benchmark:
	@echo "Running benchmarks..."
	cargo bench

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	docker-compose down -v
	docker system prune -f

# Docker commands
docker-build:
	@echo "Building Docker images..."
	docker-compose build

docker-up:
	@echo "Starting all services..."
	docker-compose up -d

docker-down:
	@echo "Stopping all services..."
	docker-compose down

docker-logs:
	@echo "Showing logs from all services..."
	docker-compose logs -f

# Development commands
dev-execution:
    @echo "Starting execution client in development mode..."
    cd fastevm/execution-client && cargo run -- --port 8551 --http.addr 0.0.0.0 --log-level debug

dev-consensus:
	@echo "Starting consensus client in development mode..."
	cd fastevm/consensus-client && cargo run -- --execution-url http://127.0.0.1:8551 --log-level debug

# Testing commands
integration-test: build
	@echo "Running comprehensive integration tests..."
	cargo test --test integration_tests -- --nocapture

unit-test:
	@echo "Running unit tests only..."
	cargo test --lib --workspace

# Code quality
lint:
	@echo "Running clippy linter..."
	cargo clippy --workspace --all-targets --all-features -- -D warnings

format:
	@echo "Formatting code..."
	cargo fmt --all

format-check:
	@echo "Checking code formatting..."
	cargo fmt --all -- --check

# Security audit
audit:
	@echo "Running security audit..."
	cargo audit

# Documentation
docs:
	@echo "Generating documentation..."
	cargo doc --workspace --no-deps --open

# Complete check pipeline
check-all: format-check lint test
	@echo "All checks passed!"

# Development setup
setup:
	@echo "Setting up development environment..."
	rustup component add clippy rustfmt
	cargo install cargo-audit
	@echo "Development environment ready!"

# Performance testing
perf-test:
	@echo "Running performance tests..."
	docker-compose up -d
	sleep 10
	@echo "Sending test requests..."
	for i in {1..100}; do \
		curl -X POST \
		-H "Content-Type: application/json" \
		-d '{"jsonrpc":"2.0","method":"engine_newPayloadV2","params":[{"parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","feeRecipient":"0x0000000000000000000000000000000000000000","stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","prevRandao":"0x0000000000000000000000000000000000000000000000000000000000000000","blockNumber":"0x1","gasLimit":"0x1c9c380","gasUsed":"0x5208","timestamp":"0x499602d2","extraData":"0x","baseFeePerGas":"0x3b9aca00","blockHash":"0x0000000000000000000000000000000000000000000000000000000000000000","transactions":[],"withdrawals":null}],"id":1}' \
		http://localhost:8551 > /dev/null 2>&1 && echo "Request $$i completed" || echo "Request $$i failed"; \
	done
	docker-compose down

# Multi-node test
multi-node-test:
	@echo "Testing multi-node setup..."
	docker-compose up -d
	sleep 15
	@echo "Testing node 0..."
	curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"engine_newPayloadV2","params":[{"parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","feeRecipient":"0x0000000000000000000000000000000000000000","stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","prevRandao":"0x0000000000000000000000000000000000000000000000000000000000000000","blockNumber":"0x1","gasLimit":"0x1c9c380","gasUsed":"0x5208","timestamp":"0x499602d2","extraData":"0x","baseFeePerGas":"0x3b9aca00","blockHash":"0x0000000000000000000000000000000000000000000000000000000000000000","transactions":[],"withdrawals":null}],"id":1}' http://localhost:8551
	@echo "Testing node 1..."
	curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"engine_newPayloadV2","params":[{"parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","feeRecipient":"0x0000000000000000000000000000000000000000","stateRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x0000000000000000000000000000000000000000000000000000000000000000","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","prevRandao":"0x0000000000000000000000000000000000000000000000000000000000000000","blockNumber":"0x1","gasLimit":"0x1c9c380","gasUsed":"0x5208","timestamp":"0x499602d2","extraData":"0x","baseFeePerGas":"0x3b9aca00","blockHash":"0x0000000000000000000000000000000000000000000000000000000000000000","transactions":[],"withdrawals":null}],"id":1}' http://localhost:8552
	docker-compose down

# Help
help:
	@echo "FastEVM Engine API Bridge - Available Commands:"
	@echo ""
	@echo "Build Commands:"
	@echo "  build          - Build all crates"
	@echo "  clean          - Clean build artifacts and Docker containers"
	@echo ""
	@echo "Testing Commands:"
	@echo "  test           - Run all tests"
	@echo "  unit-test      - Run unit tests only"
	@echo "  integration-test - Run integration tests"
	@echo "  benchmark      - Run performance benchmarks"
	@echo "  perf-test      - Run performance test against running containers"
	@echo "  multi-node-test - Test multi-node setup"
	@echo ""
	@echo "Docker Commands:"
	@echo "  docker-build   - Build Docker images"
	@echo "  docker-up      - Start all services"
	@echo "  docker-down    - Stop all services"
	@echo "  docker-logs    - Show logs from all services"
	@echo ""
	@echo "Development Commands:"
	@echo "  dev-execution  - Start execution client in dev mode"
	@echo "  dev-consensus  - Start consensus client in dev mode"
	@echo ""
	@echo "Code Quality:"
	@echo "  lint           - Run clippy linter"
	@echo "  format         - Format code"
	@echo "  format-check   - Check code formatting"
	@echo "  audit          - Run security audit"
	@echo "  check-all      - Run all quality checks"
	@echo ""
	@echo "Other:"
	@echo "  setup          - Setup development environment"
	@echo "  docs           - Generate documentation"
	@echo "  help           - Show this help message"