use crate::consensus::ConsensusPool;
use alloy_consensus::transaction::TxHashRef;
use alloy_consensus::BlockHeader;
use alloy_primitives::{Address, TxHash, B256};
use alloy_rpc_types_engine::PayloadAttributes;
use anyhow::Result;
use futures_util::StreamExt;
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
pub struct MysticetiConsensus<Provider, Payload, Pool>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
{
    consensus_pool: Arc<ConsensusPool<Pool>>,
    //payload_builder_handle: PayloadBuilderHandle<Payload>,
    rx_built_payload: Option<UnboundedReceiver<Payload::BuiltPayload>>,
    engine_handle: BeaconConsensusEngineHandle<Payload>,
    provider: Provider,
    // Keep track of the canonical state number
    block_build_interval: u64,
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
}

impl<Provider, Payload, Pool> MysticetiConsensus<Provider, Payload, Pool>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
{
    pub fn new(
        consensus_pool: Arc<ConsensusPool<Pool>>,
        provider: Provider,
        //payload_builder_handle: PayloadBuilderHandle<Payload>,
        rx_built_payload: UnboundedReceiver<Payload::BuiltPayload>,
        engine_handle: BeaconConsensusEngineHandle<Payload>,
        block_build_interval: u64,
    ) -> Self {
        Self {
            consensus_pool,
            //payload_builder_handle,
            rx_built_payload: Some(rx_built_payload),
            engine_handle,
            provider,
            block_build_interval,
            canonical_block_number: 0,
            last_processing_payload: None,
            last_built_payload: None,
            payload_buffer: VecDeque::new(),
        }
    }
}

impl<Provider, Payload, Pool> MysticetiConsensus<Provider, Payload, Pool>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
{
    // fn has_proposal_block(&self) -> bool {
    //     self.proposal_block.is_some()
    // }
    // fn proposal_block_executed(&self) -> Option<bool> {
    //     self.proposal_block
    //         .as_ref()
    //         .map(|proposal_block| proposal_block.is_executed())
    // }
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

    // async fn retrieve_payload(
    //     &self,
    //     payload_id: PayloadId,
    // ) -> Result<Option<Payload::BuiltPayload>> {
    //     self.payload_builder_handle
    //         .best_payload(payload_id)
    //         .await
    //         .transpose()
    //         .map_err(anyhow::Error::msg)
    // }

    /// Try to execute the pending payload
    async fn execute_pending_payload(
        &self,
        built_payload: &Payload::BuiltPayload,
    ) -> Result<Option<Payload::ExecutionData>> {
        debug!(
            "Execute built payload {:?}",
            built_payload.block().header().number()
        );

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
                    Ok(Some(execution_payload))
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

impl<Provider, Payload, Pool> MysticetiConsensus<Provider, Payload, Pool>
where
    Provider: ChainSpecProvider + StateProviderFactory + CanonStateSubscriptions + Unpin + 'static,
    Payload: PayloadTypes<PayloadAttributes = EthPayloadAttributes>,
    Pool: TransactionPool,
{
    pub async fn start(&mut self) -> Result<()> {
        //TODO: Add configurable interval
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(
            self.block_build_interval,
        ));
        let mut notifications = self.provider.canonical_state_stream();
        // let mut payload_events = self
        //     .payload_builder_handle
        //     .subscribe()
        //     .await
        //     .map(|events| events.into_built_payload_stream())
        //     .map_err(|e| anyhow::anyhow!("Failed to subscribe to payload events: {:?}", e))?;
        let mut built_payload_stream =
            UnboundedReceiverStream::new(self.rx_built_payload.take().unwrap());
        loop {
            tokio::select! {
                // new_event = payload_events.next() => {
                //     debug!("Received payload events updated");
                //     match new_event {
                //         Some(new_payload) => {
                //             debug!("New payload built, put it to the buffer. New payload number {}. Payload buffer size: {:?}",
                //                 new_payload.block().header().number(),
                //                 self.payload_buffer.len());
                //             self.last_built_payload.replace(new_payload.clone());
                //             self.payload_buffer.push_back(new_payload);
                //         },
                //         None => {
                //             debug!("Payload events updated: None");
                //         }
                //     }
                // }
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
                            // self.payload_builder_handle
                            //     .best_payload(payload_id)
                            //     .await
                            //     .transpose()
                            //     .map_err(anyhow::Error::msg)

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
                last_committed_subdag.commit_ref.index - first_committed_subdag.commit_ref.index
                    + 1,
                first_committed_subdag.commit_ref.index,
                first_committed_subdag.timestamp_ms,
                first_committed_subdag.leader.round,
                last_committed_subdag.commit_ref.index,
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
    // async fn process_proposal_block(
    //     &mut self,
    // ) -> Result<Option<<EthEngineTypes as PayloadTypes>::ExecutionData>> {
    //     assert!(self.has_proposal_block());
    //     let proposal_block_executed = self.proposal_block_executed();
    //     if proposal_block_executed == Some(true) {
    //         let proposal_block = self.proposal_block.as_ref().unwrap();
    //         trace!(
    //             "Current proposal block with payload id {:?} is executed. Try to build next one.",
    //             proposal_block.payload_id
    //         );
    //         match self
    //             .build_next_proposal_block(proposal_block.built_payload.clone())
    //             .await
    //         {
    //             Ok(Some(payload_id)) => {
    //                 debug!(
    //                     "Build next proposal block successfully. Pending payload id: {:?}",
    //                     payload_id
    //                 );
    //             }
    //             Ok(None) => {}
    //             Err(e) => {
    //                 error!("Build next proposal block failed: {:?}", e);
    //             }
    //         }
    //         return Ok(None);
    //     }
    //     // Proposal block is processing
    //     let (payload_id, mut built_payload) = {
    //         let proposal_block = self.proposal_block.as_ref().unwrap();
    //         (
    //             proposal_block.payload_id.clone(),
    //             proposal_block.built_payload.clone(),
    //         )
    //     };
    //     if built_payload.is_none() {
    //         debug!(
    //             "Payload {:?} is not built. Try to get it from payload builder.",
    //             payload_id
    //         );
    //         if let Ok(Some(payload)) = self.retrieve_payload(payload_id).await {
    //             built_payload.replace(payload.clone());
    //             self.proposal_block.as_mut().unwrap().set_payload(payload);
    //         }
    //     }
    //     if let Some(built_payload) = built_payload {
    //         debug!("Payload {:?} is built. Try to execute it.", payload_id);
    //         match self.execute_pending_payload(&built_payload).await {
    //             Ok(Some(execution_payload)) => {
    //                 //Set proposal block as executed
    //                 self.proposal_block.as_mut().unwrap().set_executed();
    //                 return Ok(Some(execution_payload));
    //             }
    //             Ok(None) => {
    //                 debug!(
    //                     "Payload {:?} is not executed. Try to execute it later",
    //                     payload_id
    //                 );
    //                 return Ok(None);
    //             }
    //             Err(e) => {
    //                 error!("Execute pending payload failed: {:?}", e);
    //                 return Err(anyhow::anyhow!("Execute pending payload failed: {:?}", e));
    //             }
    //         }
    //     }
    //     Ok(None)
    // }
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
