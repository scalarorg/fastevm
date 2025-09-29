//! CLI utility for block scanning
//!
//! This module provides a command-line interface for the block scanning functionality.
//! It allows users to easily scan blocks and analyze transaction data from the command line.

use clap::{Parser, Subcommand};
use eyre::Result;
use std::env;

use crate::block_scan::{
    scan_blocks, scan_blocks_count, BlockScanConfig,
};

/// CLI arguments for block scanning
#[derive(Parser, Debug)]
#[command(name = "block-scanner")]
#[command(about = "A utility to scan blockchain blocks and analyze transaction data")]
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
}

/// Main entry point for the CLI
pub async fn run_cli() -> Result<()> {
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
                max_blocks: end - start + 1,
                include_empty_blocks: include_empty,
                request_delay_ms: delay,
            };

            // Note: The current implementation scans from 0, so we need to modify it
            // to support custom start ranges. For now, we'll scan from 0 to end.
            println!("⚠️  Note: Currently scanning from block 0 to {}. Custom start range not yet implemented.", end);

            let stats = scan_blocks(config).await?;
            println!("✅ Range scan completed! Found {} blocks with transactions out of {} total blocks.", 
                    stats.blocks_with_transactions, stats.total_blocks_scanned);
        }
    }

    Ok(())
}

/// Convenience function to run block scanning with default settings
pub async fn run_default_scan() -> Result<()> {
    println!("🚀 Running default block scan (first 10 blocks)...");
    let stats = scan_blocks_count(10).await?;
    println!(
        "✅ Default scan completed! Found {} blocks with transactions.",
        stats.blocks_with_transactions
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test that CLI can be parsed without errors
        let args = vec!["block-scanner", "scan", "--count", "5"];
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
        let args = vec!["block-scanner", "scan-all"];
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
        let args = vec!["block-scanner", "range", "--start", "10", "--end", "20"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Range { start, end, .. } => {
                assert_eq!(start, 10);
                assert_eq!(end, 20);
            }
            _ => panic!("Expected Range command"),
        }
    }
}
