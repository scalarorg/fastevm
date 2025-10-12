# FastEVM Client Node Management Makefile
# Standalone client node deployment and management

.PHONY: help init plan apply destroy status connect setup-config run-test run-scan run-batch run-all update-code clean-logs clean-all bootstrap quick-start

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
	@echo "Configuration Commands:"
	@echo "  prepare-configs   Prepare configurations locally"
	@echo "  deploy-configs   Deploy configurations to remote client"
	@echo "  setup            Run setup script on remote client (bootstrap + configure)"
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
	@echo "  quick-start       Quick start: complete client node setup"
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
		CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
		if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
			echo "❌ Client node not found. Run 'make apply' first."; \
		else \
			echo "Client IP: $$CLIENT_IP"; \
			echo "SSH Command: ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP"; \
			echo ""; \
			echo "Setup Status:"; \
			ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
			ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
			SETUP_COMPLETE=$$(ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'ls /var/log/client-bootstrap-complete 2>/dev/null && echo "yes" || echo "no"'); \
			if [ "$$SETUP_COMPLETE" = "yes" ]; then \
				echo "✅ Setup completed"; \
				echo ""; \
				echo "Test Binary:"; \
				ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'ls -la /usr/local/bin/fastevm-test 2>/dev/null && echo "✅ Binary ready" || echo "❌ Binary not found"'; \
				echo ""; \
				echo "Configuration:"; \
				ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'ls -la $(CLIENT_CONFIG_DIR)/test.env 2>/dev/null && echo "✅ Config ready" || echo "❌ Config not found"'; \
			else \
				echo "⏳ Setup still running..."; \
				echo "Recent setup log:"; \
				ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'tail -3 /var/log/client-bootstrap.log 2>/dev/null || echo "No log found"'; \
			fi; \
		fi; \
	else \
		echo "❌ No Terraform state found. Run 'make apply' first."; \
	fi

# Terraform command detection
TERRAFORM_CMD := $(shell which terraform 2>/dev/null || echo "/opt/homebrew/bin/terraform")
ifeq ($(wildcard $(TERRAFORM_CMD)),)
TERRAFORM_CMD := $(shell which terraform 2>/dev/null || echo "/usr/local/bin/terraform")
endif
ifeq ($(wildcard $(TERRAFORM_CMD)),)
TERRAFORM_CMD := terraform
endif

