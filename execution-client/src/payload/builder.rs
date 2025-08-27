//! Payload component configuration for the Ethereum node.

use std::sync::Arc;

use reth_basic_payload_builder::{
    BuildArguments, BuildOutcome, MissingPayloadBehaviour, PayloadBuilder, PayloadConfig,
};
use reth_ethereum::{
    chainspec::EthereumHardforks,
    evm::EthEvmConfig,
    pool::{
        BestTransactions, BestTransactionsAttributes, PoolTransaction, TransactionOrigin,
        TransactionPool, ValidPoolTransaction,
    },
    provider::ChainSpecProvider,
    storage::StateProviderFactory,
    EthPrimitives, TransactionSigned,
};

use reth_ethereum_payload_builder::{default_ethereum_payload, EthereumBuilderConfig};
use reth_evm::{ConfigureEvm, NextBlockEnvAttributes};
use reth_payload_builder::{EthBuiltPayload, EthPayloadBuilderAttributes, PayloadBuilderError};
use tracing::debug;

/// Mysticeti payload builder that processes only local transactions.
/// Modifi reth_ethereum_payload_builder::EthereumPayloadBuilder by filter transaction with origin = Local
///
/// This payload builder extends the standard Ethereum payload builder with custom logic
/// to ensure that only transactions originating from the local node (`TransactionOrigin::Local`)
/// are included in the payload. This is useful for consensus mechanisms where nodes should
/// only process their own transactions.
///
/// The builder maintains all the standard Ethereum payload building functionality while
/// overriding the transaction selection to filter for local transactions only.
///
/// # Key Features
///
/// * Filters transactions to include only those with `TransactionOrigin::Local`
/// * Maintains proper transaction ordering and pool selection logic
/// * Preserves all standard Ethereum payload building capabilities
/// * Integrates seamlessly with the existing Reth node architecture

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MysticetiPayloadBuilder<Pool, Client, EvmConfig = EthEvmConfig> {
    /// Client providing access to node state.
    client: Client,
    /// Transaction pool.
    pool: Pool,
    /// The type responsible for creating the evm.
    evm_config: EvmConfig,
    /// Payload builder configuration.
    builder_config: EthereumBuilderConfig,
}

impl<Pool: Clone, Client: Clone, EvmConfig: Clone>
    MysticetiPayloadBuilder<Pool, Client, EvmConfig>
{
    /// Creates a new Mysticeti payload builder instance.
    ///
    /// This constructor initializes both the custom Mysticeti payload builder and the
    /// inner Ethereum payload builder. The inner builder is used for fallback scenarios
    /// and standard functionality, while the custom logic handles local transaction filtering.
    ///
    /// # Arguments
    ///
    /// * `client` - The client providing access to node state
    /// * `pool` - The transaction pool to query for transactions
    /// * `evm_config` - Configuration for creating the EVM instance
    /// * `builder_config` - Configuration for the Ethereum payload builder
    ///
    /// # Returns
    ///
    /// A new instance of `MysticetiPayloadBuilder`
    pub fn new(
        client: Client,
        pool: Pool,
        evm_config: EvmConfig,
        builder_config: EthereumBuilderConfig,
    ) -> Self {
        Self {
            client,
            pool,
            evm_config,
            builder_config,
        }
    }
}
/// Retrieves and filters the best transactions from the pool to include only local transactions.
///
/// This function is the core custom logic for the Mysticeti payload builder. It ensures that
/// only transactions with `TransactionOrigin::Local` are included in the payload, which means
/// only transactions that originated from this node will be processed.
///
/// The function works by:
/// 1. Getting the best transactions from the pool with the specified attributes
/// 2. Filtering them to keep only local transactions using `tx.is_local()`
/// 3. Returning a boxed iterator that implements `BestTransactions`
///
/// This approach maintains the proper transaction ordering and respects the pool's
/// best transaction selection while ensuring local-only processing.
///
/// # Arguments
///
/// * `pool` - The transaction pool to query for transactions
/// * `attributes` - Attributes that define how to select the best transactions
///
/// # Returns
///
/// A boxed iterator over local transactions that implements `BestTransactions`
fn get_best_transactions<
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>,
>(
    pool: &Pool,
    attributes: BestTransactionsAttributes,
) -> Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<Pool::Transaction>>>> {
    debug!("[MysticetiPayloadBuilder] get_best_transactions");
    // Get all best transactions first, then filter for local ones
    // This approach ensures we maintain the proper ordering and attributes while filtering
    let best_transactions = pool.best_transactions_with_attributes(attributes);
    // Filter to keep only local transactions using the proper BestTransactions filter method
    // This ensures the payload builder only processes transactions that originated from this node
    let local_transactions = best_transactions.filter_transactions(|tx| tx.is_local());
    Box::new(local_transactions)
}

/// Implementation of the PayloadBuilder trait for MysticetiPayloadBuilder.
///
/// This implementation overrides the standard Ethereum payload building process to use
/// our custom `get_best_transactions` function, which ensures only local transactions
/// are included in the payload. The `on_missing_payload` method delegates to the inner
/// builder for fallback behavior.
///
/// # Key Overrides
///
/// * `try_build` - Uses custom local transaction filtering via `get_best_transactions`
/// * `build_empty_payload` - Also uses local transaction filtering for empty payloads
/// * `on_missing_payload` - Delegates to the inner Ethereum payload builder
impl<Pool, Client, EvmConfig> PayloadBuilder for MysticetiPayloadBuilder<Pool, Client, EvmConfig>
where
    EvmConfig: ConfigureEvm<Primitives = EthPrimitives, NextBlockEnvCtx = NextBlockEnvAttributes>,
    Client: StateProviderFactory + ChainSpecProvider<ChainSpec: EthereumHardforks> + Clone,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>,
{
    type Attributes = EthPayloadBuilderAttributes;
    type BuiltPayload = EthBuiltPayload;

    fn try_build(
        &self,
        args: BuildArguments<EthPayloadBuilderAttributes, EthBuiltPayload>,
    ) -> Result<BuildOutcome<EthBuiltPayload>, PayloadBuilderError> {
        default_ethereum_payload(
            self.evm_config.clone(),
            self.client.clone(),
            self.pool.clone(),
            self.builder_config.clone(),
            args,
            |attributes| get_best_transactions(&self.pool, attributes),
        )
    }

    fn on_missing_payload(
        &self,
        _args: BuildArguments<Self::Attributes, Self::BuiltPayload>,
    ) -> MissingPayloadBehaviour<Self::BuiltPayload> {
        if self.builder_config.await_payload_on_missing {
            MissingPayloadBehaviour::AwaitInProgress
        } else {
            MissingPayloadBehaviour::RaceEmptyPayload
        }
    }

    fn build_empty_payload(
        &self,
        config: PayloadConfig<Self::Attributes>,
    ) -> Result<EthBuiltPayload, PayloadBuilderError> {
        let args = BuildArguments::new(Default::default(), config, Default::default(), None);

        default_ethereum_payload(
            self.evm_config.clone(),
            self.client.clone(),
            self.pool.clone(),
            self.builder_config.clone(),
            args,
            |attributes| get_best_transactions(&self.pool, attributes),
        )?
        .into_payload()
        .ok_or_else(|| PayloadBuilderError::MissingPayload)
    }
}
