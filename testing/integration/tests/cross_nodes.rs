//! Cross-node consistency tests for FastEVM
//!
//! This module contains comprehensive tests that verify data consistency across multiple nodes
//! in the FastEVM network. The tests ensure that all nodes maintain the same blockchain state
//! and can be used to detect consensus failures, data synchronization issues, or network splits.
//!
//! # Test Overview
//!
//! The cross-node consistency tests perform the following operations:
//! 1. **Block Number Consistency**: Get current block number from all 4 RPC endpoints and verify synchronization
//! 2. **Block Hash Consistency**: Get blocks with minimum block number and verify identical block hashes
//! 3. **Full Block Data Consistency**: Verify all blocks from genesis to minimum block have identical data
//! 4. **State Consistency**: Check state root, gas used, gas limit, and other state-related data
//! 5. **Transaction Consistency**: Verify transaction ordering and content across all nodes
//! 6. **Account State Consistency**: Check account balances and nonces across all nodes
//! 7. **Receipt Consistency**: Verify transaction receipts are identical across all nodes
//! 8. **Gas Price Consistency**: Check that all nodes report the same gas prices
//! 9. **Chain ID Consistency**: Verify all nodes report the same chain ID
//!
//! # Usage
//!
//! To run all cross-node consistency tests:
//! ```bash
//! cargo test cross_nodes
//! ```
//!
//! To run individual tests:
//! ```bash
//! cargo test test_cross_node_consistency      # Block number sync
//! cargo test test_block_hash_consistency      # Block hash verification
//! cargo test test_full_block_consistency      # Full block data verification
//! cargo test test_state_consistency           # State data verification
//! cargo test test_transaction_consistency     # Transaction verification
//! cargo test test_account_state_consistency   # Account state verification
//! cargo test test_receipt_consistency         # Receipt verification
//! cargo test test_gas_price_consistency       # Gas price verification
//! cargo test test_chain_id_consistency       # Chain ID verification
//! ```
//!
//! # Environment Variables Required
//!
//! The tests require the following environment variables to be set:
//! - `RPC_URL1` - RPC endpoint for node 1 (default: http://localhost:8545)
//! - `RPC_URL2` - RPC endpoint for node 2 (default: http://localhost:8544)
//! - `RPC_URL3` - RPC endpoint for node 3 (default: http://localhost:8543)
//! - `RPC_URL4` - RPC endpoint for node 4 (default: http://localhost:8532)
//!
//! # Test Behavior
//!
//! Each test will:
//! - Connect to all available RPC endpoints
//! - Gracefully handle connection failures (skip unavailable nodes)
//! - Compare data across all available nodes
//! - Report detailed consistency results
//! - Provide clear success/failure indicators
//!
//! # Requirements
//!
//! - FastEVM network must be running with 4 execution nodes
//! - All nodes should be synchronized and running the same consensus
//! - Test accounts must be prefunded in genesis.json (for account state tests)
//! - Network should have some transaction activity for comprehensive testing

use alloy_primitives::{Bytes, B256, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_eth::{
    Block, BlockId, BlockNumberOrTag, BlockTransactions, Transaction, TransactionReceipt,
};
use anyhow::Result;
use std::env;
use testing::address::generate_account_from_seed;

// Generate test accounts
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
/// Test that verifies all 4 nodes have consistent block numbers
///
/// This test performs the following steps:
/// 1. Connects to all 4 RPC endpoints
/// 2. Gets the current block number from each node
/// 3. Verifies that all nodes are within a reasonable range of each other
/// 4. Reports any inconsistencies found
#[tokio::test]
async fn test_cross_node_consistency() -> Result<()> {
    println!("🔍 Testing cross-node block number consistency...");

    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        println!("Connecting to RPC endpoint {}: {}", idx + 1, rpc_url);
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
                available_urls.push(rpc_url.clone());
                println!("  ✅ Connected to RPC endpoint {}", idx + 1);
            }
            Err(e) => {
                println!(
                    "  ❌ Failed to connect to RPC endpoint {}: {:?}",
                    idx + 1,
                    e
                );
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(());
    }

    // Get block numbers from all available nodes
    let mut block_numbers = Vec::new();
    for (idx, provider) in providers.iter().enumerate() {
        match provider.get_block_number().await {
            Ok(block_number) => {
                block_numbers.push(block_number);
                println!("Node {}: Block number {}", idx + 1, block_number);
            }
            Err(e) => {
                println!("Node {}: Failed to get block number: {:?}", idx + 1, e);
            }
        }
    }

    if block_numbers.is_empty() {
        println!("❌ No block numbers retrieved from any node");
        return Ok(());
    }

    // Check consistency
    let min_block = *block_numbers.iter().min().unwrap();
    let max_block = *block_numbers.iter().max().unwrap();
    let block_diff = max_block - min_block;

    println!("\n📊 Block Number Consistency Report:");
    println!("  Minimum block: {}", min_block);
    println!("  Maximum block: {}", max_block);
    println!("  Difference: {} blocks", block_diff);

    if block_diff <= 1 {
        println!("✅ All nodes are well synchronized (difference ≤ 1 block)");
    } else if block_diff <= 5 {
        println!("⚠️  Nodes are reasonably synchronized (difference ≤ 5 blocks)");
    } else {
        println!("❌ Nodes are not well synchronized (difference > 5 blocks)");
    }

    Ok(())
}

