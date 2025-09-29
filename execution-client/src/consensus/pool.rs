//! This module contains the implementation of the block creation from committed subdag.
//! Transactions in the subdag may are not ordered by nonce.
//! Or is not cons

use alloy_consensus::Transaction;
use alloy_primitives::{keccak256, Bytes, TxHash, B256};
use reth_ethereum::rpc::types::engine::ExecutionPayload;
use reth_extension::CommittedTransactions;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex, RwLock},
};
use tracing::debug;

use crate::consensus::BatchCommittedSubDag;

/// Struct to store committed transactions and pooled transactions
/// Pooled transactions are transactions from committed transactions that are added to the reth pool
// struct PooledCommittedTransactions<Transaction: PoolTransaction> {
//     // next mysticeti committed transactions
//     committed_transactions: CommittedTransactions<Transaction>,
//     // transactions already in reth pool
//     pooled_transactions: Vec<Arc<ValidPoolTransaction<Transaction>>>,
// }
pub struct ConsensusPool<Pool: TransactionPool>
where
    Pool: TransactionPool,
{
    committed_subdags_per_block: usize,
    next_committed_index: RwLock<u64>,
    //Store committed transactions (converted from subdag) in queue
    commited_queue: RwLock<BTreeMap<u64, CommittedTransactions<Pool::Transaction>>>,
    // Transactions are not included into last payload due to missing of ancestors
    pending_transactions: RwLock<Vec<Arc<Pool::Transaction>>>,

    lock: Mutex<()>,
    // latest_state: Option<StateProviderBox>,
}

impl<Pool: TransactionPool> ConsensusPool<Pool>
where
    Pool: TransactionPool,
{
    pub fn new(committed_subdags_per_block: usize) -> Self {
        Self {
            committed_subdags_per_block,
            //First committed index is 1
            next_committed_index: RwLock::new(1),
            commited_queue: RwLock::new(BTreeMap::new()),
            pending_transactions: RwLock::new(Vec::new()),
            lock: Mutex::new(()),
        }
    }
    pub fn next_committed_subdag_batch(&self) -> Option<BatchCommittedSubDag<Pool::Transaction>> {
        let next_committed_index = *self.next_committed_index.read().unwrap();
        let last_index = next_committed_index + self.committed_subdags_per_block as u64 - 1;
        let commited_queue = self.commited_queue.read().unwrap();
        let mut first_committed_transactions = None;
        let mut last_committed_transactions = None;
        for i in next_committed_index..=last_index {
            let committed_transactions = commited_queue.get(&i);
            if committed_transactions.is_none() {
                return None;
            }
            if i == next_committed_index {
                first_committed_transactions = committed_transactions;
            }
            if i == last_index {
                last_committed_transactions = committed_transactions;
            }
        }
        match (first_committed_transactions, last_committed_transactions) {
            (Some(first_committed_transactions), Some(last_committed_transactions)) => {
                Some(BatchCommittedSubDag {
                    first_committed_subdag: first_committed_transactions.clone(),
                    last_committed_subdag: last_committed_transactions.clone(),
                })
            }
            _ => None,
        }
    }
    /// Get last committed transactions in the next committed batch
    pub fn last_committed_transaction_in_batch(
        &self,
    ) -> Option<CommittedTransactions<Pool::Transaction>> {
        let next_committed_index = *self.next_committed_index.read().unwrap();
        let committed_transactions = {
            let last_index = next_committed_index + self.committed_subdags_per_block as u64 - 1;
            let commited_queue = self.commited_queue.read().unwrap();
            let committed_transactions = commited_queue.get(&last_index).map(|tx| tx.clone());
            if committed_transactions.is_none() {
                if commited_queue.len() < self.committed_subdags_per_block {
                    debug!(
                        "Next committed index: {}. Queue size: {}. Waiting for next batch.",
                        next_committed_index,
                        commited_queue.len()
                    );
                } else {
                    //Find all missing committed transactions
                    let mut missing_committed_transactions = Vec::new();
                    for i in next_committed_index..=last_index {
                        let committed_transactions = commited_queue.get(&i);
                        if committed_transactions.is_none() {
                            missing_committed_transactions.push(i);
                        }
                    }
                    debug!(
                        "Next committed index: {}. Missing committed transactions: {:?}.",
                        next_committed_index, missing_committed_transactions
                    );
                }
            }
            committed_transactions
        };
        committed_transactions
    }
    /// Get queue size
    pub fn queue_size(&self) -> usize {
        self.commited_queue.read().unwrap().len()
    }
}

