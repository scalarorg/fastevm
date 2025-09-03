//! Payload component configuration for the Ethereum node.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use reth_basic_payload_builder::{
    BuildArguments, BuildOutcome, MissingPayloadBehaviour, PayloadBuilder, PayloadConfig,
};
use reth_errors::RethError;
use reth_ethereum::{
    chainspec::EthereumHardforks,
    evm::EthEvmConfig,
    pool::{
        BestTransactions, BestTransactionsAttributes, PoolTransaction, TransactionPool,
        ValidPoolTransaction,
    },
    provider::ChainSpecProvider,
    storage::StateProviderFactory,
    EthPrimitives, TransactionSigned,
};
use reth_ethereum_payload_builder::{default_ethereum_payload, EthereumBuilderConfig};
use reth_evm::{ConfigureEvm, NextBlockEnvAttributes};
use reth_extension::CommittedSubDag;
use reth_payload_builder::{EthBuiltPayload, EthPayloadBuilderAttributes, PayloadBuilderError};
use tracing::{debug, warn};

/// Mysticeti payload builder that processes transactions from the subdag instead of the pool.
/// Modified reth_ethereum_payload_builder::EthereumPayloadBuilder to use subdag transactions
///
/// This payload builder extends the standard Ethereum payload builder with custom logic
/// to ensure that transactions are extracted from the committed subdag rather than from
/// the local transaction pool. This is useful for consensus mechanisms where nodes should
/// process transactions that have been committed through the consensus protocol.
///
/// The builder maintains all the standard Ethereum payload building functionality while
/// overriding the transaction selection to use subdag transactions instead of pool transactions.
///
/// # Key Features
///
/// * Extracts transactions from committed subdag blocks
/// * Converts consensus transactions to Ethereum transaction format
/// * Maintains proper transaction ordering and payload building logic
/// * Preserves all standard Ethereum payload building capabilities
/// * Integrates seamlessly with the existing Reth node architecture

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MysticetiPayloadBuilder<Pool, Client, EvmConfig = EthEvmConfig> {
    /// Client providing access to node state.
    client: Client,
    /// Transaction pool.
    pool: Pool,
    /// The type responsible for creating the evm.
    evm_config: EvmConfig,
    /// Subdag queue.
    subdag_queue: Arc<Mutex<VecDeque<CommittedSubDag>>>,
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
    /// and standard functionality, while the custom logic handles subdag transaction processing.
    ///
    /// # Arguments
    ///
    /// * `client` - The client providing access to node state
    /// * `pool` - The transaction pool (used for fallback scenarios)
    /// * `subdag_queue` - Queue of committed subdags containing transactions to process
    /// * `evm_config` - Configuration for creating the EVM instance
    /// * `builder_config` - Configuration for the Ethereum payload builder
    ///
    /// # Returns
    ///
    /// A new instance of `MysticetiPayloadBuilder`
    pub fn new(
        client: Client,
        pool: Pool,
        subdag_queue: Arc<Mutex<VecDeque<CommittedSubDag>>>,
        evm_config: EvmConfig,
        builder_config: EthereumBuilderConfig,
    ) -> Self {
        Self {
            client,
            pool,
            subdag_queue,
            evm_config,
            builder_config,
        }
    }
}
/// Retrieves and filters the best transactions from the subdag instead of the pool.
///
/// This function is the core custom logic for the Mysticeti payload builder. It extracts
/// transactions from the committed subdag and converts them to a format compatible with
/// the payload builder, instead of getting transactions from the transaction pool.
///
/// The function works by:
/// 1. Creating a SubDagTransactions iterator from the provided subdag
/// 2. Converting consensus transactions to Ethereum transactions
/// 3. Returning a boxed iterator that implements `BestTransactions`
///
/// This approach ensures that the payload builder processes transactions that have been
/// committed through the consensus mechanism rather than from the local transaction pool.
///
/// # Arguments
///
/// * `_pool` - The transaction pool (unused in this implementation)
/// * `subdag` - The committed subdag containing transactions to process
/// * `_attributes` - Attributes that define how to select the best transactions (unused)
///
/// # Returns
///
/// A boxed iterator over transactions from the subdag that implements `BestTransactions`
fn get_best_transactions<
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>,
>(
    pool: &Pool,
    subdag: CommittedSubDag,
    attributes: BestTransactionsAttributes,
) -> Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<Pool::Transaction>>>> {
    debug!("[MysticetiPayloadBuilder] get_best_transactions from subdag");

    // Create a SubDagTransactions iterator from the subdag
    // This extracts transactions from the consensus mechanism instead of the pool
    let subdag_transactions = subdag.flatten_transactions();
    // Get all default best transactions first,
    // Then convert it to a HashMap with transaction hash as key and transaction as value
    let uncommitted_transactions = pool.best_transactions_with_attributes(attributes);
    let best_transactions = super::best::CommittedTransactions::<Pool::Transaction>::new(
        uncommitted_transactions,
        subdag_transactions,
    );
    Box::new(best_transactions)
}

/// Implementation of the PayloadBuilder trait for MysticetiPayloadBuilder.
///
/// This implementation overrides the standard Ethereum payload building process to use
/// our custom `get_best_transactions` function, which ensures transactions are extracted
/// from the committed subdag instead of the transaction pool. The `on_missing_payload`
/// method delegates to the inner builder for fallback behavior.
///
/// # Key Overrides
///
/// * `try_build` - Uses custom subdag transaction extraction via `get_best_transactions`
/// * `build_empty_payload` - Also uses subdag transaction extraction for empty payloads
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
        let subdag = {
            let mut queue_guard = self
                .subdag_queue
                .lock()
                .map_err(|e| PayloadBuilderError::Internal(RethError::msg(e)))?;
            queue_guard.pop_front()
        };
        match subdag {
            Some(subdag) => default_ethereum_payload(
                self.evm_config.clone(),
                self.client.clone(),
                self.pool.clone(),
                self.builder_config.clone(),
                args,
                |attributes| get_best_transactions(&self.pool, subdag, attributes),
            ),
            None => Err(PayloadBuilderError::Internal(RethError::msg(
                "No subdag found in queue",
            ))),
        }
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
            |attributes| get_best_transactions(&self.pool, CommittedSubDag::default(), attributes),
        )?
        .into_payload()
        .ok_or_else(|| PayloadBuilderError::MissingPayload)
    }
}
