#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="/opt/workspace"

sudo apt update
sudo apt install -y build-essential \
    clang \
    gcc \
    pkg-config \
    libclang-dev \
    llvm-dev \
    libssl-dev \
    libfontconfig1-dev \
    python3 \
    python3-pip \
    make \
    git \
    tmux \
    unzip \
    curl \
    jq

# Install Terraform
if ! command -v terraform &> /dev/null; then
    echo "Installing Terraform..."
    # Add HashiCorp GPG key (skip if already exists)
    if [ ! -f /usr/share/keyrings/hashicorp-archive-keyring.gpg ]; then
        wget -O - https://apt.releases.hashicorp.com/gpg | sudo gpg --dearmor -o /usr/share/keyrings/hashicorp-archive-keyring.gpg
    fi
    # Add HashiCorp repository (skip if already exists)
    if [ ! -f /etc/apt/sources.list.d/hashicorp.list ]; then
        echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/hashicorp-archive-keyring.gpg] https://apt.releases.hashicorp.com $(grep -oP '(?<=UBUNTU_CODENAME=).*' /etc/os-release || lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/hashicorp.list
    fi
    sudo apt update && sudo apt install -y terraform
    terraform version
    echo "✅ Terraform installed"
else
    echo "✅ Terraform already installed: $(terraform version | head -n 1)"
fi

# if ! command -v terraform &> /dev/null; then
#     echo "Installing Terraform..."
#     TERRAFORM_VERSION="1.6.0"
#     wget -q "https://releases.hashicorp.com/terraform/${TERRAFORM_VERSION}/terraform_${TERRAFORM_VERSION}_linux_amd64.zip"
#     unzip -q "terraform_${TERRAFORM_VERSION}_linux_amd64.zip"
#     sudo mv terraform /usr/local/bin/
#     rm "terraform_${TERRAFORM_VERSION}_linux_amd64.zip"
#     terraform version
#     echo "✅ Terraform installed"
# else
#     echo "✅ Terraform already installed: $(terraform version | head -n 1)"
# fi

# Install Google Cloud SDK if not already installed
if ! command -v gcloud &> /dev/null; then
    echo "Installing Google Cloud SDK..."
    # Install gcloud SDK non-interactively
    export CLOUDSDK_CORE_DISABLE_PROMPTS=1
    curl https://sdk.cloud.google.com | bash -s -- --disable-prompts
    # Add to PATH for current session
    export PATH="$HOME/google-cloud-sdk/bin:$PATH"
    # Add to .bashrc for future sessions
    echo 'export PATH="$HOME/google-cloud-sdk/bin:$PATH"' >> ~/.bashrc
    gcloud --version
    echo "✅ Google Cloud SDK installed"
else
    echo "✅ Google Cloud SDK already installed: $(gcloud --version | head -n 1)"
fi 
# Install rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
. "$HOME/.cargo/env" 
cargo --version
rustup toolchain install 1.88.0
rustup default 1.88.0

# Install foundry
curl -L https://foundry.paradigm.xyz | bash -x
export PATH="$HOME/.foundry/bin:$PATH"
foundryup
# Verify installation
forge --version

# Install node.js
export DEBIAN_FRONTEND=noninteractive && \
curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash - && \
sudo -E apt-get install -y nodejs

# Install python env
sudo -E apt-get install -y python3-venv python-is-python3

: "${FASTEVM_BRANCH:=gravity}"
echo "Using FastEVM branch: $FASTEVM_BRANCH"

REPO_FASTEVM="https://github.com/scalarorg/fastevm.git"
REPO_GRAVITY_GENESIS_CONTRACT="https://github.com/scalarorg/gravity-genesis-contract.git"
REPO_GRAVITY_BENCH="https://github.com/scalarorg/gravity_bench.git"
REPO_BRANCH_FASTEVM=$FASTEVM_BRANCH
REPO_BRANCH_GRAVITY_GENESIS_CONTRACT="main"
REPO_BRANCH_GRAVITY_BENCH="fastevm"
clone_repository() {
    REPO_URL=$1
    REPO_BRANCH=$2
    # Extract repo_name from repo_url (e.g., https://github.com/user/repo.git -> repo)
    REPO_NAME=$(basename "$REPO_URL" .git)
    cd $WORKSPACE_DIR

    if [ -d "$REPO_NAME/.git" ] || [ -d "$REPO_NAME" ]; then
        echo "$REPO_NAME directory already exists. Updating repository..."
        cd $REPO_NAME
        git restore .
        git config pull.rebase true
        git fetch origin

        if git show-ref --verify --quiet "refs/heads/$REPO_BRANCH"; then
            git checkout "$REPO_BRANCH"
        else
            git checkout -b "$REPO_BRANCH" "origin/$REPO_BRANCH"
        fi

        # Restore all changes if there are any unstaged or staged changes
        if ! git diff --quiet || ! git diff --cached --quiet; then
            echo "Restoring all changes before pulling..."
            git restore .
            git restore --staged . 2>/dev/null || true
            git reset --hard HEAD
        fi

        git pull origin "$REPO_BRANCH"

        echo "Updating all submodules..."
        git submodule sync --recursive
        git submodule update --init --recursive
    else
        echo "Cloning $REPO_NAME repository to $REPO_NAME..."
        git clone $REPO_URL "$REPO_NAME"
        cd $REPO_NAME

        git checkout "$REPO_BRANCH"

        echo "Initializing and updating all submodules..."
        git submodule update --init --recursive
    fi
}

