# FastEVM Terraform Deployment Makefile
SHELL := /bin/bash
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
SETUP_SUCCESS_MSG := FastEVM services installed successfully!
WORKSPACE_DIR ?= /opt/workspace
GENESIS_CONFIG_JSON ?= /tmp/genesis_config.json

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
	@terraform plan $(TF_VARS) -out=tfplan

apply: plan ## Apply Terraform deployment
	@terraform apply tfplan
	@terraform output -json > ../deployment-info.json
	@echo "✓ Deployment applied"


# Extract deployment info and create env file
extract-deployment-info: apply
	@EXTERNAL_IPS=$$(terraform output -json instance_external_ips | jq -r '.[]' | tr '\n' ' '); \
	INTERNAL_IPS=$$(terraform output -json instance_ips | jq -r '.[]' | tr '\n' ' '); \
	EXTERNAL_ARRAY=($$EXTERNAL_IPS); INTERNAL_ARRAY=($$INTERNAL_IPS); \
	NODE_COUNT=$${#EXTERNAL_ARRAY[@]}; \
	cp node.env nodes.remote.env; \
	echo "PEER_COUNT=$$NODE_COUNT" >> nodes.remote.env; \
	echo "ENV_FILE=/opt/fastevm/node.env" >> nodes.remote.env; \
	for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
		idx=$$((i+1)); \
		echo "PEER$${idx}_IP=$${INTERNAL_ARRAY[$$i]}" >> nodes.remote.env; \
	done; \
	KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	chmod 600 "$$KEY" 2>/dev/null || true; \
	echo "✓ Environment file created"

# Copy files to all nodes
copy-files: apply
	@echo "📦 Copying files to all nodes..."
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IPS=$$(terraform output -json instance_external_ips | jq -r '.[]' | tr '\n' ' '); \
	IPS_ARRAY=($$IPS); \
	NODE_COUNT=$${#IPS_ARRAY[@]}; \
	FAILED=0; \
	FILES_TO_COPY=( \
		systemd/fastevm-execution.service \
		systemd/fastevm-consensus.service \
		node.sh \
		setup.sh \
		service.sh \
		run.sh \
		node.yml \
		validators.yml \
		parameters.yml \
	); \
	for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
		ip=$${IPS_ARRAY[$$i]}; \
		node_num=$$((i+1)); \
		echo "  📤 Copying files to node $$node_num ($$ip)..."; \
		if ssh $(SSH_OPTS) -i "$$KEY" ubuntu@$$ip "sudo mkdir -p /opt/fastevm && sudo chown ubuntu:ubuntu /opt/fastevm"; then \
			for f in "$${FILES_TO_COPY[@]}"; do \
				if scp $(SSH_OPTS) -i "$$KEY" "scripts/$$f" "ubuntu@$$ip:/opt/fastevm/"; then \
					if [[ "$$f" == *.sh ]]; then \
						ssh $(SSH_OPTS) -i "$$KEY" ubuntu@$$ip "chmod +x /opt/fastevm/$$f" || true; \
					fi; \
				else \
					echo "    ❌ Failed to copy $$f to node $$node_num"; \
					FAILED=$$((FAILED + 1)); \
				fi; \
			done; \
			if scp $(SSH_OPTS) -i "$$KEY" node.env ubuntu@$$ip:/opt/fastevm/node.env; then \
				echo "    ✅ Files copied to node $$node_num"; \
			else \
				echo "    ❌ Failed to copy node.env to node $$node_num"; \
				FAILED=$$((FAILED + 1)); \
			fi; \
		else \
			echo "    ❌ Failed to prepare directory on node $$node_num"; \
			FAILED=$$((FAILED + 1)); \
		fi; \
	done; \
	if [ $$FAILED -eq 0 ]; then \
		echo "✓ Files copied to all nodes successfully"; \
	else \
		echo "⚠️  Files copied with $$FAILED error(s) - continuing anyway"; \
	fi; \
	true  # Ensure target always succeeds

start-setup: copy-files
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	PUBLIC_IPS=$$(terraform output -json instance_external_ips | jq -r '.[]'); \
	PRIVATE_IPS=$$(terraform output -json instance_ips | jq -r '.[]'); \
	PUBLIC_ARRAY=($$PUBLIC_IPS); \
	PRIVATE_ARRAY=($$PRIVATE_IPS); \
	NODE_COUNT=$${#PUBLIC_ARRAY[@]}; \
	for (( i=0; i<NODE_COUNT; i++ )); do \
		public_ip=$${PUBLIC_ARRAY[$$i]}; \
		private_ip=$${PRIVATE_ARRAY[$$i]}; \
		ssh $(SSH_OPTS) -i "$$KEY" ubuntu@$$public_ip \
			"cd /opt/fastevm && \
			tmux kill-session -t fastevm-setup 2>/dev/null || true && \
			tmux new-session -d -s fastevm-setup \"GITHUB_BRANCH=$(GITHUB_BRANCH) bash -x node.sh setup $$private_ip $$public_ip > /tmp/fastevm-setup.log 2>&1\"" & \
	done; \
	wait



watch-setup: start-setup
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IPS=$$(terraform output -json instance_external_ips | jq -r '.[]'); \
	NODE_COUNT=$$(echo "$$IPS" | wc -l | tr -d ' '); \
	TMP_DONE=$$(mktemp); \
	trap 'rm -f $$TMP_DONE' EXIT; \
	echo ""; \
	echo "📡 Watching setup logs (auto-exit when all nodes finish)..."; \
	echo ""; \
	for ip in $$IPS; do \
		ssh -T $(SSH_OPTS) -i "$$KEY" ubuntu@$$ip \
			"bash -lc 'tail -n 20 -f /tmp/fastevm-setup.log'" | \
			while IFS= read -r line; do \
				echo "[$$ip] $$line"; \
				if echo "$$line" | grep -q "$(SETUP_SUCCESS_MSG)"; then \
					echo "$$ip" >> $$TMP_DONE; \
					break; \
				fi; \
			done & \
	done; \
	while [ $$(sort -u $$TMP_DONE 2>/dev/null | wc -l | tr -d ' ') -lt $$NODE_COUNT ]; do \
		sleep 2; \
	done; \
	echo ""; \
	echo "✅ All $$NODE_COUNT nodes finished setup"


update-bootnodes: watch-setup
	@echo "🔗 Collecting all peer IDs (using Terraform private IPs)..."
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	PUBLIC_IPS=$$(terraform output -json instance_external_ips | jq -r '.[]'); \
	PRIVATE_IPS=$$(terraform output -json instance_ips | jq -r '.[]'); \
	PUBLIC_ARRAY=($$PUBLIC_IPS); \
	PRIVATE_ARRAY=($$PRIVATE_IPS); \
	NODE_COUNT=$${#PUBLIC_ARRAY[@]}; \
	enode_urls=""; \
	for i in $$(seq 0 $$((NODE_COUNT - 1))); do \
		public_ip=$${PUBLIC_ARRAY[$$i]}; \
		private_ip=$${PRIVATE_ARRAY[$$i]}; \
		node_num=$$((i+1)); \
		peer_id=$$(ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$public_ip \
			"cat /data/execution/p2p/peer-id.hex" 2>/dev/null | tr -d '\n'); \
		if [ -n "$$peer_id" ] && [ -n "$$private_ip" ]; then \
			enode_url="enode://$${peer_id}@$${private_ip}:30303"; \
			enode_urls="$${enode_urls}$${enode_url},"; \
			echo "   ✅ Node $$node_num (pub $$public_ip, priv $$private_ip): $$enode_url"; \
		else \
			echo "   ❌ Node $$node_num ($$public_ip): missing peer-id or private IP"; \
		fi; \
	done; \
	enode_urls="$${enode_urls%,}"; \
	echo ""; \
	echo "📋 Collected enode URLs:"; \
	echo "$$enode_urls"; \
	echo ""; \
	echo "📤 Updating bootnodes on all nodes..."; \
	for public_ip in $$PUBLIC_IPS; do \
		ssh $(SSH_OPTS) -i "$$KEY" ubuntu@$$public_ip \
			"cd /opt/fastevm && bash service.sh update-bootnodes '$$enode_urls'" || exit 1; \
	done; \
	echo "✅ Bootnodes updated on all nodes"


## Collect all authority keys and update committees.yml
collect-validators: watch-setup
	@echo "🔗 Collecting authority.yml from all validators and building committees.yml"

	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	PUBLIC_IPS=$$(terraform output -json instance_external_ips | jq -r '.[]'); \
	TMP_COMMITTEES=$$(mktemp); \
	echo "epoch: 0" > $$TMP_COMMITTEES; \
	echo "authorities:" >> $$TMP_COMMITTEES; \
	INDEX=0; \
	for public_ip in $$PUBLIC_IPS; do \
		echo "  - index: $$INDEX" >> $$TMP_COMMITTEES; \
		ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$public_ip \
			"cat /data/config/authority.yml" \
		| sed 's/^/    /' \
		| grep -Ev '^(    index:|    epoch:)' \
		>> $$TMP_COMMITTEES || exit 1; \
		INDEX=$$((INDEX+1)); \
	done; \
	{ \
		echo ""; \
		echo "quorum_threshold: 2667"; \
		echo "validity_threshold: 1334"; \
	} >> $$TMP_COMMITTEES; \
	echo "📄 Generated committees.yml:"; \
	echo "--------------------------------"; \
	cat $$TMP_COMMITTEES; \
	echo "--------------------------------"; \
	echo "🚀 Updating committees.yml on all nodes..."; \
	for public_ip in $$PUBLIC_IPS; do \
		scp $(SSH_OPTS) -i "$$KEY" $$TMP_COMMITTEES "ubuntu@$$public_ip:/data/config/committees.yml" || exit 1; \
	done; \
	rm -f $$TMP_COMMITTEES; \
	echo "✅ Committees.yml updated on all nodes"
# Create genesis_config.json in the deloyment client node
genesis-config: watch-setup
	@echo "🔗 Collecting authority.yml and building genesis_config.json"

	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	PUBLIC_IPS=$$(terraform output -json instance_external_ips | jq -r '.[]'); \
	GENESIS_CONFIG_JSON="$(GENESIS_CONFIG_JSON)"; \
	\
	jq -n '{validatorAddresses: [], consensusPublicKeys: [], votingPowers: [], validatorNetworkAddresses: [], fullnodeNetworkAddresses: [], aptosAddresses: []}' > $$GENESIS_CONFIG_JSON; \
	\
	for public_ip in $$PUBLIC_IPS; do \
	  AUTH_YML=$$(ssh $(SSH_OPTS) -o ConnectTimeout=5 -i "$$KEY" ubuntu@$$public_ip \
	    "cat /data/config/authority.yml") || exit 1; \
	  \
	  VALIDATOR_ADDR=$$(echo "$$AUTH_YML" | sed -n 's/^validator_address:[[:space:]]*//p'); \
	  AUTHORITY_KEY=$$(echo "$$AUTH_YML" | sed -n 's/^authority_key:[[:space:]]*//p'); \
	  STAKE=$$(echo "$$AUTH_YML" | sed -n 's/^stake:[[:space:]]*//p'); \
	  BASE_ADDR=$$(echo "$$AUTH_YML" | sed -n 's/^address:[[:space:]]*//p'); \
	  \
	  CONS_PUBKEY=$$(echo "$$AUTHORITY_KEY" | cut -c1-96); \
	  APTOS_ADDR=$$(echo "$$VALIDATOR_ADDR" | tr '[:upper:]' '[:lower:]'); \
	  APTOS_ADDR_LAST40=$$(echo "$$APTOS_ADDR" | rev | cut -c1-40 | rev); \
	  NET_ADDR="$$BASE_ADDR/noise-ik/$$APTOS_ADDR/handshake/0"; \
	  \
	  jq --arg vaddr "0x$$APTOS_ADDR_LAST40" --arg cpk "$$CONS_PUBKEY" --arg stake "$$STAKE" --arg net "$$NET_ADDR" --arg apt "$$APTOS_ADDR" '.validatorAddresses += [$$vaddr] | .consensusPublicKeys += [$$cpk] | .votingPowers += [$$stake] | .validatorNetworkAddresses += [$$net] | .fullnodeNetworkAddresses += [$$net] | .aptosAddresses += [$$apt]' $$GENESIS_CONFIG_JSON > $$GENESIS_CONFIG_JSON.tmp && mv $$GENESIS_CONFIG_JSON.tmp $$GENESIS_CONFIG_JSON; \
	done; \
	\
	echo "📄 Generated genesis_config.json:"; \
	jq . $$GENESIS_CONFIG_JSON;

# run on the deployment client node
# within the workspace directory, gravity-genesis-contract folder
create-genesis: genesis-config
	@echo "🔗 Creating genesis.json in the deployment client node"
	@KEY_PATH=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	MAKEFILE_DIR=$$(pwd); \
	if [ "$${KEY_PATH#/}" = "$$KEY_PATH" ]; then \
		KEY="$$MAKEFILE_DIR/$$KEY_PATH"; \
	else \
		KEY="$$KEY_PATH"; \
	fi; \
	if [ ! -f "$$KEY" ]; then \
		echo "❌ SSH key not found: $$KEY"; \
		exit 1; \
	fi; \
	PUBLIC_IPS=$$(terraform output -json instance_external_ips | jq -r '.[]'); \
	GENESIS_JSON_PATH=$(WORKSPACE_DIR)/gravity-genesis-contract/genesis.json; \
	mkdir -p $(WORKSPACE_DIR)/gravity-genesis-contract/generate; \
	cp $(GENESIS_CONFIG_JSON) $(WORKSPACE_DIR)/gravity-genesis-contract/generate/genesis_config.json; \
	cd $(WORKSPACE_DIR)/gravity-genesis-contract; \
	bash ./generate_genesis.sh; \
	echo "✅ genesis.json created in the deployment client node"; \
	for public_ip in $$PUBLIC_IPS; do \
	  scp $(SSH_OPTS) -i "$$KEY" $$GENESIS_JSON_PATH \
	    ubuntu@$$public_ip:/data/config/genesis.json || exit 1; \
	done; \
	echo "✅ genesis.json is copied to all nodes"

update-envs: update-bootnodes collect-validators create-genesis
	@echo "✅ Bootnodes and committees.yml updated on all nodes"

# Start services on all nodes
start-services: update-envs
	@echo ""
	@echo "🚀 Starting services on all nodes.... Branch: $(GITHUB_BRANCH)"
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

copy-configs: 
	@echo "📦 Copying files to all nodes..."
	@KEY=$$(terraform output -raw ssh_private_key_path 2>/dev/null || echo "fastevm-deploy-key"); \
	IPS=$$(terraform output -json instance_external_ips | jq -r '.[]' | tr '\n' ' '); \
	IPS_ARRAY=($$IPS); \
	NODE_COUNT=$${#IPS_ARRAY[@]}; \
	FAILED=0; \
	FILES_TO_COPY=( \
		authority.yml \
		validator.yml \
	); \
	for i in $$(seq 1 $$NODE_COUNT); do \
		ip=$${IPS_ARRAY[$$i-1]}; \
		node_num=$$i; \
		echo "  📤 Copying files to node $$node_num ($$ip)..."; \
		for f in "$${FILES_TO_COPY[@]}"; do \
			if scp $(SSH_OPTS) -i "$$KEY" "../../.local-data/consensus$$i/$$f" "ubuntu@$$ip:/data/config/"; then \
				echo "    ✅ Files copied to node $$node_num"; \
			else \
				echo "    ❌ Failed to copy $$f to node $$node_num"; \
				FAILED=$$((FAILED + 1)); \
			fi; \
		done; \
		if scp $(SSH_OPTS) -i "$$KEY" "./committees.yml" "ubuntu@$$ip:/data/config/committees.yml"; then \
			echo "    ✅ Files copied to node $$node_num"; \
		else \
			echo "    ❌ Failed to copy $$f to node $$node_num"; \
			FAILED=$$((FAILED + 1)); \
		fi; \
	done; \
	if [ $$FAILED -eq 0 ]; then \
		echo "✓ Files copied to all nodes successfully"; \
	else \
		echo "⚠️  Files copied with $$FAILED error(s) - continuing anyway"; \
	fi; \
	true  # Ensure target always succeeds