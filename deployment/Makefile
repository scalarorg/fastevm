# FastEVM Deployment Makefile
# Simplified commands for deployment operations

.PHONY: help deploy deploy-legacy stop status setup test-connections create-network upload-files build-images start-services check-status upload-to-builder build-on-builder load-images-local distribute-images clean help-builder help-utils

# Default target
help:
	@echo "FastEVM Deployment - Available Commands:"
	@echo ""
	@echo "🚀 Main Deployment:"
	@echo "  deploy          - Deploy to all 4 nodes using builder approach"
	@echo "  deploy-legacy   - Deploy using legacy approach (build on each node)"
	@echo "  stop            - Stop all FastEVM services"
	@echo "  status          - Check service status on all nodes"
	@echo "  setup           - Setup deployment environment"
	@echo ""
	@echo "🔧 Individual Operations:"
	@echo "  test-connections - Test SSH connections to all nodes"
	@echo "  create-network   - Create Docker network on all nodes"
	@echo "  upload-files     - Upload files to all nodes"
	@echo "  build-images     - Build Docker images on all nodes"
	@echo "  start-services   - Start Docker services on all nodes"
	@echo "  check-status     - Check status of all nodes"
	@echo ""
	@echo "🏗️  Builder Operations:"
	@echo "  upload-to-builder   - Upload source code to builder node"
	@echo "  build-on-builder    - Build all Docker images on builder node"
	@echo "  load-images-local   - Load built images from builder to local"
	@echo "  distribute-images   - Copy built images to all nodes"
	@echo ""
	@echo "🧹 Maintenance:"
	@echo "  clean           - Clean up temporary files and images"
	@echo "  help-builder    - Show builder-specific commands"
	@echo "  help-utils      - Show utility commands"
	@echo ""
	@echo "📖 Usage Examples:"
	@echo "  make deploy                    # Full deployment"
	@echo "  make stop                      # Stop all services"
	@echo "  make status                    # Check status"
	@echo "  make test-connections          # Test SSH connections"
	@echo "  make upload-to-builder         # Upload to builder only"

# Main deployment workflow (builder approach)
deploy:
	@echo "🚀 Starting FastEVM deployment (builder approach)..."
	@task deploy

# Legacy deployment workflow (build on each node)
deploy-legacy:
	@echo "🚀 Starting FastEVM deployment (legacy approach)..."
	@task deploy-legacy

# Stop all services
stop:
	@echo "🛑 Stopping all FastEVM services..."
	@task stop

# Check status
status:
	@echo "📊 Checking status of all nodes..."
	@task status

# Setup deployment environment
setup:
	@echo "🔧 Setting up deployment environment..."
	@task setup

# Test SSH connections
test-connections:
	@echo "🔌 Testing SSH connections to all nodes..."
	@task test-connections

# Test builder connection
test-builder-connection:
	@echo "🔌 Testing SSH connection to builder node..."
	@task test-builder-connection

# Create Docker network
create-network:
	@echo "🌐 Creating Docker network on all nodes..."
	@task create-network

# Upload files to all nodes
upload-files:
	@echo "📤 Uploading files to all nodes..."
	@task upload-files

# Build Docker images on all nodes
build-images:
	@echo "🔨 Building Docker images on all nodes..."
	@task build-images

# Start Docker services
start-services:
	@echo "🚀 Starting Docker services on all nodes..."
	@task start-services

# Check status of all nodes
check-status:
	@echo "📊 Checking status of all nodes..."
	@task check-status

# Builder operations
upload-to-builder:
	@echo "📤 Uploading source code to builder node..."
	@task upload-to-builder

build-on-builder:
	@echo "🔨 Building Docker images on builder node..."
	@task build-on-builder

load-images-local:
	@echo "📥 Loading Docker images from builder to local..."
	@task load-images-local

distribute-images:
	@echo "📤 Distributing Docker images to all nodes..."
	@task distribute-images

# Clean up temporary files
clean:
	@echo "🧹 Cleaning up temporary files..."
	@rm -rf temp-images/
	@rm -rf logs/
	@echo "✅ Cleanup complete"

