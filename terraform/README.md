# FastEVM GCP Terraform Deployment

This directory contains Terraform configurations to deploy FastEVM nodes on Google Cloud Platform (GCP).
## 📄 License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.

## 🚀 Quick Start

### Single Command Deployment (Recommended)

The easiest way to deploy FastEVM is using the single command that handles everything:

```bash
# 1. Prerequisites
brew install terraform gcloud jq  # macOS
# or
sudo apt install terraform google-cloud-cli jq  # Ubuntu

# 2. Authenticate with GCP
gcloud auth login
gcloud auth application-default login \
      --scopes="https://www.googleapis.com/auth/cloud-platform,https://www.googleapis.com/auth/userinfo.email,openid"

   # 3. Deploy everything in one command
   cd terraform
   make deploy
```

This single command will:
- ✅ Create GCP infrastructure (VMs, networking, disks)
- ✅ Build Rust binaries on node 0 only (optimized build process)
- ✅ Distribute binaries from node 0 to all other nodes
- ✅ Prepare all node configurations locally
- ✅ Deploy and configure all nodes remotely
- ✅ Start all FastEVM services
- ✅ Verify deployment health

## 🔧 Build Process

The deployment uses an optimized build process to avoid build failures:

1. **Build on Node 0 only**: Only the first node (node 0) builds the Rust binaries
2. **Distribute binaries**: Binaries are copied from node 0 to localhost, then distributed to all nodes
3. **Install services**: Services are installed on all nodes using the distributed binaries
4. **Start services**: All services are started in parallel

This approach is more reliable and faster than building on each node individually.

## 📋 Deployment Flow to GCloud

This section documents the complete deployment flow from infrastructure provisioning to service startup when deploying FastEVM to Google Cloud Platform using Terraform.

### Overview

The deployment process consists of several phases that work together to create a fully functional FastEVM network:

1. **Infrastructure Provisioning** - Create GCP resources (VMs, networking, security)
2. **Configuration Extraction** - Gather deployment information (IPs, keys)
3. **File Distribution** - Copy scripts and configs to all nodes
4. **Node Setup** - Install dependencies and build binaries on each node
5. **Network Configuration** - Configure peer-to-peer networking and consensus
6. **Service Startup** - Start all FastEVM services
7. **Verification** - Validate deployment health

### Detailed Deployment Flow

#### Phase 1: Prerequisites and Initialization

**Targets**: `check-prereqs`, `init`, `validate`

```bash
make check-prereqs  # Validates required tools are installed
make init           # Initializes Terraform backend and providers
make validate       # Validates Terraform configuration syntax
```

**What happens:**
- Checks for required tools: `terraform`, `gcloud`, `jq`
- Verifies `PROJECT_ID` is set (from `fastevm.env` or environment)
- Initializes Terraform with GCP provider
- Validates all `.tf` files for syntax errors

**Infrastructure Created:**
- None yet (preparation phase)

---

#### Phase 2: Infrastructure Planning

**Target**: `plan`

```bash
make plan  # Creates Terraform execution plan
```

**What happens:**
- Loads variables from `terraform.tfvars` or environment
- Calculates infrastructure changes
- Generates execution plan saved to `tfplan`
- Shows what resources will be created/modified/destroyed

**Infrastructure Created:**
- None yet (planning phase)

---

#### Phase 3: Infrastructure Provisioning

**Targets**: `apply`, `import-existing` (if needed)

```bash
make apply  # Creates all GCP resources
```

**What happens:**
- Creates VPC network (`fastevm-network`) with subnet (`10.0.0.0/24`)
- Creates firewall rules (internal and external access)
- Generates SSH key pair (`fastevm-deploy-key`, `fastevm-deploy-key.pub`)
- Creates service account with compute permissions
- Provisions compute instances (default: 4x `e2-standard-4` VMs)
- Creates load balancer with global IP address
- Configures instance groups and health checks
- If resources already exist, attempts to import them

**Infrastructure Created:**
- VPC Network: `fastevm-network`
- Subnet: `fastevm-subnet` (10.0.0.0/24)
- Firewall Rules: `fastevm-internal`, `fastevm-external`
- Service Account: `fastevm-sa`
- Compute Instances: `fastevm-node-1` through `fastevm-node-N`
- Load Balancer: Global IP, health check, backend service
- SSH Keys: `fastevm-deploy-key` (private), `fastevm-deploy-key.pub` (public)

**Outputs Generated:**
- `terraform.tfstate` - Current infrastructure state
- `../deployment-info.json` - Deployment summary with IPs and endpoints

---

#### Phase 4: Deployment Information Extraction

**Target**: `extract-deployment-info`

```bash
make extract-deployment-info  # Extracts IPs and creates environment files
```

**What happens:**
- Reads Terraform outputs for instance IPs (external and internal)
- Creates `nodes.remote.env` with:
  - `PEER_COUNT` - Number of nodes
  - `PEER1_IP`, `PEER2_IP`, etc. - Internal IPs for each node
  - `ENV_FILE` - Path to environment file on remote nodes
