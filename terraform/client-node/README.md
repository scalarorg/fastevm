# FastEVM Client Node

A standalone Terraform configuration for deploying a FastEVM client node that can deploy and manage FastEVM blockchain networks. The client node is used as a control plane to deploy the network infrastructure.

## 🚀 Quick Start

### Prerequisites

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

### Two-Step Deployment

The deployment process consists of two simple steps:

**Step 1: Deploy and setup client node**
```bash
make deploy
```

This will:
1. Deploy the client node infrastructure on GCP
2. Copy `scripts/setup.sh` to the client node
3. Execute the setup script which:
   - Installs dependencies (Rust, Foundry, etc.)
   - Clones the FastEVM repository to `/opt/fastevm`
   - Builds the FastEVM binaries

**Step 2: Deploy network from client node**
```bash
make deploy-network
```

This will:
1. Connect to the client node
2. Execute terraform from `/opt/fastevm/terraform/network` to deploy the network

**View all commands:**
```bash
make help              # Show all available commands
```

## 📁 Directory Structure

```
client-node/
├── main.tf                    # Main Terraform configuration
├── variables.tf               # Variable definitions
├── terraform.tfvars.example   # Example variables file
├── Makefile                   # Management commands
├── README.md                  # This comprehensive guide
└── scripts/
    └── setup.sh               # Setup script (installs deps, clones repo, builds binaries)
```

## 🔧 Configuration

### Required Variables

- `project_id`: Your GCP project ID

### Optional Variables

- `project_name`: Resource name prefix (default: "fastevm-client")
- `region`: GCP region (default: "us-central1")
- `zone`: GCP zone (default: "us-central1-a")
- `client_machine_type`: Machine type (default: "e2-standard-2")
- `client_disk_size`: Disk size in GB (default: 50)
- `github_repo`: GitHub repository URL (default: "https://github.com/scalarorg/fastevm.git")
- `github_branch`: GitHub branch to clone (default: "main")

### Environment Variables

You can set these via environment variables or in `terraform.tfvars`:

```bash
export PROJECT_ID=your-gcp-project-id
export REGION=us-central1
export ZONE=us-central1-a
export PROJECT_NAME=fastevm-client
export GITHUB_REPO=https://github.com/scalarorg/fastevm.git
export GITHUB_BRANCH=gravity
```

Or create a `terraform.tfvars` file:

```hcl
project_id = "your-gcp-project-id"
region     = "us-central1"
zone       = "us-central1-a"
project_name = "fastevm-client"
github_repo = "https://github.com/scalarorg/fastevm.git"
github_branch = "gravity"
```

## 🏗️ Infrastructure Components

### Compute Resources
- **1 VM Instance**: e2-standard-2 machine
- **Persistent Disk**: 50GB SSD
- **Service Account**: For client operations

### Networking
- **VPC Network**: Isolated network for client node
- **Subnet**: 10.1.0.0/24 CIDR block
- **Firewall Rules**: SSH access and external testing

### Security
- **SSH Key Pair**: Generated automatically (`client-deploy-key`)
- **Service Account**: Minimal permissions
- **Firewall Rules**: Restrictive by default

## 🚦 Available Commands

### Main Commands
```bash
make deploy            # Step 1: Deploy client node and run initial setup
make deploy-network    # Step 2: Deploy network from client node
make status            # Show deployment status
make connect           # Connect to client node via SSH
make destroy           # Destroy client node
make clean             # Clean local files
make outputs           # Show all outputs
make help              # Show all available commands
```

### Terraform Commands
```bash
make init              # Initialize Terraform
make validate          # Validate Terraform configuration
make format            # Format Terraform files
make plan              # Plan deployment (included in deploy)
```

## 📊 Deployment Workflow

### Step 1: Deploy Client Node

```bash
make deploy
```

This command:
1. Initializes and validates Terraform
2. Plans the deployment
3. Applies the Terraform configuration to create the GCP instance
4. Copies `scripts/setup.sh` to the client node
5. Executes the setup script which:
   - Installs system dependencies (build tools, Rust, Foundry)
   - Clones the FastEVM repository to `/opt/fastevm`
   - Builds the FastEVM execution and consensus binaries

### Step 2: Deploy Network

After Step 1 completes and the source code is cloned to `/opt/fastevm`, run:

```bash
make deploy-network
```

