use crate::consensus::ConsensusPool;
use alloy_consensus::transaction::TxHashRef;
use alloy_consensus::BlockHeader;
use alloy_eips::eip4895::Withdrawals;
use alloy_primitives::{Address, TxHash, B256, U256};
use alloy_rlp::{Decodable, Encodable};
use alloy_rpc_types_engine::PayloadAttributes;
use alloy_rpc_types_eth::TransactionRequest;
use anyhow::Result;
use futures_util::StreamExt;
use greth::{
    gravity_storage::{block_view_storage::BlockViewStorage, GravityStorage},
    reth_pipe_exec_layer_ext_v2::{
        new_pipe_exec_layer_api, onchain_config::OnchainConfigFetcher, ExecutionArgs, OrderedBlock,
        PipeExecLayerApi,
    },
    reth_primitives::TransactionSigned,
    reth_rpc_api::eth::{helpers::EthCall, RpcTypes},
};
use reth_ethereum::primitives::Recovered;
use reth_ethereum::{
    chainspec::{ChainSpecProvider, EthChainSpec},
    node::api::{
        BeaconConsensusEngineHandle, BuiltPayload, EngineApiMessageVersion, ExecutionPayload,
        PayloadTypes,
    },
    node::engine::EthPayloadAttributes,
    primitives::SignedTransaction,
    rpc::types::engine::{ForkchoiceState, ForkchoiceUpdated},
    storage::StateProviderFactory,
};
use reth_node_api::BlockBody;
use reth_payload_builder::PayloadId;
use reth_provider::CanonStateSubscriptions;
use reth_transaction_pool::TransactionPool;
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
//const PAYLOAD_EXECUTION_TIMEOUT: u64 = 30; // 30 second timeout
//const PAYLOAD_EXECUTION_INTERVAL: u64 = 500; // 10 millisecond interval
pub struct MysticetiConsensus<Provider, Payload, Pool, Storage, EthApi>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
    Storage: GravityStorage,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    consensus_pool: Arc<ConsensusPool<Pool>>,
    //payload_builder_handle: PayloadBuilderHandle<Payload>,
    rx_built_payload: Option<UnboundedReceiver<Payload::BuiltPayload>>,
    engine_handle: BeaconConsensusEngineHandle<Payload>,
    provider: Provider,
    // Keep track of the canonical state number
    block_interval_ms: u64,
    // canonical_block_number
    // Updated when receive canonical state updated
    canonical_block_number: u64,
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
    onchain_config_fetcher: OnchainConfigFetcher<EthApi>,
}

impl<Provider, Payload, Pool, Storage, EthApi>
    MysticetiConsensus<Provider, Payload, Pool, Storage, EthApi>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
    Storage: GravityStorage,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    pub fn new(
        consensus_pool: Arc<ConsensusPool<Pool>>,
        provider: Provider,
        eth_api: EthApi,
        rx_built_payload: UnboundedReceiver<Payload::BuiltPayload>,
        engine_handle: BeaconConsensusEngineHandle<Payload>,
        pipeline_api: Option<Arc<PipeExecLayerApi<Storage, EthApi>>>,
        block_interval_ms: u64,
    ) -> Self {
        let onchain_config_fetcher = OnchainConfigFetcher::new(eth_api);
        Self {
            consensus_pool,
            rx_built_payload: Some(rx_built_payload),
            engine_handle,
            provider,
            block_interval_ms,
            canonical_block_number: 0,
            last_processing_payload: None,
            last_built_payload: None,
            payload_buffer: VecDeque::new(),
            pipeline_api,
            onchain_config_fetcher,
        }
    }
}

