//! Block scan functionality for FastEVM
//!
//! This module provides utilities to scan blocks from the first block and analyze
//! transaction data. It prints block numbers with timestamps for blocks that contain
//! transactions and aggregates the total number of transactions.

use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_eth::{BlockId, BlockNumberOrTag, BlockTransactions};
use chrono::DateTime;
use eyre::Result;
use std::env;
use std::time::Duration;

/// Configuration for block scanning
#[derive(Debug, Clone)]
pub struct BlockScanConfig {
    /// RPC URL to connect to
    pub rpc_url: String,
    /// Maximum number of blocks to scan (0 = scan all available blocks)
    pub max_blocks: u64,
    /// Whether to include empty blocks in output
    pub include_empty_blocks: bool,
    /// Delay between block requests (in milliseconds)
    pub request_delay_ms: u64,
}

impl Default for BlockScanConfig {
    fn default() -> Self {
        Self {
            rpc_url: env::var("RPC_URL1").unwrap_or_else(|_| "http://localhost:8545".to_string()),
            max_blocks: 0,
            include_empty_blocks: false,
            request_delay_ms: 100,
        }
    }
}

/// Statistics collected during block scanning
#[derive(Debug, Default)]
pub struct BlockScanStats {
    /// Total number of blocks scanned
    pub total_blocks_scanned: u64,
    /// Number of blocks with transactions
    pub blocks_with_transactions: u64,
    /// Total number of transactions found
    pub total_transactions: u64,
    /// Number of empty blocks
    pub empty_blocks: u64,
    /// First block number scanned
    pub first_block: Option<u64>,
    /// Last block number scanned
    pub last_block: Option<u64>,
}

impl BlockScanStats {
    /// Print a summary of the scan statistics
    pub fn print_summary(&self) {
        println!("\n📊 Block Scan Summary");
        println!("====================");
        println!("Total blocks scanned: {}", self.total_blocks_scanned);
        println!(
            "Blocks with transactions: {}",
            self.blocks_with_transactions
        );
        println!("Empty blocks: {}", self.empty_blocks);
        println!("🎯 Total transactions found: {}", self.total_transactions);

        if let Some(first) = self.first_block {
            println!("First block: {}", first);
        }
        if let Some(last) = self.last_block {
            println!("Last block: {}", last);
        }

        if self.total_blocks_scanned > 0 {
            let tx_percentage =
                (self.blocks_with_transactions as f64 / self.total_blocks_scanned as f64) * 100.0;
            println!(
                "Percentage of blocks with transactions: {:.1}%",
                tx_percentage
            );

            let avg_tx_per_block =
                self.total_transactions as f64 / self.total_blocks_scanned as f64;
            println!("Average transactions per block: {:.2}", avg_tx_per_block);
        }
    }
}

/// Convert Unix timestamp to readable format
fn format_timestamp(timestamp: u64) -> String {
    let datetime = DateTime::from_timestamp(timestamp as i64, 0)
        .map(|datetime| datetime.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| format_timestamp2(timestamp));
    datetime
}

