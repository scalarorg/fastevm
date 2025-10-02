/// Configuration options for the Transaction pool.
#[derive(Debug, Clone)]
pub struct TxPoolConfig {
    pub max_new_pending_txs_notifications: usize,
}

impl Default for TxPoolConfig {
    fn default() -> Self {
        Self {
            max_new_pending_txs_notifications: 200,
        }
    }
}