impl<Provider, Payload, Pool, Storage, EthApi>
    MysticetiConsensus<Provider, Payload, Pool, Storage, EthApi>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
    Storage: GravityStorage,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    // fn build_nil_ordered_block(&self) -> OrderedBlock {
    //     let block_number = 0;
    //     let epoch = self.onchain_config_fetcher.fetch_epoch(block_number);
    //     let block_hash = B256::ZERO;
    //     let parent_hash = B256::ZERO;
    //     let timestamp = 0;
    //     let withdrawals = Withdrawals::default();
    //     let proposer = None;
    //     let randomness = U256::ZERO;
    //     OrderedBlock {
    //         epoch,
    //         parent_id: parent_hash,
    //         id: block_hash,
    //         number: block_number,
    //         timestamp,
    //         coinbase: Address::ZERO,
    //         prev_randao: B256::ZERO,
    //         withdrawals,
    //         transactions: Vec::new(),
    //         senders: Vec::new(),
    //         proposer,
    //         extra_data: Vec::new(),
    //         randomness,
    //         enable_randomness: false,
    //     }
    // }

    fn build_ordered_block(&self, built_payload: &Payload::BuiltPayload) -> Result<OrderedBlock> {
        // Extract basic block information from ExecutionPayload trait
        let block_number = built_payload.block().header().number();
        let epoch = self.onchain_config_fetcher.fetch_epoch(block_number);
        let block_hash = built_payload.block().hash();
        let parent_hash = built_payload.block().header().parent_hash();
        let timestamp = built_payload.block().header().timestamp();
        let withdrawals = built_payload
            .block()
            .body()
            .withdrawals()
            .cloned()
            .map(Withdrawals::from)
            .unwrap_or_default();

        // Extract payload-specific fields (fee_recipient, prev_randao, transactions)
        // Use the ExecutionPayload trait methods to access these fields
        // let coinbase = built_payload.block().header().coinbase();
        // let prev_randao = built_payload.block().header().prev_randao();

        let coinbase = Address::ZERO;
        let prev_randao = B256::ZERO;

        // Decode transactions from raw bytes
        let mut transactions = Vec::new();
        let mut senders = Vec::new();

        for tx in built_payload.block().body().transactions() {
            // Encode transaction to bytes using RLP, then decode to Recovered to get both transaction and sender
            let mut encoded = Vec::new();
            tx.encode(&mut encoded);
            let mut tx_data = encoded;
            let recovered = Recovered::<TransactionSigned>::decode(&mut tx_data.as_slice())
                .map_err(|e| anyhow::anyhow!("Failed to decode transaction: {}", e))?;

            // Extract sender address
            let sender = recovered.signer();
            senders.push(sender);

            // Convert Recovered to TransactionSigned for OrderedBlock
            // TransactionSigned can be constructed from the transaction and signature
            let signed_tx = TransactionSigned::from(recovered.into_inner());
            transactions.push(signed_tx);
        }

        // Extract proposer from parent_beacon_block_root if available
        let proposer = built_payload
            .block()
            .header()
            .parent_beacon_block_root()
            .map(|root| {
                let mut proposer_bytes = [0u8; 32];
                proposer_bytes.copy_from_slice(root.as_slice());
                proposer_bytes
            });

        let ordered_block = OrderedBlock {
            epoch,
            parent_id: parent_hash,
            id: block_hash,
            number: block_number,
            timestamp,
            coinbase,
            prev_randao,
            withdrawals,
            transactions,
            senders,
            proposer,
            extra_data: vec![],
            randomness: U256::ZERO,
            enable_randomness: false,
        };
        Ok(ordered_block)
    }
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
    async fn execute_payload_with_pipeline(
        &self,
        built_payload: &Payload::BuiltPayload,
    ) -> Result<Option<Payload::ExecutionData>> {
        let execution_payload = Payload::block_to_payload(built_payload.block().clone());
        if let Some(pipeline_api) = self.pipeline_api.as_ref() {
            if let Ok(ordered_block) = self.build_ordered_block(built_payload) {
                if let Some(_) = pipeline_api.push_ordered_block(ordered_block) {
                    Ok(Some(execution_payload.clone()))
                } else {
                    Err(anyhow::anyhow!("Push ordered block failed"))
                }
            } else {
                Err(anyhow::anyhow!("Build ordered block failed"))
            }
        } else {
            Err(anyhow::anyhow!("Pipeline API is not set"))
        }
    }
    async fn execute_payload_with_engine(
        &self,
        execution_payload: &Payload::ExecutionData,
    ) -> Result<Option<Payload::ExecutionData>> {
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
    /// Try to execute the pending payload
    async fn execute_pending_payload(
        &self,
        built_payload: &Payload::BuiltPayload,
    ) -> Result<Option<Payload::ExecutionData>> {
        debug!(
            "Execute built payload {:?}",
            built_payload.block().header().number()
        );
        if let Some(pipeline_api) = self.pipeline_api.as_ref() {
            self.execute_payload_with_pipeline(built_payload).await
        } else {
            let execution_payload = Payload::block_to_payload(built_payload.block().clone());
            self.execute_payload_with_engine(&execution_payload).await
        }
    }
}