/// Convert Unix timestamp to HH:MM:SS format
fn format_timestamp2(timestamp: u64) -> String {
    let hours = timestamp / 3600;
    let minutes = (timestamp % 3600) / 60;
    let seconds = timestamp % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
/// Scan blocks from the first block and analyze transaction data
pub async fn scan_blocks(config: BlockScanConfig) -> Result<BlockScanStats> {
    println!("🔍 Starting block scan...");
    println!("RPC URL: {}", config.rpc_url);
    println!(
        "Max blocks to scan: {}",
        if config.max_blocks == 0 {
            "All available".to_string()
        } else {
            config.max_blocks.to_string()
        }
    );
    println!("Include empty blocks: {}", config.include_empty_blocks);
    println!();

    // Connect to RPC provider
    let provider = ProviderBuilder::new()
        .connect(&config.rpc_url)
        .await
        .map_err(|e| eyre::eyre!("Failed to connect to RPC endpoint: {}", e))?;

    // Get the latest block number to determine scan range
    let latest_block_number = provider
        .get_block_number()
        .await
        .map_err(|e| eyre::eyre!("Failed to get latest block number: {}", e))?;

    println!("Latest block number: {}", latest_block_number);

    let max_block_to_scan = if config.max_blocks == 0 {
        latest_block_number
    } else {
        std::cmp::min(config.max_blocks, latest_block_number)
    };

    println!("Scanning blocks from 0 to {}", max_block_to_scan);
    println!("🔍 Looking for transactions in each block...");
    println!();

    let mut stats = BlockScanStats::default();
    let mut blocks_with_tx_count = 0;
    let mut last_logged_block = 0;

    // Scan blocks from 0 to max_block_to_scan
    for block_number in 0..=max_block_to_scan {
        // Add delay between requests to avoid overwhelming the RPC endpoint
        if block_number > 0 && config.request_delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(config.request_delay_ms)).await;
        }

        // Get block by number
        let block_result = provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(block_number)))
            .await;

        match block_result {
            Ok(Some(block)) => {
                stats.total_blocks_scanned += 1;

                if stats.first_block.is_none() {
                    stats.first_block = Some(block_number);
                }
                stats.last_block = Some(block_number);

                let transaction_count = match &block.transactions {
                    BlockTransactions::Full(txs) => txs.len(),
                    BlockTransactions::Hashes(hashes) => hashes.len(),
                    BlockTransactions::Uncle => 0,
                };

                if transaction_count > 0 {
                    stats.blocks_with_transactions += 1;
                    stats.total_transactions += transaction_count as u64;
                    blocks_with_tx_count += 1;

                    // Format timestamp
                    let timestamp_str = format_timestamp(block.header.timestamp);

                    // Print block information with enhanced transaction logging
                    println!(
                        "🔹 Block #{} | {} | 📊 {} transactions | Total so far: {} | Hash: {:?}",
                        block_number,
                        timestamp_str,
                        transaction_count,
                        stats.total_transactions,
                        block.header.hash
                    );

                    // Print transaction details (first few transactions)
                    // match &block.transactions {
                    //     BlockTransactions::Full(txs) => {
                    //         if transaction_count <= 5 {
                    //             for (tx_idx, tx) in txs.iter().enumerate() {
                    //                 println!("  TX {}: {:?}", tx_idx + 1, tx.inner.hash());
                    //             }
                    //         } else {
                    //             println!("  ... and {} more transactions", transaction_count - 5);
                    //             for (tx_idx, tx) in txs.iter().take(5).enumerate() {
                    //                 println!("  TX {}: {:?}", tx_idx + 1, tx.inner.hash());
                    //             }
                    //         }
                    //     }
                    //     BlockTransactions::Hashes(hashes) => {
                    //         if transaction_count <= 5 {
                    //             for (tx_idx, hash) in hashes.iter().enumerate() {
                    //                 println!("  TX {}: {:?}", tx_idx + 1, hash);
                    //             }
                    //         } else {
                    //             println!("  ... and {} more transactions", transaction_count - 5);
                    //             for (tx_idx, hash) in hashes.iter().take(5).enumerate() {
                    //                 println!("  TX {}: {:?}", tx_idx + 1, hash);
                    //             }
                    //         }
                    //     }
                    //     BlockTransactions::Uncle => {
                    //         // No transactions to display
                    //     }
                    // }
                    println!();
                } else {
                    stats.empty_blocks += 1;

                    if config.include_empty_blocks {
                        let timestamp_str = format_timestamp(block.header.timestamp);
                        println!(
                            "Block #{} | {} | 0 transactions | Hash: {:?}",
                            block_number, timestamp_str, block.header.hash
                        );
                    }
                }

                // Progress indicator with enhanced transaction statistics
                if block_number % 100 == 0 && block_number > 0 {
                    println!(
                        "📈 Progress: Scanned {} blocks | Found {} blocks with transactions | Total transactions: {}",
                        block_number, blocks_with_tx_count, stats.total_transactions
                    );
                }

                // More frequent logging for transaction counts (every 10 blocks with transactions)
                if blocks_with_tx_count > 0
                    && blocks_with_tx_count % 10 == 0
                    && blocks_with_tx_count != last_logged_block
                {
                    println!(
                        "💎 Transaction Milestone: {} blocks with transactions found, {} total transactions",
                        blocks_with_tx_count, stats.total_transactions
                    );
                    last_logged_block = blocks_with_tx_count;
                }
            }
            Ok(None) => {
                println!("⚠️  Block {} not found", block_number);
            }
            Err(e) => {
                println!("❌ Error retrieving block {}: {:?}", block_number, e);
                // Continue scanning other blocks
            }
        }
    }

    println!("✅ Block scan completed!");
    println!(
        "🎯 Final Results: Found {} transactions across {} blocks",
        stats.total_transactions, stats.blocks_with_transactions
    );
    stats.print_summary();

    Ok(stats)
}

/// Convenience function to scan all available blocks
pub async fn scan_all_blocks() -> Result<BlockScanStats> {
    let config = BlockScanConfig::default();
    scan_blocks(config).await
}

/// Convenience function to scan blocks with custom RPC URL
pub async fn scan_blocks_with_url(rpc_url: &str) -> Result<BlockScanStats> {
    let config = BlockScanConfig {
        rpc_url: rpc_url.to_string(),
        ..Default::default()
    };
    scan_blocks(config).await
}

/// Convenience function to scan a specific number of blocks
pub async fn scan_blocks_count(count: u64) -> Result<BlockScanStats> {
    let config = BlockScanConfig {
        max_blocks: count,
        ..Default::default()
    };
    scan_blocks(config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_block_scan_config_default() {
        let config = BlockScanConfig::default();
        assert!(!config.rpc_url.is_empty());
        assert_eq!(config.max_blocks, 0);
        assert!(!config.include_empty_blocks);
        assert_eq!(config.request_delay_ms, 100);
    }

    #[tokio::test]
    async fn test_format_timestamp() {
        let timestamp = 1640995200; // 2022-01-01 00:00:00 UTC
        let formatted = format_timestamp(timestamp);
        assert!(!formatted.is_empty());
    }

    #[tokio::test]
    async fn test_block_scan_stats_default() {
        let stats = BlockScanStats::default();
        assert_eq!(stats.total_blocks_scanned, 0);
        assert_eq!(stats.blocks_with_transactions, 0);
        assert_eq!(stats.total_transactions, 0);
        assert_eq!(stats.empty_blocks, 0);
        assert!(stats.first_block.is_none());
        assert!(stats.last_block.is_none());
    }
}
