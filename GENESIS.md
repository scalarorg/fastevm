# Genesis Configuration Documentation

This document explains the configuration values in the `genesis.json` file used by the FastEVM network.

## Overview

The genesis block is the first block in the blockchain and defines the initial state of the network. It contains configuration parameters that determine how the blockchain operates, including hardfork activation blocks, initial account balances, and network parameters.

## Configuration Section (`config`)

### Chain Identification
- **`chainId`**: `202501`
  - Unique identifier for this blockchain network
  - Prevents transaction replay attacks between different networks
  - Used in EIP-155 transaction signing

### Hardfork Activation Blocks

All hardforks are activated at block 0, meaning the network starts with all Ethereum features enabled:

#### Early Hardforks (All at block 0)
- **`homesteadBlock`**: `0` - Homestead hardfork
  - Introduced gas cost changes and new opcodes
  - Removed the "Dust Limit" for transactions

- **`eip150Block`**: `0` - EIP-150 (Gas Reprice)
  - Increased gas costs for certain operations
  - Fixed DoS vulnerabilities

- **`eip155Block`**: `0` - EIP-155 (Simple Replay Attack Protection)
  - Introduced chain ID in transaction signatures
  - Prevents transaction replay across different networks

- **`eip158Block`**: `0` - EIP-158 (State Clearing)
  - Clears empty accounts with zero balance and code
  - Optimizes state storage

#### Byzantium Era (All at block 0)
- **`byzantiumBlock`**: `0` - Byzantium hardfork
  - Introduced REVERT opcode
  - Added new precompiled contracts
  - Modified difficulty adjustment algorithm

- **`constantinopleBlock`**: `0` - Constantinople hardfork
  - Introduced CREATE2 opcode
  - Reduced gas costs for certain operations
  - Added bitwise shifting instructions

- **`petersburgBlock`**: `0` - Petersburg hardfork
  - Reverted changes from Constantinople that caused issues
  - Kept beneficial Constantinople features

#### Istanbul Era (All at block 0)
- **`istanbulBlock`**: `0` - Istanbul hardfork
  - Added new precompiled contracts (BLAKE2b, secp256k1)
  - Modified gas costs for SLOAD and SSTORE operations
  - Improved privacy features

- **`muirGlacierBlock`**: `0` - Muir Glacier hardfork
  - Delayed the "Ice Age" (difficulty bomb)
  - No new features, just difficulty adjustment

#### Berlin Era (All at block 0)
- **`berlinBlock`**: `0` - Berlin hardfork
  - Introduced EIP-2929 (gas cost increases for state access)
  - Added EIP-2930 (optional access lists)
  - Improved transaction efficiency

#### London Era (All at block 0)
- **`londonBlock`**: `0` - London hardfork
  - **EIP-1559**: Introduced base fee and priority fee mechanism
  - **EIP-3198**: BASEFEE opcode
  - **EIP-3529**: Reduced gas refunds
  - **EIP-3541**: Reject contracts starting with 0xEF

#### Post-London Era (All at block 0)
- **`arrowGlacierBlock`**: `0` - Arrow Glacier hardfork
  - Further delayed the difficulty bomb
  - No new features

- **`grayGlacierBlock`**: `0` - Gray Glacier hardfork
  - Another difficulty bomb delay
  - No new features

### Time-based Hardforks

- **`shanghaiTime`**: `1700001200` (Unix timestamp)
  - Shanghai hardfork activation time
  - Enables EIP-4895 (Beacon Chain withdrawals)
  - Introduces PUSH0 opcode (EIP-3855)

- **`cancunTime`**: `1710000000` (Unix timestamp)
  - Cancun hardfork activation time
  - Enables EIP-4844 (Proto-Danksharding)
  - Introduces blob transactions

### Proof of Stake Configuration

- **`daoForkSupport`**: `true`
  - Enables support for the DAO hardfork
  - Allows recovery of funds from the 2016 DAO hack

