use alloy_primitives::Bytes;
use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    proc_macros::rpc,
};

use crate::CommittedSubDag;
/// trait interface for a custom rpc namespace: `txpool`
///
/// This defines an additional namespace where all methods are configured as trait functions.
#[rpc(server, client, namespace = "mysticeti")]
pub trait MysticetiTransactionApi {
    /// Returns the number of transactions in the pool.
    #[method(name = "transactionCount")]
    fn transaction_count(&self) -> RpcResult<usize>;
    /// Send a raw transaction to the network.
    #[method(name = "sendRawTransactionAsync")]
    async fn send_raw_transaction_async(&self, bytes: Bytes) -> RpcResult<()>;
    /// Send multiple raw transactions to the network in a batch.
    #[method(name = "batchSendRawTransactionAsync")]
    async fn batch_send_raw_transaction_async(&self, transactions: Vec<Bytes>) -> RpcResult<()>;
    /// Creates a subscription that listens to pending transactions in the pool.
    #[subscription(name = "subscribePendingTransactions", item = Vec<Bytes>)]
    fn subscribe_pending_transactions(&self) -> SubscriptionResult;

    /// Creates a subscription that listens to all transactions in the pool.
    #[subscription(name = "subscribeAllTransactions", item = Vec<Bytes>)]
    fn subscribe_all_transactions(&self) -> SubscriptionResult;

    /// Creates a subscription that listens to all raw transactions when it comes to rpc server.
    #[subscription(name = "subscribeRawTransactions", item = Vec<Bytes>)]
    fn subscribe_raw_transactions(&self) -> SubscriptionResult;
}

/// trait interface for a custom rpc namespace: `txpool`
///
/// This defines an additional namespace where all methods are configured as trait functions.
#[rpc(server, client, namespace = "mysticeti")]
pub trait MysticetiConsensusApi {
    /// Submit commited transactions
    #[method(name = "submitCommittedSubdags")]
    fn submit_committed_subdags(
        &self,
        #[argument(rename = "subdag")] subdags: Vec<CommittedSubDag>,
    ) -> RpcResult<()>;

    #[method(name = "submitCommittedSubdag")]
    fn submit_committed_subdag(
        &self,
        #[argument(rename = "subdag")] subdag: CommittedSubDag,
    ) -> RpcResult<()>;
}
