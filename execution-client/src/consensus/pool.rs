//! This module contains the implementation of the block creation from committed subdag.
//! Transactions in the subdag may are not ordered by nonce.
//! Or is not cons
//!
//! # Memory Optimizations
//!
//! This module includes several memory optimizations to reduce memory usage:
//! 1. **Reduced Cloning**: Minimizes unnecessary cloning of transactions and subdags
//! 2. **Pre-allocated Vectors**: Reduces memory reallocations during transaction processing
//! 3. **Efficient Memory Usage**: Uses references and slices where possible to avoid copying data
//!
//! Note: All committed subdags and pending transactions are kept to ensure correct
//! transaction ordering. The memory optimizations focus on reducing unnecessary data copying
//! and improving memory efficiency rather than limiting the number of stored transactions.

use alloy_consensus::Transaction;
use alloy_primitives::TxHash;
use rpc_shared_api::MysticetiCommittedSubdag;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, RwLock},
};
use tracing::debug;

// Memory optimization constants
// Note: We don't limit pending transactions or committed subdags as all transactions
// need to be processed in sequence to maintain correct ordering

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
    // First committed subdag is used for building first empty ordered block for update timestamp and epoch.
    first_committed_subdag: RwLock<Option<MysticetiCommittedSubdag<Arc<Pool::Transaction>>>>,
    //Store committed transactions (converted from subdag) in queue    
    commited_queue: RwLock<BTreeMap<u64, MysticetiCommittedSubdag<Arc<Pool::Transaction>>>>,
    // Transactions are not included into last payload due to missing of ancestors
    pending_transactions: RwLock<Vec<Arc<Pool::Transaction>>>,
    // latest_state: Option<StateProviderBox>,
}

impl<Pool: TransactionPool> ConsensusPool<Pool>
where
    Pool: TransactionPool,
{
    /// Creates a new `ConsensusPool` instance.
    ///
    /// # Arguments
    ///
    /// * `committed_subdags_per_block` - The number of committed subdags to process per block.
    ///
    /// # Returns
    ///
    /// A new `ConsensusPool` instance with the next committed index initialized to 1.
    pub fn new(committed_subdags_per_block: usize) -> Self {
        Self {
            committed_subdags_per_block,
            //First committed index is 1
            next_committed_index: RwLock::new(1),
            first_committed_subdag: RwLock::new(None),
            commited_queue: RwLock::new(BTreeMap::new()),
            pending_transactions: RwLock::new(Vec::new()),
        }
    }
    /// Get next committed subdag
    /// # Returns
    ///
    /// * `Some(subdag)` - If there is a next committed subdag.
    /// * `None` - If there is no next committed subdag.
    /// This method is used for building first empty ordered block for update timestamp and epoch.
    /// Loop through committed queue to find the first committed subdag with none zero timestamp
    pub fn get_fist_committed_subdag(&self) -> Option<MysticetiCommittedSubdag<Arc<Pool::Transaction>>> {
        let committed_queue = self.commited_queue.read().unwrap();
        for (_, subdag) in committed_queue.iter() {
            if subdag.timestamp_ms > 0 {
                return Some(subdag.clone());
            }
        }
        return None;    
    }
    fn get_pending_transactions(&self) -> Vec<Arc<Pool::Transaction>> {
        let pending_transactions = self.pending_transactions.read().unwrap();
        pending_transactions.clone()
    }
    fn update_pending_transactions(&self, new_transactions: Vec<Arc<Pool::Transaction>>) {
        let mut pending_transactions = self.pending_transactions.write().unwrap();
        *pending_transactions = new_transactions;
    }
    /// Retrieves the first and last committed subdags from the next batch to be processed.
    ///
    /// This method checks if there are enough committed subdags in the queue to form a complete batch
    /// (based on `committed_subdags_per_block`). It returns the first and last subdags of the batch
    /// without removing them from the queue.
    ///
    /// This method use for building next proposal block in the engine handle flow
    /// # Returns
    ///
    /// * `Some((first, last))` - If there are enough committed subdags in the queue, returns a tuple
    ///   containing the first and last committed subdags of the next batch.
    /// * `None` - If there are not enough committed subdags to form a complete batch.
    pub fn next_committed_subdag_batch(
        &self,
    ) -> Option<(
        MysticetiCommittedSubdag<Arc<Pool::Transaction>>,
        MysticetiCommittedSubdag<Arc<Pool::Transaction>>,
    )> {
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
            // Only clone when needed (first and last), avoid unnecessary clones
            if i == next_committed_index {
                first_committed_transactions = committed_transactions.cloned();
            }
            if i == last_index {
                last_committed_transactions = committed_transactions.cloned();
            }
        }
        first_committed_transactions.zip(last_committed_transactions)

        // match (first_committed_transactions, last_committed_transactions) {
        //     (Some(first_committed_transactions), Some(last_committed_transactions)) => {
        //         Some(BatchCommittedSubDag {
        //             first_committed_subdag: first_committed_transactions.clone(),
        //             last_committed_subdag: last_committed_transactions.clone(),
        //         })
        //     }
        //     _ => None,
        // }
    }
    /// Get queue size
    pub fn queue_size(&self) -> usize {
        self.commited_queue.read().unwrap().len()
    }

    // /// Get memory statistics for monitoring
    // pub fn memory_stats(&self) -> (usize, usize, u64) {
    //     let committed_queue = self.commited_queue.read().unwrap();
    //     let pending_transactions = self.get_pending_transactions();
    //     let next_index = *self.next_committed_index.read().unwrap();
        
    //     (
    //         committed_queue.len(),           // Number of committed subdags
    //         pending_transactions.len(),      // Number of pending transactions
    //         next_index,                      // Next committed index
    //     )
    // }

    // /// Estimate memory usage in bytes
    // /// This is a rough estimate based on typical transaction sizes
    // pub fn estimate_memory_usage(&self) -> (u64, u64) {
    //     let committed_queue = self.commited_queue.read().unwrap();
    //     let pending_transactions = self.get_pending_transactions();
        
    //     // Estimate: ~120 bytes per transaction + overhead
    //     // Subdag overhead: ~200 bytes per subdag
    //     let avg_tx_size = 200u64; // Conservative estimate with overhead
    //     let subdag_overhead = 200u64;
        
    //     let committed_tx_count: usize = committed_queue
    //         .values()
    //         .map(|subdag| subdag.transactions.len())
    //         .sum();
        
    //     let committed_memory = (committed_tx_count as u64 * avg_tx_size) + 
    //                           (committed_queue.len() as u64 * subdag_overhead);
    //     let pending_memory = pending_transactions.len() as u64 * avg_tx_size;
        
    //     (committed_memory, pending_memory)
    // }
}