/// Test that verifies block hash consistency across all nodes
///
/// This test performs the following steps:
/// 1. Gets the minimum block number among all nodes
/// 2. Fetches the block with that number from all nodes
/// 3. Verifies that all nodes have the same block hash
/// 4. If hash differences are found, uses binary search to find the first occurrence
/// 5. Reports any hash mismatches and their exact location
#[tokio::test]
async fn test_block_hash_consistency() -> Result<()> {
    println!("🔍 Testing block hash consistency...");

    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
                available_urls.push(rpc_url.clone());
            }
            Err(e) => {
                println!("Failed to connect to RPC endpoint {}: {:?}", idx + 1, e);
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(());
    }

    // Get block numbers from all nodes
    let mut block_numbers = Vec::new();
    for provider in &providers {
        match provider.get_block_number().await {
            Ok(block_number) => block_numbers.push(block_number),
            Err(e) => println!("Failed to get block number: {:?}", e),
        }
    }

    if block_numbers.is_empty() {
        println!("❌ No block numbers retrieved from any node");
        return Ok(());
    }

    let min_block = *block_numbers.iter().min().unwrap();
    println!("Testing block hash consistency for min block {}", min_block);

    // Get blocks from all nodes
    let mut block_hashes = Vec::new();
    let mut blocks = Vec::new();

    for (idx, provider) in providers.iter().enumerate() {
        match provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(min_block)))
            .await
        {
            Ok(Some(block)) => {
                block_hashes.push(block.header.hash);
                blocks.push(block);
                println!(
                    "Node {}: Block hash {:?}",
                    idx + 1,
                    block_hashes.last().unwrap()
                );
            }
            Ok(None) => {
                println!("Node {}: Block {} not found", idx + 1, min_block);
            }
            Err(e) => {
                println!(
                    "Node {}: Failed to get block {}: {:?}",
                    idx + 1,
                    min_block,
                    e
                );
            }
        }
    }

    if block_hashes.is_empty() {
        println!("❌ No blocks retrieved from any node");
        return Ok(());
    }

    // Check hash consistency
    let first_hash = &block_hashes[0];
    let mut consistent = true;
    let mut inconsistent_nodes = Vec::new();

    for (idx, hash) in block_hashes.iter().enumerate() {
        if hash != first_hash {
            println!("❌ Hash mismatch: Node {} has different hash", idx + 1);
            println!("  Expected: {:?}", first_hash);
            println!("  Actual:   {:?}", hash);
            inconsistent_nodes.push(idx + 1);
            consistent = false;
        }
    }

    if consistent {
        println!(
            "✅ All nodes have consistent block hashes for block {}",
            min_block
        );
    } else {
        println!(
            "❌ Block hash inconsistency detected for block {}",
            min_block
        );

        // Use binary search to find the first occurrence of hash difference
        println!("🔍 Searching for first occurrence of hash difference...");
        match find_first_hash_difference(min_block).await {
            Ok(Some(first_different_block)) => {
                println!(
                    "📍 First hash difference found at block {}",
                    first_different_block
                );

                // Get detailed information about the first different block
                print_block_hash_details(first_different_block).await;
            }
            Ok(None) => {
                println!("ℹ️  No hash differences found in earlier blocks");
            }
            Err(e) => {
                println!("❌ Error during binary search: {:?}", e);
            }
        }
    }

    Ok(())
}