impl<Provider, Payload, Pool, Storage, EthApi>
    MysticetiConsensus<Provider, Payload, Pool, Storage, EthApi>
where
    Provider: ChainSpecProvider + StateProviderFactory + CanonStateSubscriptions + Unpin + 'static,
    Payload: PayloadTypes<PayloadAttributes = EthPayloadAttributes>,
    Pool: TransactionPool,
    Storage: GravityStorage,
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    pub async fn start(&mut self) -> Result<()> {
        //TODO: Add configurable interval
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_millis(self.block_interval_ms));
        let mut notifications = self.provider.canonical_state_stream();

        let mut built_payload_stream =
            UnboundedReceiverStream::new(self.rx_built_payload.take().unwrap());
        // Create the first nil OrderedBlock to start the pipeline
        // if let Some(pipeline_api) = self.pipeline_api.as_ref() {
        //     let ordered_block = self.build_nil_ordered_block();
        //     if let Some(_) = pipeline_api.push_ordered_block(ordered_block) {
        //         info!("Push nil ordered block successfully");
        //     } else {
        //         error!("Push nil ordered block failed");
        //     }
        // } else {
        //     error!("Pipeline API is not set");
        // }
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
                                    if let Err(e) = self.execute_pending_payload(payload).await {
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
                            self.canonical_block_number = canonical_state.tip().number();
                            info!("Canonical state updated with block number: {:?}", canonical_state.tip().number());
                            let pending_block_number = self.last_processing_payload.as_ref().map(|payload| payload.block().header().number()).unwrap_or_default();
                            if pending_block_number == self.canonical_block_number  {
                                info!("Pending block number {:?} is executed. Remove mined transactions from consensus pool.", pending_block_number);
                                let payload = self.last_processing_payload.take().unwrap();
                                let tx_hashes = payload.block().body().transactions().iter().map(|tx|  tx.tx_hash().clone()).collect::<HashSet<TxHash>>();
                                // let tx_hashes = execution_payload
                                //     .transactions()
                                //     .iter()
                                //     .map(|tx| calculate_tx_hash(tx))
                                //     .collect::<HashSet<TxHash>>();
                                 //Remove pending buffer
                                self.consensus_pool.remove_mined_transactions(pending_block_number, &tx_hashes);
                                // Try to execute next built payload
                                if let Some(next_payload) = self.payload_buffer.pop_front() {
                                    info!("Execute next payload from buffer {:?}.", next_payload.block().header().number());
                                    self.last_processing_payload.replace(next_payload);
                                    if let Err(e) = self.execute_pending_payload(self.last_processing_payload.as_ref().unwrap()).await {
                                        error!("Execute pending payload failed: {:?}", e);
                                    }
                                }
                            } else {
                                info!("last executed block number is {:?}. Current canonical state number is {:?}. Skip removing mined transactions.",
                                pending_block_number, self.canonical_block_number );
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
                    if last_built_block_number == self.canonical_block_number || self.last_built_payload.is_none() {
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
