# FastEVM GCP Terraform Deployment

This directory contains Terraform configurations to deploy FastEVM nodes on Google Cloud Platform (GCP).

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
gcloud auth application-default login

# 3. Deploy everything in one command
cd terraform
make deploy-all
```

This single command will:
- ✅ Create GCP infrastructure (VMs, networking, disks)
- ✅ Prepare all node configurations locally
- ✅ Deploy and configure all nodes remotely
- ✅ Start all FastEVM services
- ✅ Verify deployment health

### Examples

```bash
# Basic deployment with 4 nodes
make deploy-all

# Deploy with 6 nodes
make deploy-nodes NODE_COUNT=6

# Deploy with custom project name
make deploy-project PROJECT_NAME=my-fastevm-network

# Deploy to different region
make deploy-region REGION=us-west1 ZONE=us-west1-a

# Deploy with larger machines
make deploy-machine MACHINE_TYPE=e2-standard-8

# Deploy to existing infrastructure (skip Terraform)
make deploy-existing

# Destroy and redeploy everything
make redeploy
```

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
   make deploy-all
   
   # Or use the original quick-start:
   make quick-start
   
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
├── deploy-all.sh             # Deploy all nodes script
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
- `disk_type`: Disk type (default: "pd-ssd")
- `disk_size`: Disk size in GB (default: 100)

## 🏗️ Infrastructure Components

### Compute Resources
- **4 VM Instances**: e2-standard-4 machines
- **Persistent Disks**: 100GB SSD per node
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
make prepare-and-deploy # Full workflow: prepare + deploy
make outputs           # Show all outputs
make backup            # Backup configuration
make costs             # Show estimated costs
```

### Single Command Deployment
```bash
make deploy-all        # Complete deployment: infrastructure + configs + deploy
make deploy-existing   # Deploy to existing infrastructure (skip Terraform)
make redeploy          # Destroy existing infrastructure and redeploy

# Custom deployment options
make deploy-nodes NODE_COUNT=6           # Deploy with 6 nodes
make deploy-project PROJECT_NAME=mynet   # Deploy with custom project name
make deploy-region REGION=us-west1 ZONE=us-west1-a  # Deploy to different region
make deploy-machine MACHINE_TYPE=e2-standard-8      # Deploy with larger machines
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
   gcloud auth application-default login
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

### Git Ignore
The repository includes comprehensive `.gitignore` files to exclude intermediate files:

- **Terraform files**: `*.tfstate`, `*.tfplan`, `.terraform/`
- **Generated configs**: `config/`, `deploy/`, `backups/`
- **Build artifacts**: `target/`, `*.log`, `*.tmp`
- **Credentials**: `*.pem`, `*.key`, `*.json`
- **IDE files**: `.vscode/`, `.idea/`, `*.swp`

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

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.

---

**Ready to deploy?** Run `make quick-start` to get started! 🚀