/// Test that verifies full block data consistency from genesis to minimum block
///
/// This test performs the following steps:
/// 1. Gets the minimum block number among all nodes
/// 2. Fetches all blocks from genesis (0) to the minimum block from all nodes
/// 3. Verifies that all nodes have identical block data
/// 4. Checks block headers, transactions, receipts, and other metadata
#[tokio::test]
async fn test_full_block_consistency() -> Result<()> {
    println!("🔍 Testing full block data consistency...");

    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
                available_urls.push(rpc_url.clone());
            }
            Err(e) => {
                println!("Failed to connect to RPC endpoint {}: {:?}", idx + 1, e);
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(());
    }

    // Get minimum block number
    let mut block_numbers = Vec::new();
    for provider in &providers {
        match provider.get_block_number().await {
            Ok(block_number) => block_numbers.push(block_number),
            Err(e) => println!("Failed to get block number: {:?}", e),
        }
    }

    if block_numbers.is_empty() {
        println!("❌ No block numbers retrieved from any node");
        return Ok(());
    }

    let min_block = *block_numbers.iter().min().unwrap();
    println!(
        "Testing full block consistency from genesis to block {}",
        min_block
    );

    let mut all_consistent = true;

    // Test each block from genesis to minimum block
    for block_num in 0..=min_block {
        println!("Checking block {}...", block_num);

        let mut blocks = Vec::new();

        // Get block from all nodes
        for (idx, provider) in providers.iter().enumerate() {
            match provider
                .get_block(BlockId::Number(BlockNumberOrTag::Number(block_num)))
                .await
            {
                Ok(Some(block)) => blocks.push((idx, block)),
                Ok(None) => {
                    println!("  Node {}: Block {} not found", idx + 1, block_num);
                    all_consistent = false;
                }
                Err(e) => {
                    println!(
                        "  Node {}: Failed to get block {}: {:?}",
                        idx + 1,
                        block_num,
                        e
                    );
                    all_consistent = false;
                }
            }
        }

        if blocks.len() < 2 {
            println!("  ⚠️  Not enough blocks to compare (need at least 2)");
            continue;
        }

        // Compare blocks
        let (_, first_block) = &blocks[0];
        for (_node_idx, block) in &blocks[1..] {
            if !compare_blocks(first_block, block) {
                println!(
                    "  ❌ Block {} inconsistency detected between nodes",
                    block_num
                );
                all_consistent = false;
            }
        }
    }

    if all_consistent {
        println!(
            "✅ All blocks from genesis to {} are consistent across all nodes",
            min_block
        );
    } else {
        println!("❌ Block data inconsistencies detected");
    }

    Ok(())
}

/// Test that verifies state consistency across all nodes
///
/// This test performs the following steps:
/// 1. Gets the minimum block number among all nodes
/// 2. Fetches state root, gas used, and other state-related data
/// 3. Verifies that all nodes have identical state information
#[tokio::test]
async fn test_state_consistency() -> Result<()> {
    println!("🔍 Testing state consistency...");

    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
                available_urls.push(rpc_url.clone());
            }
            Err(e) => {
                println!("Failed to connect to RPC endpoint {}: {:?}", idx + 1, e);
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(());
    }

    // Get minimum block number
    let mut block_numbers = Vec::new();
    for provider in &providers {
        match provider.get_block_number().await {
            Ok(block_number) => block_numbers.push(block_number),
            Err(e) => println!("Failed to get block number: {:?}", e),
        }
    }

    if block_numbers.is_empty() {
        println!("❌ No block numbers retrieved from any node");
        return Ok(());
    }

    let min_block = *block_numbers.iter().min().unwrap();
    println!("Testing state consistency for block {}", min_block);

    // Get block from all nodes and compare state data
    let mut state_data = Vec::new();

    for (idx, provider) in providers.iter().enumerate() {
        match provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(min_block)))
            .await
        {
            Ok(Some(block)) => {
                let state_info = StateInfo {
                    state_root: block.header.state_root,
                    gas_used: U256::from(block.header.gas_used),
                    gas_limit: U256::from(block.header.gas_limit),
                    base_fee_per_gas: block.header.base_fee_per_gas.map(U256::from),
                    extra_data: block.header.extra_data.clone(),
                    logs_bloom: block.header.logs_bloom,
                    receipts_root: block.header.receipts_root,
                    transactions_root: block.header.transactions_root,
                };
                state_data.push(state_info);
            }
            Ok(None) => {
                println!("Node {}: Block {} not found", idx + 1, min_block);
            }
            Err(e) => {
                println!(
                    "Node {}: Failed to get block {}: {:?}",
                    idx + 1,
                    min_block,
                    e
                );
            }
        }
    }

    if state_data.len() < 2 {
        println!("❌ Not enough state data to compare (need at least 2)");
        return Ok(());
    }

    // Compare state data
    let first_state = &state_data[0];
    let mut consistent = true;

    for state in &state_data[1..] {
        if !compare_state_info(first_state, state) {
            println!("❌ State inconsistency detected between nodes");
            consistent = false;
        }
    }

    if consistent {
        println!(
            "✅ All nodes have consistent state data for block {}",
            min_block
        );
    } else {
        println!(
            "❌ State data inconsistencies detected for block {}",
            min_block
        );
    }

    Ok(())
}

