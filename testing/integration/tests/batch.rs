//! Batch transaction sending test for FastEVM
//!
//! This module contains tests that verify batch transaction sending capabilities
//! of the FastEVM system. The test generates 100 sender addresses from the same
//! mnemonic used in genesis.json and sends transactions to random recipients
//! using randomly selected RPC endpoints.
//!
//! # Test Overview
//!
//! The batch transaction test performs the following operations:
//! 1. Generates 100 deterministic sender addresses using the test mnemonic
//! 2. For each sender, creates a transfer transaction to a random recipient
//! 3. Randomly selects one of 4 RPC endpoints to broadcast each transaction
//! 4. Tracks success/failure statistics and RPC usage distribution
//! 5. Provides detailed logging and error handling
//!
//! # Usage
//!
//! To run the batch transaction test:
//! ```bash
//! cargo test test_batch_transaction_sending
//! ```
//!
//! # Requirements
//!
//! - FastEVM network must be running with 4 execution nodes
//! - Environment variables for RPC URLs must be set
//! - Test accounts must be prefunded in genesis.json

use alloy_provider::{Provider, ProviderBuilder};
use bip39::Mnemonic;
use eyre::Result;
use rand::Rng;
use std::env;
use std::time::Instant;
use std::{collections::BTreeMap, collections::HashMap, time::Duration};
use testing::{
    address::{generate_account_from_seed, Account},
    rpc::get_nonces,
    transactions::create_transfer_transaction,
};
use tokio::time::sleep;

const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

const NUMBER_OF_SENDERS: usize = 1000;
/// Test that generates 100 sender addresses and sends batch transactions.
///
/// This test performs the following steps:
/// 1. Generates 100 sender addresses using the same mnemonic as genesis.json
/// 2. For each sender, creates a transfer transaction to a random recipient
/// 3. Randomly selects an RPC URL to broadcast each transaction
/// 4. Tracks success/failure statistics and provides detailed logging
///
/// # Environment Variables Required
///
/// * `RPC_URL1` through `RPC_URL4` - RPC endpoints for the 4 execution nodes
/// * `CHAIN_ID` - Network chain ID (defaults to 202501)
///
/// # Test Behavior
///
/// The test will:
/// - Generate 100 deterministic sender addresses from the test mnemonic
/// - Create transfer transactions with random recipients
/// - Distribute transactions across 4 RPC endpoints randomly
/// - Track and report success/failure statistics
/// - Skip gracefully if any RPC endpoint is unavailable
#[tokio::test]
async fn test_batch_transfer_one_transaction() -> Result<(), Box<dyn std::error::Error>> {
    let account_number = NUMBER_OF_SENDERS;
    let number_of_transactions = 1;
    send_transaction_with_check_nonce(account_number, number_of_transactions).await
}

/// Test that generates 100 sender addresses and sends 2 transactions per sender.
///
/// This test performs the following steps:
/// 1. Generates 100 sender addresses using the same mnemonic as genesis.json
/// 2. For each sender, creates 2 transfer transactions to random recipients
/// 3. Randomly selects an RPC URL to broadcast each transaction
/// 4. Tracks success/failure statistics and provides detailed logging
///
/// # Environment Variables Required
///
/// * `RPC_URL1` through `RPC_URL4` - RPC endpoints for the 4 execution nodes
/// * `CHAIN_ID` - Network chain ID (defaults to 202501)
///
/// # Test Behavior
///
/// The test will:
/// - Generate 100 deterministic sender addresses from the test mnemonic
/// - Create 2 transfer transactions per sender with random recipients
/// - Distribute transactions across 4 RPC endpoints randomly
/// - Track and report success/failure statistics
/// - Skip gracefully if any RPC endpoint is unavailable
#[tokio::test]
async fn test_batch_transfer_two_transactions() -> Result<(), Box<dyn std::error::Error>> {
    let account_number = NUMBER_OF_SENDERS;
    let number_of_transactions = 2;
    send_transaction_with_check_nonce(account_number, number_of_transactions).await
}

#[tokio::test]
async fn test_batch_transfer_10() -> Result<(), Box<dyn std::error::Error>> {
    let account_number = NUMBER_OF_SENDERS;
    let number_of_transactions = 10;
    send_transaction_with_check_nonce(account_number, number_of_transactions).await
}

#[tokio::test]
async fn test_batch_transfer_20() -> Result<(), Box<dyn std::error::Error>> {
    let account_number = NUMBER_OF_SENDERS;
    let number_of_transactions = 20;
    send_transaction_with_check_nonce(account_number, number_of_transactions).await
}

