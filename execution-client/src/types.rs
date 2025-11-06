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
