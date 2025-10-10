# FastEVM Client Node Management Makefile
# Standalone client node deployment and management

.PHONY: help init plan apply destroy status connect setup-config run-test run-scan run-batch run-all update-code clean-logs clean-all

# Default target
help: ## Show available commands
	@echo "FastEVM Client Node Management"
	@echo "=============================="
	@echo ""
	@echo "Deployment Commands:"
	@echo "  init              Initialize Terraform"
	@echo "  plan              Plan deployment"
	@echo "  apply             Deploy client node"
	@echo "  destroy           Destroy client node"
	@echo "  status            Show deployment status"
	@echo ""
	@echo "Client Management:"
	@echo "  connect           Connect to client node via SSH"
	@echo "  setup-config      Setup test configuration"
	@echo "  run-test          Run specific test (usage: make run-test TEST=scan)"
	@echo "  run-scan          Run block scan test"
	@echo "  run-batch         Run batch transaction test"
	@echo "  run-all           Run all tests"
	@echo ""
	@echo "Maintenance Commands:"
	@echo "  update-code       Update code and rebuild on client node"
	@echo "  clean-logs        Clean logs on client node"
	@echo "  clean-all         Clean all local files"
	@echo ""

# Configuration
CLIENT_SSH_KEY := client-deploy-key
CLIENT_USER := ubuntu
CLIENT_PROJECT_DIR := /home/$(CLIENT_USER)/fastevm
CLIENT_CONFIG_DIR := /home/$(CLIENT_USER)/test-config

# Initialize Terraform
init: ## Initialize Terraform
	@echo "🔧 Initializing Terraform..."
	@terraform init
	@echo "✅ Terraform initialized"

# Plan deployment
plan: ## Plan client node deployment
	@echo "📋 Planning client node deployment..."
	@terraform plan
	@echo "✅ Plan completed"

# Deploy client node
apply: ## Deploy client node
	@echo "🚀 Deploying FastEVM client node..."
	@terraform apply -auto-approve
	@echo "✅ Client node deployed successfully"
	@echo ""
	@echo "📋 Next steps:"
	@echo "  1. Wait for client setup to complete (2-3 minutes)"
	@echo "  2. Run 'make setup-config' to configure test settings"
	@echo "  3. Run 'make run-scan' to test the setup"

# Destroy client node
destroy: ## Destroy client node
	@echo "⚠️  Destroying client node..."
	@read -p "Are you sure? This will delete the client node and all data. (y/N): " confirm; \
	if [ "$$confirm" = "y" ] || [ "$$confirm" = "Y" ]; then \
		terraform destroy -auto-approve; \
		echo "✅ Client node destroyed"; \
	else \
		echo "❌ Operation cancelled"; \
	fi

# Show status
status: ## Show deployment status
	@echo "📊 Client Node Status"
	@echo "===================="
	@if [ -f "terraform.tfstate" ]; then \
		CLIENT_IP=$$(terraform output -json client_node_info | jq -r '.external_ip'); \
		if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
			echo "❌ Client node not found. Run 'make apply' first."; \
		else \
			echo "Client IP: $$CLIENT_IP"; \
			echo "SSH Command: ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP"; \
			echo ""; \
			echo "Setup Status:"; \
			SETUP_COMPLETE=$$(ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'ls /var/log/client-setup-complete 2>/dev/null && echo "yes" || echo "no"'); \
			if [ "$$SETUP_COMPLETE" = "yes" ]; then \
				echo "✅ Setup completed"; \
				echo ""; \
				echo "Test Binary:"; \
				ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'ls -la /home/ubuntu/fastevm/testing/integration/target/release/fastevm-test 2>/dev/null && echo "✅ Binary ready" || echo "❌ Binary not found"'; \
				echo ""; \
				echo "Configuration:"; \
				ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'ls -la $(CLIENT_CONFIG_DIR)/test.env 2>/dev/null && echo "✅ Config ready" || echo "❌ Config not found"'; \
			else \
				echo "⏳ Setup still running..."; \
				echo "Recent setup log:"; \
				ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'tail -3 /var/log/client-setup.log 2>/dev/null || echo "No log found"'; \
			fi; \
		fi; \
	else \
		echo "❌ No Terraform state found. Run 'make apply' first."; \
	fi