# Common variables and macros
CLIENT_IP_CMD = $(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'
SSH_OPTS = -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR
SSH_CMD = ssh $(SSH_OPTS) -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@
TEST_SLEEP_DURATION = 120

# Get client IP and validate it exists
define GET_CLIENT_IP
	CLIENT_IP=$$($(CLIENT_IP_CMD)); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make apply' first."; \
		exit 1; \
	fi
endef

# Get current block number from Ethereum node
define GET_CURRENT_BLOCK
	echo "🔍 Getting current block number..."; \
	CURRENT_BLOCK=$$($(SSH_CMD)$$CLIENT_IP 'cd /home/ubuntu && source ~/client-config/test.env && curl -s -X POST -H "Content-Type: application/json" --data "{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}" $$RPC_URL1 | jq -r ".result" | xargs printf "%d\n" 2>/dev/null || echo "0"'); \
	echo "📊 Current block number: $$CURRENT_BLOCK"
endef

# Execute test command on client with timeout and error handling
define RUN_TEST
	echo "$(1)"; \
	$(SSH_CMD)$$CLIENT_IP 'cd /home/ubuntu && export ENV_FILE=~/client-config/test.env && timeout $(2) $(3)'; \
	if [ $$? -eq 0 ]; then \
		echo "✅ $(4)"; \
	else \
		echo "$(5)"; \
	fi
endef

# Execute scan test with BLOCK_NUMBER environment variable
define RUN_SCAN_WITH_BLOCK
	echo "$(1)"; \
	$(SSH_CMD)$$CLIENT_IP 'cd /home/ubuntu && source ~/client-config/test.env && export BLOCK_NUMBER=$(2) && timeout $(3) $(4)'; \
	if [ $$? -eq 0 ]; then \
		echo "✅ $(5)"; \
	else \
		echo "$(6)"; \
	fi
endef

# SSH connection helper - handles host key management
define SSH_CONNECT
	$(GET_CLIENT_IP); \
	echo "🔑 Managing SSH host keys for $$CLIENT_IP..."; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	$(SSH_CMD)$$CLIENT_IP $(1)
endef

# Connect to client node
connect: ## Connect to client node via SSH
	@echo "🔗 Connecting to client node..."
	$(call SSH_CONNECT)

# Setup client configuration
setup-config: ## Setup test configuration on client node
	@echo "⚙️  Setting up client configuration..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make apply' first."; \
		exit 1; \
	fi; \
	echo "Setting up configuration on $$CLIENT_IP..."; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	SETUP_COMPLETE=$$(ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'ls /var/log/client-bootstrap-complete 2>/dev/null && echo "yes" || echo "no"'); \
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
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make apply' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
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
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make apply' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'cd /home/$(CLIENT_USER) && make update-build'
	@echo "✅ Client code updated successfully"

# Clean client logs
clean-logs: ## Clean logs on client node
	@echo "🧹 Cleaning client logs..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make apply' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'make clean-logs'
	@echo "✅ Client logs cleaned"

# Clean all local files
clean-all: ## Clean all local files
	@echo "🧹 Cleaning all local files..."
	@rm -f *.tfstate *.tfstate.backup .terraform.lock.hcl
	@rm -f client-deploy-key client-deploy-key.pub
	@rm -rf .terraform/
	@echo "✅ All local files cleaned"

# Prepare client configurations locally
prepare-configs: ## Prepare client node configurations locally (auto-detects from main deployment)
	@echo "🔧 Preparing client node configurations locally..."
	@GITHUB_REPO="$(GITHUB_REPO)" GITHUB_BRANCH="$(GITHUB_BRANCH)" ./scripts/prepare-configs.sh
	@echo "✅ Client configurations prepared successfully!"

# Deploy configurations to remote client node
deploy-configs: ## Deploy prepared configurations to remote client node
	@echo "📦 Deploying configurations to remote client node..."
	@if [ ! -d "config" ]; then echo "No config directory found. Run 'make prepare-configs' first."; exit 1; fi
	@if [ ! -f "client-deploy-key" ]; then echo "SSH key not found. Run 'make apply' first."; exit 1; fi
	@./scripts/deploy-configs.sh
	@echo "✅ Configurations deployed successfully!"

# Run setup script on remote client (bootstrap + configure)
setup: ## Run setup script on remote client node (bootstrap + configure)
	@echo "🚀 Running setup script on client node..."
	@$(GET_CLIENT_IP); \
	echo "Executing setup script on $$CLIENT_IP..."; \
	$(SSH_CMD)$$CLIENT_IP '/bin/chmod +x ~/client-config/setup.sh && sudo bash ~/client-config/setup.sh'; \
	if [ $$? -eq 0 ]; then \
		echo "✅ Setup script executed successfully"; \
		echo "⏳ Waiting for setup to complete..."; \
	else \
		echo "❌ Setup script execution failed"; \
		exit 1; \
	fi

# Automated test sequence (run after bootstrap)
auto-test: ## Run automated test sequence (get block -> batch -> sleep -> scan)
	echo "🚀 Running automated test sequence..."; \
	$(GET_CLIENT_IP); \
	echo "Running tests on $$CLIENT_IP..."; \
	$(call GET_CURRENT_BLOCK); \
	$(call RUN_TEST,"2️⃣ Running batch transaction test...",300,"fastevm-test batch","Batch transaction test completed","❌ Batch transaction test failed"); \
	echo "⏳ Waiting $(TEST_SLEEP_DURATION) seconds for transactions to be processed..."; \
	sleep $(TEST_SLEEP_DURATION); \
	$(call RUN_SCAN_WITH_BLOCK,"3️⃣ Running final block scan test...",$$CURRENT_BLOCK,60,"fastevm-test scan","Final block scan test completed","⚠️ Final block scan test had issues"); \
	echo "🎉 Automated test sequence completed!"
# Quick start (init + apply + prepare-configs + deploy-configs + setup)
quick-start: init apply prepare-configs deploy-configs setup auto-test ## Quick start: complete client node setup
	@echo "🎉 Quick start completed!"

# Show outputs
outputs: ## Show all outputs
	@echo "📋 Client Node Outputs"
	@echo "======================"
	@$(TERRAFORM_CMD) output

# Test targets (run on client node)
test-scan: ## Run block scan test
	@echo "🔍 Running block scan test..."
	@if [ -f $(CLIENT_CONFIG_DIR)/test.env ]; then \
		source $(CLIENT_CONFIG_DIR)/test.env && fastevm-test scan; \
	else \
		echo "❌ Configuration file not found. Run 'make setup-config' first."; \
		exit 1; \
	fi

test-scan-all: ## Run block scan all test
	@echo "🔍 Running block scan all test..."
	@if [ -f $(CLIENT_CONFIG_DIR)/test.env ]; then \
		source $(CLIENT_CONFIG_DIR)/test.env && fastevm-test scan-all; \
	else \
		echo "❌ Configuration file not found. Run 'make setup-config' first."; \
		exit 1; \
	fi

test-batch: ## Run batch transaction test
	@echo "📦 Running batch transaction test..."
	@if [ -f $(CLIENT_CONFIG_DIR)/test.env ]; then \
		source $(CLIENT_CONFIG_DIR)/test.env && fastevm-test batch; \
	else \
		echo "❌ Configuration file not found. Run 'make setup-config' first."; \
		exit 1; \
	fi

test-range: ## Run block range test
	@echo "📊 Running block range test..."
	@if [ -f $(CLIENT_CONFIG_DIR)/test.env ]; then \
		source $(CLIENT_CONFIG_DIR)/test.env && fastevm-test range; \
	else \
		echo "❌ Configuration file not found. Run 'make setup-config' first."; \
		exit 1; \
	fi

test-all: test-scan test-batch ## Run all tests
	@echo "✅ All tests completed"
