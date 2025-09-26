//! Payload component configuration for the Ethereum node.

use std::{collections::VecDeque, sync::Arc};

use alloy_consensus::BlockHeader;
use reth_basic_payload_builder::{
    BuildArguments, BuildOutcome, MissingPayloadBehaviour, PayloadBuilder, PayloadConfig,
};
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
use reth_payload_builder::{EthBuiltPayload, EthPayloadBuilderAttributes, PayloadBuilderError};
use tracing::debug;

use crate::consensus::ConsensusPool;

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

#[derive(Clone)]
#[non_exhaustive]
pub struct MysticetiPayloadBuilder<Pool: TransactionPool, Client, EvmConfig = EthEvmConfig> {
    /// Client providing access to node state.
    client: Client,
    /// Transaction pool.
    pool: Pool,
    /// The type responsible for creating the evm.
    evm_config: EvmConfig,
    /// Subdag queue.
    consensus_pool: Arc<ConsensusPool<Pool>>,
    /// Payload builder configuration.
    builder_config: EthereumBuilderConfig,
}

impl<Pool: TransactionPool, Client: Clone, EvmConfig: Clone>
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
        consensus_pool: Arc<ConsensusPool<Pool>>,
        evm_config: EvmConfig,
        builder_config: EthereumBuilderConfig,
    ) -> Self {
        Self {
            client,
            pool,
            consensus_pool,
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
    committed_txs: Vec<Arc<Pool::Transaction>>,
    attributes: BestTransactionsAttributes,
) -> Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<Pool::Transaction>>>> {
    // Get all default best transactions first,
    // Then convert it to a HashMap with transaction hash as key and transaction as value
    debug!(
        "[MysticetiPayloadBuilder] get_best_transactions from subdag. Number of committed transactions: {}. Number of reth pending transactions: {}",
        committed_txs.len(),
        pool.pending_transactions().len()
    );
    let best_txs = pool.best_transactions_with_attributes(attributes);
    //Get all transaction from pool
    let mut pooled_txs = VecDeque::new();
    for tx in committed_txs {
        if let Some(tx) = pool.get(tx.hash()) {
            pooled_txs.push_back(tx);
        }
    }

    let best_transactions = super::best::BestMysticetiTransactions::new(best_txs, pooled_txs);
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

    /*
    ## Deep Dive Analysis: Why `try_build` and `default_ethereum_payload` Execute Multiple Times with Same Block Number

    Based on my comprehensive analysis of the Reth codebase, I've identified the root causes for why the `try_build` method of `PayloadBuilder` and `default_ethereum_payload` function are executed multiple times with the same block number. Here's my detailed findings:

    ### **Root Cause Analysis**

    #### 1. **Continuous Payload Building Architecture**

    The primary reason for multiple executions is Reth's **continuous payload building architecture**:

    ```rust
    // From crates/payload/basic/src/lib.rs:383-390
    while this.interval.poll_tick(cx).is_ready() {
        // start a new job if there is no pending block, we haven't reached the deadline,
        // and the payload isn't frozen
        if this.pending_block.is_none() && !this.best_payload.is_frozen() {
            this.spawn_build_job();
        }
    }
    ```

    **Key Points:**
    - **Interval-based building**: By default, payloads are built every **1 second** (`Duration::from_secs(1)`)
    - **Continuous improvement**: Each interval triggers a new `try_build` call to potentially build a better payload
    - **Same block number**: All these builds target the **same parent block** and **same block number** (parent + 1)

    #### 2. **Payload ID Deduplication vs Block Number**

    The system has **payload ID deduplication** but **NOT block number deduplication**:

    ```rust
    // From crates/payload/builder/src/service.rs:401-403
    if this.contains_payload(id) {
        debug!(target: "payload_builder",%id, parent = %attr.parent(), "Payload job already in progress, ignoring.");
    } else {
        // Create new job
    }
    ```

    **Payload ID Generation** (from `crates/ethereum/engine-primitives/src/payload.rs:407-426`):
    ```rust
    pub fn payload_id(parent: &B256, attributes: &PayloadAttributes) -> PayloadId {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(parent.as_slice());
        hasher.update(&attributes.timestamp.to_be_bytes()[..]);
        hasher.update(attributes.prev_randao.as_slice());
        hasher.update(attributes.suggested_fee_recipient.as_slice());
        // ... more fields
    }
    ```

    **The Issue**: Payload IDs are unique per **parent hash + attributes combination**, but multiple payload jobs can target the **same block number** with different:
    - Timestamps
    - `prev_randao` values
    - `suggested_fee_recipient` addresses
    - Other attributes

    #### 3. **Multiple Trigger Sources**

    Several components can trigger payload building for the same block number:

    1. **Engine API calls** (`engine_forkchoiceUpdatedV1/V2/V3`)
    2. **Interval-based continuous building** (every 1 second)
    3. **New transaction arrivals** in the mempool
    4. **State changes** from new blocks

    #### 4. **Payload Job Lifecycle**

    Each `BasicPayloadJob` runs for **12 seconds** (slot duration) and continuously builds:

    ```rust
    // From crates/payload/basic/src/lib.rs:168-185
    let mut job = BasicPayloadJob {
        config,
        executor: self.executor.clone(),
        deadline,
        // ticks immediately
        interval: tokio::time::interval(self.config.interval), // 1 second
        best_payload: PayloadState::Missing,
        pending_block: None,
        cached_reads,
        payload_task_guard: self.payload_task_guard.clone(),
        metrics: Default::default(),
        builder: self.builder.clone(),
    };

    // start the first job right away
    job.spawn_build_job();
    ```

    **During this 12-second window:**
    - The job spawns a new `try_build` call every 1 second
    - Each call targets the **same block number** (parent + 1)
    - The goal is to build progressively better payloads with more transactions

    #### 5. **Transaction Pool Evolution**

    The multiple executions serve a purpose - they allow the payload builder to:

    1. **Incorporate new transactions** that arrive in the mempool
    2. **Optimize gas usage** by trying different transaction combinations
    3. **Improve fee collection** by selecting better transactions
    4. **Handle transaction ordering** changes

    ### **Why This Design Exists**

    This is **intentional behavior** for several reasons:

    1. **MEV Optimization**: Allows builders to continuously improve payloads for better MEV extraction
    2. **Transaction Inclusion**: New transactions arriving in the pool can be included in subsequent builds
    3. **Gas Optimization**: Different transaction orderings can lead to better gas utilization
    4. **Competitive Building**: Multiple builders can compete to create the best payload for the same slot

    ### **Evidence from Code**

    The debug logs show this is expected:

    ```rust
    // From crates/ethereum/payload/src/lib.rs:175
    debug!(target: "payload_builder", id=%attributes.id, parent_header = ?parent_header.hash(), parent_number = parent_header.number, "building new payload");
    ```

    This log appears **multiple times** for the same `parent_number` because it's the **intended behavior**.

    ### **Conclusion**

    The multiple executions of `try_build` and `default_ethereum_payload` with the same block number are **not a bug** but a **feature** of Reth's payload building architecture. It's designed to:

    1. **Continuously improve** payloads during the 12-second slot window
    2. **Incorporate new transactions** as they arrive
    3. **Optimize for better fees and gas usage**
    4. **Enable competitive MEV building**

    The system correctly deduplicates based on **payload ID** (which includes all attributes) but intentionally allows multiple builds for the **same block number** with different attributes to enable continuous optimization.
    */

    fn try_build(
        &self,
        args: BuildArguments<EthPayloadBuilderAttributes, EthBuiltPayload>,
    ) -> Result<BuildOutcome<EthBuiltPayload>, PayloadBuilderError> {
        let proposal_transactions = self.consensus_pool.get_proposal_transactions();
        let payload = default_ethereum_payload(
            self.evm_config.clone(),
            self.client.clone(),
            self.pool.clone(),
            self.builder_config.clone(),
            args,
            |attributes| get_best_transactions(&self.pool, proposal_transactions, attributes),
        );

        payload.map(|payload| {
            match payload {
                BuildOutcome::Better { payload, .. } => {
                    let block = payload.block();
                    let header = block.header();
                    debug!(
                        "[MysticetiPayloadBuilder] try_build with better payload. Block number: {}, parent hash: {}, header: {:?}",
                        block.header().number(),
                        hex::encode(block.header().parent_hash()),
                        header
                    );
                    //Return freeze payload instead of better payload
                    //Stop try_build process
                    BuildOutcome::Freeze(payload)
                }
                _ => payload,
            }
        })
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
            |attributes| get_best_transactions(&self.pool, Vec::default(), attributes),
        )?
        .into_payload()
        .ok_or_else(|| PayloadBuilderError::MissingPayload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_extension::CommittedSubDag;

    #[test]
    fn test_committed_subdag_operations() {
        let subdag = CommittedSubDag::default();

        // Test that we can add blocks (even if empty)
        assert!(subdag.blocks.is_empty());
        assert!(subdag.flatten_transactions().is_empty());

        // Test flatten_transactions
        let transactions = subdag.flatten_transactions();
        assert!(transactions.is_empty());
    }

    #[test]
    fn test_payload_builder_error_types() {
        // Test that error types can be created
        let error = PayloadBuilderError::MissingPayload;
        match error {
            PayloadBuilderError::MissingPayload => assert!(true),
            _ => assert!(false),
        }
    }

    #[test]
    fn test_missing_payload_behaviour() {
        // Test that we can create MissingPayloadBehaviour
        let behavior: MissingPayloadBehaviour<()> = MissingPayloadBehaviour::AwaitInProgress;
        match behavior {
            MissingPayloadBehaviour::AwaitInProgress => assert!(true),
            _ => assert!(false),
        }
    }
}
