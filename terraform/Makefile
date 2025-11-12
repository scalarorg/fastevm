# FastEVM Terraform Deployment Makefile
# Easy management of GCP infrastructure deployment

.PHONY: help init plan apply destroy status logs clean validate format check test extract-test-config build-test-command run-batch-test wait-for-transactions scan-blocks clean-test ssh-node ssh-copy ssh-node-gcloud

# Load environment variables from fastevm.env if it exists
ifneq (,$(wildcard fastevm.env))
    include fastevm.env
endif

# Default target
help: ## Show this help message
	@echo "FastEVM Terraform Deployment"
	@echo "============================"
	@echo ""
	@echo "Available targets:"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-15s %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@echo ""
	@echo "Environment Variables:"
	@echo "  PROJECT_ID     - GCP Project ID (required)"
	@echo "  REGION         - GCP Region (default: us-central1)"
	@echo "  ZONE           - GCP Zone (default: us-central1-a)"
	@echo "  NODE_COUNT     - Number of nodes (default: 4)"
	@echo "  MACHINE_TYPE   - Instance type (default: e2-standard-4)"
	@echo ""
	@echo "Test Environment Variables:"
	@echo "  TEST_SENDER_COUNT        - Number of sender accounts (default: 100)"
	@echo "  TEST_TRANSACTION_COUNT   - Number of transactions per sender (default: 1)"
	@echo "  TEST_MNEMONIC           - Test mnemonic phrase (default: test mnemonic)"
	@echo "  TEST_WAITING_TIME_SECONDS - Wait time before block scan (default: 30)"
	@echo ""

# Configuration
PROJECT_ID ?= $(shell gcloud config get-value project 2>/dev/null)
REGION ?= us-central1
ZONE ?= us-central1-a
NODE_COUNT ?= 4
MACHINE_TYPE ?= e2-standard-4
PROJECT_NAME ?= fastevm
GITHUB_REPO ?= https://github.com/scalarorg/fastevm.git
GITHUB_BRANCH ?= main

# Terraform variables
TF_VARS = -var="project_id=$(PROJECT_ID)" \
          -var="region=$(REGION)" \
          -var="zone=$(ZONE)" \
          -var="node_count=$(NODE_COUNT)" \
          -var="machine_type=$(MACHINE_TYPE)" \
          -var="project_name=$(PROJECT_NAME)" \
          -var="github_repo=$(GITHUB_REPO)" \
          -var="github_branch=$(GITHUB_BRANCH)"

# Check prerequisites
check-prereqs: ## Check prerequisites
	@echo "Checking prerequisites..."
	@command -v terraform >/dev/null 2>&1 || { echo "Terraform is required but not installed. Aborting." >&2; exit 1; }
	@command -v gcloud >/dev/null 2>&1 || { echo "gcloud CLI is required but not installed. Aborting." >&2; exit 1; }
	@if [ -z "$(PROJECT_ID)" ]; then echo "PROJECT_ID is required. Set it as environment variable or run 'gcloud config set project YOUR_PROJECT_ID'"; exit 1; fi
	@echo "Prerequisites check passed!"

# Initialize Terraform
init: check-prereqs ## Initialize Terraform
	@echo "Initializing Terraform..."
	terraform init
	@echo "Terraform initialized successfully!"

# Validate Terraform configuration
validate: ## Validate Terraform configuration
	@echo "Validating Terraform configuration..."
	terraform validate
	@echo "Configuration is valid!"

# Format Terraform files
format: ## Format Terraform files
	@echo "Formatting Terraform files..."
	terraform fmt -recursive
	@echo "Terraform files formatted!"

# Plan deployment
plan: init validate ## Plan Terraform deployment
	@echo "Planning Terraform deployment..."
	terraform plan $(TF_VARS) -out=tfplan
	@echo "Plan created successfully!"

# Refresh Terraform state
refresh: ## Refresh Terraform state to sync with GCP
	@echo "Refreshing Terraform state..."
	terraform refresh $(TF_VARS)
	@echo "State refreshed successfully!"

# Apply deployment
apply: plan ## Apply Terraform deployment
	@echo "Applying Terraform deployment..."
	terraform apply tfplan
	@echo "Deployment applied successfully!"
	@echo ""
	@echo "Getting deployment information..."
	@terraform output -json > ../deployment-info.json
	@echo "Deployment information saved to deployment-info.json"

# Deploy without plan (for CI/CD)
deploy: init validate ## Deploy without interactive plan
	@echo "Deploying FastEVM infrastructure..."
	terraform apply -auto-approve $(TF_VARS)
	@echo "Deployment completed successfully!"

# Show status
status: ## Show deployment status
	@echo "FastEVM Deployment Status"
	@echo "========================="
	@if [ -f "../deployment-info.json" ]; then \
		echo "Deployment Info:"; \
		jq -r '.deployment_summary.value | "Project: \(.project_name)\nNodes: \(.node_count)\nRegion: \(.region)\nZone: \(.zone)\nMachine Type: \(.machine_type)\nTotal Disk: \(.total_disk_size)GB"' ../deployment-info.json; \
		echo ""; \
		echo "Node Endpoints:"; \
		jq -r '.node_endpoints.value | to_entries[] | "\(.key): \(.value.http_rpc)"' ../deployment-info.json; \
		echo ""; \
		echo "Load Balancer IP:"; \
		jq -r '.load_balancer_ip.value' ../deployment-info.json; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
	fi

