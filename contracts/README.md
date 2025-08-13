# FastEVM Smart Contracts

This folder contains the smart contracts for the FastEVM project, along with build scripts to compile them and generate the necessary configuration files.

## What Has Been Set Up

✅ **Complete build system for Solidity smart contracts using Foundry**

✅ **Automatic genesis.json generation with compiled bytecode**

✅ **Deterministic contract address calculation system**

✅ **Integration with main project Makefile**

✅ **Comprehensive documentation and scripts**

## Contracts

### FeeDistributor.sol

A smart contract that distributes incoming ETH payments to multiple recipients based on predefined shares. Only the contract owner can configure the recipients and their shares. The contract uses a queuing system where payments are stored in pending balances and distributed either manually or automatically after a time interval.

## Build System

The build system uses Foundry to compile Solidity contracts and generate the necessary artifacts for the FastEVM node configuration.

### Prerequisites

- [Foundry](https://getfoundry.sh/) (includes `forge`, `cast`, `anvil`)
- Node.js (v16 or higher, for genesis generation scripts)

### Quick Start Commands

#### Build Everything (Recommended)

```bash
# From project root
make build-contracts

# Or from contracts folder
./build.sh
```

#### Individual Steps

```bash
cd contracts
forge build       # Compile contracts
npm run genesis   # Generate genesis.json
npm run bytecode  # Extract bytecode for verification
npm run address   # Calculate contract address
```

### Generated Files

After building, the following files will be created:

- **`out/`** - Compiled contract artifacts (ABI, bytecode, etc.)
- **`cache/`** - Foundry compilation cache
- **`../../execution-client/shared/genesis.json`** - Genesis configuration with contract bytecode

### Scripts

- `build.sh` - Main build script that compiles contracts and generates genesis.json
- `scripts/generate-genesis.js` - Script that reads compiled artifacts and generates genesis.json
- `scripts/extract-bytecode.js` - Script to extract contract bytecode
- `scripts/calculate-address.js` - Script to calculate deterministic contract addresses
- `scripts/Deploy.s.sol` - Foundry deployment script

## Contract Address System

### Deterministic Address Calculation

The system now uses **deterministic contract addresses** calculated based on:

- **Contract name**: "FeeDistributor"
- **Chain ID**: 1337702
- **Project identifier**: "fastevm"

### Current Contract Address

**FeeDistributor**: `0x1f794d56307e0b6ee0119c0ef29dac40d44ec805`

### Benefits

- **Predictable**: Same address every time for the same contract and chain
- **Verifiable**: Address can be recalculated independently
- **No tracking needed**: No need to remember deployment addresses
- **Chain-specific**: Different addresses for different chains

## Integration with FastEVM

The compiled contract bytecode is automatically embedded in the genesis.json file at the calculated deterministic address. This allows the FastEVM node to start with the contract already deployed and ready to use.

## Development

To modify the contracts:

1. Edit the Solidity files
2. Run `./build.sh` to recompile and regenerate genesis.json
3. Restart your FastEVM node to use the updated contract

## Foundry Advantages

- **Faster Compilation**: Foundry compiles contracts significantly faster than Hardhat
- **Better Testing**: Built-in fuzzing and comprehensive testing framework
- **Gas Optimization**: Built-in gas reporting and optimization tools
- **Solidity Scripts**: Deployment scripts written in Solidity
- **Coverage Reports**: Detailed test coverage analysis

## Address Management Features

- **Automatic Calculation**: Addresses are calculated automatically during build
- **Consistent**: Same address across all environments for the same contract
- **Transparent**: Address calculation logic is open and verifiable
- **Scalable**: Easy to add new contracts with their own addresses

## Testing

Run the contract tests:

```bash
forge test
```

Run tests with verbose output:

```bash
forge test -vv
```

Generate coverage report:

```bash
forge coverage
```

## Deployment

Deploy to local network:

```bash
forge script scripts/Deploy.s.sol --rpc-url http://127.0.0.1:8551 --broadcast
```

Deploy to testnet/mainnet:

```bash
forge script scripts/Deploy.s.sol --rpc-url <RPC_URL> --broadcast --verify
```

## Clean

Remove build artifacts:

```bash
# From project root
make clean-contracts

# Or from contracts folder
forge clean
```

## Troubleshooting

- **"Foundry is not installed"**: Install Foundry with `curl -L https://foundry.paradigm.xyz | bash`
- **"Contract artifact not found"**: Run `forge build` first
- **"No runtime bytecode found"**: Check that the contract compiles without errors
- **Permission denied on build.sh**: Run `chmod +x build.sh` to make the script executable

## Success! 🚀

Your smart contract build system is now fully operational using Foundry and integrated with the FastEVM project. You can compile contracts and generate genesis configurations with a single command!

**Key Improvements Made:**

- ✅ Switched from Hardhat to Foundry for faster compilation
- ✅ Implemented deterministic contract address calculation
- ✅ Updated address: `0x1f794d56307e0b6ee0119c0ef29dac40d44ec805`
- ✅ Enhanced documentation and scripts
- ✅ Maintained full integration with the main project
