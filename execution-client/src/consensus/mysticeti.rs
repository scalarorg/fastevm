use crate::{consensus::ConsensusPool, RethBlockChainProvider};
use alloy_consensus::transaction::TxHashRef;
use alloy_consensus::BlockHeader;
use alloy_eips::eip4895::Withdrawals;
use alloy_primitives::{Address, TxHash, B256, U256};
use alloy_rpc_types_engine::PayloadAttributes;
use alloy_rpc_types_eth::TransactionRequest;
use anyhow::Result;
use futures_util::StreamExt;
// Import GravityEvent from gravity framework (same type used by ExecutionResult)
use gravity_api_types::events::contract_event::GravityEvent;
use greth::{
    gravity_storage::{block_view_storage::BlockViewStorage, GravityStorage},
    reth_pipe_exec_layer_ext_v2::{
        ExecutionResult, OrderedBlock, PipeExecLayerApi,
    },
    reth_primitives::TransactionSigned,
    reth_rpc_api::eth::{helpers::EthCall, RpcTypes},
    reth_tasks::TaskExecutor,
};
use parking_lot::RwLock;
use reth_ethereum::{
    chainspec::{ChainSpecProvider, EthChainSpec},
    node::api::{
        BuiltPayload, ConsensusEngineHandle, EngineApiMessageVersion, ExecutionPayload,
        PayloadTypes,
    },
    node::engine::EthPayloadAttributes,
    rpc::types::engine::{ForkchoiceState, ForkchoiceUpdated},
    storage::StateProviderFactory,
};
use reth_node_api::BlockBody;
use reth_payload_builder::PayloadId;
use reth_provider::CanonStateSubscriptions;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use rpc_shared_api::MysticetiCommittedSubdag;
use std::{collections::VecDeque, sync::Arc};
use tracing::warn;

// Memory optimization: Maximum number of payloads to buffer
// If payloads are built faster than executed, this prevents unbounded growth
const MAX_PAYLOAD_BUFFER_SIZE: usize = 10;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::{debug, error, info};
// const WAITING_PENDING_TXS_TIMEOUT: u64 = 3000; // 10 second timeout
// const WAITING_PENDING_TXS_INTERVAL: u64 = 100; // 1 second interval
use std::collections::HashSet;

pub struct MysticetiConsensus<
    Provider,
    Payload,
    Pool,
    EthApi,
    Storage = BlockViewStorage<RethBlockChainProvider>,
> where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
    Storage: GravityStorage,
{
    task_executor: TaskExecutor,
    consensus_pool: Arc<ConsensusPool<Pool>>,
    //payload_builder_handle: PayloadBuilderHandle<Payload>,
    rx_built_payload: Option<UnboundedReceiver<Payload::BuiltPayload>>,
    engine_handle: ConsensusEngineHandle<Payload>,
    provider: Provider,
    // Keep track of the canonical state number
    block_interval_ms: u64,
    epoch: Arc<RwLock<u64>>,
    // canonical_block_number
    // Updated when receive canonical state updated
    canonical_block_number: Arc<RwLock<u64>>,
    // Keep track of the last sent to evm executor payload
    last_processing_payload: Option<Payload::BuiltPayload>,
    // Keep track of the last built payload
    // This payload is used to build new forkchoice state and next payloadAttribute
    last_built_payload: Option<Payload::BuiltPayload>,
    // Keep pyload buffer and send to evm executor if last payload is executed
    payload_buffer: VecDeque<Payload::BuiltPayload>,
    // Pipeline API for executing payloads
    // If None, payloads will be send to the consensus engine handle directly
    // Otherwise, payloads will be send to the pipeline API
    pipeline_api: Option<Arc<PipeExecLayerApi<Storage, EthApi>>>,
    last_ordered_block: Option<OrderedBlock>,
}

