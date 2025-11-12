# Gravity Reth Benchmark Terraform Configuration

This Terraform configuration creates a 2-node GCloud setup for benchmarking Gravity Reth:

1. **Execution Node**: Clones and builds [gravity-reth](https://github.com/Galxe/gravity-reth.git), then runs it in dev mode
2. **Client Node**: Sets up Docker, clones [gravity_bench](https://github.com/Galxe/gravity_bench.git), and starts the benchmark client

## Prerequisites

- Terraform >= 1.0
- GCP project with billing enabled
- GCP credentials configured (via `gcloud auth application-default login` or service account)
- Appropriate GCP permissions to create compute instances, networks, and firewall rules

## Quick Start

### Option 1: Using the deployment script (Recommended)

1. **Navigate to the terraform directory:**
   ```bash
   cd bench/terraform
   ```

2. **Copy and edit the variables file:**
   ```bash
   cp terraform.tfvars.example terraform.tfvars
   # Edit terraform.tfvars with your GCP project ID
   ```

3. **Run the deployment script:**
   ```bash
   ./deploy.sh
   ```

   This will:
   - Initialize Terraform
   - Create a plan
   - Apply the configuration
   - Show outputs

### Option 2: Manual Terraform commands

1. **Copy the example variables file:**
   ```bash
   cp terraform.tfvars.example terraform.tfvars
   ```

2. **Edit `terraform.tfvars` with your GCP project ID:**
   ```hcl
   project_id = "your-gcp-project-id"
   ```

3. **Initialize Terraform:**
   ```bash
   terraform init
   ```

4. **Review the plan:**
   ```bash
   terraform plan
   ```

5. **Apply the configuration:**
   ```bash
   terraform apply
   ```

6. **After deployment, get connection information:**
   ```bash
   terraform output
   ```

## Deployment Script Usage

The `deploy.sh` script provides convenient commands:

```bash
# Full deployment (init + plan + apply + output)
./deploy.sh

# Individual commands
./deploy.sh init              # Initialize only
./deploy.sh plan              # Show plan only
./deploy.sh apply             # Apply changes
./deploy.sh apply --yes       # Apply without confirmation
./deploy.sh destroy           # Destroy all resources
./deploy.sh output            # Show outputs
./deploy.sh status            # Show current status

# Re-run setup scripts (without recreating nodes)
./deploy.sh rerun-execution   # Re-run execution node setup
./deploy.sh rerun-client      # Re-run client node setup

# SSH to nodes
./deploy.sh ssh-execution     # SSH to execution node
./deploy.sh ssh-client        # SSH to client node

# View logs
./deploy.sh logs-execution    # Show execution node logs
./deploy.sh logs-client       # Show client node logs
```

## How It Works

### Setup Process

Unlike traditional startup scripts, this configuration:
1. **Creates the instances** without startup scripts
2. **Waits for SSH access** to be available
3. **Copies setup scripts** to `/opt/` on each node
4. **Executes the scripts** via SSH

This approach allows:
- **Easy re-execution**: Scripts are saved to `/opt/setup-execution-node.sh` and `/opt/setup-client-node.sh`
- **Better debugging**: You can SSH in and run scripts manually
- **Idempotent scripts**: Scripts can be safely re-run

### Execution Node

1. Installs Rust and build dependencies
2. Clones the gravity-reth repository
3. Builds the reth binary in release mode
4. Runs `dev-node.sh` to start the node in dev mode
5. The node starts with HTTP RPC, WebSocket RPC, and Engine API enabled

### Client Node

1. Installs Docker
2. Waits for the execution node to be ready
3. Clones the gravity_bench repository
4. Creates `bench_config.toml` from `bench_config.template` with the execution node's internal IP
5. Starts Docker containers or runs benchmark scripts

## Re-running Setup Scripts

If setup fails or you need to re-run it:

### Using Deployment Script

```bash
# Re-run execution node setup
./deploy.sh rerun-execution

# Re-run client node setup
./deploy.sh rerun-client
```

### Using Helper Script

```bash
# Re-run execution node setup
./run-setup.sh execution

# Re-run client node setup
./run-setup.sh client

# Re-run both
./run-setup.sh both
```

### Using Debug Script

```bash
# Re-run execution node setup
./debug.sh rerun-execution

# Re-run client node setup
./debug.sh rerun-client
```

### Manual Re-execution

SSH to the node and run the script directly:

```bash
# SSH to execution node
./deploy.sh ssh-execution

# Then on the node:
sudo /opt/setup-execution-node.sh
```

The scripts are idempotent and can be safely re-run multiple times.

## Destroying Resources

### Destroy All Resources

```bash
./deploy.sh destroy
# Or with auto-approve
./deploy.sh destroy --yes
```

To recreate after destroying, simply run:
```bash
./deploy.sh apply
# Or
./deploy.sh
```

## Configuration

### Machine Types

You can customize the machine types for each node:

- `execution_machine_type`: Default is `e2-standard-8` (8 vCPUs, 32GB RAM)
- `client_machine_type`: Default is `e2-standard-4` (4 vCPUs, 16GB RAM)

### Ports

Default ports:
- HTTP RPC: 8545
- WebSocket RPC: 8546
- Engine API: 8551
- P2P: 30303

### Repositories

- `gravity_reth_repo`: Default is `https://github.com/Galxe/gravity-reth.git`
- `gravity_bench_repo`: Default is `https://github.com/Galxe/gravity_bench.git`

## Accessing the Nodes

After deployment, use the SSH commands from the output or the deployment script:

```bash
# SSH to execution node
./deploy.sh ssh-execution

# SSH to client node
./deploy.sh ssh-client

# Or use the SSH commands from terraform output
terraform output ssh_commands
```

## Monitoring

- Execution node logs: `/var/log/execution-node-setup.log` and `/opt/gravity-reth/bench/.dev-node-logs/reth-node.log`
- Client node logs: `/var/log/client-node-setup.log` and `/var/log/gravity-bench.log`

View logs using:
```bash
./deploy.sh logs-execution
./deploy.sh logs-client

# Or for full debugging
./debug.sh logs-execution
./debug.sh monitor-execution
```

## Debugging

For detailed debugging, use the `debug.sh` script:

```bash
# Check node status
./debug.sh status-execution

# View full logs
./debug.sh logs-execution

# Monitor logs in real-time
./debug.sh monitor-execution

# Check specific components
./debug.sh check-rust
./debug.sh check-build
./debug.sh check-ports

# Run custom commands
./debug.sh exec-execution "ps aux | grep reth"
```

See [DEBUGGING.md](./DEBUGGING.md) for more details.

## Troubleshooting

### Setup Failed

1. **Check logs:**
   ```bash
   ./debug.sh logs-execution
   ```

2. **Re-run setup:**
   ```bash
   ./deploy.sh rerun-execution
   ```

3. **If still failing, destroy and recreate:**
   ```bash
   ./deploy.sh destroy
   ./deploy.sh apply
   ```

### Node Not Responding

1. **Check status:**
   ```bash
   ./debug.sh status-execution
   ```

2. **Check if ports are listening:**
   ```bash
   ./debug.sh check-ports
   ```

3. **Re-run setup:**
   ```bash
   ./deploy.sh rerun-execution
   ```

### Client Can't Connect to Execution Node

1. **Verify execution node is running:**
   ```bash
   ./debug.sh status-execution
   ./debug.sh check-ports
   ```

2. **Check firewall rules** in GCP console

3. **Re-run client setup:**
   ```bash
   ./deploy.sh rerun-client
   ```

## Files Structure

```
bench/terraform/
├── main.tf                    # Main Terraform configuration
├── variables.tf                # Variable definitions
├── outputs.tf                 # Output definitions
├── terraform.tfvars.example   # Example variables file
├── deploy.sh                  # Deployment automation script
├── debug.sh                   # Debugging helper script
├── run-setup.sh               # Re-run setup scripts
├── README.md                  # This file
├── DEBUGGING.md               # Debugging guide
├── scripts/
│   ├── execution-node-setup.sh  # Execution node setup script
│   └── client-node-setup.sh     # Client node setup script
└── templates/
    └── bench_config.template     # Benchmark configuration template
```

## Notes

- The execution node's internal IP is automatically extracted and used in the client node's `bench_config.toml`
- The setup scripts wait for services to be ready before proceeding
- All logs are written to `/var/log/` for easy debugging
- The SSH private key is saved as `gravity-deploy-key` in the terraform directory (keep it secure!)
- Setup scripts are copied to `/opt/setup-execution-node.sh` and `/opt/setup-client-node.sh` on the nodes for easy re-execution
- Scripts are idempotent and can be safely re-run if errors occur
- You can destroy and recreate individual nodes without affecting the other node