- Sets proper permissions on SSH private key (600)

**Files Created:**
- `nodes.remote.env` - Environment file for remote nodes with internal IPs

---

#### Phase 5: File Distribution

**Target**: `copy-files`

```bash
make copy-files  # Copies scripts and configs to all nodes
```

**What happens:**
- For each node:
  - Creates `/opt/fastevm/` directory on remote node
  - Copies files from `../scripts/` directory:
    - `genesis.json` - Genesis block configuration
    - `node.sh` - Node management script
    - `setup.sh` - Setup script
    - `service.sh` - Service management script
    - `run.sh` - Runtime script
    - `node.yml` - Node configuration
    - `validators.yml` - Validator configuration
    - `parameters.yml` - Consensus parameters
    - Systemd service files (`fastevm-execution.service`, `fastevm-consensus.service`)
  - Copies `nodes.remote.env` to `/opt/fastevm/node.env`
  - Sets execute permissions on shell scripts

**Files Copied to Each Node:**
```
/opt/fastevm/
├── genesis.json
├── node.sh
├── setup.sh
├── service.sh
├── run.sh
├── node.yml
├── validators.yml
├── parameters.yml
├── fastevm-execution.service
├── fastevm-consensus.service
└── node.env
```

---

#### Phase 6: Node Setup

**Targets**: `start-setup`, `watch-setup`

```bash
make start-setup   # Starts setup on all nodes in parallel
make watch-setup   # Monitors setup progress until completion
```

**What happens:**

1. **Start Setup** (`start-setup`):
   - For each node, creates a tmux session named `fastevm-setup`
   - Runs `node.sh setup <private_ip> <public_ip>` in background
   - Setup script:
     - Installs system dependencies (Rust, build tools, etc.)
     - Clones FastEVM repository from GitHub
     - Builds Rust binaries (`fastevm-execution`, `evm-consensus`)
     - Creates data directories (`/data/execution`, `/data/consensus`)
     - Generates node-specific configurations
     - Creates systemd service files
   - Logs written to `/tmp/fastevm-setup.log` on each node

2. **Watch Setup** (`watch-setup`):
   - Tails setup logs from all nodes simultaneously
   - Monitors for success message: `"FastEVM services installed successfully!"`
   - Tracks completion status for each node
   - Exits when all nodes complete setup

**Setup Process on Each Node:**
- Installs: Rust toolchain, build-essential, git, curl, jq
- Clones repository: `https://github.com/scalarorg/fastevm.git`
- Builds binaries: `cargo build --release`
- Creates directories: `/data/execution`, `/data/consensus`, `/data/config`, `/data/logs`
- Generates configurations: `authority.yml`, `validator.yml`, JWT secrets
- Installs binaries: `/usr/local/bin/fastevm-execution`, `/usr/local/bin/evm-consensus`

---

#### Phase 7: Network Configuration

**Targets**: `update-bootnodes`, `collect-validators`, `genesis-config`, `update-envs`

```bash
make update-bootnodes    # Collects peer IDs and updates bootnodes
make collect-validators # Collects authority keys and builds committees.yml
make genesis-config      # Builds genesis_config.json from all validators
make update-envs         # Updates bootnodes and committees on all nodes
```

**What happens:**

1. **Update Bootnodes** (`update-bootnodes`):
   - Connects to each node and reads `/data/execution/p2p/peer-id.hex`
   - Constructs enode URLs: `enode://<peer_id>@<private_ip>:30303`
   - Updates bootnodes configuration on all nodes using `service.sh update-bootnodes`

2. **Collect Validators** (`collect-validators`):
   - Reads `/data/config/authority.yml` from each node
   - Builds unified `committees.yml` with:
     - `epoch: 0`
     - `authorities:` array with all validator keys
     - `quorum_threshold: 2667`
     - `validity_threshold: 1334`
   - Distributes `committees.yml` to `/data/config/committees.yml` on all nodes

3. **Genesis Config** (`genesis-config`):
   - Collects `authority.yml` from all nodes
   - Extracts: validator addresses, consensus public keys, voting powers, network addresses
   - Builds `genesis_config.json` with complete validator set
   - Distributes to `/data/config/genesis_config.json` on all nodes

4. **Update Environments** (`update-envs`):
   - Runs both `update-bootnodes` and `collect-validators`
   - Ensures all nodes have consistent network configuration

**Configuration Files Updated:**
- `/data/config/committees.yml` - Consensus committee configuration
- `/data/config/genesis_config.json` - Genesis block configuration
- Execution client bootnodes configuration

---

#### Phase 8: Service Startup

**Target**: `start-services`

```bash
make start-services  # Starts all FastEVM services on all nodes
```

