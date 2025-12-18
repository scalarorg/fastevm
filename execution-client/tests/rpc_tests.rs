//! Integration tests for RPC functionality
//!
//! These tests verify the RPC API definitions and data structures
//! used for communication between consensus and execution layers.

use alloy_network::{eip2718::Encodable2718, EthereumWallet, TransactionBuilder};
use alloy_primitives::{Address, Bytes, FixedBytes, U256};
use alloy_rpc_types_eth::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;
use rpc_shared_api::{BlockRef, CommitRef, CommittedSubDag, VerifiedBlock};
use secp256k1::SecretKey;
use sha2::{Digest, Sha256};

/// Create a test transaction as raw bytes
fn create_raw_transaction(sender: Address, nonce: u64) -> Vec<u8> {
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

        tx_envelope.encoded_2718()
    })
}

/// Test BlockRef structure
#[test]
fn test_block_ref_creation() {
    let block_ref = BlockRef {
        round: 10,
        digest: [1u8; 32],
    };

    assert_eq!(block_ref.round, 10);
    assert_eq!(block_ref.digest, [1u8; 32]);
}

/// Test BlockRef default
#[test]
fn test_block_ref_default() {
    let block_ref = BlockRef::default();

    assert_eq!(block_ref.round, 0);
    assert_eq!(block_ref.digest, [0u8; 32]);
}

/// Test CommitRef structure
#[test]
fn test_commit_ref_creation() {
    let commit_ref = CommitRef {
        round: 100,
        digest: [2u8; 32],
    };

    assert_eq!(commit_ref.round, 100);
    assert_eq!(commit_ref.digest, [2u8; 32]);
}

/// Test CommitRef default
#[test]
fn test_commit_ref_default() {
    let commit_ref = CommitRef::default();

    assert_eq!(commit_ref.round, 0);
    assert_eq!(commit_ref.digest, [0u8; 32]);
}

/// Test CommittedSubDag structure with empty blocks
#[test]
fn test_committed_subdag_empty() {
    let subdag = CommittedSubDag {
        leader: BlockRef::default(),
        blocks: Vec::new(),
        timestamp_ms: 1000,
        commit_ref: CommitRef {
            round: 1,
            digest: [0u8; 32],
        },
        reputation_scores_desc: Vec::new(),
    };

    assert!(subdag.blocks.is_empty());
    assert_eq!(subdag.timestamp_ms, 1000);
    assert_eq!(subdag.commit_ref.round, 1);
}

/// Test CommittedSubDag default
#[test]
fn test_committed_subdag_default() {
    let subdag = CommittedSubDag::default();

    assert!(subdag.blocks.is_empty());
    assert_eq!(subdag.timestamp_ms, 0);
    assert_eq!(subdag.commit_ref.round, 0);
}

/// Test flatten_transactions on empty subdag
#[test]
fn test_flatten_transactions_empty() {
    let subdag = CommittedSubDag::default();
    let transactions = subdag.flatten_transactions();

    assert!(transactions.is_empty());
}

/// Test raw transaction creation
#[test]
fn test_raw_transaction_creation() {
    let sender = Address::from([1u8; 20]);
    let raw_tx = create_raw_transaction(sender, 0);

    // Raw transaction should have reasonable size (EIP-1559 tx is typically 100-200 bytes)
    assert!(raw_tx.len() > 50);
    assert!(raw_tx.len() < 500);
}

/// Test raw transaction uniqueness by nonce
#[test]
fn test_raw_transaction_different_nonces() {
    let sender = Address::from([1u8; 20]);
    let tx1 = create_raw_transaction(sender, 0);
    let tx2 = create_raw_transaction(sender, 1);

    // Different nonces should produce different raw transactions
    assert_ne!(tx1, tx2);
}

/// Test raw transaction uniqueness by sender
#[test]
fn test_raw_transaction_different_senders() {
    let sender1 = Address::from([1u8; 20]);
    let sender2 = Address::from([2u8; 20]);
    let tx1 = create_raw_transaction(sender1, 0);
    let tx2 = create_raw_transaction(sender2, 0);

    // Different senders should produce different raw transactions
    assert_ne!(tx1, tx2);
}

/// Test Bytes type operations
#[test]
fn test_bytes_operations() {
    let data = vec![1u8, 2, 3, 4, 5];
    let bytes = Bytes::from(data.clone());

    assert_eq!(bytes.len(), 5);
    assert_eq!(&bytes[..], &data[..]);
}

/// Test Bytes from raw transaction
#[test]
fn test_bytes_from_raw_transaction() {
    let sender = Address::from([1u8; 20]);
    let raw_tx = create_raw_transaction(sender, 0);
    let bytes = Bytes::from(raw_tx);

    assert!(!bytes.is_empty());
}

/// Test batch of raw transactions
#[test]
fn test_batch_raw_transactions() {
    let sender = Address::from([1u8; 20]);

    let batch: Vec<Vec<u8>> = (0..10).map(|i| create_raw_transaction(sender, i)).collect();

    assert_eq!(batch.len(), 10);

    // All should be unique
    let unique: std::collections::HashSet<_> = batch.iter().collect();
    assert_eq!(unique.len(), 10);
}

