# FastEVM Client Node

This directory contains a standalone Terraform configuration for deploying a FastEVM client node. The client node is completely separate from the main FastEVM network and can be deployed independently.

## 🚀 Quick Start

### 1. Prerequisites

```bash
# Install required tools
brew install terraform gcloud jq  # macOS
# or
sudo apt install terraform google-cloud-cli jq  # Ubuntu

# Authenticate with GCP
gcloud auth login
gcloud auth application-default login
```

### 2. Configure Variables

```bash
# Copy example configuration
cp terraform.tfvars.example terraform.tfvars

# Edit with your GCP project ID
nano terraform.tfvars
```

### 3. Deploy Client Node

```bash
# Quick start (recommended)
make quick-start

# Or step by step
make init
make plan
make apply
```

### 4. Setup and Test

```bash
# Wait for setup to complete (2-3 minutes)
make status

# Setup test configuration
make setup-config

# Connect to client node to edit configuration
make connect
# Edit /home/ubuntu/test-config/test.env with your FastEVM node IPs

# Run tests
make run-scan      # Block scan test
make run-batch     # Batch transaction test
make run-all       # All tests
```

## 📁 Directory Structure

```
client-node/
├── main.tf                    # Main Terraform configuration
├── variables.tf               # Variable definitions
├── terraform.tfvars.example   # Example variables file
├── Makefile                   # Management commands
├── README.md                  # This file
└── scripts/
    └── client-setup.sh        # Client node setup script
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
- `client_subnet_cidr`: Subnet CIDR (default: "10.1.0.0/24")

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
- **SSH Key Pair**: Generated automatically
- **Service Account**: Minimal permissions
- **Firewall Rules**: Restrictive by default

## 🚦 Available Commands

### Deployment Commands
```bash
make init              # Initialize Terraform
make plan              # Plan deployment
make apply             # Deploy client node
make destroy           # Destroy client node
make status            # Show deployment status
```

### Client Management
```bash
make connect           # Connect to client node via SSH
make setup-config      # Setup test configuration
make run-test TEST=<type>  # Run specific test
make run-scan          # Run block scan test
make run-batch         # Run batch transaction test
make run-all           # Run all tests
```

### Maintenance Commands
```bash
make update-code       # Update code and rebuild
make clean-logs        # Clean logs on client node
make clean-all         # Clean all local files
make outputs           # Show all outputs
```

## 🧪 Testing Configuration

The client node uses `/home/ubuntu/test-config/test.env` for configuration:

```bash
# RPC Endpoints (update with your FastEVM node IPs)
RPC_URL1=http://your-node1-ip:8545
RPC_URL2=http://your-node2-ip:8545
RPC_URL3=http://your-node3-ip:8545
RPC_URL4=http://your-node4-ip:8545

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

## 🔗 Integration with Main FastEVM Network

The client node is designed to test external FastEVM networks:

1. **Deploy your FastEVM network** using the main Terraform configuration
2. **Deploy this client node** using `make quick-start`
3. **Configure test settings** with actual node IPs from your FastEVM deployment
4. **Run comprehensive tests** to validate your deployment

## 💰 Cost Estimation

### Monthly Costs (Approximate)
- **1x e2-standard-2**: ~$50/month
- **1x 50GB SSD**: ~$5/month
- **Total**: ~$55/month

*Costs may vary based on usage and GCP pricing*

## 🛠️ Troubleshooting

### Common Issues

1. **Authentication Errors**
   ```bash
   gcloud auth login
   gcloud auth application-default login
   ```

2. **Client node not found**
   ```bash
   make status
   # If not found, deploy it
   make apply
   ```

3. **Tests failing**
   ```bash
   make status
   make update-code
   make connect
   cat /home/ubuntu/test-config/test.env
   ```

4. **Connection issues**
   ```bash
   chmod 600 client-deploy-key
   make connect
   ```

### Logs
- Setup logs: `/var/log/client-setup.log` (on client node)
- Test output: Displayed in terminal
- System logs: Standard systemd journal

## 🔄 Development Workflow

### Local Development
```bash
make dev              # Format, validate, plan
make quick-start      # Full deployment
```

### Testing
```bash
make setup-config     # Setup test configuration
make run-scan          # Run tests
make update-code       # Update and rebuild
```

### Cleanup
```bash
make destroy          # Destroy client node
make clean-all        # Clean local files
```

## 📚 Additional Resources

- [Main FastEVM Deployment](../README.md)
- [Terraform GCP Provider](https://registry.terraform.io/providers/hashicorp/google/latest)
- [GCP Compute Engine](https://cloud.google.com/compute)

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes
4. Test with `make quick-start`
5. Submit a pull request

---

**Ready to deploy?** Run `make quick-start` to get started! 🚀

**Need to test your FastEVM network?** This client node provides comprehensive testing capabilities! 🧪
