# FastEVM Terraform Deployment Makefile

.PHONY: help init plan apply destroy status logs clean validate format deploy ssh-node test-rpc restart health check-setup-status check-setup-logs

# Load environment variables
ifneq (,$(wildcard fastevm.env))
    include fastevm.env
endif

# Configuration
PROJECT_ID ?= $(shell gcloud config get-value project 2>/dev/null)
REGION ?= us-central1
ZONE ?= us-central1-a
NODE_COUNT ?= 4
MACHINE_TYPE ?= e2-standard-4
DISK_TYPE ?= pd-standard
PROJECT_NAME ?= fastevm
GITHUB_REPO ?= https://github.com/scalarorg/fastevm.git
GITHUB_BRANCH ?= main

# Use terraform.tfvars if it exists (Terraform auto-loads it, but we can be explicit)
# Otherwise fall back to environment variables from fastevm.env
TF_VARS = $(if $(wildcard terraform.tfvars),-var-file=terraform.tfvars,\
          -var="project_id=$(PROJECT_ID)" -var="region=$(REGION)" -var="zone=$(ZONE)" \
          -var="node_count=$(NODE_COUNT)" -var="machine_type=$(MACHINE_TYPE)" \
          -var="disk_type=$(DISK_TYPE)" -var="project_name=$(PROJECT_NAME)" \
          -var="github_repo=$(GITHUB_REPO)" -var="github_branch=$(GITHUB_BRANCH)"\
          )

# SSH options
SSH_OPTS = -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR

help: ## Show help message
	@echo "FastEVM Terraform Deployment"
	@echo "============================"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@echo ""
	@echo "Variables: PROJECT_ID, REGION, ZONE, NODE_COUNT, MACHINE_TYPE, DISK_TYPE"

check-prereqs: ## Check prerequisites
	@command -v terraform >/dev/null 2>&1 || { echo "Error: terraform required"; exit 1; }
	@command -v gcloud >/dev/null 2>&1 || { echo "Error: gcloud required"; exit 1; }
	@command -v jq >/dev/null 2>&1 || { echo "Error: jq required"; exit 1; }
	@if [ -z "$(PROJECT_ID)" ]; then echo "Error: PROJECT_ID required"; exit 1; fi

init: check-prereqs ## Initialize Terraform
	terraform init

validate: ## Validate Terraform configuration
	terraform validate

format: ## Format Terraform files
	terraform fmt -recursive

plan: init validate ## Plan Terraform deployment
	terraform plan $(TF_VARS) -out=tfplan

apply: plan ## Apply Terraform deployment
	@terraform apply tfplan || { \
		echo "Apply failed. Trying to import existing instances..."; \
		$(MAKE) import-existing || true; \
		terraform plan $(TF_VARS) -out=tfplan; \
		terraform apply tfplan; \
	}
	@terraform output -json > ../deployment-info.json
	@echo "✓ Deployment applied"

import-existing: init ## Import existing instances
	@echo "Importing existing resources..."
	@# Import global address if it exists
	@if gcloud compute addresses describe "$(PROJECT_NAME)-ip" --global --project="$(PROJECT_ID)" >/dev/null 2>&1; then \
		echo "Importing global address $(PROJECT_NAME)-ip..."; \
		terraform import "google_compute_global_address.fastevm_ip" "projects/$(PROJECT_ID)/global/addresses/$(PROJECT_NAME)-ip" 2>/dev/null || true; \
	fi
	@# Import compute instances - detect actual zone dynamically
	@for i in $$(seq 0 $$(($(NODE_COUNT) - 1))); do \
		name="$(PROJECT_NAME)-node-$$(($$i + 1))"; \
		zone=$$(gcloud compute instances describe "$$name" --project="$(PROJECT_ID)" --format="value(zone)" 2>/dev/null | awk -F'/' '{print $$NF}'); \
		if [ -n "$$zone" ]; then \
			path="projects/$(PROJECT_ID)/zones/$$zone/instances/$$name"; \
			echo "Importing instance $$name from zone $$zone..."; \
			terraform import "google_compute_instance.fastevm_nodes[$$i]" "$$path" 2>/dev/null || true; \
		fi; \
	done
	@echo "Import complete"

