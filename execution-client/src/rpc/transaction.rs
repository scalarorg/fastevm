use crate::types::TxValidatorConfig;
use alloy_primitives::Bytes;
use async_trait::async_trait;
use eyre::Result;
use futures_util::StreamExt;
use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    PendingSubscriptionSink, SubscriptionMessage,
};
use parking_lot::RwLock;
use reth_ethereum::{
    chainspec::EthereumHardforks,
    pool::TransactionPool,
    rpc::{
        api::eth::RpcConvert,
        eth::{utils::recover_raw_transaction, RpcNodeCore},
        EthApi,
    },
};
use reth_extension::{encode_transactions, MysticetiTransactionApiServer};
use reth_provider::{ChainSpecProvider, StateProviderFactory};
use reth_transaction_pool::{
    BlobStore, EthTransactionValidator, NewTransactionEvent, PoolTransaction, TransactionOrigin,
    TransactionValidationOutcome, TransactionValidationTaskExecutor, TransactionValidator,
    ValidPoolTransaction,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::{
    sync::{
        broadcast,
        mpsc::{self, Receiver},
    },
    task::JoinHandle,
};
use tracing::{debug, error, info, warn};
// Configuration constants for transaction batching
const BATCH_SIZE_THRESHOLD: usize = 100; // Send batch when we have 100 transactions
const BATCH_TIMEOUT_MS: u64 = 100; // Send batch after 10 ms even if not full
const DEFAULT_BROADCAST_CAPACITY: usize = 100_000;
const TX_QUEUE_CAPACITY: usize = 10_000; // Capacity of transaction processing queue
const LOG_BATCH_SIZE: usize = 100; // Log every N transactions to reduce I/O overhead

/// Validates a raw transaction and converts it to a pool transaction
/// Returns Ok(Some(transaction)) if valid, Ok(None) if invalid but recoverable, Err if fatal error
async fn validate_raw_transaction<
    Client: ChainSpecProvider<ChainSpec: EthereumHardforks> + StateProviderFactory,
    Pool: TransactionPool,
>(
    tx_validator: &Arc<RwLock<Option<EthTransactionValidator<Client, Pool::Transaction>>>>,
    raw_tx: &Bytes,
) -> Result<bool> {
    let transaction = recover_raw_transaction(&raw_tx)
        .map(|recovered| Pool::Transaction::from_pooled(recovered))?;
    let validator_guard = tx_validator.read();
    if let Some(validator) = &*validator_guard {
        let outcome = validator
            .validate_transaction(TransactionOrigin::Local, transaction)
            .await;
        return Ok(outcome.is_valid());
    }
    Ok(true)
}

/// Validates a batch of raw transactions and converts it to a pool transaction
/// Returns Ok(Some(transaction)) if valid, Ok(None) if invalid but recoverable, Err if fatal error
async fn validate_raw_transactions<
    Client: ChainSpecProvider<ChainSpec: EthereumHardforks> + StateProviderFactory,
    Pool: TransactionPool,
>(
    tx_validator: &Arc<RwLock<Option<EthTransactionValidator<Client, Pool::Transaction>>>>,
    raw_txs: &Vec<Bytes>,
) -> Result<Vec<TransactionValidationOutcome<Pool::Transaction>>> {
    let mut transactions = Vec::new();
    for raw_tx in raw_txs {
        let transaction = recover_raw_transaction(&raw_tx)
            .map(|recovered| Pool::Transaction::from_pooled(recovered))?;
        transactions.push(transaction);
    }
    let validator_guard = tx_validator.read();
    if let Some(validator) = &*validator_guard {
        let outcomes = validator
            .validate_transactions_with_origin(TransactionOrigin::Local, transactions)
            .await;
        return Ok(outcomes);
    }
    Ok(Vec::new())
}

/// The type that implements the `txpool` rpc namespace trait
pub struct TransactionHandler<
    Pool: TransactionPool + Clone + 'static,
    N: RpcNodeCore,
    Rpc: RpcConvert,
    C: ChainSpecProvider<ChainSpec: EthereumHardforks> + StateProviderFactory + 'static,
    S: BlobStore,
> {
    #[allow(unused)]
    pool: Pool,
    eth_api: EthApi<N, Rpc>,
    tx_validator: Arc<RwLock<Option<EthTransactionValidator<C, Pool::Transaction>>>>,
    config_receiver: Option<tokio::sync::oneshot::Receiver<TxValidatorConfig<C, S>>>,
    sender_raw_tx: broadcast::Sender<Vec<Bytes>>,
    // tx_queue_sender: mpsc::Sender<Bytes>,
    //worker_handle: tokio::task::JoinHandle<()>,
}

impl<
        Pool: TransactionPool + Clone + 'static,
        N: RpcNodeCore,
        Rpc: RpcConvert,
        C: ChainSpecProvider<ChainSpec: EthereumHardforks> + StateProviderFactory,
        S: BlobStore,
    > TransactionHandler<Pool, N, Rpc, C, S>
{
    pub fn new(pool: Pool, eth_api: EthApi<N, Rpc>) -> Self {
        let (sender_raw_tx, _) = broadcast::channel(DEFAULT_BROADCAST_CAPACITY);
        let (tx_queue_sender, _tx_queue_receiver) = mpsc::channel::<Bytes>(TX_QUEUE_CAPACITY);

        // Start single worker for transaction processing
        //info!("Starting single transaction processing worker");
        //let worker_handle = Self::start_worker(pool.clone(), tx_queue_receiver);

        Self {
            pool,
            eth_api,
            tx_validator: Arc::new(RwLock::new(None)),
            config_receiver: None,
            sender_raw_tx,
            // tx_queue_sender,
            // worker_handle,
        }
    }
    /// Start transaction processing worker to send transactions to reth pool
    /// This is not need if we use create payload from consensus pool only
    fn start_worker(pool: Pool, mut tx_queue_receiver: Receiver<Bytes>) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut processed_txs = 0;
            let mut last_log_count = 0;

            while let Some(raw_tx) = tx_queue_receiver.recv().await {
                processed_txs += 1;

                // Process the transaction
                match recover_raw_transaction(&raw_tx) {
                    Ok(recovered) => {
                        let pool_transaction =
                            <Pool as TransactionPool>::Transaction::from_pooled(recovered);
                        match pool
                            .add_transaction(TransactionOrigin::Local, pool_transaction)
                            .await
                        {
                            Ok(_) => {
                                // Only log debug messages occasionally to reduce I/O overhead
                                if processed_txs % LOG_BATCH_SIZE == 0 {
                                    debug!(
                                        "Successfully added transaction to pool (total: {})",
                                        processed_txs
                                    );
                                }
                            }
                            Err(e) => {
                                error!("Failed to add transaction to pool: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to recover transaction: {:?}", e);
                    }
                }

                // Log progress every LOG_BATCH_SIZE transactions
                if processed_txs - last_log_count >= LOG_BATCH_SIZE {
                    info!("Processed {} transactions", processed_txs);
                    last_log_count = processed_txs;
                }
            }

            info!(
                "Transaction processing worker shutting down (processed {} total)",
                processed_txs
            );
        })
    }
    pub fn with_config_receiver(
        mut self,
        receiver: tokio::sync::oneshot::Receiver<TxValidatorConfig<C, S>>,
    ) -> Self {
        self.config_receiver = Some(receiver);
        self
    }

    // /// Gracefully shutdown the worker thread
    // pub fn shutdown_worker(&self) {
    //     info!("Shutting down transaction processing worker...");
    //     self.worker_handle.abort();
    //     info!("Transaction processing worker shut down");
    // }
}
impl<
        Pool,
        N: RpcNodeCore,
        Rpc: RpcConvert,
        C: ChainSpecProvider<ChainSpec: EthereumHardforks> + StateProviderFactory + 'static,
        S: BlobStore,
    > TransactionHandler<Pool, N, Rpc, C, S>
