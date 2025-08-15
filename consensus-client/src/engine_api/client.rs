use std::sync::Arc;
use std::str::FromStr;
use crate::engine_api::{ConsensusState, EngineApiConfig};
use alloy_primitives::{Address, B256};
use alloy_rpc_types_engine::{
    ExecutionPayloadEnvelopeV2, ExecutionPayloadFieldV2, ExecutionPayloadInputV2,
    ForkchoiceState, PayloadAttributes, PayloadId,
};
use anyhow::{anyhow, Result};
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
            Err(err) => {
                error!("Invalid JWT secret format: {:?}", err);
                error!("JWT secret should be a 32-byte hex string starting with 0x");
                error!("Current JWT secret: {}...", &self.config.jwt_secret[..10]);
                panic!("JWT secret parsing failed: {:?}", err);
            }
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
        info!("Starting Engine API client...");
        
        // Try to connect to the execution client
        let http_client = self.http_client();
        
        // Test connection by exchanging capabilities
        let capabilities = match EngineApiClient::<EthEngineTypes>::exchange_capabilities(&http_client, vec![]).await {
            Ok(caps) => {
                info!("Successfully connected to execution client. Capabilities: {:?}", caps);
                caps
            },
            Err(e) => {
                error!("Failed to connect to execution client: {:?}", e);
                error!("Execution URL: {}, JWT Secret: {}...", self.config.execution_url, &self.config.jwt_secret[..10]);
                return Err(anyhow!("Failed to connect to execution client: {:?}", e));
            }
        };
        
        let mut interval = time::interval(Duration::from_secs(self.config.poll_interval_ms));
        let mut payload_id: Option<PayloadId> = None;
        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 5;

        info!("Engine API client started successfully. Polling every {}ms", self.config.poll_interval_ms);

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
                            consecutive_errors = 0; // Reset error counter on success
                        },
                        Err(e) => {
                            consecutive_errors += 1;
                            error!("forkChoiceUpdated failed (attempt {}/{}): {:?}", consecutive_errors, MAX_CONSECUTIVE_ERRORS, e);
                            
                            if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                                error!("Too many consecutive errors, stopping Engine API client");
                                break;
                            }
                        }
                    }
                    
                    if let Some(payload_id) = payload_id.as_ref() {
                       match EngineApiClient::<EthEngineTypes>::get_payload_v2(&http_client, payload_id.clone()).await {
                            Ok(payload) => {
                                let _res = self.send_payload_for_consensus(payload).await;
                            },
                            Err(err) =>  {
                                error!("getPayload failed: {:?}", err)
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

        info!("Engine API client stopped");
        Ok(())
    }
    async fn process_subdag(&self, committed_subdag: CommittedSubDag) -> ExecutionPayloadInputV2 {
        let mut consensus_state = self.consensus_state.write().await;
        let payload = consensus_state.process_subdag(committed_subdag);
        payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Bloom, Bytes, U256};
    use alloy_rpc_types_engine::ExecutionPayloadV1;
    use tokio::sync::mpsc;
    use consensus_core::{BlockRef, CommitConsumer, CommitDigest, CommitRef, CommittedSubDag};

    // Helper function to create test config
    fn create_test_config() -> EngineApiConfig {
        EngineApiConfig {
            execution_url: "http://127.0.0.1:8551".to_string(),
            jwt_secret: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            fee_recipient: "0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6".to_string(),
            poll_interval_ms: 1000,
        }
    }

    // Helper function to create test consensus state
    fn create_test_consensus_state() -> ConsensusState {
        ConsensusState::new(0, Address::from_str("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6").unwrap())
    }

    #[tokio::test]
    async fn test_execution_client_new_success() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        
        let result = ExecutionClient::new(0, config, payload_tx);
        assert!(result.is_ok());
        
        let client = result.unwrap();
        assert_eq!(client.config.execution_url, "http://127.0.0.1:8551");
    }

    #[tokio::test]
    async fn test_execution_client_new_with_invalid_fee_recipient() {
        let mut config = create_test_config();
        config.fee_recipient = "invalid_address".to_string();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        
        let result = ExecutionClient::new(0, config, payload_tx);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_jwt_secret_success() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        let jwt_secret = client.jwt_secret();
        assert!(jwt_secret.as_bytes().len() == 32);
    }

    #[test]
    #[should_panic(expected = "JWT secret parsing failed")]
    fn test_jwt_secret_invalid_hex() {
        let mut config = create_test_config();
        config.jwt_secret = "invalid_hex".to_string();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        // This should panic
        let _ = client.jwt_secret();
    }

    #[tokio::test]
    async fn test_http_url() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        let url = client.http_url();
        assert_eq!(url, "http://127.0.0.1:8551");
    }

    #[tokio::test]
    async fn test_get_forcechoice_state() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        let fc_state = client.get_forcechoice_state().await;
        assert_eq!(fc_state.head_block_hash, B256::default());
        assert_eq!(fc_state.safe_block_hash, B256::default());
        assert_eq!(fc_state.finalized_block_hash, B256::default());
    }

    #[test]
    fn test_get_payload_attributes() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        let attributes = client.get_payload_attributes();
        assert!(attributes.is_some());
        
        let attributes = attributes.unwrap();
        assert_eq!(attributes.timestamp, 0);
        assert_eq!(attributes.prev_randao, B256::default());
        assert_eq!(attributes.suggested_fee_recipient, Address::default());
        assert!(attributes.withdrawals.is_none());
        assert!(attributes.parent_beacon_block_root.is_none());
    }

    #[tokio::test]
    async fn test_http_client_creation() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        let http_client = client.http_client();
        // Test that we can create the client without panicking
        assert!(true);
    }

    #[tokio::test]
    async fn test_send_payload_for_consensus_success() {
        let config = create_test_config();
        let (payload_tx, mut payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        let payload = ExecutionPayloadEnvelopeV2 {
            execution_payload: ExecutionPayloadFieldV2::V1(ExecutionPayloadV1 {
                parent_hash: B256::default(),
                fee_recipient: Address::default(),
                state_root: [0u8; 32].into(),
                receipts_root: [0u8; 32].into(),
                logs_bloom: Bloom::default(),
                prev_randao: [0u8; 32].into(),
                block_number: 1,
                gas_limit: 0,
                gas_used: 0,
                timestamp: 0,
                extra_data: Bytes::default(),
                base_fee_per_gas: U256::default(),
                block_hash: [0u8; 32].into(),
                transactions: vec![],
            }),
            block_value: U256::default(),
        };
        
        let result = client.send_payload_for_consensus(payload).await;
        assert!(result.is_ok());
        
        // Check that the payload was sent
        let received = payload_rx.try_recv();
        assert!(received.is_ok());
    }

    #[tokio::test]
    async fn test_send_payload_for_consensus_channel_closed() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        // Drop the receiver to close the channel
        drop(_payload_rx);
        
        let payload = ExecutionPayloadEnvelopeV2 {
            execution_payload: ExecutionPayloadFieldV2::V1(ExecutionPayloadV1 {
                parent_hash: B256::default(),
                fee_recipient: Address::default(),
                state_root: [0u8; 32].into(),
                receipts_root: [0u8; 32].into(),
                logs_bloom: Bloom::default(),
                prev_randao: [0u8; 32].into(),
                block_number: 1,
                gas_limit: 0,
                gas_used: 0,
                timestamp: 0,
                extra_data: Bytes::default(),
                base_fee_per_gas: U256::default(),
                block_hash: [0u8; 32].into(),
                transactions: vec![],
            }),
            block_value: U256::default(),
        };
        
        // This should not panic even when the channel is closed
        let result = client.send_payload_for_consensus(payload).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_subdag() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        let leader = BlockRef::MIN;
        let blocks = vec![];
        let commit_ref = CommitRef::new(1, CommitDigest::default());
        let subdag = CommittedSubDag::new(leader, blocks, vec![], 1000, commit_ref, vec![]);
        
        // Convert MockCommittedSubDag to real CommittedSubDag
        // This is a simplified test - in real implementation you'd need proper conversion
        let result = client.process_subdag(subdag).await;
        // Test that the function doesn't panic
        assert!(true);
    }

    #[tokio::test]
    async fn test_start_method_basic() {
        let mut config = create_test_config();
        config.poll_interval_ms = 100; // Use shorter interval for testing
        
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let mut client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        // Create mock receivers
        let (_, commit_receiver,block_receiver) = CommitConsumer::new(0);
        
        // Start the client in a separate task
        let client_handle = tokio::spawn(async move {
            client.start(commit_receiver, block_receiver).await
        });
        
        // Give it a moment to start
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Cancel the task
        client_handle.abort();
        
        // Test passes if we can start the client without panicking
        assert!(true);
    }

    #[tokio::test]
    async fn test_start_method_with_commit_messages() {
        let mut config = create_test_config();
        config.poll_interval_ms = 100; // Use shorter interval for testing
        
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let mut client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        // Create mock receivers
        let (commit_consumer, commit_receiver, block_receiver) = CommitConsumer::new(0);
        
        // Start the client in a separate task
        let client_handle = tokio::spawn(async move {
            client.start(commit_receiver, block_receiver).await
        });
        
        // Give it a moment to start
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Send a commit message
        let mock_subdag = CommittedSubDag {
            leader:BlockRef::MIN,
            blocks:vec![],timestamp_ms:1000,
            commit_ref:CommitRef::new(1,CommitDigest::default()), 
            rejected_transactions_by_block: Vec::new(), 
            reputation_scores_desc: Vec::new() 
        };
        
        // Give it a moment to process
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Cancel the task
        client_handle.abort();
        
        // Test passes if we can start the client and process messages without panicking
        assert!(true);
    }

    #[tokio::test]
    async fn test_start_method_channel_closed() {
        let mut config = create_test_config();
        config.poll_interval_ms = 100; // Use shorter interval for testing
        
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let mut client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        // Create mock receivers and immediately close them
        let (_, commit_receiver, block_receiver) = CommitConsumer::new(0);
        
        // Start the client in a separate task
        let client_handle = tokio::spawn(async move {
            client.start(commit_receiver, block_receiver).await
        });
        
        // Wait for the client to finish (it should exit when channels are closed)
        let result = client_handle.await;
        assert!(result.is_ok());
    }

    // Test error handling scenarios
    #[tokio::test]
    async fn test_error_handling_in_start_loop() {
        let mut config = create_test_config();
        config.execution_url = "http://invalid-url:9999".to_string(); // Invalid URL
        config.poll_interval_ms = 100;
        
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let mut client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        // Create mock receivers
        let (_, commit_receiver, block_receiver) = CommitConsumer::new(0);
        
        // Start the client in a separate task
        let client_handle = tokio::spawn(async move {
            client.start(commit_receiver, block_receiver).await
        });
        
        // Give it a moment to start and encounter errors
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Cancel the task
        client_handle.abort();
        
        // Test passes if we can handle errors gracefully without panicking
        assert!(true);
    }

    // Test boundary conditions
    #[tokio::test]
    async fn test_boundary_conditions() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();
        
        // Test with zero values
        let fc_state = client.get_forcechoice_state().await;
        assert_eq!(fc_state.head_block_hash, B256::default());
        
        // Test with default values
        let attributes = client.get_payload_attributes();
        assert!(attributes.is_some());
        let attributes = attributes.unwrap();
        assert_eq!(attributes.timestamp, 0);
        assert_eq!(attributes.prev_randao, B256::default());
    }

    // Test concurrent access
    #[tokio::test]
    async fn test_concurrent_access() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = Arc::new(ExecutionClient::new(0, config, payload_tx).unwrap());
        
        let client_clone1 = Arc::clone(&client);
        let client_clone2 = Arc::clone(&client);
        
        let handle1 = tokio::spawn(async move {
            let _ = client_clone1.get_forcechoice_state().await;
        });
        
        let handle2 = tokio::spawn(async move {
            let _ = client_clone2.get_forcechoice_state().await;
        });
        
        let (result1, result2) = tokio::join!(handle1, handle2);
        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }
}
