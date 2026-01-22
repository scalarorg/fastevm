# FastEVM Client Node Management Makefile
# Simplified workflow: Deploy client node -> Setup -> Deploy network

.PHONY: help deploy setup-client deploy-network transfer-creds status connect ssh-node view-network network-logs view-setup destroy destroy_network clean copy-files

# Default target
help: ## Show available commands
	@echo "FastEVM Client Node Management"
	@echo "=============================="
	@echo ""
	@echo "Main Commands:"
	@echo "  deploy           Deploy client node and setup (Step 1)"
	@echo "  deploy-network   Setup client node and deploy network in tmux (All-in-one)"
	@echo "  transfer-creds   Transfer gcloud credentials to client node"
	@echo "  status           Show deployment status"
	@echo "  connect          Connect to client node via SSH"
	@echo "  ssh-node         Connect to network node (copies SSH key from client node)"
	@echo "  view-network     Attach to network deployment tmux session"
	@echo "  network-logs     View network deployment logs (tail -f)"
	@echo "  view-setup       View setup script logs (tail -f)"
	@echo "  destroy          Destroy client node (and network first)"
	@echo "  clean            Clean local files"
	@echo ""

# Configuration
CLIENT_SSH_KEY := client-deploy-key
CLIENT_USER := ubuntu
CLIENT_SETUP_SCRIPT := scripts/setup.sh
CLIENT_BENCH_SCRIPT := scripts/bench.sh
CLIENT_SCAN_BLOCKS_SCRIPT := scripts/scan-blocks.sh
NETWORK_TERRAFORM_DIR := ../network
FASTEVM_BRANCH ?= terraform-gravity

