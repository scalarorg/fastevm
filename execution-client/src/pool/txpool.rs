use super::MysticetiPoolConfig;
use crate::pool::{metrics::TxPoolMetrics, MysticetiPool};
use alloy_eips::{
    eip1559::{ETHEREUM_BLOCK_GAS_LIMIT_30M, MIN_PROTOCOL_BASE_FEE},
    eip4844::BLOB_TX_MIN_BLOB_GASPRICE,
};
use alloy_primitives::{Address, TxHash, B256, U256};
use parking_lot::RwLock;
use reth_extension::MysticetiCommittedSubdag;
use reth_transaction_pool::{
    error::{InvalidPoolTransactionError, PoolError, PoolErrorKind},
    validate::ValidPoolTransaction,
    AllPoolTransactions, AllTransactionsEvents, BestTransactions, BestTransactionsAttributes,
    BlobStoreError, BlockInfo, EthPoolTransaction, GetPooledTransactionLimit, NewBlobSidecar,
    NewTransactionEvent, PoolResult, PoolSize, PoolTransaction, PropagatedTransactions,
    TransactionEvents, TransactionListenerKind, TransactionOrdering, TransactionOrigin,
    TransactionPool,
};
use rustc_hash::FxHashMap;
use std::fmt;
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
};
use tokio::sync::mpsc;
use tracing::debug;

/// Guarantees max transactions for one sender, compatible with geth/erigon
pub const TXPOOL_MAX_ACCOUNT_SLOTS_PER_SENDER: usize = 16;

/// Simple implementation of BestTransactions iterator
pub struct SimpleBestTransactions<T: TransactionOrdering> {
    transactions: Vec<Arc<ValidPoolTransaction<T::Transaction>>>,
    index: usize,
    skip_blobs: bool,
}

impl<T: TransactionOrdering> SimpleBestTransactions<T> {
    pub fn new(transactions: Vec<Arc<ValidPoolTransaction<T::Transaction>>>) -> Self {
        Self {
            transactions,
            index: 0,
            skip_blobs: false,
        }
    }
}

impl<T: TransactionOrdering> Iterator for SimpleBestTransactions<T> {
    type Item = Arc<ValidPoolTransaction<T::Transaction>>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.transactions.len() {
            let tx = &self.transactions[self.index];
            self.index += 1;

            // Skip blob transactions if requested
            if self.skip_blobs && tx.is_eip4844() {
                continue;
            }

            return Some(tx.clone());
        }
        None
    }
}

impl<T: TransactionOrdering> BestTransactions for SimpleBestTransactions<T> {
    fn mark_invalid(&mut self, _transaction: &Self::Item, _kind: InvalidPoolTransactionError) {
        // TODO: Implement proper invalidation logic
        // For now, we'll just continue to the next transaction
    }

    fn no_updates(&mut self) {
        // TODO: Implement no updates logic
    }

    fn set_skip_blobs(&mut self, skip_blobs: bool) {
        self.skip_blobs = skip_blobs;
    }
}

pub struct TxPool<T: PoolTransaction> {
    /// Pool settings to enforce limits etc.
    config: MysticetiPoolConfig,
    //Store committed transactions (converted from subdag) in queue
    commited_queue: RwLock<BTreeMap<u64, MysticetiCommittedSubdag<T>>>,
    //Next committed index expected for next proposal block
    next_committed_index: RwLock<u64>,
    //Store subdag for next block
    proposal_pool: RwLock<VecDeque<MysticetiCommittedSubdag<T>>>,
    /// pending subpool
    ///
    /// Holds transactions that are tried to execute on evm but is marked as invalid in last execution.
    /// They as add to the proposal for next block.
    pending_pool: Vec<T>,