impl<Provider, Payload, Pool, EthApi, Storage>
    MysticetiConsensus<Provider, Payload, Pool, EthApi, Storage>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
    Storage: GravityStorage,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    pub fn new(
        task_executor: TaskExecutor,
        consensus_pool: Arc<ConsensusPool<Pool>>,
        provider: Provider,
        rx_built_payload: UnboundedReceiver<Payload::BuiltPayload>,
        engine_handle: ConsensusEngineHandle<Payload>,
        block_interval_ms: u64,
    ) -> Self {
        // OnchainConfigFetcher is not needed when pipeline_api is None
        Self {
            task_executor,
            consensus_pool,
            rx_built_payload: Some(rx_built_payload),
            engine_handle,
            provider,
            block_interval_ms,
            epoch: Arc::new(RwLock::new(1)),
            canonical_block_number: Arc::new(RwLock::new(0)),
            last_processing_payload: None,
            last_built_payload: None,
            payload_buffer: VecDeque::new(),
            pipeline_api: None,
            last_ordered_block: None
        }
    }
}

// Helper implementation for the type alias to allow omitting Storage type
impl<Provider, Payload, Pool, EthApi>
    MysticetiConsensus<Provider, Payload, Pool, EthApi, BlockViewStorage<RethBlockChainProvider>>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    /// Create MysticetiConsensus with default Storage type
    /// This allows omitting the Storage type parameter when pipeline_api is None
    /// The `_eth_api` parameter is only used for type inference and is not actually used
    pub fn new_with_default_storage(
        task_executor: TaskExecutor,
        consensus_pool: Arc<ConsensusPool<Pool>>,
        provider: Provider,
        rx_built_payload: UnboundedReceiver<Payload::BuiltPayload>,
        engine_handle: ConsensusEngineHandle<Payload>,
        block_interval_ms: u64,
        _eth_api: &EthApi,
    ) -> Self {
        Self::new(
            task_executor,
            consensus_pool,
            provider,
            rx_built_payload,
            engine_handle,
            block_interval_ms,
        )
    }
}

impl<Provider, Payload, Pool, EthApi, Storage>
    MysticetiConsensus<Provider, Payload, Pool, EthApi, Storage>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
    Storage: GravityStorage,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    pub fn new_with_pipeline_api(
        task_executor: TaskExecutor,
        consensus_pool: Arc<ConsensusPool<Pool>>,
        provider: Provider,
        rx_built_payload: UnboundedReceiver<Payload::BuiltPayload>,
        engine_handle: ConsensusEngineHandle<Payload>,
        pipeline_api: Option<Arc<PipeExecLayerApi<Storage, EthApi>>>,
        block_interval_ms: u64,
    ) -> Self {
        Self {
            task_executor,
            consensus_pool,
            rx_built_payload: Some(rx_built_payload),
            engine_handle,
            provider,
            block_interval_ms,
            epoch: Arc::new(RwLock::new(1)),
            canonical_block_number: Arc::new(RwLock::new(0)),
            last_processing_payload: None,
            last_built_payload: None,
            payload_buffer: VecDeque::new(),
            pipeline_api,
            last_ordered_block: None
        }
    }
}

impl<Provider, Payload, Pool, EthApi, Storage>
    MysticetiConsensus<Provider, Payload, Pool, EthApi, Storage>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
    Storage: GravityStorage,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    /// Get current forkchoice state
    async fn create_forkchoice_state(&self, last_block_hash: Option<B256>) -> ForkchoiceState {
        //Todo: Implement this
        match last_block_hash {
            Some(block_hash) => ForkchoiceState {
                head_block_hash: block_hash,
                safe_block_hash: block_hash,
                finalized_block_hash: block_hash,
            },
            None => {
                let chain_spec = self.provider.chain_spec();
                let head_block_hash = chain_spec.genesis_hash();
                let safe_block_hash = head_block_hash;
                let finalized_block_hash = head_block_hash;
                ForkchoiceState {
                    head_block_hash,
                    safe_block_hash,
                    finalized_block_hash,
                }
            }
        }
    }

    async fn execute_payload(
        &self,
        built_payload: &Payload::BuiltPayload,
    ) -> Result<Option<Payload::ExecutionData>> {
        let execution_payload = Payload::block_to_payload(built_payload.block().clone());
        match self
            .engine_handle
            .new_payload(execution_payload.clone())
            .await
        {
            Ok(payload_status) => {
                if payload_status.is_valid() {
                    let block_hash = execution_payload.block_hash();
                    let forkchoice_state = self.create_forkchoice_state(Some(block_hash)).await;
                    //Call fork_choice_updated to make last executed block canonical
                    //TODO: handle execution result
                    match self
                        .engine_handle
                        .fork_choice_updated(
                            forkchoice_state,
                            None,
                            EngineApiMessageVersion::default(),
                        )
                        .await
                    {
                        Ok(ForkchoiceUpdated {
                            payload_status,
                            payload_id,
                        }) => {
                            debug!("Forkchoice updated successfully with payload {:?} and status: {:?}", payload_id, payload_status);
                        }
                        Err(e) => {
                            error!("Forkchoice updated failed: {:?}", e);
                        }
                    }
                    Ok(Some(execution_payload.clone()))
                } else {
                    Err(anyhow::anyhow!(
                        "Execute new payload failed with status: {:?}",
                        payload_status
                    ))
                }
            }
            Err(e) => Err(anyhow::anyhow!(
                "Execute new payload failed with error: {:?}",
                e
            )),
        }
    }
}