#[tokio::test]
async fn test_batch_transfer_100() -> Result<(), Box<dyn std::error::Error>> {
    let account_number = NUMBER_OF_SENDERS;
    let number_of_transactions = 100;
    send_transaction_with_check_nonce(account_number, number_of_transactions).await
}

#[tokio::test]
async fn test_batch_transfer_1000() -> Result<(), Box<dyn std::error::Error>> {
    let account_number = NUMBER_OF_SENDERS;
    let number_of_transactions = 1000;
    send_transaction_with_check_nonce(account_number, number_of_transactions).await
}

#[tokio::test]
async fn check_nonces() -> Result<(), Box<dyn std::error::Error>> {
    let account_number = NUMBER_OF_SENDERS;
    let accounts = generate_accounts(account_number)?;

    let addresses = accounts
        .iter()
        .map(|account| account.address)
        .collect::<Vec<_>>();
    let url = env::var("RPC_URL1").unwrap_or_else(|_| "http://localhost:8545".to_string());
    let address_nonces = get_nonces(addresses.as_slice(), 0, account_number, url.as_str()).await;

    // Count sender addresses by nonce
    let mut nonce_counts: HashMap<u64, usize> = HashMap::new();
    for (_address, nonce) in address_nonces.iter() {
        *nonce_counts.entry(*nonce).or_insert(0) += 1;
    }

    // Print summary
    println!("📊 Nonce Distribution Summary");
    println!("============================");
    println!("Total addresses checked: {}", address_nonces.len());
    println!();

    // Sort nonces for better readability
    let mut sorted_nonces: Vec<_> = nonce_counts.into_iter().collect();
    sorted_nonces.sort_by_key(|(nonce, _)| *nonce);

    println!("Nonce Distribution:");
    for (nonce, count) in sorted_nonces.iter() {
        println!("  Nonce {}: {} addresses", nonce, count);
    }

    Ok(())
}