    /// queued subpool
    ///
    /// Holds all parked transactions that depend on external changes from the sender:
    ///    - valid transactions, stored transaction first commes into the pool.
    ///      They will pick and send to consensus layer.
    ///    - blocked by missing ancestor transaction (has nonce gaps)
    ///    - sender lacks funds to pay for this transaction.
    ///    - transactions that are not valid in last execution.
    // queued_pool: ParkedPool<QueuedOrd<T::Transaction>>,
    queued_pool: VecDeque<T>,
    /// All transactions in the pool.
    all_transactions: AllTransactions<T>,
    /// Transaction pool metrics
    metrics: TxPoolMetrics,
}

impl<T: PoolTransaction> TxPool<T> {
    /// Create a new graph pool instance.
    pub fn new(_ordering: T, config: MysticetiPoolConfig) -> Self {
        Self {
            config,
            commited_queue: RwLock::new(BTreeMap::new()),
            next_committed_index: RwLock::new(1),
            proposal_pool: RwLock::new(VecDeque::new()),
            pending_pool: Vec::new(),
            queued_pool: VecDeque::new(),
            all_transactions: AllTransactions::new(),
            metrics: Default::default(),
        }
    }
    /// Returns the currently tracked block values
    pub const fn block_info(&self) -> BlockInfo {
        BlockInfo {
            block_gas_limit: self.all_transactions.block_gas_limit,
            last_seen_block_hash: self.all_transactions.last_seen_block_hash,
            last_seen_block_number: self.all_transactions.last_seen_block_number,
            pending_basefee: self.all_transactions.pending_fees.base_fee,
            pending_blob_fee: Some(self.all_transactions.pending_fees.blob_fee),
        }
    }

    // /// Updates the tracked blob fee
    // fn update_blob_fee(&mut self, mut pending_blob_fee: u128, base_fee_update: Ordering) {
    //     std::mem::swap(
    //         &mut self.all_transactions.pending_fees.blob_fee,
    //         &mut pending_blob_fee,
    //     );
    //     match (
    //         self.all_transactions
    //             .pending_fees
    //             .blob_fee
    //             .cmp(&pending_blob_fee),
    //         base_fee_update,
    //     ) {
    //         (Ordering::Equal, Ordering::Equal | Ordering::Greater) => {
    //             // fee unchanged, nothing to update
    //         }
    //         (Ordering::Greater, Ordering::Equal | Ordering::Greater) => {
    //             // increased blob fee: recheck pending pool and remove all that are no longer valid
    //             let removed = self
    //                 .pending_pool
    //                 .update_blob_fee(self.all_transactions.pending_fees.blob_fee);
    //             for tx in removed {
    //                 let to = {
    //                     let tx = self
    //                         .all_transactions
    //                         .txs
    //                         .get_mut(tx.id())
    //                         .expect("tx exists in set");

    //                     // the blob fee is too high now, unset the blob fee cap block flag
    //                     tx.state.remove(TxState::ENOUGH_BLOB_FEE_CAP_BLOCK);
    //                     tx.subpool = tx.state.into();
    //                     tx.subpool
    //                 };
    //                 self.add_transaction_to_subpool(to, tx);
    //             }
    //         }
    //         (Ordering::Less, _) | (_, Ordering::Less) => {
    //             // decreased blob/base fee: recheck blob pool and promote all that are now valid
    //             let removed = self
    //                 .blob_pool
    //                 .enforce_pending_fees(&self.all_transactions.pending_fees);
    //             for tx in removed {
    //                 let to = {
    //                     let tx = self
    //                         .all_transactions
    //                         .txs
    //                         .get_mut(tx.id())
    //                         .expect("tx exists in set");
    //                     tx.state.insert(TxState::ENOUGH_BLOB_FEE_CAP_BLOCK);
    //                     tx.state.insert(TxState::ENOUGH_FEE_CAP_BLOCK);
    //                     tx.subpool = tx.state.into();
    //                     tx.subpool
    //                 };
    //                 self.add_transaction_to_subpool(to, tx);
    //             }
    //         }
    //     }
    // }

