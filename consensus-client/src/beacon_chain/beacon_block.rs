use alloy_primitives::{Address, Bloom, Bytes};
use alloy_rpc_types_engine::{CancunPayloadFields, ExecutionData, ExecutionPayload, ExecutionPayloadSidecar, ExecutionPayloadV3, PayloadError};
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use tracing::info;
use tree_hash::TreeHash;
use tree_hash_derive::TreeHash;
use tree_hash::Hash256;
use consensus_core::{CommittedSubDag, BlockAPI};
use reth_ethereum_primitives::TransactionSigned;
use reth_primitives_traits::{Block as _, SealedBlock, SignedTransaction};

/// Beacon block header containing essential block information
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, TreeHash, Derivative,
)]
#[derivative(PartialEq, Eq, Hash)]
pub struct BeaconBlockHeader {
    /// block number when this block was proposed
    pub block_number: u64,
    /// Index of the proposer
    pub proposer_index: u32,
    /// Hash of the parent block
    pub parent_root: Hash256,
    /// Hash of the state root after processing this block
    pub state_root: Hash256,
    /// Hash of the block body
    pub body_root: Hash256,
}

impl BeaconBlockHeader {
    /// Create a new block header
    pub fn new(
        block_number: u64,
        proposer_index: u32,
        parent_root: Hash256,
        state_root: Hash256,
        body_root: Hash256,
    ) -> Self {
        Self {
            block_number,
            proposer_index,
            parent_root,
            state_root,
            body_root,
        }
    }

    /// Get the canonical root of this block header
    pub fn canonical_root(&self) -> Hash256 {
        self.tree_hash_root()
    }
}

#[derive(Debug)]
pub struct SubDagBlock {
    fee_recipient: Address,
    subdag: CommittedSubDag,
}

impl SubDagBlock {
    pub fn new(
        fee_recipient: Address,
        subdag: CommittedSubDag,
    ) -> Self {
        Self { fee_recipient, subdag }
    }
    
    pub fn create_execution_data_v3(&self, parent_beacon_block_header: &mut BeaconBlockHeader,) -> Result<ExecutionData, PayloadError> {
        // Convert timestamp from milliseconds to seconds (Ethereum standard)
        let timestamp_secs = (self.subdag.timestamp_ms / 1000) as u64;
        
        // For now, create an empty execution payload since transaction extraction is complex
        // In a production environment, this would extract actual transactions from the consensus blocks
        let transactions: Vec<Bytes> = self.flatten_transactions();
        info!("[SubDagBlock::get_execution_payload_v3] transactions: {:?}", transactions);
        let total_gas_used = 0u64;
        
        // Calculate state root from transactions (simplified - in production this would be post-execution state)
        let state_root = self.calculate_state_root(transactions.as_slice()  );
        
        // Calculate receipts root (simplified - in production this would be from execution receipts)
        let receipts_root = self.calculate_receipts_root(transactions.as_slice());
        
        // Calculate logs bloom (simplified - in production this would be from execution logs)
        let logs_bloom = self.calculate_logs_bloom(transactions.as_slice());
        
        // Calculate prev_randao (mixHash) from subdag randomness
        let prev_randao = self.calculate_prev_randao();
        
        // Calculate fee recipient from leader authority
        let fee_recipient = self.fee_recipient.clone();
        
        // Calculate block number from commit reference
        let block_number = parent_beacon_block_header.block_number + 1;
        
        // Set gas limit to standard Ethereum block gas limit
        let gas_limit = 30_000_000u64;
        
        // Calculate base fee per gas (EIP-1559)
        let base_fee_per_gas = self.calculate_base_fee_per_gas();
        
        // Calculate blob gas fields for EIP-4844 (Cancun upgrade)
        let (blob_gas_used, excess_blob_gas) = self.calculate_blob_gas_fields();
        
        // Create extra data with subdag information
        let extra_data = self.create_extra_data();
        
        let execution_payload_v1 = alloy_rpc_types_engine::ExecutionPayloadV1 {
            parent_hash: parent_beacon_block_header.parent_root.clone(),
                    fee_recipient,
                    state_root,
                    receipts_root,
                    logs_bloom,
                    prev_randao,
                    block_number,
                    gas_limit,
                    gas_used: total_gas_used,
                    timestamp: timestamp_secs,
                    extra_data,
                    base_fee_per_gas,
                    block_hash: Hash256::ZERO,
                    transactions,
        };
        let mut execution_payload_v3 = ExecutionPayloadV3 {
            payload_inner: alloy_rpc_types_engine::ExecutionPayloadV2 {
                payload_inner: execution_payload_v1,
                withdrawals: Vec::new(), // Post-Shanghai blocks require withdrawals (empty for no withdrawals)
            },
            blob_gas_used,
            excess_blob_gas,
        };
        let execution_payload: ExecutionPayload = ExecutionPayload::V3(execution_payload_v3.clone());
        let sidecar = ExecutionPayloadSidecar::v3(CancunPayloadFields {
            versioned_hashes: Vec::new(),
            parent_beacon_block_root: parent_beacon_block_header.canonical_root(),
        });
        // First parse the block
        let sealed_block = execution_payload.try_into_block_with_sidecar::<TransactionSigned>(&sidecar)?.seal_slow();
        let block_hash = sealed_block.hash();
        execution_payload_v3.payload_inner.payload_inner.block_hash = block_hash;
        info!("block_hash: {:?}", block_hash);
        //Update header
        parent_beacon_block_header.block_number += 1;
        parent_beacon_block_header.parent_root = block_hash;
        Ok(ExecutionData {
            payload: execution_payload_v3.into(),
            sidecar,
        })
    }
    
