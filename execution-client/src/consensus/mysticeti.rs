use alloy_primitives::{Address, B256};
use anyhow::Result;
use reth_ethereum::{
    chainspec::{ChainSpecProvider, EthChainSpec},
    node::{
        api::{
            BeaconConsensusEngineHandle, BuiltPayload, EngineApiMessageVersion, ExecutionPayload,
            PayloadTypes,
        },
        engine::EthPayloadAttributes,
    },
    rpc::types::engine::{ForkchoiceState, ForkchoiceUpdated},
};

use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_extension::CommittedSubDag;
use reth_payload_builder::{PayloadBuilderHandle, PayloadId};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::info;

pub struct MysticetiConsensus<Provider, Payload>
where
    Provider: ChainSpecProvider + Unpin + 'static,
    Payload: PayloadTypes,
{
    subdag_rx: UnboundedReceiver<CommittedSubDag>,
    subdag_queue: Arc<Mutex<VecDeque<CommittedSubDag>>>,
    payload_builder_handle: PayloadBuilderHandle<Payload>,
    engine_handle: BeaconConsensusEngineHandle<Payload>,
    provider: Provider,
    //Keep track of the last payload
    //This payload is used to build new forkchoice state and next payloadAttribute
    last_payload: Option<Payload::ExecutionData>,
    pending_payload_id: Option<PayloadId>,
}

impl<Provider, Payload> MysticetiConsensus<Provider, Payload>
where
    Provider: ChainSpecProvider + Unpin + 'static,
    Payload: PayloadTypes,
{
    pub fn new(
        subdag_rx: UnboundedReceiver<CommittedSubDag>,
        subdag_queue: Arc<Mutex<VecDeque<CommittedSubDag>>>,
        provider: Provider,
        payload_builder_handle: PayloadBuilderHandle<Payload>,
        engine_handle: BeaconConsensusEngineHandle<Payload>,
    ) -> Self {
        Self {
            subdag_rx,
            subdag_queue,
            payload_builder_handle,
            engine_handle,
            provider,
            last_payload: None,
            pending_payload_id: None,
        }
    }
}

impl<Provider, Payload> MysticetiConsensus<Provider, Payload>
where
    Provider: ChainSpecProvider + Unpin + 'static,
    Payload: PayloadTypes,
{
    /// Process a single subdag
    async fn process_single_subdag(&mut self, subdag: CommittedSubDag) -> Result<()> {
        //Add subdag to queue
        let mut queue_guard = self
            .subdag_queue
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock subdag queue"))?;
        queue_guard.push_back(subdag.clone());
        info!(
            "Subdag queued successfully. Queue size: {}",
            queue_guard.len()
        );
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
    async fn try_execute_pending_payload(&mut self, payload_id: PayloadId) -> Result<()> {
        let pending_payload = self.retrieve_payload(payload_id).await?;
        if let Some(pending_payload) = pending_payload {
            info!(
                "Payload built successfully. Update last payload to {:?} and clear pending payload id:",
                pending_payload
            );
            self.pending_payload_id = None;
            let execution_payload = Payload::block_to_payload(pending_payload.block().clone());
            self.last_payload = Some(execution_payload.clone());
            let result = self.engine_handle.new_payload(execution_payload).await;
            if result.is_err() {
                info!("New payload failed: {:?}", result);
            }
        } else {
            info!("Payload not found");
        }

        Ok(())
    }
    /// Get the current queue size
    pub async fn queue_size(&self) -> Result<usize> {
        let queue_guard = self
            .subdag_queue
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock subdag queue"))?;
        Ok(queue_guard.len())
    }
}

impl<Provider> MysticetiConsensus<Provider, EthEngineTypes>
where
    Provider: ChainSpecProvider + Unpin + 'static,
{
    pub async fn start(&mut self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
        loop {
            if self.queue_size().await.unwrap_or_default() > 0 {
                info!(
                    "Queue size: {}. Start processing by building next payload or executing pending payload.",
                    self.queue_size().await.unwrap_or_default()
                );
                match &self.pending_payload_id {
                    //If there is a pending payload id, try to get the payload and execute it in the engine
                    Some(payload_id) => {
                        let result = self.try_execute_pending_payload(payload_id.clone()).await;
                        if result.is_err() {
                            info!("try execute pending payload failed: {:?}", result);
                        }
                    }
                    //If no pending payload id, build next payload
                    None => {
                        let result = self.build_next_payload().await;
                        if result.is_err() {
                            info!("Build next payload failed: {:?}", result);
                        }
                    }
                }
            }

            //Try get subdag from channel and add to queue
            if let Some(subdag) = self.subdag_rx.recv().await {
                let result = self.process_single_subdag(subdag).await;
                if result.is_err() {
                    info!("Process single subdag failed: {:?}", result);
                }
            }

            //Update forkchoice state and payload attributes
            interval.tick().await;
        }
    }
    /// Create next payload attributes
    async fn create_payload_attributes(
        &self,
    ) -> Option<<EthEngineTypes as PayloadTypes>::PayloadAttributes> {
        match &self.last_payload {
            Some(payload) => {
                let timestamp = payload.timestamp();
                let attributes = EthPayloadAttributes {
                    timestamp,
                    prev_randao: B256::random(),
                    suggested_fee_recipient: Address::random(),
                    withdrawals: Default::default(),
                    parent_beacon_block_root: Some(payload.block_hash()),
                };
                Some(attributes)
            }
            None => None,
        }
    }
    pub async fn build_next_payload(&mut self) -> Result<()> {
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
        if payload_status.is_valid() {
            if let Some(payload_id) = payload_id {
                info!(
                    "Forkchoice updated successfully. Update pending payload id to: {:?}.",
                    payload_id
                );
                self.pending_payload_id = Some(payload_id);
            } else {
                info!("Forkchoice updated successfully without payload id");
            }
        } else {
            info!("Forkchoice update failed: {:?}", payload_status);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_ethereum::pool::noop::NoopTransactionPool;
    use std::sync::Arc;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn test_committed_subdag_default() {
        let subdag = CommittedSubDag::default();
        assert!(subdag.blocks.is_empty());
        assert!(subdag.flatten_transactions().is_empty());
    }

    #[test]
    fn test_committed_subdag_flatten_transactions() {
        let subdag = CommittedSubDag::default();
        let transactions = subdag.flatten_transactions();
        assert!(transactions.is_empty());
    }

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

    #[test]
    fn test_subdag_queue_operations() {
        let subdag_queue = Arc::new(Mutex::new(VecDeque::new()));

        // Test adding to queue
        {
            let mut queue = subdag_queue.lock().unwrap();
            queue.push_back(CommittedSubDag::default());
        }

        // Test queue size
        assert_eq!(subdag_queue.lock().unwrap().len(), 1);

        // Test popping from queue
        {
            let mut queue = subdag_queue.lock().unwrap();
            let subdag = queue.pop_front();
            assert!(subdag.is_some());
        }

        // Test empty queue
        assert_eq!(subdag_queue.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_channel_operations() {
        let (subdag_tx, mut subdag_rx) = unbounded_channel();

        // Test sending and receiving
        let test_subdag = CommittedSubDag::default();
        subdag_tx.send(test_subdag).unwrap();

        // Test that we can receive it
        let received = subdag_rx.try_recv().unwrap();
        assert_eq!(received.blocks.len(), 0);
    }
}
