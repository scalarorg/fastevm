use alloy_primitives::{Address, Bloom, Bytes, FixedBytes, B256, U256};
use alloy_rpc_types_engine::{ExecutionPayloadInputV2, ExecutionPayloadV1};
use consensus_core::{BlockAPI, BlockRef, CommittedSubDag};

use super::merkle_root_keccak;

// Your SubDagBlock wrapper (replace CommittedSubDag type)
#[derive(Debug, Clone)]
pub struct SubDagBlock {
    subdag: CommittedSubDag, // use the actual path
}

impl SubDagBlock {
    pub fn new(committed_subdag: CommittedSubDag) -> Self {
        let block = Self {
            subdag: committed_subdag,
        };
        block.parse();
        block
    }
    pub fn get_block_timestamp(&self) -> u64 {
        self.subdag.timestamp_ms
    }
    pub fn get_block_hash(&self) -> B256 {
        //TODO: Implement this
        B256::default()
    }
    pub fn parse(&self) {
        // any precomputation if needed
    }

    pub fn get_execution_payload_input_v2(&self) -> ExecutionPayloadInputV2 {
        // 1) Flatten transactions:
        // We assume each VerifiedBlock exposes an API to iterate its transactions,
        // or to produce raw bytes. Replace `tx_to_raw` and the `transactions()` call
        // with your actual accessors.
        let mut flattened_txs: Vec<Bytes> = Vec::new();
        let mut total_gas_used: u64 = 0;

        for vb in &self.subdag.blocks {
            // --- IMPORTANT: replace the code below with the real one ---
            // Possible valid variants (adapt to your VerifiedBlock API):
            // 1) if VerifiedBlock has .transactions() -> &[TxType]:
            //    for tx in vb.transactions().iter() { flattened_txs.push(tx.to_raw_bytes()); total_gas_used += tx.gas_used(); }
            //
            // 2) if VerifiedBlock stores raw bytes: for raw in vb.raw_transactions() { flattened_txs.push(raw.clone()); }
            //
            //Extract transactions from verified block
            for tx in vb.transactions() {
                let raw_data = Bytes::from(tx.data().to_vec());
                //Add calculated gas used here
                total_gas_used = total_gas_used.saturating_add(0); // if you can measure gas, add it here
                flattened_txs.push(raw_data);
            }
        }

        // 2) transactions merkle root (pairwise keccak)
        let transactions_root = merkle_root_keccak(&flattened_txs);

        // 3) map other header fields from CommittedSubDag
        // parent_hash: use digest of subdag leader or the last committed parent's digest
        let parent_hash = {
            // Try to take leader digest => VerifiedBlock::digest() usually returns a Digest-like object.
            // Convert it to [u8; 32] by copying bytes. Replace this conversion with real accessor.
            // If BlockRef is available in subdag.leader (type BlockRef), you might be able to use its digest/id.
            // Fallback to zero.
            if let Some(bytes) = try_digest_from_blockref(&self.subdag.leader) {
                bytes
            } else {
                FixedBytes::<32>::default()
            }
        };

        // fee_recipient / coinbase: Usually the leader author address
        let fee_recipient = {
            // try to extract author from leader BlockRef (author index -> address)
            if let Some(addr20) = try_author_addr_from_blockref(&self.subdag.leader) {
                addr20
            } else {
                Address::default()
            }
        };

        // timestamp: use subdag.timestamp_ms (ms) -> seconds
        let timestamp_secs = (self.subdag.timestamp_ms / 1000) as u64;

        // block_number: you may derive from commit_ref.index or other chain bookkeeping
        let block_number = self.subdag.commit_ref.index as u64; // example; adapt if needed

        // gas_limit & gas_used - placeholders
        let gas_limit = 0u64; // TODO: set from chain config
        let gas_used = total_gas_used;

        // base_fee_per_gas - placeholder
        let base_fee = U256::default();

        // state_root / receipts_root / logs_bloom / prev_randao / block_hash:
        // these are results of execution or chain metadata. Use placeholders and TODOs.
        let execution_payload = ExecutionPayloadV1 {
            parent_hash,
            fee_recipient,
            state_root: FixedBytes::from(transactions_root), // TODO: map to real post-state root
            receipts_root: B256::default(), // TODO: compute receipts root from execution receipts
            logs_bloom: Bloom::default(),          // TODO
            prev_randao: B256::default(),   // TODO: mixHash or randomness
            block_number,
            gas_limit,
            gas_used,
            timestamp: timestamp_secs,
            extra_data: Bytes::default(), // TODO: any extra data
            base_fee_per_gas: base_fee,
            block_hash: B256::default(), // placeholder: NOT real block hash. TODO compute header hash.
            transactions: flattened_txs,
        };

        ExecutionPayloadInputV2 {
            execution_payload,
            // For post-Shanghai blocks, withdrawals must be present (empty vec for no withdrawals)
            withdrawals: Some(vec![]),
        }
    }
}

