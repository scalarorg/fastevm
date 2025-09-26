use reth_ethereum::primitives::transaction::error::InvalidTransactionError;
use reth_transaction_pool::{
    error::InvalidPoolTransactionError, BestTransactions, PoolTransaction, ValidPoolTransaction,
};
use std::sync::Arc;
use tracing::debug;

pub struct BestMysticetiTransactions<T: PoolTransaction> {
    reth_best_txs: Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<T>>>>,
    mysticeti_txs: Vec<Arc<ValidPoolTransaction<T>>>,
}

impl<T: PoolTransaction> BestMysticetiTransactions<T> {
    pub fn new(
        reth_best_txs: Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<T>>>>,
        mysticeti_txs: Vec<Arc<ValidPoolTransaction<T>>>,
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
            "Mark transaction as invalid {:?}: {:?}",
            transaction.hash(),
            kind
        );
        match kind {
            InvalidPoolTransactionError::Consensus(
                InvalidTransactionError::TxTypeNotSupported { .. },
            ) => {
                // /// Identifier for legacy transaction, however a legacy tx is technically not
                // /// typed.
                // pub const LEGACY_TX_TYPE_ID: u8 = 0;

                // /// Identifier for an EIP2930 transaction.
                // pub const EIP2930_TX_TYPE_ID: u8 = 1;

                // /// Identifier for an EIP1559 transaction.
                // pub const EIP1559_TX_TYPE_ID: u8 = 2;

                // /// Identifier for an EIP4844 transaction.
                // pub const EIP4844_TX_TYPE_ID: u8 = 3;

                // /// Identifier for an EIP7702 transaction.
                // pub const EIP7702_TX_TYPE_ID: u8 = 4;
                debug!(
                    "Transaction {:?} has type {:?}",
                    transaction.hash(),
                    transaction.tx_type()
                );
            }
            _ => {}
        }
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
        let consensus_tx = self.mysticeti_txs.pop()?;
        // 2 Pick first transaction from reth best transactions
        // This for debug purposes only
        // We cannot make sure the pending lists are the same among reth peer nodes
        // Because of the times insert to pending subpool are not the same => base fees can be different
        // if let Some(next_item) = next_item {
        //     debug!("Reth best transaction: {:?}", next_item.hash());
        // } else {
        //     debug!(
        //         "Reth best transaction not found. Using consensus transaction: {:?}",
        //         consensus_tx.hash()
        //     );
        // }
        Some(consensus_tx)
    }
}