/// Start mysticeti consensus with pipeline API handle
impl<Provider, Payload, Pool, EthApi, Storage>
    MysticetiConsensus<Provider, Payload, Pool, EthApi, Storage>
where
    Provider: ChainSpecProvider + StateProviderFactory + CanonStateSubscriptions + Unpin + 'static,
    Payload: PayloadTypes<PayloadAttributes = EthPayloadAttributes>,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>> + 'static,
    Storage: GravityStorage,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    /// Starts the Mysticeti consensus engine with pipeline API integration.
    ///
    /// This method runs in an infinite loop, periodically attempting to build the next ordered block
    /// at intervals specified by `block_interval_ms`. The pipeline API is used to execute blocks
    /// asynchronously.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the loop runs successfully. In practice, this method runs indefinitely
    /// until the process is terminated.
    ///
    /// # Errors
    ///
    /// Errors are logged but do not stop the loop. If building an ordered block fails, an error
    /// is logged and the loop continues.
    pub async fn start_with_pipeline_api(&mut self) -> Result<()> {
        //TODO: Add configurable interval
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_millis(self.block_interval_ms));

        let task_executor = self.task_executor.clone();

        if let Some(pipeline_api) = self.pipeline_api.as_ref() {
            // Task 2: Pull executed block hash from pipeline API and update canonical block number
            // This task continuously pulls execution results from the pipeline API and updates
            // the canonical block number, which is used to determine when blocks are finalized
            let pipeline_api_clone = pipeline_api.clone();
            let canonical_block_number = self.canonical_block_number.clone();
            let arc_epoch = self.epoch.clone();
            let consensus_pool = self.consensus_pool.clone();
            task_executor.spawn(async move {
                info!("Start pull executed block hash task");
                loop {
                    if let Some(ExecutionResult {
                        block_id,
                        block_number,
                        block_hash,
                        txs_info,
                        gravity_events,
                    }) = pipeline_api_clone.pull_executed_block_hash().await
                    {
                        info!("Pull executed block hash successfully: block_id={:?}, block_number={:?}, block_hash={:?}", 
                            block_id, block_number, block_hash);
                        match pipeline_api_clone
                            .commit_executed_block_hash(block_id, Some(block_hash)) {
                            Some(_) => {
                                info!("Committed executed block hash successfully: block_id={:?}, block_number={:?}, block_hash={:?}", 
                                    block_id, block_number, block_hash);
                            }
                            None => {
                                error!("Failed to commit executed block hash: block_id={:?}, block_number={:?}, block_hash={:?}", 
                                    block_id, block_number, block_hash);
                            }
                        }
                        // extract mined transactions from txs_info
                        let mined_txs = txs_info.iter().map(|tx| tx.tx_hash.clone()).collect::<HashSet<TxHash>>();
                        *canonical_block_number.write() = block_number;  
                        if block_number > 1 {
                            // Remove mined transactions from consensus pool and update next committed index
                            // Block with number 1 is synthetic block for update timestamp and epoch.
                            consensus_pool.update_mined_block(block_number, &mined_txs);
                        }
                        else {
                            info!("Executed genesis block. Skip removing mined transactions.");
                        }
                        for event in gravity_events {
                            match event {
                                GravityEvent::NewEpoch(epoch, _) => {
                                    *arc_epoch.write() = epoch;
                                    info!("New epoch: {:?}", epoch);
                                }
                                _ => {}
                            }
                        }                      
                    } else {
                        error!("No executed block hash found. Waiting for the next executed block hash.");
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                    }
                }
            });

            // Task 3: Main consensus loop - build and push ordered blocks to pipeline API
            // This is the main loop that periodically builds ordered blocks from committed subdags
            // and pushes them to the pipeline API for execution. It runs at intervals specified by block_interval_ms
            let pipeline_api_clone = pipeline_api.clone();
            loop {
                interval.tick().await;
                // Try to build next OrderedBlock at each interval
                match self.try_build_next_ordered_block().await {
                    Ok(Some(ordered_block)) => {
                        let block_number = ordered_block.number;
                        let block_id = ordered_block.id;
                        let parent_id = ordered_block.parent_id;
                        let number_of_transactions = ordered_block.transactions.len();
                        debug!("Push ordered block to pipeline API: Block number: {:?}, Block id: 0x{:?}, Parent id: 0x{:?}", block_number, block_id, parent_id);
                        // Push the block to the pipeline API (consumes the block)
                        let push_result = pipeline_api_clone.push_ordered_block(ordered_block);

                        if push_result.is_some() {
                            // Store a minimal version for tracking (we only need number and id for next block)
                            // Reconstruct a minimal OrderedBlock with just the essential fields
                            let minimal_block = OrderedBlock {
                                epoch: 0, // Not used for tracking
                                parent_id,
                                id: block_id,
                                number: block_number,
                                timestamp_us: 0,            // Not used for tracking
                                coinbase: Address::ZERO, // Not used for tracking
                                prev_randao: B256::ZERO, // Not used for tracking
                                withdrawals: Withdrawals::default(), // Not used for tracking
                                transactions: vec![],    // Not used for tracking
                                senders: vec![],         // Not used for tracking
                                proposer: None,          // Not used for tracking
                                extra_data: vec![],      // Not used for tracking
                                randomness: U256::ZERO,  // Not used for tracking
                                enable_randomness: false, // Not used for tracking
                            };
                            self.last_ordered_block.replace(minimal_block);
                            info!("Push ordered block successfully: Block number: {:?}, Block id: {:?}, Number of transactions: {:?}", 
                                block_number, block_id, number_of_transactions);
                        } else {
                            error!(
                                "Push ordered block failed: Block number: {:?}, Block id: {:?}",
                                block_number, block_id
                            );
                        };
                    }
                    Ok(None) => {
                        debug!("New ordered block is not available. Wating for the next committed subdag. Committed subdag queue size: {:?}", self.consensus_pool.queue_size());
                    }
                    Err(e) => {
                        error!("Build next ordered block failed: {:?}", e);
                    }
                }
                // Reset interval so next tick happens block_interval_ms after work completes
                // This ensures consistent spacing between work cycles, accounting for variable work duration
                interval.reset();
            }
        }
        Ok(())
    }
    /// Attempts to build the next ordered block from committed subdags in the consensus pool.
    ///
    /// This method checks if there are enough committed subdags available to form a complete batch.
    /// If available, it retrieves proposal transactions from the consensus pool and builds an
    /// `OrderedBlock` that can be sent to the pipeline API for execution.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(OrderedBlock))` - If a complete batch of committed subdags is available and
    ///   an ordered block was successfully built.
    /// * `Ok(None)` - If there are not enough committed subdags available to build a block.
    ///
    /// # Errors
    ///
    /// Returns an error if building the ordered block fails (e.g., due to missing data or
    /// invalid transactions).
    pub async fn try_build_next_ordered_block(&mut self) -> Result<Option<OrderedBlock>> {
        // First check if the last ordered block is processed
        let canonical_block_number = *self.canonical_block_number.read();
        // the last ordered block is not processed yet
        if let Some(last_ordered_block) = self.last_ordered_block.as_ref() {
            if last_ordered_block.number > canonical_block_number {
                debug!("Last ordered block number {:?} is greater than canonical block number {:?}. Wating for the next canonical block.", last_ordered_block.number, canonical_block_number);
                return Ok(None);
            }
            // 1. Get proposal transactions for the next block from consensus pool
            if let Some((first_committed_subdag, last_committed_subdag)) =
                self.consensus_pool.next_committed_subdag_batch()
            {
                info!(
                    "Queue size: {:?}, Create proposal block with committed batch size: {:?}: FirstCommittedSubdag: {{index: {:?}, timestamp: {:?}, round: {:?}}}, LastCommittedSubdag: {{index: {:?}, timestamp: {:?}, round: {:?}}}",
                    self.consensus_pool.queue_size(),
                    last_committed_subdag.commit_ref.round
                        - first_committed_subdag.commit_ref.round
                        + 1,
                    first_committed_subdag.commit_ref.round,
                    first_committed_subdag.timestamp_ms,
                    first_committed_subdag.leader.round,
                    last_committed_subdag.commit_ref.round,
                    last_committed_subdag.timestamp_ms,
                    last_committed_subdag.leader.round,
                );
                let proposal_transactions = self.consensus_pool.get_proposal_transactions();
                return self.build_ordered_block(proposal_transactions, last_committed_subdag).map(Option::Some);
            }
        } else {
            let first_committed_subdag = self.consensus_pool.get_fist_committed_subdag();
            if first_committed_subdag.is_none() {
                return Ok(None);
            }
            info!("No last ordered block. Build first empty ordered block for update timestamp and epoch.");
            return self.build_ordered_block(vec![], first_committed_subdag.unwrap()).map(Option::Some);
        }
        debug!("No proposal transactions available. Wating for the next committed subdag. Committed subdag queue size: {:?}", self.consensus_pool.queue_size());
        return Ok(None);
    }
    /// Builds an `OrderedBlock` from transactions and committed subdag information.
    ///
    /// This method constructs an `OrderedBlock` structure that contains all the necessary
    /// information for block execution, including transactions, block metadata, and proposer
    /// information. The block is built using the provided transactions and the last committed subdag.
    ///
    /// # Arguments
    ///
    /// * `transactions` - A vector of pool transactions to include in the block.
    /// * `last_committed_subdag` - The last committed subdag in the batch, used for timestamp and proposer information.
    ///
    /// # Returns
    ///
    /// Returns an `OrderedBlock` containing all block information including:
    /// - Block number (incremented from last ordered block)
    /// - Block hash (computed from commit digest)
    /// - Timestamp from committed subdag
    /// - Epoch information fetched from onchain config
    /// - Transactions and their senders
    /// - Withdrawals, coinbase, and other block metadata
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Transaction conversion fails
    /// - Epoch information cannot be fetched
    /// - Any other block construction step fails
    fn build_ordered_block(
        &self,
        transactions: Vec<Arc<Pool::Transaction>>,
        last_committed_subdag: MysticetiCommittedSubdag<Arc<Pool::Transaction>>,
    ) -> Result<OrderedBlock> {
        // Determine block number from last ordered block, or start at 0
        let parent_number = self
            .last_ordered_block
            .as_ref()
            .map(|block| block.number)
            .unwrap_or(*self.canonical_block_number.read());

        // Fetch epoch using the parent block number (block_number - 1) since the current block
        // doesn't exist yet. For the first block (block_number = 1), use block 0 (genesis).
        let block_number = parent_number + 1;
        // Use cached epoch
        let epoch = *self.epoch.read();
        // let epoch = self
        //     .onchain_config_fetcher
        //     .as_ref()
        //     .expect("onchain_config_fetcher should be Some when pipeline_api is Some")
        //     .fetch_epoch(parent_number);

        // Get parent hash from last ordered block, or use genesis hash
        let parent_id = self
            .last_ordered_block
            .as_ref()
            .map(|block| block.id)
            .unwrap_or_else(|| self.provider.chain_spec().genesis_hash());

        // Convert timestamp from milliseconds to seconds
        let timestamp = last_committed_subdag.timestamp_ms / 1000;

        // Generate block ID
        let block_id = {
            let mut input = Vec::with_capacity(16);
            input.extend_from_slice(&block_number.to_be_bytes());
            input.extend_from_slice(&timestamp.to_be_bytes());
            alloy_primitives::keccak256(input)
        };

        // Default values for fields not available from subdag
        let coinbase = Address::ZERO;
        let prev_randao = B256::ZERO;
        let withdrawals = Withdrawals::default();

        // Convert pool transactions to TransactionSigned and extract senders
        let mut signed_transactions = Vec::new();
        let mut senders = Vec::new();

        for tx in transactions.iter() {
            // Extract sender from pool transaction
            let sender = tx.sender();
            senders.push(sender);

            // Convert Pool::Transaction to TransactionSigned using clone_into_consensus
            // clone_into_consensus returns Recovered<Consensus>, so we extract the inner transaction
            // With the trait bound PoolTransaction<Consensus = TransactionSigned>,
            // the Consensus type is guaranteed to be TransactionSigned
            let recovered = (**tx).clone_into_consensus();
            let signed_tx = recovered.into_inner();
            signed_transactions.push(signed_tx);
        }

        // Extract proposer from commit digest (use first 32 bytes of digest)
        let proposer = last_committed_subdag
            .leader
            .leader_address
            .strip_prefix("0x")
            .and_then(|hex_str| {
                hex::decode(hex_str).ok().and_then(|bytes| {
                    if bytes.len() == 32 {
                        let mut array = [0u8; 32];
                        array.copy_from_slice(bytes.as_slice());
                        Some(array)
                    } else {
                        None
                    }
                })
            });
        info!("Build ordered block with number: {:?} proposer: {:?}, current timestamp: {:?} in second, number of transactions: {:?}", 
            block_number,  
            proposer.as_ref().map(|bytes| format!("0x{}", hex::encode(bytes))), 
            timestamp,
            signed_transactions.len());
        let ordered_block = OrderedBlock {
            epoch,
            parent_id: parent_id,
            id: block_id,
            number: block_number,
            // Convert timestamp from seconds to microseconds
            timestamp_us: timestamp * 1_000_000,
            coinbase,
            prev_randao,
            withdrawals,
            transactions: signed_transactions,
            senders,
            proposer,
            extra_data: vec![],
            randomness: U256::ZERO,
            enable_randomness: false,
        };
        Ok(ordered_block)
    }
}
/// Start mysticeti consensus with engine handle
impl<Provider, Payload, Pool, EthApi, Storage>
    MysticetiConsensus<Provider, Payload, Pool, EthApi, Storage>
