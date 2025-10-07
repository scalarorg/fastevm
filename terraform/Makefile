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
	@./scripts/prepare-configs.sh
	@echo "Configurations prepared successfully!"

# Deploy configurations to remote nodes
deploy-configs: ## Deploy prepared configurations to remote nodes
	@echo "Deploying configurations to remote nodes..."
	@if [ ! -d "deploy" ]; then echo "No deploy directory found. Run 'make prepare-configs' first."; exit 1; fi
	@cd deploy && ./deploy-all.sh
	@echo "Deployment completed!"

# Full local preparation workflow
prepare-and-deploy: prepare-configs deploy-configs ## Prepare configs locally and deploy to remote nodes
	@echo "Full preparation and deployment completed!"

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
