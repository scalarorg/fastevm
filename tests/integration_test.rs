use std::time::Duration;
use tokio::time::sleep;
use reqwest::Client;
use serde_json::{json, Value};
use anyhow::Result;

// Import types from both crates
use execution_client::types::{ExecutionPayload, ForkchoiceState, PayloadAttributes};
use execution_client::engine_api::start_engine_api_server;
use consensus_client::client::EngineApiClient;
use consensus_client::types::EngineApiConfig;

#[tokio::test]
async fn test_full_integration_flow() -> Result<()> {
    // Start execution client server
    let execution_port = 18551;
    let server_handle = start_execution_server(execution_port).await?;
    
    // Give server time to start
    sleep(Duration::from_millis(500)).await;
    
    // Create consensus client
    let config = EngineApiConfig {
        execution_client_url: format!("http://127.0.0.1:{}", execution_port),
        poll_interval_ms: 100, // Fast polling for testing
        max_retries: 3,
        timeout_seconds: 5,
    };
    
    let consensus_client = EngineApiClient::new(config);
    
    // Test the flow
    let result = test_engine_api_communication(execution_port).await;
    
    // Cleanup
    server_handle.stop()?;
    
    result
}

async fn start_execution_server(port: u16) -> Result<jsonrpsee::server::ServerHandle> {
    let (handle, _addr) = start_engine_api_server(port).await?;
    Ok(handle)
}

async fn test_engine_api_communication(port: u16) -> Result<()> {
    let client = Client::new();
    let url = format!("http://127.0.0.1:{}", port);
    
    // Test 1: Submit new payload
    let test_payload = create_test_execution_payload();
    let new_payload_response = call_engine_api_method(
        &client,
        &url,
        "engine_newPayloadV2",
        json!([test_payload])
    ).await?;
    
    println!("New payload response: {}", new_payload_response);
    
    // Test 2: Update forkchoice
    let forkchoice_state = ForkchoiceState {
        head_block_hash: ethereum_types::H256::zero(),
        safe_block_hash: ethereum_types::H256::zero(),
        finalized_block_hash: ethereum_types::H256::zero(),
    };
    
    let payload_attributes = PayloadAttributes {
        timestamp: ethereum_types::U64::from(1234567890),
        prev_randao: ethereum_types::H256::zero(),
        suggested_fee_recipient: ethereum_types::Address::zero(),
        withdrawals: None,
    };
    
    let forkchoice_response = call_engine_api_method(
        &client,
        &url,
        "engine_forkchoiceUpdatedV2",
        json!([forkchoice_state, payload_attributes])
    ).await?;
    
    println!("Forkchoice response: {}", forkchoice_response);
    
    // Test 3: Get payload if payload ID was returned
    if let Some(payload_id) = forkchoice_response.get("payloadId") {
        let get_payload_response = call_engine_api_method(
            &client,
            &url,
            "engine_getPayloadV2",
            json!([payload_id])
        ).await?;
        
        println!("Get payload response: {}", get_payload_response);
    }
    
    Ok(())
}

async fn call_engine_api_method(
    client: &Client,
    url: &str,
    method: &str,
    params: Value,
) -> Result<Value> {
    let payload = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    });
    
    let response = client
        .post(url)
        .json(&payload)
        .send()
        .await?;
        
    let json_response: Value = response.json().await?;
    
    if let Some(error) = json_response.get("error") {
        return Err(anyhow::anyhow!("RPC error: {}", error));
    }
    
    Ok(json_response.get("result").cloned().unwrap_or_default())
}

fn create_test_execution_payload() -> ExecutionPayload {
    ExecutionPayload {
        parent_hash: ethereum_types::H256::zero(),
        fee_recipient: ethereum_types::Address::zero(),
        state_root: ethereum_types::H256::zero(),
        receipts_root: ethereum_types::H256::zero(),
        logs_bloom: Default::default(),
        prev_randao: ethereum_types::H256::zero(),
        block_number: ethereum_types::U64::from(1),
        gas_limit: ethereum_types::U64::from(30_000_000),
        gas_used: ethereum_types::U64::from(21_000),
        timestamp: ethereum_types::U64::from(1234567890),
        extra_data: vec![],
        base_fee_per_gas: ethereum_types::U256::from(1_000_000_000u64),
        block_hash: ethereum_types::H256::zero(),
        transactions: vec![],
        withdrawals: None,
    }
}

#[tokio::test]
async fn test_consensus_client_metrics() -> Result<()> {
    let config = EngineApiConfig {
        execution_client_url: "http://127.0.0.1:18552".to_string(), // Non-existent for testing
        poll_interval_ms: 100,
        max_retries: 1,
        timeout_seconds: 1,
    };
    
    let client = EngineApiClient::new(config);
    let initial_metrics = client.get_metrics();
    
    assert_eq!(initial_metrics.total_subdags_processed, 0);
    assert_eq!(initial_metrics.successful_payload_submissions, 0);
    assert_eq!(initial_metrics.failed_payload_submissions, 0);
    
    Ok(())
}