where
    Pool: TransactionPool + Clone + 'static,
{
    // async fn validate_raw_transaction(&self, raw_tx: &Bytes) -> Result<bool> {
    //     // First, try to recover the transaction using the same validation as in mysticeti.rs
    //     // TODO: Implement proper validation logic
    //     let transaction = recover_raw_transaction(&raw_tx)
    //         .map(|recovered| Pool::Transaction::from_pooled(recovered));
    //     if transaction.is_err() {
    //         return Ok(false);
    //     }
    //     let validator_guard = self.tx_validator.read();
    //     let transaction = transaction.unwrap();
    //     if let Some(validator) = &*validator_guard {
    //         let outcome = validator
    //             .validate_transaction(TransactionOrigin::Local, transaction)
    //             .await;
    //         return Ok(outcome.is_valid());
    //     }
    //     Ok(true)
    // }
    /// Start validator reconstruction thread
    pub fn start_txvalidator_config_listener(&mut self) {
        if let Some(config_receiver) = self.config_receiver.take() {
            let tx_validator = Arc::clone(&self.tx_validator);

            std::thread::spawn(move || {
                // Wait for configuration from pool builder
                match config_receiver.blocking_recv() {
                    Ok(config) => {
                        info!("Received validator configuration, reconstructing validator in separate thread");

                        // Reconstruct the validator with the received config
                        let TxValidatorConfig {
                            provider,
                            head_timestamp,
                            max_tx_input_bytes,
                            tx_fee_cap,
                            max_tx_gas_limit,
                            minimum_priority_fee,
                            blob_store,
                            additional_validation_tasks,
                            pool_config,
                        } = config;

                        let validator = TransactionValidationTaskExecutor::eth_builder(provider)
                            .with_head_timestamp(head_timestamp)
                            .with_max_tx_input_bytes(max_tx_input_bytes)
                            .with_local_transactions_config(
                                pool_config.local_transactions_config.clone(),
                            )
                            .set_tx_fee_cap(tx_fee_cap)
                            .with_max_tx_gas_limit(max_tx_gas_limit)
                            .with_minimum_priority_fee(minimum_priority_fee)
                            .with_additional_tasks(additional_validation_tasks)
                            .build::<Pool::Transaction, S>(blob_store);

                        // Set the validator to self.tx_validator using thread-safe access
                        let mut validator_guard = tx_validator.write();
                        *validator_guard = Some(validator);
                        info!("Successfully set transaction validator");
                    }
                    Err(e) => {
                        error!("Failed to receive validator configuration: {:?}", e);
                    }
                }
            });
        }
    }
}

