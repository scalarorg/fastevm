const crypto = require('crypto');
require('dotenv').config();
/**
 * Calculate deterministic contract address based on:
 * - Contract name
 * - Chain ID
 * - Project identifier
 * 
 * This ensures the same contract gets the same address
 * across all deployments on the same chain.
 */
function calculateContractAddress(contractName, chainId) {
    const projectId = 'fastevm';

    // Create a deterministic seed from contract details
    const seed = `${contractName}-${chainId}-${projectId}`;

    // Generate a hash from the seed
    const hash = crypto.createHash('sha256').update(seed).digest('hex');

    // Take the first 20 bytes (40 hex characters) for the address
    const addressHex = hash.substring(0, 40);

    // Format as Ethereum address
    return `0x${addressHex}`;
}

// Example usage
const contractName = 'FeeDistributor';
const chainId = 202501;

const address = calculateContractAddress(contractName, chainId);

console.log('🔍 Contract Address Calculator');
console.log('==============================');
console.log('');
console.log(`📋 Contract: ${contractName}`);
console.log(`🔗 Chain ID: ${chainId}`);
console.log(`🏷️  Project: fastevm`);
console.log('');
console.log(`📍 Calculated Address: ${address}`);
console.log('');
console.log('💡 This address will be the same every time for this contract on this chain.');
console.log('   Use it in your genesis.json and deployment scripts.');
console.log('');
console.log('🔧 To calculate a different address:');
console.log('   - Change the contract name');
console.log('   - Change the chain ID');
console.log('   - Change the project identifier');

module.exports = { calculateContractAddress };
