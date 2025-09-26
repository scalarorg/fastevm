use std::fmt::Debug;

use consensus_config::AuthorityIndex;
use consensus_core::{
    BlockAPI as ConsensusVerifiedBlock, BlockRef, BlockTimestampMs, CommitRef,
    CommittedSubDag as ConsensusCommittedSubDag,
};
use serde::{Deserialize, Serialize};

use crate::{BlockDigest, SignedBlock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedBlock {
    pub block: SignedBlock,
    pub digest: BlockDigest,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommittedSubDag {
    pub leader: BlockRef,
    pub blocks: Vec<VerifiedBlock>,
    pub timestamp_ms: BlockTimestampMs,
    pub commit_ref: CommitRef,
    pub reputation_scores_desc: Vec<(AuthorityIndex, u64)>,
}
impl CommittedSubDag {
    pub fn flatten_transactions(&self) -> Vec<Vec<u8>> {
        self.blocks
            .iter()
            .flat_map(|block| {
                block
                    .block
                    .transactions()
                    .iter()
                    .map(|tx| tx.data().to_vec())
            })
            .collect()
    }
    pub fn len(&self) -> usize {
        self.blocks
            .iter()
            .map(|block| block.block.transactions().len())
            .sum()
    }
}
impl From<ConsensusCommittedSubDag> for CommittedSubDag {
    fn from(subdag: ConsensusCommittedSubDag) -> Self {
        // Convert the leader BlockRef (this is already compatible since both use consensus_core::BlockRef)
        let leader = subdag.leader;

        // Convert blocks from consensus_core::VerifiedBlock to reth_extension::VerifiedBlock
        // We can access the Block data through Deref implementation
        let blocks = subdag
            .blocks
            .into_iter()
            .map(|vb| {
                // Extract transactions from the block (available through Deref<Target = Block>)
                let transactions = vb.transactions().to_vec();

                // Create a reth_extension::SignedBlock with the actual transaction data
                let reth_signed_block = SignedBlock::new_genesis(transactions);

                // For the digest, we'll use a computed hash of the transactions for now
                // This can be enhanced when we have better access to the actual digest
                let digest_bytes = vb
                    .transactions()
                    .iter()
                    .flat_map(|tx| tx.data().to_vec())
                    .collect::<Vec<u8>>();
                let reth_digest = if !digest_bytes.is_empty() {
                    // Use a simple hash of the transaction data as a digest
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    digest_bytes.hash(&mut hasher);
                    let hash_value = hasher.finish();
                    let mut digest_array = [0u8; 32];
                    digest_array[..8].copy_from_slice(&hash_value.to_le_bytes());
                    BlockDigest(digest_array)
                } else {
                    BlockDigest::MIN
                };

                VerifiedBlock {
                    block: reth_signed_block,
                    digest: reth_digest,
                }
            })
            .collect();

        // Convert timestamp (already compatible)
        let timestamp_ms = subdag.timestamp_ms;

        // Convert commit_ref (already compatible since both use consensus_core::CommitRef)
        let commit_ref = subdag.commit_ref;

        // Convert reputation scores (already compatible)
        let reputation_scores_desc = subdag.reputation_scores_desc;

        Self {
            leader,
            blocks,
            timestamp_ms,
            commit_ref,
            reputation_scores_desc,
        }
    }
}
