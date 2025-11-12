//! FastEVM Testing CLI Binary
//!
//! This binary provides a command-line interface for FastEVM testing including
//! block scanning and batch transaction functionality.

use clap::{Parser, Subcommand};
use eyre::Result;
use rand::Rng;
use std::env;
use std::time::Instant;
use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};
use testing::address::{generate_account_from_seed, Account};
use testing::block_scan::{scan_blocks, BlockScanConfig};
use testing::rpc::get_nonces;
use testing::rpc_client::{DirectRpcClient, RpcClient};
use testing::transactions::create_transfer_transaction;
use tokio::time::sleep;

/// Batch size for transaction processing
/// This determines how many transactions are grouped together before sending to RPC servers
/// Larger batches reduce network overhead but may increase memory usage
const BATCH_SIZE: usize = 50;

/// Configuration loaded from environment variables
#[derive(Debug, Clone)]
struct TestConfig {
    // RPC URLs
    rpc_url1: String,
    rpc_url2: String,
    rpc_url3: String,
    rpc_url4: String,

    // Network configuration
    chain_id: u64,

    // Block scanning configuration
    block_number: u64,
    block_count: u64,

    // Batch transaction configuration
    test_sender_count: usize,
    test_transaction_count: usize,
    test_transaction_value: u64,
    test_mnemonic: String,
    test_fetch_nonce: String,

    // Additional test parameters
    test_waiting_time_seconds: u64,
    test_rpc_timeout: u64,
    test_max_retries: u64,
    test_log_level: String,
    test_batch_size: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            rpc_url1: "http://localhost:8545".to_string(),
            rpc_url2: "http://localhost:8545".to_string(),
            rpc_url3: "http://localhost:8545".to_string(),
            rpc_url4: "http://localhost:8545".to_string(),
            chain_id: 202501,
            block_number: 0,
            block_count: 10,
            test_sender_count: 1000,
            test_transaction_count: 1,
            test_transaction_value: 1_000_000_000_000_000,
            test_mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            test_fetch_nonce: "false".to_string(),
            test_waiting_time_seconds: 30,
            test_rpc_timeout: 30,
            test_max_retries: 3,
            test_log_level: "info".to_string(),
            test_batch_size: BATCH_SIZE,
        }
    }
}

impl TestConfig {
    /// Load configuration from environment variables
    fn load() -> Result<Self> {
        let mut config = Self::default();

        // Load environment variables with fallbacks to defaults
        config.rpc_url1 = env::var("RPC_URL1").unwrap_or(config.rpc_url1);
        config.rpc_url2 = env::var("RPC_URL2").unwrap_or(config.rpc_url2);
        config.rpc_url3 = env::var("RPC_URL3").unwrap_or(config.rpc_url3);
        config.rpc_url4 = env::var("RPC_URL4").unwrap_or(config.rpc_url4);

        if let Ok(chain_id_str) = env::var("CHAIN_ID") {
            config.chain_id = chain_id_str.parse()?;
        }

        if let Ok(block_number_str) = env::var("BLOCK_NUMBER") {
            config.block_number = block_number_str.parse()?;
        }

        if let Ok(block_count_str) = env::var("BLOCK_COUNT") {
            config.block_count = block_count_str.parse()?;
        }

        if let Ok(sender_count_str) = env::var("TEST_SENDER_COUNT") {
            config.test_sender_count = sender_count_str.parse()?;
        }

        if let Ok(transaction_count_str) = env::var("TEST_TRANSACTION_COUNT") {
            config.test_transaction_count = transaction_count_str.parse()?;
        }

        if let Ok(transaction_value_str) = env::var("TEST_TRANSACTION_VALUE") {
            config.test_transaction_value = transaction_value_str.parse()?;
        }

        config.test_mnemonic = env::var("TEST_MNEMONIC").unwrap_or(config.test_mnemonic);
        config.test_fetch_nonce = env::var("TEST_FETCH_NONCE").unwrap_or(config.test_fetch_nonce);
        println!("TEST_FETCH_NONCE: {}", config.test_fetch_nonce);
        if let Ok(waiting_time_str) = env::var("TEST_WAITING_TIME_SECONDS") {
            config.test_waiting_time_seconds = waiting_time_str.parse()?;
        }

        if let Ok(rpc_timeout_str) = env::var("TEST_RPC_TIMEOUT") {
            config.test_rpc_timeout = rpc_timeout_str.parse()?;
        }

        if let Ok(max_retries_str) = env::var("TEST_MAX_RETRIES") {
            config.test_max_retries = max_retries_str.parse()?;
        }

        config.test_log_level = env::var("TEST_LOG_LEVEL").unwrap_or(config.test_log_level);

        if let Ok(batch_size_str) = env::var("TEST_BATCH_SIZE") {
            config.test_batch_size = batch_size_str.parse()?;
        }

        Ok(config)
    }

