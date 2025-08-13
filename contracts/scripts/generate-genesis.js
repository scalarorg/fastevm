const fs = require('fs');
const path = require('path');
require('dotenv').config();
const { calculateContractAddress } = require('./calculate-address.js');

const chainId = parseInt(process.env.CHAIN_ID, 10) || 202501;
const contractAddress = calculateContractAddress('FeeDistributor', chainId);
const artifactPath = path.join(__dirname, '../out/FeeDistributor.sol/FeeDistributor.json');

console.log('🚀 Generating genesis.json with contract bytecode...');
console.log('');

try {
    // Read the contract artifact
    if (!fs.existsSync(artifactPath)) {
        throw new Error(`Contract artifact not found at ${artifactPath}. Please run "forge build" first.`);
    }

    const artifact = JSON.parse(fs.readFileSync(artifactPath, 'utf8'));
    const bytecode = artifact.bytecode.object;

    if (!bytecode || bytecode === '0x') {
        throw new Error('No runtime bytecode found in artifact. Please check the contract compilation.');
    }

    // Read the genesis template
    const templatePath = path.join(__dirname, '../../execution-client/shared/genesis.template.json');
    if (!fs.existsSync(templatePath)) {
        throw new Error(`Genesis template not found at ${templatePath}`);
    }

    const genesisTemplate = JSON.parse(fs.readFileSync(templatePath, 'utf8'));

    // Add the contract to the genesis state
    genesisTemplate.alloc[contractAddress] = {
        code: bytecode,
        balance: '0x0',
        nonce: '0x0'
    };

    // Write the generated genesis.json
    const outputPath = path.join(__dirname, '../../execution-client/shared/genesis.json');
    fs.writeFileSync(outputPath, JSON.stringify(genesisTemplate, null, 2));

    console.log('✅ Genesis file generated successfully!');
    console.log('');
    console.log('📋 Contract Details:');
    console.log(`   - Contract name: FeeDistributor`);
    console.log(`   - Address: ${contractAddress}`);
    console.log(`   - Bytecode length: ${bytecode.length - 2} bytes`);
    console.log(`   - Output file: ${outputPath}`);
    console.log('');
    console.log('🚀 Your FastEVM node will now start with the contract already deployed!');
    console.log('');
    console.log('💡 Next steps:');
    console.log('   1. Start your FastEVM node');
    console.log('   2. The contract will be available at the calculated address');
    console.log('   3. Use the contract ABI from the artifact file for interactions');

} catch (error) {
    console.error('❌ Error generating genesis.json:', error.message);
    console.log('');
    console.log('💡 Make sure to:');
    console.log('   1. Run "forge build" first to compile the contracts');
    console.log('   2. Check that the genesis template exists');
    console.log('   3. Verify the contract compiles without errors');
    process.exit(1);
}