where
    Provider: ChainSpecProvider + StateProviderFactory + CanonStateSubscriptions + Unpin + 'static,
    Payload: PayloadTypes<PayloadAttributes = EthPayloadAttributes>,
    Pool: TransactionPool,
    Storage: GravityStorage,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    pub async fn start_with_engine_handle(&mut self) -> Result<()> {
        //TODO: Add configurable interval
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_millis(self.block_interval_ms));
        let mut notifications = self.provider.canonical_state_stream();

        let mut built_payload_stream =
            UnboundedReceiverStream::new(self.rx_built_payload.take().unwrap());

        loop {
            tokio::select! {
                //Receive built payload from channel send from custom payload builder
                new_built_payload = built_payload_stream.next() => {
                    match new_built_payload {
                        Some(new_payload) => {
                            let payload_number = new_payload.block().header().number();
                            let tx_count = new_payload.block().body().transactions().len();

                            // Check if buffer is getting too large
                            if self.payload_buffer.len() >= MAX_PAYLOAD_BUFFER_SIZE {
                                warn!(
                                    "Payload buffer is full ({}), dropping oldest payload. This may indicate payload execution is slower than payload building.",
                                    self.payload_buffer.len()
                                );
                                // Remove oldest payload to make room
                                let _ = self.payload_buffer.pop_front();
                            }

                            info!("New built payload with number {} and {} txs, put it to the buffer. Current payload buffer size: {:?}",
                                payload_number,
                                tx_count,
                                self.payload_buffer.len() + 1);

                            // Avoid cloning if possible - use Arc if payload supports it, otherwise clone only when needed
                            self.last_built_payload.replace(new_payload.clone());
                            self.payload_buffer.push_back(new_payload);

                            if self.last_processing_payload.is_none() {
                                if let Some(payload) = self.payload_buffer.pop_front() {
                                    info!("Last processing payload is None. Execute next proposal block.");
                                    self.last_processing_payload.replace(payload);
                                    let payload = self.last_processing_payload.as_ref().unwrap();
                                    if let Err(e) = self.execute_payload(payload).await {
                                        error!("Execute pending payload failed: {:?}", e);
                                    }
                                }
                            }

                        }
                        None => {
                            debug!("Built payload stream closed");
                        }
                    }
                }

                canonical_state = notifications.next() => {
                    match canonical_state {
                        Some(canonical_state) => {
                            // Pop payload from buffer
                            let canonical_block_number = canonical_state.tip().number();
                            *self.canonical_block_number.write() = canonical_block_number;
                            info!("Canonical state updated with block number: {:?}", canonical_block_number);
                            let pending_block_number = self.last_processing_payload.as_ref().map(|payload| payload.block().header().number()).unwrap_or_default();
                            if pending_block_number == canonical_block_number  {
                                info!("Pending block number {:?} is executed. Remove mined transactions from consensus pool.", pending_block_number);
                                let payload = self.last_processing_payload.take().unwrap();
                                let tx_hashes = payload.block().body().transactions().iter().map(|tx|  tx.tx_hash().clone()).collect::<HashSet<TxHash>>();
                                // let tx_hashes = execution_payload
                                //     .transactions()
                                //     .iter()
                                //     .map(|tx| calculate_tx_hash(tx))
                                //     .collect::<HashSet<TxHash>>();
                                 //Remove pending buffer
                                self.consensus_pool.update_mined_block(pending_block_number, &tx_hashes);
                                // Try to execute next built payload
                                if let Some(next_payload) = self.payload_buffer.pop_front() {
                                    info!("Execute next payload from buffer {:?}.", next_payload.block().header().number());
                                    self.last_processing_payload.replace(next_payload);
                                    if let Err(e) = self.execute_payload(self.last_processing_payload.as_ref().unwrap()).await {
                                        error!("Execute pending payload failed: {:?}", e);
                                    }
                                }
                            } else {
                                info!("last executed block number is {:?}. Current canonical state number is {:?}. Skip removing mined transactions.",
                                pending_block_number, canonical_block_number );
                            }
                        }
                        None => {
                            debug!("Canonical state updated: None");
                        }
                    }
                }
                _ = interval.tick() => {
                    //Try to build next proposal block
                    let last_built_block_number = self.last_built_payload.as_ref().map(|payload| payload.block().header().number()).unwrap_or_default();
                    let canonical_block_number = *self.canonical_block_number.read();
                    if last_built_block_number == canonical_block_number || self.last_built_payload.is_none() {
                        // Clone is necessary here as build_next_proposal_block needs ownership
                        // The payload is large but this is only called periodically
                        match self.build_next_proposal_block(self.last_built_payload.clone()).await {
                            Ok(Some(payload_id)) => {
                                debug!("Build next proposal block successfully. Pending payload id: {:?}", payload_id);
                            }
                            Ok(None) => {},
                            Err(e) => {
                                error!("Build next proposal block failed: {:?}", e);
                            }
                        }
                    } else {
                        debug!("Last built payload block number {:?} is not executed. Canonical block number is {:?}. Skip building next proposal block.", last_built_block_number, self.canonical_block_number);
                    }
                    interval.reset();
                }
            }
        }
    }

    pub async fn build_next_proposal_block(
        &mut self,
        //last_built_payload: Option<<EthEngineTypes as PayloadTypes>::BuiltPayload>,
        last_built_payload: Option<Payload::BuiltPayload>,
    ) -> Result<Option<PayloadId>> {
        if let Some((first_committed_subdag, last_committed_subdag)) =
            self.consensus_pool.next_committed_subdag_batch()
        {
            info!(
                "Create proposal block with committed batch size: {:?}:
                 FirstCommittedSubdag: {{index: {:?}, timestamp: {:?}, round: {:?}}},
                 LastCommittedSubdag: {{index: {:?}, timestamp: {:?}, round: {:?}}}
                 Queue size: {:?}",
                last_committed_subdag.commit_ref.round - first_committed_subdag.commit_ref.round
                    + 1,
                first_committed_subdag.commit_ref.round,
                first_committed_subdag.timestamp_ms,
                first_committed_subdag.leader.round,
                last_committed_subdag.commit_ref.round,
                last_committed_subdag.timestamp_ms,
                last_committed_subdag.leader.round,
                self.consensus_pool.queue_size(),
            );
            let last_block_hash = last_built_payload
                .as_ref()
                .map(|payload| payload.block().hash());
            let forkchoice_state = self.create_forkchoice_state(last_block_hash).await;
            let leader_digest: [u8; 32] = last_committed_subdag
                .leader
                .digest
                .as_ref()
                .try_into()
                .expect("Leader digest must be exactly 32 bytes");
            //Create payload attributes with timestamp in seconds
            let payload_attributes = self.create_payload_attributes(
                last_committed_subdag.timestamp_ms / 1000,
                leader_digest,
                last_built_payload,
            );
            debug!(
                "Forkchoice state: {:?}, Payload attributes: {:?}",
                forkchoice_state, payload_attributes
            );
            let _ = self
                .engine_handle
                .fork_choice_updated(
                    forkchoice_state,
                    Some(payload_attributes.clone()),
                    EngineApiMessageVersion::default(),
                )
                .await
                .map_err(anyhow::Error::msg)?;
        }
        Ok(None)
    }
    /// Create next payload attributes
    /// This attributes must be equals on all the nodes
    fn create_payload_attributes(
        &self,
        timestamp: u64,
        leader_digest: [u8; 32],
        //last_build_payload: Option<<EthEngineTypes as PayloadTypes>::BuiltPayload>,
        last_build_payload: Option<Payload::BuiltPayload>,
    ) -> <Payload as PayloadTypes>::PayloadAttributes {
        //TODO:
        //1. Get actual prev_randao from the previous block's header
        //2. Check and create withdrawals vector
        //3. Set suggested fee recipient
        debug!("Create payload attributes with timestamp: {:?}", timestamp);
        let finalized_block_num_hash = self.provider.finalized_block_num_hash().unwrap();
        let genesis_hash = self.provider.chain_spec().genesis_hash();
        debug!(
            "finalized block hash: {:?}, Genesis hash: {:?}",
            finalized_block_num_hash, genesis_hash
        );
        let last_block_root = B256::from(leader_digest);
        match last_build_payload {
            Some(payload) => {
                let last_block_number = payload.block().header().number();
                let last_block_timestamp = payload.block().header().timestamp();
                debug!(
                    "Last payload number: {:?} with timestamp: {:?}. Next timestamp: {:?}",
                    last_block_number, last_block_timestamp, timestamp
                );
                //TODO: use commit_digest as parent_beacon_block_root
                PayloadAttributes {
                    timestamp,
                    prev_randao: B256::default(),
                    suggested_fee_recipient: Address::default(),
                    withdrawals: Default::default(),
                    parent_beacon_block_root: Some(last_block_root),
                }
            }
            None => {
                // let chain_spec = self.provider.chain_spec();
                PayloadAttributes {
                    timestamp,
                    prev_randao: B256::default(),
                    suggested_fee_recipient: Address::default(),
                    withdrawals: Default::default(),
                    parent_beacon_block_root: Some(last_block_root),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_id_default() {
        let payload_id = PayloadId::default();
        assert_eq!(payload_id, PayloadId::default());
    }

    #[test]
    fn test_forkchoice_state_creation() {
        let head_block_hash = B256::default();
        let safe_block_hash = B256::default();
        let finalized_block_hash = B256::default();

        let forkchoice_state = ForkchoiceState {
            head_block_hash,
            safe_block_hash,
            finalized_block_hash,
        };

        assert_eq!(forkchoice_state.head_block_hash, head_block_hash);
        assert_eq!(forkchoice_state.safe_block_hash, safe_block_hash);
        assert_eq!(forkchoice_state.finalized_block_hash, finalized_block_hash);
    }
}