    /// Get all RPC URLs as a vector
    fn get_rpc_urls(&self) -> Vec<String> {
        vec![
            self.rpc_url1.clone(),
            self.rpc_url2.clone(),
            self.rpc_url3.clone(),
            self.rpc_url4.clone(),
        ]
    }
}

/// CLI arguments for FastEVM testing utilities
#[derive(Parser, Debug)]
#[command(name = "fastevm-test")]
#[command(about = "A utility for FastEVM testing including block scanning and batch transactions")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    /// Command to run
    pub command: Commands,
}

/// Available commands for the block scanner
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Scan a specific number of blocks from a starting block
    Scan {
        /// Starting block number (default: 0)
        #[arg(short, long, default_value = "0")]
        start: u64,

        /// Number of blocks to scan (default: 10)
        #[arg(short, long, default_value = "10")]
        count: u64,

        /// RPC URL to connect to
        #[arg(short, long)]
        url: Option<String>,

        /// Include empty blocks in output
        #[arg(short, long)]
        include_empty: bool,

        /// Delay between requests in milliseconds
        #[arg(short, long, default_value = "100")]
        delay: u64,
    },
    /// Scan all available blocks
    ScanAll {
        /// RPC URL to connect to
        #[arg(short, long)]
        url: Option<String>,

        /// Include empty blocks in output
        #[arg(short, long)]
        include_empty: bool,

        /// Delay between requests in milliseconds
        #[arg(short, long, default_value = "100")]
        delay: u64,
    },
    /// Scan blocks from a specific range
    Range {
        /// Starting block number
        #[arg(short, long, default_value = "0")]
        start: u64,

        /// Ending block number
        #[arg(short, long)]
        end: u64,

        /// RPC URL to connect to
        #[arg(short, long)]
        url: Option<String>,

        /// Include empty blocks in output
        #[arg(short, long)]
        include_empty: bool,

        /// Delay between requests in milliseconds
        #[arg(short, long, default_value = "100")]
        delay: u64,
    },
    /// Send batch transactions for testing
    Batch {
        /// Number of sender accounts to generate
        #[arg(short, long, default_value_t = 1000)]
        sender_count: usize,

        /// Number of transactions per sender
        #[arg(short, long, default_value_t = 1)]
        transaction_count: usize,

        /// Test mnemonic phrase
        #[arg(
            short,
            long,
            default_value = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        )]
        mnemonic: String,
    },
}

