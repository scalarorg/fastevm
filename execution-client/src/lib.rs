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

// Re-export commonly used types
pub use consensus::{ConsensusPool, MysticetiConsensus};
pub use payload::{MysticetiPayloadBuilder, MysticetiPayloadBuilderFactory};
pub use pool::MysticetiPoolBuilder;
pub use rpc::{
    MysticetiConsensusApiServer, MysticetiConsensusHandler, RawTransactionApiServer,
    TransactionHandler,
};
pub use types::TxValidatorConfig;

