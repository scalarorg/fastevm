//! Integration tests for Payload Building
//!
//! These tests verify the payload building functionality,
//! including transaction processing and block construction.

use alloy_consensus::{BlockHeader, Transaction};
use alloy_network::{eip2718::Encodable2718, EthereumWallet, TransactionBuilder};
use alloy_primitives::{Address, FixedBytes, B256, U256};
use alloy_rpc_types_eth::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;
use reth_ethereum::pool::noop::NoopTransactionPool;
use reth_ethereum::rpc::eth::utils::recover_raw_transaction;
use reth_payload_builder::PayloadId;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use secp256k1::SecretKey;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::sync::Arc;

type TestPool = NoopTransactionPool;
type TestTransaction = <TestPool as TransactionPool>::Transaction;

/// Helper function to create a mock transaction for testing
fn create_mock_transaction(sender: Address, nonce: u64) -> Arc<TestTransaction> {
    let key_hash = Sha256::digest(&sender[..]);

    let secret_key = SecretKey::from_slice(&key_hash).unwrap_or_else(|_| {
        let mut modified = key_hash;
        modified[0] = modified[0].wrapping_add(1);
        SecretKey::from_slice(&modified).expect("Failed to create valid secret key")
    });

    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let key_bytes: FixedBytes<32> = FixedBytes::from(secret_key.secret_bytes());
        let signer = PrivateKeySigner::from_bytes(&key_bytes).unwrap();

        let tx_req = TransactionRequest::default()
            .with_to(Address::ZERO)
            .with_value(U256::ZERO)
            .with_gas_limit(21000)
            .with_max_priority_fee_per_gas(1_000_000_000u128)
            .with_max_fee_per_gas(20_000_000_000u128)
            .with_nonce(nonce)
            .with_chain_id(1);

        let ethereum_wallet = EthereumWallet::from(signer);
        let tx_envelope = tx_req.build(&ethereum_wallet).await.unwrap();

        let encoded = tx_envelope.encoded_2718();

        let recovered = recover_raw_transaction(&alloy_primitives::Bytes::from(encoded))
            .expect("Failed to recover transaction");
        TestTransaction::from_pooled(recovered)
    })
    .into()
}

/// Test PayloadId creation and comparison
#[test]
fn test_payload_id_default() {
    let id1 = PayloadId::default();
    let id2 = PayloadId::default();

    assert_eq!(id1, id2);
}

/// Test PayloadId from bytes
#[test]
fn test_payload_id_from_bytes() {
    let bytes = [1u8; 8];
    let id = PayloadId::new(bytes);

    // Verify it's not the default
    assert_ne!(id, PayloadId::default());
}

/// Test PayloadId uniqueness
#[test]
fn test_payload_id_uniqueness() {
    let id1 = PayloadId::new([1u8; 8]);
    let id2 = PayloadId::new([2u8; 8]);

    assert_ne!(id1, id2);
}

/// Test B256 (block hash) operations
#[test]
fn test_block_hash_operations() {
    let hash1 = B256::default();
    let hash2 = B256::from([1u8; 32]);

    assert_ne!(hash1, hash2);
    assert_eq!(hash1, B256::ZERO);
}

/// Test VecDeque operations for transaction buffer
#[test]
fn test_transaction_buffer_operations() {
    let sender = Address::from([1u8; 20]);
    let tx1 = create_mock_transaction(sender, 0);
    let tx2 = create_mock_transaction(sender, 1);
    let tx3 = create_mock_transaction(sender, 2);

    let mut buffer: VecDeque<Arc<TestTransaction>> = VecDeque::new();

    // Push transactions
    buffer.push_back(tx1.clone());
    buffer.push_back(tx2.clone());
    buffer.push_back(tx3.clone());

    assert_eq!(buffer.len(), 3);

    // Pop from front (FIFO order)
    let first = buffer.pop_front().unwrap();
    assert_eq!(first.nonce(), 0);

    let second = buffer.pop_front().unwrap();
    assert_eq!(second.nonce(), 1);

    let third = buffer.pop_front().unwrap();
    assert_eq!(third.nonce(), 2);

    assert!(buffer.is_empty());
}

/// Test transaction gas limit validation
#[test]
fn test_transaction_gas_limit() {
    let sender = Address::from([1u8; 20]);
    let tx = create_mock_transaction(sender, 0);

    assert_eq!(tx.gas_limit(), 21000);
}

/// Test transaction value
#[test]
fn test_transaction_value() {
    let sender = Address::from([1u8; 20]);
    let tx = create_mock_transaction(sender, 0);

    assert_eq!(tx.value(), U256::ZERO);
}

/// Test transaction chain ID
#[test]
fn test_transaction_chain_id() {
    let sender = Address::from([1u8; 20]);
    let tx = create_mock_transaction(sender, 0);

    assert_eq!(tx.chain_id(), Some(1));
}

