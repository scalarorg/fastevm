use alloy_primitives::{Address, B256};
use alloy_rpc_types_eth::TransactionTrait;
use anyhow::Result;
use reth_ethereum::{
    chainspec::{ChainSpecProvider, EthChainSpec},
    evm::revm::database::EvmStateProvider,
    node::{
        api::{
            BeaconConsensusEngineHandle, BuiltPayload, EngineApiMessageVersion, ExecutionPayload,
            PayloadTypes,
        },
        engine::EthPayloadAttributes,
    },
    rpc::types::engine::{ForkchoiceState, ForkchoiceUpdated},
    storage::StateProviderFactory,
};
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_extension::{CommittedSubDag, CommittedTransactions};
use reth_payload_builder::{PayloadBuilderHandle, PayloadId};
use reth_transaction_pool::{PoolTransaction, TransactionOrigin, TransactionPool};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver};
use tracing::{debug, error, info, trace, warn};

use crate::consensus::ConsensusPool;

const BLOCK_INTERVAL: u64 = 1000; //1 second
const PAYLOAD_EXECUTION_TIMEOUT: u64 = 30; // 30 second timeout
const PAYLOAD_EXECUTION_INTERVAL: u64 = 500; // 10 millisecond interval
pub struct MysticetiConsensus<Provider, Payload, Pool>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
{
    subdag_rx: UnboundedReceiver<CommittedSubDag>,
    consensus_pool: Arc<Mutex<ConsensusPool<Pool>>>,
    transaction_pool: Pool,
    payload_builder_handle: PayloadBuilderHandle<Payload>,
    engine_handle: BeaconConsensusEngineHandle<Payload>,
    provider: Provider,
    //Keep track of the last payload
    //This payload is used to build new forkchoice state and next payloadAttribute
    last_payload: Option<Payload::ExecutionData>,
    pending_payload_id: Option<PayloadId>,
}

impl<Provider, Payload, Pool> MysticetiConsensus<Provider, Payload, Pool>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
{
    pub fn new(
        subdag_rx: UnboundedReceiver<CommittedSubDag>,
        consensus_pool: Arc<Mutex<ConsensusPool<Pool>>>,
        transaction_pool: Pool,
        provider: Provider,
        payload_builder_handle: PayloadBuilderHandle<Payload>,
        engine_handle: BeaconConsensusEngineHandle<Payload>,
    ) -> Self {
        Self {
            subdag_rx,
            consensus_pool,
            transaction_pool,
            payload_builder_handle,
            engine_handle,
            provider,
            last_payload: None,
            pending_payload_id: None,
        }
    }
}

impl<Provider, Payload, Pool> MysticetiConsensus<Provider, Payload, Pool>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
{
    /// Process a single subdag
    async fn process_single_subdag(&mut self, subdag: CommittedSubDag) -> Result<()> {
        let committed_transactions = CommittedTransactions::<Pool::Transaction>::try_from(subdag)?;
        //Add subdag to transaction pool
        self.update_pool_with_transactions(&committed_transactions)
            .await?;
        //Add committed transactions to consensuspool
        let mut consensus_pool = self
            .consensus_pool
            .lock()
            .map_err(|_e| anyhow::anyhow!("Failed to lock subdag queue"))?;
        consensus_pool.add_committed_transactions(committed_transactions);
        debug!(
            "Subdag queued successfully. Queue size: {}",
            consensus_pool.queue_size()
        );
        Ok(())
    }

