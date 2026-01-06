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

mod args;
mod consensus;
// mod coordinator;
mod payload;
mod pool;
mod rpc;
mod types;

use clap::Parser;
//use coordinator::GravityCoordinator;
use fastevm_execution::{RethBlockChainProvider, RethPipeExecLayerApi};
use reth_ethereum_engine_primitives::EthPayloadTypes;
use reth_transaction_pool::blobstore::DiskFileBlobStore;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::oneshot;
// Suppress warnings for dependencies used by CLI binary
use crate::{
    consensus::{ConsensusPool, MysticetiConsensus},
    payload::MysticetiPayloadBuilderFactory,
    pool::MysticetiPoolBuilder,
    rpc::{
        MysticetiConsensusApiServer, MysticetiConsensusHandler, RawTransactionApiServer,
        TransactionHandler,
    },
    types::TxValidatorConfig,
};
use greth::{
    gravity_storage::block_view_storage::BlockViewStorage,
    reth_pipe_exec_layer_ext_v2::{new_pipe_exec_layer_api, ExecutionArgs},
    reth_rpc_api::eth::{helpers::EthCall, RpcTypes},
};
use reth_chainspec::ChainSpec;
use reth_ethereum::{
    chainspec::EthChainSpec,
    cli::{chainspec::EthereumChainSpecParser, Cli},
    node::{
        builder::{components::BasicPayloadServiceBuilder, NodeHandle},
        node::EthereumAddOns,
        EthereumNode,
    },
};
use reth_node_api::Block;
use reth_provider::{BlockHashReader, BlockNumReader, BlockReader};
// use reth_ethereum_cli::{chainspec::EthereumChainSpecParser, interface::Cli};
use alloy_eips::BlockHashOrNumber;
use alloy_rpc_types_eth::TransactionRequest;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Extends the node with pipe execution layer functionality.
///
/// This method sets up the pipe execution layer API, creates a coordinator,
/// and initializes the pipe API for OrderedBlock injection.
///
/// Based on the pattern from gravity_bench/gravity_node.rs
///
/// Note: This implementation requires gravity-reth dependencies:
/// - greth::gravity_storage::block_view_storage::BlockViewStorage
/// - greth::reth_pipe_exec_layer_ext_v2::{ExecutionArgs, PipeExecLayerApi, new_pipe_exec_layer_api}
/// - A coordinator implementation (e.g., GravityBenchCoordinator)
async fn pipe_extend<EthApi>(
    provider: RethBlockChainProvider,
    chain_spec: Arc<ChainSpec>,
    eth_api: EthApi,
) -> eyre::Result<Arc<RethPipeExecLayerApi<EthApi>>>
where
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    info!("📦 [Gravity] Creating pipe execution layer API");

    // Get latest block information
    let latest_block_number = match provider.last_block_number() {
        Ok(num) => num,
        Err(e) => {
            warn!("❌ [Gravity] Failed to get latest block number: {}", e);
            return Err(eyre::eyre!("Failed to get latest block number: {}", e));
        }
    };

    let latest_block_hash = match provider.block_hash(latest_block_number) {
        Ok(Some(hash)) => hash,
        Ok(None) => {
            warn!("❌ [Gravity] Latest block hash not found");
            return Err(eyre::eyre!("Latest block hash not found"));
        }
        Err(e) => {
            warn!("❌ [Gravity] Failed to get latest block hash: {}", e);
            return Err(eyre::eyre!("Failed to get latest block hash: {}", e));
        }
    };

    let latest_block = match provider.block(BlockHashOrNumber::Number(latest_block_number)) {
        Ok(Some(block)) => block,
        Ok(None) => {
            info!("ℹ️ [Gravity] No blocks found, skipping setup (will sync on first block)");
            return Err(eyre::eyre!(
                "No blocks found, skipping setup (will sync on first block)"
            ));
        }
        Err(e) => {
            warn!("❌ [Gravity] Failed to get latest block: {}", e);
            return Err(eyre::eyre!("Failed to get latest block: {}", e));
        }
    };

    info!(
        "   Latest block: number={}, hash={:?}",
        latest_block_number, latest_block_hash
    );

    // Create storage wrapper
    // Note: Requires greth::gravity_storage::block_view_storage::BlockViewStorage
    let storage = BlockViewStorage::new(provider.clone());

    // Get chain_id from chain_spec
    // Note: Requires greth::reth_chainspec::ChainKind
    let chain_id = chain_spec.chain().id();
    info!("   Chain ID: {}", chain_id);

    // Create execution args channel for pipe execution layer
    // Note: Requires greth::reth_pipe_exec_layer_ext_v2::ExecutionArgs
    let (execution_args_tx, execution_args_rx) = oneshot::channel::<ExecutionArgs>();

    // Create pipe execution layer API
    // Note: Requires greth::reth_pipe_exec_layer_ext_v2::{PipeExecLayerApi, new_pipe_exec_layer_api}
    let pipeline_api = new_pipe_exec_layer_api(
        chain_spec.clone(),
        storage,
        latest_block.header().clone(),
        latest_block_hash,
        execution_args_rx,
        eth_api.clone(),
    );
    let pipeline_api_arc = Arc::new(pipeline_api);
    // Create channel for coordinator to signal block execution completion
    // (block_number, new_epoch)
    // let (block_executed_tx, _block_executed_rx) = unbounded_channel::<(u64, Option<u64>)>();

    // Create coordinator
    // Note: Requires a coordinator implementation (e.g., GravityCoordinator)

    // let coordinator = GravityCoordinator::new(
    //     pipeline_api_arc.clone(),
    //     provider.clone(),
    //     chain_id,
    //     Some(block_executed_tx),
    // );

    // Start coordinator tasks (start_execution, start_commit_vote, start_commit)
    // coordinator.run();
    // info!("✅ [Gravity] Coordinator started (execution, commit_vote, commit tasks)");

    // Send execution args
    // Note: This would send the execution args to the pipe execution layer
    // This is initial mapping of block number to block id
    // By default, we use empty mapping
    let _ = execution_args_tx.send(ExecutionArgs {
        block_number_to_block_id: std::collections::BTreeMap::new(),
    });

    // Initialize pipe API and start injection loop
    // This would require additional setup similar to gravity_node.rs
    // The injection loop would handle OrderedBlock injection triggered by coordinator
    // when block execution completes
    // let provider_clone = provider.clone();
    // let eth_api_clone = eth_api.clone();

    // Spawn background task for pipe execution layer coordination
    // Note: This is a placeholder - actual implementation would require:
    // 1. PipeExecLayerApi setup
    // 2. Coordinator implementation
    // 3. Injection loop implementation
    // task_executor.spawn(async move {
    //     info!("🔄 [Gravity] Pipe execution layer background task started");
    //     info!("   This would handle OrderedBlock injection when block execution completes");

    //     // Placeholder: In actual implementation, this would:
    //     // 1. Wait for block execution completion signals from coordinator
    //     // 2. Process OrderedBlocks from buffer
    //     // 3. Inject them into the execution pipeline

    //     // For now, just log that the task is running
    //     let mut block_executed_rx = block_executed_rx;
    //     while let Some((block_number, epoch)) = block_executed_rx.recv().await {
    //         info!(
    //             "📦 [Gravity] Block execution completed: number={}, epoch={:?}",
    //             block_number, epoch
    //         );
    //         // In actual implementation, this would trigger OrderedBlock injection
    //     }
    // });

    info!("✅ [Gravity] Pipe execution layer setup completed");
    info!("   Note: Full implementation requires gravity-reth dependencies:");
    info!("   - greth::gravity_storage::block_view_storage::BlockViewStorage");
    info!("   - greth::reth_pipe_exec_layer_ext_v2");
    // info!("   - Coordinator implementation");

    Ok(pipeline_api_arc)
}

