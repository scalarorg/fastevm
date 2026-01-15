# FastEVM GCP Infrastructure
# This Terraform configuration creates 4 nodes for FastEVM network deployment

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

provider "google" {
  project = var.project_id
  region  = var.region
  zone    = var.zone
}

# Generate SSH key pair for deployment
resource "tls_private_key" "fastevm_ssh" {
  algorithm = "RSA"
  rsa_bits  = 4096
}

# Save private key to local file
resource "local_file" "fastevm_private_key" {
  content         = tls_private_key.fastevm_ssh.private_key_pem
  filename        = "${path.module}/fastevm-deploy-key"
  file_permission = "0600"
}

# Save public key to local file
resource "local_file" "fastevm_public_key" {
  content         = tls_private_key.fastevm_ssh.public_key_openssh
  filename        = "${path.module}/fastevm-deploy-key.pub"
  file_permission = "0644"
}

# Reference existing network (created by client-node)
data "google_compute_network" "fastevm_network" {
  name = "fastevm-network"
}

# Reference existing subnet (created by client-node)
data "google_compute_subnetwork" "fastevm_subnet" {
  name   = "fastevm-subnet"
  region = var.region
}

# Firewall rules are created by client-node, so we don't need to create them here
# They will be automatically available for all nodes in the network

# No startup script - nodes will be initialized via deploy.sh after creation
# This allows for better control, logging, and error handling during deployment

# Create compute instances
resource "google_compute_instance" "fastevm_nodes" {
  count        = var.node_count
  name         = "${var.project_name}-node-${count.index + 1}"
  machine_type = var.machine_type
  zone         = var.zone

  # Allow stopping instances for updates (required for machine type changes)
  allow_stopping_for_update = true

  # Lifecycle rules to handle existing instances
  lifecycle {
    create_before_destroy = false
    ignore_changes = [
      # Ignore changes to metadata that might be updated externally
      metadata["ssh-keys"],
    ]
  }

  tags = ["fastevm-node"]

  boot_disk {
    initialize_params {
      image = var.image
      size  = var.disk_size
      type  = var.disk_type
    }
  }

  network_interface {
    network    = data.google_compute_network.fastevm_network.id
    subnetwork = data.google_compute_subnetwork.fastevm_subnet.id
    access_config {
      // Ephemeral public IP (automatically assigned)
    }
  }

  metadata = {
    node-index   = count.index
    node-count   = var.node_count
    project-name = var.project_name
    ssh-keys     = "ubuntu:${tls_private_key.fastevm_ssh.public_key_openssh}"
  }

  service_account {
    email  = google_service_account.fastevm_sa.email
    scopes = ["cloud-platform"]
  }
}

# Create service account
resource "google_service_account" "fastevm_sa" {
  account_id   = "${var.project_name}-sa"
  display_name = "FastEVM Service Account"
  description  = "Service account for FastEVM nodes"
}

# Create IAM binding for service account
resource "google_project_iam_binding" "fastevm_sa_binding" {
  project = var.project_id
  role    = "roles/compute.instanceAdmin"

  members = [
    "serviceAccount:${google_service_account.fastevm_sa.email}",
  ]
}

# Create load balancer for external access
resource "google_compute_global_address" "fastevm_ip" {
  name = "${var.project_name}-ip"
}

resource "google_compute_health_check" "fastevm_health_check" {
  name                = "${var.project_name}-health-check"
  check_interval_sec  = 5
  timeout_sec         = 5
  healthy_threshold   = 2
  unhealthy_threshold = 3

  http_health_check {
    port         = 8545
    request_path = "/"
  }
}

resource "google_compute_backend_service" "fastevm_backend" {
  name        = "${var.project_name}-backend"
  protocol    = "HTTP"
  port_name   = "http"
  timeout_sec = 10

  backend {
    group = google_compute_instance_group.fastevm_group.id
  }

  health_checks = [google_compute_health_check.fastevm_health_check.id]

  depends_on = [
    google_compute_instance_group.fastevm_group
  ]
}

resource "google_compute_instance_group" "fastevm_group" {
  name        = "${var.project_name}-group"
  description = "FastEVM node group"
  zone        = var.zone

  instances = google_compute_instance.fastevm_nodes[*].id

  named_port {
    name = "http"
    port = 8545
  }

  named_port {
    name = "engine-api"
    port = 8551
  }

  depends_on = [
    google_compute_instance.fastevm_nodes
  ]

  lifecycle {
    create_before_destroy = true
  }
}

resource "google_compute_url_map" "fastevm_url_map" {
  name            = "${var.project_name}-url-map"
  default_service = google_compute_backend_service.fastevm_backend.id
}

resource "google_compute_target_http_proxy" "fastevm_proxy" {
  name    = "${var.project_name}-proxy"
  url_map = google_compute_url_map.fastevm_url_map.id
}

resource "google_compute_global_forwarding_rule" "fastevm_forwarding_rule" {
  name       = "${var.project_name}-forwarding-rule"
  target     = google_compute_target_http_proxy.fastevm_proxy.id
  port_range = "80"
  ip_address = google_compute_global_address.fastevm_ip.address
}
