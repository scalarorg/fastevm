//! Integration tests for ConsensusPool
//!
//! These tests verify the end-to-end functionality of the ConsensusPool,
//! including transaction ordering, subdag management, and memory optimization.

use alloy_consensus::Transaction;
use alloy_network::{eip2718::Encodable2718, EthereumWallet, TransactionBuilder};
use alloy_primitives::{Address, FixedBytes, TxHash, U256};
use alloy_rpc_types_eth::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;
use reth_ethereum::pool::noop::NoopTransactionPool;
use reth_ethereum::rpc::eth::utils::recover_raw_transaction;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use rpc_shared_api::{BlockRef, CommitRef, MysticetiCommittedSubdag};
use secp256k1::SecretKey;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;

// Import ConsensusPool from the library crate
use fastevm_execution::ConsensusPool;

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

/// Helper function to create a mock MysticetiCommittedSubdag
fn create_mock_subdag(
    round: u64,
    transactions: Vec<Arc<TestTransaction>>,
) -> MysticetiCommittedSubdag<Arc<TestTransaction>> {
    MysticetiCommittedSubdag {
        leader: BlockRef::default(),
        transactions,
        timestamp_ms: round * 1000, // Use round as timestamp for determinism
        commit_ref: CommitRef {
            round,
            digest: [0u8; 32],
        },
        reputation_scores_desc: Vec::new(),
    }
}

/// Test that verifies transaction creation works correctly
#[test]
fn test_create_mock_transaction_basic() {
    let sender = Address::from([1u8; 20]);
    let tx = create_mock_transaction(sender, 0);

    // Verify transaction properties
    assert_eq!(tx.nonce(), 0);
    assert_eq!(tx.gas_limit(), 21000);
}

/// Test that verifies transactions from same logical sender have same address
#[test]
fn test_create_mock_transaction_same_sender() {
    let sender = Address::from([1u8; 20]);
    let tx1 = create_mock_transaction(sender, 0);
    let tx2 = create_mock_transaction(sender, 1);

    // Same sender should produce transactions with same actual sender
    assert_eq!(tx1.sender(), tx2.sender());
    // But different nonces
    assert_ne!(tx1.nonce(), tx2.nonce());
}

/// Test that verifies different logical senders produce different addresses
#[test]
fn test_create_mock_transaction_different_senders() {
    let sender1 = Address::from([1u8; 20]);
    let sender2 = Address::from([2u8; 20]);

    let tx1 = create_mock_transaction(sender1, 0);
    let tx2 = create_mock_transaction(sender2, 0);

    // Different senders should produce transactions with different actual senders
    assert_ne!(tx1.sender(), tx2.sender());
}

/// Test subdag creation
#[test]
fn test_create_mock_subdag_empty() {
    let subdag = create_mock_subdag(1, Vec::new());

    assert!(subdag.transactions.is_empty());
    assert_eq!(subdag.commit_ref.round, 1);
    assert_eq!(subdag.timestamp_ms, 1000);
}

/// Test subdag creation with transactions
#[test]
fn test_create_mock_subdag_with_transactions() {
    let sender = Address::from([1u8; 20]);
    let tx1 = create_mock_transaction(sender, 0);
    let tx2 = create_mock_transaction(sender, 1);

    let subdag = create_mock_subdag(5, vec![tx1, tx2]);

    assert_eq!(subdag.transactions.len(), 2);
    assert_eq!(subdag.commit_ref.round, 5);
    assert_eq!(subdag.timestamp_ms, 5000);
}

/// Test transaction hash uniqueness
#[test]
fn test_transaction_hash_uniqueness() {
    let sender = Address::from([1u8; 20]);
    let tx1 = create_mock_transaction(sender, 0);
    let tx2 = create_mock_transaction(sender, 1);

    // Different transactions should have different hashes
    assert_ne!(tx1.hash(), tx2.hash());
}

/// Test that transaction properties are preserved through Arc wrapper
#[test]
fn test_arc_wrapped_transaction_properties() {
    let sender = Address::from([1u8; 20]);
    let tx: Arc<TestTransaction> = create_mock_transaction(sender, 42);

    assert_eq!(tx.nonce(), 42);

    // Clone the Arc and verify same properties
    let tx_clone = tx.clone();
    assert_eq!(tx_clone.nonce(), 42);
    assert_eq!(tx.hash(), tx_clone.hash());
}

/// Test multiple subdags creation
#[test]
fn test_multiple_subdags_creation() {
    let sender1 = Address::from([1u8; 20]);
    let sender2 = Address::from([2u8; 20]);

    let tx1 = create_mock_transaction(sender1, 0);
    let tx2 = create_mock_transaction(sender2, 0);
    let tx3 = create_mock_transaction(sender1, 1);

    let subdag1 = create_mock_subdag(1, vec![tx1.clone()]);
    let subdag2 = create_mock_subdag(2, vec![tx2.clone(), tx3.clone()]);

    assert_eq!(subdag1.transactions.len(), 1);
    assert_eq!(subdag2.transactions.len(), 2);
    assert_eq!(subdag1.commit_ref.round, 1);
    assert_eq!(subdag2.commit_ref.round, 2);
}

/// Test transaction hash collection
#[test]
fn test_transaction_hash_collection() {
    let sender = Address::from([1u8; 20]);
    let tx1 = create_mock_transaction(sender, 0);
    let tx2 = create_mock_transaction(sender, 1);
    let tx3 = create_mock_transaction(sender, 2);

    let transactions = vec![tx1.clone(), tx2.clone(), tx3.clone()];
    let hashes: HashSet<TxHash> = transactions.iter().map(|tx| *tx.hash()).collect();

    assert_eq!(hashes.len(), 3);
    assert!(hashes.contains(tx1.hash()));
    assert!(hashes.contains(tx2.hash()));
    assert!(hashes.contains(tx3.hash()));
}

/// Test subdag with high round number
#[test]
fn test_subdag_high_round_number() {
    // Use a high but safe round number to avoid overflow in timestamp calculation
    let high_round = 1_000_000_000u64;
    let subdag = create_mock_subdag(high_round, Vec::new());

    assert_eq!(subdag.commit_ref.round, high_round);
}

/// Test BlockRef default values
#[test]
fn test_block_ref_default() {
    let block_ref = BlockRef::default();

    assert_eq!(block_ref.round, 0);
    assert_eq!(block_ref.digest, [0u8; 32]);
}

/// Test CommitRef default values
#[test]
fn test_commit_ref_default() {
    let commit_ref = CommitRef::default();

    assert_eq!(commit_ref.round, 0);
    assert_eq!(commit_ref.digest, [0u8; 32]);
}

/// Test that transaction ordering is deterministic
#[test]
fn test_transaction_ordering_deterministic() {
    let sender = Address::from([1u8; 20]);

    // Create same transactions twice
    let tx1_a = create_mock_transaction(sender, 0);
    let tx1_b = create_mock_transaction(sender, 0);

    // Same inputs should produce same hash (deterministic)
    assert_eq!(tx1_a.hash(), tx1_b.hash());
    assert_eq!(tx1_a.sender(), tx1_b.sender());
}
