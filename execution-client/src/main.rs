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
        builder::{components::BasicPayloadServiceBuilder, NodeHandle},
        node::EthereumAddOns,
        EthereumNode,
    },
};

mod payload;
mod rpc;
mod transaction_listener;

use crate::{
    payload::MysticetiPayloadBuilderFactory,
    rpc::{CliTxpoolListener, ConsensusTransactionsHandler, TxpoolListener},
};
use reth_extension::{ConsensusTransactionApiServer, TxpoolListenerApiServer};
use tracing::info;

fn main() {
    Cli::<EthereumChainSpecParser, CliTxpoolListener>::parse()
        .run(|builder, args| async move {
            let mysticeti_payload_builder =
                BasicPayloadServiceBuilder::<MysticetiPayloadBuilderFactory>::default();

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
                    let task_executor = ctx.node().task_executor.clone();
                    let listener = TxpoolListener::new(task_executor.clone(), pool.clone());
                    let consensus_handler = ConsensusTransactionsHandler::new(task_executor, pool);
                    // now we merge our extension namespace into all configured transports
                    ctx.modules.merge_configured(listener.into_rpc())?;
                    ctx.modules.merge_http(consensus_handler.into_rpc())?;
                    info!("txpool listener enabled");

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