impl<Pool: TransactionPool> ConsensusPool<Pool>
where
    Pool: TransactionPool,
{
    pub fn add_committed_transactions(
        &self,
        committed_transactions: CommittedTransactions<Pool::Transaction>,
    ) {
        self.commited_queue.write().unwrap().insert(
            committed_transactions.commit_ref.index as u64,
            committed_transactions,
        );
    }
    /// Append transactions from next committed subdags to pending transactions
    /// TODO: Add some system transaction
    pub fn append_proposal_transactions(
        &self,
        pending_transactions: &mut Vec<Arc<Pool::Transaction>>,
        next_committed_subdags_batch: Vec<CommittedTransactions<Pool::Transaction>>,
    ) {
        let first_committed_transactions = next_committed_subdags_batch.first().unwrap();
        let last_committed_transactions = next_committed_subdags_batch.last().unwrap();
        debug!(
            "Append transactions within {} subdags from {} to {}",
            next_committed_subdags_batch.len(),
            first_committed_transactions.commit_ref.index,
            last_committed_transactions.commit_ref.index,
        );
        for committed_transactions in next_committed_subdags_batch {
            pending_transactions.extend(committed_transactions.transactions);
        }

        // Sort transactions by nonce-based ordering:
        // 1. Find minimum expected nonce for each sender
        // 2. Pick transactions with minimum nonce for each sender
        // 3. Repeat until no more transactions can be picked
        // 4. Put remaining transactions at the end
        self.sort_transactions_by_nonce(pending_transactions);
    }

    /// Sort transactions by nonce-based ordering:
    /// 1. Find minimum expected nonce for each sender
    /// 2. Pick transactions with minimum nonce for each sender
    /// 3. Repeat until no more transactions can be picked
    /// 4. Put remaining transactions at the end
    fn sort_transactions_by_nonce(&self, pending_transactions: &mut Vec<Arc<Pool::Transaction>>) {
        if pending_transactions.is_empty() {
            return;
        }

        let mut sorted_transactions = Vec::new();
        let mut remaining_transactions = pending_transactions.clone();

        // Track expected nonce for each sender, start with minimun nonce in the pending transactions
        // Some senders may have missing nonce, their transactions will be marked as invalid
        let mut sender_expected_nonce: BTreeMap<alloy_primitives::Address, u64> = BTreeMap::new();

        // Find minimum nonce for each sender
        for tx in &remaining_transactions {
            let sender = tx.sender();
            let nonce = tx.nonce();
            sender_expected_nonce
                .entry(sender)
                .and_modify(|expected| *expected = (*expected).min(nonce))
                .or_insert(nonce);
        }

        loop {
            let mut picked_any = false;
            let mut picked_indices = Vec::new();

            // Pick transactions with minimum expected nonce for each sender
            for (i, tx) in remaining_transactions.iter().enumerate() {
                let sender = tx.sender();
                let nonce = tx.nonce();

                if let Some(&expected_nonce) = sender_expected_nonce.get(&sender) {
                    if nonce == expected_nonce {
                        sorted_transactions.push(tx.clone());
                        picked_indices.push(i);
                        picked_any = true;

                        // Increment expected nonce for this sender
                        *sender_expected_nonce.get_mut(&sender).unwrap() += 1;
                    }
                }
            }

            // Remove picked transactions from remaining (in reverse order to maintain indices)
            for &i in picked_indices.iter().rev() {
                remaining_transactions.remove(i);
            }

            // If no transactions were picked in this iteration, we're done
            if !picked_any {
                break;
            }
        }
        if !remaining_transactions.is_empty() {
            debug!("Remaining transactions: {:?}", remaining_transactions.len());
            // Add remaining transactions at the end
            sorted_transactions.extend(remaining_transactions);
        }

        // Replace the original vector with sorted transactions
        *pending_transactions = sorted_transactions;
    }

    /// Get all pending transactions and transactions from next {committed_subdags_per_block} committed subdags
    /// Order them by sender nonce
    /// We remove processed committed transactions from committed queue when next proposal block is executed
    /// Make sure this method is not change underly pending transactions except first call
    pub fn get_proposal_transactions(&self) -> Vec<Arc<Pool::Transaction>> {
        // 1. Append transactions from next committed subdag to pending transactions
        let mut pending_transactions = self.pending_transactions.read().unwrap().clone();
        let committed_queue = self.commited_queue.read().unwrap();
        //Check if there are enough committed transactions to fill the block
        if committed_queue.len() < self.committed_subdags_per_block {
            return Vec::new();
        }
        let mut next_committed_subdags_batch = Vec::new();
        //Pop enough committed transactions to fill the block
        let next_committed_index = *self.next_committed_index.read().unwrap();
        for i in 0..self.committed_subdags_per_block {
            let index = next_committed_index + i as u64;
            let next_committed_transactions = committed_queue.get(&index).map(|tx| tx.clone());
            //This next_committed_transactions should be some
            assert!(next_committed_transactions.is_some());
            let next_committed_transactions = next_committed_transactions.unwrap();
            next_committed_subdags_batch.push(next_committed_transactions);
        }
        let last_committed_index = next_committed_subdags_batch
            .last()
            .unwrap()
            .commit_ref
            .index;
        self.append_proposal_transactions(&mut pending_transactions, next_committed_subdags_batch);
        debug!(
            "Get proposal transactions with {:?} transactions. Last committed index: {:?}",
            pending_transactions.len(),
            last_committed_index
        );
        //Log pending transactions for debug purposes
        for tx in pending_transactions.iter() {
            debug!(
                "transaction: {:?}, sender: {:?}, nonce: {:?}",
                tx.hash(),
                tx.sender_ref(),
                tx.nonce()
            );
        }
        //Clone pending transactions for building a BestTransactions iterator
        return pending_transactions;
    }
    /// Remove mined transactions from both pending transactions and committed queue
    pub fn remove_mined_transactions(&self, execution_payload: &ExecutionPayload) {
        let tx_hashes = execution_payload
            .transactions()
            .iter()
            .map(|tx| calculate_tx_hash(tx))
            .collect::<HashSet<TxHash>>();
        //We lock the consensus pool to ensure thread safety
        //make sure committed queue is not modified while removing mined transactions
        let _lock = self.lock.lock().unwrap();
        // Lock both collections to ensure thread safety
        let mut pending_transactions = self.pending_transactions.write().unwrap();
        let mut committed_queue = self.commited_queue.write().unwrap();
        let mut next_committed_index = self.next_committed_index.write().unwrap();
        let mut next_committed_subdags_batch = Vec::new();
        for i in 0..self.committed_subdags_per_block {
            let index = *next_committed_index + i as u64;
            let committed_transactions = committed_queue.remove(&index);
            assert!(committed_transactions.is_some());
            next_committed_subdags_batch.push(committed_transactions.unwrap());
        }
        self.append_proposal_transactions(&mut pending_transactions, next_committed_subdags_batch);
        let initial_pending_len = pending_transactions.len();
        // Remove mined transactions from pending transactions
        pending_transactions.retain(|tx| !tx_hashes.contains(tx.hash()));
        //Increase next committed index for next batch
        *next_committed_index += self.committed_subdags_per_block as u64;

        debug!(
            "Removed mined transactions in block number {:?} with {:?} mined txs. Pending txs reduced from {} to {}. Remain committed subdags len: {}",
            execution_payload.block_number(),
            tx_hashes.len(),
            initial_pending_len,
            pending_transactions.len(),
            committed_queue.len()
        );
    }
}

