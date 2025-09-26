use alloy_consensus::BlockHeader;
use alloy_primitives::{Address, B256};
use alloy_rpc_types_engine::PayloadAttributes;
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
use reth_payload_builder::{PayloadBuilderHandle, PayloadId};
use reth_transaction_pool::TransactionPool;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::consensus::{ConsensusPool, ProposalBlock};

const BLOCK_INTERVAL: u64 = 3000; //3 second

// const WAITING_PENDING_TXS_TIMEOUT: u64 = 3000; // 10 second timeout
// const WAITING_PENDING_TXS_INTERVAL: u64 = 100; // 1 second interval

//const PAYLOAD_EXECUTION_TIMEOUT: u64 = 30; // 30 second timeout
//const PAYLOAD_EXECUTION_INTERVAL: u64 = 500; // 10 millisecond interval
pub struct MysticetiConsensus<Provider, Payload, Pool>
where
    Provider: ChainSpecProvider + StateProviderFactory + Unpin + 'static,
    Payload: PayloadTypes,
    Pool: TransactionPool,
{
    consensus_pool: Arc<ConsensusPool<Pool>>,
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
        consensus_pool: Arc<ConsensusPool<Pool>>,
        provider: Provider,
        payload_builder_handle: PayloadBuilderHandle<Payload>,
        engine_handle: BeaconConsensusEngineHandle<Payload>,
    ) -> Self {
        Self {
            consensus_pool,
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
                            "Process proposal block successfully. Execution payload number: {:?}, hash: {:?}",
                            execution_payload.block_number(),
                            execution_payload.block_hash()
                        );
                        {
                            self.consensus_pool
                                .remove_mined_transactions(&execution_payload.payload);
                        }
                    }
                    Ok(None) => {}
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
                    Ok(None) => {}
                    Err(e) => {
                        error!("Build next proposal block failed: {:?}", e);
                    }
                }
            }
            interval.tick().await;
        }
    }

    pub async fn build_next_proposal_block(
        &mut self,
        last_built_payload: Option<<EthEngineTypes as PayloadTypes>::BuiltPayload>,
    ) -> Result<Option<PayloadId>> {
        let committed_transactions = self.consensus_pool.last_committed_transaction_in_batch();
        if let Some(committed_transactions) = committed_transactions {
            debug!(
                "Create proposal block with committed transactions: Round: {:?}, Author: {:?}, Leader Digest: {:?}, Timestamp: {:?}, Number of transactions: {:?}",
                committed_transactions.leader.round,
                committed_transactions.leader.author,
                hex::encode(committed_transactions.leader.digest.as_ref()),
                committed_transactions.timestamp_ms,
                committed_transactions.transactions.len()
            );
            let last_block_hash = last_built_payload
                .as_ref()
                .map(|payload| payload.block().hash());
            let forkchoice_state = self.create_forkchoice_state(last_block_hash).await;
            //Create payload attributes with timestamp in seconds
            let leader_digest: [u8; 32] = committed_transactions
                .leader
                .digest
                .as_ref()
                .try_into()
                .expect("Leader digest must be exactly 32 bytes");

            let payload_attributes = self
                .create_payload_attributes(
                    committed_transactions.timestamp_ms / 1000,
                    leader_digest,
                    last_built_payload,
                )
                .await;
            debug!(
                "Forkchoice state: {:?}, Payload attributes: {:?}",
                forkchoice_state, payload_attributes
            );
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
        leader_digest: [u8; 32],
        last_build_payload: Option<<EthEngineTypes as PayloadTypes>::BuiltPayload>,
    ) -> <EthEngineTypes as PayloadTypes>::PayloadAttributes {
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
                Ok(None) => {}
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
