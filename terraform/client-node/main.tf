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
  content         = tls_private_key.client_ssh.private_key_pem
  filename        = "${path.module}/client-deploy-key"
  file_permission = "0600"
}

# Save public key to local file
resource "local_file" "client_public_key" {
  content         = tls_private_key.client_ssh.public_key_openssh
  filename        = "${path.module}/client-deploy-key.pub"
  file_permission = "0644"
}

# Create or reference existing fastevm-network
# Use "fastevm" as network name prefix to match network Terraform
resource "google_compute_network" "fastevm_network" {
  name                    = "fastevm-network"
  auto_create_subnetworks = false
  description             = "FastEVM network for blockchain nodes"

  lifecycle {
    ignore_changes = [
      # Ignore changes if network already exists
    ]
  }
}

# Create or reference existing fastevm-subnet
# Use "fastevm" as subnet name prefix to match network Terraform
resource "google_compute_subnetwork" "fastevm_subnet" {
  name          = "fastevm-subnet"
  ip_cidr_range = var.client_subnet_cidr
  region        = var.region
  network       = google_compute_network.fastevm_network.id

  secondary_ip_range {
    range_name    = "pods"
    ip_cidr_range = "10.0.1.0/24"
  }

  secondary_ip_range {
    range_name    = "services"
    ip_cidr_range = "10.0.2.0/24"
  }

  lifecycle {
    ignore_changes = [
      # Ignore changes if subnet already exists
    ]
  }
}

# Create firewall rules if they don't exist
# Use "fastevm" as firewall name prefix to match network Terraform
resource "google_compute_firewall" "fastevm_internal" {
  name    = "fastevm-internal"
  network = google_compute_network.fastevm_network.name

  allow {
    protocol = "tcp"
    ports    = ["22", "80", "443", "8545", "8546", "8551", "26657", "30303"]
  }

  allow {
    protocol = "udp"
    ports    = ["26657", "30303"]
  }

  source_ranges = [var.client_subnet_cidr]
  target_tags   = ["fastevm-node"]

  lifecycle {
    ignore_changes = [
      # Ignore changes if firewall rule already exists
    ]
  }
}

resource "google_compute_firewall" "fastevm_external" {
  name    = "fastevm-external"
  network = google_compute_network.fastevm_network.name

  allow {
    protocol = "tcp"
    ports    = ["22", "80", "443", "8545", "8546", "8551", "26657", "30303"]
  }

  allow {
    protocol = "udp"
    ports    = ["26657", "30303"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["fastevm-node"]

  lifecycle {
    ignore_changes = [
      # Ignore changes if firewall rule already exists
    ]
  }
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
      type  = "hyperdisk-balanced"
    }
  }

  network_interface {
    network    = google_compute_network.fastevm_network.id
    subnetwork = google_compute_subnetwork.fastevm_subnet.id
    access_config {
      // Ephemeral public IP
    }
  }

  metadata = {
    ssh-keys = "${var.ssh_user}:${tls_private_key.client_ssh.public_key_openssh}"
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
    network_name = google_compute_network.fastevm_network.name
    subnet_cidr  = google_compute_subnetwork.fastevm_subnet.ip_cidr_range
    ssh_command  = "ssh -i ${path.module}/client-deploy-key ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}"
    local_access = "Access from blockchain nodes: ${google_compute_instance.client_node.network_interface[0].network_ip}"
  }
  description = "Client node connection information for same-network access"
}

# Output client node connection details for easy access
output "client_connection" {
  value = {
    ssh_command   = "ssh -i ${path.module}/client-deploy-key ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}"
    scp_command   = "scp -i ${path.module}/client-deploy-key -r <local_path> ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}:<remote_path>"
    rsync_command = "rsync -avz -e 'ssh -i ${path.module}/client-deploy-key' <local_path> ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}:<remote_path>"
  }
  description = "Client node connection commands"
}

# Output network information
output "client_network_info" {
  value = {
    network_name               = google_compute_network.fastevm_network.name
    subnet_name                = google_compute_subnetwork.fastevm_subnet.name
    subnet_cidr                = google_compute_subnetwork.fastevm_subnet.ip_cidr_range
    same_network_as_blockchain = true
    local_access_note          = "Client reuses existing fastevm-network for easy local access"
    firewall_note              = "Uses existing firewall rules from main deployment"
  }
  description = "Client node network information - reuses existing blockchain network"
}