This command:
1. Connects to the client node via SSH
2. Changes to `/opt/fastevm/terraform/network`
3. Executes `make deploy` to deploy the network infrastructure

### Check Status

```bash
make status
```

This shows:
- Client node IP address
- Whether source code is cloned to `/opt/fastevm`
- Whether network terraform directory is ready

## 💰 Cost Estimation

### Monthly Costs (Approximate)
- **1x e2-standard-2**: ~$50/month
- **1x 50GB SSD**: ~$5/month
- **Total**: ~$55/month

*Costs may vary based on usage and GCP pricing*

## 🔗 Network Deployment

The client node serves as a control plane for deploying the FastEVM network. After the client node is set up with the source code, you can deploy the network directly from it.

### Prerequisites
1. **Deploy client node** using `make deploy`
2. **Wait for setup to complete** (source code cloned to `/opt/fastevm`)
3. **Deploy network** using `make deploy-network`

### Network Deployment Process
1. **Client Setup**: Client node is deployed and configured with all dependencies
2. **Source Code**: FastEVM repository is cloned to `/opt/fastevm`
3. **Network Deployment**: Terraform is executed from `/opt/fastevm/terraform/network` to deploy the network infrastructure

## 🛠️ Troubleshooting

### Common Issues

1. **Authentication Errors**
   ```bash
   gcloud auth login
   gcloud auth application-default login \
      --scopes="https://www.googleapis.com/auth/cloud-platform,https://www.googleapis.com/auth/userinfo.email,openid"
   ```

2. **Client node not found**
   ```bash
   make status
   # If not found, deploy it
   make deploy
   ```

3. **Setup script failed**
   ```bash
   # Check status
   make status
   
   # Connect and check logs
   make connect
   tail -f ~/setup.sh.log  # or check the setup script output
   ```

4. **Source code not cloned**
   ```bash
   make connect
   ls -la /opt/fastevm
   # If missing, the setup script may have failed
   # Check the setup script execution logs
   ```

5. **Network deployment failed**
   ```bash
   make connect
   cd /opt/fastevm/terraform/network
   # Check terraform state and logs
   terraform show
   ```

6. **Connection issues**
   ```bash
   chmod 600 client-deploy-key
   make connect
   ```

### Logs and Monitoring

- **Setup script**: Executed during `make deploy`, output visible in terminal
- **Source code location**: `/opt/fastevm` (on client node)
- **Network terraform**: `/opt/fastevm/terraform/network` (on client node)
- **SSH access**: Use `make connect` to access the client node

### Debug Commands

```bash
# Check client node status
make status

# Connect to client node
make connect

# Check if source code is cloned
ls -la /opt/fastevm

# Check network terraform directory
ls -la /opt/fastevm/terraform/network

# View terraform outputs
make outputs
```

## 📚 Advanced Usage

### Custom Configuration

You can customize the deployment by:

1. **Setting environment variables**:
   ```bash
   export GITHUB_BRANCH=your-branch
   export PROJECT_NAME=my-fastevm-client
   make deploy
   ```

2. **Using terraform.tfvars**:
   ```bash
   cp terraform.tfvars.example terraform.tfvars
   # Edit terraform.tfvars with your values
   make deploy
   ```

3. **Modifying setup script**:
   ```bash
   # Edit scripts/setup.sh before running make deploy
   nano scripts/setup.sh
   make deploy
   ```

### Integration with CI/CD

The client node can be integrated into CI/CD pipelines:

```bash
# In your CI/CD pipeline
make deploy
if [ $? -eq 0 ]; then
    echo "Client node deployment successful"
    make deploy-network
    if [ $? -eq 0 ]; then
        echo "Network deployment successful"
    else
        echo "Network deployment failed"
        exit 1
    fi
else
    echo "Client node deployment failed"
    exit 1
fi
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes
4. Test with `make quick-start`
5. Submit a pull request

## 📚 Additional Resources

- [Main FastEVM Deployment](../README.md)
- [Terraform GCP Provider](https://registry.terraform.io/providers/hashicorp/google/latest)
- [GCP Compute Engine](https://cloud.google.com/compute)

---

**Ready to deploy?** Run `make deploy` to get started! 🚀

**Need to deploy the network?** After client setup, run `make deploy-network`! 🌐

**Want to see all available commands?** Run `make help` for a complete list! 📋