**What happens:**
- For each node:
  - Executes `cd /opt/fastevm && bash node.sh start`
  - Starts systemd services:
    - `fastevm-execution.service` - Execution client
    - `fastevm-consensus.service` - Consensus client
  - Waits 30 seconds for services to initialize

**Services Started:**
- **Execution Client** (`fastevm-execution`):
  - HTTP RPC: Port 8545
  - WebSocket RPC: Port 8546
  - Engine API: Port 8551
  - P2P: Port 30303
- **Consensus Client** (`evm-consensus`):
  - API: Port 26657
  - P2P: Port 26657 (UDP)

---

#### Phase 9: Deployment Verification

**Target**: `verify-deployment`

```bash
make verify-deployment  # Tests RPC endpoints on all nodes
```

**What happens:**
- Waits 30 seconds for services to stabilize
- For each node, sends JSON-RPC request:
  ```json
  {
    "jsonrpc": "2.0",
    "method": "eth_blockNumber",
    "params": [],
    "id": 1
  }
  ```
- Validates response contains valid block number
- Reports success/failure for each node

**Verification Checks:**
- HTTP RPC endpoint responding (port 8545)
- Valid JSON-RPC response format
- Block number retrieval successful

---

### Complete Deployment Command

The `make deploy` target orchestrates all phases:

```bash
make deploy
```

**Execution Order:**
1. `check-prereqs` - Validate prerequisites
2. `init` - Initialize Terraform
3. `plan` - Create execution plan
4. `apply` - Provision infrastructure
5. `extract-deployment-info` - Extract IPs and create env files
6. `copy-files` - Copy scripts to all nodes
7. `start-setup` - Start setup on all nodes
8. `watch-setup` - Monitor setup completion
9. `update-envs` - Configure network (bootnodes + committees)
10. `start-services` - Start all services
11. `verify-deployment` - Verify deployment health

**Note**: The `deploy` target references `run-setup` and `wait-for-setup` which are aliases for `start-setup` and `watch-setup` respectively.

---

### Deployment Timeline

Typical deployment times:

- **Infrastructure Provisioning**: 2-5 minutes
- **File Distribution**: 1-2 minutes
- **Node Setup** (build binaries): 10-20 minutes per node (parallel)
- **Network Configuration**: 1-2 minutes
- **Service Startup**: 30 seconds
- **Verification**: 1 minute

**Total Time**: ~15-30 minutes depending on:
- Number of nodes
- Build time for Rust binaries
- Network latency
- GCP resource provisioning speed

---

