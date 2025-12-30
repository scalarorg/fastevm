//! FastEVM Execution Client Library
//!
//! This library provides the core components for the FastEVM execution layer,
//! including consensus pool management, payload building, and RPC handling.

#![warn(unused_crate_dependencies)]

pub mod args;
pub mod consensus;
pub mod payload;
pub mod pool;
pub mod rpc;
pub mod types;

use std::sync::Arc;

use alloy_rpc_types_eth::TransactionRequest;
// Re-export commonly used types
pub use consensus::{ConsensusPool, MysticetiConsensus};
use greth::{
    gravity_storage::block_view_storage::BlockViewStorage,
    reth_db::DatabaseEnv,
    reth_node_api::NodeTypesWithDBAdapter,
    reth_node_ethereum::EthereumNode,
    reth_pipe_exec_layer_ext_v2::PipeExecLayerApi,
    reth_provider::providers::BlockchainProvider,
    reth_rpc_api::eth::{helpers::EthCall, RpcTypes},
};
pub use payload::{MysticetiPayloadBuilder, MysticetiPayloadBuilderFactory};
pub use pool::MysticetiPoolBuilder;
pub use rpc::{
    MysticetiConsensusApiServer, MysticetiConsensusHandler, RawTransactionApiServer,
    TransactionHandler,
};
pub use types::TxValidatorConfig;

pub type RethBlockChainProvider =
    BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>;

pub trait RethEthCall:
    EthCall<NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>>
{
}

impl<T> RethEthCall for T where
    T: EthCall<NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>>
{
}

pub type RethPipeExecLayerApi<EthApi> =
    PipeExecLayerApi<BlockViewStorage<RethBlockChainProvider>, EthApi>;

/// Type alias for MysticetiConsensus with default Storage type
/// This allows omitting the Storage type parameter when pipeline_api is None
pub type MysticetiConsensusDefaultStorage<Provider, Payload, Pool, EthApi> =
    MysticetiConsensus<Provider, Payload, Pool, EthApi, BlockViewStorage<RethBlockChainProvider>>;
