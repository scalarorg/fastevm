# Gravity Reth Benchmark Terraform Variables Example
# Copy this file to terraform.tfvars and customize the values

# Required: GCP Project ID
project_id = "scalar-459101"

# Optional: Project name prefix for all resources
project_name = "gravity-bench"

# Optional: GCP Region and Zone
region = "us-central1"
zone   = "us-central1-a"

# Optional: Machine types
# These can be set via .env file (EXECUTION_MACHINE_TYPE, CLIENT_MACHINE_TYPE)
# or uncomment and set here. .env file takes precedence if both are set.
# execution_machine_type = "c4-highcpu-16"
# client_machine_type    = "e2-standard-8"

# Optional: Disk sizes (in GB)
execution_disk_size = 100
execution_disk_type = "hyperdisk-balanced"
client_disk_size    = 40

# Optional: Boot disk image
image = "ubuntu-os-cloud/ubuntu-2204-lts"

# Optional: Network configuration
subnet_cidr = "10.0.0.0/24"

# Optional: SSH user
ssh_user = "ubuntu"

# Optional: Repository configuration
gravity_reth_repo   = "https://github.com/Galxe/gravity-reth.git"
gravity_reth_branch = "main"

gravity_bench_repo   = "https://github.com/Galxe/gravity_bench.git"
gravity_bench_branch = "main"

# Optional: Port configuration
http_port   = 8545
ws_port     = 8546
engine_port = 8551
p2p_port    = 30303