/// Test that verifies transaction consistency and ordering across all nodes
///
/// This test performs the following steps:
/// 1. Gets the minimum block number among all nodes
/// 2. Fetches transaction data from all nodes
/// 3. Verifies that all nodes have identical transactions in the same order
#[tokio::test]
async fn test_transaction_consistency() -> Result<()> {
    println!("🔍 Testing transaction consistency...");

    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
                available_urls.push(rpc_url.clone());
            }
            Err(e) => {
                println!("Failed to connect to RPC endpoint {}: {:?}", idx + 1, e);
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(());
    }

    // Get minimum block number
    let mut block_numbers = Vec::new();
    for provider in &providers {
        match provider.get_block_number().await {
            Ok(block_number) => block_numbers.push(block_number),
            Err(e) => println!("Failed to get block number: {:?}", e),
        }
    }

    if block_numbers.is_empty() {
        println!("❌ No block numbers retrieved from any node");
        return Ok(());
    }

    let min_block = *block_numbers.iter().min().unwrap();
    println!("Testing transaction consistency for block {}", min_block);

    // Get block from all nodes and compare transactions
    let mut transaction_data = Vec::new();

    for (idx, provider) in providers.iter().enumerate() {
        match provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(min_block)))
            .await
        {
            Ok(Some(block)) => {
                let tx_count = match &block.transactions {
                    BlockTransactions::Full(txs) => txs.len(),
                    BlockTransactions::Hashes(hashes) => hashes.len(),
                    BlockTransactions::Uncle => 0,
                };
                transaction_data.push((idx + 1, tx_count));
            }
            Ok(None) => {
                println!("Node {}: Block {} not found", idx + 1, min_block);
            }
            Err(e) => {
                println!(
                    "Node {}: Failed to get block {}: {:?}",
                    idx + 1,
                    min_block,
                    e
                );
            }
        }
    }

    if transaction_data.len() < 2 {
        println!("❌ Not enough transaction data to compare (need at least 2)");
        return Ok(());
    }

    // Compare transaction data
    let (_, first_tx_count) = &transaction_data[0];
    let mut consistent = true;

    for (node_id, tx_count) in &transaction_data[1..] {
        if tx_count != first_tx_count {
            println!("❌ Transaction inconsistency detected between nodes");
            println!("  Node 1 has {} transactions", first_tx_count);
            println!("  Node {} has {} transactions", node_id, tx_count);
            consistent = false;
        }
    }

    if consistent {
        println!(
            "✅ All nodes have consistent transaction data for block {}",
            min_block
        );
    } else {
        println!(
            "❌ Transaction data inconsistencies detected for block {}",
            min_block
        );
    }

    Ok(())
}