# Connect to client node
connect: ## Connect to client node via SSH
	@echo "🔗 Connecting to client node..."
	@CLIENT_IP=$$(terraform output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make apply' first."; \
		exit 1; \
	fi; \
	echo "Connecting to $$CLIENT_IP..."; \
	ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP

# Setup client configuration
setup-config: ## Setup test configuration on client node
	@echo "⚙️  Setting up client configuration..."
	@CLIENT_IP=$$(terraform output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make apply' first."; \
		exit 1; \
	fi; \
	echo "Setting up configuration on $$CLIENT_IP..."; \
	SETUP_COMPLETE=$$(ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'ls /var/log/client-setup-complete 2>/dev/null && echo "yes" || echo "no"'); \
	if [ "$$SETUP_COMPLETE" = "no" ]; then \
		echo "⏳ Client setup is still running. Please wait for it to complete."; \
		echo "You can check progress with: make status"; \
		echo "Or wait a few minutes and try again."; \
		exit 1; \
	fi; \
	CONFIG_EXISTS=$$(ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'ls $(CLIENT_CONFIG_DIR)/test.env 2>/dev/null && echo "yes" || echo "no"'); \
	if [ "$$CONFIG_EXISTS" = "no" ]; then \
		echo "📝 Creating test configuration..."; \
		ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'mkdir -p $(CLIENT_CONFIG_DIR) && cp $(CLIENT_CONFIG_DIR)/test.env.example $(CLIENT_CONFIG_DIR)/test.env 2>/dev/null || echo "Configuration files will be created during setup"'; \
	fi; \
	echo "✅ Configuration ready"; \
	echo ""; \
	echo "📝 Next steps:"; \
	echo "  1. SSH to client: make connect"; \
	echo "  2. Edit config: nano $(CLIENT_CONFIG_DIR)/test.env"; \
	echo "  3. Update RPC URLs with actual node IPs from your FastEVM deployment"; \
	echo "  4. Run tests: make run-test TEST=scan"

# Run remote test
run-test: ## Run specific test on client node (usage: make run-test TEST=scan)
	@if [ -z "$(TEST)" ]; then \
		echo "❌ Please specify TEST parameter"; \
		echo "Usage: make run-test TEST=<test_type>"; \
		echo "Available tests: scan, scan-all, batch, range, all"; \
		exit 1; \
	fi
	@echo "🧪 Running $(TEST) test on client node..."
	@CLIENT_IP=$$(terraform output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make apply' first."; \
		exit 1; \
	fi; \
	ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'cd /home/$(CLIENT_USER) && make test-$(TEST)'

# Run remote scan test
run-scan: ## Run block scan test on client node
	@$(MAKE) run-test TEST=scan

# Run remote batch test
run-batch: ## Run batch transaction test on client node
	@$(MAKE) run-test TEST=batch

# Run remote all tests
run-all: ## Run all tests on client node
	@$(MAKE) run-test TEST=all

# Update client code
update-code: ## Update code and rebuild on client node
	@echo "🔄 Updating client code..."
	@CLIENT_IP=$$(terraform output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make apply' first."; \
		exit 1; \
	fi; \
	ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'cd /home/$(CLIENT_USER) && make update-build'
	@echo "✅ Client code updated successfully"

# Clean client logs
clean-logs: ## Clean logs on client node
	@echo "🧹 Cleaning client logs..."
	@CLIENT_IP=$$(terraform output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make apply' first."; \
		exit 1; \
	fi; \
	ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'make clean-logs'
	@echo "✅ Client logs cleaned"

# Clean all local files
clean-all: ## Clean all local files
	@echo "🧹 Cleaning all local files..."
	@rm -f *.tfstate *.tfstate.backup .terraform.lock.hcl
	@rm -f client-deploy-key client-deploy-key.pub
	@rm -rf .terraform/
	@echo "✅ All local files cleaned"

# Quick start (init + apply)
quick-start: init apply ## Quick start: initialize and deploy client node
	@echo "🎉 Quick start completed!"
	@echo "Run 'make setup-config' to configure test settings"

# Show outputs
outputs: ## Show all outputs
	@echo "📋 Client Node Outputs"
	@echo "======================"
	@terraform output
