//! Local RPC API trait definitions using the correct jsonrpsee version.
//! These traits are compatible with gravity-reth's jsonrpsee version (0.26)
//! while using types from rpc-shared-api.

use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    proc_macros::rpc,
};
use rpc_shared_api::CommittedSubDag;

/// Bytes type alias for raw transaction data.
pub type Bytes = Vec<u8>;

/// RPC trait for raw transaction operations.
#[rpc(server, client, namespace = "rawtx")]
pub trait RawTransactionApi {
    /// Send a raw transaction to the network.
    #[method(name = "sendRawTransactionAsync")]
    async fn send_raw_transaction_async(&self, bytes: Bytes) -> RpcResult<()>;

    /// Send multiple raw transactions to the network in a batch.
    #[method(name = "sendRawTransactionsAsync")]
    async fn send_raw_transactions_async(&self, transactions: Vec<Bytes>) -> RpcResult<()>;

    /// Creates a subscription that listens to all raw transactions when it comes to rpc server.
    #[subscription(name = "subscribeRawTransactions", item = Vec<Bytes>)]
    fn subscribe_raw_transactions(&self) -> SubscriptionResult;
}

/// RPC trait for Mysticeti consensus operations.
#[rpc(server, client, namespace = "mysticeti")]
pub trait MysticetiConsensusApi {
    /// Submit committed transactions
    #[method(name = "submitCommittedSubdags")]
    fn submit_committed_subdags(
        &self,
        #[argument(rename = "subdag")] subdags: Vec<CommittedSubDag>,
    ) -> RpcResult<()>;

    /// Submit a single committed subdag
    #[method(name = "submitCommittedSubdag")]
    fn submit_committed_subdag(
        &self,
        #[argument(rename = "subdag")] subdag: CommittedSubDag,
    ) -> RpcResult<()>;
}

