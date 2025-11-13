# Gravity Reth Benchmark Terraform Configuration
# This creates 2 GCloud nodes:
# 1. Execution node: Clones gravity-reth, builds it, and runs dev-node.sh
# 2. Client node: Sets up docker, clones gravity_bench, and starts docker container

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
    null = {
      source  = "hashicorp/null"
      version = "~> 3.0"
    }
  }
}

provider "google" {
  project = var.project_id
  region  = var.region
  zone    = var.zone
}

# Generate SSH key pair for deployment
resource "tls_private_key" "gravity_ssh" {
  algorithm = "RSA"
  rsa_bits  = 4096
}

# Save private key to local file
resource "local_file" "gravity_private_key" {
  content         = tls_private_key.gravity_ssh.private_key_pem
  filename        = "${path.module}/gravity-deploy-key"
  file_permission = "0600"
}

# Save public key to local file
resource "local_file" "gravity_public_key" {
  content         = tls_private_key.gravity_ssh.public_key_openssh
  filename        = "${path.module}/gravity-deploy-key.pub"
  file_permission = "0644"
}

# Create VPC network
resource "google_compute_network" "gravity_network" {
  name                    = "${var.project_name}-network"
  auto_create_subnetworks = false
  description             = "Gravity Reth benchmark network"
}

# Create subnet
resource "google_compute_subnetwork" "gravity_subnet" {
  name          = "${var.project_name}-subnet"
  ip_cidr_range = var.subnet_cidr
  region        = var.region
  network      = google_compute_network.gravity_network.id
}

# Create firewall rules
resource "google_compute_firewall" "gravity_internal" {
  name    = "${var.project_name}-internal"
  network = google_compute_network.gravity_network.name

  allow {
    protocol = "tcp"
    ports    = ["22", "80", "443", "8545", "8546", "8551", "30303"]
  }

  allow {
    protocol = "udp"
    ports    = ["30303"]
  }

  source_ranges = [var.subnet_cidr]
  target_tags   = ["gravity-node"]
}

resource "google_compute_firewall" "gravity_external" {
  name    = "${var.project_name}-external"
  network = google_compute_network.gravity_network.name

  allow {
    protocol = "tcp"
    ports    = ["22", "80", "443", "8545", "8546", "8551", "30303"]
  }

  allow {
    protocol = "udp"
    ports    = ["30303"]
  }

  source_ranges = ["0.0.0.0/0"]
  target_tags   = ["gravity-node"]
}

# Service account for nodes
resource "google_service_account" "gravity_sa" {
  account_id   = "${var.project_name}-sa"
  display_name = "Gravity Reth Service Account"
  description  = "Service account for Gravity Reth benchmark nodes"
}

# Create IAM binding for service account
resource "google_project_iam_binding" "gravity_sa_binding" {
  project = var.project_id
  role    = "roles/compute.instanceAdmin"

  members = [
    "serviceAccount:${google_service_account.gravity_sa.email}",
  ]
}

# Prepare execution node setup script with variables
locals {
  execution_setup_script_content = templatefile("${path.module}/scripts/execution-node-setup.sh", {
    gravity_reth_repo   = var.gravity_reth_repo
    gravity_reth_branch = var.gravity_reth_branch
    http_port           = var.http_port
    ws_port             = var.ws_port
    engine_port         = var.engine_port
    p2p_port            = var.p2p_port
  })

  client_setup_script_content = templatefile("${path.module}/scripts/client-node-setup.sh", {
    gravity_bench_repo        = var.gravity_bench_repo
    gravity_bench_branch      = var.gravity_bench_branch
    execution_node_internal_ip = google_compute_instance.execution_node.network_interface[0].network_ip
    http_port                 = var.http_port
  })
}

# Execution node instance
resource "google_compute_instance" "execution_node" {
  name         = "${var.project_name}-execution"
  machine_type  = var.execution_machine_type
  zone          = var.zone

  allow_stopping_for_update = true

  tags = ["gravity-node", "gravity-execution"]

  boot_disk {
    initialize_params {
      image = var.image
      size  = var.execution_disk_size
      type  = "pd-standard"
    }
  }

  network_interface {
    network    = google_compute_network.gravity_network.id
    subnetwork = google_compute_subnetwork.gravity_subnet.id
    access_config {
      // Ephemeral public IP
    }
  }

  metadata = {
    ssh-keys = "${var.ssh_user}:${tls_private_key.gravity_ssh.public_key_openssh}"
  }

  service_account {
    email  = google_service_account.gravity_sa.email
    scopes = ["cloud-platform"]
  }

  labels = {
    environment = "testing"
    project     = "gravity-reth"
    managed-by  = "terraform"
    role        = "execution"
  }
}

# Client node instance (depends on execution node to get its IP)
resource "google_compute_instance" "client_node" {
  name         = "${var.project_name}-client"
  machine_type  = var.client_machine_type
  zone          = var.zone

  allow_stopping_for_update = true

  tags = ["gravity-node", "gravity-client"]

  boot_disk {
    initialize_params {
      image = var.image
      size  = var.client_disk_size
      type  = "pd-standard"
    }
  }

  network_interface {
    network    = google_compute_network.gravity_network.id
    subnetwork = google_compute_subnetwork.gravity_subnet.id
    access_config {
      // Ephemeral public IP
    }
  }

  metadata = {
    ssh-keys = "${var.ssh_user}:${tls_private_key.gravity_ssh.public_key_openssh}"
  }

  service_account {
    email  = google_service_account.gravity_sa.email
    scopes = ["cloud-platform"]
  }

  labels = {
    environment = "testing"
    project     = "gravity-reth"
    managed-by  = "terraform"
    role        = "client"
  }

  # Wait for execution node to be created first
  depends_on = [google_compute_instance.execution_node]
}

