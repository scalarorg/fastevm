use reth_transaction_pool::{
    error::{Eip4844PoolTransactionError, InvalidPoolTransactionError},
    identifier::{SenderId, TransactionId},
    BestTransactions, PoolTransaction, ValidPoolTransaction,
};
use rustc_hash::FxHashMap;
use std::{
    collections::{hash_map::Entry, BTreeMap, HashSet},
    sync::Arc,
};
use tokio::sync::broadcast::Receiver;
use tracing::{debug, warn};

/// An iterator that returns transactions that can be executed on the current state (*best*
/// transactions).
///
/// The [`PendingPool`](crate::pool::pending::PendingPool) contains transactions that *could* all
/// be executed on the current state, but only yields transactions that are ready to be executed
/// now. While it contains all gapless transactions of a sender, it _always_ only returns the
/// transaction with the current on chain nonce.
#[derive(Debug)]
pub struct BestMysticetiTransactions<T: PoolTransaction> {
    /// Contains a copy of _all_ transactions from pending pool ordered by commited subdag
    /// and by nonce of each sender.
    pub(crate) ordered: BTreeMap<TransactionId, Arc<ValidPoolTransaction<T>>>,

    /// Highest nonce for each sender
    pub(crate) highest_nonces: FxHashMap<SenderId, Arc<ValidPoolTransaction<T>>>,

    /// There might be the case where a yielded transactions is invalid, this will track it.
    pub(crate) invalid: HashSet<SenderId>,
    /// Used to receive any new pending transactions that have been added to the pool after this
    /// iterator was static filtered
    ///
    /// These new pending transactions are inserted into this iterator's pool before yielding the
    /// next value
    pub(crate) new_transaction_receiver: Option<Receiver<Arc<ValidPoolTransaction<T>>>>,
    /// Flag to control whether to skip blob transactions (EIP4844).
    pub(crate) skip_blobs: bool,
}

impl<T: PoolTransaction> BestMysticetiTransactions<T> {
    pub fn new(
        reth_best_txs: Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<T>>>>,
        subdag_transactions: Vec<T>,
    ) -> Self {
        let mut best_transactions = Self {
            ordered: BTreeMap::new(),
            highest_nonces: FxHashMap::default(),
            invalid: HashSet::new(),
            new_transaction_receiver: None,
            skip_blobs: false,
        };
        debug!(
            "Create best transactions with {} subdag transactions",
            subdag_transactions.len()
        );
        let added_count =
            best_transactions.populate_transactions(reth_best_txs, subdag_transactions);
        debug!("Added {} subdag transactions to pool", added_count);
        best_transactions
    }
    pub(crate) fn populate_transactions(
        &mut self,
        mut reth_best_txs: Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<T>>>>,
        subdag_transactions: Vec<T>,
    ) -> usize {
        let mut added_count = 0;
        for commtted_tx in subdag_transactions {
            //let tx_hash = TxHash::from_slice(&tx_bytes);
            if let Some(tx) = reth_best_txs
                .find(|pool_tx: &Arc<ValidPoolTransaction<T>>| pool_tx.hash() == commtted_tx.hash())
            {
                added_count += 1;
                self.add_transaction(Arc::clone(&tx));
            } else {
                //This branch must not happen
                warn!(
                    "Transaction not found in transactions pool: {:?}",
                    commtted_tx.hash()
                );
            }
        }
        added_count
    }
}
impl<T: PoolTransaction> BestMysticetiTransactions<T> {
    /// Mark the transaction and it's descendants as invalid.
    pub(crate) fn mark_invalid(
        &mut self,
        tx: &Arc<ValidPoolTransaction<T>>,
        _kind: InvalidPoolTransactionError,
    ) {
        self.invalid.insert(tx.transaction_id.sender);
    }

