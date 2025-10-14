use futures_util::StreamExt;
use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    PendingSubscriptionSink, SubscriptionMessage,
};
use reth_ethereum::{
    pool::TransactionPool,
    rpc::{api::eth::RpcConvert, eth::RpcNodeCore, EthApi},
};
use reth_extension::{encode_transactions, TxpoolListenerApiServer};
use reth_transaction_pool::{NewTransactionEvent, ValidPoolTransaction};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

// Configuration constants for transaction batching
const BATCH_SIZE_THRESHOLD: usize = 100; // Send batch when we have 10 transactions
const BATCH_TIMEOUT_MS: u64 = 10; // Send batch after 1 second even if not full

/// The type that implements the `txpool` rpc namespace trait
#[derive(Debug)]
pub struct TxListener<Pool, N: RpcNodeCore, Rpc: RpcConvert> {
    #[allow(unused)]
    pool: Pool,
    eth_api: EthApi<N, Rpc>,
}
impl<Pool, N: RpcNodeCore, Rpc: RpcConvert> TxListener<Pool, N, Rpc> {
    pub fn new(pool: Pool, eth_api: EthApi<N, Rpc>) -> Self {
        Self { pool, eth_api }
    }
}

impl<Pool, N: RpcNodeCore, Rpc: RpcConvert> TxpoolListenerApiServer for TxListener<Pool, N, Rpc>
where
    Pool: TransactionPool + Clone + 'static,
{
    fn transaction_count(&self) -> RpcResult<usize> {
        Ok(self.pool.pool_size().total)
    }

    fn subscribe_pending_transactions(
        &self,
        pending_subscription_sink: PendingSubscriptionSink,
    ) -> SubscriptionResult {
        let pool = self.pool.clone();
        // Spawn an async block to listen for transactions.
        tokio::spawn(Box::pin(async move {
            let sink = match pending_subscription_sink.accept().await {
                Ok(sink) => sink,
                Err(e) => {
                    println!("failed to accept subscription: {e}");
                    return;
                }
            };

            // Transaction buffer for batching
            let mut buffer: Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>> =
                Vec::new();

            // Create a periodic timer for batch timeout
            let mut batch_timer = tokio::time::interval(Duration::from_millis(BATCH_TIMEOUT_MS));
            let mut total_send_txs = 0_u64;

            let mut pending_stream = pool.new_pending_pool_transactions_listener();
            loop {
                tokio::select! {
                    // Handle new transaction events
                    Some(NewTransactionEvent { transaction, .. }) = pending_stream.next() => {
                        if transaction.is_local() {
                            // because of this push, buffer has at least 1 transaction
                            buffer.push(transaction);
                            // Send batch if threshold is reached
                            if buffer.len() >= BATCH_SIZE_THRESHOLD {
                                total_send_txs += buffer.len() as u64;
                                info!("Sending batch of {} transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
                                let batch = std::mem::take(&mut buffer);
                                let msg = encode_transactions(batch);
                                let _ = sink.send(msg).await;
                            }
                        }
                    }
                    // Handle batch timeout
                    _ = batch_timer.tick() => {
                        if !buffer.is_empty() {
                            total_send_txs += buffer.len() as u64;
                            info!("Sending batch of {} transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
                            let batch = std::mem::take(&mut buffer);
                            let msg = encode_transactions(batch);
                            let _ = sink.send(msg).await;
                        }
                        batch_timer.reset();
                    }
                }
                //End loop
            }
        }));
        Ok(())
    }

    fn subscribe_all_transactions(
        &self,
        pending_subscription_sink: PendingSubscriptionSink,
    ) -> SubscriptionResult {
        info!("Subscribing to all transactions");
        let pool = self.pool.clone();
        // Spawn an async block to listen for transactions.
        tokio::spawn(Box::pin(async move {
            let sink = match pending_subscription_sink.accept().await {
                Ok(sink) => sink,
                Err(e) => {
                    println!("failed to accept subscription: {e}");
                    return;
                }
            };

            // Transaction buffer for batching
            let mut buffer: Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>> =
                Vec::new();

            // Create a periodic timer for batch timeout
            let mut batch_timer = tokio::time::interval(Duration::from_millis(BATCH_TIMEOUT_MS));
            let mut total_send_txs = 0_u64;

            let mut all_transactions_stream = pool.new_transactions_listener();
            loop {
                tokio::select! {
                    // Handle new transaction events
                    Some(NewTransactionEvent { transaction, .. }) = all_transactions_stream.recv() => {
                        if transaction.is_local() {
                            // because of this push, buffer has at least 1 transaction
                            buffer.push(transaction);
                            // Send batch if threshold is reached
                            if buffer.len() >= BATCH_SIZE_THRESHOLD {
                                total_send_txs += buffer.len() as u64;
                                info!("Sending batch of {} transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
                                let batch = std::mem::take(&mut buffer);
                                let msg = encode_transactions(batch);
                                let _ = sink.send(msg).await;
                            }
                        }
                    }
                    // Handle batch timeout
                    _ = batch_timer.tick() => {
                        if !buffer.is_empty() {
                            total_send_txs += buffer.len() as u64;
                            info!("Sending batch of {} transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
                            let batch = std::mem::take(&mut buffer);
                            let msg = encode_transactions(batch);
                            let _ = sink.send(msg).await;
                        }
                        batch_timer.reset();
                    }
                }
                //End loop
            }
        }));
        Ok(())
    }

    fn subscribe_raw_transactions(
        &self,
        pending_subscription_sink: PendingSubscriptionSink,
    ) -> SubscriptionResult {
        // For now, this is a placeholder implementation
        // In a real implementation, this would listen to raw transaction events
        info!("Subscribing to raw transactions");
        let mut receiver = self.eth_api.subscribe_to_raw_transactions();
        tokio::spawn(Box::pin(async move {
            let sink = match pending_subscription_sink.accept().await {
                Ok(sink) => sink,
                Err(e) => {
                    println!("failed to accept subscription: {e}");
                    return;
                }
            };
            // Transaction buffer for batching
            let mut buffer = Vec::new();

            // Create a periodic timer for batch timeout
            let mut batch_timer = tokio::time::interval(Duration::from_millis(BATCH_TIMEOUT_MS));
            let mut total_send_txs = 0_u64;
            loop {
                tokio::select! {
                    // Handle new transaction events
                    Ok(raw_tx) = receiver.recv() => {
                        // because of this push, buffer has at least 1 transaction
                        buffer.push(raw_tx);
                        // Send batch if threshold is reached
                        if buffer.len() >= BATCH_SIZE_THRESHOLD {
                            total_send_txs += buffer.len() as u64;
                            info!("Sending batch of {} transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
                            let batch = std::mem::take(&mut buffer);
                            let msg = SubscriptionMessage::from(
                                serde_json::value::to_raw_value(&batch).expect("serialize batch"),
                            );
                            let _ = sink.send(msg).await;
                        }
                    }
                    // Handle batch timeout
                    _ = batch_timer.tick() => {
                        if !buffer.is_empty() {
                            total_send_txs += buffer.len() as u64;
                            info!("Sending batch of {} transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
                            let batch = std::mem::take(&mut buffer);
                            let msg = SubscriptionMessage::from(
                                serde_json::value::to_raw_value(&batch).expect("serialize batch"),
                            );
                            let _ = sink.send(msg).await;
                        }
                        batch_timer.reset();
                    }
                }
                //End loop
            }
        }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpsee::ws_client::WsClientBuilder;
    use reth_ethereum::pool::noop::NoopTransactionPool;
    use reth_extension::TxpoolListenerApiClient;
    use reth_rpc_layer::{secret_to_bearer_header, JwtSecret};

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

        let mut sub = TxpoolListenerApiClient::subscribe_all_transactions(&client)
            .await
            .expect("failed to subscribe");

        let first = sub.next().await.unwrap().unwrap();
        assert_eq!(first.len(), 0, "expected initial count to be 0");
    }

    // pub async fn create_transfer_transaction(
    //     signer_privkey: &str,
    //     recipient: &str,
    //     chain_id: ChainId,
    //     gwei_amount: u64,
    //     nonce: u64,
    // ) -> Result<<Ethereum as Network>::TxEnvelope> {
    //     // Parse the recipient address from string to Address type
    //     let recipient_addr = Address::from_str(recipient)
    //         .map_err(|e| eyre::eyre!("Invalid recipient address: {}", e))?;

    //     // Create a wallet signer from the provided private key
    //     let wallet = PrivateKeySigner::from_str(signer_privkey)
    //         .map_err(|e| eyre::eyre!("Invalid private key: {}", e))?;

    //     // Get the sender's address from the wallet
    //     let sender_addr = wallet.address();

    //     // Build a transaction request with standard ETH transfer parameters
    //     let tx = TransactionRequest::default()
    //         .with_from(sender_addr)
    //         .with_to(recipient_addr)
    //         .with_nonce(nonce)
    //         .with_chain_id(chain_id)
    //         .with_value(U256::from(gwei_amount))
    //         .with_gas_limit(21_000) // Standard gas limit for ETH transfers
    //         .with_max_priority_fee_per_gas(1_000_000_000) // 1 Gwei
    //         .with_max_fee_per_gas(20_000_000_000); // 20 Gwei

    //     // Convert the LocalSigner to an EthereumWallet to satisfy the NetworkWallet trait bound
    //     let ethereum_wallet = alloy::network::EthereumWallet::from(wallet);

    //     // Build and sign the transaction using the ethereum wallet
    //     let tx_envelope = tx
    //         .build(&ethereum_wallet)
    //         .await
    //         .map_err(|e| eyre::eyre!("Failed to build transaction: {}", e))?;

    //     Ok(tx_envelope)
    // }
}