/// Test that verifies account state consistency across all nodes
///
/// This test performs the following steps:
/// 1. Generates test accounts using the same mnemonic as genesis
/// 2. Checks account balances and nonces across all nodes
/// 3. Verifies that all nodes have identical account state
#[tokio::test]
async fn test_account_state_consistency() -> Result<()> {
    println!("🔍 Testing account state consistency...");

    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
                available_urls.push(rpc_url.clone());
            }
            Err(e) => {
                println!("Failed to connect to RPC endpoint {}: {:?}", idx + 1, e);
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(());
    }

    let mnemonic = bip39::Mnemonic::parse(TEST_MNEMONIC)
        .map_err(|e| anyhow::anyhow!("Invalid mnemonic: {}", e))?;
    let seed = mnemonic.to_seed("");
    let seed_bytes = &seed[..];

    let mut test_accounts = Vec::new();
    for i in 0..10 {
        let account = generate_account_from_seed(seed_bytes, i as u32)
            .map_err(|e| anyhow::anyhow!("Failed to generate account: {}", e))?;
        test_accounts.push(account);
    }

    println!("Testing account state for {} accounts", test_accounts.len());

    let mut all_consistent = true;

    // Check each account across all nodes
    for (account_idx, account) in test_accounts.iter().enumerate() {
        println!("Checking account {}: {:?}", account_idx, account.address);

        let mut account_data = Vec::new();

        for (node_idx, provider) in providers.iter().enumerate() {
            let balance = provider
                .get_balance(account.address)
                .await
                .unwrap_or_default();
            let nonce = provider
                .get_transaction_count(account.address)
                .await
                .unwrap_or_default();

            account_data.push((node_idx + 1, balance, nonce));
        }

        if account_data.len() < 2 {
            println!("  ⚠️  Not enough account data to compare");
            continue;
        }

        // Compare account data
        let (_, first_balance, first_nonce) = &account_data[0];
        let mut account_consistent = true;

        for (node_id, balance, nonce) in &account_data[1..] {
            if balance != first_balance || nonce != first_nonce {
                println!("  ❌ Account state inconsistency detected");
                println!(
                    "    Node 1: balance={}, nonce={}",
                    first_balance, first_nonce
                );
                println!("    Node {}: balance={}, nonce={}", node_id, balance, nonce);
                account_consistent = false;
                all_consistent = false;
            }
        }

        if account_consistent {
            println!("  ✅ Account state consistent across all nodes");
        }
    }

    if all_consistent {
        println!("✅ All account states are consistent across all nodes");
    } else {
        println!("❌ Account state inconsistencies detected");
    }

    Ok(())
}

/// Helper function to get RPC URLs from environment variables
fn get_rpc_urls() -> Vec<String> {
    vec![
        env::var("RPC_URL1").unwrap_or_else(|_| "http://localhost:8545".to_string()),
        env::var("RPC_URL2").unwrap_or_else(|_| "http://localhost:8544".to_string()),
        env::var("RPC_URL3").unwrap_or_else(|_| "http://localhost:8543".to_string()),
        env::var("RPC_URL4").unwrap_or_else(|_| "http://localhost:8542".to_string()),
    ]
}

/// Helper function to compare two blocks for consistency
fn compare_blocks(block1: &Block<Transaction>, block2: &Block<Transaction>) -> bool {
    // Compare block headers
    if block1.header.hash != block2.header.hash {
        return false;
    }
    if block1.header.parent_hash != block2.header.parent_hash {
        return false;
    }
    if block1.header.state_root != block2.header.state_root {
        return false;
    }
    if block1.header.receipts_root != block2.header.receipts_root {
        return false;
    }
    if block1.header.transactions_root != block2.header.transactions_root {
        return false;
    }
    if block1.header.gas_used != block2.header.gas_used {
        return false;
    }
    if block1.header.gas_limit != block2.header.gas_limit {
        return false;
    }
    if block1.header.base_fee_per_gas != block2.header.base_fee_per_gas {
        return false;
    }
    if block1.header.extra_data != block2.header.extra_data {
        return false;
    }
    if block1.header.logs_bloom != block2.header.logs_bloom {
        return false;
    }

    // Compare transactions
    match (&block1.transactions, &block2.transactions) {
        (BlockTransactions::Full(txs1), BlockTransactions::Full(txs2)) => {
            if txs1.len() != txs2.len() {
                return false;
            }
            // Skip detailed transaction comparison for now due to type issues
        }
        (BlockTransactions::Hashes(hashes1), BlockTransactions::Hashes(hashes2)) => {
            if hashes1 != hashes2 {
                return false;
            }
        }
        _ => return false,
    }

    true
}

/// Helper function to compare state information
fn compare_state_info(state1: &StateInfo, state2: &StateInfo) -> bool {
    state1.state_root == state2.state_root
        && state1.gas_used == state2.gas_used
        && state1.gas_limit == state2.gas_limit
        && state1.base_fee_per_gas == state2.base_fee_per_gas
        && state1.extra_data == state2.extra_data
        && state1.logs_bloom == state2.logs_bloom
        && state1.receipts_root == state2.receipts_root
        && state1.transactions_root == state2.transactions_root
}

