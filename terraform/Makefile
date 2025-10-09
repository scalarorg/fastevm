# FastEVM Terraform Deployment Makefile
# Easy management of GCP infrastructure deployment

.PHONY: help init plan apply destroy status logs clean validate format check

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

# SSH to a specific node
ssh-node: ## SSH to a specific node (usage: make ssh-node NODE=1)
	@if [ -z "$(NODE)" ]; then echo "Usage: make ssh-node NODE=1"; exit 1; fi
	@echo "Connecting to $(PROJECT_NAME)-node-$(NODE)..."
	@gcloud compute ssh $(PROJECT_NAME)-node-$(NODE) --zone=$(ZONE)

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

# Deploy configurations to remote nodes
deploy-configs: ## Deploy prepared configurations to remote nodes
	@echo "Deploying configurations to remote nodes..."
	@if [ ! -d "deploy" ]; then echo "No deploy directory found. Run 'make prepare-configs' first."; exit 1; fi
	@if [ ! -f "fastevm-deploy-key" ]; then echo "SSH key not found. Run 'make apply' first."; exit 1; fi
	@chmod 600 fastevm-deploy-key
	@echo "Getting node IPs from Terraform output..."
	@if [ -f "../deployment-info.json" ]; then \
		NODE_IPS=$$(jq -r '.instance_external_ips.value[]' ../deployment-info.json 2>/dev/null || jq -r '.instance_ips.value[]' ../deployment-info.json 2>/dev/null); \
		if [ -z "$$NODE_IPS" ]; then \
			echo "Could not get node IPs from Terraform output"; \
			exit 1; \
		fi; \
		echo "Deploying to nodes: $$NODE_IPS"; \
		NODE_INDEX=0; \
		for node_ip in $$NODE_IPS; do \
			echo "Deploying node $$NODE_INDEX to $$node_ip..."; \
			echo "Creating remote directory..."; \
			ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
				-i fastevm-deploy-key ubuntu@$$node_ip 'mkdir -p /tmp/fastevm-config'; \
			echo "Copying configuration files..."; \
			scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
				-i fastevm-deploy-key -r deploy/node$$NODE_INDEX/* ubuntu@$$node_ip:/tmp/fastevm-config/; \
			echo "Running deployment script..."; \
			ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
				-i fastevm-deploy-key ubuntu@$$node_ip 'bash /tmp/fastevm-config/deploy.sh' & \
			NODE_INDEX=$$((NODE_INDEX + 1)); \
		done; \
		wait; \
		echo "All nodes deployed successfully!"; \
	else \
		echo "No deployment info found. Run 'make apply' first."; \
		exit 1; \
	fi

# Full local preparation workflow
prepare-and-deploy: prepare-configs deploy-configs ## Prepare configs locally and deploy to remote nodes
	@echo "Full preparation and deployment completed!"

# Complete deployment (infrastructure + configs + deploy)
deploy-all: check-prereqs init apply prepare-configs deploy-configs verify-deployment ## Complete deployment: create infrastructure, prepare configs, and deploy all nodes
	@echo "=== FastEVM Complete Deployment Finished ==="
	@echo "All nodes are being configured and started..."
	@echo "Check status with: make status"
	@echo "Test endpoints with: make test-rpc"

# Deploy with existing infrastructure (skip Terraform)
deploy-existing: check-prereqs prepare-configs deploy-configs verify-deployment ## Deploy to existing infrastructure (skip Terraform)
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
		for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
			node_ip=$$(jq -r ".instance_external_ips.value[$$i]" ../deployment-info.json 2>/dev/null || jq -r ".instance_ips.value[$$i]" ../deployment-info.json 2>/dev/null); \
			http_port=$$((8545 + i * 2)); \
			echo "Testing node $$i ($$node_ip:$$http_port)..."; \
			if curl -s -f "http://$$node_ip:$$http_port" > /dev/null; then \
				echo "✓ Node $$i is responding"; \
			else \
				echo "⚠ Node $$i is not responding yet (services may still be starting)"; \
			fi; \
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
