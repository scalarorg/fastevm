use reth_transaction_pool::{
    metrics::TxPoolMetrics,
    pool::{ParkedPool, QueuedOrd},
    PoolConfig, TransactionOrdering,
};

pub struct TxPool<T: TransactionOrdering> {
    /// pending subpool
    ///
    /// Holds transactions that are ready to be executed on the current state.
    pending_pool: PendingPool<T>,
    /// Pool settings to enforce limits etc.
    config: PoolConfig,
    /// queued subpool
    ///
    /// Holds all parked transactions that depend on external changes from the sender:
    ///
    ///    - blocked by missing ancestor transaction (has nonce gaps)
    ///    - sender lacks funds to pay for this transaction.
    queued_pool: ParkedPool<QueuedOrd<T::Transaction>>,
    /// Transaction pool metrics
    metrics: TxPoolMetrics,
}

impl<T: TransactionOrdering> TxPool<T> {
    /// Create a new graph pool instance.
    pub fn new(ordering: T, config: PoolConfig) -> Self {
        Self {
            pending_pool: PendingPool::with_buffer(
                ordering,
                config.max_new_pending_txs_notifications,
            ),
            queued_pool: Default::default(),
            config,
            metrics: Default::default(),
        }
    }
    pub(crate) fn add_transaction(
        &mut self,
        tx: ValidPoolTransaction<T::Transaction>,
        on_chain_balance: U256,
        on_chain_nonce: u64,
        on_chain_code_hash: Option<B256>,
    ) -> PoolResult<AddedTransaction<T::Transaction>> {
        if self.contains(tx.hash()) {
            return Err(PoolError::new(*tx.hash(), PoolErrorKind::AlreadyImported));
        }
    }
}