### Deployment Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ Phase 1: Prerequisites & Initialization                     │
│ - Check tools (terraform, gcloud, jq)                       │
│ - Initialize Terraform                                       │
│ - Validate configuration                                     │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 2: Infrastructure Planning                             │
│ - Load variables                                             │
│ - Generate execution plan                                    │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 3: Infrastructure Provisioning                          │
│ - Create VPC, subnet, firewall rules                         │
│ - Generate SSH keys                                          │
│ - Create service account                                     │
│ - Provision compute instances                                │
│ - Configure load balancer                                    │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 4: Extract Deployment Info                             │
│ - Read Terraform outputs                                     │
│ - Create nodes.remote.env                                    │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 5: File Distribution                                    │
│ - Copy scripts to /opt/fastevm/ on all nodes                │
│ - Copy node.env to each node                                 │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 6: Node Setup (Parallel)                               │
│ - Install dependencies                                       │
│ - Clone repository                                           │
│ - Build Rust binaries                                        │
│ - Generate configurations                                    │
│ - Monitor completion                                         │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 7: Network Configuration                                │
│ - Collect peer IDs → Update bootnodes                        │
│ - Collect authority keys → Build committees.yml              │
│ - Build genesis_config.json                                  │
│ - Distribute to all nodes                                    │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 8: Service Startup                                     │
│ - Start execution client (systemd)                          │
│ - Start consensus client (systemd)                          │
│ - Wait for initialization                                    │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│ Phase 9: Verification                                         │
│ - Test RPC endpoints                                         │
│ - Validate block number retrieval                            │
│ - Report deployment status                                   │
└─────────────────────────────────────────────────────────────┘
```

---

### Troubleshooting Deployment

#### Check Setup Status
```bash
make check-setup-status  # Check if setup is running/completed on all nodes
make check-setup-logs    # View setup logs from all nodes
```

#### Manual Steps
If deployment fails at any phase:

1. **Infrastructure Issues**:
   ```bash
   make instances        # Check GCP instance status
   make outputs          # View Terraform outputs
   ```

2. **Setup Issues**:
   ```bash
   make ssh-node NODE=1  # SSH to node 1
   tail -f /tmp/fastevm-setup.log  # View setup log
   ```

3. **Network Configuration Issues**:
   ```bash
   make update-bootnodes    # Retry bootnode update
   make collect-validators  # Retry validator collection
   ```

4. **Service Issues**:
   ```bash
   make restart           # Restart all services
   make health            # Check node health
   make test-rpc          # Test RPC endpoints
   ```

### Deployment

The single deployment command handles everything:

```bash
make deploy
```

This command will:
1. ✅ Prepare Terraform plan and execute it
2. ✅ Extract outputs: public and local IPs, SSH keys
3. ✅ Copy `genesis.json`, `scripts/node.sh`, `scripts/setup.sh`, and remote env file to all remote machines
4. ✅ Run setup on all remote nodes in parallel
5. ✅ Periodically check to ensure all nodes are successfully setup
6. ✅ Start all services on all remote nodes

### Alternative: Step-by-Step Deployment

1. **Prerequisites**
   ```bash
   # Install required tools
   brew install terraform gcloud jq  # macOS
   # or
   sudo apt install terraform google-cloud-cli jq  # Ubuntu
   
   # Authenticate with GCP
   gcloud auth login
   gcloud auth application-default login \
      --scopes="https://www.googleapis.com/auth/cloud-platform,https://www.googleapis.com/auth/userinfo.email,openid"

   ```

2. **Configure Variables**
   ```bash
   cd terraform
   cp terraform.tfvars.example terraform.tfvars
   # Edit terraform.tfvars with your GCP project ID
   ```

3. **Deploy Infrastructure**
   ```bash
   # Single command deployment (recommended):
   make deploy
   
   # Or manually:
   make init
   make plan
   make apply
   ```

4. **Check Status**
   ```bash
   make status
   make health
   make test-rpc
   ```

## 📁 Directory Structure

```
terraform/
├── main.tf                 # Main Terraform configuration
├── variables.tf            # Variable definitions
├── outputs.tf             # Output definitions
├── terraform.tfvars.example # Example variables file
├── Makefile               # Deployment management
├── scripts/
│   ├── bootstrap.sh       # Node initialization script
│   └── prepare-configs.sh # Local configuration preparation script
└── README.md             # This file
```

## 📋 File Reference

This section categorizes all files used in the FastEVM deployment process.

### Input Files (Configuration - Can Be Modified)

These are the files you can edit to customize your deployment:

#### Terraform Configuration Files
- **`main.tf`** - Main Terraform configuration for infrastructure (VMs, networking, disks, firewall rules)
- **`variables.tf`** - Variable definitions and default values
- **`outputs.tf`** - Output definitions (IPs, endpoints, SSH keys, etc.)
- **`terraform.tfvars`** - Variable values (project_id, region, zone, node_count, machine_type, etc.)
  - ⚠️ **Important**: Copy from `terraform.tfvars.example` and customize with your GCP project ID
  - This file is ignored by git (contains sensitive information)

#### Deployment Scripts (from `../scripts/` directory)
- **`scripts/setup.sh`** - Setup script for installing dependencies and building FastEVM
- **`scripts/genesis.json`** - Genesis block configuration
- **`scripts/node.sh`** - Node management script (start, stop, setup functions)

#### Terraform Scripts
- **`scripts/bootstrap.sh`** - Node initialization script (runs on instance creation)
- **`scripts/prepare-configs.sh`** - Local configuration preparation script
- **`scripts/prepare-binaries.sh`** - Binary preparation and distribution script
- **`scripts/deploy-all.sh`** - Legacy deployment orchestration script (use `make deploy` instead)

### Output Files (Generated - Do Not Edit)

These files are automatically generated during deployment:

#### Environment Files
- **`nodes.local.env`** - Environment file for local use with public IPs
  - Contains: `NODE1_EXTERNAL_IP`, `NODE1_HTTP_RPC`, etc.
  - Use this file on your local machine to access nodes
- **`nodes.remote.env`** - Environment file for remote nodes with internal IPs
  - Contains: `NODE1_INTERNAL_IP`, `NODE1_HTTP_RPC` (using internal IPs), etc.
  - Automatically copied to `/opt/fastevm/nodes.remote.env` on each node

#### SSH Keys (Generated by Terraform)
- **`fastevm-deploy-key`** - Private SSH key for node access
- **`fastevm-deploy-key.pub`** - Public SSH key
  - ⚠️ **Security**: These files are ignored by git and should never be committed

#### Terraform State Files
- **`terraform.tfstate`** - Current Terraform state (tracks deployed resources)
- **`terraform.tfstate.backup`** - Backup of previous state
- **`.terraform/`** - Terraform provider plugins and modules

#### Generated Configuration Files
- **`config/`** - Generated configurations (genesis.json, committees.yml, node configs, etc.)
- **`deploy/`** - Deployment packages for each node (node0/, node1/, etc.)
- **`deployment-info.json`** - Deployment summary with IPs and endpoints
- **`peer-config.json`** - Peer-to-peer configuration

### Intermediate Files (Ignored by Git)

These files are generated during the deployment process and are automatically ignored by `.gitignore`:

#### Terraform Intermediate Files
- **`*.tfplan`** - Terraform plan files
- **`*.tfstate.*`** - Terraform state backup files
- **`.terraform.lock.hcl`** - Terraform dependency lock file
- **`crash.log`** - Terraform crash logs

#### Build Artifacts
- **`binaries/`** - Compiled binaries (fastevm-execution, fastevm-consensus)
- **`target/`** - Rust build artifacts
- **`*.log`** - Log files from deployment and execution

#### Temporary Files
- **`*.tmp`**, **`*.temp`** - Temporary files
- **`backups/`** - Configuration backup files
- **`tmp/`**, **`temp/`** - Temporary directories

#### Credentials and Secrets
- **`*.pem`**, **`*.key`** - Private keys and certificates
- **`*.env`**, **`.env.*`** - Environment variable files
- **`jwt*.hex`** - JWT secret files
- **`service-account*.json`** - GCP service account credentials

#### IDE and Editor Files
- **`.vscode/`**, **`.idea/`** - IDE configuration directories
- **`*.swp`**, **`*.swo`** - Vim swap files
- **`.DS_Store`** - macOS system files

### Remote Node File Structure

After deployment, each node has the following structure in `/opt/fastevm/`:

```
/opt/fastevm/
├── setup.sh          # Setup script (copied from scripts/)
├── genesis.json      # Genesis block (copied from scripts/)
├── node.sh          # Node management script (copied from scripts/)
└── nodes.remote.env # Environment file with internal IPs (generated)
```

### File Lifecycle

1. **Before Deployment**:
   - Edit input files: `terraform.tfvars`, `main.tf`, `variables.tf` (if needed)
   - Ensure required scripts exist in `../scripts/` directory

2. **During Deployment**:
   - Terraform generates: SSH keys, state files
   - Scripts generate: environment files, configuration files
   - Files are copied to remote nodes

3. **After Deployment**:
   - Use `nodes.local.env` for local access
   - Remote nodes use `nodes.remote.env` for inter-node communication
   - Intermediate files can be safely ignored or cleaned up

## 🔧 Configuration

### Configuration Preparation Workflow

FastEVM supports two deployment approaches:

#### **Approach 1: Local Configuration + Remote Build (Recommended)**
```bash
# 1. Prepare all configurations locally
make prepare-configs

