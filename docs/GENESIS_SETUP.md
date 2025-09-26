# FastEVM JWT Secret Setup

This document describes how the JWT secret generation and configuration system works in FastEVM.

## Overview

The FastEVM system uses JWT (JSON Web Token) secrets for secure communication between execution clients and consensus clients. The `init.sh` script automatically:

1. Generates unique JWT secrets for each execution client node
2. Updates consensus client configuration files with the corresponding JWT secrets
3. Extracts genesis block hashes from execution client initialization
4. Handles both existing and new configuration files

## How It Works

### 1. JWT Secret Generation

The `scripts/init.sh` script generates JWT secrets using OpenSSL for cryptographically secure random data:

```bash
openssl rand -hex 32 | tr -d "\n" > "$key_file"
```

This creates a 64-character hexadecimal string (32 bytes) that serves as the JWT secret.

### 2. Configuration Update Process

For each node, the script:

1. **Generates JWT secret** in `/execution{N}/jwt.hex`
2. **Copies genesis file** from shared directory
3. **Initializes execution client** to extract genesis block hash
4. **Updates consensus config** with the JWT secret and genesis block hash
5. **Creates consensus configs** from template files

### 3. Node Mapping

The system maps execution client data directories to consensus client config files:

| Execution Client | Consensus Client Config | Authority Index |
|------------------|-------------------------|-----------------|
| `/execution1`    | `/consensus1/node.yml` | 0               |
| `/execution2`    | `/consensus2/node.yml` | 1               |
| `/execution3`    | `/consensus3/node.yml` | 2               |
| `/execution4`    | `/consensus4/node.yml` | 3               |

## Usage

### Basic Setup

Run the initialization script from the project root:

```bash
./scripts/init.sh
```

### Consensus Committee Setup

Generate committee configuration for validator nodes:

```bash
./scripts/init.sh --consensus
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
├── test-init.sh                   # Test script
└── ...

execution-client/
├── shared/
│   └── genesis.json              # Shared genesis file
├── data1/
│   ├── jwt.hex                   # JWT secret for node 1
│   └── genesis.json              # Genesis for node 1
├── data2/
│   ├── jwt.hex                   # JWT secret for node 2
│   └── genesis.json              # Genesis for node 2
└── ...

consensus-client/
├── examples/
│   ├── node.template.yml         # Config template
│   └── ...
└── ...
```

## Configuration Template

The consensus client configuration template (`node.template.yml`) contains placeholders:

```yaml
# Node {NODE_INDEX} configuration for FastEVM Consensus Client
execution_url: "http://fastevm-execution{NODE_INDEX}:8551"
jwt_secret: "{JWT_SECRET}"
genesis_block_hash: "{GENESIS_BLOCK_HASH}"
node_index: {AUTHORITY_INDEX}
```

These placeholders are automatically replaced during the initialization process:
- `{NODE_INDEX}` → Node number (1, 2, 3, 4)
- `{AUTHORITY_INDEX}` → Authority index (0, 1, 2, 3)
- `{JWT_SECRET}` → Generated JWT secret
- `{GENESIS_BLOCK_HASH}` → Extracted genesis block hash

## Security Features

- **Cryptographically secure** JWT secret generation using OpenSSL
- **Automatic backup** creation before modifications
- **Validation** of JWT secret format (64-character hex)
- **Error handling** with detailed logging and color-coded output
- **Rollback capability** using backup files

## Error Handling

The script includes comprehensive error handling:

- **Permission checks** for write access
- **File existence validation** for required files
- **JWT format validation** to ensure proper hex strings
- **Backup creation** before any modifications
- **Detailed logging** with color-coded output (INFO, WARNING, ERROR)

## JWT Secret Format

JWT secrets are generated as 64-character hexadecimal strings and automatically formatted with `0x` prefix:

```
0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef
```

## Genesis Block Hash Extraction

The script automatically extracts genesis block hashes by running the execution client initialization:

```bash
fastevm-execution init --datadir $data_dir --chain $data_dir/genesis.json
```

The genesis block hash is extracted from the output and used in consensus client configuration.

## Consensus Committee Generation

The `--consensus` flag generates committee configuration for each validator node:

```bash
fastevm-consensus generate-committee \
  --output "/consensus${i}/committees.yml" \
  --authorities "4" \
  --epoch "0" \
  --stake "1000" \
  --ip-addresses "172.20.0.10,172.20.0.11,172.20.0.12,172.20.0.13" \
  --network-ports "26657,26657,26657,26657"
```

## Troubleshooting

### Common Issues

1. **Permission Denied**: Ensure the script has write access to the project directory
2. **Missing Directories**: Run from the project root directory
3. **Invalid JWT Format**: Check that JWT files contain valid 64-character hex strings
4. **Missing Genesis File**: Ensure `/shared/genesis.json` exists

### Recovery

If something goes wrong:

1. **Check logs**: Look for detailed error messages with color-coded output
2. **Verify files**: Check that required directories and files exist
3. **Re-run initialization**: Execute `./scripts/init.sh` again

## Testing

Use the test script to verify the setup:

```bash
./scripts/test-init.sh
```

This creates a test environment and demonstrates the complete process:
- Creates test data directories
- Sets up symbolic links
- Runs the initialization script
- Verifies JWT secrets and config files
- Cleans up test environment

## Best Practices

1. **Always run from project root**: Ensure you're in the correct directory
2. **Verify JWT secrets** are properly formatted with 0x prefix
3. **Check consensus configs** after updates
4. **Test communication** between execution and consensus clients
5. **Monitor logs** for any errors or warnings
6. **Use test script** before running in production

## Dependencies

- `bash` shell
- `openssl` for JWT generation
- `sed` for file modifications
- `fastevm-execution` binary for genesis initialization
- `fastevm-consensus` binary for committee generation
- Color terminal support for enhanced output

## Command Line Options

The `init.sh` script supports several command line options:

- **No arguments**: Full setup (default)
- **`--consensus` or `-c`**: Generate committee config for validator nodes
- **`--test` or `-t`**: Test genesis block hash extraction
- **`--help` or `-h`**: Show usage information

## Future Enhancements

- **Configurable node count** via environment variables
- **Custom JWT secret sources** (e.g., from external key management)
- **Automatic validation** of client communication
- **Integration testing** with actual client binaries
- **Backup and restore** functionality for configurations
