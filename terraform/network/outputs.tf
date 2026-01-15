# FastEVM Terraform Outputs
# Output values for the deployed infrastructure

output "project_id" {
  description = "The GCP project ID"
  value       = var.project_id
}

output "network_name" {
  description = "Name of the created VPC network"
  value       = google_compute_network.fastevm_network.name
}

output "subnet_name" {
  description = "Name of the created subnet"
  value       = google_compute_subnetwork.fastevm_subnet.name
}

output "instance_names" {
  description = "Names of the created instances"
  value       = google_compute_instance.fastevm_nodes[*].name
}

output "instance_ips" {
  description = "Internal IP addresses of the instances"
  value       = google_compute_instance.fastevm_nodes[*].network_interface[0].network_ip
}

output "instance_external_ips" {
  description = "External IP addresses of the instances"
  value       = google_compute_instance.fastevm_nodes[*].network_interface[0].access_config[0].nat_ip
}

output "load_balancer_ip" {
  description = "External IP address of the load balancer"
  value       = google_compute_global_address.fastevm_ip.address
}

output "service_account_email" {
  description = "Email of the service account"
  value       = google_service_account.fastevm_sa.email
}

output "ssh_private_key_path" {
  description = "Path to the generated SSH private key"
  value       = local_file.fastevm_private_key.filename
}

output "ssh_public_key_path" {
  description = "Path to the generated SSH public key"
  value       = local_file.fastevm_public_key.filename
}

output "ssh_public_key" {
  description = "Generated SSH public key"
  value       = tls_private_key.fastevm_ssh.public_key_openssh
  sensitive   = false
}

output "node_endpoints" {
  description = "RPC endpoints for each node"
  value = {
    for i in range(var.node_count) : "node-${i + 1}" => {
      internal_ip   = google_compute_instance.fastevm_nodes[i].network_interface[0].network_ip
      external_ip   = google_compute_instance.fastevm_nodes[i].network_interface[0].access_config[0].nat_ip
      http_rpc      = "http://${google_compute_instance.fastevm_nodes[i].network_interface[0].access_config[0].nat_ip}:8545"
      ws_rpc        = "ws://${google_compute_instance.fastevm_nodes[i].network_interface[0].access_config[0].nat_ip}:8546"
      engine_api    = "http://${google_compute_instance.fastevm_nodes[i].network_interface[0].access_config[0].nat_ip}:8551"
      consensus_api = "http://${google_compute_instance.fastevm_nodes[i].network_interface[0].access_config[0].nat_ip}:26657"
    }
  }
}

output "peer_configuration" {
  description = "Peer-to-peer configuration for consensus nodes"
  value = {
    authorities = [
      for i in range(var.node_count) : {
        index         = i
        stake         = 1000
        hostname      = "fastevm-consensus${i}"
        address       = "/ip4/${google_compute_instance.fastevm_nodes[i].network_interface[0].access_config[0].nat_ip}/udp/26657"
        authority_key = "AuthorityPublicKey(placeholder-${i})"
        protocol_key  = "ProtocolPublicKey(placeholder-${i})"
        network_key   = "NetworkPublicKey(placeholder-${i})"
      }
    ]
    docker_network = {
      base_ip  = "10.0.0"
      start_ip = 10
      end_ip   = 10 + var.node_count - 1
      port     = 26657
    }
    quorum_threshold   = var.node_count
    validity_threshold = var.node_count
  }
}

output "execution_bootnodes" {
  description = "Bootnode configuration for execution clients"
  value = [
    for i in range(var.node_count) :
    "enode://${google_compute_instance.fastevm_nodes[i].network_interface[0].access_config[0].nat_ip}:30303"
  ]
}

output "monitoring_endpoints" {
  description = "Monitoring and health check endpoints"
  value = {
    load_balancer_health = "http://${google_compute_global_address.fastevm_ip.address}/health"
    node_health_checks = [
      for i in range(var.node_count) :
      "http://${google_compute_instance.fastevm_nodes[i].network_interface[0].access_config[0].nat_ip}:8545"
    ]
  }
}

output "ssh_commands" {
  description = "SSH commands to connect to each node"
  value = [
    for i in range(var.node_count) :
    "gcloud compute ssh ${google_compute_instance.fastevm_nodes[i].name} --zone=${var.zone}"
  ]
}

output "deployment_summary" {
  description = "Summary of the deployment"
  value = {
    project_name          = var.project_name
    node_count            = var.node_count
    region                = var.region
    zone                  = var.zone
    machine_type          = var.machine_type
    total_disk_size       = var.disk_size * var.node_count
    network_cidr          = var.subnet_cidr
    load_balancer_enabled = var.enable_load_balancer
    monitoring_enabled    = var.enable_monitoring
  }
}

output "next_steps" {
  description = "Next steps after deployment"
  value = [
    "1. Wait for bootstrap script to complete on all nodes",
    "2. Check node status: gcloud compute instances list --filter='name~fastevm-node'",
    "3. SSH to nodes to verify FastEVM is running",
    "4. Test RPC endpoints using the node_endpoints output",
    "5. Configure monitoring if enabled",
    "6. Update DNS records to point to load balancer IP if needed"
  ]
}