    // /// Updates the tracked basefee
    // ///
    // /// Depending on the change in direction of the basefee, this will promote or demote
    // /// transactions from the basefee pool.
    // fn update_basefee(&mut self, mut pending_basefee: u64) -> Ordering {
    //     std::mem::swap(
    //         &mut self.all_transactions.pending_fees.base_fee,
    //         &mut pending_basefee,
    //     );
    //     match self
    //         .all_transactions
    //         .pending_fees
    //         .base_fee
    //         .cmp(&pending_basefee)
    //     {
    //         Ordering::Equal => {
    //             // fee unchanged, nothing to update
    //             Ordering::Equal
    //         }
    //         Ordering::Greater => {
    //             // increased base fee: recheck pending pool and remove all that are no longer valid
    //             let removed = self
    //                 .pending_pool
    //                 .update_base_fee(self.all_transactions.pending_fees.base_fee);
    //             for tx in removed {
    //                 let to = {
    //                     let tx = self
    //                         .all_transactions
    //                         .txs
    //                         .get_mut(tx.id())
    //                         .expect("tx exists in set");
    //                     tx.state.remove(TxState::ENOUGH_FEE_CAP_BLOCK);
    //                     tx.subpool = tx.state.into();
    //                     tx.subpool
    //                 };
    //                 self.add_transaction_to_subpool(to, tx);
    //             }

    //             Ordering::Greater
    //         }
    //         Ordering::Less => {
    //             // decreased base fee: recheck basefee pool and promote all that are now valid
    //             let removed = self
    //                 .basefee_pool
    //                 .enforce_basefee(self.all_transactions.pending_fees.base_fee);
    //             for tx in removed {
    //                 let to = {
    //                     let tx = self
    //                         .all_transactions
    //                         .txs
    //                         .get_mut(tx.id())
    //                         .expect("tx exists in set");
    //                     tx.state.insert(TxState::ENOUGH_FEE_CAP_BLOCK);
    //                     tx.subpool = tx.state.into();
    //                     tx.subpool
    //                 };
    //                 self.add_transaction_to_subpool(to, tx);
    //             }

    //             Ordering::Less
    //         }
    //     }
    // }
    /// Sets the current block info for the pool.
    ///
    /// This will also apply updates to the pool based on the new base fee
    pub fn set_block_info(&mut self, info: BlockInfo) {
        let BlockInfo {
            block_gas_limit,
            last_seen_block_hash,
            last_seen_block_number,
            pending_basefee: _,
            pending_blob_fee: _,
        } = info;
        self.all_transactions.last_seen_block_hash = last_seen_block_hash;
        self.all_transactions.last_seen_block_number = last_seen_block_number;
        //TODO: handle basefee update
        //let basefee_ordering = self.update_basefee(pending_basefee);

        self.all_transactions.block_gas_limit = block_gas_limit;
        //TODO: handle blob fee update
        // if let Some(blob_fee) = pending_blob_fee {
        //     self.update_blob_fee(blob_fee, basefee_ordering)
        // }
    }

    pub(crate) fn add_transaction(
        &mut self,
        tx: ValidPoolTransaction<T>,
        _on_chain_balance: U256,
        on_chain_nonce: u64,
        on_chain_code_hash: Option<B256>,
    ) -> PoolResult<TxHash> {
        if self.contains(tx.hash()) {
            return Err(PoolError::new(*tx.hash(), PoolErrorKind::AlreadyImported));
        }
        self.validate_auth(&tx, on_chain_nonce, on_chain_code_hash)?;
        // Update metrics
        self.metrics.queued_transactions.increment(1);

        Ok(*tx.hash())
    }

    // /// Determines if the tx sender is delegated or has a  pending delegation, and if so, ensures
    // /// they have at most one in-flight **executable** transaction, e.g. disallow stacked and
    // /// nonce-gapped transactions from the account.
    // fn check_delegation_limit(
    //     &self,
    //     transaction: &ValidPoolTransaction<T::Transaction>,
    //     on_chain_nonce: u64,
    //     on_chain_code_hash: Option<B256>,
    // ) -> Result<(), PoolError> {
    //     // Short circuit if the sender has neither delegation nor pending delegation.
    //     if (on_chain_code_hash.is_none() || on_chain_code_hash == Some(KECCAK_EMPTY))
    //         && !self
    //             .all_transactions
    //             .auths
    //             .contains_key(&transaction.sender_id())
    //     {
    //         return Ok(());
    //     }