    async fn update_pool_with_transactions(
        &mut self,
        committed_transactions: &CommittedTransactions<Pool::Transaction>,
    ) -> Result<()> {
        let mut added_count = 0;
        let pending_txs = self.transaction_pool.pending_transactions();
        let mut pending_txs_map = HashMap::new();
        for tx in pending_txs {
            pending_txs_map.insert(tx.hash().clone(), Arc::clone(&tx));
        }
        let mut subscribed_txs = Vec::new();
        let mut missing_txs = HashSet::new();
        for tx in committed_transactions.transactions.iter() {
            let tx_hash = tx.hash();
            //If transaction is already in the pending pool, skip
            if pending_txs_map.contains_key(tx_hash) {
                continue;
            }
            if !self.transaction_pool.contains(tx_hash) {
                missing_txs.insert(tx_hash.clone());
                //TODO: consider add transaction here
                // Or wait until all of them are added to the pending pool throught p2p network
                match self
                    .transaction_pool
                    .add_transaction_and_subscribe(TransactionOrigin::External, tx.clone())
                    .await
                {
                    Ok(tx_events) => {
                        subscribed_txs.push(tx_events);
                        added_count += 1;
                        debug!(
                            "Added subdag transaction to pool: {:?}, sender: {:?}, nonce: {:?}",
                            tx_hash,
                            tx.sender_ref(),
                            tx.nonce()
                        );
                    }
                    Err(e) => {
                        error!("Add subdag transaction {:?}, sender: {:?}, nonce: {:?} to pool failed: {:?}", 
                            tx_hash, tx.sender_ref(), tx.nonce(), e);
                    }
                }
            } else {
                debug!("Transaction already in pool: {:?}", tx_hash);
            }
        }
        debug!(
            "Added {}/{} subdag transactions to pool",
            added_count,
            committed_transactions.transactions.len()
        );
        //TODO: Try add waiting transactions to the pending subpool
        Ok(())
    }
    /// Get current forkchoice state
    async fn create_forkchoice_state(&self) -> ForkchoiceState {
        //Todo: Implement this
        match &self.last_payload {
            Some(payload) => {
                let block_hash = payload.block_hash();
                ForkchoiceState {
                    head_block_hash: block_hash,
                    safe_block_hash: block_hash,
                    finalized_block_hash: block_hash,
                }
            }
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

    async fn retrieve_payload(
        &self,
        payload_id: PayloadId,
    ) -> Result<Option<Payload::BuiltPayload>> {
        self.payload_builder_handle
            .best_payload(payload_id)
            .await
            .transpose()
            .map_err(anyhow::Error::msg)
    }

    /// Try to execute the pending payload
    async fn try_execute_pending_payload(
        &self,
        payload_id: PayloadId,
    ) -> Result<Option<Payload::ExecutionData>> {
        match self.retrieve_payload(payload_id).await? {
            Some(pending_payload) => {
                debug!(
                "Payload built successfully. Update last payload to {:?} and clear pending payload id:",
                pending_payload
            );
                let execution_payload = Payload::block_to_payload(pending_payload.block().clone());
                match self
                    .engine_handle
                    .new_payload(execution_payload.clone())
                    .await
                {
                    Ok(payload_status) => {
                        if payload_status.is_valid() {
                            info!("New payload sent successfully");
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
            None => Err(anyhow::anyhow!("Payload {:?} not found", payload_id)),
        }
    }
    /// Get the current queue size
    pub async fn queue_size(&self) -> Result<usize> {
        let consensus_pool = self
            .consensus_pool
            .lock()
            .map_err(|_e| anyhow::anyhow!("Failed to lock subdag queue"))?;
        Ok(consensus_pool.queue_size())
    }
}

impl<Provider, Pool> MysticetiConsensus<Provider, EthEngineTypes, Pool>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Pool: TransactionPool,
{
    pub async fn start(&mut self) {
        //TODO: Add configurable interval
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_millis(BLOCK_INTERVAL));
        loop {
            match &self.pending_payload_id {
                //If there is a pending payload id, try to get the payload and execute it in the engine
                Some(payload_id) => {
                    debug!("Try to execute pending payload: {:?}", payload_id);
                    let result = self.try_execute_pending_payload(payload_id.clone()).await;
                    match result {
                        Ok(Some(execution_payload)) => {
                            let forkchoice_state = self.create_forkchoice_state().await;
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
                                    debug!(
                                        "Forkchoice updated successfully with payload {:?} and status: {:?}",
                                        payload_id,
                                        payload_status
                                    );
                                }
                                Err(e) => {
                                    error!("Forkchoice updated failed: {:?}", e);
                                }
                            }
                            self.last_payload = Some(execution_payload);
                            self.pending_payload_id = None;
                            debug!("Try to execute pending payload successfully");
                        }
                        Ok(None) => {
                            debug!(
                                "Payload {:?} is not executed. Try to execute it later",
                                payload_id
                            );
                        }
                        Err(e) => {
                            error!("try execute pending payload failed: {:?}", e);
                        }
                    }
                }
                //If no pending payload id, build next payload
                None => {
                    debug!("No pending payload id. Build next payload.");
                    let queue_size = self.queue_size().await.unwrap_or_default();
                    if queue_size > 0 {
                        debug!(
                            "Queue size: {}. Start processing by building next payload or executing pending payload.",
                            queue_size
                        );
                        match self.build_next_payload().await {
                            Ok(Some(payload_id)) => {
                                debug!(
                                    "Build next payload successfully. Pending payload id: {:?}",
                                    payload_id
                                );
                                self.pending_payload_id.replace(payload_id);
                            }
                            Ok(None) => {
                                debug!("Build next payload successfully without payload id");
                            }
                            Err(e) => {
                                error!("Build next payload failed: {:?}", e);
                            }
                        }
                    } else {
                        trace!("Queue is empty. Waiting for subdag.");
                    }
                }
            }

            //Try get subdag from channel and add to queue
            match self.subdag_rx.try_recv() {
                Ok(subdag) => {
                    //Receive subdag from channel and update local transaction pool
                    //Add subdag to consensus pool for further processing
                    let result = self.process_single_subdag(subdag).await;
                    if result.is_err() {
                        error!("Process single subdag failed: {:?}", result);
                    }
                }
                Err(e) => match e {
                    TryRecvError::Disconnected => {
                        error!("Subdag channel closed, stopping consensus loop");
                        break;
                    }
                    TryRecvError::Empty => {
                        trace!("No subdag received from channel: {:?}", e);
                    }
                },
            }
            //Update forkchoice state and payload attributes
            interval.tick().await;
        }
    }
    /// Create next payload attributes
    async fn create_payload_attributes(
        &self,
    ) -> Option<<EthEngineTypes as PayloadTypes>::PayloadAttributes> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        //TODO:
        //1. Get actual prev_randao from the previous block's header
        //2. Check and create withdrawals vector
        //3. Set suggested fee recipient

        let attributes = match &self.last_payload {
            Some(payload) => {
                debug!(
                    "Last payload number: {:?} with timestamp: {:?}. Next timestamp: {:?}",
                    payload.block_number(),
                    payload.timestamp(),
                    timestamp
                );
                if timestamp < payload.timestamp() {
                    warn!("Timestamp is less than last payload timestamp. Waiting for next iteration.");
                    return None;
                }
                let attributes = EthPayloadAttributes {
                    timestamp,
                    prev_randao: B256::random(),
                    suggested_fee_recipient: Address::random(),
                    withdrawals: Default::default(),
                    parent_beacon_block_root: Some(payload.block_hash()),
                };
                Some(attributes)
            }
            None => {
                let chain_spec = self.provider.chain_spec();
                let attributes = EthPayloadAttributes {
                    timestamp,
                    prev_randao: B256::random(),
                    suggested_fee_recipient: Address::random(),
                    withdrawals: Default::default(),
                    parent_beacon_block_root: Some(chain_spec.genesis_hash()),
                };
                Some(attributes)
            }
        };
        attributes
    }
    pub async fn build_next_payload(&mut self) -> Result<Option<PayloadId>> {
        // //Befor build next payload, try to get latest state, and put it into consensus_pool
        // let latest_state = self.provider.latest();
        // if let Ok(latest_state) = latest_state {
        //     let mut consensus_pool = self
        //         .consensus_pool
        //         .lock()
        //         .map_err(|_e| anyhow::anyhow!("Failed to lock consensus pool"))?;
        //     consensus_pool.set_latest_state(latest_state);
        // }
        let forkchoice_state = self.create_forkchoice_state().await;
        let payload_attributes = self.create_payload_attributes().await;
        let ForkchoiceUpdated {
            payload_status,
            payload_id,
        } = self
            .engine_handle
            .fork_choice_updated(
                forkchoice_state,
                payload_attributes,
                EngineApiMessageVersion::default(),
            )
            .await
            .map_err(anyhow::Error::msg)?;
        info!(
            "Forkchoice updated: Payload status: {:?}; Payload id: {:?}",
            payload_status, payload_id
        );
        if payload_status.is_valid() {
            if let Some(payload_id) = payload_id {
                info!(
                    "Forkchoice updated successfully. Update pending payload id to: {:?}.",
                    payload_id
                );
                return Ok(Some(payload_id));
            } else {
                info!("Forkchoice updated successfully without payload id");
            }
        } else {
            info!("Forkchoice update failed: {:?}", payload_status);
        }
        Ok(None)
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
