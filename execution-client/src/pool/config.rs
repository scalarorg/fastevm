use reth_transaction_pool::PoolConfig;

/// Configuration options for the Transaction pool.
#[derive(Debug, Clone)]
pub struct MysticetiPoolConfig {
    pub pool_config: PoolConfig,
    pub committed_subdags_per_block: usize,
}
impl MysticetiPoolConfig {
    pub fn new(pool_config: PoolConfig, committed_subdags_per_block: usize) -> Self {
        Self {
            pool_config,
            committed_subdags_per_block,
        }
    }
}
impl Default for MysticetiPoolConfig {
    fn default() -> Self {
        Self {
            pool_config: PoolConfig::default(),
            committed_subdags_per_block: 30,
        }
    }
}