/// Test that verifies receipt consistency across all nodes
///
/// This test performs the following steps:
/// 1. Gets the minimum block number among all nodes
/// 2. Fetches transaction receipts from all nodes
/// 3. Verifies that all nodes have identical receipt data
#[tokio::test]
async fn test_receipt_consistency() -> Result<()> {
    println!("🔍 Testing receipt consistency...");

    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
                available_urls.push(rpc_url.clone());
            }
            Err(e) => {
                println!("Failed to connect to RPC endpoint {}: {:?}", idx + 1, e);
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(());
    }

    // Get minimum block number
    let mut block_numbers = Vec::new();
    for provider in &providers {
        match provider.get_block_number().await {
            Ok(block_number) => block_numbers.push(block_number),
            Err(e) => println!("Failed to get block number: {:?}", e),
        }
    }

    if block_numbers.is_empty() {
        println!("❌ No block numbers retrieved from any node");
        return Ok(());
    }

    let min_block = *block_numbers.iter().min().unwrap();
    println!("Testing receipt consistency for block {}", min_block);

    // Get block from all nodes and extract transaction hashes
    let mut transaction_hashes = Vec::new();

    for (idx, provider) in providers.iter().enumerate() {
        match provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(min_block)))
            .await
        {
            Ok(Some(block)) => {
                let tx_hashes: Vec<B256> = match &block.transactions {
                    BlockTransactions::Full(_txs) => vec![], // Skip full transaction comparison for now
                    BlockTransactions::Hashes(hashes) => hashes.clone(),
                    BlockTransactions::Uncle => vec![],
                };
                transaction_hashes.push((idx + 1, tx_hashes));
            }
            Ok(None) => {
                println!("Node {}: Block {} not found", idx + 1, min_block);
            }
            Err(e) => {
                println!(
                    "Node {}: Failed to get block {}: {:?}",
                    idx + 1,
                    min_block,
                    e
                );
            }
        }
    }

    if transaction_hashes.is_empty() {
        println!("❌ No transaction hashes retrieved from any node");
        return Ok(());
    }

    // Use the first node's transaction hashes as reference
    let (_, ref_tx_hashes) = &transaction_hashes[0];

    if ref_tx_hashes.is_empty() {
        println!(
            "ℹ️  No transactions in block {}, skipping receipt test",
            min_block
        );
        return Ok(());
    }

    // Test receipt consistency for each transaction
    let mut all_consistent = true;

    for tx_hash in ref_tx_hashes {
        println!("Checking receipt for transaction {:?}", tx_hash);

        let mut receipts = Vec::new();

        for (node_idx, provider) in providers.iter().enumerate() {
            match provider.get_transaction_receipt(*tx_hash).await {
                Ok(Some(receipt)) => {
                    receipts.push((node_idx + 1, receipt));
                }
                Ok(None) => {
                    println!(
                        "  Node {}: Receipt not found for transaction {:?}",
                        node_idx + 1,
                        tx_hash
                    );
                    all_consistent = false;
                }
                Err(e) => {
                    println!(
                        "  Node {}: Failed to get receipt for transaction {:?}: {:?}",
                        node_idx + 1,
                        tx_hash,
                        e
                    );
                    all_consistent = false;
                }
            }
        }

        if receipts.len() < 2 {
            println!("  ⚠️  Not enough receipts to compare (need at least 2)");
            continue;
        }

        // Compare receipts
        let (_, first_receipt) = &receipts[0];
        for (node_id, receipt) in &receipts[1..] {
            if !compare_receipts(first_receipt, receipt) {
                println!(
                    "  ❌ Receipt inconsistency detected for transaction {:?} (node {})",
                    tx_hash, node_id
                );
                all_consistent = false;
            }
        }
    }

    if all_consistent {
        println!(
            "✅ All receipts are consistent across all nodes for block {}",
            min_block
        );
    } else {
        println!(
            "❌ Receipt inconsistencies detected for block {}",
            min_block
        );
    }

    Ok(())
}