# 2. Review generated configurations
make show-configs

# 3. Deploy to remote nodes
make deploy-configs

# Alternative: Full workflow in one command
make prepare-and-deploy
```

This approach:
- ✅ Prepares all configurations locally for review
- ✅ Builds FastEVM remotely on target architecture
- ✅ Copies pre-prepared configs to remote nodes
- ✅ Generates Docker Compose files for local testing

#### **Approach 2: Terraform + Remote Build (Original)**
```bash
# Deploy infrastructure and build remotely
make quick-start
```

This approach:
- ✅ Creates GCP infrastructure with Terraform
- ✅ Builds FastEVM remotely on each node
- ✅ Generates configurations on each node

### Generated Configuration Files

When using `make prepare-configs`, the following files are generated:

```
config/
├── genesis.json              # Genesis block configuration
├── committees.yml            # Consensus committee configuration
├── parameters.yml            # Consensus parameters
├── docker-compose.yml        # Docker Compose configuration
├── network-summary.txt       # Network configuration summary
├── README.md                 # Comprehensive documentation
├── node0.yml to node3.yml    # Individual consensus node configurations
├── execution0.toml to execution3.toml  # Individual execution node configurations
└── jwt0.hex to jwt3.hex      # JWT secrets for each node

deploy/
├── deploy-all.sh             # Legacy deployment script (use `make deploy` instead)
└── node0/ to node3/          # Individual node deployment packages
    ├── deploy.sh             # Node deployment script
    ├── node.yml              # Node configuration
    ├── execution.toml         # Execution configuration
    ├── jwt.hex               # JWT secret
    ├── genesis.json          # Genesis block
    ├── committees.yml        # Committee configuration
    └── parameters.yml        # Parameters configuration
```

### Required Variables

- `project_id`: Your GCP project ID

### Optional Variables

- `project_name`: Resource name prefix (default: "fastevm")
- `region`: GCP region (default: "us-central1")
- `zone`: GCP zone (default: "us-central1-a")
- `node_count`: Number of nodes (default: 4)
- `machine_type`: Instance type (default: "e2-standard-4")
- `disk_type`: Disk type (default: "pd-standard")
- `disk_size`: Disk size in GB (default: 100)

## 🏗️ Infrastructure Components

### Compute Resources
- **4 VM Instances**: e2-standard-4 machines
- **Persistent Disks**: 100GB standard per node
- **Service Account**: For node operations

### Networking
- **VPC Network**: Isolated network for FastEVM
- **Subnet**: 10.0.0.0/24 CIDR block
- **Firewall Rules**: Allow necessary ports
- **Load Balancer**: External access (optional)

### Port Configuration
| Service | Port | Description |
|---------|------|-------------|
| HTTP RPC | 8545 | Ethereum RPC endpoints (all nodes) |
| WebSocket RPC | 8546 | WebSocket RPC endpoints (all nodes) |
| Engine API | 8551 | Engine API endpoints (all nodes) |
| Consensus API | 26657 | Consensus client APIs (all nodes) |
| P2P | 30303 | Peer-to-peer networking (all nodes) |

**Note**: All nodes use the same fixed ports for consistency and easier management.

## 🐳 Docker Compose Deployment

For local testing or development, you can use Docker Compose:

```bash
# 1. Prepare configurations
make prepare-configs