fn calculate_tx_hash(tx: &Bytes) -> TxHash {
    let tx_hash: B256 = keccak256(tx);

    // // Decode the transaction
    // let transaction: TransactionSigned = TransactionSigned::decode_2718(&mut tx)?;

    // // Get the hash
    // *transaction.tx_hash()
    tx_hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;

    // Simple test to verify the sorting logic works correctly
    #[test]
    fn test_transaction_ordering_logic() {
        // Test the core sorting logic without complex mocks
        let mut addresses = vec![
            Address::from([2u8; 20]), // Higher address
            Address::from([1u8; 20]), // Lower address
            Address::from([3u8; 20]), // Highest address
        ];

        // Sort addresses to verify ordering
        addresses.sort();

        // Verify deterministic ordering
        assert_eq!(addresses[0], Address::from([1u8; 20]));
        assert_eq!(addresses[1], Address::from([2u8; 20]));
        assert_eq!(addresses[2], Address::from([3u8; 20]));
    }

    #[test]
    fn test_address_comparison() {
        // Test that Address implements proper comparison traits
        let addr1 = Address::from([1u8; 20]);
        let addr2 = Address::from([2u8; 20]);
        let addr3 = Address::from([1u8; 20]);

        // Test equality
        assert_eq!(addr1, addr3);
        assert_ne!(addr1, addr2);

        // Test ordering
        assert!(addr1 < addr2);
        assert!(addr2 > addr1);
    }

    #[test]
    fn test_sorting_consistency() {
        // Test that the same input always produces the same output
        let mut data1 = vec![3, 1, 2];
        let mut data2 = vec![3, 1, 2];

        data1.sort();
        data2.sort();

        assert_eq!(data1, data2);
        assert_eq!(data1, vec![1, 2, 3]);
    }

    #[test]
    fn test_nonce_based_sorting() {
        // Test the new nonce-based sorting logic
        // This simulates the core logic of the sort_transactions_by_nonce method

        // Create test data with different senders and nonces
        let test_data = vec![
            (Address::from([2u8; 20]), 3u64), // sender2, nonce 3
            (Address::from([1u8; 20]), 2u64), // sender1, nonce 2
            (Address::from([2u8; 20]), 1u64), // sender2, nonce 1
            (Address::from([1u8; 20]), 4u64), // sender1, nonce 4
            (Address::from([3u8; 20]), 1u64), // sender3, nonce 1
            (Address::from([1u8; 20]), 1u64), // sender1, nonce 1
        ];

        // Simulate the nonce-based sorting algorithm
        let mut sorted_data = Vec::new();
        let mut remaining_data = test_data.clone();

        // Track expected nonce for each sender
        let mut sender_expected_nonce: BTreeMap<Address, u64> = BTreeMap::new();

        // Find minimum nonce for each sender
        for (sender, nonce) in &remaining_data {
            sender_expected_nonce
                .entry(*sender)
                .and_modify(|expected| *expected = (*expected).min(*nonce))
                .or_insert(*nonce);
        }

        loop {
            let mut picked_any = false;
            let mut picked_indices = Vec::new();

            // Pick transactions with minimum expected nonce for each sender
            for (i, (sender, nonce)) in remaining_data.iter().enumerate() {
                if let Some(&expected_nonce) = sender_expected_nonce.get(sender) {
                    if *nonce == expected_nonce {
                        sorted_data.push((*sender, *nonce));
                        picked_indices.push(i);
                        picked_any = true;

                        // Increment expected nonce for this sender
                        *sender_expected_nonce.get_mut(sender).unwrap() += 1;
                    }
                }
            }

            // Remove picked transactions from remaining (in reverse order to maintain indices)
            for &i in picked_indices.iter().rev() {
                remaining_data.remove(i);
            }

            // If no transactions were picked in this iteration, we're done
            if !picked_any {
                break;
            }
        }

        // Add remaining transactions at the end
        sorted_data.extend(remaining_data);

        // Verify the expected ordering:
        // The algorithm picks transactions in the order they appear in the original vector
        // when multiple senders have the same minimum nonce
        assert_eq!(sorted_data[0], (Address::from([2u8; 20]), 1u64));
        assert_eq!(sorted_data[1], (Address::from([3u8; 20]), 1u64));
        assert_eq!(sorted_data[2], (Address::from([1u8; 20]), 1u64));
        assert_eq!(sorted_data[3], (Address::from([1u8; 20]), 2u64));
        assert_eq!(sorted_data[4], (Address::from([2u8; 20]), 3u64));
        assert_eq!(sorted_data[5], (Address::from([1u8; 20]), 4u64));
    }

    #[test]
    fn test_nonce_based_sorting_with_gaps() {
        // Test sorting with nonce gaps (missing nonces)
        let test_data = vec![
            (Address::from([1u8; 20]), 3u64), // sender1, nonce 3
            (Address::from([1u8; 20]), 1u64), // sender1, nonce 1
            (Address::from([1u8; 20]), 5u64), // sender1, nonce 5 (gap at 2, 4)
            (Address::from([2u8; 20]), 2u64), // sender2, nonce 2
            (Address::from([2u8; 20]), 4u64), // sender2, nonce 4
        ];

        // Simulate the nonce-based sorting algorithm
        let mut sorted_data = Vec::new();
        let mut remaining_data = test_data.clone();

        // Track expected nonce for each sender
        let mut sender_expected_nonce: BTreeMap<Address, u64> = BTreeMap::new();

        // Find minimum nonce for each sender
        for (sender, nonce) in &remaining_data {
            sender_expected_nonce
                .entry(*sender)
                .and_modify(|expected| *expected = (*expected).min(*nonce))
                .or_insert(*nonce);
        }

        loop {
            let mut picked_any = false;
            let mut picked_indices = Vec::new();

            // Pick transactions with minimum expected nonce for each sender
            for (i, (sender, nonce)) in remaining_data.iter().enumerate() {
                if let Some(&expected_nonce) = sender_expected_nonce.get(sender) {
                    if *nonce == expected_nonce {
                        sorted_data.push((*sender, *nonce));
                        picked_indices.push(i);
                        picked_any = true;

                        // Increment expected nonce for this sender
                        *sender_expected_nonce.get_mut(sender).unwrap() += 1;
                    }
                }
            }

            // Remove picked transactions from remaining (in reverse order to maintain indices)
            for &i in picked_indices.iter().rev() {
                remaining_data.remove(i);
            }

            // If no transactions were picked in this iteration, we're done
            if !picked_any {
                break;
            }
        }

        // Add remaining transactions at the end
        sorted_data.extend(remaining_data);

        // Verify the expected ordering:
        // First iteration: sender1(nonce 1), sender2(nonce 2)
        // Second iteration: sender1(nonce 3)
        // Remaining: sender1(nonce 5), sender2(nonce 4) - these are at the end due to gaps
        assert_eq!(sorted_data[0], (Address::from([1u8; 20]), 1u64));
        assert_eq!(sorted_data[1], (Address::from([2u8; 20]), 2u64));
        assert_eq!(sorted_data[2], (Address::from([1u8; 20]), 3u64));
        assert_eq!(sorted_data[3], (Address::from([1u8; 20]), 5u64));
        assert_eq!(sorted_data[4], (Address::from([2u8; 20]), 4u64));
    }
}
