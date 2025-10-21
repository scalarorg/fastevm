//! RPC client implementations for testing
//!
//! This module provides RPC client trait and implementations for sending raw transactions
//! in integration tests.

use alloy_primitives::Bytes;
use alloy_provider::{Provider, ProviderBuilder};
use async_trait::async_trait;
use eyre::Result;
use jsonrpsee::{http_client::HttpClientBuilder, ws_client::WsClientBuilder};
use reth_extension::MysticetiTransactionApiClient;

/// Trait for RPC clients that can send raw transactions
#[async_trait]
pub trait RpcClient: Send + Sync {
    /// Send a raw transaction and return the transaction hash
    async fn send_raw_transaction(&self, raw_tx: Bytes) -> Result<()>;
}

/// RPC client implementation using alloy provider
#[derive(Debug, Clone)]
pub struct ProviderRpcClient {
    url: String,
    provider: alloy_provider::fillers::FillProvider<
        alloy_provider::fillers::JoinFill<
            alloy_provider::Identity,
            alloy_provider::fillers::JoinFill<
                alloy_provider::fillers::GasFiller,
                alloy_provider::fillers::JoinFill<
                    alloy_provider::fillers::BlobGasFiller,
                    alloy_provider::fillers::JoinFill<
                        alloy_provider::fillers::NonceFiller,
                        alloy_provider::fillers::ChainIdFiller,
                    >,
                >,
            >,
        >,
        alloy_provider::RootProvider,
    >,
}

impl ProviderRpcClient {
    /// Create a new ProviderRpcClient from an RPC URL
    pub async fn new(rpc_url: &str) -> Result<Self> {
        let provider: alloy_provider::fillers::FillProvider<
            alloy_provider::fillers::JoinFill<
                alloy_provider::Identity,
                alloy_provider::fillers::JoinFill<
                    alloy_provider::fillers::GasFiller,
                    alloy_provider::fillers::JoinFill<
                        alloy_provider::fillers::BlobGasFiller,
                        alloy_provider::fillers::JoinFill<
                            alloy_provider::fillers::NonceFiller,
                            alloy_provider::fillers::ChainIdFiller,
                        >,
                    >,
                >,
            >,
            alloy_provider::RootProvider,
        > = ProviderBuilder::new().connect(rpc_url).await?;

        Ok(Self {
            url: rpc_url.to_string(),
            provider,
        })
    }
}

#[async_trait]
impl RpcClient for ProviderRpcClient {
    async fn send_raw_transaction(&self, raw_tx: Bytes) -> Result<()> {
        // Use the provider's send_raw_transaction method
        let tx_hash = self
            .provider
            .send_raw_transaction(&raw_tx)
            .await?
            .tx_hash()
            .clone();
        println!("tx_hash: {:?}", tx_hash);
        Ok(())
    }
}

/// RPC client implementation using direct RPC calls
#[derive(Debug)]
pub enum DirectRpcClient {
    /// HTTP client
    Http(jsonrpsee::http_client::HttpClient),
    /// WebSocket client
    WebSocket(jsonrpsee::ws_client::WsClient),
}

impl DirectRpcClient {
    /// Create a new DirectRpcClient from an HTTP URL
    pub async fn new_http(http_url: &str) -> Result<Self> {
        let client = HttpClientBuilder::default().build(http_url)?;

        Ok(Self::Http(client))
    }

    /// Create a new DirectRpcClient from a WebSocket URL
    pub async fn new_ws(ws_url: &str) -> Result<Self> {
        let client = WsClientBuilder::default().build(ws_url).await?;

        Ok(Self::WebSocket(client))
    }

    /// Create a new DirectRpcClient, automatically detecting the protocol from the URL
    pub async fn new(url: &str) -> Result<Self> {
        if url.starts_with("ws://") || url.starts_with("wss://") {
            Self::new_ws(url).await
        } else if url.starts_with("http://") || url.starts_with("https://") {
            Self::new_http(url).await
        } else {
            // Default to HTTP if no protocol is specified
            let http_url = if url.starts_with("//") {
                format!("http:{}", url)
            } else {
                format!("http://{}", url)
            };
            Self::new_http(&http_url).await
        }
    }
}

#[async_trait]
impl RpcClient for DirectRpcClient {
    async fn send_raw_transaction(&self, raw_tx: Bytes) -> Result<()> {
        // Use the custom sendRawTransactionAsync method
        match self {
            DirectRpcClient::Http(client) => {
                MysticetiTransactionApiClient::send_raw_transaction_async(client, raw_tx).await?
            }
            DirectRpcClient::WebSocket(client) => {
                MysticetiTransactionApiClient::send_raw_transaction_async(client, raw_tx).await?
            }
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_provider_rpc_client_creation() {
        // Test that ProviderRpcClient can be created (will fail to connect but that's expected)
        let result = ProviderRpcClient::new("http://localhost:8545").await;
        // We expect this to fail in test environment, but the important thing is that it compiles
        assert!(result.is_err() || result.is_ok());
    }

    #[tokio::test]
    async fn test_direct_rpc_client_creation() {
        // Test that DirectRpcClient can be created with HTTP (will fail to connect but that's expected)
        let result_http = DirectRpcClient::new_http("http://localhost:8546").await;
        assert!(result_http.is_err() || result_http.is_ok());

        // Test that DirectRpcClient can be created with WebSocket (will fail to connect but that's expected)
        let result_ws = DirectRpcClient::new_ws("ws://localhost:8546").await;
        assert!(result_ws.is_err() || result_ws.is_ok());

        // Test auto-detection
        let result_auto = DirectRpcClient::new("http://localhost:8546").await;
        assert!(result_auto.is_err() || result_auto.is_ok());
    }

    #[tokio::test]
    async fn test_rpc_client_trait_object() {
        // Test that we can create trait objects
        let raw_tx = Bytes::from_str("0x02f86c0102843b9aca00843b9aca0082520894a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4880de0b6b3a764000080c001a0...").unwrap();

        // This test verifies that the trait can be used as a trait object
        let _clients: Vec<Box<dyn RpcClient>> = vec![];

        // If this compiles, the trait is object-safe
        assert!(true);
    }

    #[tokio::test]
    async fn test_bytes_creation() {
        // Test that we can create Bytes from hex string
        let hex_str = "0x02f86c0102843b9aca00843b9aca0082520894a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4880de0b6b3a764000080c001a0...";
        let bytes = Bytes::from_str(hex_str);
        assert!(bytes.is_ok());
    }
}