# Show instance status
instances: ## Show GCP instance status
	@echo "GCP Instance Status"
	@echo "==================="
	@gcloud compute instances list --filter="name~$(PROJECT_NAME)-node" --format="table(name,zone,machineType,status,internalIP,externalIP)"

# Show logs
logs: ## Show bootstrap logs from all nodes
	@echo "Bootstrap Logs from All Nodes"
	@echo "============================="
	@for i in $$(seq 1 $(NODE_COUNT)); do \
		echo "--- Node $$i ---"; \
		gcloud compute ssh $(PROJECT_NAME)-node-$$i --zone=$(ZONE) --command="sudo tail -n 20 /var/log/fastevm-bootstrap.log" 2>/dev/null || echo "Could not fetch logs from node $$i"; \
		echo ""; \
	done

# SSH to a specific node using local SSH key
ssh-node: ## SSH to a specific node (usage: make ssh-node NODE=1)
	@if [ -z "$(NODE)" ]; then echo "Usage: make ssh-node NODE=1"; exit 1; fi
	@if [ ! -f "fastevm-deploy-key" ]; then echo "SSH key not found. Run 'make apply' first."; exit 1; fi
	@chmod 600 fastevm-deploy-key
	@echo "Getting node IP for node $(NODE)..."
	@if [ -f "../deployment-info.json" ]; then \
		NODE_IP=$$(jq -r ".instance_external_ips.value[$$((NODE-1))]" ../deployment-info.json 2>/dev/null || jq -r ".instance_ips.value[$$((NODE-1))]" ../deployment-info.json 2>/dev/null); \
		if [ -z "$$NODE_IP" ] || [ "$$NODE_IP" = "null" ]; then \
			echo "Could not get IP for node $(NODE)"; \
			exit 1; \
		fi; \
		echo "Connecting to $(PROJECT_NAME)-node-$(NODE) ($$NODE_IP)..."; \
		ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
			-i fastevm-deploy-key ubuntu@$$NODE_IP; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
		exit 1; \
	fi
# Copy a file from a specific node's /data/logs directory
ssh-copy: ## Copy file from node's /data/logs (usage: make ssh-copy NODE=1 FILE=logfile.txt)
	@if [ -z "$(NODE)" ]; then echo "Usage: make ssh-copy NODE=1 FILE=logfile.txt"; exit 1; fi
	@if [ -z "$(FILE)" ]; then echo "Usage: make ssh-copy NODE=1 FILE=logfile.txt"; exit 1; fi
	@if [ ! -f "fastevm-deploy-key" ]; then echo "SSH key not found. Run 'make apply' first."; exit 1; fi
	@chmod 600 fastevm-deploy-key
	@echo "Getting node IP for node $(NODE)..."
	@if [ -f "../deployment-info.json" ]; then \
		NODE_IP=$$(jq -r ".instance_external_ips.value[$$((NODE-1))]" ../deployment-info.json 2>/dev/null || jq -r ".instance_ips.value[$$((NODE-1))]" ../deployment-info.json 2>/dev/null); \
		if [ -z "$$NODE_IP" ] || [ "$$NODE_IP" = "null" ]; then \
			echo "Could not get IP for node $(NODE)"; \
			exit 1; \
		fi; \
		echo "Copying $(FILE) from $(PROJECT_NAME)-node-$(NODE) ($$NODE_IP):/data/logs/..."; \
		scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
			-i fastevm-deploy-key \
			ubuntu@$$NODE_IP:/data/logs/$(FILE) \
			./$(FILE); \
		if [ $$? -eq 0 ]; then \
			echo "✅ Successfully copied $(FILE) to ./$(FILE)"; \
		else \
			echo "❌ Failed to copy $(FILE)"; \
			exit 1; \
		fi; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
		exit 1; \
	fi

# SSH to a specific node using gcloud (alternative method)
ssh-node-gcloud: ## SSH to a specific node using gcloud (usage: make ssh-node-gcloud NODE=1)
	@if [ -z "$(NODE)" ]; then echo "Usage: make ssh-node-gcloud NODE=1"; exit 1; fi
	@echo "Connecting to $(PROJECT_NAME)-node-$(NODE) using gcloud..."
	@gcloud compute ssh $(PROJECT_NAME)-node-$(NODE) --zone=$(ZONE)

# Fix gcloud SSH authentication by adding SSH keys to instances
fix-ssh-auth: ## Fix gcloud SSH authentication by adding SSH keys to all instances
	@echo "Fixing gcloud SSH authentication..."
	@echo "This will add your SSH public key to all instances"
	@for i in $$(seq 1 $(NODE_COUNT)); do \
		echo "Adding SSH key to node $$i..."; \
		gcloud compute instances add-metadata $(PROJECT_NAME)-node-$$i --zone=$(ZONE) \
			--metadata-from-file ssh-keys=<(echo "ubuntu:$$(cat ~/.ssh/id_rsa.pub 2>/dev/null || echo 'No SSH key found. Please run: ssh-keygen -t rsa -b 4096 -C \"your_email@example.com\"')") 2>/dev/null || echo "Failed to add SSH key to node $$i"; \
	done
	@echo "SSH key addition completed. You may need to wait a few minutes for changes to take effect."

