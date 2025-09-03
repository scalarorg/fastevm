use futures_util::StreamExt;
use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    PendingSubscriptionSink, SubscriptionMessage,
};

use reth_ethereum::{
    pool::{NewTransactionEvent, TransactionPool},
    tasks::TaskExecutor,
};
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
pub struct TxpoolListener<Pool> {
    #[allow(unused)]
    task_executor: TaskExecutor,
    pool: Pool,
}
impl<Pool> TxpoolListener<Pool> {
    pub fn new(task_executor: TaskExecutor, pool: Pool) -> Self {
        Self {
            task_executor,
            pool,
        }
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
        self.task_executor.spawn(Box::pin(async move {
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
    #[cfg(test)]
    use alloy_primitives::Bytes;
    use jsonrpsee::{
        http_client::HttpClientBuilder, server::ServerBuilder, ws_client::WsClientBuilder,
    };
    use reth_ethereum::pool::noop::NoopTransactionPool;
    use reth_extension::TxpoolListenerApiClient;
    use std::time::Duration;
    use tokio::time::sleep;

    // #[cfg(test)]
    // impl<Pool> TxpoolListenerApiServer for TxpoolListener<Pool>
    // where
    //     Pool: TransactionPool + Clone + 'static,
    // {
    //     fn transaction_count(&self) -> RpcResult<usize> {
    //         Ok(self.pool.pool_size().total)
    //     }

    //     fn subscribe_transactions(&self, pending: PendingSubscriptionSink) -> SubscriptionResult {
    //         let delay = 10;
    //         let pool = self.pool.clone();
    //         tokio::spawn(async move {
    //             // Accept the subscription
    //             let sink = match pending.accept().await {
    //                 Ok(sink) => sink,
    //                 Err(err) => {
    //                     eprintln!("failed to accept subscription: {err}");
    //                     return;
    //                 }
    //             };

    //             // Send pool size repeatedly, with a 10-second delay
    //             loop {
    //                 sleep(Duration::from_millis(delay)).await;
    //                 let message = SubscriptionMessage::from(
    //                     serde_json::value::to_raw_value(&pool.pool_size().total)
    //                         .expect("serialize usize"),
    //                 );

    //                 // Just ignore errors if a client has dropped
    //                 let _ = sink.send(message).await;
    //             }
    //         });
    //         Ok(())
    //     }
    // }

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
    async fn test_subscribe_transaction_ws() {
        let server_addr = start_server().await;
        let ws_url = format!("ws://{server_addr}");
        let client = WsClientBuilder::default().build(&ws_url).await.unwrap();

        let mut sub = TxpoolListenerApiClient::subscribe_transactions(&client)
            .await
            .expect("failed to subscribe");

        let first = sub.next().await.unwrap().unwrap();
        // assert_eq!(first, 0, "expected initial count to be 0");

        // let second = sub.next().await.unwrap().unwrap();
        // assert_eq!(second, 0, "still expected 0 from our NoopTransactionPool");
    }

    async fn start_server() -> std::net::SocketAddr {
        let server = ServerBuilder::default().build("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        let pool = NoopTransactionPool::default();
        let api = TxpoolListener {
            task_executor: TaskExecutor::current(),
            pool,
        };
        let server_handle = server.start(api.into_rpc());

        tokio::spawn(server_handle.stopped());

        addr
    }
}
