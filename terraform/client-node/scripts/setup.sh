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
    jq \
    curl \
    gnupg \
    unzip 

# Install Terraform
if ! command -v terraform &> /dev/null; then
    echo "Installing Terraform..."
    TERRAFORM_VERSION="1.6.0"
    curl -fsSL -o /tmp/terraform_${TERRAFORM_VERSION}_linux_amd64.zip https://releases.hashicorp.com/terraform/${TERRAFORM_VERSION}/terraform_${TERRAFORM_VERSION}_linux_amd64.zip
    unzip -q /tmp/terraform_${TERRAFORM_VERSION}_linux_amd64.zip -d /tmp
    sudo mv /tmp/terraform /usr/local/bin/
    rm /tmp/terraform_${TERRAFORM_VERSION}_linux_amd64.zip
    terraform --version
else
    echo "Terraform already installed: $(terraform --version)"
fi

# Install Google Cloud SDK
if ! command -v gcloud &> /dev/null; then
    echo "Installing Google Cloud SDK..."
    curl -fsSL https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo gpg --dearmor -o /usr/share/keyrings/cloud.google.gpg
    echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" | sudo tee -a /etc/apt/sources.list.d/google-cloud-sdk.list
    sudo apt-get update && sudo apt-get install -y google-cloud-cli
    gcloud --version
else
    echo "Google Cloud SDK already installed: $(gcloud --version | head -n 1)"
fi

# Verify jq installation
if ! command -v jq &> /dev/null; then
    echo "Error: jq installation failed"
    exit 1
else
    echo "jq installed: $(jq --version)"
fi

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

: "${FASTEVM_BRANCH:=gravity}"
echo "Using FastEVM branch: $FASTEVM_BRANCH"

REPO_FASTEVM="https://github.com/scalarorg/fastevm.git"
REPO_GRAVITY_GENESIS_CONTRACT="https://github.com/scalarorg/gravity-genesis-contract.git"
REPO_BRANCH_FASTEVM=$FASTEVM_BRANCH
REPO_BRANCH_GRAVITY_GENESIS_CONTRACT="main"

clone_repository() {
    REPO_URL=$1
    REPO_BRANCH=$2
    # Extract repo_name from repo_url (e.g., https://github.com/user/repo.git -> repo)
    REPO_NAME=$(basename "$REPO_URL" .git)
    cd $WORKSPACE_DIR

    if [ -d "$REPO_NAME/.git" ] || [ -d "$REPO_NAME" ]; then
        echo "$REPO_NAME directory already exists. Updating repository..."
        cd $REPO_NAME
        git config pull.rebase true
        git fetch origin

        if git show-ref --verify --quiet "refs/heads/$REPO_BRANCH"; then
            git checkout "$REPO_BRANCH"
        else
            git checkout -b "$REPO_BRANCH" "origin/$REPO_BRANCH"
        fi

        git pull origin "$REPO_BRANCH"

        echo "Updating all submodules..."
        git submodule sync --recursive
        git submodule update --init --recursive
    else
        echo "Cloning $REPO_NAME repository to $REPO_NAME..."
        if [ "$REPO_NAME" = "fastevm" ]; then
            git clone $REPO_URL "$REPO_NAME"
        else
            git clone $REPO_URL "$REPO_NAME"
        fi
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
    cargo build --release
    sudo cp target/release/gravity-genesis /usr/local/bin/gravity-genesis
}
# Create /opt/fastevm directory
sudo mkdir -p $WORKSPACE_DIR
sudo chown $USER:$USER $WORKSPACE_DIR

clone_repository $REPO_FASTEVM $REPO_BRANCH_FASTEVM
#build_fastevm
clone_repository $REPO_GRAVITY_GENESIS_CONTRACT $REPO_BRANCH_GRAVITY_GENESIS_CONTRACT
build_gravity_genesis_contract