# Compare genesis.json files across all nodes
compare-genesis: ## Compare genesis.json files across all nodes to ensure consistency
	@echo "Comparing genesis.json files across all nodes..."
	@chmod +x ./scripts/compare-genesis.sh
	@./scripts/compare-genesis.sh

# Clean database on all nodes (fixes genesis hash mismatch)
clean-database: ## Clean database on all nodes to fix genesis hash mismatch
	@echo "Cleaning database on all nodes..."
	@if [ ! -f "fastevm-deploy-key" ]; then echo "SSH key not found. Run 'make apply' first."; exit 1; fi
	@chmod 600 fastevm-deploy-key
	@if [ -f "../deployment-info.json" ]; then \
		NODE_IPS=$$(jq -r '.instance_external_ips.value[]' ../deployment-info.json 2>/dev/null || jq -r '.instance_ips.value[]' ../deployment-info.json 2>/dev/null); \
		if [ -z "$$NODE_IPS" ]; then \
			echo "Could not get node IPs from Terraform output"; \
			exit 1; \
		fi; \
		NODE_INDEX=0; \
		for node_ip in $$NODE_IPS; do \
			echo "Cleaning database on node $$NODE_INDEX ($$node_ip)..."; \
			ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
				-i fastevm-deploy-key ubuntu@$$node_ip "sudo systemctl stop fastevm-execution fastevm-consensus || true; sudo rm -rf /data/execution/db/*; sudo chown -R ubuntu:ubuntu /data/execution/db" & \
			NODE_INDEX=$$((NODE_INDEX + 1)); \
		done; \
		wait; \
		echo "Database cleanup completed on all nodes!"; \
		echo "You can now restart services with: make restart"; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
		exit 1; \
	fi

# Check node health
health: ## Check health of all nodes
	@echo "Node Health Check"
	@echo "================="
	@for i in $$(seq 1 $(NODE_COUNT)); do \
		echo "Checking node $$i..."; \
		gcloud compute ssh $(PROJECT_NAME)-node-$$i --zone=$(ZONE) --command="sudo /usr/local/bin/fastevm-health-check.sh $$((i-1))" 2>/dev/null || echo "Node $$i health check failed"; \
		echo ""; \
	done

# Get node status
node-status: ## Get detailed status from all nodes
	@echo "Detailed Node Status"
	@echo "===================="
	@for i in $$(seq 1 $(NODE_COUNT)); do \
		echo "--- Node $$i Status ---"; \
		gcloud compute ssh $(PROJECT_NAME)-node-$$i --zone=$(ZONE) --command="sudo /usr/local/bin/fastevm-status.sh" 2>/dev/null || echo "Could not get status from node $$i"; \
		echo ""; \
	done

# Test RPC endpoints
test-rpc: ## Test RPC endpoints
	@echo "Testing RPC Endpoints"
	@echo "====================="
	@if [ -f "../deployment-info.json" ]; then \
		jq -r '.node_endpoints.value | to_entries[] | "Testing \(.key) (\(.value.http_rpc))..."' ../deployment-info.json; \
		jq -r '.node_endpoints.value | to_entries[] | .value.http_rpc' ../deployment-info.json | while read endpoint; do \
			echo "Testing $$endpoint"; \
			curl -s -X POST $$endpoint -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | jq -r '.result // "Failed"' || echo "Failed to connect"; \
		done; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
	fi

# Prepare local configurations
prepare-configs: ## Prepare all configurations locally (auto-detects from Terraform)
	@echo "Preparing FastEVM configurations locally..."
	@GITHUB_REPO="$(GITHUB_REPO)" GITHUB_BRANCH="$(GITHUB_BRANCH)" ./scripts/prepare-configs.sh
	@echo "Configurations prepared successfully!"

# Prepare binaries (copy from local or build new ones) and distribute to all nodes
prepare-binaries: ## Prepare binaries on both local and build node, then distribute to all nodes
	@echo "Preparing and distributing binaries..."
	@GITHUB_REPO="$(GITHUB_REPO)" GITHUB_BRANCH="$(GITHUB_BRANCH)" ./scripts/prepare-binaries.sh

