//!
//! Run with
//!
//! ```sh
//! cargo run -p execution-client -- node
//! ```
//!
//! This launches a regular reth node with transaction listener and custom RPC server.

#![warn(unused_crate_dependencies)]
use clap::Parser;
use reth_ethereum::{
    cli::{chainspec::EthereumChainSpecParser, interface::Cli},
    node::{
        builder::{
            components::BasicPayloadServiceBuilder,
            rpc::{RpcAddOns, RpcHandle},
            EngineNodeLauncher, FullNode, NodeHandle,
        },
        node::EthereumAddOns,
        EthereumNode,
    },
};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::Mutex;
mod consensus;
mod payload;
mod rpc;
mod transaction_listener;

use crate::{
    consensus::MysticetiConsensus,
    payload::MysticetiPayloadBuilderFactory,
    rpc::{CliTxpoolListener, ConsensusTransactionsHandler, TxpoolListener},
};
use reth_extension::{ConsensusTransactionApiServer, TxpoolListenerApiServer};
use tracing::info;

/// Flow hook execution:
/// on_component_initialized
/// Exex
/// extend_rpc_modules
/// on_rpc_started
/// on_node_started:
fn main() {
    Cli::<EthereumChainSpecParser, CliTxpoolListener>::parse()
        .run(|builder, args| async move {
            //Create a channel for sending subdag received from rpc server to BeaconConsensusEngineHandle
            let (subdag_tx, subdag_rx) = unbounded_channel();
            let subdag_queue = Arc::new(Mutex::new(VecDeque::new()));
            let mysticeti_payload_builder =
                BasicPayloadServiceBuilder::<MysticetiPayloadBuilderFactory>::new(
                    MysticetiPayloadBuilderFactory::new(subdag_queue.clone()),
                );

            let NodeHandle {
                node: _,
                node_exit_future,
            } = builder
                .with_types::<EthereumNode>()
                // Configure the components of the node
                // use default ethereum components but use our custom payload builder
                .with_components(EthereumNode::components().payload(mysticeti_payload_builder))
                .with_add_ons(EthereumAddOns::default())
                .extend_rpc_modules(move |ctx| {
                    if !args.enable_txpool_listener {
                        return Ok(());
                    }

                    // here we get the configured pool.
                    let pool = ctx.pool().clone();
                    //FullNode
                    let node_adapter = ctx.node();
                    let task_executor = node_adapter.task_executor.clone();
                    let listener = TxpoolListener::new(task_executor.clone(), pool.clone());
                    let consensus_handler = ConsensusTransactionsHandler::new(subdag_tx, pool);
                    // now we merge our extension namespace into all configured transports
                    ctx.modules.merge_configured(listener.into_rpc())?;
                    ctx.modules.merge_http(consensus_handler.into_rpc())?;
                    info!("txpool listener enabled");

                    Ok(())
                })
                .on_node_started(move |node| {
                    let payload_builder_handle = node.payload_builder_handle.clone();
                    let engine_handle = node.add_ons_handle.beacon_engine_handle;
                    let mut mysticeti_consensus =
                        MysticetiConsensus::new(subdag_rx, payload_builder_handle, engine_handle);
                    node.task_executor.spawn(async move {
                        mysticeti_consensus.start().await;
                    });

                    Ok(())
                })
                .launch()
                .await?;
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
