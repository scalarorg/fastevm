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
    /// Creates a subscription that returns the number of transactions in the pool every 10s.
    #[subscription(name = "subscribeTransactions", item = Bytes)]
    fn subscribe_transactions(&self) -> SubscriptionResult;
}

/// trait interface for a custom rpc namespace: `txpool`
///
/// This defines an additional namespace where all methods are configured as trait functions.
#[rpc(server, client, namespace = "consensus")]
pub trait ConsensusTransactionApi {
    /// Submit commited transactions
    #[method(name = "submitCommittedSubdag")]
    fn submit_committed_subdag(
        &self,
        #[argument(rename = "subdag")] subdag: CommittedSubDag,
    ) -> RpcResult<()>;
}
