use alloy_consensus::Transaction;
use futures_util::StreamExt;
use reth_ethereum::{
    node::{
        api::FullNodeComponents,
        builder::{rpc::RethRpcAddOns, FullNode},
    },
    pool::{NewTransactionEvent, TransactionPool},
};
use tokio_stream::wrappers::ReceiverStream;
use tracing::info;
pub(crate) fn start_transaction_listener<Node, AddOns>(node: FullNode<Node, AddOns>)
where
    Node: FullNodeComponents,
    AddOns: RethRpcAddOns<Node>,
{
    // Get the transaction pool from the node
    let pending_transactions = node.pool.new_transactions_listener();
    // Convert Receiver to Stream
    let mut stream = ReceiverStream::new(pending_transactions);
    // Spawn an async block to listen for transactions.
    node.task_executor.spawn(Box::pin(async move {
        // Waiting for new transactions
        while let Some(NewTransactionEvent { transaction, .. }) = stream.next().await {
            info!("Transaction received: {transaction:?}");
            if transaction.is_local() {
                let input = transaction.transaction.input();
                let input = String::from_utf8(input.to_vec()).unwrap();
                info!("Input: {input}");
                info!("Transaction is local");
            }
        }
    }));
}
