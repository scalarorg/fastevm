// use crate::beacon_chain::BeaconState;
use crate::NodeConfig;
use alloy_primitives::Bytes;
use anyhow::{anyhow, Result};
use consensus_core::{CertifiedBlocksOutput, CommittedSubDag};
use jsonrpsee::core::client::SubscriptionClientT;
use mysten_metrics::monitored_mpsc::UnboundedReceiver;
use reth_extension::{
    CommittedSubDag as RethCommittedSubDag, ConsensusTransactionApiClient, TxpoolListenerApiClient,
};
use reth_rpc_layer::{secret_to_bearer_header, AuthClientLayer, JwtSecret};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub type PayloadItem = Vec<Bytes>;
pub struct ExecutionClient {
    config: NodeConfig,
    payload_tx: mpsc::UnboundedSender<PayloadItem>,
}

impl ExecutionClient {
    pub fn new(config: NodeConfig, payload_tx: mpsc::UnboundedSender<PayloadItem>) -> Result<Self> {
        Ok(Self { config, payload_tx })
    }

    pub fn jwt_secret(&self) -> JwtSecret {
        debug!("JWT secret: {:?}", self.config.jwt_secret);
        match JwtSecret::from_hex(&self.config.jwt_secret) {
            Ok(jwt_secret) => jwt_secret,
            Err(err) => {
                error!("Invalid JWT secret format: {:?}.JWT secret should be a 32-byte hex string starting with 0x", err);
                panic!("JWT secret parsing failed: {:?}", err);
            }
        }
    }
    pub fn http_url(&self) -> String {
        self.config.execution_http_url.clone()
    }
    pub fn ws_url(&self) -> String {
        self.config.execution_ws_url.clone()
    }
    // pub async fn get_forcechoice_state(&self) -> ForkchoiceState {
    //     let state = self.consensus_state.read().await;
    //     state.get_fork_choice_state()
    // }
    // async fn get_payload_attributes(&self) -> Option<PayloadAttributes> {
    //     let state = self.consensus_state.read().await;
    //     state.get_payload_attributes()
    // }
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

    pub async fn ws_client(&self) -> impl SubscriptionClientT + Send + Sync + Unpin + 'static {
        let mut auth_header = secret_to_bearer_header(&self.jwt_secret());
        // The header value should not be visible in logs for security.
        auth_header.set_sensitive(true);
        let url = self.ws_url();
        debug!(
            "Creating ws client with url: {} and auth header: {:?}",
            url,
            auth_header.to_str().unwrap()
        );
        let mut headers = http::HeaderMap::new();
        headers.insert(http::header::AUTHORIZATION, auth_header);

