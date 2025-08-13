const fs = require('fs');
const path = require('path');
require('dotenv').config();
const { calculateContractAddress } = require('./calculate-address.js');

// Read chainId from environment variable, fallback to default
const chainId = parseInt(process.env.CHAIN_ID, 10) || 202501;

const contractAddress = calculateContractAddress('FeeDistributor', chainId);
const artifactPath = path.join(__dirname, '../out/FeeDistributor.sol/FeeDistributor.json');

console.log('🔍 Extracting contract bytecode...');
console.log('');

try {
    // Read the artifact file
    const artifact = JSON.parse(fs.readFileSync(artifactPath, 'utf8'));

    if (!artifact.bytecode || !artifact.bytecode.object) {
        throw new Error('No bytecode found in artifact');
    }

    const bytecode = artifact.bytecode.object;

    if (bytecode === '0x') {
        throw new Error('Contract has no runtime bytecode (abstract contract or interface)');
    }

    console.log('✅ Bytecode extracted successfully!');
    console.log('');
    console.log('📋 Contract: FeeDistributor');
    console.log(`📍 Address: ${contractAddress}`);
    console.log(`📏 Bytecode length: ${bytecode.length - 2} bytes (${Math.ceil((bytecode.length - 2) / 2)} words)`);
    console.log('');
    console.log('📄 Bytecode:');
    console.log(bytecode);
    console.log('');
    console.log('💡 You can use this bytecode to:');
    console.log('   - Deploy the contract manually');
    console.log('   - Verify the contract on block explorers');
    console.log('   - Compare with deployed contracts');
    console.log(`   - Contract: FeeDistributor`);

} catch (error) {
    console.error('❌ Error extracting bytecode:', error.message);
    console.log('');
    console.log('💡 Make sure to:');
    console.log('   1. Run "forge build" first to compile the contracts');
    console.log('   2. Check that the artifact file exists');
    console.log('   3. Verify the contract compiles without errors');
    process.exit(1);
}
