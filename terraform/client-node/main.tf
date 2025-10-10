# FastEVM Client Node Terraform Configuration
# This creates a standalone client node for testing FastEVM networks

terraform {
  required_version = ">= 1.0"
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
    tls = {
      source  = "hashicorp/tls"
      version = "~> 4.0"
    }
    local = {
      source  = "hashicorp/local"
      version = "~> 2.0"
    }
  }
}

# Use the same provider configuration as main deployment
provider "google" {
  project = var.project_id
  region  = var.region
  zone    = var.zone
}

# Generate SSH key pair for client node deployment
resource "tls_private_key" "client_ssh" {
  algorithm = "RSA"
  rsa_bits  = 4096
}

# Save private key to local file
resource "local_file" "client_private_key" {
  content  = tls_private_key.client_ssh.private_key_pem
  filename = "${path.module}/client-deploy-key"
  file_permission = "0600"
}

# Save public key to local file
resource "local_file" "client_public_key" {
  content  = tls_private_key.client_ssh.public_key_openssh
  filename = "${path.module}/client-deploy-key.pub"
  file_permission = "0644"
}

# Create VPC network for client node (separate from main network)
resource "google_compute_network" "client_network" {
  name                    = "fastevm-client-network"
  auto_create_subnetworks = false
  description             = "VPC network for FastEVM client node"
}

# Create subnet for client node
resource "google_compute_subnetwork" "client_subnet" {
  name          = "fastevm-client-subnet"
  ip_cidr_range = var.client_subnet_cidr
  region        = var.region
  network       = google_compute_network.client_network.id
  description   = "Subnet for FastEVM client node"
}

# Service account for client node
resource "google_service_account" "client_service_account" {
  account_id   = "fastevm-client-sa"
  display_name = "FastEVM Client Service Account"
  description  = "Service account for FastEVM client node operations"
}

# Client node instance
resource "google_compute_instance" "client_node" {
  name         = "fastevm-client"
  machine_type = var.client_machine_type
  zone         = var.zone

  tags = ["fastevm-client", "test-client"]

  boot_disk {
    initialize_params {
      image = var.image
      size  = var.client_disk_size
      type  = var.disk_type
    }
  }

  network_interface {
    network    = google_compute_network.client_network.name
    subnetwork = google_compute_subnetwork.client_subnet.name
    access_config {
      // Ephemeral public IP
    }
  }

  metadata = {
    ssh-keys = "${var.ssh_user}:${tls_private_key.client_ssh.public_key_openssh}"
  }

  metadata_startup_script = <<-EOF
#!/bin/bash

# FastEVM Client Node Setup Script
# This script sets up a client node for testing FastEVM networks

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging function
log() {
    echo -e "$${BLUE}[$$(date '+%Y-%m-%d %H:%M:%S')]$${NC} $1" | tee -a /var/log/client-setup.log
}

log "Starting FastEVM client node setup..."

# Update system packages
log "Updating system packages..."
apt-get update -y

# Install essential packages
log "Installing essential packages..."
apt-get install -y curl wget git build-essential pkg-config libssl-dev

# Install Rust for ubuntu user
log "Installing Rust..."
sudo -u ubuntu bash -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
sudo -u ubuntu bash -c 'source ~/.cargo/env && echo "source ~/.cargo/env" >> ~/.bashrc'

# Clone FastEVM repository
log "Cloning FastEVM repository..."
cd /home/ubuntu
git clone https://github.com/scalar-labs/fastevm.git
cd fastevm

# Checkout specific branch if provided
if [ -n "${var.github_branch}" ]; then
    log "Checking out branch: ${var.github_branch}"
    git checkout ${var.github_branch}
fi

# Build the test binary
log "Building FastEVM test binary..."
cd testing/integration
sudo -u ubuntu bash -c 'source ~/.cargo/env && cargo build --release --bin fastevm-test'

# Create test configuration directory
log "Creating test configuration..."
mkdir -p /home/ubuntu/test-config

# Create test configuration template
cat > /home/ubuntu/test-config/test.env.example << 'CONFIG_EOF'
# FastEVM Test Configuration Example
# Copy this file to test.env and update with your actual values

# RPC Endpoints (update with actual node IPs)
RPC_URL1=http://10.0.0.10:8545
RPC_URL2=http://10.0.0.11:8545
RPC_URL3=http://10.0.0.12:8545
RPC_URL4=http://10.0.0.13:8545

# Network Configuration
CHAIN_ID=202501

# Batch Transaction Test Parameters
TEST_SENDER_COUNT=100
TEST_TRANSACTION_COUNT=1
TEST_TRANSACTION_VALUE=1000000000000000
TEST_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

# Test Timing Configuration
TEST_WAITING_TIME_SECONDS=30
TEST_FETCH_NONCE=false
CONFIG_EOF

# Create default test configuration
cp /home/ubuntu/test-config/test.env.example /home/ubuntu/test-config/test.env

# Set proper permissions
chown -R ubuntu:ubuntu /home/ubuntu/test-config
chown -R ubuntu:ubuntu /home/ubuntu/fastevm

# Create completion marker
touch /var/log/client-setup-complete

log "FastEVM client node setup completed successfully!"
log "Test binary location: /home/ubuntu/fastevm/testing/integration/target/release/fastevm-test"
log "Configuration location: /home/ubuntu/test-config/"
log "Next steps:"
log "  1. Update RPC URLs in /home/ubuntu/test-config/test.env"
log "  2. Run tests using the fastevm-test binary"
EOF

  service_account {
    email  = google_service_account.client_service_account.email
    scopes = ["cloud-platform"]
  }

  labels = {
    environment = "testing"
    project     = "fastevm"
    managed-by  = "terraform"
    role        = "client"
    type        = "test-node"
  }
}

# Firewall rule for client node SSH access
resource "google_compute_firewall" "client_ssh" {
  name    = "fastevm-client-ssh"
  network = google_compute_network.client_network.name

  allow {
    protocol = "tcp"
    ports    = ["22"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["fastevm-client"]
}

# Firewall rule for client node to access external networks (for testing)
resource "google_compute_firewall" "client_external_access" {
  name    = "fastevm-client-external"
  network = google_compute_network.client_network.name

  allow {
    protocol = "tcp"
    ports    = ["80", "443", "8545", "8546", "8551", "26657", "30303"]
  }

  source_tags = ["fastevm-client"]
}

# Output client node information
output "client_node_info" {
  value = {
    name         = google_compute_instance.client_node.name
    external_ip  = google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip
    internal_ip  = google_compute_instance.client_node.network_interface[0].network_ip
    zone         = google_compute_instance.client_node.zone
    ssh_command  = "ssh -i ${path.module}/client-deploy-key ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}"
  }
  description = "Client node connection information"
}

# Output client node connection details for easy access
output "client_connection" {
  value = {
    ssh_command = "ssh -i ${path.module}/client-deploy-key ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}"
    scp_command = "scp -i ${path.module}/client-deploy-key -r <local_path> ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}:<remote_path>"
    rsync_command = "rsync -avz -e 'ssh -i ${path.module}/client-deploy-key' <local_path> ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}:<remote_path>"
  }
  description = "Client node connection commands"
}

# Output network information
output "client_network_info" {
  value = {
    network_name = google_compute_network.client_network.name
    subnet_name  = google_compute_subnetwork.client_subnet.name
    subnet_cidr  = var.client_subnet_cidr
  }
  description = "Client node network information"
}
