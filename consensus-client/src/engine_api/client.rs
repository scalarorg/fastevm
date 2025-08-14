use std::sync::Arc;
use std::str::FromStr;
use crate::engine_api::{ConsensusState, EngineApiConfig, SubDagBlock};
use alloy_primitives::{Address, B256};
use alloy_rpc_types_engine::{
    ExecutionPayloadEnvelopeV2, ExecutionPayloadFieldV2, ExecutionPayloadInputV2,
    ExecutionPayloadV1, ForkchoiceState, PayloadAttributes, PayloadId,
};
use anyhow::Result;
use consensus_core::{CertifiedBlocksOutput, CommittedSubDag};
use jsonrpsee::core::client::SubscriptionClientT;
use mysten_metrics::monitored_mpsc::UnboundedReceiver;
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_rpc_api::clients::EngineApiClient;
use reth_rpc_layer::{AuthClientLayer, JwtSecret};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{self, Duration};
use tracing::{error, info};

pub struct ExecutionClient {
    config: EngineApiConfig,
    consensus_state: Arc<RwLock<ConsensusState>>,
    //metrics: Arc<Mutex<ConsensusMetrics>>,
    payload_tx: mpsc::UnboundedSender<ExecutionPayloadFieldV2>,
}

impl ExecutionClient {
    pub fn new(
        node_index: u32,
        config: EngineApiConfig,
        payload_tx: mpsc::UnboundedSender<ExecutionPayloadFieldV2>,
    ) -> Result<Self> {
        let fee_recipient = Address::from_str(&config.fee_recipient)?;
        let consensus_state = ConsensusState::new(node_index, fee_recipient);
        Ok(Self {
            config,
            consensus_state: Arc::new(RwLock::new(consensus_state)),
            //metrics: Arc::new(Mutex::new(ConsensusMetrics::default())),
            payload_tx,
        })
    }
    pub fn jwt_secret(&self) -> JwtSecret {
        match JwtSecret::from_hex(&self.config.jwt_secret) {
            Ok(jwt_secret) => jwt_secret,
            Err(err) => panic!("{:?}", err),
        }
    }
    pub fn http_url(&self) -> String {
        self.config.execution_url.clone()
    }
    pub async fn get_forcechoice_state(&self) -> ForkchoiceState {
        let state = self.consensus_state.read().await;
        let block_hash = state.finalized_block_hash();
        ForkchoiceState {
            head_block_hash: block_hash.clone(),
            safe_block_hash: block_hash.clone(),
            finalized_block_hash: block_hash.clone(),
        }
    }
    fn get_payload_attributes(&self) -> Option<PayloadAttributes> {
        let attributes = PayloadAttributes {
            timestamp: 0,
            prev_randao: B256::default(),
            suggested_fee_recipient: Address::default(),
            withdrawals: None,
            parent_beacon_block_root: None,
        };
        Some(attributes)
    }
    /// Returns a http client connected to the server.
    ///
    /// This client uses the JWT token to authenticate requests.
    pub fn http_client(&self) -> impl SubscriptionClientT + Clone + Send + Sync + Unpin + 'static {
        // Create a middleware that adds a new JWT token to every request.
        let secret_layer = AuthClientLayer::new(self.jwt_secret());
        let middleware = tower::ServiceBuilder::default().layer(secret_layer);
        jsonrpsee::http_client::HttpClientBuilder::default()
            .set_http_middleware(middleware)
            .build(self.http_url())
            .expect("Failed to create http client")
    }

    // pub fn get_metrics(&self) -> ConsensusMetrics {
    //     self.metrics.lock().unwrap().clone()
    // }
    async fn send_payload_for_consensus(&self, payload: ExecutionPayloadEnvelopeV2) -> Result<()> {
        let ExecutionPayloadEnvelopeV2 {
            execution_payload,
            block_value: _block_value,
        } = payload;
        if let Err(err) = self.payload_tx.send(execution_payload) {
            error!("Error when broadcast execution payload {:?}", err)
        }
        Ok(())
    }
    pub async fn start(
        &mut self,
        mut commit_receiver: UnboundedReceiver<CommittedSubDag>,
        _block_receiver: UnboundedReceiver<CertifiedBlocksOutput>,
    ) -> Result<()> {
        // Placeholder main loop for the consensus client.
        // In a real implementation this would poll the execution client
        // and push Engine API calls.
        // exchange capabilities
        let http_client = self.http_client();
        let capabilities =
            EngineApiClient::<EthEngineTypes>::exchange_capabilities(&http_client, vec![]).await?;
        info!("exchange capabilities: {:?}", capabilities);
        let mut interval = time::interval(Duration::from_secs(self.config.poll_interval_ms));
        let mut payload_id: Option<PayloadId> = None;

        loop {
            tokio::select! {
               _ = interval.tick() => {
                    // Call forkChoiceUpdated
                    let fc_state = self.get_forcechoice_state().await;
                    let payload_attributes = self.get_payload_attributes();
                    match EngineApiClient::<EthEngineTypes>::fork_choice_updated_v2(&http_client, fc_state.clone(), payload_attributes).await {
                        Ok(resp) => {
                            info!("forkChoiceUpdated response: {:?}", resp);
                            payload_id = resp.payload_id;
                        },
                        Err(e) => error!("forkChoiceUpdated failed: {:?}", e),
                    }
                    if let Some(payload_id) = payload_id.as_ref() {
                       match EngineApiClient::<EthEngineTypes>::get_payload_v2(&http_client, payload_id.clone()).await {
                            Ok(payload) => {
                                let _res = self.send_payload_for_consensus(payload).await;
                            },
                            Err(err) =>  {
                                error!("{:?}", err)
                            }
                        }
                    }
               }
               // ---- Incoming message from Mysticeti consensus ----
                maybe_msg = commit_receiver.recv() => {
                    match maybe_msg {
                        Some(subdag) => {
                            let payload = self.process_subdag(subdag).await;
                            match EngineApiClient::<EthEngineTypes>::new_payload_v2(&http_client, payload).await {
                                Ok(resp) => info!("newPayload response: {:?}", resp),
                                Err(e) => error!("newPayload failed: {:?}", e),
                            }
                        }
                        None => {
                            info!("Consensus channel closed, stopping Engine API loop");
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }
    async fn process_subdag(&self, committed_subdag: CommittedSubDag) -> ExecutionPayloadInputV2 {
        let mut consensus_state = self.consensus_state.write().await;
        let payload = consensus_state.process_subdag(committed_subdag);
        payload
    }
}
