//! Block scan tests for FastEVM
//!
//! This module contains tests for the block scanning functionality.

use eyre::Result;
use testing::block_scan::{scan_all_blocks, scan_blocks, scan_blocks_with_url, BlockScanConfig};

/// Test function to demonstrate block scanning functionality
#[tokio::test]
async fn test_block_scan_demo() -> Result<()> {
    println!("🚀 Starting block scan demo test...");

    // Test scanning the first 10 blocks
    let config = BlockScanConfig {
        max_blocks: 10,
        include_empty_blocks: true,
        request_delay_ms: 50,
        ..Default::default()
    };

    let stats = scan_blocks(config).await?;

    // Verify we got some results
    assert!(
        stats.total_blocks_scanned > 0,
        "Should have scanned at least one block"
    );

    println!("✅ Block scan demo test completed successfully!");
    Ok(())
}

/// Test function to scan all available blocks (use with caution)
#[tokio::test]
async fn test_scan_all_blocks() -> Result<()> {
    println!("🔍 Testing scan all blocks functionality...");

    // This test scans all available blocks - use carefully in production
    let stats = scan_all_blocks().await?;

    // Verify we got results
    assert!(
        stats.total_blocks_scanned > 0,
        "Should have scanned at least one block"
    );

    println!("✅ Scan all blocks test completed successfully!");
    Ok(())
}

/// Test function to scan blocks with custom RPC URL
#[tokio::test]
async fn test_scan_blocks_custom_url() -> Result<()> {
    println!("🌐 Testing block scan with custom RPC URL...");

    let custom_url =
        std::env::var("RPC_URL1").unwrap_or_else(|_| "http://localhost:8545".to_string());
    let stats = scan_blocks_with_url(&custom_url).await?;

    // Verify we got results
    assert!(
        stats.total_blocks_scanned > 0,
        "Should have scanned at least one block"
    );

    println!("✅ Custom URL block scan test completed successfully!");
    Ok(())
}
