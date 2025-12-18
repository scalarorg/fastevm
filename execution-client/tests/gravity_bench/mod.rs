//! Gravity Bench Integration Module
//!
//! This module provides integration with [gravity_bench](https://github.com/Galxe/gravity_bench.git)
//! for benchmarking and stress-testing the FastEVM execution client.
//!
//! # Usage
//!
//! ```rust,ignore
//! use gravity_bench::{BenchConfig, BenchRunner};
//!
//! let config = BenchConfig::builder()
//!     .rpc_url("http://localhost:8545")
//!     .target_tps(1000)
//!     .duration_secs(60)
//!     .build();
//!
//! let runner = BenchRunner::new(config);
//! runner.run().await?;
//! ```

// Re-export gravity_bench crate when available
#[cfg(feature = "bench")]
pub use ::gravity_bench::*;

// Configuration module
pub mod config;

// Test runner utilities
pub mod runner;

pub use config::*;
pub use runner::*;
