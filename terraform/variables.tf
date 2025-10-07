# FastEVM Terraform Variables
# Configuration variables for GCP deployment

variable "project_id" {
  description = "The GCP project ID"
  type        = string
  default     = "your-gcp-project-id"
}

variable "project_name" {
  description = "Name prefix for all resources"
  type        = string
  default     = "fastevm"
}

variable "region" {
  description = "The GCP region"
  type        = string
  default     = "us-central1"
}

variable "zone" {
  description = "The GCP zone"
  type        = string
  default     = "us-central1-a"
}

variable "node_count" {
  description = "Number of nodes to deploy"
  type        = number
  default     = 4
}

variable "machine_type" {
  description = "Machine type for the instances"
  type        = string
  default     = "e2-standard-4"
}

variable "disk_type" {
  description = "Type of persistent disk"
  type        = string
  default     = "pd-ssd"
}

variable "disk_size" {
  description = "Size of persistent disk in GB"
  type        = number
  default     = 100
}

variable "image" {
  description = "Boot disk image"
  type        = string
  default     = "ubuntu-os-cloud/ubuntu-2204-lts"
}

variable "subnet_cidr" {
  description = "CIDR block for the subnet"
  type        = string
  default     = "10.0.0.0/24"
}

variable "github_repo" {
  description = "GitHub repository URL"
  type        = string
  default     = "https://github.com/scalarorg/fastevm.git"
}

variable "github_branch" {
  description = "GitHub branch to clone"
  type        = string
  default     = "main"
}

variable "enable_monitoring" {
  description = "Enable monitoring and logging"
  type        = bool
  default     = true
}

variable "enable_load_balancer" {
  description = "Enable load balancer for external access"
  type        = bool
  default     = true
}

variable "node_labels" {
  description = "Labels to apply to all nodes"
  type        = map(string)
  default = {
    environment = "production"
    project     = "fastevm"
    managed-by  = "terraform"
  }
}

variable "additional_tags" {
  description = "Additional tags for firewall rules"
  type        = list(string)
  default     = []
}

variable "ssh_keys" {
  description = "SSH public keys for access"
  type        = list(string)
  default     = []
}

variable "custom_startup_script" {
  description = "Custom startup script to run after bootstrap"
  type        = string
  default     = ""
}

variable "consensus_parameters" {
  description = "Consensus parameters configuration"
  type = object({
    leader_timeout_ms         = number
    min_round_delay_ms        = number
    max_forward_time_drift_ms = number
    max_blocks_per_sync       = number
    max_blocks_per_fetch      = number
  })
  default = {
    leader_timeout_ms         = 200
    min_round_delay_ms        = 100
    max_forward_time_drift_ms = 500
    max_blocks_per_sync       = 32
    max_blocks_per_fetch      = 1000
  }
}

variable "execution_config" {
  description = "Execution client configuration"
  type = object({
    http_port_start   = number
    ws_port_start     = number
    engine_port_start = number
    p2p_port_start    = number
    log_level         = string
  })
  default = {
    http_port_start   = 8545
    ws_port_start     = 8546
    engine_port_start = 8551
    p2p_port_start    = 30303
    log_level         = "info"
  }
}

variable "consensus_config" {
  description = "Consensus client configuration"
  type = object({
    port_start = number
    log_level  = string
  })
  default = {
    port_start = 26657
    log_level  = "info"
  }
}
