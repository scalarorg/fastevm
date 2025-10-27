use super::ScalarEvmFactory;
use alloy_consensus::Header;
use alloy_eips::Decodable2718;
use alloy_evm::block::{BlockExecutionError, BlockExecutorFactory};
use alloy_primitives::{Bytes, U256};
use alloy_rpc_types_engine::ExecutionData;
use reth_ethereum::{
    chainspec::{ChainSpec, EthChainSpec, Hardforks},
    evm::{revm_spec_by_timestamp_and_block_number, EthEvmConfig},
    primitives::{
        constants::MAX_TX_GAS_LIMIT_OSAKA, SealedBlock, SealedHeader, SignedTransaction, TxTy,
    },
    Block, EthPrimitives, TransactionSigned,
};
use reth_evm::{
    eth::{spec::EthExecutorSpec, EthBlockExecutionCtx},
    execute::{BlockBuilder, Executor},
    precompiles::PrecompilesMap,
    ConfigureEngineEvm, ConfigureEvm, EvmEnv, EvmEnvFor, EvmFactory, EvmFactoryFor,
    ExecutableTxIterator, ExecutionCtxFor, FromRecoveredTx, FromTxWithEncoded,
    NextBlockEnvAttributes, TransactionEnv,
};

use reth_provider::errors::any::AnyError;
use reth_revm::{cached::CachedReads, State};
use revm::{
    context::{BlockEnv, CfgEnv},
    context_interface::block::BlobExcessGasAndPrice,
    primitives::hardfork::SpecId,
};
use std::{
    borrow::Cow,
    collections::HashSet,
    fmt::Debug,
    sync::{Arc, RwLock},
};

/// EVM config with caching layer
#[derive(Debug, Clone)]
pub struct ScalarEvmConfig<C = ChainSpec, EvmFactory = ScalarEvmFactory> {
    inner: EthEvmConfig<C, EvmFactory>,
    //cache: Arc<RwLock<CachedReads>>,
    active_accounts: Arc<RwLock<HashSet<alloy_primitives::Address>>>,
}

impl ScalarEvmConfig {
    // Creates a cached database wrapper
    // fn create_cached_db<DB>(&self, db: DB) -> reth_revm::cached::CachedReadsDbMut<'_, DB>
    // where
    //     DB: reth_revm::DatabaseRef,
    // {
    //     let mut cache = self.cache.write().unwrap();
    //     cache.as_db_mut(db)
    // }

    /// Extract addresses from transactions for cache warming
    fn update_active_accounts(&self, transactions: &[impl AsRef<TransactionSigned>]) {
        let mut accounts = self.active_accounts.write().unwrap();
        for tx in transactions {
            // Extract sender and to addresses
            // accounts.insert(sender);
            // accounts.insert(tx.to());
        }
    }
}

