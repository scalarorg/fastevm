use alloy_consensus::BlockHeader;
use alloy_primitives::{Address, TxHash, B256};
use alloy_rpc_types_engine::PayloadAttributes;
use alloy_rpc_types_eth::TransactionTrait;
use anyhow::Result;
use reth_ethereum::{
    chainspec::{ChainSpecProvider, EthChainSpec},
    node::api::{
        BeaconConsensusEngineHandle, BuiltPayload, EngineApiMessageVersion, ExecutionPayload,
        PayloadTypes,
    },
    rpc::types::engine::{ForkchoiceState, ForkchoiceUpdated},
    storage::StateProviderFactory,
};
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_extension::{CommittedSubDag, CommittedTransactions};
use reth_payload_builder::{PayloadBuilderHandle, PayloadId};
use reth_transaction_pool::{
    PoolTransaction, TransactionOrigin, TransactionPool, ValidPoolTransaction,
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc::{error::TryRecvError, Receiver, UnboundedReceiver};
use tracing::{debug, error, info, trace, warn};

use crate::consensus::{ConsensusPool, ProposalBlock};

const BLOCK_INTERVAL: u64 = 1000; //1 second
const WAITING_PENDING_TXS_TIMEOUT: u64 = 3000; // 10 second timeout
const WAITING_PENDING_TXS_INTERVAL: u64 = 100; // 1 second interval

//const PAYLOAD_EXECUTION_TIMEOUT: u64 = 30; // 30 second timeout
//const PAYLOAD_EXECUTION_INTERVAL: u64 = 500; // 10 millisecond interval
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
    proposal_block: Option<ProposalBlock<Payload>>,
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
            proposal_block: None,
        }
    }
}

