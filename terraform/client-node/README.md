# FastEVM Client Node

A standalone Terraform configuration for deploying a FastEVM client node that can test and interact with FastEVM blockchain networks. The client node is completely separate from the main FastEVM network and can be deployed independently.

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

### Complete Automated Deployment

```bash
# Quick start (recommended) - Complete automation
make deploy-client
```

This single command will:
1. Initialize Terraform
2. Deploy client node infrastructure
3. Prepare configurations locally (auto-detects RPC URLs from main deployment)
4. Deploy configurations to remote client
5. Run setup script (bootstrap + configuration + automated testing)
6. Report completion status

**Alternative quick start:**
```bash
make help              # Show all available commands
```

### Step-by-Step Deployment

```bash
# Step 1: Deploy infrastructure
make init
make plan
make apply

# Step 2: Prepare and deploy configurations
make prepare-configs  # Auto-detects RPC URLs from main deployment
make deploy-configs   # Copies configs to remote client

# Step 3: Setup client node
make setup           # Runs bootstrap + configuration + automated tests
```

## 📁 Directory Structure

```
client-node/
├── main.tf                    # Main Terraform configuration
├── variables.tf               # Variable definitions
├── terraform.tfvars.example   # Example variables file
├── Makefile                   # Management commands
├── README.md                  # This comprehensive guide
├── scripts/
│   ├── prepare-configs.sh    # Prepare configurations locally
│   ├── deploy-configs.sh     # Deploy configurations to remote
│   └── client-setup.sh       # Legacy setup script (unused)
├── config/                    # Generated configurations
│   ├── test.env              # Test environment configuration
│   └── setup.sh             # Combined setup script
└── bootstrap.sh              # Legacy bootstrap script (unused)
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

### Environment Configuration

The client node configuration can be customized by editing the `../fastevm.env` file:

```bash
# Repository configuration
GITHUB_REPO=https://github.com/scalarorg/fastevm.git
GITHUB_BRANCH=terraform
CHAIN_ID=202501

# Test configuration defaults
TEST_SENDER_COUNT=10000
TEST_TRANSACTION_COUNT=10
TEST_TRANSACTION_VALUE=1000000000000000
TEST_MNEMONIC='abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about'
TEST_FETCH_NONCE=true
TEST_WAITING_TIME_SECONDS=30
TEST_RPC_TIMEOUT=30
TEST_MAX_RETRIES=3
TEST_LOG_LEVEL=info
```

**Key Configuration Variables:**
- `GITHUB_REPO`: FastEVM repository URL
- `GITHUB_BRANCH`: Branch to use for deployment
- `CHAIN_ID`: Blockchain chain ID
- `TEST_SENDER_COUNT`: Number of test accounts to create
- `TEST_TRANSACTION_COUNT`: Number of transactions per test
- `TEST_TRANSACTION_VALUE`: Value in wei for test transactions
- `TEST_MNEMONIC`: Mnemonic phrase for test accounts
- `TEST_FETCH_NONCE`: Whether to fetch nonce from network
- `TEST_WAITING_TIME_SECONDS`: Wait time between operations
- `TEST_RPC_TIMEOUT`: RPC request timeout in seconds
- `TEST_MAX_RETRIES`: Maximum retry attempts
- `TEST_LOG_LEVEL`: Logging level (debug, info, warn, error)

The `prepare-configs.sh` script automatically loads these values from `../fastevm.env` and uses them as defaults when generating the client configuration.

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

### Deployment Commands
```bash
make init              # Initialize Terraform
make plan              # Plan deployment
make apply             # Deploy client node
make destroy           # Destroy client node
make status            # Show deployment status
```

### Configuration Commands
```bash
make prepare-configs   # Prepare configurations locally (auto-detects RPC URLs)
make deploy-configs   # Deploy configurations to remote client
make setup            # Run setup script on remote client (bootstrap + configure)
```

### Client Management
```bash
make connect           # Connect to client node via SSH
make setup-config      # Setup test configuration on client node
make run-test TEST=<type>  # Run specific test (scan-all, batch, range, all)
make run-scan [START_NUMBER=X] [COUNTER=Y]  # Run block scan test with optional parameters
make run-batch         # Run batch transaction test
make run-all           # Run all tests
```

### Testing Commands (Local)
```bash
make test-scan-all     # Run block scan all test locally
make test-batch        # Run batch transaction test locally
make test-range        # Run block range test locally
```

### Automated Testing
```bash
make auto-test         # Run automated test sequence (get block -> batch -> sleep -> scan)
```

### Maintenance Commands
```bash
make update-code       # Update code and rebuild on client node
make clean-logs        # Clean logs on client node
make clean-all         # Clean all local files
make outputs           # Show all outputs
```

### Quick Start Commands
```bash
make deploy-client     # Complete client node setup (init + apply + prepare-configs + deploy-configs + setup)
make help              # Show all available commands
```

## 🧪 Testing

### Automatic Configuration Detection

The system automatically detects RPC URLs from your main FastEVM deployment:

1. **Primary**: Uses `../deployment-info.json` if available
2. **Fallback**: Uses `../terraform.tfstate` if deployment-info.json not found
3. **Preference**: Uses internal IPs first, falls back to external IPs

### Automated Test Sequence

The setup script automatically runs a comprehensive test sequence:

1. **Block Scan Test** (60s timeout) - Verify network connectivity
2. **Batch Transaction Test** (300s timeout) - Send multiple transactions
3. **Final Block Scan Test** (60s timeout) - Verify transactions were processed

### Manual Testing

#### Remote Testing (on client node)
```bash
# SSH into client node
make connect