# Help for builder operations
help-builder:
	@echo "🏗️  Builder Operations:"
	@echo "  upload-to-builder   - Upload source code to builder node"
	@echo "  build-on-builder    - Build all Docker images on builder node"
	@echo "  load-images-local   - Load built images from builder to local"
	@echo "  distribute-images   - Copy built images to all nodes"
	@echo "  test-builder-connection - Test SSH connection to builder"
	@echo ""
	@echo "📖 Builder Workflow:"
	@echo "  1. make upload-to-builder"
	@echo "  2. make build-on-builder"
	@echo "  3. make load-images-local"
	@echo "  4. make distribute-images"
	@echo "  5. make start-services"

# Help for utility operations
help-utils:
	@echo "🔧 Utility Operations:"
	@echo "  test-connections - Test SSH connections to all nodes"
	@echo "  create-network   - Create Docker network on all nodes"
	@echo "  upload-files     - Upload files to all nodes"
	@echo "  build-images     - Build Docker images on all nodes"
	@echo "  start-services   - Start Docker services on all nodes"
	@echo "  check-status     - Check status of all nodes"
	@echo "  clean            - Clean up temporary files"
	@echo ""
	@echo "📖 Individual Workflow:"
	@echo "  1. make test-connections"
	@echo "  2. make create-network"
	@echo "  3. make upload-files"
	@echo "  4. make build-images"
	@echo "  5. make start-services"
	@echo "  6. make check-status"

# Quick deployment (assumes setup is done)
quick-deploy: test-connections create-network upload-files build-images start-services check-status
	@echo "🎉 Quick deployment completed!"

# Quick builder deployment
quick-builder-deploy: test-connections test-builder-connection create-network upload-to-builder build-on-builder load-images-local distribute-images start-services check-status
	@echo "🎉 Quick builder deployment completed!"

# Development workflow
dev-setup: setup test-connections create-network
	@echo "🔧 Development environment setup complete!"

# Production deployment
prod-deploy: setup test-connections test-builder-connection create-network upload-to-builder build-on-builder load-images-local distribute-images start-services check-status
	@echo "🚀 Production deployment completed!"

# Emergency stop
emergency-stop:
	@echo "🚨 Emergency stop - stopping all services..."
	@task stop

# Show configuration
show-config:
	@echo "📋 Current configuration:"
	@echo "Config file: config/nodes.env"
	@if [ -f config/nodes.env ]; then \
		echo "✅ Configuration file exists"; \
		cat config/nodes.env; \
	else \
		echo "❌ Configuration file not found"; \
		echo "Run 'make setup' first or copy config/nodes.env.example to config/nodes.env"; \
	fi

# Validate configuration
validate-config:
	@echo "🔍 Validating configuration..."
	@if [ ! -f config/nodes.env ]; then \
		echo "❌ Configuration file not found"; \
		exit 1; \
	fi
	@echo "✅ Configuration file found"
	@echo "🔍 Checking required variables..."
	@source config/nodes.env && \
		[ -n "$$NODE1_HOST" ] && echo "✅ NODE1_HOST set" || echo "❌ NODE1_HOST missing" && \
		[ -n "$$NODE2_HOST" ] && echo "✅ NODE2_HOST set" || echo "❌ NODE2_HOST missing" && \
		[ -n "$$NODE3_HOST" ] && echo "✅ NODE3_HOST set" || echo "❌ NODE3_HOST missing" && \
		[ -n "$$NODE4_HOST" ] && echo "✅ NODE4_HOST set" || echo "❌ NODE4_HOST missing" && \
		[ -n "$$BUILDER_HOST" ] && echo "✅ BUILDER_HOST set" || echo "❌ BUILDER_HOST missing"

# Show logs
logs:
	@echo "📋 Showing service logs..."
	@task logs 2>/dev/null || echo "No logs available. Run 'make status' to check service status."

# Restart services
restart: stop start-services
	@echo "🔄 Services restarted!"

# Full cleanup and redeploy
redeploy: clean stop deploy
	@echo "🔄 Full redeploy completed!"

# Show this help
all-help: help help-builder help-utils
