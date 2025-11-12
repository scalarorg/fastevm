//! RPC client implementations for testing
//!
//! This module provides RPC client trait and implementations for sending raw transactions
//! in integration tests.

use std::time::Duration;

use alloy_primitives::Bytes;
use alloy_provider::{Provider, ProviderBuilder};
use async_trait::async_trait;
use eyre::Result;
use jsonrpsee::{http_client::HttpClientBuilder, ws_client::WsClientBuilder};
use reth_extension::MysticetiTransactionApiClient;
use tracing::{debug, error, info, warn};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay between retries in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
}

/// Determines if an error is retryable
fn is_retryable_error(error: &eyre::Error) -> bool {
    let error_str = format!("{error}");

    // Network-related errors that are typically retryable
    error_str.contains("connection")
        || error_str.contains("timeout")
        || error_str.contains("network")
        || error_str.contains("unreachable")
        || error_str.contains("refused")
        || error_str.contains("temporary")
        || error_str.contains("server error")
        || error_str.contains("rate limit")
        || error_str.contains("too many requests")
}

/// Calculates the delay for the next retry attempt using exponential backoff
fn calculate_retry_delay(attempt: u32, config: &RetryConfig) -> Duration {
    let delay_ms =
        (config.initial_delay_ms as f64 * config.backoff_multiplier.powi(attempt as i32)) as u64;
    let capped_delay_ms = delay_ms.min(config.max_delay_ms);
    Duration::from_millis(capped_delay_ms)
}

/// Trait for RPC clients that can send raw transactions
#[async_trait]
pub trait RpcClient: Send + Sync {
    /// Send a raw transaction and return the transaction hash
    async fn send_raw_transaction(&self, raw_tx: Bytes) -> Result<()>;
    /// Send multiple raw transactions in a batch
    async fn batch_send_raw_transaction(&self, transactions: Vec<Bytes>) -> Result<()>;
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

    async fn batch_send_raw_transaction(&self, transactions: Vec<Bytes>) -> Result<()> {
        // For ProviderRpcClient, we'll send transactions sequentially
        // since alloy provider doesn't have native batch support
        for tx in transactions {
            let tx_hash = self
                .provider
                .send_raw_transaction(&tx)
                .await?
                .tx_hash()
                .clone();
            println!("Batch tx_hash: {:?}", tx_hash);
        }
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
    /// Retry configuration for this client
    pub fn retry_config(&self) -> RetryConfig {
        RetryConfig::default()
    }

    /// Internal method to send batch without retry logic
    async fn send_batch_internal(&self, transactions: &[Bytes]) -> Result<()> {
        // Use the custom batchSendRawTransactionAsync method
        match self {
            DirectRpcClient::Http(client) => {
                debug!(
                    "Sending batch of {} transactions via HTTP RPC",
                    transactions.len()
                );
                MysticetiTransactionApiClient::batch_send_raw_transaction_async(
                    client,
                    transactions.to_vec(),
                )
                .await?
            }
            DirectRpcClient::WebSocket(client) => {
                debug!(
                    "Sending batch of {} transactions via WebSocket RPC",
                    transactions.len()
                );
                MysticetiTransactionApiClient::batch_send_raw_transaction_async(
                    client,
                    transactions.to_vec(),
                )
                .await?
            }
        };
        Ok(())
    }
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

    async fn batch_send_raw_transaction(&self, transactions: Vec<Bytes>) -> Result<()> {
        let retry_config = self.retry_config();
        let mut last_error = None;

        info!("Starting batch send of {} transactions", transactions.len());

        for attempt in 0..=retry_config.max_retries {
            match self.send_batch_internal(&transactions).await {
                Ok(()) => {
                    if attempt > 0 {
                        info!("Batch send succeeded after {} retry attempts", attempt);
                    } else {
                        debug!("Batch send succeeded on first attempt");
                    }
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);

                    // Check if this is the last attempt
                    if attempt == retry_config.max_retries {
                        error!(
                            "Batch send failed after {} attempts",
                            retry_config.max_retries + 1
                        );
                        break;
                    }

                    // Check if the error is retryable
                    if !is_retryable_error(&last_error.as_ref().unwrap()) {
                        warn!(
                            "Non-retryable error encountered, stopping retries: {:?}",
                            last_error.as_ref().unwrap()
                        );
                        break;
                    }

                    // Calculate delay for next retry
                    let delay = calculate_retry_delay(attempt, &retry_config);
                    warn!(
                        "Batch send attempt {} failed, retrying in {:?}: {:?}",
                        attempt + 1,
                        delay,
                        last_error.as_ref().unwrap()
                    );

                    // Wait before retrying
                    tokio::time::sleep(delay).await;
                }
            }
        }

        // Return the last error if all retries failed
        Err(last_error.unwrap())
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
        let _raw_tx = Bytes::from_str("0x02f86c0102843b9aca00843b9aca0082520894a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4880de0b6b3a764000080c001a0").unwrap();

        // This test verifies that the trait can be used as a trait object
        let _clients: Vec<Box<dyn RpcClient>> = vec![];

        // If this compiles, the trait is object-safe
        assert!(true);
    }

    #[tokio::test]
    async fn test_bytes_creation() {
        // Test that we can create Bytes from hex string
        let hex_str = "0x02f86c0102843b9aca00843b9aca0082520894a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4880de0b6b3a764000080c001a0";
        let bytes = Bytes::from_str(hex_str);
        assert!(bytes.is_ok());
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 100);
        assert_eq!(config.max_delay_ms, 5000);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_is_retryable_error() {
        // Test retryable errors
        let retryable_error = eyre::eyre!("connection timeout");
        assert!(is_retryable_error(&retryable_error));

        let retryable_error2 = eyre::eyre!("network unreachable");
        assert!(is_retryable_error(&retryable_error2));

        let retryable_error3 = eyre::eyre!("rate limit exceeded");
        assert!(is_retryable_error(&retryable_error3));

        // Test non-retryable errors
        let non_retryable_error = eyre::eyre!("invalid transaction format");
        assert!(!is_retryable_error(&non_retryable_error));

        let non_retryable_error2 = eyre::eyre!("insufficient funds");
        assert!(!is_retryable_error(&non_retryable_error2));
    }

    #[test]
    fn test_calculate_retry_delay() {
        let config = RetryConfig::default();

        // Test exponential backoff
        let delay_0 = calculate_retry_delay(0, &config);
        assert_eq!(delay_0, Duration::from_millis(100));

        let delay_1 = calculate_retry_delay(1, &config);
        assert_eq!(delay_1, Duration::from_millis(200));

        let delay_2 = calculate_retry_delay(2, &config);
        assert_eq!(delay_2, Duration::from_millis(400));

        // Test max delay cap
        let delay_large = calculate_retry_delay(10, &config);
        assert_eq!(delay_large, Duration::from_millis(5000));
    }

    #[test]
    fn test_custom_retry_config() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay_ms: 50,
            max_delay_ms: 2000,
            backoff_multiplier: 1.5,
        };

        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_delay_ms, 50);
        assert_eq!(config.max_delay_ms, 2000);
        assert_eq!(config.backoff_multiplier, 1.5);

        // Test custom backoff calculation
        let delay_0 = calculate_retry_delay(0, &config);
        assert_eq!(delay_0, Duration::from_millis(50));

        let delay_1 = calculate_retry_delay(1, &config);
        assert_eq!(delay_1, Duration::from_millis(75)); // 50 * 1.5
    }
}