# 2. Deploy with Docker Compose
cd config
docker-compose up -d

# 3. Check status
docker-compose ps
docker-compose logs -f

# 4. Stop services
docker-compose down
```

This creates a local FastEVM network with the same configuration as the GCP deployment.

## 🚦 Deployment Commands

### Basic Commands
```bash
make help              # Show all available commands
make init              # Initialize Terraform
make plan              # Plan deployment
make apply             # Apply deployment
make destroy           # Destroy infrastructure
```

### Management Commands
```bash
make status            # Show deployment status
make instances         # Show GCP instance status
make health            # Check node health
make logs              # Show bootstrap logs
make test-rpc          # Test RPC endpoints
```

### Node Management
```bash
make ssh-node NODE=1   # SSH to specific node
make restart           # Restart all services
make update            # Update all nodes
make clean-data        # Clean node data
```

### Configuration Management
```bash
make prepare-configs   # Prepare all configurations locally
make show-configs      # Show prepared configuration summary
make deploy-configs    # Deploy prepared configurations to remote nodes
make distribute-binaries # Distribute binaries from build node to all nodes
make prepare-and-deploy # Full workflow: prepare + deploy
make outputs           # Show all outputs
make backup            # Backup configuration
make costs             # Show estimated costs
```

### Single Command Deployment
```bash
make deploy            # Complete deployment: infrastructure + copy files + setup + start services
```

## 📊 Monitoring

### Health Checks
- **Execution Client**: HTTP RPC endpoint
- **Engine API**: Authentication endpoint
- **Consensus Client**: API endpoint

### Logs
- **Bootstrap Logs**: `/var/log/fastevm-bootstrap.log`
- **Service Logs**: `journalctl -u fastevm-*`
- **Application Logs**: `/data/logs/`

### Monitoring Scripts
- `fastevm-health-check.sh`: Health check script
- `fastevm-monitor.sh`: System monitoring
- `fastevm-status.sh`: Detailed status

### Service Management
The deployment includes a comprehensive service management script (`service.sh`) that provides:

```bash
# Service management commands (run on remote nodes)
sudo bash /tmp/fastevm-config/service.sh install     # Install systemd services
sudo bash /tmp/fastevm-config/service.sh start       # Start services
sudo bash /tmp/fastevm-config/service.sh restart     # Restart services
sudo bash /tmp/fastevm-config/service.sh stop        # Stop services
sudo bash /tmp/fastevm-config/service.sh status      # Check service status
sudo bash /tmp/fastevm-config/service.sh logs        # View service logs
sudo bash /tmp/fastevm-config/service.sh follow      # Follow live logs
```

**Key Features:**
- **Fixed Ports**: All nodes use consistent ports (8545, 8546, 8551, 30303, 26657)
- **Separate Log Files**: Each service writes to dedicated log files in `/data/logs/`
- **Health Checks**: Built-in health monitoring for all services
- **Service Control**: Easy start/stop/restart functionality
- **Log Management**: Centralized logging with rotation support

## 🔐 Security

### Authentication
- **JWT Secrets**: Generated per node
- **Service Account**: Minimal permissions
- **Firewall Rules**: Restrictive by default

### Network Security
- **VPC Isolation**: Private network
- **Firewall Rules**: Port-specific access
- **Load Balancer**: Optional external access

## 💰 Cost Estimation

### Monthly Costs (Approximate)
- **4x e2-standard-4**: ~$200/month
- **4x 100GB SSD**: ~$40/month
- **Load Balancer**: ~$20/month
- **Total**: ~$260/month

*Costs may vary based on usage and GCP pricing*

## 🛠️ Troubleshooting

### Common Issues

1. **Authentication Errors**
   ```bash
   gcloud auth login
   gcloud auth application-default login \
      --scopes="https://www.googleapis.com/auth/cloud-platform,https://www.googleapis.com/auth/userinfo.email,openid"
   ```

2. **Permission Errors**
   ```bash
   gcloud projects add-iam-policy-binding PROJECT_ID \
     --member="serviceAccount:SA_EMAIL" \
     --role="roles/compute.instanceAdmin"
   ```

3. **Node Startup Issues**
   ```bash
   make logs
   make ssh-node NODE=1
   sudo journalctl -u fastevm-execution
   sudo journalctl -u fastevm-consensus
   
   # Check service status
   sudo bash /tmp/fastevm-config/service.sh status
   
   # View service logs
   sudo bash /tmp/fastevm-config/service.sh logs execution
   sudo bash /tmp/fastevm-config/service.sh logs consensus
   ```

4. **Configuration Issues**
   ```bash
   make show-configs      # Check prepared configurations
   make prepare-configs   # Regenerate configurations
   ```

5. **Network Connectivity**
   ```bash
   make health
   make test-rpc
   ```

6. **Port Conflicts (Fixed Ports)**
   Since all nodes use the same fixed ports, ensure no conflicts:
   ```bash
   # Check if ports are in use
   sudo netstat -tlnp | grep -E "(8545|8546|8551|30303|26657)"
   
   # Check service status
   sudo bash /tmp/fastevm-config/service.sh status
   
   # Restart services if needed
   sudo bash /tmp/fastevm-config/service.sh restart
   ```

### Debug Commands
```bash
# Check instance status
gcloud compute instances list

