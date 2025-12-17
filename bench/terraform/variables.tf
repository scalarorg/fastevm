# Gravity Reth Benchmark Terraform Variables

variable "project_id" {
  description = "The GCP project ID"
  type        = string
}

variable "project_name" {
  description = "Name prefix for all resources"
  type        = string
  default     = "gravity-bench"
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

variable "execution_machine_type" {
  description = "Machine type for the execution node"
  type        = string
  default     = "e2-standard-8"
}

variable "client_machine_type" {
  description = "Machine type for the client node"
  type        = string
  default     = "e2-standard-4"
}

variable "execution_disk_size" {
  description = "Size of persistent disk for execution node in GB"
  type        = number
  default     = 200
}

variable "execution_disk_type" {
  description = "Type of persistent disk for execution node"
  type        = string
  default     = "hyperdisk-balanced"
}

variable "execution_local_ssd_count" {
  description = "Number of local SSD disks to attach to execution node. Note: Not all machine types support local SSDs (e.g., c4-highcpu-16 does not support them). Set to 0 to disable."
  type        = number
  default     = 0
}

variable "client_disk_size" {
  description = "Size of persistent disk for client node in GB"
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

variable "ssh_user" {
  description = "SSH user for node access"
  type        = string
  default     = "ubuntu"
}

variable "gravity_reth_repo" {
  description = "GitHub repository URL for gravity-reth"
  type        = string
  default     = "https://github.com/Galxe/gravity-reth.git"
}

variable "gravity_reth_branch" {
  description = "GitHub branch for gravity-reth"
  type        = string
  default     = "main"
}

variable "gravity_sdk_repo" {
  description = "GitHub repository URL for gravity-sdk"
  type        = string
  default     = "https://github.com/Galxe/gravity-sdk.git"
}

variable "gravity_sdk_branch" {
  description = "GitHub branch for gravity-sdk"
  type        = string
  default     = "main"
}

variable "gravity_bench_repo" {
  description = "GitHub repository URL for gravity_bench"
  type        = string
  default     = "https://github.com/Galxe/gravity_bench.git"
}

variable "gravity_bench_branch" {
  description = "GitHub branch for gravity_bench"
  type        = string
  default     = "main"
}

variable "http_port" {
  description = "HTTP RPC port for execution node"
  type        = number
  default     = 8545
}

variable "ws_port" {
  description = "WebSocket RPC port for execution node"
  type        = number
  default     = 8546
}

variable "engine_port" {
  description = "Engine API port for execution node"
  type        = number
  default     = 8551
}

variable "p2p_port" {
  description = "P2P port for execution node"
  type        = number
  default     = 30303
}

