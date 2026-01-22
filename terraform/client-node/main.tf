# FastEVM Client Node Terraform Configuration
# This creates a standalone client node for deploying FastEVM networks and executing client code
# Note: This uses a separate network configuration from the main network deployment

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

# Provider configuration for client node deployment
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

# Create client-specific network (separate from main network deployment)
# Uses client-specific naming to avoid conflicts with main network
resource "google_compute_network" "client_network" {
  name                    = "${var.client_network_name_prefix}-network"
  auto_create_subnetworks = false
  description             = "FastEVM client node network for deploying and testing networks"

  lifecycle {
    ignore_changes = [
      # Ignore changes if network already exists
    ]
  }
}

# Create client-specific subnet (separate from main network deployment)
# Uses client-specific naming to avoid conflicts with main network
resource "google_compute_subnetwork" "client_subnet" {
  name          = "${var.client_network_name_prefix}-subnet"
  ip_cidr_range = var.client_subnet_cidr
  region        = var.region
  network       = google_compute_network.client_network.id

  secondary_ip_range {
    range_name    = "pods"
    ip_cidr_range = "10.1.1.0/24"
  }

  secondary_ip_range {
    range_name    = "services"
    ip_cidr_range = "10.1.2.0/24"
  }

  lifecycle {
    ignore_changes = [
      # Ignore changes if subnet already exists
    ]
  }
}

# Create client-specific firewall rules (separate from main network deployment)
# Uses client-specific naming to avoid conflicts with main network
resource "google_compute_firewall" "client_internal" {
  name    = "${var.client_network_name_prefix}-internal"
  network = google_compute_network.client_network.name

  allow {
    protocol = "tcp"
    ports    = ["22", "80", "443", "8545", "8546", "8551", "26657", "30303"]
  }

  allow {
    protocol = "udp"
    ports    = ["26657", "30303"]
  }

  source_ranges = [var.client_subnet_cidr]
  target_tags   = ["${var.client_network_name_prefix}-node"]

  lifecycle {
    ignore_changes = [
      # Ignore changes if firewall rule already exists
    ]
  }
}

resource "google_compute_firewall" "client_external" {
  name    = "${var.client_network_name_prefix}-external"
  network = google_compute_network.client_network.name

  allow {
    protocol = "tcp"
    ports    = ["22", "80", "443", "8545", "8546", "8551", "26657", "30303"]
  }

  allow {
    protocol = "udp"
    ports    = ["26657", "30303"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["${var.client_network_name_prefix}-node"]

  lifecycle {
    ignore_changes = [
      # Ignore changes if firewall rule already exists
    ]
  }
}

# Service account for client node
resource "google_service_account" "client_service_account" {
  account_id   = "${var.client_project_name}-sa"
  display_name = "FastEVM Client Service Account"
  description  = "Service account for FastEVM client node operations"
}

# Create IAM binding for service account
resource "google_project_iam_binding" "client_sa_binding" {
  project = var.project_id
  role    = "roles/compute.instanceAdmin"

  members = [
    "serviceAccount:${google_service_account.client_service_account.email}",
  ]
}

# Client node instance - standalone node for deploying networks and executing client code
resource "google_compute_instance" "client_node" {
  name         = var.client_project_name
  machine_type = var.client_machine_type
  zone         = var.zone

  # Allow stopping instances for updates (required for machine type changes)
  allow_stopping_for_update = true

  tags = ["${var.client_network_name_prefix}-node", "${var.client_network_name_prefix}-client"]

  boot_disk {
    initialize_params {
      image = var.image
      size  = var.client_disk_size
      type  = var.disk_type
    }
  }

  network_interface {
    network    = google_compute_network.client_network.id
    subnetwork = google_compute_subnetwork.client_subnet.id
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
    type        = "client-node"
    purpose     = "network-deployment"
  }
}

# Output client node information
output "client_node_info" {
  value = {
    name         = google_compute_instance.client_node.name
    external_ip  = google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip
    internal_ip  = google_compute_instance.client_node.network_interface[0].network_ip
    zone         = google_compute_instance.client_node.zone
    network_name = google_compute_network.client_network.name
    subnet_cidr  = google_compute_subnetwork.client_subnet.ip_cidr_range
    ssh_command  = "ssh -i ${path.module}/client-deploy-key ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}"
  }
  description = "Client node connection information"
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
    network_name   = google_compute_network.client_network.name
    subnet_name    = google_compute_subnetwork.client_subnet.name
    subnet_cidr    = google_compute_subnetwork.client_subnet.ip_cidr_range
    network_type   = "standalone-client-network"
    firewall_rules = [google_compute_firewall.client_internal.name, google_compute_firewall.client_external.name]
    note           = "This is a separate network from the main FastEVM network deployment. Used for client node operations only."
  }
  description = "Client node network information - standalone network for client operations"
}
