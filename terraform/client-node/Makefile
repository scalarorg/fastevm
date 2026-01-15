# FastEVM Client Node Management Makefile
# Simplified workflow: Deploy client node -> Setup -> Deploy network

.PHONY: help deploy setup-client deploy-network transfer-creds status connect destroy clean

# Default target
help: ## Show available commands
	@echo "FastEVM Client Node Management"
	@echo "=============================="
	@echo ""
	@echo "Main Commands:"
	@echo "  deploy           Deploy client node and setup (Step 1)"
	@echo "  deploy-network   Deploy network from client node (Step 2)"
	@echo "  transfer-creds   Transfer gcloud credentials to client node"
	@echo "  status           Show deployment status"
	@echo "  connect          Connect to client node via SSH"
	@echo "  destroy          Destroy client node"
	@echo "  clean            Clean local files"
	@echo ""

# Configuration
CLIENT_SSH_KEY := client-deploy-key
CLIENT_USER := ubuntu
CLIENT_SETUP_SCRIPT := scripts/setup.sh
NETWORK_TERRAFORM_DIR := ../network

# Use terraform.tfvars if it exists (Terraform auto-loads it, but we can be explicit)
# Otherwise fall back to environment variables
TF_VARS = $(if $(wildcard terraform.tfvars),-var-file=terraform.tfvars,\
          -var="project_id=$(PROJECT_ID)" -var="region=$(REGION)" -var="zone=$(ZONE)" \
          -var="project_name=$(PROJECT_NAME)" \
          -var="github_repo=$(GITHUB_REPO)" -var="github_branch=$(GITHUB_BRANCH)"\
          )

# Initialize Terraform
init: ## Initialize Terraform
	@terraform init

validate: ## Validate Terraform configuration
	@terraform validate

format: ## Format Terraform files
	@terraform fmt -recursive

plan: init validate format ## Plan client node deployment
	@terraform plan $(TF_VARS) -out=tfplan

# Step 1: Deploy client node, copy setup.sh and execute it
deploy: plan ## Deploy client node and run initial setup
	@echo "🚀 Step 1: Deploying FastEVM client node..."
	@terraform apply -auto-approve
	@echo "✅ Client node deployed"
	@echo ""
	@echo "📦 Copying setup script to client node..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_SETUP_SCRIPT) $(CLIENT_USER)@$$CLIENT_IP:~/setup.sh || { \
		echo "❌ Failed to copy setup.sh"; exit 1; \
	}; \
	echo "🔧 Executing setup script on client node..."; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP "FASTEVM_BRANCH=$${FASTEVM_BRANCH:-main} bash ~/setup.sh"; \
	echo "✅ Step 1 completed: Client node is ready"; \
	echo "📝 Setup script is available at: ~/setup.sh"
	@echo ""
	@echo "📋 Next step: Run 'make deploy-network' to deploy the network from this client node"

setup: deploy
	@echo "🔧 Executing setup script on client node..."; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP "FASTEVM_BRANCH=$${FASTEVM_BRANCH:-main} bash ~/setup.sh"; \
	echo "✅ Step 1 completed: Client node is ready"; \
	@echo "📋 Next step: Run 'make deploy-network' to deploy the network from this client node"

# Transfer gcloud credentials to client node
transfer-creds: deploy ## Transfer gcloud credentials to client node
	@echo "🔐 Transferring gcloud credentials to client node..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	GCLOUD_CREDS=$$HOME/.config/gcloud/application_default_credentials.json; \
	if [ -f "$$GCLOUD_CREDS" ]; then \
		ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
			-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP "mkdir -p ~/.config/gcloud"; \
		scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
			-i $(CLIENT_SSH_KEY) $$GCLOUD_CREDS $(CLIENT_USER)@$$CLIENT_IP:~/.config/gcloud/application_default_credentials.json || { \
			echo "❌ Failed to copy gcloud credentials"; exit 1; \
		}; \
		echo "✅ gcloud credentials transferred"; \
		PROJECT_ID=$$(gcloud config get-value project 2>/dev/null || echo ""); \
		if [ -n "$$PROJECT_ID" ]; then \
			ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
				-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP "gcloud config set project $$PROJECT_ID" || true; \
			echo "✅ gcloud project set to $$PROJECT_ID"; \
		fi; \
	else \
		echo "❌ gcloud credentials not found at $$GCLOUD_CREDS"; \
		echo "   Please run 'gcloud auth application-default login' first."; \
		exit 1; \
	fi

# Step 2: Deploy network from client node (after source code is cloned to /opt/fastevm)
deploy-network: transfer-creds ## Deploy network from client node (Step 2)
	@echo "🚀 Step 2: Deploying network from client node..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	echo "📡 Connecting to client node at $$CLIENT_IP..."; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	echo "🔧 Executing network deployment from /opt/fastevm/terraform/network..."; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP \
		'cd /opt/workspace/fastevm/terraform/network && make deploy'; \
	echo "✅ Step 2 completed: Network deployed from client node"

# Destroy client node
destroy: ## Destroy client node
	@echo "⚠️  Destroying client node..."
	@read -p "Are you sure? This will delete the client node and all data. (y/N): " confirm; \
	if [ "$$confirm" = "y" ] || [ "$$confirm" = "Y" ]; then \
		terraform destroy -auto-approve -refresh=false; \
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
			echo "❌ Client node not found. Run 'make deploy' first."; \
		else \
			echo "Client IP: $$CLIENT_IP"; \
			echo "SSH Command: ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP"; \
			echo ""; \
			echo "Setup Status:"; \
			ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
			ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
			SOURCE_CLONED=$$(ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'test -d /opt/fastevm && echo "yes" || echo "no"'); \
			if [ "$$SOURCE_CLONED" = "yes" ]; then \
				echo "✅ Source code cloned to /opt/fastevm"; \
				NETWORK_READY=$$(ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'test -d /opt/fastevm/terraform/network && echo "yes" || echo "no"'); \
				if [ "$$NETWORK_READY" = "yes" ]; then \
					echo "✅ Network terraform directory ready"; \
				else \
					echo "⏳ Network terraform directory not found"; \
				fi; \
			else \
				echo "⏳ Source code not yet cloned to /opt/fastevm"; \
			fi; \
		fi; \
	else \
		echo "❌ No Terraform state found. Run 'make deploy' first."; \
	fi

# Terraform command detection - use PATH to find terraform
TERRAFORM_CMD := $(shell which terraform 2>/dev/null || echo "terraform")


# Connect to client node
connect: ## Connect to client node via SSH
	@echo "🔗 Connecting to client node..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP


# Clean local files
clean: ## Clean local files
	@echo "🧹 Cleaning local files..."
	@rm -f *.tfstate *.tfstate.backup .terraform.lock.hcl
	@rm -f client-deploy-key client-deploy-key.pub
	@rm -rf .terraform/
	@echo "✅ Local files cleaned"

# Show outputs
outputs: ## Show all outputs
	@echo "📋 Client Node Outputs"
	@echo "======================"
	@$(TERRAFORM_CMD) output

copy-files:
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_SETUP_SCRIPT) $(CLIENT_USER)@$$CLIENT_IP:~/setup.sh || { \
		echo "❌ Failed to copy setup.sh"; exit 1; \
	}; \
	echo "🔧 Executing setup script on client node..."; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP "FASTEVM_BRANCH=$${FASTEVM_BRANCH:-main} bash ~/setup.sh"; \
	echo "✅ Step 1 completed: Client node is ready"; \
	echo "📝 Setup script is available at: ~/setup.sh"
	@echo ""
	@echo "📋 Next step: Run 'make deploy-network' to deploy the network from this client node"