    /// Calculate state root from transactions
    fn calculate_state_root(&self, transactions: &[Bytes]) -> Hash256 {
        if transactions.is_empty() {
            return Hash256::ZERO;
        }
        
        // Create a simple merkle-like hash from transactions
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        for tx in transactions {
            tx.hash(&mut hasher);
        }
        
        let hash_value = hasher.finish();
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&hash_value.to_le_bytes());
        
        Hash256::from(bytes)
    }
    
    /// Calculate receipts root from transactions
    fn calculate_receipts_root(&self, transactions: &[Bytes]) -> Hash256 {
        if transactions.is_empty() {
            return Hash256::ZERO;
        }
        
        // In a real implementation, this would be calculated from execution receipts
        // For now, create a deterministic hash
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        transactions.len().hash(&mut hasher);
        
        let hash_value = hasher.finish();
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&hash_value.to_le_bytes());
        
        Hash256::from(bytes)
    }
    
    /// Calculate logs bloom from transactions
    fn calculate_logs_bloom(&self, transactions: &[Bytes]) -> Bloom {
        if transactions.is_empty() {
            return Bloom::default();
        }
        
        // In a real implementation, this would be calculated from execution logs
        // For now, create a simple bloom filter
        let mut bloom = Bloom::default();
        
        // Set some bits based on transaction count
        let tx_count = transactions.len() as u64;
        for i in 0..8 {
            let bit_index = (tx_count + i) % 2048;
            let byte_index = (bit_index / 8) as usize;
            let bit_offset = (bit_index % 8) as u8;
            if byte_index < 256 {
                bloom.0[byte_index] |= 1 << bit_offset;
            }
        }
        
        bloom
    }
    
    /// Calculate prev_randao (mixHash) from subdag randomness
    fn calculate_prev_randao(&self) -> Hash256 {
        // In a real implementation, this would be the RANDAO reveal from the beacon chain
        // For now, create a deterministic hash from timestamp and round
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        self.subdag.timestamp_ms.hash(&mut hasher);
        self.subdag.leader.round.hash(&mut hasher);
        
        let hash_value = hasher.finish();
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&hash_value.to_le_bytes());
        
        Hash256::from(bytes)
    }
    
    /// Calculate base fee per gas (EIP-1559)
    fn calculate_base_fee_per_gas(&self) -> alloy_primitives::U256 {
        // In a real implementation, this would be calculated based on network congestion
        // For now, use a reasonable default base fee
        alloy_primitives::U256::from(1_000_000_000u64) // 1 gwei
    }
    
    /// Calculate blob gas fields for EIP-4844
    fn calculate_blob_gas_fields(&self) -> (u64, u64) {
        // In a real implementation, these would be calculated based on blob transactions
        // For now, return zeros (no blob gas used)
        (0, 0)
    }
    
    /// Create extra data with subdag information
    fn create_extra_data(&self) -> Bytes {
        // Include subdag metadata in extra data
        let mut extra_data = Vec::new();
        
        // Add subdag round
        extra_data.extend_from_slice(&self.subdag.leader.round.to_le_bytes());
        
        // Add timestamp
        extra_data.extend_from_slice(&self.subdag.timestamp_ms.to_le_bytes());
        
        // Add commit reference index
        extra_data.extend_from_slice(&self.subdag.commit_ref.index.to_le_bytes());
        
        // Add a marker to identify this as a FastEVM block
        extra_data.extend_from_slice(b"FastEVM");
        
        Bytes::from(extra_data)
    }
    
    pub fn flatten_transactions(&self) -> Vec<Bytes> {
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
        flattened_txs
    }
}