# Check firewall rules
gcloud compute firewall-rules list

# Check network
gcloud compute networks list

# SSH to node
gcloud compute ssh fastevm-node-1 --zone=us-central1-a
```

## 📁 File Management

> **Note**: For a complete reference of all files, see the [File Reference](#-file-reference) section above.

### Git Ignore
The repository includes comprehensive `.gitignore` files to exclude intermediate files. See the [Intermediate Files](#intermediate-files-ignored-by-git) section for details.

Key ignored items:
- **Terraform files**: `*.tfstate`, `*.tfplan`, `.terraform/`
- **Generated configs**: `config/`, `deploy/`, `backups/`
- **Build artifacts**: `target/`, `*.log`, `*.tmp`
- **Credentials**: `*.pem`, `*.key`, `*.json`, `fastevm-deploy-key*`
- **IDE files**: `.vscode/`, `.idea/`, `*.swp`
- **Environment files**: `*.env`, `nodes.*.env`

### Directory Structure
```
terraform/
├── .gitignore              # Git ignore rules
├── config/                 # Generated configurations (ignored)
│   └── .gitkeep           # Preserve empty directory
├── deploy/                 # Deployment packages (ignored)
│   └── .gitkeep           # Preserve empty directory
├── backups/               # Configuration backups (ignored)
│   └── .gitkeep           # Preserve empty directory
└── scripts/               # Deployment scripts
    ├── bootstrap.sh       # Node initialization
    └── prepare-configs.sh # Configuration preparation
```

## 🔄 Development Workflow

### Local Development
```bash
make dev              # Format, validate, plan
make quick-start      # Full deployment
```

### Configuration Development
```bash
make prepare-configs   # Generate local configurations
make show-configs      # Review generated configs
# Edit configurations as needed
make deploy-configs    # Deploy to remote nodes
```

### Production Deployment
```bash
make prod             # Full production workflow
```

### Updates
```bash
make update           # Update all nodes
make restart          # Restart services
```

## 📝 Customization

### Custom Configuration
1. Edit `terraform.tfvars`
2. Modify `variables.tf` for new variables
3. Update `main.tf` for infrastructure changes
4. Customize `scripts/bootstrap.sh` for node setup

### Adding Nodes
1. Increase `node_count` in `terraform.tfvars`
2. Run `terraform plan` to see changes
3. Apply changes with `terraform apply`

### Custom Images
1. Build custom Docker images
2. Update image references in `bootstrap.sh`
3. Redeploy with `make update`

## 📚 Additional Resources

- [Terraform GCP Provider](https://registry.terraform.io/providers/hashicorp/google/latest)
- [GCP Compute Engine](https://cloud.google.com/compute)
- [FastEVM Documentation](../README.md)
- [Docker Compose Setup](../docker-compose.yml)

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes
4. Test with `make dev`
5. Submit a pull request

## 🧪 Client Node Testing

FastEVM includes a dedicated client node for comprehensive testing of your deployed network. The client node is deployed separately and can test external FastEVM networks.

### Quick Start with Client Node

```bash
# 1. Deploy your FastEVM network (main deployment)
make deploy

# 2. Deploy the client node (separate deployment)
cd client-node
make quick-start

# 3. Wait for setup to complete (2-3 minutes)
make status

# 4. Setup test configuration
make setup-config

# 5. Configure test settings
make connect
# Edit /home/ubuntu/test-config/fastevm.env with actual node IPs from main deployment

