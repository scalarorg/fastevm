use reth_ethereum::node::api::{
    BeaconConsensusEngineHandle, BeaconForkChoiceUpdateError, EngineApiMessageVersion, PayloadTypes,
};
use reth_ethereum::rpc::types::engine::{ForkchoiceState, ForkchoiceUpdated, PayloadStatus};
use reth_ethereum::tasks::TaskExecutor;
use reth_extension::CommittedSubDag;
use reth_payload_builder::PayloadBuilderHandle;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::info;

pub struct MysticetiConsensus<Payload>
where
    Payload: PayloadTypes,
{
    subdag_rx: UnboundedReceiver<CommittedSubDag>,
    subdag_queue: Arc<Mutex<VecDeque<CommittedSubDag>>>,
    payload_builder_handle: PayloadBuilderHandle<Payload>,
    engine_handle: BeaconConsensusEngineHandle<Payload>,
}

impl<Payload> MysticetiConsensus<Payload>
where
    Payload: PayloadTypes,
{
    pub fn new(
        subdag_rx: UnboundedReceiver<CommittedSubDag>,
        payload_builder_handle: PayloadBuilderHandle<Payload>,
        engine_handle: BeaconConsensusEngineHandle<Payload>,
    ) -> Self {
        let subdag_queue = Arc::new(Mutex::new(VecDeque::new()));

        Self {
            subdag_rx,
            subdag_queue: Arc::clone(&subdag_queue),
            payload_builder_handle,
            engine_handle,
        }
    }
}

impl<Payload> MysticetiConsensus<Payload>
where
    Payload: PayloadTypes,
{
    pub async fn start(&mut self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
        loop {
            // Process subdags from the queue
            let subdag = {
                let mut queue_guard = self.subdag_queue.lock().await;
                queue_guard.pop_front()
            };
            //Process subdag from queue
            if let Some(subdag) = subdag {
                let result = self.process_single_subdag(subdag).await;
                if result.is_err() {
                    info!("Forkchoice update failed: {:?}", result);
                }
            }

            //Try get subdag from channel and add to queue
            if let Some(subdag) = self.subdag_rx.recv().await {
                self.queue_subdag(subdag).await;
            }

            //Update forkchoice state and payload attributes
            interval.tick().await;
        }
    }
    /// Create next payload attributes
    async fn create_payload_attributes(&self) -> Option<Payload::PayloadAttributes> {
        //Todo: Implement this
        None
    }
    /// Get current forkchoice state
    async fn create_forkchoice_state(&self) -> ForkchoiceState {
        //Todo: Implement this
        ForkchoiceState::default()
    }

    /// Process a single subdag
    async fn process_single_subdag(
        &self,
        subdag: CommittedSubDag,
    ) -> Result<(), BeaconForkChoiceUpdateError> {
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
            .await?;
        if payload_status.is_valid() {
            info!("Forkchoice updated successfully: {:?}", payload_id);
            if let Some(payload_id) = payload_id {
                // 1. Todo: Build next payload
                match self.payload_builder_handle.best_payload(payload_id).await {
                    Some(Ok(payload)) => {
                        info!("Payload built successfully: {:?}", payload);
                        // self.engine_handle.new_payload(payload.execution_data).await;
                    }
                    Some(Err(e)) => {
                        info!("Payload build failed: {:?}", e);
                    }
                    None => info!("Payload not found"),
                }
                // 2. Todo: Commit new payload
                //Self::commit_new_payload(&self.engine_handle).await;
            }
        } else {
            info!("Forkchoice update failed: {:?}", payload_status);
        }
        Ok(())
    }
}

impl<Payload> MysticetiConsensus<Payload>
where
    Payload: PayloadTypes,
{
    /// Add committed subdag to the queue
    pub async fn queue_subdag(&mut self, subdag: CommittedSubDag) {
        let mut queue_guard = self.subdag_queue.lock().await;
        queue_guard.push_back(subdag.clone());
        info!(
            "Subdag queued successfully. Queue size: {}",
            queue_guard.len()
        );
    }

    /// Get the current queue size
    pub async fn queue_size(&self) -> usize {
        let queue_guard = self.subdag_queue.lock().await;
        queue_guard.len()
    }
}

impl<Payload> MysticetiConsensus<Payload>
where
    Payload: PayloadTypes,
{
    pub async fn build_next_payload(subdag: &CommittedSubDag) {
        info!("Building payload for subdag: {:?}", subdag);
        // 1. Get next committed subdag from the queue
        // 2. Build a new payload with transactions
    }

    pub async fn commit_new_payload(_engine_handle: &BeaconConsensusEngineHandle<Payload>) {
        info!("Committing new payload to engine");
        // Todo: Get created payload then send to the engine_handle
        // let payload = Payload::default();
        // engine_handle.new_payload(payload).await;
    }
}