#[async_trait]
impl<
        Pool,
        N: RpcNodeCore,
        Rpc: RpcConvert,
        C: ChainSpecProvider<ChainSpec: EthereumHardforks> + StateProviderFactory + 'static,
        S: BlobStore,
    > MysticetiTransactionApiServer for TransactionHandler<Pool, N, Rpc, C, S>
where
    Pool: TransactionPool + Clone + 'static,
{
    fn transaction_count(&self) -> RpcResult<usize> {
        Ok(self.pool.pool_size().total)
    }
    async fn send_raw_transaction_async(&self, tx: Bytes) -> RpcResult<()> {
        // Broadcast raw transaction to subscribers
        let _ = self.sender_raw_tx.send(vec![tx]);
        // Try don't put transaction into reth pool
        // Send transaction to processing queue

        // if let Err(e) = self.tx_queue_sender.send(tx).await {
        //     error!("Failed to send transaction to processing queue: {:?}", e);
        //     return Err(ErrorObjectOwned::owned(
        //         INVALID_REQUEST_CODE,
        //         "Transaction queue is full".to_string(),
        //         None::<()>,
        //     ));
        // }

        Ok(())
    }
    async fn batch_send_raw_transaction_async(&self, txs: Vec<Bytes>) -> RpcResult<()> {
        // Broadcast raw transactions to subscribers
        let _ = self.sender_raw_tx.send(txs.clone());
        Ok(())
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
            // Pre-allocate with batch threshold to reduce reallocations
            let mut buffer: Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>> =
                Vec::with_capacity(BATCH_SIZE_THRESHOLD);

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
                                debug!("[Threshold] Sending batch of {} transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
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
                            debug!("[Timer] Sending batch of {} transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
                            let batch = std::mem::take(&mut buffer);
                            let msg = encode_transactions(batch);
                            let _ = sink.send(msg).await;
                        }
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
            // Pre-allocate with batch threshold to reduce reallocations
            let mut buffer: Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>> =
                Vec::with_capacity(BATCH_SIZE_THRESHOLD);

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
        info!("Subscribing to raw transactions with validation");
        //let mut receiver = self.eth_api.subscribe_to_raw_transactions();
        let mut receiver = self.sender_raw_tx.subscribe();
        let tx_validator = Arc::clone(&self.tx_validator);
        tokio::spawn(Box::pin(async move {
            let sink = match pending_subscription_sink.accept().await {
                Ok(sink) => sink,
                Err(e) => {
                    error!("Failed to accept subscription: {e}");
                    return;
                }
            };

            // Transaction buffer for batching - now stores validated transactions
            // Pre-allocate with batch threshold to reduce reallocations
            let mut buffer: Vec<Bytes> = Vec::with_capacity(BATCH_SIZE_THRESHOLD);

            // Create a periodic timer for batch timeout
            let mut batch_timer = tokio::time::interval(Duration::from_millis(BATCH_TIMEOUT_MS));
            let mut total_send_txs = 0_u64;
            let mut validation_failures = 0_u64;

            loop {
                tokio::select! {
                    // Handle new transaction events
                    Ok(raw_txs) = receiver.recv() => {
                        // Validate the raw transaction before adding to buffer
                        // let start_time = Instant::now();
                        // Reserve capacity if needed to avoid multiple reallocations
                        if buffer.len() + raw_txs.len() > buffer.capacity() {
                            buffer.reserve(BATCH_SIZE_THRESHOLD);
                        }
                        buffer.extend(raw_txs);
                        if buffer.len() >= BATCH_SIZE_THRESHOLD {
                            total_send_txs += buffer.len() as u64;
                            debug!("[Threshold] Sending batch of {} validated transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
                            let batch = std::mem::take(&mut buffer);
                            let msg = SubscriptionMessage::from(
                                serde_json::value::to_raw_value(&batch).expect("serialize batch"),
                            );
                            let _ = sink.send(msg).await;
                        }
                        // TODO: Add validation logic
                        // match validate_raw_transaction::<C, Pool>(&tx_validator, &raw_tx).await {
                        //     Ok(true) => {
                        //         // debug!("Transaction validated in {:?}", start_time.elapsed());
                        //         // Transaction is valid, add to buffer
                        //         buffer.push(raw_tx);

                        //         // Send batch if threshold is reached
                        //         if buffer.len() >= BATCH_SIZE_THRESHOLD {
                        //             total_send_txs += buffer.len() as u64;
                        //             info!("[Threshold] Sending batch of {} validated transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
                        //             let batch = std::mem::take(&mut buffer);
                        //             let msg = SubscriptionMessage::from(
                        //                 serde_json::value::to_raw_value(&batch).expect("serialize batch"),
                        //             );
                        //             let _ = sink.send(msg).await;
                        //         }
                        //     }
                        //     Ok(false) => {
                        //         // Transaction failed validation, skip it
                        //         validation_failures += 1;
                        //         warn!("Skipping invalid transaction. Total validation failures: {}", validation_failures);
                        //     },
                        //     Err(e) => {
                        //         // Fatal validation error
                        //         error!("Fatal validation error: {}", e);
                        //         validation_failures += 1;
                        //     }
                        // }
                    }
                    // Handle batch timeout
                    _ = batch_timer.tick() => {
                        if !buffer.is_empty() {
                            total_send_txs += buffer.len() as u64;
                            info!("[Timer] Sending batch of {} validated transactions. Total sent transactions: {}", buffer.len(), total_send_txs);
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
    use reth_extension::MysticetiTransactionApiClient;
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

        let mut sub = MysticetiTransactionApiClient::subscribe_all_transactions(&client)
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