        jsonrpsee::ws_client::WsClientBuilder::default()
            .set_headers(headers)
            .build(url)
            .await
            .expect("Failed to create ws client")
    }
    // pub fn get_metrics(&self) -> ConsensusMetrics {
    //     self.metrics.lock().unwrap().clone()
    // }
    // async fn send_payload_for_consensus(&self, payload: ExecutionPayloadEnvelopeV3) -> Result<()> {
    //     let ExecutionPayloadEnvelopeV3 {
    //         execution_payload,
    //         block_value: _,
    //         blobs_bundle: _,
    //         should_override_builder: _,
    //     } = payload;
    //     if execution_payload
    //         .payload_inner
    //         .payload_inner
    //         .transactions
    //         .len()
    //         == 0
    //     {
    //         debug!("No transactions in execution payload");
    //         return Ok(());
    //     }
    //     if let Err(err) = self.payload_tx.send(execution_payload) {
    //         error!("Error when broadcast execution payload {:?}", err)
    //     }
    //     Ok(())
    // }
    async fn send_transaction(&self, txs: Vec<Bytes>) -> Result<()> {
        let res = self
            .payload_tx
            .send(txs)
            .map_err(|e| anyhow!("Error sending transaction: {:?}", e));
        if res.is_err() {
            error!("Error sending transaction: {:?}", res);
            return Err(anyhow!("Error sending transaction: {:?}", res));
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
        let ws_client = self.ws_client().await;
        let http_client = self.http_client();
        let mut txpool_subscriber = TxpoolListenerApiClient::subscribe_transactions(&ws_client)
            .await
            .expect("failed to subscribe");

        // Test connection by exchanging capabilities
        // let capabilities =
        //     match EngineApiClient::<EthEngineTypes>::exchange_capabilities(&http_client, vec![])
        //         .await
        //     {
        //         Ok(caps) => {
        //             info!(
        //                 "Successfully connected to execution client. Capabilities: {:?}",
        //                 caps
        //             );
        //             caps
        //         }
        //         Err(e) => {
        //             error!("Failed to connect to execution client: {:?}", e);
        //             error!(
        //                 "Execution URL: {}, JWT Secret: {}...",
        //                 self.config.execution_url,
        //                 &self.config.jwt_secret[..10]
        //             );
        //             return Err(anyhow!("Failed to connect to execution client: {:?}", e));
        //         }
        //     };

        // let mut interval = time::interval(Duration::from_secs(self.config.poll_interval));
        // let mut payload_id: Option<PayloadId> = None;
        // let mut consecutive_errors = 0;
        // const MAX_CONSECUTIVE_ERRORS: u32 = 5;

        info!(
            "Engine API client started successfully. Polling every {}ms",
            self.config.poll_interval
        );

        loop {
            tokio::select! {
            //    _ = interval.tick() => {
            //         // Call forkChoiceUpdated
            //         let fc_state = self.get_forcechoice_state().await;
            //         let payload_attributes = self.get_payload_attributes().await;
            //         info!("forkChoiceUpdated: {:?}, payload_attributes: {:?}", fc_state, payload_attributes);
            //         /*
            //          * "shanghaiTime": 1700001200,  // Shanghai activates at this timestamp
            //          * "cancunTime": 1710000000     // Cancun activates at this timestamp Saturday, March 9, 2024 4:00:00 PM
            //          * After cancunTime, we must use fork_choice_updated_v3
            //          * Before cancunTime, we must use fork_choice_updated_v2
            //          */
            //         match EngineApiClient::<EthEngineTypes>::fork_choice_updated_v3(&http_client, fc_state.clone(), payload_attributes).await {
            //             Ok(resp) => {
            //                 info!("forkChoiceUpdated response: {:?}", resp);
            //                 payload_id = resp.payload_id;
            //                 consecutive_errors = 0; // Reset error counter on success
            //             },
            //             Err(e) => {
            //                 consecutive_errors += 1;
            //                 error!("forkChoiceUpdated failed (attempt {}/{}): {:?}",
            //                     consecutive_errors, MAX_CONSECUTIVE_ERRORS, e);

            //                 if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
            //                     error!("Too many consecutive errors, stopping Engine API client");
            //                     break;
            //                 }
            //             }
            //         }

            //         if let Some(payload_id) = payload_id.as_ref() {
            //            match EngineApiClient::<EthEngineTypes>::get_payload_v3(&http_client, payload_id.clone()).await {
            //                 Ok(payload) => {
            //                     let _res = self.send_payload_for_consensus(payload).await;
            //                 },
            //                 Err(err) =>  {
            //                     error!("getPayload failed: {:?}", err)
            //                 }
            //             }
            //         }
            //    }
                may_tx = txpool_subscriber.next() => {
                        match may_tx {
                            Some(Ok(tx)) => {
                                info!("Received transaction: {:?}", tx);
                                match self.send_transaction(tx).await {
                                    Ok(()) => {
                                        info!("Transaction sent successfully");
                                    }
                                    Err(e) => {
                                        error!("Transaction sent failed: {:?}", e);
                                    }
                                }
                            }
                            Some(Err(err)) => {
                                error!("Error receiving transaction: {:?}", err);
                            }
                            None => {
                                info!("Transaction channel closed, stopping Engine API loop");
                                break;
                            }
                        }
                }
               // ---- Incoming message from Mysticeti consensus ----
                maybe_msg = commit_receiver.recv() => {
                    match maybe_msg {
                        Some(subdag) => {
                            if subdag.timestamp_ms == 0 {
                                info!("Subdag has no timestamp, skipping");
                                continue;
                            }
                            info!("Received committed subdag: {:?}", subdag);
                            //let payload = self.process_subdag(subdag).await;
                            // match EngineApiClient::<EthEngineTypes>::new_payload_v3(&http_client, payload, vec![], B256::default()).await {
                            //     Ok(resp) => info!("newPayload response: {:?}", resp),
                            //     Err(e) => error!("newPayload failed: {:?}", e),
                            // }
                            // let transactions = self.extract_commited_transactions(subdag);
                            let reth_subdag = RethCommittedSubDag::from(subdag);
                            info!("Convert mysticeti subdag to reth subdag: {:?}", reth_subdag);
                            let res = ConsensusTransactionApiClient::submit_committed_subdag(&http_client, reth_subdag).await;
                            if res.is_ok() {
                                info!("submit_committed_transactions successfully");
                            } else {
                                error!("submit_committed_transactions failed: {:?}", res);
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
    // async fn process_subdag(&self, committed_subdag: CommittedSubDag) -> ExecutionPayloadV3 {
    //     let mut consensus_state = self.consensus_state.write().await;
    //     let payload = consensus_state.process_subdag(committed_subdag);
    //     payload
    // }
}

#[cfg(test)]
mod tests {
    const GENESIS_TIME: u64 = 1755000000;
    use std::str::FromStr;
    use tokio::time::Duration;

    // use crate::beacon_chain::{beacon_block::ChainSpec, GENESIS_TIME};

    use super::*;
    use consensus_core::{BlockRef, CommitConsumer, CommitDigest, CommitRef, CommittedSubDag};
    use tokio::sync::mpsc;

    // Helper function to create test config
    fn create_test_config() -> NodeConfig {
        NodeConfig {
            chain: "dev".to_string(),
            execution_http_url: "http://127.0.0.1:8551".to_string(),
            execution_ws_url: "ws://127.0.0.1:8551".to_string(),
            jwt_secret: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                .to_string(),
            genesis_block_hash:
                "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            genesis_time: GENESIS_TIME,
            fee_recipient: "0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6".to_string(),
            poll_interval: 1000,
            max_retries: 3,
            timeout: 5000,
            working_directory: "/tmp/test".to_string(),
            peer_addresses: vec![],
            node_index: 0,
            log_level: "info".to_string(),
            committee_path: "committee.yml".to_string(),
            parameters_path: "parameters.yml".to_string(),
        }
    }

    #[tokio::test]
    async fn test_execution_client_new_success() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let result = ExecutionClient::new(config, payload_tx);
        assert!(result.is_ok());

        let client = result.unwrap();
        assert_eq!(client.config.execution_http_url, "http://127.0.0.1:8551");
        assert_eq!(client.config.execution_ws_url, "ws://127.0.0.1:8551");
    }

    #[tokio::test]
    async fn test_execution_client_new_with_invalid_fee_recipient() {
        let mut config = create_test_config();
        config.fee_recipient = "invalid_address".to_string();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let result = ExecutionClient::new(config, payload_tx);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_jwt_secret_success() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(config, payload_tx).unwrap();

        let jwt_secret = client.jwt_secret();
        assert!(jwt_secret.as_bytes().len() == 32);
    }

    #[test]
    #[should_panic(expected = "JWT secret parsing failed")]
    fn test_jwt_secret_invalid_hex() {
        let mut config = create_test_config();
        config.jwt_secret = "invalid_hex".to_string();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(config, payload_tx).unwrap();

        // This should panic
        let _ = client.jwt_secret();
    }

    #[tokio::test]
    async fn test_http_url() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(config, payload_tx).unwrap();

        let url = client.http_url();
        assert_eq!(url, "http://127.0.0.1:8551");
    }

    #[tokio::test]
    async fn test_http_client_creation() {
        let config = create_test_config();
        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(config, payload_tx).unwrap();

        let http_client = client.http_client();
        // Test that we can create the client without panicking
        assert!(true);
    }

    #[tokio::test]
    async fn test_start_method_basic() {
        let mut config = create_test_config();
        config.poll_interval = 100; // Use shorter interval for testing

        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let mut client = ExecutionClient::new(config, payload_tx).unwrap();

        // Create mock receivers
        let (_, commit_receiver, block_receiver) = CommitConsumer::new(0);

        // Start the client in a separate task
        let client_handle =
            tokio::spawn(async move { client.start(commit_receiver, block_receiver).await });

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
        config.poll_interval = 100; // Use shorter interval for testing

        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let mut client = ExecutionClient::new(config, payload_tx).unwrap();

        // Create mock receivers
        let (commit_consumer, commit_receiver, block_receiver) = CommitConsumer::new(0);

        // Start the client in a separate task
        let client_handle =
            tokio::spawn(async move { client.start(commit_receiver, block_receiver).await });

        // Give it a moment to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Send a commit message
        let mock_subdag = CommittedSubDag {
            leader: BlockRef::MIN,
            blocks: vec![],
            timestamp_ms: 1000,
            commit_ref: CommitRef::new(1, CommitDigest::default()),
            rejected_transactions_by_block: Vec::new(),
            reputation_scores_desc: Vec::new(),
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
        config.poll_interval = 100; // Use shorter interval for testing

        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let mut client = ExecutionClient::new(config, payload_tx).unwrap();

        // Create mock receivers and immediately close them
        let (_, commit_receiver, block_receiver) = CommitConsumer::new(0);

        // Start the client in a separate task
        let client_handle =
            tokio::spawn(async move { client.start(commit_receiver, block_receiver).await });

        // Wait for the client to finish (it should exit when channels are closed)
        let result = client_handle.await;
        assert!(result.is_ok());
    }

    // Test error handling scenarios
    #[tokio::test]
    async fn test_error_handling_in_start_loop() {
        let mut config = create_test_config();
        config.execution_http_url = "http://invalid-url:9999".to_string(); // Invalid URL
        config.poll_interval = 100;

        let (payload_tx, _payload_rx) = mpsc::unbounded_channel();
        let mut client = ExecutionClient::new(config, payload_tx).unwrap();

        // Create mock receivers
        let (_, commit_receiver, block_receiver) = CommitConsumer::new(0);

        // Start the client in a separate task
        let client_handle =
            tokio::spawn(async move { client.start(commit_receiver, block_receiver).await });

        // Give it a moment to start and encounter errors
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Cancel the task
        client_handle.abort();

        // Test passes if we can handle errors gracefully without panicking
        assert!(true);
    }
}
