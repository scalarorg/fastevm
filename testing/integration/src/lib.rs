//! Integration testing utilities for FastEVM
//!
//! This crate provides utility functions and test helpers for integration testing
//! of the FastEVM consensus and execution clients.
//!
pub mod address;
pub mod rpc;
pub mod transactions;

// Re-export block scanning functionality
pub mod block_scan;
pub mod block_scanner_cli;