# 6. Run tests
make run-scan      # Block scan test
make run-batch     # Batch transaction test
make run-all       # All tests
```

### Client Node Features

#### Standalone Deployment
- ✅ Completely separate from main FastEVM network
- ✅ Own VPC network and resources
- ✅ Independent Terraform state
- ✅ Can test external FastEVM networks

#### Automatic Setup
The client node automatically:
- ✅ Installs Rust toolchain and dependencies
- ✅ Clones FastEVM repository
- ✅ Builds the test binary (`fastevm-test`)
- ✅ Creates test configuration templates
- ✅ Sets up log rotation and monitoring
- ✅ Configures systemd service for health monitoring

#### Comprehensive Test Runner
- ✅ Color-coded output with progress tracking
- ✅ Error handling and detailed logging
- ✅ Configuration management via environment files
- ✅ Multiple test types (scan, batch, range, all)
- ✅ Real-time progress updates and ETA calculations

### Client Node Commands

#### Deployment Commands
```bash
cd client-node
make init              # Initialize Terraform
make plan              # Plan deployment
make apply             # Deploy client node
make destroy           # Destroy client node
make status            # Show deployment status
```

#### Client Management
```bash
make connect           # Connect to client node via SSH
make setup-config      # Setup test configuration
make run-test TEST=<type>  # Run specific test type
make run-scan          # Run block scan test
make run-batch         # Run batch transaction test
make run-all           # Run all tests
```

#### Maintenance Commands
```bash
make update-code       # Update code and rebuild
make clean-logs        # Clean logs on client node
make clean-all         # Clean all local files
make outputs           # Show all outputs
```

### Client Node Configuration

#### Test Configuration File
The client node uses `/home/ubuntu/test-config/fastevm.env` for configuration:

```bash
# RPC Endpoints (update with actual node IPs from main deployment)
RPC_URL1=http://node1-ip:8545
RPC_URL2=http://node2-ip:8545
RPC_URL3=http://node3-ip:8545
RPC_URL4=http://node4-ip:8545

# Network Configuration
CHAIN_ID=202501

# Test Parameters
TEST_SENDER_COUNT=100
TEST_TRANSACTION_COUNT=1
TEST_TRANSACTION_VALUE=1000000000000000
TEST_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
TEST_WAITING_TIME_SECONDS=30
TEST_FETCH_NONCE=false
```

#### Terraform Variables
Key variables for client node configuration:

```hcl
variable "project_id" {
  description = "The GCP project ID"
  type        = string
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

variable "client_subnet_cidr" {
  description = "CIDR block for the client subnet"
  type        = string
  default     = "10.1.0.0/24"
}
```

### Client Node Architecture

#### Network Configuration
- Client node has its own VPC network (`10.1.0.0/24`)
- Separate from main FastEVM network (`10.0.0.0/24`)
- Firewall rules allow external testing
- SSH access configured for management

#### Security
- Uses separate SSH key pair (`client-deploy-key`)
- Service account with minimal required permissions
- Firewall rules restrict access to necessary ports only

#### Monitoring
- Systemd service for basic monitoring
- Log rotation configured
- Status commands for health checking

### Client Node Commands (On Remote Node)

Once connected to the client node, you can use these commands:

```bash
# Test commands
make test-scan      # Run block scan test
make test-batch     # Run batch transaction test
make test-all       # Run all tests
make test-range     # Run range scan test

# Maintenance commands
make update-build   # Update code and rebuild
make status         # Show system status
make clean-logs     # Clean logs
```

### Client Node Troubleshooting

#### Common Issues

1. **Client node not found**
   ```bash
   cd client-node
   make status
   # If not found, deploy it
   make apply
   ```

2. **Tests failing**
   ```bash
   make status
   make update-code
   make connect
   cat /home/ubuntu/test-config/fastevm.env
   ```

3. **Connection issues**
   ```bash
   chmod 600 client-deploy-key
   make connect
   ```

#### Logs
- Setup logs: `/var/log/client-setup.log`
- Test output: Displayed in terminal
- System logs: Standard systemd journal

### Cost Optimization

The client node uses minimal resources:
- `e2-standard-2` machine type (2 vCPUs, 8GB RAM)
- 50GB SSD disk
- Estimated cost: ~$55/month

You can adjust these in `client-node/variables.tf`:
```hcl
variable "client_machine_type" {
  default = "e2-micro"  # Even smaller for basic testing
}
```

### Integration with Main Deployment

The client node is designed to test external FastEVM networks:

1. **Deploy your FastEVM network** using the main Terraform configuration
2. **Deploy the client node** using `cd client-node && make quick-start`
3. **Configure test settings** with actual node IPs from your FastEVM deployment
4. **Run comprehensive tests** to validate your deployment

### Directory Structure

```
terraform/
├── main.tf                 # Main FastEVM network
├── variables.tf            # Main network variables
├── outputs.tf             # Main network outputs
├── Makefile               # Main network management
├── README.md              # Main documentation
└── client-node/           # Client node subfolder
    ├── main.tf            # Client node Terraform
    ├── variables.tf       # Client node variables
    ├── Makefile          # Client node management
    ├── README.md         # Client node documentation
    └── scripts/
        └── client-setup.sh # Client setup script
```

---

**Ready to deploy?** Run `make quick-start` to get started! 🚀

**Want to test your deployment?** Deploy a client node with `cd client-node && make quick-start` and run comprehensive tests! 🧪