# Extract deployment info and create env file
extract-deployment-info: apply
	@EXTERNAL_IPS=$$(terraform output -json instance_external_ips | jq -r '.[]' | tr '\n' ' '); \
	INTERNAL_IPS=$$(terraform output -json instance_ips | jq -r '.[]' | tr '\n' ' '); \
	EXTERNAL_ARRAY=($$EXTERNAL_IPS); INTERNAL_ARRAY=($$INTERNAL_IPS); \
	NODE_COUNT=$${#EXTERNAL_ARRAY[@]}; \
	echo "# FastEVM Nodes - Local IPs Only" > nodes.remote.env; \
	echo "# This file contains only local/internal IPs of all nodes" >> nodes.remote.env; \
	echo "PEER_COUNT=$$NODE_COUNT" >> nodes.remote.env; \
	for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
		idx=$$((i+1)); \
		echo "PEER$${idx}_IP=$${INTERNAL_ARRAY[$$i]}" >> nodes.remote.env; \
	done; \
	KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	chmod 600 "$$KEY" 2>/dev/null || true; \
	echo "✓ Environment file created"

# Copy files to all nodes
copy-files: extract-deployment-info
	@echo "📦 Copying files to all nodes..."
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IPS=$$(terraform output -json instance_external_ips | jq -r '.[]' | tr '\n' ' '); \
	IPS_ARRAY=($$IPS); \
	NODE_COUNT=$${#IPS_ARRAY[@]}; \
	FAILED=0; \
	for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
		ip=$${IPS_ARRAY[$$i]}; \
		node_num=$$((i+1)); \
		echo "  📤 Copying files to node $$node_num ($$ip)..."; \
		if ssh $(SSH_OPTS) -i "$$KEY" ubuntu@$$ip "sudo mkdir -p /opt/fastevm && sudo chown ubuntu:ubuntu /opt/fastevm" 2>&1; then \
			for f in genesis.json node.sh setup.sh service.sh; do \
				if scp $(SSH_OPTS) -i "$$KEY" ../scripts/$$f ubuntu@$$ip:/opt/fastevm/ 2>&1; then \
					[[ $$f == *.sh ]] && ssh $(SSH_OPTS) -i "$$KEY" ubuntu@$$ip "chmod +x /opt/fastevm/$$f" 2>&1 || true; \
				else \
					echo "    ❌ Failed to copy $$f to node $$node_num"; \
					FAILED=$$((FAILED + 1)); \
				fi; \
			done; \
			if scp $(SSH_OPTS) -i "$$KEY" nodes.remote.env ubuntu@$$ip:/opt/fastevm/fastevm.env 2>&1; then \
				echo "    ✅ Files copied to node $$node_num"; \
			else \
				echo "    ❌ Failed to copy nodes.remote.env to node $$node_num"; \
				FAILED=$$((FAILED + 1)); \
			fi; \
		else \
			echo "    ❌ Failed to create directory on node $$node_num"; \
			FAILED=$$((FAILED + 1)); \
		fi; \
	done; \
	if [ $$FAILED -eq 0 ]; then \
		echo "✓ Files copied to all nodes successfully"; \
	else \
		echo "⚠️  Files copied with $$FAILED error(s) - continuing anyway"; \
	fi; \
	true  # Ensure target always succeeds

# Run setup on all nodes in parallel
run-setup: copy-files
	@echo ""
	@echo "🚀 Starting setup on all nodes in parallel..."
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IPS=$$(terraform output -json instance_external_ips | jq -r '.[]' | tr '\n' ' '); \
	IPS_ARRAY=($$IPS); \
	NODE_COUNT=$${#IPS_ARRAY[@]}; \
	PIDS=""; \
	for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
		ip=$${IPS_ARRAY[$$i]}; \
		node_num=$$((i+1)); \
		echo "  📦 Starting setup on node $$node_num ($$ip)..."; \
		(ssh $(SSH_OPTS) -i "$$KEY" ubuntu@$$ip "cd /opt/fastevm && nohup bash node.sh setup > /tmp/fastevm-setup.log 2>&1 &" 2>&1 && \
		 echo "    ✅ Setup process started on node $$node_num" || \
		 echo "    ❌ Failed to start setup on node $$node_num") & \
		PIDS="$$PIDS $$!"; \
	done; \
	for pid in $$PIDS; do \
		wait $$pid || true; \
	done; \
	echo "✓ Setup started on all nodes (running in parallel)"

# Wait for setup completion and verify services
wait-for-setup: run-setup
	@echo ""
	@echo "⏳ Waiting for setup to complete on all nodes..."
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IPS=$$(terraform output -json instance_external_ips | jq -r '.[]' | tr '\n' ' '); \
	IPS_ARRAY=($$IPS); \
	NODE_COUNT=$${#IPS_ARRAY[@]}; \
	ELAPSED=0; MAX_WAIT=1800; CHECK_INTERVAL=15; \
	while [ $$ELAPSED -lt $$MAX_WAIT ]; do \
		READY=0; \
		RUNNING=0; \
		echo ""; \
		echo "📊 Status check at $$(date +%H:%M:%S) (elapsed: $$ELAPSEDs / $$MAX_WAITs)"; \
		for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
			ip=$${IPS_ARRAY[$$i]}; \
			node_num=$$((i+1)); \
			SETUP_RUNNING=$$(ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$ip \
				"pgrep -f 'node.sh setup\|setup.sh' > /dev/null 2>&1 && echo 'yes' || echo 'no'" 2>/dev/null || echo "no"); \
			BINARY_EXISTS=$$(ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$ip \
				"test -f /usr/local/bin/fastevm-execution && echo 'yes' || echo 'no'" 2>/dev/null || echo "no"); \
			CONSENSUS_EXISTS=$$(ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$ip \
				"test -f /usr/local/bin/evm-consensus && echo 'yes' || echo 'no'" 2>/dev/null || echo "no"); \
			if [ "$$BINARY_EXISTS" = "yes" ] && [ "$$CONSENSUS_EXISTS" = "yes" ] && [ "$$SETUP_RUNNING" = "no" ]; then \
				echo "  ✅ Node $$node_num ($$ip): Binaries ready"; \
				READY=$$((READY + 1)); \
			elif [ "$$SETUP_RUNNING" = "yes" ]; then \
				echo "  🔄 Node $$node_num ($$ip): Setup in progress..."; \
				RUNNING=$$((RUNNING + 1)); \
			else \
				echo "  ⏸️  Node $$node_num ($$ip): Waiting..."; \
			fi; \
		done; \
		echo "  Summary: $$READY/$$NODE_COUNT ready, $$RUNNING running"; \
		if [ $$READY -eq $$NODE_COUNT ]; then \
			echo ""; \
			echo "✅ All nodes binaries ready! Verifying services..."; \
			break; \
		fi; \
		sleep $$CHECK_INTERVAL; \
		ELAPSED=$$((ELAPSED + CHECK_INTERVAL)); \
	done; \
	if [ $$ELAPSED -ge $$MAX_WAIT ]; then \
		echo ""; \
		echo "❌ Timeout waiting for setup to complete"; \
		echo "Run 'make check-setup-logs' to see what's happening"; \
		exit 1; \
	fi; \
	echo ""; \
	echo "🔍 Verifying systemd services are installed and enabled..."; \
	SERVICES_READY=0; \
	for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
		ip=$${IPS_ARRAY[$$i]}; \
		node_num=$$((i+1)); \
		EXEC_SERVICE=$$(ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$ip \
			"systemctl list-unit-files | grep -q 'fastevm-execution.service' && systemctl is-enabled fastevm-execution.service > /dev/null 2>&1 && echo 'yes' || echo 'no'" 2>/dev/null || echo "no"); \
		if [ "$$EXEC_SERVICE" = "yes" ]; then \
			echo "  ✅ Node $$node_num ($$ip): Services installed and enabled"; \
			SERVICES_READY=$$((SERVICES_READY + 1)); \
		else \
			echo "  ⚠️  Node $$node_num ($$ip): Services not ready (may need manual installation)"; \
		fi; \
	done; \
	if [ $$SERVICES_READY -eq $$NODE_COUNT ]; then \
		echo "✅ All services verified and ready!"; \
	else \
		echo "⚠️  Some services may need manual installation. Continuing anyway..."; \
	fi

# Start services on all nodes
start-services: wait-for-setup
	@echo ""
	@echo "🚀 Starting services on all nodes..."
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IPS=$$(terraform output -json instance_external_ips | jq -r '.[]' | tr '\n' ' '); \
	IPS_ARRAY=($$IPS); \
	NODE_COUNT=$${#IPS_ARRAY[@]}; \
	FAILED=0; \
	for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
		ip=$${IPS_ARRAY[$$i]}; \
		node_num=$$((i+1)); \
		echo "  ▶️  Starting services on node $$node_num ($$ip)..."; \
		if ssh $(SSH_OPTS) -i "$$KEY" ubuntu@$$ip "cd /opt/fastevm && bash node.sh start" 2>&1; then \
			echo "    ✅ Services started on node $$node_num"; \
		else \
			echo "    ❌ Failed to start services on node $$node_num"; \
			FAILED=$$((FAILED + 1)); \
		fi; \
	done; \
	if [ $$FAILED -eq 0 ]; then \
		echo "✓ All services started successfully"; \
		echo "⏳ Waiting 30 seconds for services to initialize..."; \
		sleep 30; \
	else \
		echo "⚠️  Services started with $$FAILED error(s)"; \
	fi

# Verify deployment
verify-deployment:
	@sleep 30; \
	IPS=$$(jq -r '.instance_external_ips.value[]' ../deployment-info.json 2>/dev/null || echo ""); \
	for ip in $$IPS; do \
		curl -s -X POST "http://$$ip:8545" -H "Content-Type: application/json" \
			-d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
			| jq -e '.result' >/dev/null 2>&1 && echo "✓ $$ip responding" || echo "⚠ $$ip not ready"; \
	done

# Main deployment target
deploy: check-prereqs init plan apply extract-deployment-info copy-files run-setup wait-for-setup start-services verify-deployment ## Complete deployment
	@echo "=== FastEVM Deployment Complete ==="
	@echo "Check status: make status"
	@echo "Test RPC: make test-rpc"

status: ## Show deployment status
	@if [ -f "../deployment-info.json" ]; then \
		jq -r '.deployment_summary.value | "Project: \(.project_name)\nNodes: \(.node_count)\nRegion: \(.region)"' ../deployment-info.json; \
		echo ""; \
		jq -r '.node_endpoints.value | to_entries[] | "\(.key): \(.value.http_rpc)"' ../deployment-info.json; \
	else \
		echo "No deployment info. Run 'make deploy' first."; \
	fi

instances: ## Show GCP instance status
	@gcloud compute instances list --filter="name~$(PROJECT_NAME)-node" \
		--format="table(name,zone,status,internalIP,externalIP)"

check-setup-status: ## Check setup status on all nodes
	@echo "🔍 Checking setup status on all nodes..."
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IPS=$$(terraform output -json instance_external_ips | jq -r '.[]' | tr '\n' ' '); \
	IPS_ARRAY=($$IPS); \
	NODE_COUNT=$${#IPS_ARRAY[@]}; \
	for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
		ip=$${IPS_ARRAY[$$i]}; \
		node_num=$$((i+1)); \
		echo ""; \
		echo "📋 Node $$node_num ($$ip):"; \
		SETUP_RUNNING=$$(ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$ip \
			"pgrep -f 'node.sh setup\|setup.sh' > /dev/null 2>&1 && echo 'yes' || echo 'no'" 2>/dev/null || echo "no"); \
		BINARY_EXISTS=$$(ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$ip \
			"test -f /usr/local/bin/fastevm-execution && echo 'yes' || echo 'no'" 2>/dev/null || echo "no"); \
		CONSENSUS_EXISTS=$$(ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$ip \
			"test -f /usr/local/bin/evm-consensus && echo 'yes' || echo 'no'" 2>/dev/null || echo "no"); \
		echo "  Setup running: $$SETUP_RUNNING"; \
		echo "  Binary exists: $$BINARY_EXISTS"; \
		echo "  Consensus binary: $$CONSENSUS_EXISTS"; \
	done

check-setup-logs: ## Show setup logs from all nodes
	@echo "📜 Setup logs from all nodes:"
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IPS=$$(terraform output -json instance_external_ips | jq -r '.[]' | tr '\n' ' '); \
	IPS_ARRAY=($$IPS); \
	NODE_COUNT=$${#IPS_ARRAY[@]}; \
	for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
		ip=$${IPS_ARRAY[$$i]}; \
		node_num=$$((i+1)); \
		echo ""; \
		echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"; \
		echo "Node $$node_num ($$ip) - Last 30 lines:"; \
		echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"; \
		ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$ip \
			"tail -n 30 /tmp/fastevm-setup.log 2>/dev/null || echo 'No setup log found'" 2>/dev/null || echo "  ⚠️  Could not connect to node"; \
	done

logs: ## Show bootstrap logs from all nodes
	@for i in $$(seq 1 $(NODE_COUNT)); do \
		echo "--- Node $$i ---"; \
		gcloud compute ssh $(PROJECT_NAME)-node-$$i --zone=$(ZONE) \
			--command="sudo tail -n 20 /var/log/fastevm-bootstrap.log" 2>/dev/null || echo "No logs"; \
	done

ssh-node: ## SSH to node (usage: make ssh-node NODE=1, or just make ssh-node for node 1)
	@NODE_NUM=$${NODE:-1}; \
	if [ -z "$$NODE_NUM" ] || [ "$$NODE_NUM" -lt 1 ]; then \
		echo "❌ Invalid node number. Node must be >= 1"; \
		exit 1; \
	fi; \
	KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IP=""; \
	if [ -f "../deployment-info.json" ]; then \
		IP=$$(jq -r ".instance_external_ips.value[$$((NODE_NUM-1))]" ../deployment-info.json 2>/dev/null); \
	fi; \
	if [ -z "$$IP" ] || [ "$$IP" = "null" ]; then \
		echo "📡 Getting IP from Terraform outputs..."; \
		IPS=$$(terraform output -json instance_external_ips 2>/dev/null | jq -r '.[]' 2>/dev/null); \
		if [ -n "$$IPS" ]; then \
			IP=$$(echo "$$IPS" | sed -n "$${NODE_NUM}p"); \
		fi; \
	fi; \
	if [ -z "$$IP" ] || [ "$$IP" = "null" ]; then \
		echo "❌ Node $$NODE_NUM not found"; \
		echo ""; \
		echo "Available nodes:"; \
		if [ -f "../deployment-info.json" ]; then \
			jq -r '.instance_external_ips.value | to_entries[] | "  Node \(.key + 1): \(.value)"' ../deployment-info.json 2>/dev/null || true; \
		fi; \
		IPS=$$(terraform output -json instance_external_ips 2>/dev/null | jq -r '.[]' 2>/dev/null); \
		if [ -n "$$IPS" ]; then \
			n=1; \
			for ip in $$IPS; do \
				echo "  Node $$n: $$ip"; \
				n=$$((n+1)); \
			done; \
		fi; \
		exit 1; \
	fi; \
	echo "🔌 Connecting to node $$NODE_NUM ($$IP)..."; \
	ssh $(SSH_OPTS) -i "$$KEY" ubuntu@$$IP

test-rpc: ## Test RPC endpoints
	@IPS=$$(jq -r '.node_endpoints.value | to_entries[] | .value.http_rpc' ../deployment-info.json 2>/dev/null); \
	for endpoint in $$IPS; do \
		echo "Testing $$endpoint..."; \
		curl -s -X POST $$endpoint -H "Content-Type: application/json" \
			-d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
			| jq -r '.result // "Failed"' || echo "Failed"; \
	done

restart: ## Restart services on all nodes
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IPS=$$(terraform output -json instance_external_ips | jq -r '.[]'); \
	for ip in $$IPS; do \
		ssh $(SSH_OPTS) -i "$$KEY" ubuntu@$$ip "cd /opt/fastevm && bash node.sh start" & \
	done; \
	wait; \
	echo "✓ Services restarted"

health: ## Check health of all nodes
	@for i in $$(seq 1 $(NODE_COUNT)); do \
		echo "Node $$i:"; \
		gcloud compute ssh $(PROJECT_NAME)-node-$$i --zone=$(ZONE) \
			--command="sudo /usr/local/bin/fastevm-health-check.sh $$((i-1))" 2>/dev/null || echo "  Health check failed"; \
	done

destroy: ## Destroy deployment
	@echo "Destroying infrastructure... (y/N)"; \
	read -r response; \
	[ "$$response" = "y" ] || [ "$$response" = "Y" ] && terraform destroy -auto-approve $(TF_VARS) && rm -f ../deployment-info.json || echo "Cancelled"

clean: ## Clean local files
	@rm -f tfplan ../deployment-info.json nodes.remote.env
	@echo "✓ Cleaned"

outputs: ## Show Terraform outputs
	@terraform output
