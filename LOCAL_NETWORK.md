# FastEVM Local Network Guide

This comprehensive guide explains how to run the FastEVM network locally without Docker for faster development and testing.

## 🎉 Setup Complete!

I've successfully analyzed your Docker Compose file and created a comprehensive local development setup that allows you to run the FastEVM network without Docker for much faster development cycles.

## Quick Start

```bash
# Test the setup
make local-test-setup

# Start the local network (4 execution + 4 consensus nodes)
make local-network

# Or use the helper script
./scripts/local-dev.sh start

# Check status
make local-status

# View logs
make local-logs

# Stop the network
make local-stop
```

## Prerequisites

- **Rust**: Install from [rustup.rs](https://rustup.rs/)
- **OpenSSL**: For JWT secret generation
- **jq**: For JSON processing
- **curl**: For testing connectivity

### Install Dependencies

**macOS:**
```bash
brew install openssl jq curl
```

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install openssl jq curl build-essential
```

## Available Commands

### Makefile Targets

| Command | Description |
|---------|-------------|
| `make local-network` | Start the full local network |
| `make local-start` | Start the local network |
| `make local-stop` | Stop the local network |
| `make local-restart` | Restart the local network |
| `make local-status` | Show network status |
| `make local-logs` | Show all logs |
| `make local-test` | Test network connectivity |
| `make local-cleanup` | Clean up all local data |
| `make local-init` | Initialize node data only |
| `make local-test-setup` | Test the setup |

### Helper Script Commands

```bash
# Network management
./scripts/local-dev.sh start          # Start network
./scripts/local-dev.sh stop           # Stop network
./scripts/local-dev.sh restart        # Restart network
./scripts/local-dev.sh status         # Show status
./scripts/local-dev.sh logs [service] # Show logs
./scripts/local-dev.sh test           # Test connectivity
./scripts/local-dev.sh clean          # Clean up
./scripts/local-dev.sh dev            # Development mode

# Development
./scripts/local-dev.sh build          # Build project
./scripts/local-dev.sh check          # Check code
./scripts/local-dev.sh test-unit      # Run unit tests
./scripts/local-dev.sh test-integration # Run integration tests
./scripts/local-dev.sh fmt            # Format code
./scripts/local-dev.sh clippy         # Run linter
```

## Network Architecture

The local network consists of:

### Execution Nodes (4 nodes)
- **Node 1**: RPC port 8545, Engine API port 8551, P2P port 30303
- **Node 2**: RPC port 8547, Engine API port 8552, P2P port 30304
- **Node 3**: RPC port 8549, Engine API port 8553, P2P port 30305
- **Node 4**: RPC port 8555, Engine API port 8554, P2P port 30306

### Consensus Nodes (4 nodes)
- **Node 1**: Port 26657, IP 127.0.0.1
- **Node 2**: Port 26658, IP 127.0.0.1
- **Node 3**: Port 26659, IP 127.0.0.1
- **Node 4**: Port 26660, IP 127.0.0.1

## Directory Structure

```
fastevm/
├── .local-data/           # Node data directories
│   ├── execution1/        # Execution node 1 data
│   ├── execution2/        # Execution node 2 data
│   ├── execution3/        # Execution node 3 data
│   ├── execution4/        # Execution node 4 data
│   ├── consensus1/        # Consensus node 1 data
│   ├── consensus2/        # Consensus node 2 data
│   ├── consensus3/        # Consensus node 3 data
│   └── consensus4/        # Consensus node 4 data
├── .local-logs/           # Log files
│   ├── execution-node1.log
│   ├── execution-node2.log
│   ├── execution-node3.log
│   ├── execution-node4.log
│   ├── consensus-node1.log
│   ├── consensus-node2.log
│   ├── consensus-node3.log
│   └── consensus-node4.log
├── .local-pids/           # Process ID files
│   ├── execution-node1.pid
│   ├── execution-node2.pid
│   ├── execution-node3.pid
│   ├── execution-node4.pid
│   ├── consensus-node1.pid
│   ├── consensus-node2.pid
│   ├── consensus-node3.pid
│   └── consensus-node4.pid
└── scripts/               # Development scripts
    ├── local-network.sh   # Main network management script
    ├── local-dev.sh       # Helper script with convenient commands
    └── test-local-setup.sh # Test script to verify setup
```

## Development Workflow

### 1. Initial Setup
```bash
# Clone and build the project
git clone <repository>
cd fastevm

# Build the project
make build-release

# Initialize node data
make local-init
```

### 2. Start Development
```bash
# Start the network
make local-network

# In another terminal, check status
make local-status

# View logs
make local-logs

# Test connectivity
make local-test
```

### 3. Development Cycle
```bash
# Make code changes
# ...

# Rebuild (if needed)
make build-release

# Restart network
make local-restart

# Check logs for issues
make local-logs execution-node1
```

### 4. Cleanup
```bash
# Stop the network
make local-stop

# Clean up all data
make local-cleanup
```

## Testing the Network

### Test Execution Nodes
```bash
# Test RPC endpoints
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Test Engine API
curl -X POST http://localhost:8551 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"engine_getPayloadV2","params":["0x123"],"id":1}'
```

### Test All Nodes
```bash
# Use the built-in test command
make local-test
```

## 🧪 Testing

Run the test script to verify everything works:

```bash
make local-test-setup
```

This will test:
- Prerequisites installation
- Project structure
- Script permissions
- Build system
- Network initialization
- Network startup (brief test)
- Cleanup functionality

## Troubleshooting

### Common Issues

1. **Port already in use**
   ```bash
   # Check what's using the port
   lsof -i :8545
   
   # Kill the process
   kill -9 <PID>
   ```

2. **Build failures**
   ```bash
   # Clean and rebuild
   make clean
   make build-release
   ```

3. **Node startup failures**
   ```bash
   # Check logs
   make local-logs execution-node1
   
   # Check if data directory exists
   ls -la .local-data/execution1/
   ```

4. **Network connectivity issues**
   ```bash
   # Test individual ports
   curl -s http://localhost:8545
   
   # Check if processes are running
   make local-status
   ```

5. **Permission issues**
   ```bash
   chmod +x scripts/*.sh
   ```

6. **Missing dependencies**
   ```bash
   # Check what's missing
   make local-test-setup
   ```

### Log Locations

- **Execution nodes**: `.local-logs/execution-node*.log`
- **Consensus nodes**: `.local-logs/consensus-node*.log`
- **All logs**: `make local-logs`

### Process Management

- **Check running processes**: `make local-status`
- **Stop all processes**: `make local-stop`
- **Force cleanup**: `make local-cleanup`

## Performance Tips

1. **Use release builds**: Always use `make build-release` for better performance
2. **Monitor resources**: Use `htop` or `top` to monitor CPU/memory usage
3. **Check logs regularly**: Monitor logs for errors or warnings
4. **Clean up regularly**: Use `make local-cleanup` to free up disk space

## Integration with Docker

The local development setup is designed to be independent of Docker, but you can still use Docker for specific components:

```bash
# Use Docker for specific services
docker-compose up -d execution-node1

# Use local development for others
make local-start
```

## 📊 Benefits Over Docker

1. **Faster Builds**: No Docker image building required
2. **Faster Startup**: Direct binary execution
3. **Better Debugging**: Direct access to logs and processes
4. **Resource Efficient**: Lower memory and CPU usage
5. **Easier Development**: Direct file access and modification

## 🎯 Key Features

- ✅ **4 Execution Nodes** with RPC and Engine API endpoints
- ✅ **4 Consensus Nodes** with proper networking
- ✅ **P2P Networking** between execution nodes
- ✅ **JWT Authentication** for Engine API
- ✅ **Genesis Block** initialization
- ✅ **Comprehensive Logging** for debugging
- ✅ **Process Management** with PID tracking
- ✅ **Easy Cleanup** and restart functionality
- ✅ **Health Checks** and connectivity testing
- ✅ **Development Mode** with auto-restart

## 🔄 Migration from Docker

The local setup is designed to be a drop-in replacement for your Docker setup:

- Same port configuration
- Same network architecture  
- Same data structure
- Same API endpoints
- Same logging format

You can switch between Docker and local development seamlessly!

## Next Steps

1. **Explore the code**: Check out the source code in `execution-client/` and `consensus-client/`
2. **Run tests**: Use `make test` to run the test suite
3. **Read documentation**: Check out the main README.md for more details
4. **Contribute**: Make changes and test them locally before submitting PRs

## Support

If you encounter issues:

1. Check the logs: `make local-logs`
2. Verify prerequisites are installed
3. Try cleaning up and starting fresh: `make local-cleanup && make local-network`
4. Check the main project documentation
5. Open an issue on GitHub with logs and error details

---

**Ready to start developing?** Run `make local-network` and begin coding! 🚀