# Wait for execution node to be ready (SSH accessible)
resource "null_resource" "wait_for_execution_ssh" {
  depends_on = [google_compute_instance.execution_node]

  provisioner "local-exec" {
    command = <<-EOT
      echo "Waiting for execution node to be SSH accessible..."
      for i in {1..30}; do
        ssh -i ${path.module}/gravity-deploy-key \
            -o StrictHostKeyChecking=no \
            -o ConnectTimeout=5 \
            -o BatchMode=yes \
            ${var.ssh_user}@${google_compute_instance.execution_node.network_interface[0].access_config[0].nat_ip} \
            "echo 'SSH ready'" && break || sleep 10
      done
    EOT
  }

  triggers = {
    instance_id = google_compute_instance.execution_node.id
  }
}

# NOTE: Script copying and execution are now handled by deploy.sh
# This allows for easier reruns without recreating infrastructure
# The following resources have been moved to deploy.sh:
# - copy_dev_node_script
# - copy_execution_setup_script  
# - execute_execution_setup

# Wait for client node to be ready (SSH accessible)
resource "null_resource" "wait_for_client_ssh" {
  depends_on = [google_compute_instance.client_node]

  provisioner "local-exec" {
    command = <<-EOT
      echo "Waiting for client node to be SSH accessible..."
      for i in {1..30}; do
        ssh -i ${path.module}/gravity-deploy-key \
            -o StrictHostKeyChecking=no \
            -o ConnectTimeout=5 \
            -o BatchMode=yes \
            ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip} \
            "echo 'SSH ready'" && break || sleep 10
      done
    EOT
  }

  triggers = {
    instance_id = google_compute_instance.client_node.id
  }
}

# Copy Dockerfile to client node
resource "null_resource" "copy_dockerfile" {
  depends_on = [null_resource.wait_for_client_ssh]

  provisioner "file" {
    source      = "${path.module}/scripts/Dockerfile"
    destination = "/tmp/Dockerfile"

    connection {
      type        = "ssh"
      user        = var.ssh_user
      private_key = tls_private_key.gravity_ssh.private_key_pem
      host        = google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip
    }
  }

  triggers = {
    dockerfile_file = filemd5("${path.module}/scripts/Dockerfile")
    instance_id     = google_compute_instance.client_node.id
  }
}

# Copy bench_config.template to client node
resource "null_resource" "copy_bench_config_template" {
  depends_on = [null_resource.wait_for_client_ssh]

  provisioner "file" {
    source      = "${path.module}/templates/bench_config.template"
    destination = "/tmp/bench_config.template"

    connection {
      type        = "ssh"
      user        = var.ssh_user
      private_key = tls_private_key.gravity_ssh.private_key_pem
      host        = google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip
    }
  }

  triggers = {
    template_file = filemd5("${path.module}/templates/bench_config.template")
    instance_id   = google_compute_instance.client_node.id
  }
}

# Copy client node setup script
resource "null_resource" "copy_client_setup_script" {
  depends_on = [
    null_resource.wait_for_client_ssh,
    google_compute_instance.execution_node,
    null_resource.copy_dockerfile,
    null_resource.copy_bench_config_template
  ]

  provisioner "file" {
    content     = local.client_setup_script_content
    destination = "/tmp/setup-client-node.sh"

    connection {
      type        = "ssh"
      user        = var.ssh_user
      private_key = tls_private_key.gravity_ssh.private_key_pem
      host        = google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip
    }
  }

  triggers = {
    script_content     = sha256(local.client_setup_script_content)
    instance_id        = google_compute_instance.client_node.id
    execution_node_ip  = google_compute_instance.execution_node.network_interface[0].network_ip
  }
}

# Execute client node setup script
resource "null_resource" "execute_client_setup" {
  depends_on = [null_resource.copy_client_setup_script]

  provisioner "remote-exec" {
    inline = [
      "if [ -f /tmp/setup-client-node.sh ]; then chmod +x /tmp/setup-client-node.sh && sudo mv /tmp/setup-client-node.sh /opt/setup-client-node.sh; fi",
      "if [ ! -f /opt/setup-client-node.sh ]; then echo 'ERROR: Setup script not found at /opt/setup-client-node.sh or /tmp/setup-client-node.sh' && exit 1; fi",
      "sudo bash /opt/setup-client-node.sh 2>&1 | sudo tee -a /var/log/client-node-setup.log || true",
      "test -f /var/log/client-node-setup-complete && exit 0 || exit 1"
    ]

    connection {
      type        = "ssh"
      user        = var.ssh_user
      private_key = tls_private_key.gravity_ssh.private_key_pem
      host        = google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip
    }
  }

  triggers = {
    script_content     = sha256(local.client_setup_script_content)
    instance_id        = google_compute_instance.client_node.id
    execution_node_ip  = google_compute_instance.execution_node.network_interface[0].network_ip
  }
}