    //     let mut txs_by_sender = self
    //         .pending_pool
    //         .iter_txs_by_sender(transaction.sender_id())
    //         .peekable();

    //     if txs_by_sender.peek().is_none() {
    //         // Transaction with gapped nonce is not supported for delegated accounts
    //         if transaction.nonce() > on_chain_nonce {
    //             return Err(PoolError::new(
    //                 *transaction.hash(),
    //                 PoolErrorKind::InvalidTransaction(InvalidPoolTransactionError::Eip7702(
    //                     Eip7702PoolTransactionError::OutOfOrderTxFromDelegated,
    //                 )),
    //             ));
    //         }
    //         return Ok(());
    //     }

    //     if txs_by_sender.any(|id| id == &transaction.transaction_id) {
    //         // Transaction replacement is supported
    //         return Ok(());
    //     }

    //     Err(PoolError::new(
    //         *transaction.hash(),
    //         PoolErrorKind::InvalidTransaction(InvalidPoolTransactionError::Eip7702(
    //             Eip7702PoolTransactionError::InflightTxLimitReached,
    //         )),
    //     ))
    // }

    /// This verifies that the transaction complies with code authorization
    /// restrictions brought by EIP-7702 transaction type:
    /// 1. Any account with a deployed delegation or an in-flight authorization to deploy a
    ///    delegation will only be allowed a single transaction slot instead of the standard limit.
    ///    This is due to the possibility of the account being sweeped by an unrelated account.
    /// 2. In case the pool is tracking a pending / queued transaction from a specific account, it
    ///    will reject new transactions with delegations from that account with standard in-flight
    ///    transactions.
    fn validate_auth(
        &self,
        _transaction: &ValidPoolTransaction<T>,
        _on_chain_nonce: u64,
        _on_chain_code_hash: Option<B256>,
    ) -> Result<(), PoolError> {
        //TODO: implement this
        // Allow at most one in-flight tx for delegated accounts or those with a
        // pending authorization.
        // self.check_delegation_limit(transaction, on_chain_nonce, on_chain_code_hash)?;

        // if let Some(authority_list) = &transaction.authority_ids {
        //     for sender_id in authority_list {
        //         if self.all_transactions.txs_iter(*sender_id).next().is_some() {
        //             return Err(PoolError::new(
        //                 *transaction.hash(),
        //                 PoolErrorKind::InvalidTransaction(InvalidPoolTransactionError::Eip7702(
        //                     Eip7702PoolTransactionError::AuthorityReserved,
        //                 )),
        //             ));
        //         }
        //     }
        // }

        Ok(())
    }

    /// Check if a transaction with the given hash exists in the pool
    pub fn contains(&self, hash: &TxHash) -> bool {
        // Simple implementation - in reality this would check both pools
        self.all_transactions.contains(hash)
    }

    /// Get the total number of transactions in the pool
    pub fn len(&self) -> usize {
        // Simple implementation - in reality this would count both pools
        self.all_transactions.len()
    }

    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Mission method - processes transactions for consensus
    pub fn mission(&mut self) -> PoolResult<Vec<TxHash>> {
        let processed_hashes = Vec::new();

        // This is where consensus-specific logic would go
        // For now, return an empty vector
        // In a real implementation, this would:
        // 1. Get the best transactions from the pending pool
        // 2. Process them according to consensus rules
        // 3. Return the hashes of processed transactions

        Ok(processed_hashes)
    }