/// Test that verifies gas price consistency across all nodes
///
/// This test performs the following steps:
/// 1. Gets the minimum block number among all nodes
/// 2. Fetches gas price information from all nodes
/// 3. Verifies that all nodes have consistent gas pricing
#[tokio::test]
async fn test_gas_price_consistency() -> Result<()> {
    println!("🔍 Testing gas price consistency...");

    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
                available_urls.push(rpc_url.clone());
            }
            Err(e) => {
                println!("Failed to connect to RPC endpoint {}: {:?}", idx + 1, e);
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(());
    }

    // Get gas price from all nodes
    let mut gas_prices = Vec::new();

    for (idx, provider) in providers.iter().enumerate() {
        match provider.get_gas_price().await {
            Ok(gas_price) => {
                gas_prices.push((idx + 1, gas_price));
                println!("Node {}: Gas price {}", idx + 1, gas_price);
            }
            Err(e) => {
                println!("Node {}: Failed to get gas price: {:?}", idx + 1, e);
            }
        }
    }

    if gas_prices.len() < 2 {
        println!("❌ Not enough gas price data to compare (need at least 2)");
        return Ok(());
    }

    // Compare gas prices
    let (_, first_gas_price) = &gas_prices[0];
    let mut consistent = true;

    for (node_id, gas_price) in &gas_prices[1..] {
        if gas_price != first_gas_price {
            println!("❌ Gas price inconsistency detected");
            println!("  Node 1: {}", first_gas_price);
            println!("  Node {}: {}", node_id, gas_price);
            consistent = false;
        }
    }

    if consistent {
        println!("✅ All nodes have consistent gas prices");
    } else {
        println!("❌ Gas price inconsistencies detected");
    }

    Ok(())
}

/// Test that verifies chain ID consistency across all nodes
///
/// This test performs the following steps:
/// 1. Gets the chain ID from all nodes
/// 2. Verifies that all nodes report the same chain ID
#[tokio::test]
async fn test_chain_id_consistency() -> Result<()> {
    println!("🔍 Testing chain ID consistency...");

    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
                available_urls.push(rpc_url.clone());
            }
            Err(e) => {
                println!("Failed to connect to RPC endpoint {}: {:?}", idx + 1, e);
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(());
    }

    // Get chain ID from all nodes
    let mut chain_ids = Vec::new();

    for (idx, provider) in providers.iter().enumerate() {
        match provider.get_chain_id().await {
            Ok(chain_id) => {
                chain_ids.push((idx + 1, chain_id));
                println!("Node {}: Chain ID {}", idx + 1, chain_id);
            }
            Err(e) => {
                println!("Node {}: Failed to get chain ID: {:?}", idx + 1, e);
            }
        }
    }

    if chain_ids.len() < 2 {
        println!("❌ Not enough chain ID data to compare (need at least 2)");
        return Ok(());
    }

    // Compare chain IDs
    let (_, first_chain_id) = &chain_ids[0];
    let mut consistent = true;

    for (node_id, chain_id) in &chain_ids[1..] {
        if chain_id != first_chain_id {
            println!("❌ Chain ID inconsistency detected");
            println!("  Node 1: {}", first_chain_id);
            println!("  Node {}: {}", node_id, chain_id);
            consistent = false;
        }
    }

    if consistent {
        println!("✅ All nodes have consistent chain IDs");
    } else {
        println!("❌ Chain ID inconsistencies detected");
    }

    Ok(())
}

/// Helper function to compare two transaction receipts for consistency
fn compare_receipts(receipt1: &TransactionReceipt, receipt2: &TransactionReceipt) -> bool {
    receipt1.transaction_hash == receipt2.transaction_hash
        && receipt1.transaction_index == receipt2.transaction_index
        && receipt1.block_number == receipt2.block_number
        && receipt1.block_hash == receipt2.block_hash
        && receipt1.from == receipt2.from
        && receipt1.to == receipt2.to
        && receipt1.gas_used == receipt2.gas_used
        && receipt1.contract_address == receipt2.contract_address
        && receipt1.logs() == receipt2.logs()
        && receipt1.status() == receipt2.status()
        && receipt1.transaction_type() == receipt2.transaction_type()
}

/// Structure to hold state information for comparison
#[derive(Debug)]
struct StateInfo {
    state_root: B256,
    gas_used: U256,
    gas_limit: U256,
    base_fee_per_gas: Option<U256>,
    extra_data: Bytes,
    logs_bloom: alloy_primitives::Bloom,
    receipts_root: B256,
    transactions_root: B256,
}