# Use terraform.tfvars if it exists (Terraform auto-loads it, but we can be explicit)
# Otherwise fall back to environment variables
TF_VARS = $(if $(wildcard terraform.tfvars),-var-file=terraform.tfvars,\
          -var="project_id=$(PROJECT_ID)" -var="region=$(REGION)" -var="zone=$(ZONE)" \
          -var="client_project_name=$(CLIENT_PROJECT_NAME)" \
          -var="client_network_name_prefix=$(CLIENT_NETWORK_NAME_PREFIX)" \
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
	@$(MAKE) copy-files
	@echo "✅ Step 1 completed: Client node is ready"
	@echo "📝 Setup script is available at: ~/setup.sh"
	@echo "📝 Bench script is available at: ~/bench.sh"

copy-files: ## Copy setup scripts to client node
	@echo "⏳ Waiting for SSH to be available..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found."; \
		exit 1; \
	fi; \
	echo "Client node IP: $$CLIENT_IP"; \
	bash scripts/wait-for-ssh.sh "$$CLIENT_IP" "$(CLIENT_SSH_KEY)" "$(CLIENT_USER)" || exit 1; \
	echo ""
	@echo "📦 Copying setup script to client node..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_SETUP_SCRIPT) $(CLIENT_BENCH_SCRIPT) $(CLIENT_SCAN_BLOCKS_SCRIPT) $(CLIENT_USER)@$$CLIENT_IP:~/ || { \
		echo "❌ Failed to copy setup.sh, bench.sh and scan-blocks.sh"; exit 1; \
	}


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

startup: transfer-creds
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	echo "🔧 Starting setup script in tmux session on client node..."; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	echo "Killing all tmux sessions..."; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP \
		"tmux kill-server 2>/dev/null || true; pkill -9 tmux 2>/dev/null || true; sleep 1" || true; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP \
		"tmux new-session -d -s fastevm-setup 'FASTEVM_BRANCH=$(FASTEVM_BRANCH) bash ~/setup.sh 2>&1 | tee ~/setup.log' || true"; \
	echo "✅ Setup script started in tmux session 'fastevm-setup'"; \
	echo "📝 Log file: ~/setup.log on client node"; \
	echo ""; \
	echo "📊 Showing progress (Ctrl+C to stop watching, session continues in background):"; \
	sleep 2; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'tail -f ~/setup.log 2>/dev/null || (echo "Waiting for log file..." && sleep 1 && tail -f ~/setup.log)' || true; \
	echo ""; \
	echo "✅ Setup script is running in tmux session 'fastevm-setup'"; \
	echo "📋 To view progress: make view-setup"; \
	echo "📋 To attach to session: ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'tmux attach -t fastevm-setup'"; \
	echo "📋 To view log file: ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'tail -f ~/setup.log'"

bench:
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	echo "🔧 Starting benchmark in tmux session on client node..."; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP \
		"tmux kill-session -t fastevm-benchmark 2>/dev/null || true; \
		tmux new-session -d -s fastevm-benchmark 'FASTEVM_BRANCH=$(FASTEVM_BRANCH) bash ~/bench.sh start 2>&1 | tee ~/setup.log' || true"; \
	echo "✅ Benchmark started in tmux session 'fastevm-benchmark'"; 

show-log:
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'bash ~/bench.sh show-log' || true; \
	echo ""; 

scan-blocks:
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	echo "🔧 Starting scan-blocks script on client node $(CLIENT_IP)..."; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP "bash scan-blocks.sh";

view-benchmark: ## View benchmark script logs
	@echo "📜 Viewing benchmark script logs..."; \
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'tail -f ~/benchmark.log 2>/dev/null || echo "❌ Log file not found. Run make benchmark first."'

# Destroy network (from client node)
destroy_network: ## Destroy network from client node
	@echo "⚠️  Destroying network from client node..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	echo "Connecting to client node to destroy network..."; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP \
		"cd /opt/workspace/fastevm/terraform/network && make destroy" || { \
		echo "⚠️  Network destroy failed or network not deployed. Continuing..."; \
	}

# Destroy client node
destroy: destroy_network ## Destroy client node (and network first)
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
		CLIENT_NAME=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.name'); \
		CLIENT_ZONE=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.zone'); \
		if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
			echo "❌ Client node not found. Run 'make deploy' first."; \
		else \
			echo "Client Name: $$CLIENT_NAME"; \
			echo "Client IP: $$CLIENT_IP"; \
			echo "Zone: $$CLIENT_ZONE"; \
			INSTANCE_STATUS=$$(gcloud compute instances describe $$CLIENT_NAME --zone=$$CLIENT_ZONE --format="value(status)" 2>/dev/null || echo "UNKNOWN"); \
			echo "Instance Status: $$INSTANCE_STATUS"; \
			echo "SSH Command: ssh -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP"; \
			echo ""; \
			echo "Setup Status:"; \
			ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
			ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
			if [ "$$INSTANCE_STATUS" = "RUNNING" ]; then \
				SOURCE_CLONED=$$(ssh -o ConnectTimeout=5 -o BatchMode=yes -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'test -d /opt/workspace/fastevm && echo "yes" || echo "no"' 2>/dev/null || echo "no"); \
				if [ "$$SOURCE_CLONED" = "yes" ]; then \
					echo "✅ Source code cloned to /opt/workspace/fastevm"; \
					NETWORK_READY=$$(ssh -o ConnectTimeout=5 -o BatchMode=yes -i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'test -d /opt/workspace/fastevm/terraform/network && echo "yes" || echo "no"' 2>/dev/null || echo "no"); \
					if [ "$$NETWORK_READY" = "yes" ]; then \
						echo "✅ Network terraform directory ready"; \
					else \
						echo "⏳ Network terraform directory not found"; \
					fi; \
				else \
					echo "⏳ Source code not yet cloned to /opt/workspace/fastevm"; \
					echo "   (SSH may not be ready yet if instance just started)"; \
				fi; \
			else \
				echo "⚠️  Instance is not RUNNING (status: $$INSTANCE_STATUS)"; \
				echo "   Wait for instance to be RUNNING before checking setup status"; \
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