impl<ChainSpec, EvmFactory> ScalarEvmConfig<ChainSpec, EvmFactory> {
    /// Creates a new Ethereum EVM configuration with the given chain spec and EVM factory.
    pub fn new_with_evm_factory(chain_spec: Arc<ChainSpec>, evm_factory: EvmFactory) -> Self {
        Self {
            inner: EthEvmConfig::new_with_evm_factory(chain_spec, evm_factory),
            active_accounts: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Returns the chain spec associated with this configuration.
    pub const fn chain_spec(&self) -> &Arc<ChainSpec> {
        self.inner.chain_spec()
    }

    /// Sets the extra data for the block assembler.
    pub fn with_extra_data(mut self, extra_data: Bytes) -> Self {
        self.inner.block_assembler.extra_data = extra_data;
        self
    }
}

// impl ScalarEvmConfig {
//     /// Helper method for creating executor with state
//     fn executor_for_state<DB: reth_revm::Database>(
//         &self,
//         db: State<DB>,
//     ) -> impl reth_evm::Executor<
//         State<DB>,
//         Primitives = Self::Primitives,
//         Error = reth_execution_errors::BlockExecutionError,
//     > {
//         // Use inner's executor implementation
//         self.inner.executor(db)
//     }

//     fn batch_executor_for_state<DB: reth_revm::Database>(
//         &self,
//         db: State<DB>,
//     ) -> impl reth_evm::Executor<
//         State<DB>,
//         Primitives = Self::Primitives,
//         Error = reth_execution_errors::BlockExecutionError,
//     > {
//         self.inner.batch_executor(db)
//     }
// }

impl<ChainSpec, EvmF> ConfigureEvm for ScalarEvmConfig<ChainSpec, EvmF>
where
    ChainSpec: EthExecutorSpec + EthChainSpec<Header = Header> + Hardforks + 'static,
    EvmF: EvmFactory<
            Tx: TransactionEnv
                    + FromRecoveredTx<TransactionSigned>
                    + FromTxWithEncoded<TransactionSigned>,
            Spec = SpecId,
            Precompiles = PrecompilesMap,
        > + Clone
        + Debug
        + Send
        + Sync
        + Unpin
        + 'static,
{
    type Primitives = EthPrimitives;
    type Error = core::convert::Infallible;
    type NextBlockEnvCtx = NextBlockEnvAttributes;
    type BlockExecutorFactory =
        <EthEvmConfig<ChainSpec, EvmF> as ConfigureEvm>::BlockExecutorFactory;
    type BlockAssembler = <EthEvmConfig<ChainSpec, EvmF> as ConfigureEvm>::BlockAssembler;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        self.inner.block_executor_factory()
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        self.inner.block_assembler()
    }

    fn evm_env(&self, header: &Header) -> Result<EvmEnv, Self::Error> {
        self.inner.evm_env(header)
    }

    fn next_evm_env(
        &self,
        parent: &Header,
        attributes: &NextBlockEnvAttributes,
    ) -> Result<reth_evm::EvmEnv, Self::Error> {
        self.inner.next_evm_env(parent, attributes)
    }

    fn context_for_block<'a>(
        &self,
        block: &'a SealedBlock<Block>,
    ) -> Result<EthBlockExecutionCtx<'a>, Self::Error> {
        Ok(EthBlockExecutionCtx {
            parent_hash: block.header().parent_hash,
            parent_beacon_block_root: block.header().parent_beacon_block_root,
            ommers: &block.body().ommers,
            withdrawals: block.body().withdrawals.as_ref().map(Cow::Borrowed),
        })
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader,
        attributes: Self::NextBlockEnvCtx,
    ) -> Result<EthBlockExecutionCtx<'_>, Self::Error> {
        self.inner.context_for_next_block(parent, attributes)
    }

    fn evm_factory(&self) -> &EvmFactoryFor<Self> {
        self.inner.evm_factory()
    }

    // fn evm_with_env<DB: reth_revm::Database>(
    //     &self,
    //     db: DB,
    //     evm_env: reth_evm::EvmEnv,
    // ) -> reth_evm::EvmFor<Self, DB> {
    //     self.inner.evm_with_env(db, evm_env)
    // }

    // fn evm_for_block<DB: reth_revm::Database>(
    //     &self,
    //     db: DB,
    //     header: &Header,
    // ) -> Result<reth_evm::EvmFor<Self, DB>, Self::Error> {
    //     self.inner.evm_for_block(db, header)
    // }

    // fn evm_with_env_and_inspector<DB, I>(
    //     &self,
    //     db: DB,
    //     evm_env: reth_evm::EvmEnv,
    //     inspector: I,
    // ) -> reth_evm::EvmFor<Self, DB, I>
    // where
    //     DB: reth_revm::Database,
    //     I: reth_evm::InspectorFor<Self, DB>,
    // {
    //     self.inner
    //         .evm_with_env_and_inspector(db, evm_env, inspector)
    // }

    // fn create_executor<'a, DB, I>(
    //     &'a self,
    //     evm: reth_evm::EvmFor<Self, &'a mut State<DB>, I>,
    //     ctx: <Self::BlockExecutorFactory as alloy_evm::block::BlockExecutorFactory>::ExecutionCtx<
    //         'a,
    //     >,
    // ) -> impl alloy_evm::block::BlockExecutorFor<'a, Self::BlockExecutorFactory, DB, I>
    // where
    //     DB: reth_revm::Database + 'a,
    //     I: reth_evm::InspectorFor<Self, &'a mut State<DB>> + 'a,
    // {
    //     self.inner.create_executor(evm, ctx)
    // }

    // fn executor_for_block<'a, DB: reth_revm::Database>(
    //     &'a self,
    //     db: &'a mut State<DB>,
    //     block: &'a SealedBlock<<Self::Primitives as NodePrimitives>::Block>,
    // ) -> Result<
    //     impl alloy_evm::block::BlockExecutorFor<'a, Self::BlockExecutorFactory, DB>,
    //     Self::Error,
    // > {
    //     // Wrap db with cached reads
    //     let cached_db = self.create_cached_db(db.as_ref());

    //     // Continue with normal flow
    //     self.inner.executor_for_block(db, block)
    // }

    // fn create_block_builder<'a, DB, I>(
    //     &'a self,
    //     evm: reth_evm::EvmFor<Self, &'a mut State<DB>, I>,
    //     parent: &'a SealedHeader<HeaderTy<Self::Primitives>>,
    //     ctx: <Self::BlockExecutorFactory as BlockExecutorFactory>::ExecutionCtx<'a>,
    // ) -> impl BlockBuilder<Primitives = Self::Primitives>
    // where
    //     DB: reth_revm::Database,
    //     I: reth_evm::InspectorFor<Self, &'a mut State<DB>> + 'a,
    // {
    //     self.inner.create_block_builder(evm, parent, ctx)
    // }

    // fn builder_for_next_block<'a, DB: reth_revm::Database>(
    //     &'a self,
    //     db: &'a mut State<DB>,
    //     parent: &'a SealedHeader<<Self::Primitives as NodePrimitives>::BlockHeader>,
    //     attributes: Self::NextBlockEnvCtx,
    // ) -> Result<impl BlockBuilder<Primitives = Self::Primitives>, Self::Error> {
    //     self.inner.builder_for_next_block(db, parent, attributes)
    // }
    // //Todo: use db reference instead of state
    // #[auto_impl::auto_impl(keep_default_for(&, Arc))]
    // fn executor<DB: reth_revm::Database>(
    //     &self,
    //     db: DB,
    // ) -> impl Executor<DB, Primitives = Self::Primitives, Error = BlockExecutionError> {
    //     // Use cached database for reduced I/O
    //     // let cached_db = self.create_cached_db(db.as_ref());

    //     // let db = State::builder()
    //     //     .with_database(cached_db)
    //     //     .with_bundle_update()
    //     //     .without_state_clear() // KEY: Keep state between blocks!
    //     //     .build();

    //     // self.inner.executor_for_state(db)
    //     self.inner.executor(db)
    // }

    // #[auto_impl::auto_impl(keep_default_for(&, Arc))]
    // fn batch_executor<DB: reth_revm::Database>(
    //     &self,
    //     db: DB,
    // ) -> impl Executor<DB, Primitives = Self::Primitives, Error = BlockExecutionError> {
    //     // Use cached database for reduced I/O in batch execution
    //     // let cached_db = self.create_cached_db(db.as_ref());

    //     // let db = State::builder()
    //     //     .with_database(cached_db)
    //     //     .with_bundle_update()
    //     //     .without_state_clear() // Keep bundle state between blocks
    //     //     .build();

    //     // self.inner.batch_executor_for_state(db)
    //     self.inner.batch_executor(db)
    // }
}

