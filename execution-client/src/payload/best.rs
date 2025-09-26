use reth_transaction_pool::{
    error::InvalidPoolTransactionError, BestTransactions, PoolTransaction, ValidPoolTransaction,
};
use std::{collections::VecDeque, sync::Arc};
use tracing::debug;

pub struct BestMysticetiTransactions<T: PoolTransaction> {
    reth_best_txs: Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<T>>>>,
    mysticeti_txs: VecDeque<Arc<ValidPoolTransaction<T>>>,
}

impl<T: PoolTransaction> BestMysticetiTransactions<T> {
    pub fn new(
        reth_best_txs: Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<T>>>>,
        mysticeti_txs: VecDeque<Arc<ValidPoolTransaction<T>>>,
    ) -> Self {
        Self {
            reth_best_txs,
            mysticeti_txs,
        }
    }
}

impl<T: PoolTransaction> BestTransactions for BestMysticetiTransactions<T> {
    fn mark_invalid(&mut self, transaction: &Self::Item, kind: InvalidPoolTransactionError) {
        debug!(
            "Mark invalid transaction {:?}: sender {:?}, nonce {:?}, Invalid reason: {:?}",
            transaction.hash(),
            transaction.sender_ref(),
            transaction.nonce(),
            kind
        );
        self.reth_best_txs.mark_invalid(transaction, kind);
    }

    fn no_updates(&mut self) {
        self.reth_best_txs.no_updates();
    }

    fn set_skip_blobs(&mut self, skip_blobs: bool) {
        self.reth_best_txs.set_skip_blobs(skip_blobs);
    }
}
impl<T: PoolTransaction> Iterator for BestMysticetiTransactions<T> {
    type Item = Arc<ValidPoolTransaction<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        //1 Pick first transaction from mysticeti transactions
        let consensus_tx = self.mysticeti_txs.pop_front()?;
        debug!(
            "BestMysticetiTransactions# Pick transaction: {:?}, sender {:?}, nonce {:?}",
            consensus_tx.hash(),
            consensus_tx.sender_ref(),
            consensus_tx.nonce()
        );
        Some(consensus_tx)
    }
}