impl<Provider, Payload, Pool> MysticetiConsensus<Provider, Payload, Pool>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
{
    fn has_proposal_block(&self) -> bool {
        self.proposal_block.is_some()
    }
    fn proposal_block_executed(&self) -> Option<bool> {
        self.proposal_block
            .as_ref()
            .map(|proposal_block| proposal_block.is_executed())
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

    /// Process a single subdag
    async fn process_single_subdag(&mut self, subdag: CommittedSubDag) -> Result<()> {
        let committed_transactions = CommittedTransactions::<Pool::Transaction>::try_from(subdag)?;
        //Add subdag to transaction pool
        let pooled_txs = self
            .update_pool_with_transactions(&committed_transactions)
            .await?;
        //Add committed transactions to consensuspool
        {
            let mut consensus_pool = self
                .consensus_pool
                .lock()
                .map_err(|_e| anyhow::anyhow!("Failed to lock subdag queue"))?;
            consensus_pool.add_committed_transactions(committed_transactions, pooled_txs);
            debug!(
                "Subdag queued successfully. Queue size: {}",
                consensus_pool.queue_size()
            );
        } // MutexGuard is dropped here
        Ok(())
    }

    async fn update_pool_with_transactions(
        &mut self,
        committed_transactions: &CommittedTransactions<Pool::Transaction>,
    ) -> Result<Vec<Arc<ValidPoolTransaction<Pool::Transaction>>>> {
        //Get pending transactions listener for waiting all missing transactions are put to the pending subpool of timeout
        let pending_tx_listener = self.transaction_pool.pending_transactions_listener();
        let mut waiting_txs: HashSet<TxHash> = HashSet::new();
        let external_pending_txs = self
            .transaction_pool
            .get_pending_transactions_by_origin(TransactionOrigin::External)
            .iter()
            .map(|tx| tx.hash().clone())
            .collect::<HashSet<_>>();
        let mut added_count = 0;
        let mut pooled_txs = Vec::new();

        // First pass: add transactions to pool and collect waiting transactions
        for tx in committed_transactions.transactions.iter() {
            let tx_hash = tx.hash();
            //Add committed transaction to pool if missing
            let mut pooled_tx = self.transaction_pool.get(tx_hash);
            if pooled_tx.is_none() {
                debug!(
                    "Added subdag transaction to pool: {:?}, sender: {:?}, nonce: {:?}",
                    tx_hash,
                    tx.sender_ref(),
                    tx.nonce()
                );
                added_count += 1;
                pooled_tx = self
                    .transaction_pool
                    .add_external_transaction(tx.as_ref().clone())
                    .await
                    .map(|tx_hash| self.transaction_pool.get(&tx_hash))
                    .ok()
                    .flatten();
                //Add transaction to waiting transactions
                waiting_txs.insert(tx_hash.clone());
            }
            if let Some(pooled_tx) = pooled_tx {
                if !external_pending_txs.contains(tx_hash) {
                    //Add transaction to waiting transactions
                    waiting_txs.insert(tx_hash.clone());
                }
                pooled_txs.push(pooled_tx);
            }
        }

        debug!(
            "Added {}/{} subdag transactions to pool. Waiting for {} transactions to arrive in pending subpool",
            added_count,
            committed_transactions.transactions.len(),
            waiting_txs.len()
        );

        // Wait for all waiting transactions to arrive in pending subpool or timeout
        self.waiting_for_pending_txs(&mut waiting_txs, pending_tx_listener)
            .await;

        Ok(pooled_txs)
    }

    /// Wait for all waiting transactions to arrive in pending subpool or timeout
    async fn waiting_for_pending_txs(
        &self,
        waiting_txs: &mut HashSet<TxHash>,
        mut pending_tx_listener: Receiver<TxHash>,
    ) {
        if waiting_txs.is_empty() {
            return;
        }

        let timeout_duration = std::time::Duration::from_millis(WAITING_PENDING_TXS_TIMEOUT);
        let start_time = std::time::Instant::now();

        debug!(
            "Waiting for {} transactions to arrive in pending subpool: {:?}",
            waiting_txs.len(),
            waiting_txs
        );

        while !waiting_txs.is_empty() && start_time.elapsed() < timeout_duration {
            // Wait for next transaction event or timeout
            match pending_tx_listener.try_recv() {
                Ok(tx_hash) => {
                    let removed = waiting_txs.remove(&tx_hash);
                    if removed {
                        debug!(
                            "Transaction {:?} arrived in pending subpool. Remaining: {}",
                            tx_hash,
                            waiting_txs.len()
                        );
                    }
                }
                Err(e) => match e {
                    TryRecvError::Empty => {
                        debug!(
                            "No pending transaction received. Sleeping for {} milliseconds",
                            WAITING_PENDING_TXS_INTERVAL
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(
                            WAITING_PENDING_TXS_INTERVAL,
                        ))
                        .await;
                    }
                    TryRecvError::Disconnected => {
                        warn!("Pending transaction listener channel closed");
                        break;
                    }
                },
            }
        }
        pending_tx_listener.close();
        if !waiting_txs.is_empty() {
            warn!(
                "Timeout waiting for {} transactions to arrive in pending subpool: {:?}",
                waiting_txs.len(),
                waiting_txs
            );
        } else {
            debug!("All waiting transactions arrived in pending subpool");
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
                    info!("New payload sent successfully");
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
            //Try process proposal block
            let has_proposal_block = self.has_proposal_block();
            if has_proposal_block {
                match self.process_proposal_block().await {
                    Ok(Some(execution_payload)) => {
                        debug!(
                            "Process proposal block successfully. Execution payload: {:?}",
                            execution_payload
                        );
                        {
                            self.consensus_pool
                                .lock()
                                .unwrap()
                                .remove_mined_transactions(&execution_payload.payload);
                        } // MutexGuard is dropped here
                    }
                    Ok(None) => {
                        debug!("Process proposal block successfully without execution payload");
                    }
                    Err(e) => {
                        error!("Process proposal block failed: {:?}", e);
                    }
                }
            } else {
                debug!("No proposal block. Try build next one.");
                match self.build_next_proposal_block(None).await {
                    Ok(Some(payload_id)) => {
                        debug!(
                            "Build next proposal block successfully. Pending payload id: {:?}",
                            payload_id
                        );
                    }
                    Ok(None) => {
                        debug!("Build next proposal block successfully without payload id");
                    }
                    Err(e) => {
                        error!("Build next proposal block failed: {:?}", e);
                    }
                }
            }

            //Try get subdag from channel and add to queue
            match self.subdag_rx.try_recv() {
                Ok(subdag) => {
                    //Receive subdag from channel and update local transaction pool
                    //Add subdag to consensus pool for further processing
                    debug!("Received committed subdag: {:?}", subdag);
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

    pub async fn build_next_proposal_block(
        &mut self,
        last_built_payload: Option<<EthEngineTypes as PayloadTypes>::BuiltPayload>,
    ) -> Result<Option<PayloadId>> {
        let committed_transactions = {
            self.consensus_pool
                .lock()
                .unwrap()
                .next_committed_transactions()
                .cloned()
        };
        if let Some(committed_transactions) = committed_transactions {
            debug!(
                "Create proposal block with committed transactions: Index: {:?}, Round: {:?}, Author: {:?}, Timestamp: {:?}, Number of transactions: {:?}",
                committed_transactions.commit_ref.index,
                committed_transactions.leader.round,
                committed_transactions.leader.author,
                committed_transactions.timestamp_ms,
                committed_transactions.transactions.len()
            );
            let last_block_hash = last_built_payload
                .as_ref()
                .map(|payload| payload.block().hash());
            let forkchoice_state = self.create_forkchoice_state(last_block_hash).await;
            //Create payload attributes with timestamp in seconds
            let payload_attributes = self
                .create_payload_attributes(
                    committed_transactions.timestamp_ms / 1000,
                    last_built_payload,
                )
                .await;
            let ForkchoiceUpdated {
                payload_status,
                payload_id,
            } = self
                .engine_handle
                .fork_choice_updated(
                    forkchoice_state,
                    Some(payload_attributes.clone()),
                    EngineApiMessageVersion::default(),
                )
                .await
                .map_err(anyhow::Error::msg)?;

            if payload_status.is_valid() {
                if let Some(payload_id) = payload_id {
                    info!(
                        "Forkchoice updated successfully. Create proposal block with payload id: {:?}.",
                        payload_id
                    );
                    self.proposal_block =
                        Some(ProposalBlock::new(payload_id.clone(), payload_attributes));
                    return Ok(Some(payload_id));
                } else {
                    info!("Forkchoice updated successfully without payload id");
                }
            } else {
                info!("Forkchoice update failed: {:?}", payload_status);
            }
        }
        Ok(None)
    }
    /// Create next payload attributes
    /// This attributes must be equals on all the nodes
    async fn create_payload_attributes(
        &self,
        timestamp: u64,
        last_build_payload: Option<<EthEngineTypes as PayloadTypes>::BuiltPayload>,
    ) -> <EthEngineTypes as PayloadTypes>::PayloadAttributes {
        //TODO:
        //1. Get actual prev_randao from the previous block's header
        //2. Check and create withdrawals vector
        //3. Set suggested fee recipient
        debug!("Create payload attributes with timestamp: {:?}", timestamp);
        match last_build_payload {
            Some(payload) => {
                let last_block_number = payload.block().header().number();
                let last_block_hash = payload.block().hash();
                let last_block_timestamp = payload.block().header().timestamp();
                debug!(
                    "Last payload number: {:?} with timestamp: {:?}. Next timestamp: {:?}",
                    last_block_number, last_block_timestamp, timestamp
                );

                PayloadAttributes {
                    timestamp,
                    prev_randao: B256::default(),
                    suggested_fee_recipient: Address::default(),
                    withdrawals: Default::default(),
                    parent_beacon_block_root: Some(last_block_hash),
                }
            }
            None => {
                let chain_spec = self.provider.chain_spec();
                PayloadAttributes {
                    timestamp,
                    prev_randao: B256::default(),
                    suggested_fee_recipient: Address::default(),
                    withdrawals: Default::default(),
                    parent_beacon_block_root: Some(chain_spec.genesis_hash()),
                }
            }
        }
    }
    async fn process_proposal_block(
        &mut self,
    ) -> Result<Option<<EthEngineTypes as PayloadTypes>::ExecutionData>> {
        assert!(self.has_proposal_block());
        let proposal_block_executed = self.proposal_block_executed();
        if proposal_block_executed == Some(true) {
            let proposal_block = self.proposal_block.as_ref().unwrap();
            debug!(
                "Current proposal block with payload id {:?} is executed. Try to build next one.",
                proposal_block.payload_id
            );
            match self
                .build_next_proposal_block(proposal_block.built_payload.clone())
                .await
            {
                Ok(Some(payload_id)) => {
                    debug!(
                        "Build next proposal block successfully. Pending payload id: {:?}",
                        payload_id
                    );
                }
                Ok(None) => {
                    debug!("Build next proposal block successfully without payload id");
                }
                Err(e) => {
                    error!("Build next proposal block failed: {:?}", e);
                }
            }
            return Ok(None);
        }
        // Proposal block is processing
        let (payload_id, mut built_payload) = {
            let proposal_block = self.proposal_block.as_ref().unwrap();
            (
                proposal_block.payload_id.clone(),
                proposal_block.built_payload.clone(),
            )
        };
        if built_payload.is_none() {
            debug!(
                "Payload {:?} is not built. Try to get it from payload builder.",
                payload_id
            );
            if let Ok(Some(payload)) = self.retrieve_payload(payload_id).await {
                built_payload.replace(payload.clone());
                self.proposal_block.as_mut().unwrap().set_payload(payload);
            }
        }
        if let Some(built_payload) = built_payload {
            debug!("Payload {:?} is built. Try to execute it.", payload_id);
            match self.execute_pending_payload(&built_payload).await {
                Ok(Some(execution_payload)) => {
                    //Set proposal block as executed
                    self.proposal_block.as_mut().unwrap().set_executed();
                    return Ok(Some(execution_payload));
                }
                Ok(None) => {
                    debug!(
                        "Payload {:?} is not executed. Try to execute it later",
                        payload_id
                    );
                    return Ok(None);
                }
                Err(e) => {
                    error!("Execute pending payload failed: {:?}", e);
                    return Err(anyhow::anyhow!("Execute pending payload failed: {:?}", e));
                }
            }
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
