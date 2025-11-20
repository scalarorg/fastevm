# Gravity Reth Benchmark Terraform Outputs

output "execution_node_info" {
  value = {
    name        = google_compute_instance.execution_node.name
    external_ip = google_compute_instance.execution_node.network_interface[0].access_config[0].nat_ip
    internal_ip = google_compute_instance.execution_node.network_interface[0].network_ip
    zone        = google_compute_instance.execution_node.zone
    http_rpc    = "http://${google_compute_instance.execution_node.network_interface[0].network_ip}:${var.http_port}"
    ws_rpc      = "ws://${google_compute_instance.execution_node.network_interface[0].network_ip}:${var.ws_port}"
    engine_api  = "http://${google_compute_instance.execution_node.network_interface[0].network_ip}:${var.engine_port}"
  }
  description = "Execution node connection information"
}

output "client_node_info" {
  value = {
    name        = google_compute_instance.client_node.name
    external_ip = google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip
    internal_ip = google_compute_instance.client_node.network_interface[0].network_ip
    zone        = google_compute_instance.client_node.zone
  }
  description = "Client node connection information"
}

output "ssh_commands" {
  value = {
    execution_node = "ssh -i ${path.module}/gravity-deploy-key ${var.ssh_user}@${google_compute_instance.execution_node.network_interface[0].access_config[0].nat_ip}"
    client_node    = "ssh -i ${path.module}/gravity-deploy-key ${var.ssh_user}@${google_compute_instance.client_node.network_interface[0].access_config[0].nat_ip}"
  }
  description = "SSH commands to connect to nodes"
}

output "network_info" {
  value = {
    network_name = google_compute_network.gravity_network.name
    subnet_name  = google_compute_subnetwork.gravity_subnet.name
    subnet_cidr  = google_compute_subnetwork.gravity_subnet.ip_cidr_range
  }
  description = "Network information"
}

# Outputs for script template variables
output "gravity_reth_repo" {
  value       = var.gravity_reth_repo
  description = "Gravity Reth repository URL"
}

output "gravity_reth_branch" {
  value       = var.gravity_reth_branch
  description = "Gravity Reth branch"
}

output "gravity_sdk_repo" {
  value       = var.gravity_sdk_repo
  description = "Gravity SDK repository URL"
}

output "gravity_sdk_branch" {
  value       = var.gravity_sdk_branch
  description = "Gravity SDK branch"
}

output "http_port" {
  value       = var.http_port
  description = "HTTP RPC port"
}

output "ws_port" {
  value       = var.ws_port
  description = "WebSocket RPC port"
}

output "engine_port" {
  value       = var.engine_port
  description = "Engine API port"
}

output "p2p_port" {
  value       = var.p2p_port
  description = "P2P port"
}