impl<ChainSpec, EvmF> ConfigureEngineEvm<ExecutionData> for ScalarEvmConfig<ChainSpec, EvmF>
where
    ChainSpec: EthExecutorSpec + EthChainSpec<Header = Header> + Hardforks + 'static,
    EvmF: EvmFactory<
            Tx: TransactionEnv
                    + FromRecoveredTx<TransactionSigned>
                    + FromTxWithEncoded<TransactionSigned>,
            Spec = SpecId,
            Precompiles = PrecompilesMap,
        > + Clone
        + Debug
        + Send
        + Sync
        + Unpin
        + 'static,
{
    fn evm_env_for_payload(&self, payload: &ExecutionData) -> EvmEnvFor<Self> {
        let timestamp = payload.payload.timestamp();
        let block_number = payload.payload.block_number();

        let blob_params = self.chain_spec().blob_params_at_timestamp(timestamp);
        let spec =
            revm_spec_by_timestamp_and_block_number(self.chain_spec(), timestamp, block_number);

        // configure evm env based on parent block
        let mut cfg_env = CfgEnv::new()
            .with_chain_id(self.chain_spec().chain().id())
            .with_spec(spec);

        if let Some(blob_params) = &blob_params {
            cfg_env.set_max_blobs_per_tx(blob_params.max_blobs_per_tx);
        }

        if self.chain_spec().is_osaka_active_at_timestamp(timestamp) {
            cfg_env.tx_gas_limit_cap = Some(MAX_TX_GAS_LIMIT_OSAKA);
        }

        // derive the EIP-4844 blob fees from the header's `excess_blob_gas` and the current
        // blobparams
        let blob_excess_gas_and_price =
            payload
                .payload
                .excess_blob_gas()
                .zip(blob_params)
                .map(|(excess_blob_gas, params)| {
                    let blob_gasprice = params.calc_blob_fee(excess_blob_gas);
                    BlobExcessGasAndPrice {
                        excess_blob_gas,
                        blob_gasprice,
                    }
                });

        let block_env = BlockEnv {
            number: U256::from(block_number),
            beneficiary: payload.payload.fee_recipient(),
            timestamp: U256::from(timestamp),
            difficulty: if spec >= SpecId::MERGE {
                U256::ZERO
            } else {
                payload.payload.as_v1().prev_randao.into()
            },
            prevrandao: (spec >= SpecId::MERGE).then(|| payload.payload.as_v1().prev_randao),
            gas_limit: payload.payload.gas_limit(),
            basefee: payload.payload.saturated_base_fee_per_gas(),
            blob_excess_gas_and_price,
        };

        EvmEnv { cfg_env, block_env }
    }

    fn context_for_payload<'a>(&self, payload: &'a ExecutionData) -> ExecutionCtxFor<'a, Self> {
        EthBlockExecutionCtx {
            parent_hash: payload.parent_hash(),
            parent_beacon_block_root: payload.sidecar.parent_beacon_block_root(),
            ommers: &[],
            withdrawals: payload
                .payload
                .withdrawals()
                .map(|w| Cow::Owned(w.clone().into())),
        }
    }

    fn tx_iterator_for_payload(&self, payload: &ExecutionData) -> impl ExecutableTxIterator<Self> {
        payload
            .payload
            .transactions()
            .clone()
            .into_iter()
            .map(|tx| {
                let tx = TxTy::<Self::Primitives>::decode_2718_exact(tx.as_ref())
                    .map_err(AnyError::new)?;
                let signer = tx.try_recover().map_err(AnyError::new)?;
                Ok::<_, AnyError>(tx.with_signer(signer))
            })
    }
}