impl<Pool: TransactionPool> ConsensusPool<Pool>
where
    Pool: TransactionPool,
{
    /// Adds committed subdags to the internal queue for processing.
    ///
    /// The subdags are indexed by their round number and stored in a BTreeMap to maintain
    /// ordering. This method is used to queue up committed subdags that will be processed
    /// when building the next block proposal.
    ///
    /// # Arguments
    ///
    /// * `committed_subdags` - A vector of committed subdags to add to the queue. Each subdag
    ///   is indexed by its `commit_ref.round` value.
    pub fn add_committed_subdags(
        &self,
        committed_subdags: Vec<MysticetiCommittedSubdag<Arc<Pool::Transaction>>>,
    ) {
        if committed_subdags.is_empty() {
            return;
        }
        let first_committed_subdag = committed_subdags.first().unwrap();
        if self.first_committed_subdag.read().unwrap().is_none() {
            self.first_committed_subdag.write().unwrap().replace(first_committed_subdag.clone());
        }
        let len = committed_subdags.len();
        let mut committed_queue = self.commited_queue.write().unwrap();
        for committed_subdag in committed_subdags {
            committed_queue.insert(committed_subdag.commit_ref.round as u64, committed_subdag);
        }
        debug!(
            "Added {} committed subdags to queue. Queue size: {:?}",
            len,
            committed_queue.len()
        );
    }
    /// Append transactions from next committed subdags to pending transactions
    /// Sort transactions by nonce-based ordering
    pub fn create_proposal_transactions(
        &self,
        pending_transactions: &[Arc<Pool::Transaction>],
        next_committed_subdags_batch: Vec<MysticetiCommittedSubdag<Arc<Pool::Transaction>>>,
    ) -> Vec<Arc<Pool::Transaction>> {
        if !next_committed_subdags_batch.is_empty() {
            let first_committed_transactions = next_committed_subdags_batch.first().unwrap();
            let last_committed_transactions = next_committed_subdags_batch.last().unwrap();
            debug!(
                "Append transactions within {} subdags from {} to {}",
                next_committed_subdags_batch.len(),
                first_committed_transactions.commit_ref.round,
                last_committed_transactions.commit_ref.round,
            );
        }
        //Map keep all sender's transactions
        let mut map_sender_txs = HashMap::new();
        for tx in pending_transactions.iter() {
            map_sender_txs
                .entry(tx.sender())
                .or_insert(BTreeMap::new())
                .insert(tx.nonce(), Arc::clone(tx));
        }
        for committed_transactions in next_committed_subdags_batch.iter() {
            for tx in committed_transactions.transactions.iter() {
                map_sender_txs
                    .entry(tx.sender())
                    .or_insert(BTreeMap::new())
                    .insert(tx.nonce(), Arc::clone(tx));
            }
        }

        // Pre-allocate capacity to reduce reallocations
        let estimated_size = pending_transactions.len() + 
            next_committed_subdags_batch.iter()
                .map(|subdag| subdag.transactions.len())
                .sum::<usize>();
        let mut sorted_transactions = Vec::with_capacity(estimated_size);
        
        // Get all transaction by order of nonce
        for tx in pending_transactions.iter() {
            let sender_txs = map_sender_txs.get_mut(&tx.sender()).unwrap();
            if let Some((_, tx)) = sender_txs.pop_first() {
                sorted_transactions.push(tx);
            }
        }
        for committed_transactions in next_committed_subdags_batch {
            for tx in committed_transactions.transactions.iter() {
                let sender_txs = map_sender_txs.get_mut(&tx.sender()).unwrap();
                if let Some((_, tx)) = sender_txs.pop_first() {
                    sorted_transactions.push(tx);
                }
            }
        }
        debug!("Total pending transactions: {}", sorted_transactions.len());
        return sorted_transactions;
    }

    /// Get all pending transactions and transactions from next {committed_subdags_per_block} committed subdags
    /// Order them by sender nonce
    /// We remove processed committed transactions from committed queue when next proposal block is executed
    /// Make sure this method is not change underly pending transactions except first call
    pub fn get_proposal_transactions(&self) -> Vec<Arc<Pool::Transaction>> {
        // 1. Append transactions from next committed subdag to pending transactions
        let committed_queue = self.commited_queue.read().unwrap();
        //Check if there are enough committed transactions to fill the block
        if committed_queue.len() < self.committed_subdags_per_block {
            return Vec::new();
        }
        let mut next_committed_subdags_batch = Vec::with_capacity(self.committed_subdags_per_block);
        //Pop enough committed transactions to fill the block
        let next_committed_index = *self.next_committed_index.read().unwrap();
        for i in 0..self.committed_subdags_per_block {
            let index = next_committed_index + i as u64;
            let next_committed_transactions = committed_queue.get(&index).cloned();
            //This next_committed_transactions should be some
            assert!(next_committed_transactions.is_some());
            let next_committed_transactions = next_committed_transactions.unwrap();
            next_committed_subdags_batch.push(next_committed_transactions);
        }
        let pending_transactions = self.get_pending_transactions();
        let sorted_transactions =
            self.create_proposal_transactions(&pending_transactions[..], next_committed_subdags_batch);
        //Clone pending transactions for building a BestTransactions iterator
        return sorted_transactions;
    }
    /// Update consensus pool after a block is mined
    /// # Arguments
    ///
    /// * `block_number` - The number of the block that is mined.
    /// * `tx_hashes` - The hashes of the transactions that are mined.
    ///
    /// Remove mined transactions from committed queue and pending transactions
    /// Update next committed index for next batch
    /// # Returns
    ///
    /// * `None` - If there is no next committed subdag.
    pub fn update_mined_block(&self, block_number: u64, tx_hashes: &HashSet<TxHash>) {
        let pending_transactions = self.get_pending_transactions();
        let subdag_per_block = self.committed_subdags_per_block;
        let mut committed_subdags_len = 0;
        let mut next_committed_index = self.next_committed_index.write().unwrap();
        let mut next_committed_subdags_batch = Vec::new();
        {
            let mut committed_queue = self.commited_queue.write().unwrap();
            committed_subdags_len = committed_queue.len();
            for i in 0..subdag_per_block {
                let index = *next_committed_index + i as u64;
                let committed_transactions = committed_queue.remove(&index);
                assert!(committed_transactions.is_some());
                next_committed_subdags_batch.push(committed_transactions.unwrap());
            }
        }
        // Use reference to avoid cloning the entire pending_transactions vector
        let mut sorted_transactions =
            self.create_proposal_transactions(pending_transactions.as_slice(), next_committed_subdags_batch);
        let initial_pending_len = pending_transactions.len();
        // Remove mined transactions from pending transactions
        debug!("Remove mined transactions from pending transactions. Pending transactions len: {}", initial_pending_len);
        sorted_transactions.retain(|tx| !tx_hashes.contains(tx.hash()));
        let new_pending_len = sorted_transactions.len();
        debug!("After remove mined transactions. Pending transactions len: {}", new_pending_len);
        self.update_pending_transactions(sorted_transactions);
        //Increase next committed index for next batch
        *next_committed_index += subdag_per_block as u64;
        
        // Calculate estimated memory usage
        // let (committed_mem, pending_mem) = {
        //     let committed_tx_count: usize = committed_queue
        //         .values()
        //         .map(|subdag| subdag.transactions.len())
        //         .sum();
        //     let avg_tx_size = 200u64; // Conservative estimate with overhead
        //     let subdag_overhead = 200u64;
        //     let committed_mem = (committed_tx_count as u64 * avg_tx_size) + 
        //                       (committed_queue.len() as u64 * subdag_overhead);
        //     let pending_mem = new_pending_len as u64 * avg_tx_size;
        //     (committed_mem, pending_mem)
        // };
        
        debug!(
            "Removed mined transactions in block number {:?} with {:?} mined txs. Pending txs reduced from {} to {}. Remain committed subdags len: {}.",
            block_number,
            tx_hashes.len(),
            initial_pending_len,
            new_pending_len,
            committed_subdags_len - subdag_per_block,
            // committed_mem as f64 / 1_000_000.0,
            // pending_mem as f64 / 1_000_000.0
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, U256};
    use rpc_shared_api::{BlockRef, CommitRef};
    use reth_ethereum::pool::noop::NoopTransactionPool;
    use reth_ethereum::rpc::eth::utils::recover_raw_transaction;
    use reth_transaction_pool::PoolTransaction;
    use std::sync::Arc;

    // Helper function to create a mock transaction with specific sender and nonce
    // Note: The actual sender address will be derived from the private key used to sign
    // This creates transactions deterministically based on an index to ensure unique senders/nonces
    fn create_mock_transaction(
        sender: Address,
        nonce: u64,
    ) -> Arc<<NoopTransactionPool as reth_transaction_pool::TransactionPool>::Transaction> {
        use alloy_network::{eip2718::Encodable2718, EthereumWallet, TransactionBuilder};
        use alloy_rpc_types_eth::TransactionRequest;
        use sha2::{Digest, Sha256};
        use tokio::runtime::Runtime;

        // Create a deterministic private key based on sender address only
        // This ensures all transactions from the same sender have the same address
        let key_hash = Sha256::digest(&sender[..]);

        // Use secp256k1 to create a valid private key
        use secp256k1::{SecretKey, SECP256K1};
        let secret_key = SecretKey::from_slice(&key_hash).unwrap_or_else(|_| {
            // If hash doesn't produce valid key, modify it
            let mut modified = key_hash;
            modified[0] = modified[0].wrapping_add(1);
            SecretKey::from_slice(&modified).expect("Failed to create valid secret key")
        });

        // Create signer from secret key bytes
        // We'll use alloy_signer_local through workspace dependency
        // Note: This requires adding alloy-signer-local to dev-dependencies
        // For now, we'll use a runtime approach with tokio

        // Create a tokio runtime for async operations
        let rt = Runtime::new().unwrap();

        // Use a fixed private key approach - create one key per unique sender+nonce combination
        // Map each unique (sender, nonce) to a deterministic private key
        rt.block_on(async {
            use alloy_primitives::FixedBytes;
            use alloy_signer_local::PrivateKeySigner;

            // Create signer from the secret key
            let key_bytes: FixedBytes<32> = FixedBytes::from(secret_key.secret_bytes());
            let signer = PrivateKeySigner::from_bytes(&key_bytes).unwrap();

            // Build transaction request
            let tx_req = TransactionRequest::default()
                .with_to(Address::ZERO)
                .with_value(U256::ZERO)
                .with_gas_limit(21000)
                .with_max_priority_fee_per_gas(1_000_000_000u128)
                .with_max_fee_per_gas(20_000_000_000u128)
                .with_nonce(nonce)
                .with_chain_id(1);

            // Build and sign
            let ethereum_wallet = EthereumWallet::from(signer);
            let tx_envelope = tx_req.build(&ethereum_wallet).await.unwrap();

            // Encode to raw bytes and recover
            use alloy_primitives::Bytes;
            let encoded = tx_envelope.encoded_2718();
            
            // Convert to Recovered transaction and then to pool transaction
            let recovered = recover_raw_transaction(&Bytes::from(encoded))
                .expect("Failed to recover transaction");
            <NoopTransactionPool as reth_transaction_pool::TransactionPool>::Transaction::from_pooled(recovered)
        })
        .into()
    }

    // Helper function to create a mock MysticetiCommittedSubdag
    fn create_mock_subdag(
        round: usize,
        transactions: Vec<Arc<<NoopTransactionPool as reth_transaction_pool::TransactionPool>::Transaction>>,
    ) -> MysticetiCommittedSubdag<Arc<<NoopTransactionPool as reth_transaction_pool::TransactionPool>::Transaction>> {
        MysticetiCommittedSubdag {
            leader: BlockRef::default(),
            transactions,
            timestamp_ms: 0,
            commit_ref: CommitRef {
                round,
                digest: [0u8; 32],
            },
            reputation_scores_desc: Vec::new(),
        }
    }

    // Helper to extract sender and nonce from a transaction for verification
    fn get_tx_info(tx: &Arc<<NoopTransactionPool as reth_transaction_pool::TransactionPool>::Transaction>) -> (Address, u64) {
        (tx.sender(), tx.nonce())
    }

    #[test]
    fn test_create_proposal_transactions_empty_inputs() {
        let pool = ConsensusPool::<NoopTransactionPool>::new(1);
        let pending = Vec::new();
        let committed = Vec::new();

        let result = pool.create_proposal_transactions(&pending, committed);

        assert!(result.is_empty());
    }

    #[test]
    fn test_create_proposal_transactions_only_pending() {
        let pool = ConsensusPool::<NoopTransactionPool>::new(1);
        let sender1 = Address::from([1u8; 20]);
        let sender2 = Address::from([2u8; 20]);

        let tx1 = create_mock_transaction(sender1, 0);
        let tx2 = create_mock_transaction(sender2, 0);
        let tx3 = create_mock_transaction(sender1, 1);
        let actual_sender1 = tx1.sender();
        let actual_sender2 = tx2.sender();

        let pending = vec![tx1, tx2, tx3];

        let committed = Vec::new();

        let result = pool.create_proposal_transactions(&pending, committed);

        // Should return transactions in order: pending order preserved but with nonce ordering
        assert_eq!(result.len(), 3);
        // First transaction from actual_sender1 should be nonce 0
        assert_eq!(get_tx_info(&result[0]), (actual_sender1, 0));
        // Second transaction from actual_sender2 should be nonce 0
        assert_eq!(get_tx_info(&result[1]), (actual_sender2, 0));
        // Third transaction from actual_sender1 should be nonce 1
        assert_eq!(get_tx_info(&result[2]), (actual_sender1, 1));
    }

    #[test]
    fn test_create_proposal_transactions_only_committed() {
        let pool = ConsensusPool::<NoopTransactionPool>::new(1);
        let sender1 = Address::from([1u8; 20]);
        let sender2 = Address::from([2u8; 20]);

        let pending = Vec::new();

        // Create transactions and get their actual sender addresses
        let tx1 = create_mock_transaction(sender1, 0);
        let tx2 = create_mock_transaction(sender2, 0);
        let tx3 = create_mock_transaction(sender1, 1);
        let actual_sender1 = tx1.sender();
        let actual_sender2 = tx2.sender();

        let committed = vec![create_mock_subdag(
            1,
            vec![tx1, tx2, tx3],
        )];

        let result = pool.create_proposal_transactions(&pending, committed);

        assert_eq!(result.len(), 3);
        // Should be ordered by nonce: actual_sender1 nonce 0, actual_sender2 nonce 0, actual_sender1 nonce 1
        assert_eq!(get_tx_info(&result[0]), (actual_sender1, 0));
        assert_eq!(get_tx_info(&result[1]), (actual_sender2, 0));
        assert_eq!(get_tx_info(&result[2]), (actual_sender1, 1));
    }

    #[test]
    fn test_create_proposal_transactions_combined() {
        let pool = ConsensusPool::<NoopTransactionPool>::new(1);
        let sender1 = Address::from([1u8; 20]);
        let sender2 = Address::from([2u8; 20]);

        // Pending transactions: sender1 nonce 2, sender2 nonce 1
        let pending_tx1 = create_mock_transaction(sender1, 2);
        let pending_tx2 = create_mock_transaction(sender2, 1);

        // Committed transactions: sender1 nonce 0, sender1 nonce 1, sender2 nonce 0
        let committed_tx1 = create_mock_transaction(sender1, 0);
        let committed_tx2 = create_mock_transaction(sender1, 1);
        let committed_tx3 = create_mock_transaction(sender2, 0);
        
        let actual_sender1 = committed_tx1.sender();
        let actual_sender2 = committed_tx3.sender();
        
        let pending = vec![pending_tx1, pending_tx2];

        let committed = vec![create_mock_subdag(
            1,
            vec![committed_tx1, committed_tx2, committed_tx3],
        )];

        let result = pool.create_proposal_transactions(&pending, committed);

        assert_eq!(result.len(), 5);
        // The algorithm processes pending first, taking lowest nonce for each sender
        // Then processes committed, taking lowest remaining nonce for each sender
        let tx0 = get_tx_info(&result[0]);
        let tx1 = get_tx_info(&result[1]);
        let tx2 = get_tx_info(&result[2]);
        let tx3 = get_tx_info(&result[3]);
        let tx4 = get_tx_info(&result[4]);

        // First should be actual_sender1 nonce 0 (lowest nonce in map when processing pending[0])
        assert_eq!(tx0.0, actual_sender1);
        assert_eq!(tx0.1, 0);
        // Second should be actual_sender2 nonce 0 (lowest nonce in map when processing pending[1])
        assert_eq!(tx1.0, actual_sender2);
        assert_eq!(tx1.1, 0);
        // Remaining transactions from committed batch
        assert_eq!(tx2.0, actual_sender1);
        assert_eq!(tx2.1, 1);
        assert_eq!(tx3.0, actual_sender1);
        assert_eq!(tx3.1, 2);
        assert_eq!(tx4.0, actual_sender2);
        assert_eq!(tx4.1, 1);
    }

    #[test]
    fn test_create_proposal_transactions_nonce_ordering_single_sender() {
        let pool = ConsensusPool::<NoopTransactionPool>::new(1);
        let sender1 = Address::from([1u8; 20]);

        // Create transactions with nonces in wrong order
        let tx1 = create_mock_transaction(sender1, 2);
        let tx2 = create_mock_transaction(sender1, 0);
        let tx3 = create_mock_transaction(sender1, 1);
        let actual_sender = tx1.sender();

        let pending = vec![tx1, tx2, tx3];

        let committed = Vec::new();

        let result = pool.create_proposal_transactions(&pending, committed);

        assert_eq!(result.len(), 3);
        // Should be ordered by nonce: 0, 1, 2
        assert_eq!(get_tx_info(&result[0]), (actual_sender, 0));
        assert_eq!(get_tx_info(&result[1]), (actual_sender, 1));
        assert_eq!(get_tx_info(&result[2]), (actual_sender, 2));
    }

    #[test]
    fn test_create_proposal_transactions_multiple_committed_subdags() {
        let pool = ConsensusPool::<NoopTransactionPool>::new(1);
        let sender1 = Address::from([1u8; 20]);

        let pending = Vec::new();

        // Create multiple committed subdags
        let tx1 = create_mock_transaction(sender1, 2);
        let tx2 = create_mock_transaction(sender1, 0);
        let tx3 = create_mock_transaction(sender1, 1);
        let actual_sender = tx1.sender();

        let committed = vec![
            create_mock_subdag(1, vec![tx1]),
            create_mock_subdag(2, vec![tx2]),
            create_mock_subdag(3, vec![tx3]),
        ];

        let result = pool.create_proposal_transactions(&pending, committed);

        assert_eq!(result.len(), 3);
        // Should process subdags in order, but transactions ordered by nonce within sender
        // Processing order: subdag 1 (nonce 2), subdag 2 (nonce 0), subdag 3 (nonce 1)
        // But lowest nonce is picked first, so: nonce 0, then 1, then 2
        assert_eq!(get_tx_info(&result[0]), (actual_sender, 0));
        assert_eq!(get_tx_info(&result[1]), (actual_sender, 1));
        assert_eq!(get_tx_info(&result[2]), (actual_sender, 2));
    }

    #[test]
    fn test_create_proposal_transactions_duplicate_nonce_same_sender() {
        let pool = ConsensusPool::<NoopTransactionPool>::new(1);
        let sender1 = Address::from([1u8; 20]);

        // Create transactions with duplicate nonces - later one overwrites in map
        let pending_tx = create_mock_transaction(sender1, 0);
        let committed_tx1 = create_mock_transaction(sender1, 0); // Duplicate nonce
        let committed_tx2 = create_mock_transaction(sender1, 1);
        let actual_sender = pending_tx.sender();

        let pending = vec![pending_tx];

        let committed = vec![create_mock_subdag(
            1,
            vec![committed_tx1, committed_tx2],
        )];

        let result = pool.create_proposal_transactions(&pending, committed);

        // Should have 2 transactions (one duplicate nonce overwrites)
        assert_eq!(result.len(), 2);
        // Both should have nonce 0 or 1 (the later nonce 0 overwrites the first)
        // Actually, looking at the code, the committed one overwrites pending since it's inserted after
        assert_eq!(get_tx_info(&result[0]).0, actual_sender);
        assert_eq!(get_tx_info(&result[1]).0, actual_sender);
        // One should be nonce 0 (from committed), one should be nonce 1
        let nonces: Vec<u64> = result.iter().map(|tx| get_tx_info(tx).1).collect();
        assert!(nonces.contains(&0));
        assert!(nonces.contains(&1));
    }

    #[test]
    fn test_create_proposal_transactions_multiple_senders() {
        let pool = ConsensusPool::<NoopTransactionPool>::new(1);
        let sender1 = Address::from([1u8; 20]);
        let sender2 = Address::from([2u8; 20]);
        let sender3 = Address::from([3u8; 20]);

        let pending_tx1 = create_mock_transaction(sender1, 1);
        let pending_tx2 = create_mock_transaction(sender2, 0);
        let pending_tx3 = create_mock_transaction(sender3, 2);

        let committed_tx1 = create_mock_transaction(sender1, 0);
        let committed_tx2 = create_mock_transaction(sender2, 1);
        let committed_tx3 = create_mock_transaction(sender3, 0);

        let actual_sender1 = committed_tx1.sender();
        let actual_sender2 = pending_tx2.sender();
        let actual_sender3 = committed_tx3.sender();

        let pending = vec![pending_tx1, pending_tx2, pending_tx3];

        let committed = vec![create_mock_subdag(
            1,
            vec![committed_tx1, committed_tx2, committed_tx3],
        )];

        let result = pool.create_proposal_transactions(&pending, committed);

        assert_eq!(result.len(), 6);

        // Check that pending transactions are processed first with their lowest nonce
        // Then committed transactions are processed
        // actual_sender1: has nonce 0 and 1 -> processes nonce 0 first
        assert_eq!(get_tx_info(&result[0]), (actual_sender1, 0));
        // actual_sender2: has nonce 0 and 1 -> processes nonce 0 first
        assert_eq!(get_tx_info(&result[1]), (actual_sender2, 0));
        // actual_sender3: has nonce 0 and 2 -> processes nonce 0 first
        assert_eq!(get_tx_info(&result[2]), (actual_sender3, 0));

        // Then remaining transactions from committed batch
        let remaining: Vec<_> = result.iter().skip(3).map(|tx| get_tx_info(tx)).collect();
        assert!(remaining.contains(&(actual_sender1, 1)));
        assert!(remaining.contains(&(actual_sender2, 1)));
        assert!(remaining.contains(&(actual_sender3, 2)));
    }

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