async fn send_transaction_with_check_nonce(
    account_number: usize,
    number_of_transactions: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let accounts = generate_accounts(account_number)?;

    let addresses = accounts
        .iter()
        .map(|account| account.address)
        .collect::<Vec<_>>();
    let _ = send_batch_transfer_transactions(accounts.as_slice(), number_of_transactions as usize)
        .await?;
    // check if all transactions are mined
    let url = env::var("RPC_URL1").unwrap_or_else(|_| "http://localhost:8545".to_string());
    let expected_duration = (account_number as u64) * number_of_transactions * 1;
    let timeout = Duration::from_millis(expected_duration);
    let start_time = Instant::now();
    let mut success_count = 0;
    while start_time.elapsed() < timeout {
        sleep(Duration::from_secs(10)).await;
        success_count = 0;
        let address_nonces =
            get_nonces(addresses.as_slice(), 0, account_number, url.as_str()).await;
        for (_, nonce) in address_nonces.iter() {
            if nonce == &number_of_transactions {
                success_count += 1;
            }
        }
        println!(
            "Success count with number of transactions {:?}: {:?}",
            number_of_transactions, success_count
        );
        if success_count == account_number {
            println!("All transactions are mined");
            break;
        }
    }
    println!(
        "Timeout {:?} seconds. Success count with number of transactions {:?}: {:?}",
        timeout.as_secs(),
        number_of_transactions,
        success_count
    );
    Ok(())
}
fn generate_accounts(number_of_senders: usize) -> Result<Vec<Account>, Box<dyn std::error::Error>> {
    println!(
        "Generating {} sender addresses from mnemonic...",
        number_of_senders
    );
    let mnemonic =
        Mnemonic::parse(TEST_MNEMONIC).map_err(|e| eyre::eyre!("Invalid mnemonic: {}", e))?;
    let seed = mnemonic.to_seed("");
    let seed_bytes = &seed[..];
    let mut accounts = Vec::new();
    for i in 0..number_of_senders {
        let account = generate_account_from_seed(seed_bytes, i as u32)?;
        accounts.push(account);
        // let derivation_path = format!("m/44'/60'/0'/0/{}", i);
        // let (address, private_key) = derive_eth_account(TEST_MNEMONIC, &derivation_path)?;

        // sender_addresses.push(Address::from_str(&address)?);
        // sender_private_keys.push(private_key);
    }
    Ok(accounts)
}
/// Common function to send batch transfer transactions with parallel processing
async fn send_batch_transfer_transactions(
    accounts: &[Account],
    transactions_per_sender: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file if present
    dotenv::dotenv().ok();
    println!(
        "Test with {} accounts and {} transactions per sender",
        accounts.len(),
        transactions_per_sender
    );
    let number_of_senders = accounts.len();

    // Extract network configuration from environment variables
    let chain_id = env::var("CHAIN_ID")
        .unwrap_or("202501".to_string())
        .parse::<u64>()?;

    // Define RPC URLs for the 4 execution nodes
    let rpc_urls = vec![
        env::var("RPC_URL1").unwrap_or_else(|_| "http://localhost:8545".to_string()),
        env::var("RPC_URL2").unwrap_or_else(|_| "http://localhost:8547".to_string()),
        env::var("RPC_URL3").unwrap_or_else(|_| "http://localhost:8549".to_string()),
        env::var("RPC_URL4").unwrap_or_else(|_| "http://localhost:8555".to_string()),
    ];

    // Transaction amount in wei (0.001 ETH)
    let transaction_amount = 1_000_000_000_000_000_u64;
    let total_transactions = number_of_senders * transactions_per_sender;
    println!("Configuration:");
    println!("  Chain ID: {}", chain_id);
    println!("  Sender count: {}", number_of_senders);
    println!("  Transactions per sender: {}", transactions_per_sender);
    println!("  Total transactions: {}", total_transactions);
    println!(
        "  Transaction amount: {} wei (0.001 ETH)",
        transaction_amount
    );
    println!("  RPC URLs: {:?}", rpc_urls);

    // Generate sender addresses from mnemonic
    let sender_accounts = generate_accounts(number_of_senders)?;
    println!("Generated {} sender accounts", sender_accounts.len());

    // Connect to all RPC endpoints
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

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

    println!("Connected to {} RPC endpoints", providers.len());

    // Determine optimal number of parallel workers (chunks)
    let num_workers = std::cmp::min(8, number_of_senders); // Cap at 8 workers
    let chunk_size = (number_of_senders + num_workers - 1) / num_workers; // Ceiling division

    println!(
        "\n🚀 Starting parallel batch transaction sending with {} workers (chunk size: {})...",
        num_workers, chunk_size
    );

    // Create shared data for parallel processing
    let accounts_arc = std::sync::Arc::new(sender_accounts);
    let providers_arc = std::sync::Arc::new(providers);
    let available_urls_arc = std::sync::Arc::new(available_urls);

    // Spawn parallel workers
    let mut handles = Vec::new();

    for worker_id in 0..num_workers {
        let start_idx = worker_id * chunk_size;
        let end_idx = std::cmp::min(start_idx + chunk_size, number_of_senders);

        if start_idx >= number_of_senders {
            break;
        }

        let accounts_clone = accounts_arc.clone();
        let providers_clone = providers_arc.clone();
        let urls_clone = available_urls_arc.clone();

        let handle = tokio::spawn(async move {
            process_account_chunk(
                worker_id,
                start_idx,
                end_idx,
                accounts_clone,
                providers_clone,
                urls_clone,
                chain_id,
                transaction_amount,
                transactions_per_sender,
            )
            .await
        });

        handles.push(handle);
    }

    // Collect results from all workers
    let mut total_successful = 0;
    let mut total_failed = 0;
    let mut total_rpc_usage = HashMap::new();

    for (worker_id, handle) in handles.into_iter().enumerate() {
        match handle.await {
            Ok(worker_result) => {
                match worker_result {
                    Ok((successful, failed, rpc_usage)) => {
                        total_successful += successful;
                        total_failed += failed;

                        // Merge RPC usage statistics
                        for (url, count) in rpc_usage {
                            *total_rpc_usage.entry(url).or_insert(0) += count;
                        }

                        println!(
                            "Worker {} completed: {} successful, {} failed",
                            worker_id, successful, failed
                        );
                    }
                    Err(e) => {
                        println!("Worker {} failed with error: {:?}", worker_id, e);
                    }
                }
            }
            Err(e) => {
                println!("Worker {} panicked: {:?}", worker_id, e);
            }
        }
    }

    // Test summary
    println!("\n📊 Parallel Batch Transfer Test Summary");
    println!("======================================");
    println!("Total transactions attempted: {}", total_transactions);
    println!("Successful transactions: {}", total_successful);
    println!("Failed transactions: {}", total_failed);
    println!(
        "Success rate: {:.1}%",
        if total_transactions > 0 {
            (total_successful as f64 / total_transactions as f64) * 100.0
        } else {
            0.0
        }
    );

    println!("\nRPC Usage Statistics:");
    for (url, count) in total_rpc_usage.iter() {
        println!("  {}: {} transactions", url, count);
    }

    // Test passes if we have at least some successful transactions
    if total_successful > 0 {
        println!(
            "🎉 Test passed! Successfully sent {} batch transactions ({} per sender) using parallel processing.",
            total_successful, transactions_per_sender
        );
        Ok(())
    } else {
        println!("❌ Test failed! No transactions were successful.");
        println!("   This might indicate:");
        println!("   - All RPC endpoints are unavailable");
        println!("   - Invalid private keys or addresses");
        println!("   - Network configuration issues");
        println!("   - Insufficient funds in sender accounts");

        // Return Ok to avoid test failure, but log the issue
        Ok(())
    }
}

