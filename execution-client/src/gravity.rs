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
mod payload;
mod pool;
mod rpc;
mod types;
mod coordinator;

use clap::Parser;
use coordinator::GravityCoordinator;
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
use reth_ethereum::{
    chainspec::{ChainSpecProvider, EthChainSpec},
    cli::{chainspec::EthereumChainSpecParser, Cli},
    node::{
        builder::{components::BasicPayloadServiceBuilder, NodeHandle, FullNodeFor},
        node::EthereumAddOns,
        EthereumNode,
    },
};
use reth_node_api::Block;
use reth_provider::{BlockNumReader, BlockReader, BlockHashReader};
use greth::{
    gravity_storage::block_view_storage::BlockViewStorage,
    reth_pipe_exec_layer_ext_v2::{ExecutionArgs, new_pipe_exec_layer_api}, 
    reth_rpc_api::eth::{helpers::EthCall, RpcTypes}
};
// use reth_ethereum_cli::{chainspec::EthereumChainSpecParser, interface::Cli};
use alloy_eips::BlockHashOrNumber;
use alloy_rpc_types_eth::TransactionRequest;
use std::sync::Arc;
use tracing::{error, info, warn};

// Use in cli
use bip39 as _;
use hdwallet as _;
use hex as _;
use reth_network_peers as _;
use reth_rpc_layer as _;
use secp256k1::{self as _};
use serde_json as _;
use sha2 as _;

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
fn pipe_extend<EthApi>(node: &FullNodeFor<EthereumNode>, eth_api: EthApi) -> eyre::Result<()>
where
    EthApi: EthCall + Clone + Send + Sync + 'static,
    EthApi::NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>,
{
    info!("📦 [Gravity] Creating pipe execution layer API");

    // Get required components from node
    let provider = node.provider();
    let chain_spec = node.chain_spec();

    // Get latest block information
    let latest_block_number = match provider.last_block_number() {
        Ok(num) => num,
        Err(e) => {
            warn!("❌ [Gravity] Failed to get latest block number: {}", e);
            return Ok(());
        }
    };

    let latest_block_hash = match provider.block_hash(latest_block_number) {
        Ok(Some(hash)) => hash,
        Ok(None) => {
            warn!("❌ [Gravity] Latest block hash not found");
            return Ok(());
        }
        Err(e) => {
            warn!("❌ [Gravity] Failed to get latest block hash: {}", e);
            return Ok(());
        }
    };

    let latest_block = match provider.block(BlockHashOrNumber::Number(latest_block_number)) {
        Ok(Some(block)) => block,
        Ok(None) => {
            info!("ℹ️ [Gravity] No blocks found, skipping setup (will sync on first block)");
            return Ok(());
        }
        Err(e) => {
            warn!("❌ [Gravity] Failed to get latest block: {}", e);
            return Ok(());
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

    // Create channel for coordinator to signal block execution completion
    // (block_number, new_epoch)
    let (block_executed_tx, block_executed_rx) = unbounded_channel::<(u64, Option<u64>)>();

    // Create coordinator
    // Note: Requires a coordinator implementation (e.g., GravityCoordinator)
    let pipeline_api_arc = Arc::new(pipeline_api);
    let coordinator = GravityCoordinator::new(
        pipeline_api_arc.clone(),
        provider.clone(),
        chain_id,
        Some(block_executed_tx),
    );

    // Start coordinator tasks (start_execution, start_commit_vote, start_commit)
    coordinator.run();
    // info!("✅ [Gravity] Coordinator started (execution, commit_vote, commit tasks)");

    // Send execution args
    // Note: This would send the execution args to the pipe execution layer
    let _ = execution_args_tx.send(ExecutionArgs {
        block_number_to_block_id: std::collections::BTreeMap::new(),
    });

    // Initialize pipe API and start injection loop
    // This would require additional setup similar to gravity_node.rs
    // The injection loop would handle OrderedBlock injection triggered by coordinator
    // when block execution completes
    let task_executor = &node.task_executor;
    let provider_clone = provider.clone();
    let eth_api_clone = eth_api.clone();
    
    // Spawn background task for pipe execution layer coordination
    // Note: This is a placeholder - actual implementation would require:
    // 1. PipeExecLayerApi setup
    // 2. Coordinator implementation
    // 3. Injection loop implementation
    task_executor.spawn(async move {
        info!("🔄 [Gravity] Pipe execution layer background task started");
        info!("   This would handle OrderedBlock injection when block execution completes");
        
        // Placeholder: In actual implementation, this would:
        // 1. Wait for block execution completion signals from coordinator
        // 2. Process OrderedBlocks from buffer
        // 3. Inject them into the execution pipeline
        
        // For now, just log that the task is running
        let mut block_executed_rx = block_executed_rx;
        while let Some((block_number, epoch)) = block_executed_rx.recv().await {
            info!(
                "📦 [Gravity] Block execution completed: number={}, epoch={:?}",
                block_number, epoch
            );
            // In actual implementation, this would trigger OrderedBlock injection
        }
    });

    info!("✅ [Gravity] Pipe execution layer setup completed");
    info!("   Note: Full implementation requires gravity-reth dependencies:");
    info!("   - greth::gravity_storage::block_view_storage::BlockViewStorage");
    info!("   - greth::reth_pipe_exec_layer_ext_v2");
    info!("   - Coordinator implementation");

    Ok(())
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
            use std::sync::atomic::{AtomicBool, Ordering};
            let pipe_extend_enabled = Arc::new(AtomicBool::new(false));

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
                    let pipe_extend_enabled = pipe_extend_enabled.clone();
                    move |ctx| {
                        if !args.enable_tx_subscription {
                            return Ok(());
                        }
                        let chain_spec = ctx.provider().chain_spec();
                        // Access the EthApi instance from the registry
                        let eth_api = ctx.registry.eth_api().clone();
                        
                        // Mark that pipe_extend can be called (eth_api is available)
                        pipe_extend_enabled.store(true, Ordering::Relaxed);

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
                    // Note: Since we can't easily pass eth_api from extend_rpc_modules to on_node_started
                    // due to type constraints, we'll skip the pipe_extend call for now.
                    // In a full implementation with gravity-reth dependencies, you would:
                    // 1. Access rpc_registry directly from the node (if available)
                    // 2. Or restructure to pass eth_api through a different mechanism
                    // 3. Or make pipe_extend work without eth_api initially
                    
                    // For now, we'll call pipe_extend without eth_api
                    // This will set up the structure but won't fully initialize the pipe execution layer
                    // The actual pipe execution layer setup requires gravity-reth dependencies
                    if pipe_extend_enabled.load(Ordering::Relaxed) {
                        // Note: pipe_extend requires eth_api, but we can't easily pass it here
                        // In a full implementation, you would access it from node.rpc_registry
                        // or restructure the code to pass it through
                        info!("ℹ️ [Gravity] Pipe execution layer setup skipped - requires eth_api access");
                        info!("   Full implementation requires gravity-reth dependencies and eth_api access");
                    }

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