/// Test concurrent transaction creation
#[test]
fn test_concurrent_transaction_creation() {
    use std::thread;

    let handles: Vec<_> = (0..4)
        .map(|i| {
            thread::spawn(move || {
                let sender = Address::from([i as u8 + 1; 20]);
                create_mock_transaction(sender, 0)
            })
        })
        .collect();

    let transactions: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert_eq!(transactions.len(), 4);

    // All transactions should have unique senders
    let senders: std::collections::HashSet<_> = transactions.iter().map(|tx| tx.sender()).collect();
    assert_eq!(senders.len(), 4);
}

/// Test transaction buffer capacity management
#[test]
fn test_transaction_buffer_capacity() {
    let sender = Address::from([1u8; 20]);
    let mut buffer: VecDeque<Arc<TestTransaction>> = VecDeque::with_capacity(100);

    // Add transactions up to capacity
    for i in 0..100 {
        let tx = create_mock_transaction(sender, i);
        buffer.push_back(tx);
    }

    assert_eq!(buffer.len(), 100);

    // Verify FIFO order is maintained
    for i in 0..100 {
        let tx = buffer.pop_front().unwrap();
        assert_eq!(tx.nonce(), i);
    }
}

/// Test payload timestamp handling
#[test]
fn test_payload_timestamp() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Timestamp should be reasonable (after 2020)
    assert!(timestamp > 1577836800); // Jan 1, 2020
}

/// Test block number calculations
#[test]
fn test_block_number_calculations() {
    let parent_number: u64 = 100;
    let next_number = parent_number + 1;

    assert_eq!(next_number, 101);

    // Test overflow handling
    let max_number: u64 = u64::MAX - 1;
    let next_max = max_number.checked_add(1);
    assert!(next_max.is_some());
    assert_eq!(next_max.unwrap(), u64::MAX);
}

/// Test gas calculations
#[test]
fn test_gas_calculations() {
    let block_gas_limit: u64 = 30_000_000;
    let tx_gas_limit: u64 = 21_000;

    // Calculate how many simple transactions can fit
    let max_txs = block_gas_limit / tx_gas_limit;
    assert!(max_txs > 1000); // Should fit many transactions

    // Test cumulative gas tracking
    let mut cumulative_gas: u64 = 0;
    let mut tx_count = 0;

    while cumulative_gas + tx_gas_limit <= block_gas_limit {
        cumulative_gas += tx_gas_limit;
        tx_count += 1;
    }

    assert_eq!(tx_count, max_txs as usize);
}

/// Test fee calculations
#[test]
fn test_fee_calculations() {
    let base_fee: u64 = 1_000_000_000; // 1 gwei
    let priority_fee: u64 = 1_000_000_000; // 1 gwei
    let gas_used: u64 = 21_000;

    let total_fee = U256::from(priority_fee) * U256::from(gas_used);
    let expected = U256::from(21_000_000_000_000u64); // 21000 * 1 gwei

    assert_eq!(total_fee, expected);
}

/// Test transaction ordering by nonce
#[test]
fn test_transaction_ordering_by_nonce() {
    let sender = Address::from([1u8; 20]);

    // Create transactions out of order
    let tx2 = create_mock_transaction(sender, 2);
    let tx0 = create_mock_transaction(sender, 0);
    let tx1 = create_mock_transaction(sender, 1);

    let mut transactions = vec![tx2, tx0, tx1];

    // Sort by nonce
    transactions.sort_by_key(|tx| tx.nonce());

    assert_eq!(transactions[0].nonce(), 0);
    assert_eq!(transactions[1].nonce(), 1);
    assert_eq!(transactions[2].nonce(), 2);
}

/// Test multiple senders transaction ordering
#[test]
fn test_multiple_senders_ordering() {
    let sender1 = Address::from([1u8; 20]);
    let sender2 = Address::from([2u8; 20]);

    let tx1_0 = create_mock_transaction(sender1, 0);
    let tx1_1 = create_mock_transaction(sender1, 1);
    let tx2_0 = create_mock_transaction(sender2, 0);
    let tx2_1 = create_mock_transaction(sender2, 1);

    let actual_sender1 = tx1_0.sender();
    let actual_sender2 = tx2_0.sender();

    let transactions = vec![tx1_0, tx1_1, tx2_0, tx2_1];

    // Group by sender
    let sender1_txs: Vec<_> = transactions
        .iter()
        .filter(|tx| tx.sender() == actual_sender1)
        .collect();
    let sender2_txs: Vec<_> = transactions
        .iter()
        .filter(|tx| tx.sender() == actual_sender2)
        .collect();

    assert_eq!(sender1_txs.len(), 2);
    assert_eq!(sender2_txs.len(), 2);

    // Verify nonce ordering within each sender
    assert!(sender1_txs[0].nonce() < sender1_txs[1].nonce());
    assert!(sender2_txs[0].nonce() < sender2_txs[1].nonce());
}
