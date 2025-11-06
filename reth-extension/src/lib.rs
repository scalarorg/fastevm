//! Reth extension crate for FastEVM
//!
//! This crate provides extensions and utilities for the FastEVM project,
//! including consensus-related types and RPC API definitions.

pub mod rpc_api;
pub mod types;

// Re-export commonly used types
pub use rpc_api::*;
pub use types::*;
