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
mod rpc;

use clap::Parser;
// Suppress warnings for dependencies used by CLI binary
use crate::{
    consensus::{ConsensusPool, MysticetiConsensus},
    payload::MysticetiPayloadBuilderFactory,
    rpc::{ConsensusTransactionsHandler, TxpoolListener},
};

use reth_ethereum::{
    chainspec::ChainSpecProvider,
    cli::{chainspec::EthereumChainSpecParser, interface::Cli},
    node::{
        builder::{components::BasicPayloadServiceBuilder, NodeHandle},
        node::EthereumAddOns,
        EthereumNode,
    },
};
use reth_extension::{ConsensusTransactionApiServer, TxpoolListenerApiServer};
use std::sync::Arc;
use tracing::info;
// Use in cli
use bip39 as _;
use hdwallet as _;
use hex as _;
use reth_network_peers as _;
use reth_rpc_layer as _;
use secp256k1 as _;
use serde_json as _;
use sha2 as _;

/// Our custom cli args extension that adds one flag to reth default CLI.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub(crate) struct CliMysticetiArgs {
    /// CLI flag to enable the txpool extension namespace
    #[arg(long)]
    pub enable_txpool_listener: bool,
    /// Number of transactions to send in a batch
    #[arg(long)]
    pub committed_subdags_per_block: usize,
    /// Build interval in milliseconds
    #[arg(long)]
    pub block_build_interval_ms: u64,
}

/// Flow hook execution:
/// on_component_initialized
/// Exex
/// extend_rpc_modules
/// on_rpc_started
/// on_node_started:
fn main() {
    Cli::<EthereumChainSpecParser, CliMysticetiArgs>::parse()
        .run(|builder, args| async move {
            //Create a channel for sending subdag received from rpc server to BeaconConsensusEngineHandle
            // let (subdag_tx, subdag_rx) = unbounded_channel();
            let consensus_pool = Arc::new(ConsensusPool::new(args.committed_subdags_per_block));
            let mysticeti_payload_builder = BasicPayloadServiceBuilder::new(
                MysticetiPayloadBuilderFactory::new(consensus_pool.clone()),
            );
            let handle = builder
                .with_types::<EthereumNode>()
                // Configure the components of the node
                // use default ethereum components but use our custom payload builder
                .with_components(EthereumNode::components().payload(mysticeti_payload_builder))
                .with_add_ons(EthereumAddOns::default())
                .extend_rpc_modules({
                    let consensus_pool = consensus_pool.clone();
                    move |ctx| {
                        if !args.enable_txpool_listener {
                            return Ok(());
                        }

                        // here we get the configured pool.
                        let pool = ctx.pool().clone();
                        let listener = TxpoolListener::new(pool.clone());
                        let chain_spec = ctx.provider().chain_spec();
                        let consensus_handler = ConsensusTransactionsHandler::new(
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
                    let payload_builder_handle: reth_payload_builder::PayloadBuilderHandle<
                        reth_ethereum::node::EthEngineTypes,
                    > = node.payload_builder_handle.clone();
                    let engine_handle = node.add_ons_handle.beacon_engine_handle;
                    // let engine_events = node.add_ons_handle.engine_events.new_listener();
                    // use reth_ethereum::chainspec::ChainSpecProvider;
                    // node.provider.chain_spec();
                    // let transaction_pool = node.pool.clone();
                    let mut mysticeti_consensus = MysticetiConsensus::new(
                        consensus_pool,
                        node.provider,
                        payload_builder_handle,
                        engine_handle,
                        args.block_build_interval_ms,
                    );
                    node.task_executor.spawn(async move {
                        mysticeti_consensus.start().await;
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
}