    // Removes the currently best independent transaction from the independent set and the total set.
    fn pop_next(&mut self) -> Option<Arc<ValidPoolTransaction<T>>> {
        self.ordered.pop_first().map(|(_, tx)| tx)
    }
}
impl<T: PoolTransaction> BestMysticetiTransactions<T> {
    /// Add a transaction to the committed transactions
    /// Returns true if the transaction is added to the ordered list, false if it is added to the unordered list
    pub fn add_transaction(&mut self, tx: Arc<ValidPoolTransaction<T>>) -> bool {
        //Order transaction by nonce
        match self.highest_nonces.entry(tx.transaction_id.sender) {
            Entry::Occupied(mut entry) => {
                if entry.get().transaction.nonce() + 1 == tx.transaction.nonce() {
                    self.ordered.insert(tx.transaction_id.clone(), tx.clone());
                    *entry.get_mut() = tx.clone();
                    return true;
                } else {
                    debug!(
                        "Transaction nonce {:?} is not the next nonce for sender: {:?}. Expected nonce: {:?}",
                        tx.transaction.nonce(),
                        tx.transaction_id.sender,
                        entry.get().transaction.nonce() + 1
                    );
                    return false;
                }
            }
            Entry::Vacant(entry) => {
                self.ordered.insert(tx.transaction_id.clone(), tx.clone());
                entry.insert(tx.clone());
                return true;
            }
        }
    }
}
impl<T: PoolTransaction> BestTransactions for BestMysticetiTransactions<T> {
    fn mark_invalid(&mut self, tx: &Self::Item, kind: InvalidPoolTransactionError) {
        Self::mark_invalid(self, tx, kind)
    }

    fn no_updates(&mut self) {
        self.new_transaction_receiver.take();
    }

    fn skip_blobs(&mut self) {
        self.set_skip_blobs(true);
    }

    fn set_skip_blobs(&mut self, skip_blobs: bool) {
        self.skip_blobs = skip_blobs;
    }
}

impl<T: PoolTransaction> Iterator for BestMysticetiTransactions<T> {
    type Item = Arc<ValidPoolTransaction<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Remove the next independent tx with the highest priority
            let best = self.pop_next()?;
            let sender_id = best.transaction_id.sender;

            // skip transactions for which sender was marked as invalid
            if self.invalid.contains(&sender_id) {
                debug!(
                    target: "txpool",
                    "[{:?}] skipping invalid transaction",
                    best.transaction.hash()
                );
                continue;
            }

            // Insert transactions that just got unlocked.
            // if let Some(unlocked) = self.all.get(&best.unlocks()) {
            //     self.independent.insert(unlocked.clone());
            // }

            if self.skip_blobs && best.transaction.is_eip4844() {
                // blobs should be skipped, marking them as invalid will ensure that no dependent
                // transactions are returned
                self.mark_invalid(
                    &best,
                    InvalidPoolTransactionError::Eip4844(
                        Eip4844PoolTransactionError::NoEip4844Blobs,
                    ),
                )
            } else {
                return Some(best);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_ethereum::pool::noop::NoopTransactionPool;
    use reth_extension::CommittedSubDag;
    use reth_transaction_pool::TransactionPool;

    #[test]
    fn test_committed_transactions_new() {
        let pool = NoopTransactionPool::default();
        let uncommitted_transactions = pool.best_transactions();
        let subdag_transactions = vec![];

        let committed_tx =
            BestMysticetiTransactions::new(uncommitted_transactions, subdag_transactions);

        assert!(committed_tx.ordered.is_empty());
        assert!(committed_tx.highest_nonces.is_empty());
        assert!(committed_tx.invalid.is_empty());
    }

    #[test]
    fn test_committed_transactions_skip_blobs() {
        let pool = NoopTransactionPool::default();
        let uncommitted_transactions = pool.best_transactions();
        let subdag_transactions = vec![];

        let mut committed_tx =
            BestMysticetiTransactions::new(uncommitted_transactions, subdag_transactions);

        // Test skip_blobs functionality
        assert!(!committed_tx.skip_blobs);
        committed_tx.skip_blobs();
        assert!(committed_tx.skip_blobs);

        committed_tx.set_skip_blobs(false);
        assert!(!committed_tx.skip_blobs);
    }

    #[test]
    fn test_committed_transactions_no_updates() {
        let pool = NoopTransactionPool::default();
        let uncommitted_transactions = pool.best_transactions();
        let subdag_transactions = vec![];

        let mut committed_tx =
            BestMysticetiTransactions::new(uncommitted_transactions, subdag_transactions);

        // Test no_updates functionality
        committed_tx.no_updates();
        assert!(committed_tx.new_transaction_receiver.is_none());
    }

    #[test]
    fn test_committed_subdag_operations() {
        let subdag = CommittedSubDag::default();
        assert!(subdag.blocks.is_empty());
        assert!(subdag.flatten_transactions().is_empty());

        let transactions = subdag.flatten_transactions();
        assert!(transactions.is_empty());
    }
}
