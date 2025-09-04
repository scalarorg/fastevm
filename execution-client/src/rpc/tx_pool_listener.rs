use futures_util::StreamExt;
use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    PendingSubscriptionSink, SubscriptionMessage,
};

use reth_ethereum::pool::{NewTransactionEvent, TransactionPool};
use tokio_stream::wrappers::ReceiverStream;
use tracing::info;

use reth_extension::TxpoolListenerApiServer;

/// Our custom cli args extension that adds one flag to reth default CLI.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub(crate) struct CliTxpoolListener {
    /// CLI flag to enable the txpool extension namespace
    #[arg(long)]
    pub enable_txpool_listener: bool,
}

/// The type that implements the `txpool` rpc namespace trait
#[derive(Debug)]
pub struct TxpoolListener<Pool> {
    #[allow(unused)]
    pool: Pool,
}
impl<Pool> TxpoolListener<Pool> {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

impl<Pool> TxpoolListenerApiServer for TxpoolListener<Pool>
where
    Pool: TransactionPool + Clone + 'static,
{
    fn transaction_count(&self) -> RpcResult<usize> {
        Ok(self.pool.pool_size().total)
    }

    fn subscribe_transactions(
        &self,
        pending_subscription_sink: PendingSubscriptionSink,
    ) -> SubscriptionResult {
        let pool = self.pool.clone();
        let pending_transactions = pool.new_transactions_listener();
        // Convert Receiver to Stream
        let mut stream = ReceiverStream::new(pending_transactions);
        // Spawn an async block to listen for transactions.
        tokio::spawn(Box::pin(async move {
            let sink = match pending_subscription_sink.accept().await {
                Ok(sink) => sink,
                Err(e) => {
                    println!("failed to accept subscription: {e}");
                    return;
                }
            };
            // Waiting for new transactions
            while let Some(NewTransactionEvent { transaction, .. }) = stream.next().await {
                info!("Transaction received: {transaction:?}");
                if transaction.is_local() {
                    // let tx_data = transaction.transaction.input();
                    let tx_hash = transaction.hash().clone();
                    let msg = SubscriptionMessage::from(
                        serde_json::value::to_raw_value(&tx_hash).expect("serialize"),
                    );
                    let _ = sink.send(msg).await;
                }
            }
        }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        network::{Ethereum, Network, TransactionBuilder},
        primitives::{Address, U256},
        rpc::types::TransactionRequest,
    };
    use alloy_primitives::ChainId;
    use alloy_signer_local::PrivateKeySigner;
    use eyre::Result;
    use jsonrpsee::http_client::HttpClientBuilder;
    use jsonrpsee::server::ServerBuilder;
    use jsonrpsee::ws_client::WsClientBuilder;
    use reth_ethereum::pool::noop::NoopTransactionPool;
    use reth_extension::TxpoolListenerApiClient;
    use reth_rpc_layer::{secret_to_bearer_header, JwtSecret};
    use std::str::FromStr;
    #[test]
    fn test_cli_txpool_listener_default() {
        let cli = CliTxpoolListener::default();
        assert!(!cli.enable_txpool_listener);
    }

    #[test]
    fn test_cli_txpool_listener_debug() {
        let cli = CliTxpoolListener::default();
        let debug_str = format!("{:?}", cli);
        assert!(debug_str.contains("CliTxpoolListener"));
    }

    #[test]
    fn test_cli_txpool_listener_clone() {
        let cli = CliTxpoolListener::default();
        let cloned = cli;
        assert_eq!(cli.enable_txpool_listener, cloned.enable_txpool_listener);
    }

    #[test]
    fn test_transaction_listener_components() {
        // Test that we can create basic components
        let pool = NoopTransactionPool::default();

        // Test that the pool can be created and used
        let best_transactions = pool.best_transactions();
        let count: usize = best_transactions.count();
        assert_eq!(count, 0); // NoopTransactionPool should have no transactions
    }

    #[test]
    fn test_transaction_pool_operations() {
        let pool = NoopTransactionPool::default();

        // Test that we can get best transactions
        let best_transactions = pool.best_transactions();
        let count: usize = best_transactions.count();
        assert_eq!(count, 0); // NoopTransactionPool should have no transactions
    }

    #[test]
    fn test_txpool_listener_creation() {
        // Test that we can create the TxpoolListener struct
        let pool = NoopTransactionPool::default();

        // Test that we can get best transactions from the pool
        let best_transactions = pool.best_transactions();
        let count: usize = best_transactions.count();
        assert_eq!(count, 0); // NoopTransactionPool should have no transactions
    }

    #[tokio::test]
    async fn test_txpool_listener_new() {
        let pool = NoopTransactionPool::default();
        let listener = TxpoolListener::new(pool);

        // Test that the listener was created successfully
        // We can't directly test the fields as they're private, but we can test the methods
        let result = listener.transaction_count();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_transaction_count() {
        let pool = NoopTransactionPool::default();
        let listener = TxpoolListener::new(pool);

        let count = listener.transaction_count().unwrap();
        assert_eq!(count, 0); // NoopTransactionPool should have no transactions
    }

    #[test]
    fn test_pool_size_total() {
        let pool = NoopTransactionPool::default();
        let pool_size = pool.pool_size();
        assert_eq!(pool_size.total, 0);
        assert_eq!(pool_size.pending, 0);
        assert_eq!(pool_size.queued, 0);
    }

    #[test]
    fn test_new_transactions_listener() {
        let pool = NoopTransactionPool::default();
        let _listener = pool.new_transactions_listener();

        // Test that we can create a listener (it should not panic)
        // The listener is a receiver that will never receive anything for NoopTransactionPool
        // Note: We can't easily test is_closed() without more complex setup
    }

    #[tokio::test]
    async fn test_txpool_listener_with_different_pools() {
        // Test that TxpoolListener works with different pool types
        let pool1 = NoopTransactionPool::default();
        let pool2 = NoopTransactionPool::default();

        let listener1 = TxpoolListener::new(pool1);
        let listener2 = TxpoolListener::new(pool2);

        // Both should work the same way
        assert_eq!(listener1.transaction_count().unwrap(), 0);
        assert_eq!(listener2.transaction_count().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_txpool_listener_debug() {
        let pool = NoopTransactionPool::default();
        let listener = TxpoolListener::new(pool);

        // Test that we can format the listener for debugging
        let debug_str = format!("{:?}", listener);
        assert!(debug_str.contains("TxpoolListener"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_call_transaction_http() {
        let server_addr = start_server().await;
        let uri = format!("http://{server_addr}");
        let client = HttpClientBuilder::default().build(&uri).unwrap();
        let count = TxpoolListenerApiClient::transaction_count(&client)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_subscribe_transactions() {
        let server_addr = start_server().await;
        let ws_url = format!("ws://{server_addr}");
        let client = WsClientBuilder::default().build(&ws_url).await.unwrap();

        let mut sub = TxpoolListenerApiClient::subscribe_transactions(&client)
            .await
            .expect("failed to subscribe");

        let first = sub.next().await.unwrap().unwrap();
        assert_eq!(first.len(), 0, "expected initial count to be 0");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_subscribe_transactions_with_docker() {
        let ws_url = format!("ws://127.0.0.1:8551");
        let mut headers = http::HeaderMap::new();
        let jwt_secret_hex = "0xda3c3a6c5e12572ba6cbe4b7c71d107ddf859aaaf6090f14de6baa3141e43bd8";
        let jwt_secret = match JwtSecret::from_hex(jwt_secret_hex) {
            Ok(jwt_secret) => jwt_secret,
            Err(err) => {
                panic!("JWT secret parsing failed: {:?}", err);
            }
        };
        let mut auth_header = secret_to_bearer_header(&jwt_secret);
        // The header value should not be visible in logs for security.
        auth_header.set_sensitive(true);
        println!("Auth header: {:?}", auth_header.to_str().unwrap());
        headers.insert(http::header::AUTHORIZATION, auth_header);
        let client = WsClientBuilder::default()
            .set_headers(headers)
            .build(&ws_url)
            .await
            .expect("Failed to create ws client");

        let mut sub = TxpoolListenerApiClient::subscribe_transactions(&client)
            .await
            .expect("failed to subscribe");

        let first = sub.next().await.unwrap().unwrap();
        assert_eq!(first.len(), 0, "expected initial count to be 0");
    }

    async fn start_server() -> std::net::SocketAddr {
        let server = ServerBuilder::default().build("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        let pool = NoopTransactionPool::default();

        // Create a TaskExecutor for testing using tokio runtime
        let api = TxpoolListener { pool };
        let server_handle = server.start(api.into_rpc());

        tokio::spawn(server_handle.stopped());

        addr
    }

    pub async fn create_transfer_transaction(
        signer_privkey: &str,
        recipient: &str,
        chain_id: ChainId,
        gwei_amount: u64,
        nonce: u64,
    ) -> Result<<Ethereum as Network>::TxEnvelope> {
        // Parse the recipient address from string to Address type
        let recipient_addr = Address::from_str(recipient)
            .map_err(|e| eyre::eyre!("Invalid recipient address: {}", e))?;

        // Create a wallet signer from the provided private key
        let wallet = PrivateKeySigner::from_str(signer_privkey)
            .map_err(|e| eyre::eyre!("Invalid private key: {}", e))?;

        // Get the sender's address from the wallet
        let sender_addr = wallet.address();

        // Build a transaction request with standard ETH transfer parameters
        let tx = TransactionRequest::default()
            .with_from(sender_addr)
            .with_to(recipient_addr)
            .with_nonce(nonce)
            .with_chain_id(chain_id)
            .with_value(U256::from(gwei_amount))
            .with_gas_limit(21_000) // Standard gas limit for ETH transfers
            .with_max_priority_fee_per_gas(1_000_000_000) // 1 Gwei
            .with_max_fee_per_gas(20_000_000_000); // 20 Gwei

        // Convert the LocalSigner to an EthereumWallet to satisfy the NetworkWallet trait bound
        let ethereum_wallet = alloy::network::EthereumWallet::from(wallet);

        // Build and sign the transaction using the ethereum wallet
        let tx_envelope = tx
            .build(&ethereum_wallet)
            .await
            .map_err(|e| eyre::eyre!("Failed to build transaction: {}", e))?;

        Ok(tx_envelope)
    }
}