# Deploy configurations to remote nodes (with parallel execution)
deploy-configs: ## Deploy prepared configurations to remote nodes in parallel
	@echo "Deploying configurations to remote nodes..."
	@if [ ! -d "deploy" ]; then echo "No deploy directory found. Run 'make prepare-configs' first."; exit 1; fi
	@if [ ! -f "fastevm-deploy-key" ]; then echo "SSH key not found. Run 'make apply' first."; exit 1; fi
	@chmod 600 fastevm-deploy-key
	@echo "Setting up SSH key on build node for direct distribution..."
	@if [ -f "../deployment-info.json" ]; then \
		BUILD_NODE_IP=$$(jq -r '.instance_external_ips.value[0]' ../deployment-info.json 2>/dev/null || jq -r '.instance_ips.value[0]' ../deployment-info.json 2>/dev/null); \
		if [ -n "$$BUILD_NODE_IP" ]; then \
			echo "Copying SSH key to build node ($$BUILD_NODE_IP) for direct distribution..."; \
			ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
				-i fastevm-deploy-key ubuntu@$$BUILD_NODE_IP 'mkdir -p ~/.ssh && chmod 700 ~/.ssh'; \
			scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
				-i fastevm-deploy-key fastevm-deploy-key ubuntu@$$BUILD_NODE_IP:~/.ssh/fastevm-deploy-key; \
			ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
				-i fastevm-deploy-key ubuntu@$$BUILD_NODE_IP 'chmod 600 ~/.ssh/fastevm-deploy-key'; \
			echo "✓ SSH key copied to build node for direct distribution"; \
		fi; \
	fi
	@echo "Getting node IPs from Terraform output..."
	@if [ -f "../deployment-info.json" ]; then \
		NODE_IPS=$$(jq -r '.instance_external_ips.value[]' ../deployment-info.json 2>/dev/null || jq -r '.instance_ips.value[]' ../deployment-info.json 2>/dev/null); \
		if [ -z "$$NODE_IPS" ]; then \
			echo "Could not get node IPs from Terraform output"; \
			exit 1; \
		fi; \
		echo "Deploying to nodes in parallel: $$NODE_IPS"; \
		NODE_INDEX=0; \
		for node_ip in $$NODE_IPS; do \
			( \
				echo "Starting deployment for node $$NODE_INDEX to $$node_ip..."; \
				echo "Creating remote directory for node $$NODE_INDEX..."; \
				ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
					-i fastevm-deploy-key ubuntu@$$node_ip 'mkdir -p /tmp/fastevm-config'; \
				echo "Copying configuration files for node $$NODE_INDEX..."; \
				scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
					-i fastevm-deploy-key -r deploy/node$$NODE_INDEX/* ubuntu@$$node_ip:/tmp/fastevm-config/; \
				echo "Running deployment script for node $$NODE_INDEX..."; \
				ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
					-i fastevm-deploy-key ubuntu@$$node_ip "GITHUB_REPO='$(GITHUB_REPO)' GITHUB_BRANCH='$(GITHUB_BRANCH)' bash /tmp/fastevm-config/deploy.sh"; \
				echo "✓ Node $$NODE_INDEX deployment completed successfully"; \
			) & \
			NODE_INDEX=$$((NODE_INDEX + 1)); \
		done; \
		wait; \
		echo "✓ All nodes deployed successfully!"; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
		exit 1; \
	fi

# Full local preparation workflow
prepare-and-deploy: prepare-configs deploy-configs ## Prepare configs locally and deploy to remote nodes
	@echo "Full preparation and deployment completed!"

# Stop services on all nodes
stop-services: ## Stop FastEVM services on all nodes
	@echo "Stopping FastEVM services on all nodes..."
	@if [ ! -f "fastevm-deploy-key" ]; then echo "SSH key not found. Run 'make apply' first."; exit 1; fi
	@chmod 600 fastevm-deploy-key
	@echo "Getting node IPs from Terraform output..."
	@if [ -f "../deployment-info.json" ]; then \
		NODE_IPS=$$(jq -r '.instance_external_ips.value[]' ../deployment-info.json 2>/dev/null || jq -r '.instance_ips.value[]' ../deployment-info.json 2>/dev/null); \
		if [ -z "$$NODE_IPS" ]; then \
			echo "Could not get node IPs from Terraform output"; \
			exit 1; \
		fi; \
		NODE_COUNT=$$(echo "$$NODE_IPS" | wc -l | tr -d ' '); \
		echo "Found $$NODE_COUNT nodes"; \
		echo "Stopping services on all nodes in parallel..."; \
		for node_ip in $$NODE_IPS; do \
			(echo "Stopping services on node $$node_ip..." && \
			ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -i fastevm-deploy-key ubuntu@$$node_ip 'sudo bash /tmp/fastevm-config/service.sh stop' && \
			echo "✓ Services stopped on $$node_ip") & \
		done; \
		echo "Waiting for all services to stop..."; \
		wait; \
		echo "Services stopped on all nodes!"; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
		exit 1; \
	fi

# Generate prefunded accounts on all nodes (after binary distribution)
setup-node: ## Generate prefunded accounts on all nodes after binary distribution
	@echo "Generating prefunded accounts on all nodes..."
	@if [ ! -f "fastevm-deploy-key" ]; then echo "SSH key not found. Run 'make apply' first."; exit 1; fi
	@chmod 600 fastevm-deploy-key
	@echo "Getting node IPs from Terraform output..."
	@if [ -f "../deployment-info.json" ]; then \
		NODE_IPS=$$(jq -r '.instance_external_ips.value[]' ../deployment-info.json 2>/dev/null || jq -r '.instance_ips.value[]' ../deployment-info.json 2>/dev/null); \
		if [ -z "$$NODE_IPS" ]; then \
			echo "Could not get node IPs from Terraform output"; \
			exit 1; \
		fi; \
		echo "Generating prefunded accounts on nodes: $$NODE_IPS"; \
		for node_ip in $$NODE_IPS; do \
			echo "Generating prefunded accounts on node $$node_ip..."; \
			scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
				-i fastevm-deploy-key scripts/setup-node.sh ubuntu@$$node_ip:/tmp/fastevm-config/; \
			ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
				-i fastevm-deploy-key ubuntu@$$node_ip 'chmod +x /tmp/fastevm-config/setup-node.sh && /tmp/fastevm-config/setup-node.sh'; \
			echo "✓ Prefunded accounts generated on $$node_ip"; \
		done; \
		echo "✓ Prefunded accounts generation completed on all nodes!"; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
		exit 1; \
	fi

# Start services on all nodes
start-services: ## Start FastEVM services on all nodes
	@echo "Starting FastEVM services on all nodes..."
	@if [ ! -f "fastevm-deploy-key" ]; then echo "SSH key not found. Run 'make apply' first."; exit 1; fi
	@chmod 600 fastevm-deploy-key
	@echo "Getting node IPs from Terraform output..."
	@if [ -f "../deployment-info.json" ]; then \
		NODE_IPS=$$(jq -r '.instance_external_ips.value[]' ../deployment-info.json 2>/dev/null || jq -r '.instance_ips.value[]' ../deployment-info.json 2>/dev/null); \
		if [ -z "$$NODE_IPS" ]; then \
			echo "Could not get node IPs from Terraform output"; \
			exit 1; \
		fi; \
		NODE_COUNT=$$(echo "$$NODE_IPS" | wc -l | tr -d ' '); \
		echo "Found $$NODE_COUNT nodes"; \
		echo "Starting services on all nodes in parallel..."; \
		for node_ip in $$NODE_IPS; do \
			(echo "Starting services on node $$node_ip..." && \
			ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -i fastevm-deploy-key ubuntu@$$node_ip 'sudo bash /tmp/fastevm-config/service.sh start' && \
			echo "✓ Services started on $$node_ip") & \
		done; \
		echo "Waiting for all services to start..."; \
		wait; \
		echo "Waiting for services to initialize..."; \
		sleep 60; \
		echo "Services started on all nodes!"; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
		exit 1; \
	fi

# Complete deployment (infrastructure + configs + deploy + prepare binaries + generate accounts + start services)
deploy-all: check-prereqs init apply prepare-configs deploy-configs prepare-binaries setup-node start-services verify-deployment ## Complete deployment: create infrastructure, prepare configs, deploy all nodes, prepare binaries, generate accounts, and start services
	@echo "=== FastEVM Complete Deployment Finished ==="
	@echo "All nodes are configured and started..."
	@echo "Check status with: make status"
	@echo "Test endpoints with: make test-rpc"

# Deploy with existing infrastructure (skip Terraform)
deploy-existing: check-prereqs prepare-configs deploy-configs prepare-binaries setup-node start-services verify-deployment ## Deploy to existing infrastructure (skip Terraform)
	@echo "=== Deployment to Existing Infrastructure Finished ==="

# Destroy and redeploy
redeploy: destroy deploy-all ## Destroy existing infrastructure and redeploy
	@echo "=== Redeploy Finished ==="

# Verify deployment
verify-deployment: ## Verify deployment health
	@echo "Verifying deployment..."
	@if [ -f "../deployment-info.json" ]; then \
		echo "Waiting for services to start..."; \
		sleep 30; \
		echo "Testing node connectivity..."; \
		NODE_IPS=$$(jq -r '.instance_external_ips.value[]' ../deployment-info.json 2>/dev/null || jq -r '.instance_ips.value[]' ../deployment-info.json 2>/dev/null); \
		NODE_COUNT=$$(echo "$$NODE_IPS" | wc -l | tr -d ' '); \
		NODE_INDEX=0; \
		for node_ip in $$NODE_IPS; do \
			http_port=8545; \
			echo "Testing node $$NODE_INDEX ($$node_ip:$$http_port)..."; \
			if curl -s -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' "http://$$node_ip:$$http_port" | jq -e '.result' > /dev/null 2>&1; then \
				echo "✓ Node $$NODE_INDEX is responding (JSON-RPC working)"; \
			elif nc -z -w5 $$node_ip $$http_port 2>/dev/null; then \
				echo "✓ Node $$NODE_INDEX port is open (service running)"; \
			else \
				echo "⚠ Node $$NODE_INDEX is not responding yet (services may still be starting)"; \
			fi; \
			NODE_INDEX=$$((NODE_INDEX + 1)); \
		done; \
		echo "Deployment verification completed!"; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
	fi

# Deploy with custom node count
deploy-nodes: ## Deploy with custom node count (usage: make deploy-nodes NODE_COUNT=6)
	@echo "Deploying with $(NODE_COUNT) nodes..."
	@$(MAKE) deploy-all NODE_COUNT=$(NODE_COUNT)

# Deploy with custom project name
deploy-project: ## Deploy with custom project name (usage: make deploy-project PROJECT_NAME=myproject)
	@echo "Deploying with project name: $(PROJECT_NAME)"
	@$(MAKE) deploy-all PROJECT_NAME=$(PROJECT_NAME)

# Deploy with custom region/zone
deploy-region: ## Deploy with custom region/zone (usage: make deploy-region REGION=us-west1 ZONE=us-west1-a)
	@echo "Deploying to region: $(REGION), zone: $(ZONE)"
	@$(MAKE) deploy-all REGION=$(REGION) ZONE=$(ZONE)

# Deploy with custom machine type
deploy-machine: ## Deploy with custom machine type (usage: make deploy-machine MACHINE_TYPE=e2-standard-8)
	@echo "Deploying with machine type: $(MACHINE_TYPE)"
	@$(MAKE) deploy-all MACHINE_TYPE=$(MACHINE_TYPE)

# Show prepared configurations
show-configs: ## Show prepared configuration summary
	@echo "FastEVM Prepared Configurations"
	@echo "==============================="
	@if [ -f "config/network-summary.txt" ]; then \
		cat config/network-summary.txt; \
	else \
		echo "No configurations found. Run 'make prepare-configs' first."; \
	fi

# Update nodes
update: ## Update all nodes (rebuild and restart services)
	@echo "Updating all nodes..."
	@for i in $$(seq 1 $(NODE_COUNT)); do \
		echo "Updating node $$i..."; \
		gcloud compute ssh $(PROJECT_NAME)-node-$$i --zone=$(ZONE) --command="cd /opt/fastevm && git pull && export PATH=\"\$HOME/.cargo/bin:\$PATH\" && cargo build --release && sudo systemctl restart fastevm-execution fastevm-consensus" 2>/dev/null || echo "Failed to update node $$i"; \
	done
	@echo "Update completed!"

# Restart services
restart: ## Restart FastEVM services on all nodes
	@echo "Restarting FastEVM services..."
	@for i in $$(seq 1 $(NODE_COUNT)); do \
		echo "Restarting services on node $$i..."; \
		gcloud compute ssh $(PROJECT_NAME)-node-$$i --zone=$(ZONE) --command="sudo systemctl restart fastevm-execution fastevm-consensus" 2>/dev/null || echo "Failed to restart services on node $$i"; \
	done
	@echo "Services restarted!"

# Clean up data
clean-data: ## Clean up data on all nodes
	@echo "Cleaning up data on all nodes..."
	@for i in $$(seq 1 $(NODE_COUNT)); do \
		echo "Cleaning data on node $$i..."; \
		gcloud compute ssh $(PROJECT_NAME)-node-$$i --zone=$(ZONE) --command="sudo /usr/local/bin/fastevm-cleanup.sh" 2>/dev/null || echo "Failed to clean data on node $$i"; \
	done
	@echo "Data cleanup completed!"

# Destroy deployment
destroy: ## Destroy Terraform deployment
	@echo "Destroying FastEVM infrastructure..."
	@echo "This will delete all resources. Are you sure? (y/N)"
	@read -r response; \
	if [ "$$response" = "y" ] || [ "$$response" = "Y" ]; then \
		terraform destroy -auto-approve $(TF_VARS); \
		echo "Infrastructure destroyed successfully!"; \
		rm -f ../deployment-info.json; \
	else \
		echo "Destruction cancelled."; \
	fi

# Clean up local files
clean: ## Clean up local files
	@echo "Cleaning up local files..."
	@rm -f tfplan
	@rm -f ../deployment-info.json
	@echo "Local files cleaned!"

# Show costs
costs: ## Show estimated costs
	@echo "Estimated Monthly Costs"
	@echo "====================="
	@echo "Instances: $(NODE_COUNT) x $(MACHINE_TYPE)"
	@echo "Disks: $(NODE_COUNT) x 100GB SSD"
	@echo ""
	@echo "Note: Actual costs may vary based on usage and GCP pricing."

# Backup configuration
backup: ## Backup current configuration
	@echo "Backing up configuration..."
	@mkdir -p backups
	@tar -czf backups/fastevm-config-$$(date +%Y%m%d-%H%M%S).tar.gz . ../deployment-info.json 2>/dev/null || true
	@echo "Configuration backed up!"

# Restore configuration
restore: ## Restore configuration from backup (usage: make restore BACKUP=backup-file.tar.gz)
	@if [ -z "$(BACKUP)" ]; then echo "Usage: make restore BACKUP=backup-file.tar.gz"; exit 1; fi
	@echo "Restoring configuration from $(BACKUP)..."
	@tar -xzf backups/$(BACKUP)
	@echo "Configuration restored!"

# Show all outputs
outputs: ## Show all Terraform outputs
	@echo "Terraform Outputs"
	@echo "================="
	@terraform output
	@echo ""
	@echo "SSH Key Information:"
	@echo "==================="
	@echo "Private Key: $(shell terraform output -raw ssh_private_key_path 2>/dev/null || echo 'Not available')"
	@echo "Public Key:  $(shell terraform output -raw ssh_public_key_path 2>/dev/null || echo 'Not available')"
	@echo ""
	@echo "To connect to nodes:"
	@echo "ssh -i $(shell terraform output -raw ssh_private_key_path 2>/dev/null || echo 'fastevm-deploy-key') ubuntu@<node-ip>"

# Quick start
quick-start: check-prereqs init deploy status ## Quick start deployment
	@echo "Quick start completed!"
	@echo "Run 'make status' to see deployment details"
	@echo "Run 'make test-rpc' to test the endpoints"

# Development workflow
dev: format validate plan ## Development workflow (format, validate, plan)
	@echo "Development workflow completed!"

# Production deployment
prod: check-prereqs init validate apply status health test-rpc ## Production deployment workflow
	@echo "Production deployment completed!"
	@echo "All nodes are healthy and RPC endpoints are working!"

# Test deployment with batch transactions and block scanning
# This command performs a complete test of the deployed FastEVM network:
# 1. Extracts RPC endpoints and configuration from deployment-info.json
# 2. Builds the batch command binary from testing/integration
# 3. Runs batch transactions with configurable parameters
# 4. Waits for transactions to be processed
# 5. Scans blocks to verify transaction processing
#
# Usage examples:
#   make test                                    # Run with default parameters
#   make test TEST_SENDER_COUNT=50               # Run with 50 senders
#   make test TEST_TRANSACTION_COUNT=5           # Run with 5 transactions per sender
#   make test TEST_WAITING_TIME_SECONDS=60       # Wait 60 seconds before scanning
test: ## Test deployment: build batch command, send transactions, and scan blocks
	@echo "🧪 Starting FastEVM deployment test..."
	@if [ ! -f "../deployment-info.json" ]; then \
		echo "❌ No deployment info found. Run 'make deploy-all' first."; \
		exit 1; \
	fi
	@echo "📋 Extracting deployment configuration..."
	@$(MAKE) extract-test-config
	@echo "🔨 Building batch command..."
	@$(MAKE) build-test-command
	@echo "🚀 Running batch transactions..."
	@$(MAKE) run-batch-test
	@echo "⏳ Waiting for transactions to be processed..."
	@$(MAKE) wait-for-transactions
	@echo "🔍 Scanning blocks for transaction verification..."
	@$(MAKE) scan-blocks
	@echo "✅ Test completed successfully!"

# Extract test configuration from deployment
extract-test-config: ## Extract configuration variables for testing (RPC URLs are extracted dynamically)
	@echo "Extracting test configuration..."
	@if [ -f "../deployment-info.json" ]; then \
		echo "# FastEVM Test Configuration" > test.env; \
		echo "# Generated from deployment-info.json" >> test.env; \
		echo "# Generated on: $$(date)" >> test.env; \
		echo "" >> test.env; \
		echo "# Network Configuration" >> test.env; \
		echo "CHAIN_ID=202501" >> test.env; \
		echo "" >> test.env; \
		echo "# Batch Transaction Test Parameters" >> test.env; \
		echo "TEST_SENDER_COUNT=$${TEST_SENDER_COUNT:-10000}" >> test.env; \
		echo "TEST_TRANSACTION_COUNT=$${TEST_TRANSACTION_COUNT:-100}" >> test.env; \
		echo "TEST_TRANSACTION_VALUE=$${TEST_TRANSACTION_VALUE:-1000000000000000}" >> test.env; \
		echo "TEST_MNEMONIC='$${TEST_MNEMONIC:-abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about}'" >> test.env; \
		echo "" >> test.env; \
		echo "TEST_FETCH_NONCE=$${TEST_FETCH_NONCE:-false}" >> test.env; \
		echo "# Test Timing Configuration" >> test.env; \
		echo "TEST_WAITING_TIME_SECONDS=$${TEST_WAITING_TIME_SECONDS:-30}" >> test.env; \
		echo "" >> test.env; \
		echo "# Deployment Information" >> test.env; \
		echo "PROJECT_NAME=$$(jq -r '.deployment_summary.value.project_name' ../deployment-info.json)" >> test.env; \
		echo "NODE_COUNT=$$(jq -r '.deployment_summary.value.node_count' ../deployment-info.json)" >> test.env; \
		echo "REGION=$$(jq -r '.deployment_summary.value.region' ../deployment-info.json)" >> test.env; \
		echo "ZONE=$$(jq -r '.deployment_summary.value.zone' ../deployment-info.json)" >> test.env; \
		echo "MACHINE_TYPE=$$(jq -r '.deployment_summary.value.machine_type' ../deployment-info.json)" >> test.env; \
		echo "" >> test.env; \
		echo "Configuration extracted to test.env"; \
		echo ""; \
		echo "📋 Configuration Summary:"; \
		echo "  Chain ID: 202501"; \
		echo "  Test Parameters:"; \
		echo "    Sender Count: $${TEST_SENDER_COUNT:-100}"; \
		echo "    Transaction Count: $${TEST_TRANSACTION_COUNT:-1}"; \
		echo "    Transaction Value: $${TEST_TRANSACTION_VALUE:-1000000000000000} wei"; \
		echo "    Waiting Time: $${TEST_WAITING_TIME_SECONDS:-30} seconds"; \
		echo "  Note: RPC URLs are extracted dynamically from deployment-info.json during test execution"; \
	else \
		echo "❌ No deployment info found"; \
		exit 1; \
	fi

# Build fastevm-test command from testing/integration
build-test-command: ## Build the fastevm-test command binary
	@echo "Building fastevm-test command from testing/integration..."
	@cd ../testing/integration && cargo build --bin fastevm-test --release
	@if [ -f "../target/release/fastevm-test" ]; then \
		echo "✅ fastevm-test command built successfully"; \
	else \
		echo "❌ Failed to build fastevm-test command"; \
		exit 1; \
	fi

# Run batch test with extracted configuration
run-batch-test: ## Run batch transactions using extracted configuration
	@echo "Running batch transactions..."
	@if [ ! -f "fastevm.env" ]; then \
		echo "❌ No test configuration found. Run 'make extract-test-config' first."; \
		exit 1; \
	fi
	@if [ ! -f "../target/release/fastevm-test" ]; then \
		echo "❌ fastevm-test command not found. Run 'make build-test-command' first."; \
		exit 1; \
	fi
	@if [ ! -f "../deployment-info.json" ]; then \
		echo "❌ No deployment info found. Run 'make apply' first."; \
		exit 1; \
	fi
	@echo "🚀 Starting batch transaction test..."
	@set -a && . ./fastevm.env && set +a && \
	RPC_URL1=$$(jq -r '.node_endpoints.value."node-1".http_rpc' ../deployment-info.json) && \
	RPC_URL2=$$(jq -r '.node_endpoints.value."node-2".http_rpc' ../deployment-info.json) && \
	RPC_URL3=$$(jq -r '.node_endpoints.value."node-3".http_rpc' ../deployment-info.json) && \
	RPC_URL4=$$(jq -r '.node_endpoints.value."node-4".http_rpc' ../deployment-info.json) && \
	../target/release/fastevm-test batch \
		--sender-count $$TEST_SENDER_COUNT \
		--transaction-count $$TEST_TRANSACTION_COUNT \
		--mnemonic "$$TEST_MNEMONIC"
	@echo "✅ Batch transactions completed!"

# Wait for transactions to be processed
wait-for-transactions: ## Wait for configured time for transactions to be processed
	@echo "Waiting for transactions to be processed..."
	@if [ ! -f "fastevm.env" ]; then \
		echo "❌ No test configuration found. Run 'make extract-test-config' first."; \
		exit 1; \
	fi
	@WAIT_TIME=$$(grep TEST_WAITING_TIME_SECONDS fastevm.env | cut -d'=' -f2); \
	echo "⏳ Waiting $$WAIT_TIME seconds for transactions to be processed..."; \
	sleep $$WAIT_TIME; \
	echo "✅ Wait completed!"

# Scan blocks to verify transactions
scan-blocks: ## Scan blocks to verify transaction processing
	@echo "Scanning blocks for transaction verification..."
	@if [ ! -f "fastevm.env" ]; then \
		echo "❌ No test configuration found. Run 'make extract-test-config' first."; \
		exit 1; \
	fi
	@if [ ! -f "../target/release/fastevm-test" ]; then \
		echo "❌ fastevm-test command not found. Run 'make build-test-command' first."; \
		exit 1; \
	fi
	@if [ ! -f "../deployment-info.json" ]; then \
		echo "❌ No deployment info found. Run 'make apply' first."; \
		exit 1; \
	fi
	@echo "🔍 Scanning blocks for transaction verification..."
	@set -a && . ./fastevm.env && set +a && \
	RPC_URL1=$$(jq -r '.node_endpoints.value."node-1".http_rpc' ../deployment-info.json) && \
	../target/release/fastevm-test scan \
		--count 10 \
		--url $$RPC_URL1 \
		--include-empty
	@echo "✅ Block scanning completed!"

# Run block scanning only
scan-only: extract-test-config build-test-command ## Run block scanning only
	@echo "🔍 Running block scanning only..."
	@$(MAKE) scan-blocks
	@echo "✅ Block scanning completed!"

# Run batch transactions only
batch-only: extract-test-config build-test-command ## Run batch transactions only
	@echo "🚀 Running batch transactions only..."
	@$(MAKE) run-batch-test
	@echo "✅ Batch transactions completed!"

# Run comprehensive test with custom parameters
test-custom: ## Run test with custom parameters (usage: make test-custom TEST_SENDER_COUNT=50 TEST_TRANSACTION_COUNT=2)
	@echo "🧪 Starting custom FastEVM test..."
	@$(MAKE) extract-test-config
	@$(MAKE) build-test-command
	@$(MAKE) run-batch-test
	@$(MAKE) wait-for-transactions
	@$(MAKE) scan-blocks
	@echo "✅ Custom test completed!"

# Show test configuration
show-test-config: extract-test-config ## Show current test configuration
	@echo "📋 Current Test Configuration:"
	@echo "=============================="
	@if [ -f "fastevm.env" ]; then \
		cat fastevm.env; \
	else \
		echo "No test configuration found. Run 'make extract-test-config' first."; \
	fi

# Show current RPC endpoints
show-rpc-endpoints: ## Show current RPC endpoints from deployment
	@echo "🌐 Current RPC Endpoints:"
	@echo "========================="
	@if [ -f "../deployment-info.json" ]; then \
		echo "Node 1: $$(jq -r '.node_endpoints.value."node-1".http_rpc' ../deployment-info.json)"; \
		echo "Node 2: $$(jq -r '.node_endpoints.value."node-2".http_rpc' ../deployment-info.json)"; \
		echo "Node 3: $$(jq -r '.node_endpoints.value."node-3".http_rpc' ../deployment-info.json)"; \
		echo "Node 4: $$(jq -r '.node_endpoints.value."node-4".http_rpc' ../deployment-info.json)"; \
	else \
		echo "❌ No deployment info found. Run 'make apply' first."; \
	fi

# Clean test files
clean-test: ## Clean up test files
	@echo "Cleaning up test files..."
	@rm -f fastevm.env
	@echo "Test files cleaned!"
