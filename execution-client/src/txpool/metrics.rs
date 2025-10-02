use reth_metrics::metrics;
use reth_metrics::{
    metrics::{Counter, Gauge, Histogram},
    Metrics,
};

/// Transaction pool metrics
#[derive(Metrics)]
#[metrics(scope = "transaction_pool")]
pub struct TxPoolMetrics {
    /// Number of queued subdags
    pub queued_subdags: Counter,
    /// Number of queued transactions
    pub queued_transactions: Counter,
    /// Number of pending transactions
    pub pending_transactions: Counter,
    /// Number of proposal subdags
    pub proposal_subdags: Counter,
}

/// All Transactions metrics
#[derive(Metrics)]
#[metrics(scope = "transaction_pool")]
pub struct AllTransactionsMetrics {
    /// Number of all transactions by hash in the pool
    pub(crate) all_transactions_by_hash: Gauge,
    // /// Number of all transactions by id in the pool
    // pub(crate) all_transactions_by_id: Gauge,
    // /// Number of all transactions by all senders in the pool
    // pub(crate) all_transactions_by_all_senders: Gauge,
    // /// Number of blob transactions nonce gaps.
    // pub(crate) blob_transactions_nonce_gaps: Counter,
    // /// The current blob base fee
    // pub(crate) blob_base_fee: Gauge,
    // /// The current base fee
    // pub(crate) base_fee: Gauge,
}
