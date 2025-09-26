# FastEVM Remote Deployment

This directory contains the Taskfile-based deployment system for FastEVM, using [Taskfile.dev](https://taskfile.dev/) for task automation.

## Overview

The deployment system automates the process of building and starting Docker containers on 4 remote SSH nodes. It provides a comprehensive set of tasks for deployment, monitoring, maintenance, and cleanup.

## Features

- **Task-based Architecture**: Uses Taskfile.dev for modern task automation
- **SSH Management**: Secure connections to remote nodes with key or password authentication
- **Docker Operations**: Automated building, starting, and managing Docker containers
- **File Transfer**: Efficient file upload using rsync and scp
- **Monitoring**: Comprehensive status checks and health monitoring
- **Cleanup**: Automated cleanup of resources and temporary files
- **Configuration**: Flexible configuration management with environment variables

## Quick Start

1. **Install Taskfile**:
   ```bash
   # macOS
   brew install go-task/tap/go-task
   
   # Linux
   sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin
   
   # Windows
   scoop install task
   ```

2. **Configure Remote Nodes**:
   ```bash
   cd deployment
   cp config/nodes.env.example config/nodes.env
   # Edit config/nodes.env with your remote node details
   ```

3. **Deploy FastEVM**:
   ```bash
   task deploy
   ```

## Configuration

### Environment Variables

Create a `config/nodes.env` file with your remote node configuration:

```bash
# Node 1 Configuration
NODE1_HOST=192.168.1.100
NODE1_USER=ubuntu
NODE1_PORT=22
NODE1_KEY_PATH=/path/to/private/key
# NODE1_PASSWORD=your_password_here

# Node 2 Configuration
NODE2_HOST=192.168.1.101
NODE2_USER=ubuntu
NODE2_PORT=22
NODE2_KEY_PATH=/path/to/private/key

# ... (similar for nodes 3 and 4)

# Project Configuration
REMOTE_PROJECT_PATH=/opt/fastevm
DOCKER_NETWORK=fastevm-network
DOCKER_SUBNET=172.20.0.0/16
```

### SSH Key Setup

For passwordless authentication, set up SSH keys:

```bash
# Generate SSH key if you don't have one
ssh-keygen -t rsa -b 4096 -f ~/.ssh/id_rsa

# Copy public key to all nodes
task ssh:setup-keys
```

## Available Tasks

### Main Deployment Tasks

- `task deploy` - Deploy to all 4 nodes
- `task deploy-quick` - Quick deployment (no file upload)
- `task deploy-node NODE=node1` - Deploy to specific node
- `task stop` - Stop all services
- `task restart` - Restart all services
- `task status` - Check service status
- `task clean` - Clean up everything

### Development Tasks

- `task dev` - Development deployment with hot reload
- `task update` - Update existing deployment
- `task deploy-dev` - Development deployment
- `task deploy-prod` - Production deployment

### Monitoring Tasks

- `task health` - Comprehensive health check
- `task logs` - Show service logs
- `task monitor:check-status` - Check status of all nodes
- `task monitor:check-connectivity` - Check network connectivity
- `task monitor:check-resources` - Check system resources

### Maintenance Tasks

- `task backup` - Backup configuration and data
- `task restore` - Restore from backup
- `task cleanup:full-cleanup` - Full cleanup of all resources
- `task cleanup:clean-docker` - Clean Docker resources
- `task cleanup:clean-system` - Clean system resources

### Emergency Tasks

- `task emergency-stop` - Emergency stop all services
- `task emergency-deploy` - Emergency deployment with minimal checks

## Available Tasks

### Main Tasks
- `deploy` - Deploy FastEVM to all 4 remote nodes
- `stop` - Stop all FastEVM services
- `status` - Check service status
- `setup` - Setup deployment environment

### Individual Tasks
- `test-connections` - Test SSH connections to all nodes
- `create-network` - Create Docker network on all nodes
- `upload-files` - Upload files to all nodes
- `build-images` - Build Docker images on all nodes
- `start-services` - Start Docker services on all nodes
- `check-status` - Check status of all nodes

### Utility Tasks
- `help` - Show available tasks and usage information

## Usage Examples

### Basic Deployment
```bash
# Deploy to all nodes
task deploy

# Check status
task status

# Stop services
task stop
```

### Individual Tasks
```bash
# Test connections
task test-connections

# Upload files only
task upload-files

# Build images only
task build-images

# Start services only
task start-services
```

### Setup and Configuration
```bash
# Setup environment
task setup

# Show help
task help
```

## File Structure

```
deployment/
├── Taskfile.yml              # Main task file (standalone)
├── config/                   # Configuration files
│   ├── nodes.env            # Node configuration (created by setup)
│   └── nodes.env.example    # Example node configuration
├── logs/                     # Log files and reports (created at runtime)
│   ├── backups/             # Backup files
│   └── reports/             # Monitoring reports
├── install.sh               # Setup script
└── README.md                # This file
```

## Prerequisites

- **Taskfile**: Install from [taskfile.dev](https://taskfile.dev/)
- **SSH Access**: SSH access to all 4 remote nodes
- **Docker**: Docker and Docker Compose installed on remote nodes
- **Required Tools**: ssh, scp, rsync, docker, docker-compose

## Security Considerations

- Use SSH key-based authentication when possible
- Keep private keys secure and use proper permissions
- Consider using SSH agent for key management
- Regularly rotate SSH keys
- Monitor SSH access logs

## Troubleshooting

### Common Issues

1. **SSH Connection Failed**:
   ```bash
   # Test SSH connection
   task ssh:test-connections
   
   # Setup SSH keys
   task ssh:setup-keys
   ```

2. **Docker Build Failed**:
   ```bash
   # Check Docker status
   task monitor:check-status
   
   # Clean and rebuild
   task cleanup:clean-docker
   task docker:build-images
   ```

3. **File Upload Failed**:
   ```bash
   # Test file transfer
   task ssh:test-transfer
   
   # Check disk space
   task monitor:check-resources
   ```

### Debug Mode

Enable debug logging by setting environment variables:

```bash
export LOG_LEVEL=debug
task deploy
```

### Logs and Reports

- **Service Logs**: `task logs`
- **System Logs**: `task monitor:check-logs`
- **Monitoring Report**: `task monitor:generate-report`
- **Local Logs**: Check `logs/` directory

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test your changes
5. Submit a pull request

## License

This project is part of the FastEVM project and follows the same license terms.

## Support

For issues and questions:
1. Check the troubleshooting section
2. Review the logs and reports
3. Check the [Taskfile.dev documentation](https://taskfile.dev/)
4. Open an issue in the repository
