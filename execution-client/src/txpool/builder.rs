use alloy_eips::{eip7840::BlobParams, merge::EPOCH_SLOTS};
use reth_ethereum::{
    chainspec::{EthChainSpec, EthereumHardforks},
    node::{
        api::{FullNodeTypes, NodeTypes},
        builder::{components::PoolBuilder, BuilderContext},
    },
    primitives::NodePrimitives,
    provider::CanonStateSubscriptions,
    TransactionSigned,
};
use reth_node_api::TxTy;
use reth_tracing::tracing::{debug, info};
use reth_transaction_pool::{
    blobstore::DiskFileBlobStore, CoinbaseTipOrdering, EthTransactionPool, PoolConfig,
    PoolTransaction, TransactionPool, TransactionValidationTaskExecutor,
};
use std::time::SystemTime;

use crate::txpool::MysticetiPool;

/// A basic mysticeti transaction pool.
///
/// This contains various settings that can be configured and take precedence over the node's
/// config.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct MysticetiPoolBuilder {
    // TODO add options for txpool args
    pool_config: PoolConfig,
}

impl<Types, Node> PoolBuilder<Node> for MysticetiPoolBuilder
where
    Types: NodeTypes<
        ChainSpec: EthereumHardforks,
        Primitives: NodePrimitives<SignedTx = TransactionSigned>,
    >,
    Node: FullNodeTypes<Types = Types>,
{
    // type Pool = EthTransactionPool<Node::Provider, DiskFileBlobStore>;
    type Pool = MysticetiPool<Node::Provider, DiskFileBlobStore>;

    async fn build_pool(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::Pool> {
        let pool_config = ctx.pool_config();

        let blob_cache_size = if let Some(blob_cache_size) = pool_config.blob_cache_size {
            Some(blob_cache_size)
        } else {
            // get the current blob params for the current timestamp, fallback to default Cancun
            // params
            let current_timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs();
            let blob_params = ctx
                .chain_spec()
                .blob_params_at_timestamp(current_timestamp)
                .unwrap_or_else(BlobParams::cancun);

            // Derive the blob cache size from the target blob count, to auto scale it by
            // multiplying it with the slot count for 2 epochs: 384 for pectra
            Some((blob_params.target_blob_count * EPOCH_SLOTS * 2) as u32)
        };

        let blob_store =
            reth_node_builder::components::create_blob_store_with_cache(ctx, blob_cache_size)?;

        let validator = TransactionValidationTaskExecutor::eth_builder(ctx.provider().clone())
            .with_head_timestamp(ctx.head().timestamp)
            .with_max_tx_input_bytes(ctx.config().txpool.max_tx_input_bytes)
            .kzg_settings(ctx.kzg_settings()?)
            .with_local_transactions_config(pool_config.local_transactions_config.clone())
            .set_tx_fee_cap(ctx.config().rpc.rpc_tx_fee_cap)
            .with_max_tx_gas_limit(ctx.config().txpool.max_tx_gas_limit)
            .with_minimum_priority_fee(ctx.config().txpool.minimum_priority_fee)
            .with_additional_tasks(ctx.config().txpool.additional_validation_tasks)
            .build_with_tasks(ctx.task_executor().clone(), blob_store.clone());

        if validator.validator().eip4844() {
            // initializing the KZG settings can be expensive, this should be done upfront so that
            // it doesn't impact the first block or the first gossiped blob transaction, so we
            // initialize this in the background
            let kzg_settings = validator.validator().kzg_settings().clone();
            ctx.task_executor().spawn_blocking(async move {
                let _ = kzg_settings.get();
                debug!(target: "reth::cli", "Initialized KZG settings");
            });
        }
        // clone tx pool builder from reth
        // let transaction_pool = TxPoolBuilder::new(ctx)
        //     .with_validator(validator)
        //     .build_and_spawn_maintenance_task(blob_store, pool_config)?;

        let transaction_pool = reth_transaction_pool::Pool::new(
            validator,
            CoinbaseTipOrdering::default(),
            blob_store,
            pool_config.clone(),
        );

        let transaction_pool = MysticetiPool::new(
            validator,
            CoinbaseTipOrdering::default(),
            blob_store,
            pool_config.clone(),
        );

        // Spawn maintenance tasks using standalone functions
        spawn_maintenance_tasks(ctx, transaction_pool.clone(), &pool_config)?;
        info!(target: "reth::cli", "Transaction pool initialized");
        debug!(target: "reth::cli", "Spawned txpool maintenance task");

        Ok(transaction_pool)
    }
}

/// Spawn all maintenance tasks for a transaction pool (backup + main maintenance).
fn spawn_maintenance_tasks<Node, Pool>(
    ctx: &BuilderContext<Node>,
    pool: Pool,
    pool_config: &PoolConfig,
) -> eyre::Result<()>
where
    Node: FullNodeTypes,
    Pool: reth_transaction_pool::TransactionPoolExt + Clone + 'static,
    Pool::Transaction: PoolTransaction<Consensus = TxTy<Node::Types>>,
{
    spawn_local_backup_task(ctx, pool.clone())?;
    spawn_pool_maintenance_task(ctx, pool, pool_config)?;
    Ok(())
}

/// Spawn local transaction backup task if enabled.
fn spawn_local_backup_task<Node, Pool>(ctx: &BuilderContext<Node>, pool: Pool) -> eyre::Result<()>
where
    Node: FullNodeTypes,
    Pool: TransactionPool + Clone + 'static,
{
    if !ctx.config().txpool.disable_transactions_backup {
        let data_dir = ctx.config().datadir();
        let transactions_path = ctx
            .config()
            .txpool
            .transactions_backup_path
            .clone()
            .unwrap_or_else(|| data_dir.txpool_transactions());

        let transactions_backup_config =
            reth_transaction_pool::maintain::LocalTransactionBackupConfig::with_local_txs_backup(
                transactions_path,
            );

        ctx.task_executor()
            .spawn_critical_with_graceful_shutdown_signal(
                "local transactions backup task",
                |shutdown| {
                    reth_transaction_pool::maintain::backup_local_transactions_task(
                        shutdown,
                        pool,
                        transactions_backup_config,
                    )
                },
            );
    }
    Ok(())
}

/// Spawn the main maintenance task for transaction pool.
fn spawn_pool_maintenance_task<Node, Pool>(
    ctx: &BuilderContext<Node>,
    pool: Pool,
    pool_config: &PoolConfig,
) -> eyre::Result<()>
where
    Node: FullNodeTypes,
    Pool: reth_transaction_pool::TransactionPoolExt + Clone + 'static,
    Pool::Transaction: PoolTransaction<Consensus = TxTy<Node::Types>>,
{
    let chain_events = ctx.provider().canonical_state_stream();
    let client = ctx.provider().clone();

    ctx.task_executor().spawn_critical(
        "txpool maintenance task",
        reth_transaction_pool::maintain::maintain_transaction_pool_future(
            client,
            pool,
            chain_events,
            ctx.task_executor().clone(),
            reth_transaction_pool::maintain::MaintainPoolConfig {
                max_tx_lifetime: pool_config.max_queued_lifetime,
                no_local_exemptions: pool_config.local_transactions_config.no_exemptions,
                max_reload_accounts: 1000,
                ..Default::default()
            },
        ),
    );

    Ok(())
}
