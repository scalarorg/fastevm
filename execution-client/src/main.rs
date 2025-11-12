//!
//! Run with
//!
//! ```sh
//! cargo run -p execution-client -- node
//! ```
//!
//! This launches a regular reth node with transaction listener and custom RPC server.

#![warn(unused_crate_dependencies)]
// alloy_consensus is used in transaction_listener.rs
use alloy_consensus as _;
mod consensus;
mod payload;
mod pool;
mod rpc;

mod types;
use clap::Parser;
use reth_ethereum_engine_primitives::EthPayloadTypes;
use reth_transaction_pool::blobstore::DiskFileBlobStore;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::oneshot;
// Suppress warnings for dependencies used by CLI binary
use crate::{
    consensus::{ConsensusPool, MysticetiConsensus},
    payload::MysticetiPayloadBuilderFactory,
    pool::MysticetiPoolBuilder,
    rpc::{MysticetiConsensusHandler, TransactionHandler},
    types::TxValidatorConfig,
};
use reth_ethereum::{
    chainspec::ChainSpecProvider,
    cli::{chainspec::EthereumChainSpecParser, Cli},
    node::{
        builder::{components::BasicPayloadServiceBuilder, NodeBuilder, NodeHandle},
        node::EthereumAddOns,
        EthereumNode,
    },
};
// use reth_ethereum_cli::{chainspec::EthereumChainSpecParser, interface::Cli};
use reth_extension::{MysticetiConsensusApiServer, MysticetiTransactionApiServer};
use std::sync::Arc;
use tracing::{error, info};
// Use in cli
use bip39 as _;
use hdwallet as _;
use hex as _;
use reth_network_peers as _;
use reth_rpc_layer as _;
use secp256k1::{self as _};
use serde_json as _;
use sha2 as _;

/// Our custom cli args extension that adds one flag to reth default CLI.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub(crate) struct CliMysticetiArgs {
    /// CLI flag to enable the txpool extension namespace
    #[arg(long)]
    pub enable_tx_subscription: bool,
    /// Number of transactions to send in a batch
    #[arg(long)]
    pub committed_subdags_per_block: usize,
    /// Build interval in milliseconds
    #[arg(long)]
    pub block_build_interval_ms: u64,
    /// Maximum number of accounts to reload during pool maintenance
    #[arg(long, default_value = "500")]
    pub max_reload_accounts: u64,
}

/// Flow hook execution:
/// on_component_initialized
/// Exex
/// extend_rpc_modules
/// on_rpc_started
/// on_node_started:
fn main() -> eyre::Result<()> {
    Cli::<EthereumChainSpecParser, CliMysticetiArgs>::parse()
        .run(|builder, args| async move {
            // Extract config and create custom database
            // let config = builder.config();
            // let datadir = config.datadir();
            // let db_path = datadir.db();
            // info!(path = ?db_path, "Creating custom database");
            // let db_args = DatabaseArguments::from(&config.db);
            // let custom_database = Arc::new(init_db(db_path, db_args)?);

            // // Create task executor and rebuild builder with custom database
            // let task_executor = builder.task_executor().clone();
            // let config = config.clone();

            // // Build node from scratch with custom database (cleaner approach)
            // let builder = NodeBuilder::new(config)
            //     .with_database(custom_database)
            //     .with_launch_context(task_executor);

            // Create a channel for sending built payload to mysticeti consensus
            let (tx_built_payload, rx_built_payload) = unbounded_channel();

            // Create oneshot channel for validator configuration
            let (config_sender, config_receiver) =
                oneshot::channel::<TxValidatorConfig<_, DiskFileBlobStore>>();

            let consensus_pool = Arc::new(ConsensusPool::new(args.committed_subdags_per_block));
            let mysticeti_payload_builder = BasicPayloadServiceBuilder::new(
                MysticetiPayloadBuilderFactory::<_, EthPayloadTypes>::new(
                    consensus_pool.clone(),
                    tx_built_payload,
                ),
            );

            let handle = builder
                .with_types::<EthereumNode>()
                // Configure the components of the node
                // use default ethereum components but use our custom payload builder
                .with_components(
                    EthereumNode::components()
                        .payload(mysticeti_payload_builder)
                        .pool(
                            MysticetiPoolBuilder::<_, DiskFileBlobStore>::default()
                                .with_config_sender(config_sender),
                        ),
                )
                .with_add_ons(EthereumAddOns::default())
                .extend_rpc_modules({
                    let consensus_pool = consensus_pool.clone();
                    move |ctx| {
                        if !args.enable_tx_subscription {
                            return Ok(());
                        }
                        let chain_spec = ctx.provider().chain_spec();
                        // Access the EthApi instance from the registry
                        let eth_api = ctx.registry.eth_api().clone();

                        let pool = ctx.pool();
                        let mut listener = TransactionHandler::new(pool.clone(), eth_api)
                            .with_config_receiver(config_receiver);

                        // Start validator reconstruction thread
                        listener.start_txvalidator_config_listener();

                        let consensus_handler = MysticetiConsensusHandler::new(
                            consensus_pool.clone(),
                            pool.clone(),
                            chain_spec,
                        );
                        // now we merge our extension namespace into all configured transports
                        ctx.modules.merge_configured(listener.into_rpc())?;
                        ctx.modules.merge_http(consensus_handler.into_rpc())?;
                        info!("successfully extended rpc modules with txpool listener");
                        Ok(())
                    }
                })
                .on_node_started(move |node| {
                    // let payload_builder_handle: reth_payload_builder::PayloadBuilderHandle<
                    //     reth_ethereum::node::EthEngineTypes,
                    // > = node.payload_builder_handle.clone();
                    let engine_handle = node.add_ons_handle.beacon_engine_handle;
                    // Get the canonical state stream
                    let mut mysticeti_consensus = MysticetiConsensus::new(
                        consensus_pool,
                        node.provider,
                        //payload_builder_handle,
                        rx_built_payload,
                        engine_handle,
                        args.block_build_interval_ms,
                    );
                    node.task_executor.spawn(async move {
                        if let Err(e) = mysticeti_consensus.start().await {
                            error!("Failed to start mysticeti consensus: {:?}", e);
                        }
                    });

                    Ok(())
                })
                .launch()
                .await?;
            let NodeHandle {
                node: _,
                node_exit_future,
            } = handle;

            // create a new subscription to pending transactions
            // let mut pending_transactions = node.pool.new_pending_pool_transactions_listener();
            // start_transaction_listener(node.clone());

            info!("FastEVM execution client started with transaction listener and RPC server");

            // Wait for node exit
            node_exit_future.await
            // handle.wait_for_node_exit().await
        })
        .unwrap();

    Ok(())
}
