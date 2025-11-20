//! Script to analyze transaction sizes and memory usage
//! Run with: cargo run --bin analyze_memory

use alloy_network::{eip2718::Encodable2718, EthereumWallet, TransactionBuilder};
use alloy_primitives::{Address, Bytes, ChainId, U256};
use alloy_rpc_types_eth::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;

#[tokio::main]
async fn main() {
    println!("=== Transaction Size Analysis ===\n");

    // Create a basic transfer transaction
    let private_key = [1u8; 32];
    let wallet = PrivateKeySigner::from_slice(&private_key).unwrap();
    let recipient = Address::from([2u8; 20]);

    let tx_request = TransactionRequest::default()
        .with_to(recipient)
        .with_value(U256::from(1_000_000_000_000_000_000u64)) // 1 ETH
        .with_gas_limit(21_000) // Standard ETH transfer
        .with_max_priority_fee_per_gas(1_000_000_000) // 1 Gwei
        .with_max_fee_per_gas(20_000_000_000) // 20 Gwei
        .with_nonce(0)
        .with_chain_id(1);

    let ethereum_wallet = EthereumWallet::from(wallet);
    let tx_envelope = tx_request.build(&ethereum_wallet).await.unwrap();
    let raw_tx = tx_envelope.encoded_2718();
    let tx_size = raw_tx.len();

    println!("Basic Transfer Transaction:");
    println!("  Raw transaction size: {} bytes", tx_size);
    println!("  With Arc overhead (8 bytes): {} bytes", tx_size + 8);
    println!("  With Vec overhead (~24 bytes): {} bytes", tx_size + 24);
    println!();

    // Calculate for 400,000 transactions
    let num_txs = 400_000u64;
    let size_per_tx = tx_size as u64;
    let size_with_arc = (tx_size + 8) as u64;
    let size_with_vec = (tx_size + 24) as u64;

    println!("For {} transactions:", num_txs);
    println!("  Raw transactions only: {:.2} MB", (size_per_tx * num_txs) as f64 / 1_000_000.0);
    println!("  With Arc overhead: {:.2} MB", (size_with_arc * num_txs) as f64 / 1_000_000.0);
    println!("  With Vec overhead: {:.2} MB", (size_with_vec * num_txs) as f64 / 1_000_000.0);
    println!("  With 2x overhead (Arc + Vec): {:.2} MB", (size_with_vec * num_txs * 2) as f64 / 1_000_000.0);
    println!();

    // Estimate memory for committed subdags structure
    // Each subdag contains: leader (BlockRef), transactions Vec, timestamp, commit_ref, reputation_scores
    let subdag_overhead = 200; // Rough estimate for subdag metadata
    let txs_per_subdag = 100; // Average transactions per subdag
    let num_subdags = num_txs / txs_per_subdag;
    
    println!("Committed Subdags Structure:");
    println!("  Estimated subdags: {}", num_subdags);
    println!("  Subdag metadata overhead: ~{} bytes each", subdag_overhead);
    println!("  Total subdag overhead: {:.2} MB", (subdag_overhead as u64 * num_subdags) as f64 / 1_000_000.0);
    println!();

    // Total estimated memory
    let total_estimated = (size_with_vec * num_txs * 2) + (subdag_overhead as u64 * num_subdags);
    println!("Total Estimated Memory:");
    println!("  Transactions + overhead: {:.2} MB", total_estimated as f64 / 1_000_000.0);
    println!("  In GB: {:.2} GB", total_estimated as f64 / 1_000_000_000.0);
    println!();

    println!("=== Memory Leak Check Points ===");
    println!("1. Check if committed_queue.remove() is called for all processed subdags");
    println!("2. Check if pending_transactions are properly removed after mining");
    println!("3. Check if payload_buffer grows unbounded");
    println!("4. Check if reth transaction pool removes processed transactions");
    println!("5. Check for duplicate transactions in committed_queue and pending_transactions");
}

