use alloy_primitives::Bytes;
use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    proc_macros::rpc,
};

use crate::CommittedSubDag;
/// trait interface for a custom rpc namespace: `txpool`
///
/// This defines an additional namespace where all methods are configured as trait functions.
#[rpc(server, client, namespace = "txpoolListener")]
pub trait TxpoolListenerApi {
    /// Returns the number of transactions in the pool.
    #[method(name = "transactionCount")]
    fn transaction_count(&self) -> RpcResult<usize>;

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
}
