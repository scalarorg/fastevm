# FastEVM Client Node Variables
# Configuration variables for standalone client node deployment

variable "project_id" {
  description = "The GCP project ID"
  type        = string
  default     = "your-gcp-project-id"
}

variable "project_name" {
  description = "Name prefix for all resources"
  type        = string
  default     = "fastevm-client"
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

variable "client_machine_type" {
  description = "Machine type for the client instance"
  type        = string
  default     = "e2-standard-2"
}

variable "client_disk_size" {
  description = "Size of persistent disk for client node in GB"
  type        = number
  default     = 50
}

variable "disk_type" {
  description = "Type of persistent disk"
  type        = string
  default     = "pd-standard"
}

variable "image" {
  description = "Boot disk image"
  type        = string
  default     = "ubuntu-os-cloud/ubuntu-2204-lts"
}

variable "client_subnet_cidr" {
  description = "CIDR block for the client subnet (same as main network)"
  type        = string
  default     = "10.0.0.0/24"
}

variable "ssh_user" {
  description = "SSH user for client node access"
  type        = string
  default     = "ubuntu"
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