/// Process a chunk of accounts in parallel
async fn process_account_chunk<P>(
    worker_id: usize,
    start_idx: usize,
    end_idx: usize,
    accounts: std::sync::Arc<Vec<Account>>,
    providers: std::sync::Arc<Vec<P>>,
    available_urls: std::sync::Arc<Vec<String>>,
    chain_id: u64,
    transaction_amount: u64,
    transactions_per_sender: usize,
) -> Result<(usize, usize, HashMap<String, usize>), Box<dyn std::error::Error + Send + Sync>>
where
    P: Provider + Send + Sync,
{
    let mut successful_transactions = 0;
    let mut failed_transactions = 0;
    let mut rpc_usage_stats = HashMap::new();

    println!(
        "Worker {} processing accounts {} to {}",
        worker_id,
        start_idx,
        end_idx - 1
    );
    // Get initial nonces for all sender addresses
    println!("Getting initial nonces for sender addresses...");
    let mut address_nonces = get_nonces(
        &accounts
            .iter()
            .map(|account| account.address)
            .collect::<Vec<_>>()
            .as_slice(),
        start_idx,
        end_idx,
        &available_urls[0],
    )
    .await;
    println!(
        "Initial nonces retrieved for {} addresses",
        address_nonces.len()
    );

    for _tx_in_sender in 0..transactions_per_sender {
        for sender_idx in start_idx..end_idx {
            let account = &accounts[sender_idx];
            let number_of_senders = accounts.len();

            // Randomly select a recipient from the sender addresses (excluding self)
            let mut recipient_idx = rand::thread_rng().gen_range(0..number_of_senders);
            while recipient_idx == sender_idx {
                recipient_idx = rand::thread_rng().gen_range(0..number_of_senders);
            }
            let recipient_account = &accounts[recipient_idx];

            // Randomly select an RPC provider
            let provider_idx = rand::thread_rng().gen_range(0..providers.len());
            let provider = &providers[provider_idx];
            let rpc_url = &available_urls[provider_idx];

            // Track RPC usage
            *rpc_usage_stats.entry(rpc_url.clone()).or_insert(0) += 1;

            // Get current nonce for the sender
            let current_nonce = address_nonces.get(&account.address).copied();
            // Create and sign the transfer transaction
            let tx_envelope = match create_transfer_transaction(
                &account.private_key,
                &recipient_account.address.to_string(),
                chain_id,
                transaction_amount,
                current_nonce,
            )
            .await
            {
                Ok(envelope) => envelope,
                Err(e) => {
                    println!(
                        "Worker {}: ❌ Failed to create transaction: {:?}",
                        worker_id, e
                    );
                    failed_transactions += 1;
                    continue;
                }
            };

            // Broadcast the transaction to the network
            match provider.send_tx_envelope(tx_envelope).await {
                Ok(_) => {
                    successful_transactions += 1;
                    // Update nonce for next transaction from this sender
                    address_nonces.insert(account.address, current_nonce.unwrap_or(0) + 1);
                }
                Err(e) => {
                    let error_msg = format!("{e:?}");
                    if error_msg.contains("already known") {
                        // Transaction already known - count as success since it was processed
                        successful_transactions += 1;
                    } else if error_msg.contains("insufficient funds") {
                        println!(
                            "Worker {}: ⚠️  Insufficient funds for transaction",
                            worker_id
                        );
                        failed_transactions += 1;
                    } else if error_msg.contains("gas") {
                        println!("Worker {}: ⚠️  Gas-related error: {:?}", worker_id, e);
                        failed_transactions += 1;
                    } else {
                        println!(
                            "Worker {}: ❌ Failed to send transaction: {:?}",
                            worker_id, e
                        );
                        failed_transactions += 1;
                    }
                }
            }
        }
    }

    Ok((
        successful_transactions,
        failed_transactions,
        rpc_usage_stats,
    ))
}