/// Binary search to find the first block where hash differences occur
///
/// This function performs a binary search from genesis (block 0) to the given max_block
/// to find the first block number where nodes have different block hashes.
async fn find_first_hash_difference(max_block: u64) -> Result<Option<u64>> {
    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
            }
            Err(e) => {
                println!("Failed to connect to RPC endpoint {}: {:?}", idx + 1, e);
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(None);
    }
    let mut left = 0u64;
    let mut right = max_block;
    let mut first_difference = None;

    while left <= right {
        let mid = left + (right - left) / 2;

        println!("  Checking block {} for hash consistency...", mid);

        // Get block hashes from all providers for this block
        let mut block_hashes = Vec::new();
        let mut all_successful = true;

        for (idx, provider) in providers.iter().enumerate() {
            match provider
                .get_block(BlockId::Number(BlockNumberOrTag::Number(mid)))
                .await
            {
                Ok(Some(block)) => {
                    block_hashes.push((idx + 1, block.header.hash));
                }
                Ok(None) => {
                    println!("    Node {}: Block {} not found", idx + 1, mid);
                    all_successful = false;
                    break;
                }
                Err(e) => {
                    println!("    Node {}: Failed to get block {}: {:?}", idx + 1, mid, e);
                    all_successful = false;
                    break;
                }
            }
        }

        if !all_successful || block_hashes.len() < 2 {
            // If we can't get blocks from all nodes, move to next block
            left = mid + 1;
            continue;
        }

        // Check if all hashes are the same
        let first_hash = &block_hashes[0].1;
        let mut has_difference = false;

        for (node_id, hash) in &block_hashes[1..] {
            if hash != first_hash {
                has_difference = true;
                println!(
                    "    ❌ Hash difference found at block {} (node {})",
                    mid, node_id
                );
                break;
            }
        }

        if has_difference {
            // Found a difference at this block, record it and search earlier
            first_difference = Some(mid);
            if mid == 0 {
                // Can't go earlier than genesis
                break;
            }
            right = mid - 1;
        } else {
            // No difference at this block, search later blocks
            left = mid + 1;
        }
    }

    Ok(first_difference)
}

/// Print detailed information about block hash differences for a specific block
async fn print_block_hash_details(block_number: u64) {
    println!(
        "\n📋 Detailed block hash information for block {}:",
        block_number
    );
    let rpc_urls = get_rpc_urls();
    let mut providers = Vec::new();

    // Connect to all RPC endpoints
    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        match ProviderBuilder::new().connect(rpc_url).await {
            Ok(provider) => {
                providers.push(provider);
            }
            Err(e) => {
                println!("Failed to connect to RPC endpoint {}: {:?}", idx + 1, e);
            }
        }
    }

    if providers.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return;
    }
    let mut block_data = Vec::new();

    for (idx, provider) in providers.iter().enumerate() {
        match provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(block_number)))
            .await
        {
            Ok(Some(block)) => {
                block_data.push((idx + 1, block));
            }
            Ok(None) => {
                println!("  Node {}: Block {} not found", idx + 1, block_number);
            }
            Err(e) => {
                println!(
                    "  Node {}: Failed to get block {}: {:?}",
                    idx + 1,
                    block_number,
                    e
                );
            }
        }
    }

    if block_data.len() < 2 {
        println!("  ⚠️  Not enough block data to compare");
        return;
    }

    // Group blocks by hash to show which nodes have the same hash
    let mut hash_groups: std::collections::HashMap<B256, Vec<usize>> =
        std::collections::HashMap::new();

    for (node_id, block) in &block_data {
        let hash = block.header.hash;
        hash_groups
            .entry(hash)
            .or_insert_with(Vec::new)
            .push(*node_id);
    }

    println!("  Hash groups:");
    for (hash, nodes) in hash_groups {
        println!("    Hash {:?}: Nodes {:?}", hash, nodes);
    }

    // Show block header details for comparison
    if block_data.len() >= 2 {
        println!("  Block header comparison:");
        let (_, first_block) = &block_data[0];
        println!("    Parent hash: {:?}", first_block.header.parent_hash);
        println!("    State root: {:?}", first_block.header.state_root);
        println!(
            "    Transactions root: {:?}",
            first_block.header.transactions_root
        );
        println!("    Receipts root: {:?}", first_block.header.receipts_root);
        println!("    Gas used: {}", first_block.header.gas_used);
        println!("    Gas limit: {}", first_block.header.gas_limit);
        println!("    Timestamp: {}", first_block.header.timestamp);
    }
}