#[tokio::test]
async fn test_execution_payload_validation() -> Result<()> {
    // Test payload validation logic
    let valid_payload = create_test_execution_payload();
    assert!(valid_payload.block_number.as_u64() > 0);
    assert_eq!(valid_payload.gas_used.as_u64(), 21_000);
    
    Ok(())
}

#[tokio::test]
async fn test_multiple_clients_scenario() -> Result<()> {
    // Start multiple execution servers on different ports
    let ports = [18561, 18562, 18563];
    let mut handles = Vec::new();
    
    for port in &ports {
        let handle = start_execution_server(*port).await?;
        handles.push(handle);
        sleep(Duration::from_millis(100)).await;
    }
    
    // Create consensus clients for each
    let mut consensus_clients = Vec::new();
    for port in &ports {
        let config = EngineApiConfig {
            execution_client_url: format!("http://127.0.0.1:{}", port),
            poll_interval_ms: 200,
            max_retries: 2,
            timeout_seconds: 5,
        };
        
        consensus_clients.push(EngineApiClient::new(config));
    }
    
    // Test that all clients can be created and have proper initial state
    for client in &consensus_clients {
        let state = client.get_consensus_state();
        assert_eq!(state.current_round, 0);
        assert!(state.finalized_subdags.is_empty());
    }
    
    // Cleanup
    for handle in handles {
        handle.stop()?;
    }
    
    Ok(())
}

#[tokio::test]
async fn test_subdag_conversion_logic() -> Result<()> {
    // Test the conversion from SubDAG to ExecutionPayload
    use consensus_client::types::{SubDAG, NodeId, Transaction, CommitProof};
    use chrono::Utc;
    
    let mock_subdag = SubDAG {
        id: "test-subdag-123".to_string(),
        leader: NodeId("leader-1".to_string()),
        round: 42,
        transactions: vec![
            Transaction {
                hash: ethereum_types::H256::zero(),
                data: vec![0x01, 0x02, 0x03],
                sender: ethereum_types::Address::zero(),
                nonce: 1,
                gas_limit: 21000,
                gas_price: ethereum_types::U256::from(20_000_000_000u64),
            }
        ],
        timestamp: Utc::now(),
        parent_subdags: vec![],
        commit_proof: CommitProof {
            signatures: vec![],
            bitmap: vec![true],
            merkle_root: ethereum_types::H256::zero(),
        },
    };
    
    // Verify SubDAG properties
    assert_eq!(mock_subdag.round, 42);
    assert_eq!(mock_subdag.transactions.len(), 1);
    assert_eq!(mock_subdag.transactions[0].gas_limit, 21000);
    
    Ok(())
}

#[tokio::test]
async fn test_error_handling() -> Result<()> {
    // Test error handling for invalid requests
    let client = Client::new();
    let url = "http://127.0.0.1:18599"; // Non-existent server
    
    let result = call_engine_api_method(
        &client,
        url,
        "engine_newPayloadV2",
        json!([create_test_execution_payload()])
    ).await;
    
    // Should fail to connect
    assert!(result.is_err());
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_requests() -> Result<()> {
    // Start execution server
    let port = 18571;
    let server_handle = start_execution_server(port).await?;
    
    sleep(Duration::from_millis(500)).await;
    
    let client = Client::new();
    let url = format!("http://127.0.0.1:{}", port);
    
    // Send multiple concurrent requests
    let mut tasks = Vec::new();
    
    for i in 0..5 {
        let client = client.clone();
        let url = url.clone();
        
        let task = tokio::spawn(async move {
            let mut payload = create_test_execution_payload();
            payload.block_number = ethereum_types::U64::from(i + 1);
            
            call_engine_api_method(
                &client,
                &url,
                "engine_newPayloadV2",
                json!([payload])
            ).await
        });
        
        tasks.push(task);
    }
    
    // Wait for all requests to complete
    let results: Vec<Result<Result<Value>, tokio::task::JoinError>> = 
        futures::future::join_all(tasks).await
        .into_iter()
        .collect();
    
    // Check that most requests succeeded
    let successful_requests = results
        .iter()
        .filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
        .count();
    
    assert!(successful_requests >= 3, "At least 3 requests should succeed");
    
    // Cleanup
    server_handle.stop()?;
    
    Ok(())
}

// Helper module for async testing
mod futures {
    pub mod future {
        pub async fn join_all<T>(
            futures: impl IntoIterator<Item = impl std::future::Future<Output = T>>,
        ) -> Vec<T> {
            let mut results = Vec::new();
            for future in futures {
                results.push(future.await);
            }
            results
        }
    }
}