    /// Remove a transaction from the pool
    pub fn remove_transaction(&mut self, _hash: &TxHash) -> Option<ValidPoolTransaction<T>> {
        // Simple implementation - in reality this would remove from the appropriate pool
        None
    }
}
impl<T: PoolTransaction> MysticetiPool for TxPool<T> {
    /// Add committed subdags to queue and proposal pool
    /// If the committed index is the same as the next committed index, add to proposal pool
    /// Otherwise, add to committed queue
    /// Update next committed index
    fn add_committed_subdags(&self, committed_subdags: Vec<MysticetiCommittedSubdag<T>>) {
        let len = committed_subdags.len();
        let mut committed_queue = self.commited_queue.write();

        let (mut next_committed_index, mut proposal_pool_size) = {
            let proposal_pool = self.proposal_pool.read();
            let pool_size = proposal_pool.len();
            let next_index = proposal_pool
                .back()
                .map(|subdag| subdag.commit_ref.index as u64)
                .unwrap_or_else(|| {
                    let next_committed_index = self.next_committed_index.read();
                    *next_committed_index
                });
            (next_index, pool_size)
        };
        for committed_subdag in committed_subdags {
            if proposal_pool_size < self.config.committed_subdags_per_block
                && committed_subdag.commit_ref.index as u64 == next_committed_index
            {
                self.proposal_pool.write().push_back(committed_subdag);
                next_committed_index += 1;
                proposal_pool_size += 1;
            } else {
                committed_queue.insert(committed_subdag.commit_ref.index as u64, committed_subdag);
            }
        }
        *self.next_committed_index.write() = next_committed_index;
        debug!(
            "Added {} committed subdags to queue. Queue size: {:?}",
            len,
            committed_queue.len()
        );
    }
}
impl<T: PoolTransaction> fmt::Debug for TxPool<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TxPool")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Container for _all_ transaction in the pool.
///
/// This is the sole entrypoint that's guarding all sub-pools, all sub-pool actions are always
/// derived from this set. Updates returned from this type must be applied to the sub-pools.
pub(crate) struct AllTransactions<T: PoolTransaction> {
    /// Minimum base fee required by the protocol.
    ///
    /// Transactions with a lower base fee will never be included by the chain
    minimal_protocol_basefee: u64,
    /// The max gas limit of the block
    block_gas_limit: u64,
    /// Max number of executable transaction slots guaranteed per account
    max_account_slots: usize,
    /// _All_ transactions identified by their hash.
    by_hash: HashMap<TxHash, Arc<ValidPoolTransaction<T>>>,
    /// Tracks the number of transactions by sender that are currently in the pool.
    tx_counter: FxHashMap<Address, usize>,
    /// The current block number the pool keeps track of.
    last_seen_block_number: u64,
    /// The current block hash the pool keeps track of.
    last_seen_block_hash: B256,
    /// Expected blob and base fee for the pending block.
    pending_fees: PendingFees,
}

impl<T: PoolTransaction> AllTransactions<T> {
    /// Create a new instance
    fn new() -> Self {
        Self {
            block_gas_limit: 0,
            by_hash: HashMap::new(),
            last_seen_block_number: 0,
            last_seen_block_hash: B256::ZERO,
            ..Default::default()
        }
    }
    /// Returns if the transaction for the given hash is already included in this pool
    pub(crate) fn contains(&self, tx_hash: &TxHash) -> bool {
        self.by_hash.contains_key(tx_hash)
    }
    /// Number of transactions in the entire pool
    pub(crate) fn len(&self) -> usize {
        self.by_hash.len()
    }

    /// Whether the pool is empty
    pub(crate) fn is_empty(&self) -> bool {
        self.by_hash.is_empty()
    }
}

impl<T: PoolTransaction> Default for AllTransactions<T> {
    fn default() -> Self {
        Self {
            max_account_slots: TXPOOL_MAX_ACCOUNT_SLOTS_PER_SENDER,
            minimal_protocol_basefee: MIN_PROTOCOL_BASE_FEE,
            block_gas_limit: ETHEREUM_BLOCK_GAS_LIMIT_30M,
            by_hash: Default::default(),
            tx_counter: Default::default(),
            last_seen_block_number: Default::default(),
            last_seen_block_hash: Default::default(),
            pending_fees: Default::default(),
        }
    }
}

