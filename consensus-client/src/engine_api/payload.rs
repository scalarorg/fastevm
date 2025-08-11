use alloy_primitives::{Address, Bloom, Bytes, FixedBytes, U256};
use alloy_rpc_types_engine::{ExecutionPayloadInputV2, ExecutionPayloadV1};
use consensus_core::{BlockAPI, BlockRef, CommittedSubDag, VerifiedBlock};

use super::merkle_root_keccak;

type FixedByte32 = FixedBytes<32>;

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
            receipts_root: FixedByte32::default(), // TODO: compute receipts root from execution receipts
            logs_bloom: Bloom::default(),          // TODO
            prev_randao: FixedByte32::default(),   // TODO: mixHash or randomness
            block_number,
            gas_limit,
            gas_used,
            timestamp: timestamp_secs,
            extra_data: Bytes::default(), // TODO: any extra data
            base_fee_per_gas: base_fee,
            block_hash: FixedByte32::default(), // placeholder: NOT real block hash. TODO compute header hash.
            transactions: flattened_txs,
        };

        ExecutionPayloadInputV2 {
            execution_payload,
            withdrawals: None,
        }
    }
}

fn try_digest_from_blockref(_br: &BlockRef) -> Option<FixedByte32> {
    // TODO: BlockRef likely contains digest info (display uses BlockRef in commit.rs).
    // Example:
    // if br has a .digest() -> Digest type which can be converted into bytes, do that.
    None
}

fn try_author_addr_from_blockref(_br: &BlockRef) -> Option<Address> {
    // TODO: derive 20-byte address from block author if available.
    None
}
