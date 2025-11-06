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

# Reference existing fastevm-network (created by main deployment)
data "google_compute_network" "fastevm_network" {
  name = "fastevm-network"
}

# Reference existing fastevm-subnet (created by main deployment)
data "google_compute_subnetwork" "fastevm_subnet" {
  name   = "fastevm-subnet"
  region = var.region
}

# Service account for client node
resource "google_service_account" "client_service_account" {
  account_id   = "${var.project_name}-sa"
  display_name = "FastEVM Client Service Account"
  description  = "Service account for FastEVM client node operations"
}

# Create IAM binding for service account (same pattern as main nodes)
resource "google_project_iam_binding" "client_sa_binding" {
  project = var.project_id
  role    = "roles/compute.instanceAdmin"

  members = [
    "serviceAccount:${google_service_account.client_service_account.email}",
  ]
}

# Client node instance (same pattern as main nodes)
resource "google_compute_instance" "client_node" {
  name         = var.project_name
  machine_type = var.client_machine_type
  zone         = var.zone

  # Allow stopping instances for updates (required for machine type changes)
  allow_stopping_for_update = true

  tags = ["fastevm-node", "fastevm-client"]

  boot_disk {
    initialize_params {
      image = var.image
      size  = 40
      type  = "pd-standard"
    }
  }

  network_interface {
    network    = data.google_compute_network.fastevm_network.id
    subnetwork = data.google_compute_subnetwork.fastevm_subnet.id
    access_config {
      // Ephemeral public IP
    }
  }

  metadata = {
    ssh-keys = "${var.ssh_user}:${tls_private_key.client_ssh.public_key_openssh}"
    startup-script = <<-EOF
      #!/bin/bash
      set -e
      
      # Logging function
      log() {
          echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a /var/log/client-startup.log
      }
      
      log "Starting FastEVM client node startup script..."
      
      # Update system packages
      log "Updating system packages..."
      apt-get update -y
      apt-get upgrade -y
      
      # Install required packages (including C compiler)
      log "Installing required packages..."
      apt-get install -y \
          curl \
          wget \
          git \
          build-essential \
          pkg-config \
          libssl-dev \
          libclang-dev \
          llvm-dev \
          cmake \
          jq \
          htop \
          vim \
          unzip \
          software-properties-common \
          apt-transport-https \
          ca-certificates \
          gnupg \
          lsb-release \
          openssh-client
      
      # Wait for package installation to fully complete
      log "Waiting for package installation to complete..."
      sleep 5
      
      # Refresh environment and verify compilers
      log "Refreshing environment..."
      export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
      hash -r
      
      # Verify C compiler is available with retry logic
      log "Verifying C compiler installation..."
      for i in {1..5}; do
          if command -v cc &> /dev/null && command -v gcc &> /dev/null; then
              log "C compiler verification passed (attempt $i)"
              break
          else
              log "C compiler not found, retrying... (attempt $i/5)"
              sleep 2
              hash -r
          fi
      done
      
      # Final verification
      if ! command -v cc &> /dev/null; then
          log "ERROR: C compiler (cc) not found after installation"
          exit 1
      fi
      
      if ! command -v gcc &> /dev/null; then
          log "ERROR: GCC compiler not found after installation"
          exit 1
      fi
      
      # Create completion marker for other scripts
      log "Creating startup completion marker..."
      touch /var/log/client-startup-complete
      
      # Final verification that everything is working
      log "Performing final system verification..."
      if command -v cc &> /dev/null && command -v gcc &> /dev/null; then
          log "Final verification: Compilers are available"
      else
          log "WARNING: Compilers may not be properly available"
      fi
      
      log "Package installation completed successfully!"
      log "C compiler verification passed"
      log "Startup script completed - ready for compilation"
      EOF
  }

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

# Note: Firewall rules are managed by the main deployment
# The existing fastevm-network already has the necessary firewall rules:
# - fastevm-internal: for subnet-to-subnet communication
# - fastevm-external: for external access
# Client node uses the same tags (fastevm-node) so it will be covered by existing rules

# Output client node information
output "client_node_info" {
  value = {
    name         = google_compute_instance.client_node.name
    external_ip  = google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip
    internal_ip  = google_compute_instance.client_node.network_interface[0].network_ip
    zone         = google_compute_instance.client_node.zone
    network_name = data.google_compute_network.fastevm_network.name
    subnet_cidr  = data.google_compute_subnetwork.fastevm_subnet.ip_cidr_range
    ssh_command  = "ssh -i ${path.module}/client-deploy-key ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}"
    local_access = "Access from blockchain nodes: ${google_compute_instance.client_node.network_interface[0].network_ip}"
  }
  description = "Client node connection information for same-network access"
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
    network_name = data.google_compute_network.fastevm_network.name
    subnet_name  = data.google_compute_subnetwork.fastevm_subnet.name
    subnet_cidr  = data.google_compute_subnetwork.fastevm_subnet.ip_cidr_range
    same_network_as_blockchain = true
    local_access_note = "Client reuses existing fastevm-network for easy local access"
    firewall_note = "Uses existing firewall rules from main deployment"
  }
  description = "Client node network information - reuses existing blockchain network"
}
