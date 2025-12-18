use reth_ethereum::chainspec::EthereumHardforks;
use reth_provider::{ChainSpecProvider, StateProviderFactory};
use reth_transaction_pool::{BlobStore, PoolConfig};

/// Configuration for validator reconstruction in tx_listener
#[derive(Debug, Clone)]
pub struct TxValidatorConfig<
    Client: ChainSpecProvider<ChainSpec: EthereumHardforks> + StateProviderFactory + 'static,
    S: BlobStore,
> {
    pub provider: Client,
    pub head_timestamp: u64,
    pub max_tx_input_bytes: usize,
    pub tx_fee_cap: u128,
    pub max_tx_gas_limit: Option<u64>,
    pub minimum_priority_fee: Option<u128>,
    pub additional_validation_tasks: usize,
    pub pool_config: PoolConfig,
    pub blob_store: S,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        
        // Verify PoolConfig can be created with defaults
        // Use the actual field names from the current API
        assert!(config.pending_limit.max_size > 0);
        assert!(config.queued_limit.max_size > 0);
    }

    #[test]
    fn test_pool_config_clone() {
        let config = PoolConfig::default();
        let cloned = config.clone();
        
        assert_eq!(config.pending_limit.max_size, cloned.pending_limit.max_size);
        assert_eq!(config.queued_limit.max_size, cloned.queued_limit.max_size);
    }

    #[test]
    fn test_pool_config_max_tx_lifetime() {
        let config = PoolConfig::default();
        
        // Default max_queued_lifetime should be set
        assert!(config.max_queued_lifetime > std::time::Duration::ZERO);
    }

    #[test]
    fn test_pool_config_local_transactions() {
        let config = PoolConfig::default();
        
        // local_transactions_config should be accessible
        let _ = config.local_transactions_config.clone();
    }
}