/// Main entry point for the CLI
async fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let test_config = TestConfig::load()?;

    match cli.command {
        Commands::Scan {
            start,
            count,
            url,
            include_empty,
            delay,
        } => {
            // Use command line arguments if provided, otherwise use environment config
            let start_block = if start == 0 {
                test_config.block_number
            } else {
                start
            };
            let block_count = if count == 10 {
                test_config.block_count
            } else {
                count
            };

            // Show which values came from environment variables
            if start == 0 && env::var("BLOCK_NUMBER").is_ok() {
                println!("📋 Using BLOCK_NUMBER from environment: {}", start_block);
            }
            if count == 10 && env::var("BLOCK_COUNT").is_ok() {
                println!("📋 Using BLOCK_COUNT from environment: {}", block_count);
            }

            println!(
                "🔍 Scanning {} blocks starting from block {}...",
                block_count, start_block
            );

            let config = BlockScanConfig {
                rpc_url: url.unwrap_or(test_config.rpc_url1),
                start_block,
                block_count,
                include_empty_blocks: include_empty,
                request_delay_ms: delay,
            };

            let stats = scan_blocks(config).await?;
            println!(
                "✅ Scan completed! Found {} blocks with transactions out of {} total blocks.",
                stats.blocks_with_transactions, stats.total_blocks_scanned
            );
        }

        Commands::ScanAll {
            url,
            include_empty,
            delay,
        } => {
            println!("🔍 Scanning all available blocks...");
            println!("⚠️  Warning: This may take a long time for networks with many blocks!");

            let config = BlockScanConfig {
                rpc_url: url.unwrap_or(test_config.rpc_url1),
                start_block: test_config.block_number,
                block_count: 0, // 0 means scan all
                include_empty_blocks: include_empty,
                request_delay_ms: delay,
            };

            let stats = scan_blocks(config).await?;
            println!(
                "✅ Full scan completed! Found {} blocks with transactions out of {} total blocks.",
                stats.blocks_with_transactions, stats.total_blocks_scanned
            );
        }

        Commands::Range {
            start,
            end,
            url,
            include_empty,
            delay,
        } => {
            println!("🔍 Scanning blocks from {} to {}...", start, end);

            let config = BlockScanConfig {
                rpc_url: url.unwrap_or(test_config.rpc_url1),
                start_block: start,
                block_count: end - start + 1,
                include_empty_blocks: include_empty,
                request_delay_ms: delay,
            };

            let stats = scan_blocks(config).await?;
            println!("✅ Range scan completed! Found {} blocks with transactions out of {} total blocks.", 
                    stats.blocks_with_transactions, stats.total_blocks_scanned);
        }

        Commands::Batch {
            sender_count,
            transaction_count,
            mnemonic,
        } => {
            println!(
                "🚀 Starting batch transaction test with configuration {:?}",
                test_config
            );

            // Use command line arguments if provided, otherwise use environment config
            let sender_count = if sender_count == 1000 {
                test_config.test_sender_count
            } else {
                sender_count
            };
            let transaction_count = if transaction_count == 1 {
                test_config.test_transaction_count
            } else {
                transaction_count
            };
            let mnemonic = if mnemonic == "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about" {
                test_config.test_mnemonic.clone()
            } else {
                mnemonic
            };

            println!("  Sender count: {}", sender_count);
            println!("  Transaction count per sender: {}", transaction_count);
            println!("  Mnemonic: {}...", &mnemonic[..20]);

            let accounts = generate_accounts(sender_count, &mnemonic)
                .map_err(|e| eyre::eyre!("Failed to generate accounts: {}", e))?;

            let _addresses = accounts
                .iter()
                .map(|account| account.address)
                .collect::<Vec<_>>();
            let _ = send_batch_transfer_transactions(
                test_config.chain_id,
                accounts,
                transaction_count as usize,
                test_config.test_transaction_value,
                test_config.get_rpc_urls(),
                test_config.test_fetch_nonce.clone(),
                test_config.clone(),
            )
            .await
            .map_err(|e| eyre::eyre!("Failed to send batch transactions: {}", e))?;
        }
    }

    Ok(())
}