- **`terminalTotalDifficulty`**: `"0x0"`
  - Total difficulty threshold for Proof of Work to Proof of Stake transition
  - Set to 0 means PoS is active from genesis

- **`terminalTotalDifficultyPassed`**: `true`
  - Indicates that the terminal total difficulty has been reached
  - Confirms PoS is active from the start

## Genesis Block Parameters

### Block Header Fields
- **`nonce`**: `"0x0"`
  - Nonce value for the genesis block
  - Not used in PoS networks

- **`timestamp`**: `"0x689b2cc0"` (1750001200 in decimal)
  - Unix timestamp when the genesis block was created
  - Used for time-based hardfork activation

- **`extraData`**: `"0x00"`
  - Additional data in the block header
  - Empty in this configuration

- **`gasLimit`**: `"0x1c9c380"` (30,000,000 in decimal)
  - Maximum gas allowed per block
  - Determines block size and transaction capacity

- **`difficulty`**: `"0x400000000"` (17,179,869,184 in decimal)
  - Initial difficulty for Proof of Work
  - Not used in PoS networks but required for compatibility

- **`mixHash`**: `"0x0000000000000000000000000000000000000000000000000000000000000000"`
  - Mix hash for Proof of Work
  - Not used in PoS networks

- **`coinbase`**: `"0x0000000000000000000000000000000000000000"`
  - Address that receives block rewards
  - Not used in PoS networks

- **`number`**: `"0x0"`
  - Block number (0 for genesis block)

## Account Allocations (`alloc`)

The genesis block pre-funds multiple accounts with initial balances:

### High Balance Accounts (4 accounts)
These accounts receive `0xd3c21bcecceda1000000` wei (1,000,000 ETH each):
- `0x86343d6826A67cFdBeB0F8A27B1B9A91BA68C047`
- `0x89fE9036dD10dCEf9aFA4490d366102f9BaE5425`
- `0x9526c275E44eFA546Cfca1782F12E7b7A039A1FF`
- `0x84190088eDC5dF5B21A7ED4C026D08f9CD8e9C5f`

### Smart Contract Account
- **Address**: `0xbd75b1d737d90e37d70502351d936374816a9bb2`
- **Balance**: `0x0` (no ETH)
- **Code**: Pre-deployed smart contract bytecode
- **Nonce**: `0x0`

### Standard Balance Accounts (70+ accounts)
These accounts receive `0x3635c9adc5dea00000` wei (1,000 ETH each):
- Multiple test accounts for development and testing
- Used for transaction testing and network validation

## Network Characteristics

### Consensus Mechanism
- **Type**: Proof of Stake (PoS) from genesis
- **No Mining**: All hardforks activated from block 0
- **Beacon Chain**: Integrated with execution layer

### Transaction Support
- **Legacy Transactions**: Supported (Type 0)
- **EIP-2930 Transactions**: Supported (Type 1) - Access lists
- **EIP-1559 Transactions**: Supported (Type 2) - Base fee + priority fee
- **EIP-4844 Transactions**: Supported (Type 3) - Blob transactions (after Cancun)

### Gas Configuration
- **Block Gas Limit**: 30,000,000 gas
- **Base Fee**: Dynamic (EIP-1559)
- **Priority Fee**: User-defined
- **Gas Price**: Legacy transactions only

## Development and Testing

This genesis configuration is designed for:
- **Development**: Pre-funded accounts for testing
- **Consensus Testing**: Mysticeti consensus integration
- **Transaction Testing**: All transaction types supported
- **Network Simulation**: Multiple accounts for realistic testing

## Security Considerations

- **Chain ID**: Unique identifier prevents replay attacks
- **Pre-funded Accounts**: Only for development/testing
- **Hardfork Activation**: All features enabled from start
- **PoS from Genesis**: No mining rewards or centralization risks

## Usage

This genesis file is used by:
- Execution clients (Reth-based)
- Consensus clients (Mysticeti-based)
- Testing infrastructure
- Development environments

The configuration ensures full Ethereum compatibility while being optimized for the FastEVM consensus mechanism and development workflow.