# Connect to network node (copies SSH key from client node first)
ssh-node: ## Connect to network node (usage: make ssh-node NODE=1, or just make ssh-node for node 1)
	@NODE_NUM=$${NODE:-1}; \
	if [ -z "$$NODE_NUM" ] || [ "$$NODE_NUM" -lt 1 ]; then \
		echo "❌ Invalid node number. Node must be >= 1"; \
		exit 1; \
	fi; \
	echo "📡 Getting client node IP..."; \
	CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	NETWORK_KEY_PATH="/opt/workspace/fastevm/terraform/network/fastevm-deploy-key"; \
	LOCAL_NETWORK_KEY="network-deploy-key"; \
	echo "📥 Copying network SSH key from client node..."; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP:$$NETWORK_KEY_PATH $$LOCAL_NETWORK_KEY || { \
		echo "❌ Failed to copy network SSH key from client node."; \
		echo "   Make sure the network has been deployed from the client node."; \
		exit 1; \
	}; \
	chmod 600 $$LOCAL_NETWORK_KEY; \
	echo "✅ Network SSH key copied to $$LOCAL_NETWORK_KEY"; \
	echo "📡 Getting network node IPs from client node..."; \
	NETWORK_IPS=$$(ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP \
		"cd /opt/workspace/fastevm/terraform/network && terraform output -json instance_external_ips 2>/dev/null | jq -r '.[]' 2>/dev/null" || echo ""); \
	if [ -z "$$NETWORK_IPS" ]; then \
		echo "❌ Failed to get network node IPs from client node."; \
		echo "   Make sure the network has been deployed from the client node."; \
		rm -f $$LOCAL_NETWORK_KEY; \
		exit 1; \
	fi; \
	IP=$$(echo "$$NETWORK_IPS" | sed -n "$${NODE_NUM}p"); \
	if [ -z "$$IP" ] || [ "$$IP" = "null" ]; then \
		echo "❌ Network node $$NODE_NUM not found"; \
		echo ""; \
		echo "Available network nodes:"; \
		n=1; \
		for ip in $$NETWORK_IPS; do \
			echo "  Node $$n: $$ip"; \
			n=$$((n+1)); \
		done; \
		rm -f $$LOCAL_NETWORK_KEY; \
		exit 1; \
	fi; \
	echo "🔌 Connecting to network node $$NODE_NUM ($$IP)..."; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $$LOCAL_NETWORK_KEY ubuntu@$$IP; \
	SSH_EXIT=$$?; \
	rm -f $$LOCAL_NETWORK_KEY; \
	exit $$SSH_EXIT

# View network deployment tmux session
view-network: ## Attach to network deployment tmux session
	@echo "🔗 Attaching to network deployment tmux session..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'tmux attach -t fastevm-network || echo "❌ Tmux session not found. Run make deploy-network first."'

# View network deployment logs
network-logs: ## View network deployment logs
	@echo "📜 Viewing network deployment logs..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'tail -f ~/network-deployment.log 2>/dev/null || echo "❌ Log file not found. Run make deploy-network first."'

# View setup script logs
view-setup: ## View setup script logs (tail -f)
	@echo "📜 Viewing setup script logs..."
	@CLIENT_IP=$$($(TERRAFORM_CMD) output -json client_node_info | jq -r '.external_ip'); \
	if [ "$$CLIENT_IP" = "null" ] || [ -z "$$CLIENT_IP" ]; then \
		echo "❌ Client node not found. Run 'make deploy' first."; \
		exit 1; \
	fi; \
	ssh-keygen -R $$CLIENT_IP 2>/dev/null || true; \
	ssh-keyscan -H $$CLIENT_IP >> ~/.ssh/known_hosts 2>/dev/null || true; \
	ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-i $(CLIENT_SSH_KEY) $(CLIENT_USER)@$$CLIENT_IP 'tail -f ~/setup.log 2>/dev/null || echo "❌ Log file not found. Run make startup first."'


# Clean local files
clean: ## Clean local files
	@echo "🧹 Cleaning local files..."
	@rm -f *.tfstate *.tfstate.backup .terraform.lock.hcl
	@rm -f client-deploy-key client-deploy-key.pub
	@rm -f network-deploy-key
	@rm -rf .terraform/
	@echo "✅ Local files cleaned"

# Show outputs
outputs: ## Show all outputs
	@echo "📋 Client Node Outputs"
	@echo "======================"
	@$(TERRAFORM_CMD) output