/// Flow hook execution:
/// on_component_initialized
/// Exex
/// extend_rpc_modules
/// on_rpc_started
/// on_node_started:
fn main() -> eyre::Result<()> {
    Cli::<EthereumChainSpecParser, args::CliMysticetiArgs>::parse()
        .run(|builder, args| async move {
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

            // Create a shared state to pass eth_api from extend_rpc_modules to on_node_started
            // We'll store it as a flag and access it differently since EthApi is generic

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
                        if ctx.config().gravity.disable_pipe_execution {
                            info!("📦 [Gravity] Pipe execution disabled");
                            return Ok(());
                        }
                        // let chain_spec = ctx.provider().chain_spec();
                        // Access the EthApi instance from the registry
                        let eth_api = ctx.registry.eth_api().clone();

                        let pool = ctx.pool();
                        let mut listener = TransactionHandler::new(pool.clone(), eth_api)
                            .with_config_receiver(config_receiver);

                        // Start validator reconstruction thread
                        listener.start_txvalidator_config_listener();

                        let consensus_handler: MysticetiConsensusHandler<
                            reth_transaction_pool::Pool<
                                reth_transaction_pool::TransactionValidationTaskExecutor<
                                    reth_transaction_pool::EthTransactionValidator<
                                        _,
                                        reth_transaction_pool::EthPooledTransaction,
                                    >,
                                >,
                                reth_transaction_pool::CoinbaseTipOrdering<
                                    reth_transaction_pool::EthPooledTransaction,
                                >,
                                DiskFileBlobStore,
                            >,
                        > = MysticetiConsensusHandler::new(consensus_pool);
                        // now we merge our extension namespace into all configured transports
                        ctx.modules.merge_configured(listener.into_rpc())?;
                        ctx.modules.merge_http(consensus_handler.into_rpc())?;
                        info!("successfully extended rpc modules with txpool listener");
                        Ok(())
                    }
                })
                .on_node_started(move |node| {
                    // Access gravity arguments from node.config.gravity
                    let gravity = node.config.gravity.clone();
                    let task_executor = node.task_executor.clone();
                    let provider = node.provider.clone();
                    let eth_api = node.rpc_registry.eth_api().clone();
                    let chain_spec = node.chain_spec().clone();
                    let engine_handle = node.add_ons_handle.beacon_engine_handle;
                    node.task_executor.spawn(async move {
                        // Check for gravity.disable-pipe-execution flag
                        if !gravity.disable_pipe_execution {
                            info!("Extending pipe execution layer");
                            match pipe_extend(provider, chain_spec, eth_api.clone()).await {
                                Ok(pipeline_api) => {
                                    // Get the canonical state stream
                                    let mut mysticeti_consensus =
                                        MysticetiConsensus::new_with_pipeline_api(
                                            task_executor,
                                            consensus_pool,
                                            node.provider,
                                            rx_built_payload,
                                            engine_handle,
                                            Some(pipeline_api),
                                            args.block_interval_ms,
                                        );
                                    if let Err(e) =
                                        mysticeti_consensus.start_with_pipeline_api().await
                                    {
                                        error!("Failed to start mysticeti consensus: {:?}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to extend pipe execution layer: {:?}", e);
                                }
                            }
                        } else {
                            info!("Pipe execution layer disabled");
                            // Use helper function to avoid specifying Storage type explicitly
                            let mut mysticeti_consensus =
                                MysticetiConsensus::new_with_default_storage(
                                    task_executor,
                                    consensus_pool,
                                    provider,
                                    rx_built_payload,
                                    engine_handle,
                                    args.block_interval_ms,
                                    &eth_api,
                                );
                            if let Err(e) = mysticeti_consensus.start_with_engine_handle().await {
                                error!("Failed to start mysticeti consensus: {:?}", e);
                            }
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