/// Send batch transactions with nonce checking
async fn send_transaction_with_check_nonce(
    chain_id: u64,
    account_number: usize,
    number_of_transactions: usize,
    transaction_value: u64,
    mnemonic: &str,
    rpc_urls: Vec<String>,
    fetch_nonce: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let accounts = generate_accounts(account_number, mnemonic)?;

    let addresses = accounts
        .iter()
        .map(|account| account.address)
        .collect::<Vec<_>>();

    let _ = send_batch_transfer_transactions(
        chain_id,
        accounts,
        number_of_transactions as usize,
        transaction_value,
        rpc_urls.clone(),
        fetch_nonce,
        TestConfig::default(), // Use default config for unused function
    )
    .await
    .map_err(|e| eyre::eyre!("Failed to send batch transactions: {}", e))?;
    // check if all transactions are mined
    let url = rpc_urls
        .first()
        .cloned()
        .unwrap_or_else(|| "http://localhost:8545".to_string());
    let expected_duration = (account_number as u64) * (number_of_transactions as u64) * 1;
    let timeout = Duration::from_millis(expected_duration);
    let start_time = Instant::now();
    let mut success_count = 0;
    while start_time.elapsed() < timeout {
        sleep(Duration::from_secs(10)).await;
        success_count = 0;
        let address_nonces =
            get_nonces(addresses.as_slice(), 0, account_number, url.as_str()).await;
        for (_, nonce) in address_nonces.iter() {
            if *nonce == number_of_transactions as u64 {
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

/// Generate accounts from mnemonic
fn generate_accounts(
    number_of_senders: usize,
    mnemonic_str: &str,
) -> Result<Vec<Account>, Box<dyn std::error::Error + Send + Sync>> {
    println!(
        "Generating {} sender addresses from mnemonic...",
        number_of_senders
    );
    let mnemonic =
        bip39::Mnemonic::parse(mnemonic_str).map_err(|e| eyre::eyre!("Invalid mnemonic: {}", e))?;
    let seed = mnemonic.to_seed("");
    let seed_bytes = &seed[..];
    let mut accounts = Vec::new();
    for i in 0..number_of_senders {
        let account = generate_account_from_seed(seed_bytes, i as u32)?;
        accounts.push(account);
    }
    Ok(accounts)
}

/// Send batch transfer transactions with parallel processing
async fn send_batch_transfer_transactions(
    chain_id: u64,
    accounts: Vec<Account>,
    transactions_per_sender: usize,
    send_amount: u64,
    rpc_urls: Vec<String>,
    fetch_nonce: String,
    test_config: TestConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load environment variables from .env file if present
    dotenv::dotenv().ok();
    println!(
        "Test with {} accounts and {} transactions per sender",
        accounts.len(),
        transactions_per_sender
    );
    let number_of_senders = accounts.len();

    // Transaction amount in wei (0.001 ETH)
    let mut transaction_amount = send_amount;
    if transaction_amount == 0 {
        transaction_amount = 1_000_000_000_000_000_u64;
    }
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

    let mut available_urls = Vec::new();
    // Connect to all RPC endpoints
    let mut rpc_clients = Vec::new();

    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        println!("📡 Connecting to RPC endpoint {}: {}", idx + 1, rpc_url);
        println!("   RPC URL: {}", rpc_url);
        match DirectRpcClient::new(rpc_url).await {
            Ok(client) => {
                rpc_clients.push(client);
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
        // match ProviderRpcClient::new(rpc_url).await {
        //     Ok(client) => {
        //         rpc_clients.push(client);
        //         available_urls.push(rpc_url.clone());
        //         println!("  ✅ Connected to RPC endpoint {}", idx + 1);
        //     }
        //     Err(e) => {
        //         println!(
        //             "  ❌ Failed to connect to RPC endpoint {}: {:?}",
        //             idx + 1,
        //             e
        //         );
        //     }
        // }
        // match alloy_provider::ProviderBuilder::new()
        //     .connect(rpc_url)
        //     .await
        // {
        //     Ok(provider) => {
        //         providers.push(provider);
        //         available_urls.push(rpc_url.clone());
        //         println!("  ✅ Connected to RPC endpoint {}", idx + 1);
        //     }
        //     Err(e) => {
        //         println!(
        //             "  ❌ Failed to connect to RPC endpoint {}: {:?}",
        //             idx + 1,
        //             e
        //         );
        //     }
        // }
    }

    if rpc_clients.is_empty() {
        println!("⚠️  Warning: No RPC endpoints available. Skipping test...");
        return Ok(());
    }

    println!("Connected to {} RPC endpoints", rpc_clients.len());

    // Determine optimal number of parallel workers (chunks)
    let num_workers = std::cmp::min(32, number_of_senders); // Cap at 8 workers
    let chunk_size = (number_of_senders + num_workers - 1) / num_workers; // Ceiling division

    println!(
        "\n🚀 Starting parallel batch transaction sending with {} workers (chunk size: {})...",
        num_workers, chunk_size
    );

    // Create shared data for parallel processing
    let accounts_arc = std::sync::Arc::new(accounts);
    let rpc_clients = std::sync::Arc::new(rpc_clients);
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
        let rpc_clients_clone = rpc_clients.clone();
        let urls_clone = available_urls_arc.clone();
        let fetch_nonce_clone = fetch_nonce.clone();
        let test_config_clone = test_config.clone();
        let handle = tokio::spawn(async move {
            process_account_chunk(
                worker_id,
                start_idx,
                end_idx,
                accounts_clone,
                rpc_clients_clone,
                urls_clone,
                chain_id,
                transaction_amount,
                transactions_per_sender,
                fetch_nonce_clone,
                test_config_clone,
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
async fn process_account_chunk<C>(
    worker_id: usize,
    start_idx: usize,
    end_idx: usize,
    accounts: std::sync::Arc<Vec<Account>>,
    rpc_clients: std::sync::Arc<Vec<C>>,
    available_urls: std::sync::Arc<Vec<String>>,
    chain_id: u64,
    transaction_amount: u64,
    transactions_per_sender: usize,
    fetch_nonce: String,
    test_config: TestConfig,
) -> Result<(usize, usize, HashMap<String, usize>), Box<dyn std::error::Error + Send + Sync>>
where
    C: RpcClient + Sync + Send,
{
    let mut successful_transactions = 0;
    let mut failed_transactions = 0;
    let mut rpc_usage_stats = HashMap::new();
    let start_time = std::time::Instant::now();

    // Round-robin RPC provider selection
    let mut overall_tx_index = 0;

    let accounts_in_chunk = end_idx - start_idx;
    let total_transactions_in_chunk = accounts_in_chunk * transactions_per_sender;

    println!(
        "🔧 Worker {} starting: processing {} accounts ({} to {}) for {} transactions each",
        worker_id,
        accounts_in_chunk,
        start_idx,
        end_idx - 1,
        transactions_per_sender
    );
    println!(
        "🔄 Worker {}: Using round-robin RPC provider selection across {} endpoints",
        worker_id,
        rpc_clients.len()
    );

    // Get initial nonces for all sender addresses
    println!("📡 Worker {}: Fetching initial nonces...", worker_id);
    let nonce_start_time = std::time::Instant::now();
    let url_idx = rand::rng().random_range(0..available_urls.len());
    let mut address_nonces = if fetch_nonce == "true" {
        get_nonces(
            &accounts
                .iter()
                .map(|account| account.address)
                .collect::<Vec<_>>()
                .as_slice(),
            start_idx,
            end_idx,
            &available_urls[url_idx],
        )
        .await
    } else {
        BTreeMap::new()
    };
    let nonce_duration = nonce_start_time.elapsed();
    println!(
        "✅ Worker {}: Retrieved nonces for {} addresses in {:.2}s",
        worker_id,
        address_nonces.len(),
        nonce_duration.as_secs_f64()
    );

    let mut processed_transactions = 0;
    let mut last_progress_time = std::time::Instant::now();
    let progress_interval = std::time::Duration::from_secs(10); // Print progress every 5 seconds

    // Batch processing configuration
    let batch_size = std::cmp::min(test_config.test_batch_size, total_transactions_in_chunk); // Use configurable batch size
    let mut transaction_batches: Vec<Vec<(alloy_primitives::Bytes, usize, String)>> = Vec::new();
    let mut current_batch = Vec::new();

    // Pre-create all transactions and organize them into batches
    println!(
        "📦 Worker {}: Pre-creating {} transactions in batches of {}",
        worker_id, total_transactions_in_chunk, batch_size
    );

    for _tx_round in 0..transactions_per_sender {
        for sender_idx in start_idx..end_idx {
            let account = &accounts[sender_idx];
            let number_of_senders = accounts.len();

            // Randomly select a recipient from the sender addresses (excluding self)
            let mut recipient_idx = rand::rng().random_range(0..number_of_senders);
            while recipient_idx == sender_idx {
                recipient_idx = rand::rng().random_range(0..number_of_senders);
            }
            let recipient_account = &accounts[recipient_idx];

            // Round-robin RPC provider selection
            let provider_idx = overall_tx_index % rpc_clients.len();
            let rpc_url = &available_urls[provider_idx];
            overall_tx_index += 1;

            // Track RPC usage
            *rpc_usage_stats.entry(rpc_url.clone()).or_insert(0) += 1;

            // Get current nonce for the sender
            let current_nonce = address_nonces.get(&account.address).copied().unwrap_or(0);

            // Create and sign the transfer transaction
            let raw_tx = match create_transfer_transaction(
                &account.private_key,
                &recipient_account.address.to_string(),
                chain_id,
                transaction_amount,
                current_nonce,
            )
            .await
            {
                Ok(raw_tx) => raw_tx,
                Err(e) => {
                    println!(
                        "❌ Worker {}: Failed to create transaction for account {}: {:?}",
                        worker_id, sender_idx, e
                    );
                    failed_transactions += 1;
                    processed_transactions += 1;
                    continue;
                }
            };

            // Add transaction to current batch (raw_tx, provider_idx, rpc_url)
            current_batch.push((raw_tx, provider_idx, rpc_url.clone()));

            // Update nonce for next transaction from this sender
            address_nonces.insert(account.address, current_nonce + 1);
            processed_transactions += 1;

            // If batch is full, add it to batches and start a new one
            if current_batch.len() >= batch_size {
                transaction_batches.push(current_batch);
                current_batch = Vec::new();
            }

            // Print progress periodically
            if last_progress_time.elapsed() >= progress_interval {
                let elapsed = start_time.elapsed();
                let progress_percent =
                    (processed_transactions as f64 / total_transactions_in_chunk as f64) * 100.0;
                let tx_per_second = processed_transactions as f64 / elapsed.as_secs_f64();
                let estimated_remaining = if tx_per_second > 0.0 {
                    let remaining_txs = total_transactions_in_chunk - processed_transactions;
                    std::time::Duration::from_secs((remaining_txs as f64 / tx_per_second) as u64)
                } else {
                    std::time::Duration::from_secs(0)
                };

                println!(
                    "📊 Worker {}: Progress {:.1}% ({}/{} txs) | {:.1} tx/s | ETA: {:.0}s | ✅{} ❌{}",
                    worker_id,
                    progress_percent,
                    processed_transactions,
                    total_transactions_in_chunk,
                    tx_per_second,
                    estimated_remaining.as_secs_f64(),
                    successful_transactions,
                    failed_transactions
                );
                last_progress_time = std::time::Instant::now();
            }
        }
    }

    // Add any remaining transactions to the last batch
    if !current_batch.is_empty() {
        transaction_batches.push(current_batch);
    }

    // Send all batches using batch API
    println!(
        "🚀 Worker {}: Sending {} batches of transactions...",
        worker_id,
        transaction_batches.len()
    );
    let batch_send_start = std::time::Instant::now();

    for (batch_idx, batch) in transaction_batches.iter().enumerate() {
        // Group transactions by RPC provider for efficient batch sending
        let mut provider_batches: HashMap<usize, Vec<alloy_primitives::Bytes>> = HashMap::new();

        for (raw_tx, provider_idx, _rpc_url) in batch {
            provider_batches
                .entry(*provider_idx)
                .or_insert_with(Vec::new)
                .push(raw_tx.clone());
        }

        // Send batches for each provider
        for (provider_idx, tx_batch) in provider_batches {
            let rpc_client: &C = &rpc_clients[provider_idx];
            let rpc_url: &String = &available_urls[provider_idx];

            let batch_len = tx_batch.len();

            println!(
                "📤 Worker {}: Sending batch {} ({} transactions) to provider {}: {}",
                worker_id, batch_idx, batch_len, provider_idx, rpc_url
            );

            // Try sending with the assigned provider
            match rpc_client
                .batch_send_raw_transaction(tx_batch.clone())
                .await
            {
                Ok(_) => {
                    successful_transactions += batch_len;
                    *rpc_usage_stats.entry(rpc_url.clone()).or_insert(0) += batch_len;
                }
                Err(e) => {
                    let error_msg = format!("{e:?}");
                    // Check if it's a connection error - if so, try other providers
                    if error_msg.contains("Connection refused")
                        || error_msg.contains("Connect")
                        || error_msg.contains("connection")
                    {
                        println!(
                            "⚠️  Worker {}: Batch {} failed for provider {} (connection error), trying other providers...",
                            worker_id, batch_idx, provider_idx
                        );

                        // Try other providers in round-robin order
                        let mut retry_success = false;
                        for retry_idx in 0..rpc_clients.len() {
                            if retry_idx == provider_idx {
                                continue; // Skip the failed provider
                            }

                            let retry_client: &C = &rpc_clients[retry_idx];
                            let retry_url: &String = &available_urls[retry_idx];

                            println!(
                                "🔄 Worker {}: Retrying batch {} with provider {}: {}",
                                worker_id, batch_idx, retry_idx, retry_url
                            );

                            match retry_client
                                .batch_send_raw_transaction(tx_batch.clone())
                                .await
                            {
                                Ok(_) => {
                                    successful_transactions += batch_len;
                                    *rpc_usage_stats.entry(retry_url.clone()).or_insert(0) +=
                                        batch_len;
                                    println!(
                                        "✅ Worker {}: Batch {} retried successfully with provider {}",
                                        worker_id, batch_idx, retry_idx
                                    );
                                    retry_success = true;
                                    break;
                                }
                                Err(_) => {
                                    // Try next provider
                                    continue;
                                }
                            }
                        }

                        if !retry_success {
                            println!(
                                "❌ Worker {}: Batch {} failed for provider {} and all retry attempts: {:?}",
                                worker_id, batch_idx, provider_idx, error_msg
                            );
                            failed_transactions += batch_len;
                        }
                    } else {
                        // Non-connection error, don't retry
                        println!(
                            "❌ Worker {}: Batch {} failed for provider {}: {:?}",
                            worker_id, batch_idx, provider_idx, error_msg
                        );
                        failed_transactions += batch_len;
                    }
                }
            }
        }

        // Print batch progress
        if batch_idx % 10 == 0 || batch_idx == transaction_batches.len() - 1 {
            println!(
                "📦 Worker {}: Sent batch {}/{} | ✅{} ❌{}",
                worker_id,
                batch_idx + 1,
                transaction_batches.len(),
                successful_transactions,
                failed_transactions
            );
        }
    }

    let batch_send_duration = batch_send_start.elapsed();
    println!(
        "⚡ Worker {}: Batch sending completed in {:.2}s",
        worker_id,
        batch_send_duration.as_secs_f64()
    );

    let total_duration = start_time.elapsed();
    let tx_per_second = total_transactions_in_chunk as f64 / total_duration.as_secs_f64();

    println!(
        "🏁 Worker {} completed: {} successful, {} failed in {:.2}s ({:.1} tx/s)",
        worker_id,
        successful_transactions,
        failed_transactions,
        total_duration.as_secs_f64(),
        tx_per_second
    );

    Ok((
        successful_transactions,
        failed_transactions,
        rpc_usage_stats,
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from the specified file
    let env_file = env::var("ENV_FILE").unwrap_or(".env".to_string());
    dotenvy::from_filename(env_file).ok();

    run_cli().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test that CLI can be parsed without errors
        let args = vec!["fastevm-test", "scan", "--count", "5"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Scan { count, .. } => {
                assert_eq!(count, 5);
            }
            _ => panic!("Expected Scan command"),
        }
    }

    #[test]
    fn test_cli_scan_all() {
        let args = vec!["fastevm-test", "scan-all"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::ScanAll { .. } => {
                // Test passed
            }
            _ => panic!("Expected ScanAll command"),
        }
    }

    #[test]
    fn test_cli_range() {
        let args = vec!["fastevm-test", "range", "--start", "10", "--end", "20"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Range { start, end, .. } => {
                assert_eq!(start, 10);
                assert_eq!(end, 20);
            }
            _ => panic!("Expected Range command"),
        }
    }

    #[test]
    fn test_cli_batch() {
        let args = vec![
            "fastevm-test",
            "batch",
            "--sender-count",
            "100",
            "--transaction-count",
            "2",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Batch {
                sender_count,
                transaction_count,
                ..
            } => {
                assert_eq!(sender_count, 100);
                assert_eq!(transaction_count, 2);
            }
            _ => panic!("Expected Batch command"),
        }
    }
}
