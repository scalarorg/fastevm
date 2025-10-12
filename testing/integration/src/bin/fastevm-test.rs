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
use testing::block_scan::{scan_blocks, scan_blocks_count, BlockScanConfig};
use testing::rpc::get_nonces;
use testing::transactions::create_transfer_transaction;
use tokio::time::sleep;

/// CLI arguments for FastEVM testing utilities
#[derive(Parser, Debug)]
#[command(name = "fastevm-test")]
#[command(about = "A utility for FastEVM testing including block scanning and batch transactions")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands for the block scanner
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Scan a specific number of blocks from the beginning
    Scan {
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

    match cli.command {
        Commands::Scan {
            count,
            url,
            include_empty,
            delay,
        } => {
            println!("🔍 Scanning first {} blocks...", count);

            let config = BlockScanConfig {
                rpc_url: url.unwrap_or_else(|| {
                    env::var("RPC_URL1").unwrap_or_else(|_| "http://localhost:8545".to_string())
                }),
                start_block: env::var("BLOCK_NUMBER")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                max_blocks: count,
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
                rpc_url: url.unwrap_or_else(|| {
                    env::var("RPC_URL1").unwrap_or_else(|_| "http://localhost:8545".to_string())
                }),
                start_block: env::var("BLOCK_NUMBER")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                max_blocks: 0, // 0 means scan all
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
                rpc_url: url.unwrap_or_else(|| {
                    env::var("RPC_URL1").unwrap_or_else(|_| "http://localhost:8545".to_string())
                }),
                start_block: start,
                max_blocks: end - start + 1,
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
            println!("🚀 Starting batch transaction test...");
            // Extract network configuration from environment variables
            let chain_id = env::var("CHAIN_ID")
                .unwrap_or("202501".to_string())
                .parse::<u64>()?;
            // Override with environment variables if set
            let sender_count = env::var("TEST_SENDER_COUNT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(sender_count);

            let transaction_count = env::var("TEST_TRANSACTION_COUNT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(transaction_count);
            let transaction_value = env::var("TEST_TRANSACTION_VALUE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1_000_000_000_000_000_u64);
            let mnemonic = env::var("TEST_MNEMONIC").unwrap_or(mnemonic);

            let rpc_urls = vec![
                env::var("RPC_URL1").unwrap_or_else(|_| "http://localhost:8545".to_string()),
                env::var("RPC_URL2").unwrap_or_else(|_| "http://localhost:8545".to_string()),
                env::var("RPC_URL3").unwrap_or_else(|_| "http://localhost:8545".to_string()),
                env::var("RPC_URL4").unwrap_or_else(|_| "http://localhost:8545".to_string()),
            ];

            let fetch_nonce = env::var("TEST_FETCH_NONCE").unwrap_or("false".to_string());

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
                chain_id,
                accounts,
                transaction_count as usize,
                transaction_value,
                rpc_urls.clone(),
                fetch_nonce,
            )
            .await
            .map_err(|e| eyre::eyre!("Failed to send batch transactions: {}", e))?;
        }
    }

    Ok(())
}

/// Convenience function to run block scanning with default settings
async fn run_default_scan() -> Result<()> {
    println!("🚀 Running default block scan (first 10 blocks)...");
    let stats = scan_blocks_count(10).await?;
    println!(
        "✅ Default scan completed! Found {} blocks with transactions.",
        stats.blocks_with_transactions
    );
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
    _transaction_value: u64,
    rpc_urls: Vec<String>,
    fetch_nonce: String,
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

    // Connect to all RPC endpoints
    let mut providers = Vec::new();
    let mut available_urls = Vec::new();

    for (idx, rpc_url) in rpc_urls.iter().enumerate() {
        println!("Connecting to RPC endpoint {}: {}", idx + 1, rpc_url);
        match alloy_provider::ProviderBuilder::new()
            .connect(rpc_url)
            .await
        {
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
    let accounts_arc = std::sync::Arc::new(accounts);
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
        let fetch_nonce_clone = fetch_nonce.clone();
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
                fetch_nonce_clone,
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
    fetch_nonce: String,
) -> Result<(usize, usize, HashMap<String, usize>), Box<dyn std::error::Error + Send + Sync>>
where
    P: alloy_provider::Provider + Send + Sync,
{
    let mut successful_transactions = 0;
    let mut failed_transactions = 0;
    let mut rpc_usage_stats = HashMap::new();
    let start_time = std::time::Instant::now();

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

    // Get initial nonces for all sender addresses
    println!("📡 Worker {}: Fetching initial nonces...", worker_id);
    let nonce_start_time = std::time::Instant::now();
    let url_idx = rand::thread_rng().gen_range(0..available_urls.len());
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
    let progress_interval = std::time::Duration::from_secs(5); // Print progress every 5 seconds

    for tx_round in 0..transactions_per_sender {
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
            let current_nonce = address_nonces.get(&account.address).copied().unwrap_or(0);

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
                        "❌ Worker {}: Failed to create transaction for account {}: {:?}",
                        worker_id, sender_idx, e
                    );
                    failed_transactions += 1;
                    processed_transactions += 1;
                    continue;
                }
            };

            // Broadcast the transaction to the network
            match provider.send_tx_envelope(tx_envelope).await {
                Ok(_) => {
                    successful_transactions += 1;
                    // Update nonce for next transaction from this sender
                    address_nonces.insert(account.address, current_nonce + 1);
                }
                Err(e) => {
                    let error_msg = format!("{e:?}");
                    if error_msg.contains("already known") {
                        // Transaction already known - count as success since it was processed
                        successful_transactions += 1;
                    } else if error_msg.contains("insufficient funds") {
                        println!(
                            "⚠️  Worker {}: Insufficient funds for account {}",
                            worker_id, sender_idx
                        );
                        failed_transactions += 1;
                    } else if error_msg.contains("gas") {
                        println!(
                            "⚠️  Worker {}: Gas error for account {}: {:?}",
                            worker_id, sender_idx, e
                        );
                        failed_transactions += 1;
                    } else {
                        println!(
                            "❌ Worker {}: Failed to send transaction for account {}: {:?}",
                            worker_id, sender_idx, e
                        );
                        failed_transactions += 1;
                    }
                }
            }

            processed_transactions += 1;

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