# Run tests manually
cd /home/ubuntu
source /home/ubuntu/client-config/test.env

# Individual tests
fastevm-test scan      # Block scan test
fastevm-test batch     # Batch transaction test
fastevm-test range     # Block range test
```

#### Local Testing (from your machine)
```bash
# Run tests remotely from local machine
make run-scan [START_NUMBER=10] [COUNTER=5]  # Block scan test with optional parameters
make run-batch         # Batch transaction test
make run-all           # All tests
make run-test TEST=scan-all  # Specific test type

# Automated test sequence
make auto-test         # Get current block -> run batch -> wait -> run scan
```

#### Local Testing (if you have the binary locally)
```bash
# Run tests locally (requires local fastevm-test binary)
make test-scan-all     # Block scan all test
make test-batch        # Batch transaction test  
make test-range        # Block range test
```

## 💰 Cost Estimation

### Monthly Costs (Approximate)
- **1x e2-standard-2**: ~$50/month
- **1x 50GB SSD**: ~$5/month
- **Total**: ~$55/month

*Costs may vary based on usage and GCP pricing*

## 🔗 Integration with Main FastEVM Network

### Prerequisites
1. **Deploy your FastEVM network** using the main Terraform configuration
2. **Ensure deployment-info.json exists** in the main terraform directory
3. **Deploy this client node** using `make quick-start`

### Integration Steps
1. **Automatic Detection**: Client node automatically detects RPC URLs from main deployment
2. **Configuration**: Test configuration is generated with correct internal IPs
3. **Testing**: Comprehensive automated testing validates the integration
4. **Validation**: All tests must pass before considering the setup complete

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
   make apply
   ```

3. **RPC URL detection failed**
   ```bash
   # Check if main deployment exists
   ls ../deployment-info.json
   ls ../terraform.tfstate
   
   # Manual configuration
   make connect
   nano /home/ubuntu/client-config/test.env
   ```

4. **Tests failing**
   ```bash
   make status
   make connect
   tail -f /var/log/client-setup.log
   cat /home/ubuntu/client-config/test.env
   
   # Try running tests manually
   make run-scan
   make auto-test
   ```

5. **Connection issues**
   ```bash
   chmod 600 client-deploy-key
   make connect
   ```

### Logs and Monitoring

- **Setup logs**: `/var/log/client-setup.log` (on client node)
- **Bootstrap logs**: `/var/log/client-bootstrap.log` (on client node)
- **Configuration logs**: `/var/log/client-config.log` (on client node)
- **Test output**: Displayed in terminal
- **System logs**: Standard systemd journal

### Debug Commands

```bash
# Check network connectivity
ping <node-ip>

# Check RPC endpoint
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://<node-ip>:8545

# View logs remotely
make connect
tail -f /var/log/client-setup.log
```

## 📚 Advanced Usage

### Custom Configuration

If you need to customize the configuration:

```bash
# Prepare configurations
make prepare-configs

# Edit the generated configuration
nano config/test.env

# Deploy with custom configuration
make deploy-configs
make setup
```

### Integration with CI/CD

The client node can be integrated into CI/CD pipelines:

```bash
# In your CI/CD pipeline
make quick-start
if [ $? -eq 0 ]; then
    echo "Client node deployment successful"
    make run-all
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

**Ready to deploy?** Run `make deploy-client` to get started! 🚀

**Need to test your FastEVM network?** This client node provides comprehensive testing capabilities! 🧪

**Want to see all available commands?** Run `make help` for a complete list! 📋