/// Test CommittedSubDag with reputation scores
#[test]
fn test_committed_subdag_with_scores() {
    let subdag = CommittedSubDag {
        leader: BlockRef::default(),
        blocks: Vec::new(),
        timestamp_ms: 1000,
        commit_ref: CommitRef::default(),
        reputation_scores_desc: vec![(1, 100), (2, 90), (3, 80)],
    };

    assert_eq!(subdag.reputation_scores_desc.len(), 3);
    assert_eq!(subdag.reputation_scores_desc[0], (1, 100));
}

/// Test BlockRef comparison
#[test]
fn test_block_ref_comparison() {
    let ref1 = BlockRef {
        round: 1,
        digest: [0u8; 32],
    };
    let ref2 = BlockRef {
        round: 1,
        digest: [0u8; 32],
    };
    let ref3 = BlockRef {
        round: 2,
        digest: [0u8; 32],
    };

    // Same values should be equal
    assert_eq!(ref1.round, ref2.round);
    assert_eq!(ref1.digest, ref2.digest);

    // Different round
    assert_ne!(ref1.round, ref3.round);
}

/// Test CommitRef comparison
#[test]
fn test_commit_ref_comparison() {
    let ref1 = CommitRef {
        round: 100,
        digest: [1u8; 32],
    };
    let ref2 = CommitRef {
        round: 100,
        digest: [1u8; 32],
    };
    let ref3 = CommitRef {
        round: 101,
        digest: [1u8; 32],
    };

    assert_eq!(ref1.round, ref2.round);
    assert_eq!(ref1.digest, ref2.digest);
    assert_ne!(ref1.round, ref3.round);
}

/// Test timestamp handling
#[test]
fn test_timestamp_handling() {
    let timestamp_ms: u64 = 1_700_000_000_000; // November 2023 in milliseconds
    let timestamp_s = timestamp_ms / 1000;

    assert_eq!(timestamp_s, 1_700_000_000);

    // Convert back
    assert_eq!(timestamp_s * 1000, timestamp_ms);
}

/// Test round number handling
#[test]
fn test_round_number_handling() {
    // Test normal rounds
    let round1: u64 = 1;
    let round2: u64 = 1000;
    let round_max: u64 = u64::MAX;

    assert!(round1 < round2);
    assert!(round2 < round_max);

    // Test round incrementing
    let next_round = round1 + 1;
    assert_eq!(next_round, 2);
}

/// Test authority ID handling
#[test]
fn test_authority_id_handling() {
    let authority1: u64 = 0;
    let authority2: u64 = 1;
    let authority3: u64 = 3;

    // In a 4-node network, authorities are 0-3
    let authorities = vec![authority1, authority2, authority3];
    assert_eq!(authorities.len(), 3);
}

/// Test digest (hash) handling
#[test]
fn test_digest_handling() {
    let digest1 = [0u8; 32];
    let digest2 = [1u8; 32];
    let mut digest3 = [0u8; 32];
    digest3[0] = 1;

    assert_ne!(digest1, digest2);
    assert_ne!(digest1, digest3);
    assert_ne!(digest2, digest3);
}

/// Test multiple subdags ordering
#[test]
fn test_multiple_subdags_ordering() {
    let subdags: Vec<CommittedSubDag> = (1..=5)
        .map(|i| CommittedSubDag {
            leader: BlockRef::default(),
            blocks: Vec::new(),
            timestamp_ms: i * 1000,
            commit_ref: CommitRef {
                round: i,
                digest: [0u8; 32],
            },
            reputation_scores_desc: Vec::new(),
        })
        .collect();

    // Verify ordering
    for (i, subdag) in subdags.iter().enumerate() {
        assert_eq!(subdag.commit_ref.round, (i + 1) as u64);
        assert_eq!(subdag.timestamp_ms, (i + 1) as u64 * 1000);
    }
}

/// Test subdag batch processing order
#[test]
fn test_subdag_batch_order() {
    let mut subdags = Vec::new();

    // Create subdags out of order
    subdags.push(CommittedSubDag {
        leader: BlockRef::default(),
        blocks: Vec::new(),
        timestamp_ms: 3000,
        commit_ref: CommitRef {
            round: 3,
            digest: [0u8; 32],
        },
        reputation_scores_desc: Vec::new(),
    });
    subdags.push(CommittedSubDag {
        leader: BlockRef::default(),
        blocks: Vec::new(),
        timestamp_ms: 1000,
        commit_ref: CommitRef {
            round: 1,
            digest: [0u8; 32],
        },
        reputation_scores_desc: Vec::new(),
    });
    subdags.push(CommittedSubDag {
        leader: BlockRef::default(),
        blocks: Vec::new(),
        timestamp_ms: 2000,
        commit_ref: CommitRef {
            round: 2,
            digest: [0u8; 32],
        },
        reputation_scores_desc: Vec::new(),
    });

    // Sort by round
    subdags.sort_by_key(|s| s.commit_ref.round);

    assert_eq!(subdags[0].commit_ref.round, 1);
    assert_eq!(subdags[1].commit_ref.round, 2);
    assert_eq!(subdags[2].commit_ref.round, 3);
}
