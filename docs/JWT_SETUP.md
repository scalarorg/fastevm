# FastEVM JWT Secret Setup

This document describes how the JWT secret generation and configuration system works in FastEVM.

## Overview

The FastEVM system uses JWT (JSON Web Token) secrets for secure communication between execution clients and consensus clients. The updated `init.sh` script automatically:

1. Generates unique JWT secrets for each execution client node
2. Updates consensus client configuration files with the corresponding JWT secrets
3. Creates backups of existing configuration files
4. Handles both existing and new configuration files

## How It Works

### 1. JWT Secret Generation

The `execution-client/shared/init.sh` script generates JWT secrets using cryptographically secure random data:

```bash
dd if=/dev/urandom bs=32 count=1 2>/dev/null | xxd -p -c 64 > jwt.hex
```

This creates a 64-character hexadecimal string (32 bytes) that serves as the JWT secret.

### 2. Configuration Update Process

For each node, the script:

1. **Generates JWT secret** in `/data{N}/jwt.hex`
2. **Copies genesis file** from template
3. **Updates consensus config** with the JWT secret
4. **Creates backup** of existing config files

### 3. Node Mapping

The system maps execution client data directories to consensus client config files:

| Execution Client | Consensus Client Config |
|------------------|-------------------------|
| `/data1`         | `node0.yml`            |
| `/data2`         | `node1.yml`            |
| `/data3`         | `node2.yml`            |
| `/data4`         | `node3.yml`            |

## Usage

### Basic Setup

Run the initialization script from the project root:

```bash
./scripts/init.sh
```

### Verification Mode

To check JWT secrets without updating files:

```bash
./scripts/update-jwt-secrets.sh --verify
```

### Manual Update

To manually update JWT secrets in existing configs:

```bash
./scripts/update-jwt-secrets.sh
```

### Testing

Use the test script to verify the setup:

```bash
./scripts/test-init.sh
```

## File Structure

```
scripts/
├── init.sh                        # Main initialization script
├── update-jwt-secrets.sh          # JWT secret update utility
├── test-init.sh                   # Test script
└── ...

execution-client/
├── shared/
│   ├── genesis.template.json      # Genesis template
│   └── genesis.json              # Generated genesis file
├── data1/
│   ├── jwt.hex                   # JWT secret for node 0
│   └── genesis.json              # Genesis for node 0
├── data2/
│   ├── jwt.hex                   # JWT secret for node 1
│   └── genesis.json              # Genesis for node 1
└── ...

consensus-client/
├── examples/
│   ├── node.template.yml         # Config template
│   ├── node0.yml                 # Config for node 0
│   ├── node1.yml                 # Config for node 1
│   ├── node2.yml                 # Config for node 2
│   └── node3.yml                 # Config for node 3
└── ...
```

## Configuration Template

The consensus client configuration template (`node.template.yml`) contains placeholders:

```yaml
# Node {NODE_INDEX} configuration for FastEVM Consensus Client
execution_url: "http://fastevm-execution{NODE_INDEX}:8551"
jwt_secret: "{JWT_SECRET}"
node_index: {NODE_INDEX}
```

These placeholders are automatically replaced during the initialization process.

## Security Features

- **Cryptographically secure** JWT secret generation
- **Automatic backup** creation before modifications
- **Validation** of JWT secret format (64-character hex with 0x prefix)
- **Error handling** with detailed logging
- **Rollback capability** using backup files

## Error Handling

The script includes comprehensive error handling:

- **Permission checks** for write access
- **File existence validation** for required files
- **JWT format validation** to ensure proper hex strings
- **Backup creation** before any modifications
- **Detailed logging** with color-coded output

## Troubleshooting

### Common Issues

1. **Permission Denied**: Ensure the script has write access to the project directory
2. **Missing Directories**: Run from the project root directory
3. **Invalid JWT Format**: Check that JWT files contain valid 64-character hex strings
4. **Backup Files**: Remove old backup files if disk space is limited

### Recovery

If something goes wrong:

1. **Check backup files**: Look for `.backup.YYYYMMDD_HHMMSS` files
2. **Restore from backup**: Copy the backup file back to the original location
3. **Re-run initialization**: Execute `init.sh` again

## Testing

Use the test script to verify the setup:

```bash
./scripts/test-init.sh
```

This creates a test environment and demonstrates the complete process.

## Best Practices

1. **Always backup** before running the script
2. **Verify JWT secrets** are properly formatted
3. **Check consensus configs** after updates
4. **Test communication** between execution and consensus clients
5. **Monitor logs** for any errors or warnings

## Dependencies

- `bash` shell with associative array support
- `dd`, `xxd` for JWT generation
- `sed` for file modifications
- `find` for file discovery
- Color terminal support (optional)

## Future Enhancements

- **Configurable node count** via environment variables
- **Custom JWT secret sources** (e.g., from external key management)
- **Automatic validation** of client communication
- **Integration testing** with actual client binaries