/// Represents updated fees for the pending block.
#[derive(Debug, Clone)]
pub(crate) struct PendingFees {
    /// The pending base fee
    pub(crate) base_fee: u64,
    /// The pending blob fee
    pub(crate) blob_fee: u128,
}

impl Default for PendingFees {
    fn default() -> Self {
        Self {
            base_fee: Default::default(),
            blob_fee: BLOB_TX_MIN_BLOB_GASPRICE,
        }
    }
}

// Additional methods for TxPool to support the TransactionPool trait
impl<T: PoolTransaction> TxPool<T> {
    /// Get transactions by sender
    pub fn get_transactions_by_sender(&self, sender: Address) -> Vec<Arc<ValidPoolTransaction<T>>> {
        self.all_transactions
            .by_hash
            .values()
            .filter(|tx| tx.sender() == sender)
            .cloned()
            .collect()
    }

    /// Get pending transactions with predicate
    pub fn get_pending_transactions_with_predicate(
        &self,
        mut predicate: impl FnMut(&ValidPoolTransaction<T>) -> bool,
    ) -> Vec<Arc<ValidPoolTransaction<T>>> {
        self.all_transactions
            .by_hash
            .values()
            .filter(|tx| predicate(tx))
            .cloned()
            .collect()
    }

    /// Get pending transactions by sender
    pub fn get_pending_transactions_by_sender(
        &self,
        sender: Address,
    ) -> Vec<Arc<ValidPoolTransaction<T>>> {
        self.all_transactions
            .by_hash
            .values()
            .filter(|tx| tx.sender() == sender)
            .cloned()
            .collect()
    }

    /// Get queued transactions by sender
    pub fn get_queued_transactions_by_sender(
        &self,
        sender: Address,
    ) -> Vec<Arc<ValidPoolTransaction<T>>> {
        self.all_transactions
            .by_hash
            .values()
            .filter(|tx| tx.sender() == sender)
            .cloned()
            .collect()
    }

    /// Get highest transaction by sender
    pub fn get_highest_transaction_by_sender(
        &self,
        sender: Address,
    ) -> Option<Arc<ValidPoolTransaction<T>>> {
        self.all_transactions
            .by_hash
            .values()
            .filter(|tx| tx.sender() == sender)
            .max_by_key(|tx| tx.nonce())
            .cloned()
    }

    /// Get highest consecutive transaction by sender
    pub fn get_highest_consecutive_transaction_by_sender(
        &self,
        sender: Address,
        on_chain_nonce: u64,
    ) -> Option<Arc<ValidPoolTransaction<T>>> {
        self.all_transactions
            .by_hash
            .values()
            .filter(|tx| tx.sender() == sender && tx.nonce() == on_chain_nonce)
            .next()
            .cloned()
    }

    /// Get transactions by origin
    pub fn get_transactions_by_origin(
        &self,
        _origin: TransactionOrigin,
    ) -> Vec<Arc<ValidPoolTransaction<T>>> {
        self.all_transactions.by_hash.values().cloned().collect()
    }

    /// Get pending transactions by origin
    pub fn get_pending_transactions_by_origin(
        &self,
        _origin: TransactionOrigin,
    ) -> Vec<Arc<ValidPoolTransaction<T>>> {
        self.all_transactions.by_hash.values().cloned().collect()
    }

    /// Get queued transactions max
    pub fn get_queued_transactions_max(&self, max: usize) -> Vec<Arc<ValidPoolTransaction<T>>> {
        self.all_transactions
            .by_hash
            .values()
            .take(max)
            .cloned()
            .collect()
    }

    /// Get sender ID
    pub fn get_sender_id(&self, _sender: Address) -> reth_transaction_pool::identifier::SenderId {
        // Simplified implementation - you may need to expand this
        reth_transaction_pool::identifier::SenderId::from(1)
    }
}
