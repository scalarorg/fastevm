#![warn(unused_crate_dependencies)]

mod engine_api;
mod validator;

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Parser;
use consensus_core::CommitConsumer;
use engine_api::EngineApiConfig;
use mysten_metrics::RegistryService;
use prometheus::Registry;
use tokio::sync::mpsc;
use tracing::{info, Level};
use tracing_subscriber;
use validator::ValidatorNode;

use crate::engine_api::ExecutionClient;

#[derive(Parser, Debug)]
#[command(name = "consensus-client")]
#[command(about = "FastEVM Consensus Client - Mysticeti Engine API Bridge")]
struct Args {
    /// Execution client URL
    #[arg(short, long, default_value = "http://127.0.0.1:8551")]
    execution_url: String,

    /// Jwt secret in hex
    #[arg(short, long, default_value = "")]
    jwt_secret: String,

    /// Polling interval in milliseconds
    #[arg(short, long, default_value = "1000")]
    poll_interval: u64,

    /// Maximum retries for failed requests
    #[arg(short, long, default_value = "3")]
    max_retries: u32,

    /// Request timeout in seconds
    #[arg(short, long, default_value = "30")]
    timeout: u64,

    #[arg(long, value_name = "FEE_RECIPIENT")]
    fee_recipient: String,

    /// The working directory where the validator node will store its data.
    #[clap(long, value_name = "DIR", default_value = "consensus-client")]
    working_directory: PathBuf,

    /// Comma-separated list of peer addresses (e.g., "172.20.0.11:26657,172.20.0.12:26657")
    #[clap(long, value_name = "ADDRESSES")]
    peer_addresses: Option<String>,

    /// Node ID for multi-node setups
    #[arg(short, long, value_name = "INDEX", default_value = "0")]
    node_index: u32,

    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    let log_level = match args.log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    // let filter = EnvFilter::builder()
    //     .with_default_directive(log_level.into())
    //     .from_env_lossy();

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    info!(
        "Starting FastEVM Consensus Client (Node {}, Execution URL: {}, Working Directory: {}, Fee Recipient: {})",
        args.node_index,
        args.execution_url,
        args.working_directory.display(),
        args.fee_recipient,
    );
    // Create validator node
    let mut validator = ValidatorNode::new(args.node_index, args.working_directory.clone());

    // Create committee and keypairs - use Docker configuration if peer addresses are provided
    let committee_size = 4; // We'll create a 4-node committee even for single node
    let (committee, keypairs) = if args.peer_addresses.is_some() {
        info!(
            "Using Docker network configuration with peer addresses: {:?}",
            args.peer_addresses
        );
        consensus_config::docker_committee_and_keys(0, vec![1; committee_size])
    } else {
        info!("Using local network configuration");
        consensus_config::local_committee_and_keys(0, vec![1; committee_size])
    };

    // Create metrics registry
    let registry_service = RegistryService::new(Registry::new());
    // Create channels for exchange payload between engine client and consensus process
    let (payload_tx, payload_rx) = mpsc::unbounded_channel();
    // Create channels for exchange block between engine client and consensus process
    // Create commit consumer
    let (commit_consumer, commit_receiver, block_receiver) = CommitConsumer::new(0);
    // Start the validator node
    validator
        .start(
            committee,
            keypairs,
            registry_service,
            commit_consumer,
            payload_rx,
        )
        .await
        .map_err(|e| anyhow!("Failed to start validator node: {}", e))?;

    // Create configuration
    let config = EngineApiConfig {
        execution_url: args.execution_url,
        jwt_secret: args.jwt_secret,
        fee_recipient: args.fee_recipient,
        poll_interval_ms: args.poll_interval,
    };

    // Create and start the Engine API client
    let mut client = ExecutionClient::new(args.node_index, config, payload_tx)?;

    info!("Consensus client starting...");

    // Start the main client loop
    tokio::spawn(async move {
        client.start(commit_receiver, block_receiver).await
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_client_creation() {
        let config = EngineApiConfig::default();
        let (payload_tx, payload_rx) = mpsc::unbounded_channel();
        let client = ExecutionClient::new(0, config, payload_tx).unwrap();

        let state = client.get_forcechoice_state().await;
    }
}