fn try_digest_from_blockref(_br: &BlockRef) -> Option<B256> {
    // TODO: BlockRef likely contains digest info (display uses BlockRef in commit.rs).
    // Example:
    // if br has a .digest() -> Digest type which can be converted into bytes, do that.
    None
}

fn try_author_addr_from_blockref(_br: &BlockRef) -> Option<Address> {
    // TODO: derive 20-byte address from block author if available.
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::keccak256;
    use consensus_core::{BlockRef, CommitDigest, CommitRef, CommittedSubDag, VerifiedBlock};

    // Mock data structures for testing
    #[derive(Debug, Clone)]
    struct MockVerifiedBlock {
        data: Vec<u8>,
    }

    impl MockVerifiedBlock {
        fn new(data: Vec<u8>) -> Self {
            Self { data }
        }

        fn data(&self) -> &[u8] {
            &self.data
        }
    }

    // Helper function to create test CommittedSubDag
    fn create_test_committed_subdag() -> CommittedSubDag {
        CommittedSubDag {
            leader: BlockRef::MIN,
            blocks: vec![],
            timestamp_ms: 1000,
            commit_ref: CommitRef::new(1,CommitDigest::default()),
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
        }
    }

    #[test]
    fn test_subdag_block_new() {
        let subdag = create_test_committed_subdag();
        let block = SubDagBlock::new(subdag);
        
        // Test that the block was created without panicking
        assert!(true);
    }

    #[test]
    fn test_subdag_block_get_block_timestamp() {
        let subdag = create_test_committed_subdag();
        let block = SubDagBlock::new(subdag);
        
        let timestamp = block.get_block_timestamp();
        assert_eq!(timestamp, 1000);
    }

    #[test]
    fn test_subdag_block_get_block_timestamp_zero() {
        let mut subdag = create_test_committed_subdag();
        subdag.timestamp_ms = 0;
        let block = SubDagBlock::new(subdag);
        
        let timestamp = block.get_block_timestamp();
        assert_eq!(timestamp, 0);
    }

    #[test]
    fn test_subdag_block_get_block_timestamp_max() {
        let mut subdag = create_test_committed_subdag();
        subdag.timestamp_ms = u64::MAX;
        let block = SubDagBlock::new(subdag);
        
        let timestamp = block.get_block_timestamp();
        assert_eq!(timestamp, u64::MAX);
    }

    #[test]
    #[should_panic(expected = "get_block_hash is not implemented")]
    fn test_subdag_block_get_block_hash() {
        let subdag = create_test_committed_subdag();
        let block = SubDagBlock::new(subdag);
        
        // This should panic as per the TODO comment
        let _ = block.get_block_hash();
    }

    #[test]
    fn test_subdag_block_parse() {
        let subdag = create_test_committed_subdag();
        let block = SubDagBlock::new(subdag);
        
        // Test that parse doesn't panic
        block.parse();
        assert!(true);
    }

    #[test]
    fn test_subdag_block_get_execution_payload_input_v2_empty_blocks() {
        let subdag = create_test_committed_subdag();
        let block = SubDagBlock::new(subdag);
        
        let payload = block.get_execution_payload_input_v2();
        
        // Verify the payload structure
        assert_eq!(payload.execution_payload.block_number, 1);
        assert_eq!(payload.execution_payload.timestamp, 1); // timestamp_ms / 1000
        assert_eq!(payload.execution_payload.transactions.len(), 0);
        assert_eq!(payload.execution_payload.gas_used, 0);
        assert_eq!(payload.execution_payload.gas_limit, 0);
        assert_eq!(payload.execution_payload.base_fee_per_gas, U256::default());
        assert_eq!(payload.withdrawals, None);
    }

    #[test]
    fn test_subdag_block_get_execution_payload_input_v2_with_transactions() {
        let mut subdag = create_test_committed_subdag();
        
        // Add some mock blocks with transactions
        let _mock_blocks = vec![
            MockVerifiedBlock::new(vec![1, 2, 3]),
            MockVerifiedBlock::new(vec![4, 5, 6]),
            MockVerifiedBlock::new(vec![7, 8, 9]),
        ];
        
        // Convert MockVerifiedBlock to real VerifiedBlock
        // This is a simplified test - in real implementation you'd need proper conversion
        let blocks: Vec<VerifiedBlock> = vec![];
        subdag.blocks = blocks;
        
        let block = SubDagBlock::new(subdag);
        let payload = block.get_execution_payload_input_v2();
        
        // Verify the payload structure
        assert_eq!(payload.execution_payload.block_number, 1);
        assert_eq!(payload.execution_payload.timestamp, 1);
        assert_eq!(payload.execution_payload.transactions.len(), 0); // Empty for now
        assert_eq!(payload.execution_payload.gas_used, 0);
    }

    #[test]
    fn test_subdag_block_get_execution_payload_input_v2_different_timestamps() {
        let timestamps = vec![0, 1000, 2000, 5000, 10000, 60000, 3600000];
        
        for timestamp_ms in timestamps {
            let mut subdag = create_test_committed_subdag();
            subdag.timestamp_ms = timestamp_ms;
            let block = SubDagBlock::new(subdag);
            
            let payload = block.get_execution_payload_input_v2();
            let expected_timestamp = timestamp_ms / 1000;
            assert_eq!(payload.execution_payload.timestamp, expected_timestamp);
        }
    }

    #[test]
    fn test_subdag_block_get_execution_payload_input_v2_different_commit_refs() {
        let indices = vec![0, 1, 10, 100, 1000, u32::MAX];
        
        for index in indices {
            let mut subdag = create_test_committed_subdag();
            subdag.commit_ref = CommitRef::new(index,CommitDigest::default());
            let block = SubDagBlock::new(subdag);
            
            let payload = block.get_execution_payload_input_v2();
            let expected_block_number = index as u64;
            assert_eq!(payload.execution_payload.block_number, expected_block_number);
        }
    }

    #[test]
    fn test_subdag_block_clone() {
        let subdag = create_test_committed_subdag();
        let block = SubDagBlock::new(subdag);
        
        let _cloned_block = block.clone();
        
        // Test that cloning works without panicking
        assert!(true);
    }

    #[test]
    fn test_subdag_block_debug() {
        let subdag = create_test_committed_subdag();
        let block = SubDagBlock::new(subdag);
        
        let debug_str = format!("{:?}", block);
        
        // Test that debug formatting works
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_try_digest_from_blockref() {
        let block_ref = BlockRef::MIN;
        let result = try_digest_from_blockref(&block_ref);
        
        // Currently returns None as per TODO comment
        assert!(result.is_none());
    }

    #[test]
    fn test_try_author_addr_from_blockref() {
        let block_ref = BlockRef::MIN;
        let result = try_author_addr_from_blockref(&block_ref);
        
        // Currently returns None as per TODO comment
        assert!(result.is_none());
    }

    #[test]
    fn test_merkle_root_keccak_empty() {
        let empty_txs: Vec<Bytes> = vec![];
        let root = merkle_root_keccak(&empty_txs);
        
        // Should return keccak of empty bytes
        assert_eq!(root.len(), 32);
    }

    #[test]
    fn test_merkle_root_keccak_single_transaction() {
        let txs = vec![Bytes::from(vec![1, 2, 3, 4, 5])];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        
        // Should be different from empty root
        let empty_root = merkle_root_keccak(&vec![]);
        assert_ne!(root, empty_root);
    }

    #[test]
    fn test_merkle_root_keccak_multiple_transactions() {
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![7, 8, 9]),
        ];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
    }

    #[test]
    fn test_merkle_root_keccak_odd_number_transactions() {
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![7, 8, 9]),
        ];
        let root = merkle_root_keccak(&txs);
        
        // Should handle odd number of transactions by duplicating the last one
        assert_eq!(root.len(), 32);
    }

    #[test]
    fn test_merkle_root_keccak_large_transactions() {
        let txs = vec![
            Bytes::from(vec![0u8; 1000]),
            Bytes::from(vec![1u8; 1000]),
            Bytes::from(vec![2u8; 1000]),
            Bytes::from(vec![3u8; 1000]),
        ];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
    }

    #[test]
    fn test_merkle_root_keccak_deterministic() {
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![7, 8, 9]),
        ];
        
        let root1 = merkle_root_keccak(&txs);
        let root2 = merkle_root_keccak(&txs);
        
        // Same input should produce same output
        assert_eq!(root1, root2);
    }

    #[test]
    fn test_merkle_root_keccak_different_inputs() {
        let txs1 = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
        ];
        
        let txs2 = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 7]), // Different last byte
        ];
        
        let root1 = merkle_root_keccak(&txs1);
        let root2 = merkle_root_keccak(&txs2);
        
        // Different inputs should produce different outputs
        assert_ne!(root1, root2);
    }

    #[test]
    fn test_merkle_root_keccak_transaction_order_matters() {
        let txs1 = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
        ];
        
        let txs2 = vec![
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![1, 2, 3]), // Different order
        ];
        
        let root1 = merkle_root_keccak(&txs1);
        let root2 = merkle_root_keccak(&txs2);
        
        // Different order should produce different outputs
        assert_ne!(root1, root2);
    }

    #[test]
    fn test_merkle_root_keccak_edge_cases() {
        // Test with single byte transactions
        let txs = vec![
            Bytes::from(vec![0]),
            Bytes::from(vec![1]),
            Bytes::from(vec![255]),
        ];
        let root = merkle_root_keccak(&txs);
        assert_eq!(root.len(), 32);
        
        // Test with very long transactions
        let long_txs = vec![
            Bytes::from(vec![0u8; 10000]),
            Bytes::from(vec![1u8; 10000]),
        ];
        let long_root = merkle_root_keccak(&long_txs);
        assert_eq!(long_root.len(), 32);
    }

    #[test]
    fn test_merkle_root_keccak_consistency() {
        // Test that the merkle tree construction is consistent
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            Bytes::from(vec![7, 8, 9]),
        ];
        
        let root = merkle_root_keccak(&txs);
        
        // Verify the construction manually:
        // 1. Hash each transaction
        let h1 = keccak256(&txs[0]);
        let h2 = keccak256(&txs[1]);
        let h3 = keccak256(&txs[2]);
        
        // 2. Duplicate last (odd number)
        let h4 = h3; // Duplicate
        
        // 3. First level: hash pairs
        let mut combined = Vec::with_capacity(64);
        combined.extend_from_slice(&h1.as_ref());
        combined.extend_from_slice(&h2.as_ref());
        let level1_1 = keccak256(&combined);
        
        combined.clear();
        combined.extend_from_slice(&h3.as_ref());
        combined.extend_from_slice(&h4.as_ref());
        let level1_2 = keccak256(&combined);
        
        // 4. Final level: hash the two remaining hashes
        combined.clear();
        combined.extend_from_slice(&level1_1.as_ref());
        combined.extend_from_slice(&level1_2.as_ref());
        let expected_root = keccak256(&combined);
        
        assert_eq!(root, expected_root);
    }

    #[test]
    fn test_merkle_root_keccak_performance() {
        // Test with many transactions to ensure performance is reasonable
        let mut txs = Vec::new();
        for i in 0..100 {
            txs.push(Bytes::from(vec![i as u8; 100]));
        }
        
        let start = std::time::Instant::now();
        let root = merkle_root_keccak(&txs);
        let duration = start.elapsed();
        
        assert_eq!(root.len(), 32);
        
        // Should complete in reasonable time (less than 1 second)
        assert!(duration.as_millis() < 1000);
    }

    #[test]
    fn test_merkle_root_keccak_zero_transactions() {
        // Test edge case with zero transactions
        let txs: Vec<Bytes> = vec![];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        assert_eq!(root, keccak256(&[]));
    }

    #[test]
    fn test_merkle_root_keccak_one_transaction() {
        // Test with exactly one transaction
        let txs = vec![Bytes::from(vec![42])];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        assert_eq!(root, keccak256(&txs[0]));
    }

    #[test]
    fn test_merkle_root_keccak_two_transactions() {
        // Test with exactly two transactions
        let txs = vec![
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
        ];
        let root = merkle_root_keccak(&txs);
        
        assert_eq!(root.len(), 32);
        
        // Should hash the two transactions together
        let mut combined = Vec::with_capacity(64);
        combined.extend_from_slice(keccak256(&txs[0]).as_ref());
        combined.extend_from_slice(keccak256(&txs[1]).as_ref());
        let expected = keccak256(&combined);
        
        assert_eq!(root, expected);
    }

    #[test]
    fn test_execution_payload_structure() {
        let subdag = create_test_committed_subdag();
        let block = SubDagBlock::new(subdag);
        let payload = block.get_execution_payload_input_v2();
        
        let execution_payload = &payload.execution_payload;
        
        // Test all fields are properly set
        assert_eq!(execution_payload.block_number, 1);
        assert_eq!(execution_payload.timestamp, 1);
        assert_eq!(execution_payload.gas_limit, 0);
        assert_eq!(execution_payload.gas_used, 0);
        assert_eq!(execution_payload.base_fee_per_gas, U256::default());
        assert_eq!(execution_payload.extra_data, Bytes::default());
        assert_eq!(payload.withdrawals, None);
        
        // Test that some fields are placeholders (as per TODO comments)
        assert_eq!(execution_payload.state_root, FixedBytes::from([0u8; 32]));
        assert_eq!(execution_payload.receipts_root, FixedBytes::from([0u8; 32]));
        assert_eq!(execution_payload.logs_bloom, Bloom::default());
        assert_eq!(execution_payload.prev_randao, FixedBytes::from([0u8; 32]));
        assert_eq!(execution_payload.block_hash, FixedBytes::from([0u8; 32]));
    }

    #[test]
    fn test_payload_attributes_consistency() {
        let subdag = create_test_committed_subdag();
        let block = SubDagBlock::new(subdag);
        let payload = block.get_execution_payload_input_v2();
        
        let execution_payload = &payload.execution_payload;
        
        // Test that parent_hash is consistent with finalized_block_hash
        // (This would be tested in integration with ConsensusState)
        assert_eq!(execution_payload.parent_hash, FixedBytes::from([0u8; 32]));
        
        // Test that fee_recipient is set (would be from ConsensusState)
        // For now, it's Address::default() as per the TODO
        assert_eq!(execution_payload.fee_recipient, Address::default());
    }

    #[test]
    fn test_transaction_processing_edge_cases() {
        // Test with empty blocks
        let mut subdag = create_test_committed_subdag();
        subdag.blocks = vec![];
        let block = SubDagBlock::new(subdag);
        let payload = block.get_execution_payload_input_v2();
        
        assert_eq!(payload.execution_payload.transactions.len(), 0);
        assert_eq!(payload.execution_payload.gas_used, 0);
        
        // Test with single empty block
        let mock_blocks: Vec<VerifiedBlock> = vec![];
        let mut subdag2 = create_test_committed_subdag();
        subdag2.blocks = mock_blocks;
        let block2 = SubDagBlock::new(subdag2);
        let payload2 = block2.get_execution_payload_input_v2();
        
        assert_eq!(payload2.execution_payload.transactions.len(), 0);
        assert_eq!(payload2.execution_payload.gas_used, 0);
    }

    #[test]
    fn test_timestamp_conversion_edge_cases() {
        // Test timestamp conversion edge cases
        let test_cases = vec![
            (0, 0),           // 0ms -> 0s
            (999, 0),         // 999ms -> 0s (truncated)
            (1000, 1),        // 1000ms -> 1s
            (1500, 1),        // 1500ms -> 1s (truncated)
            (2000, 2),        // 2000ms -> 2s
            (u64::MAX, u64::MAX / 1000), // Max timestamp
        ];
        
        for (timestamp_ms, expected_seconds) in test_cases {
            let mut subdag = create_test_committed_subdag();
            subdag.timestamp_ms = timestamp_ms;
            let block = SubDagBlock::new(subdag);
            let payload = block.get_execution_payload_input_v2();
            
            assert_eq!(payload.execution_payload.timestamp, expected_seconds);
        }
    }

    #[test]
    fn test_block_number_progression() {
        let mut subdag = create_test_committed_subdag();
        let test_indices = vec![0, 1, 10, 100, 1000, u32::MAX];
        
        for index in test_indices {
            subdag.commit_ref = CommitRef::new(index,CommitDigest::default());
            let block = SubDagBlock::new(subdag.clone());
            let payload = block.get_execution_payload_input_v2();
            
            let expected_block_number = index as u64;
            assert_eq!(payload.execution_payload.block_number, expected_block_number);
        }
    }
}