# build_fastevm() {
#     REPO_NAME=$(basename "$REPO_FASTEVM" .git)
#     cd $WORKSPACE_DIR/$REPO_NAME
#     cargo build --release
#     sudo systemctl stop fastevm-execution || true
#     sudo cp target/release/fastevm-execution /usr/local/bin/fastevm-execution
#     sudo cp target/release/fastevm-cli /usr/local/bin/fastevm-cli
#     # cargo clean
#     cd modules/mysticeti
#     # cargo build --release
#     sudo systemctl stop fastevm-consensus || true
#     cargo build --release -p evm-consensus --bin evm-consensus
#     sudo cp target/release/evm-consensus /usr/local/bin/evm-consensus
#     # cargo clean
# }

build_gravity_genesis_contract() {
    REPO_NAME=$(basename "$REPO_GRAVITY_GENESIS_CONTRACT" .git)
    cd $WORKSPACE_DIR/$REPO_NAME
    forge build
    cargo build --release
    sudo cp target/release/gravity-genesis /usr/local/bin/gravity-genesis
}
build_gravity_bench() {
    local REPO_NAME=$(basename "$REPO_GRAVITY_BENCH" .git)
    cd $WORKSPACE_DIR/$REPO_NAME
    cargo build --release
    sudo cp target/release/gravity_bench /usr/local/bin/gravity_bench
}
start_network() {
    local REPO_NAME=$(basename "$REPO_FASTEVM" .git)
    cd $WORKSPACE_DIR/${REPO_NAME}/terraform/network
    echo "Starting network with branch: $FASTEVM_BRANCH"
    GITHUB_BRANCH=$FASTEVM_BRANCH make start-services
    echo "Network started successfully!"
}
start_bench() {
    local REPO_NAME=$(basename "$REPO_GRAVITY_BENCH" .git)
    local REPO_NAME_FASTEVM=$(basename "$REPO_FASTEVM" .git)
    local LOG_LEVEL=info
    local CHAIN_ID=7771625
    cd $WORKSPACE_DIR/$REPO_NAME
    
    # Read public IPs from file and replace localhost URLs in bench_config_fastevm.toml
    # Try multiple possible paths for public_ips.txt
    PUBLIC_IPS_FILE="$WORKSPACE_DIR/$REPO_NAME_FASTEVM/terraform/public_ips.txt"
    if [ -f "$PUBLIC_IPS_FILE" ]; then
        echo "Reading public IPs from $PUBLIC_IPS_FILE"
        # Read IPs into array
        mapfile -t IPS < <(cat "$PUBLIC_IPS_FILE" | grep -v '^$')
        
        if [ ${#IPS[@]} -gt 0 ]; then
            echo "Found ${#IPS[@]} IPs, updating bench_config.toml"
            
            if [ -f "bench_config.toml" ]; then
                
                # Build the new nodes array content in a temporary file
                TMP_NODES=$(mktemp)
                echo "nodes = [" > "$TMP_NODES"
                for IP in "${IPS[@]}"; do
                    echo "    { rpc_url = \"http://${IP}:8545\", chain_id = ${CHAIN_ID} }," >> "$TMP_NODES"
                done
                # Remove trailing comma from last line and add closing bracket
                sed -i '$ s/,$//' "$TMP_NODES"
                echo "]" >> "$TMP_NODES"
                
                # Use awk to replace the nodes array section
                awk '
                    BEGIN { 
                        in_nodes = 0
                        # Read the new nodes content
                        while ((getline line < "'"$TMP_NODES"'") > 0) {
                            new_nodes = new_nodes line "\n"
                        }
                        close("'"$TMP_NODES"'")
                    }
                    /^nodes = \[/ {
                        in_nodes = 1
                        printf "%s", new_nodes
                        next
                    }
                    in_nodes {
                        # Skip lines until we find the closing bracket
                        if (/^\]/) {
                            in_nodes = 0
                        }
                        next
                    }
                    { print }
                ' bench_config.toml > bench_config_fastevm.toml
                
                # Clean up temporary file
                rm -f "$TMP_NODES"
                
                echo "✓ Updated bench_config_fastevm.toml with real IPs"
            else
                echo "⚠️  bench_config_fastevm.toml not found, skipping IP replacement"
            fi
        else
            echo "⚠️  No IPs found in $PUBLIC_IPS_FILE, using default configuration"
            exit 1
        fi
    else
        echo "⚠️  public_ips.txt not found (checked: $PUBLIC_IPS_FILE), using default configuration"
        exit 1
    fi
    source ./setup.sh
    cargo run --bin gravity_bench --release -- --config bench_config_fastevm.toml --log-level ${LOG_LEVEL:-info}
}
# Create /opt/fastevm directory
sudo mkdir -p $WORKSPACE_DIR
sudo chown $USER:$USER $WORKSPACE_DIR

clone_repository $REPO_FASTEVM $REPO_BRANCH_FASTEVM
#build_fastevm
clone_repository $REPO_GRAVITY_GENESIS_CONTRACT $REPO_BRANCH_GRAVITY_GENESIS_CONTRACT
build_gravity_genesis_contract
clone_repository $REPO_GRAVITY_BENCH $REPO_BRANCH_GRAVITY_BENCH
build_gravity_bench
echo "Starting network with prepared terraform and gcloud credentials..."
start_network
echo "Network started successfully!"

