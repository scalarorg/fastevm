use crate::evm::{alloy::ScalarBlockExecutorFactory, ScalarBlockAssembler};

use super::ScalarEvmFactory;
use alloy_consensus::Header;
use alloy_eips::Decodable2718;
use alloy_primitives::{Bytes, U256};
use alloy_rpc_types_engine::ExecutionData;
use reth_ethereum::{
    chainspec::{ChainSpec, EthChainSpec, Hardforks},
    evm::{revm_spec_by_timestamp_and_block_number, EthEvmConfig, RethReceiptBuilder},
    primitives::{
        constants::MAX_TX_GAS_LIMIT_OSAKA, HeaderTy, SealedBlock, SealedHeader, SignedTransaction,
        TxTy,
    },
    Block, EthPrimitives, TransactionSigned,
};

use reth_evm::{
    block::{BlockExecutionError, BlockExecutorFactory, BlockExecutorFor},
    eth::{spec::EthExecutorSpec, EthBlockExecutionCtx},
    execute::{BasicBlockBuilder, BasicBlockExecutor, BlockBuilder, Executor},
    precompiles::PrecompilesMap,
    ConfigureEngineEvm, ConfigureEvm, Database, EvmEnv, EvmEnvFor, EvmFactory, EvmFactoryFor,
    EvmFor, ExecutableTxIterator, ExecutionCtxFor, FromRecoveredTx, FromTxWithEncoded,
    InspectorFor, NextBlockEnvAttributes, TransactionEnv,
};

use reth_node_api::NodePrimitives;
use reth_provider::errors::any::AnyError;
use reth_revm::State;
use revm::{
    context::{BlockEnv, CfgEnv},
    context_interface::block::BlobExcessGasAndPrice,
    primitives::hardfork::SpecId,
};
use std::{
    borrow::Cow,
    collections::HashSet,
    convert::Infallible,
    fmt::Debug,
    sync::{Arc, RwLock},
};

/// EVM config with caching layer
#[derive(Debug, Clone)]
pub struct ScalarEvmConfig<C = ChainSpec, EvmFactory = ScalarEvmFactory> {
    inner: EthEvmConfig<C, EvmFactory>,
    pub(super) executor_factory: ScalarBlockExecutorFactory<RethReceiptBuilder, Arc<C>, EvmFactory>,
    pub(super) block_assembler: ScalarBlockAssembler<C>,
    //cache: Arc<RwLock<CachedReads>>,
    pub(super) active_accounts: Arc<RwLock<HashSet<alloy_primitives::Address>>>,
}
impl<ChainSpec> ScalarEvmConfig<ChainSpec> {
    /// Creates a new Ethereum EVM configuration with the given chain spec.
    pub fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self::ethereum(chain_spec)
    }

    /// Creates a new Ethereum EVM configuration.
    pub fn ethereum(chain_spec: Arc<ChainSpec>) -> Self {
        Self::new_with_evm_factory(chain_spec, ScalarEvmFactory::default())
    }
}

impl<ChainSpec, EvmFactory: Clone> ScalarEvmConfig<ChainSpec, EvmFactory> {
    /// Creates a new Ethereum EVM configuration with the given chain spec and EVM factory.
    pub fn new_with_evm_factory(chain_spec: Arc<ChainSpec>, evm_factory: EvmFactory) -> Self {
        Self {
            inner: EthEvmConfig::new_with_evm_factory(chain_spec.clone(), evm_factory.clone()),
            block_assembler: ScalarBlockAssembler::new(chain_spec.clone()),
            executor_factory: ScalarBlockExecutorFactory::new(
                RethReceiptBuilder::default(),
                chain_spec,
                evm_factory,
            ),
            active_accounts: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Returns the chain spec associated with this configuration.
    pub const fn chain_spec(&self) -> &Arc<ChainSpec> {
        self.executor_factory.spec()
    }

    /// Sets the extra data for the block assembler.
    pub fn with_extra_data(self, extra_data: Bytes) -> Self {
        let Self {
            inner,
            block_assembler,
            executor_factory,
            active_accounts,
        } = self;
        let block_assembler = block_assembler.with_extra_data(extra_data);
        Self {
            inner,
            block_assembler,
            executor_factory,
            active_accounts,
        }
    }
}

/// Scalar EVM config with caching layer
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
    type Error = Infallible;
    type NextBlockEnvCtx = NextBlockEnvAttributes;
    type BlockExecutorFactory =
        ScalarBlockExecutorFactory<RethReceiptBuilder, Arc<ChainSpec>, EvmF>;
    type BlockAssembler = ScalarBlockAssembler<ChainSpec>;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        &self.executor_factory
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        &self.block_assembler
    }

    fn evm_env(&self, header: &HeaderTy<Self::Primitives>) -> Result<EvmEnvFor<Self>, Self::Error> {
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
        self.block_executor_factory().evm_factory()
    }

    fn evm_with_env<DB: Database>(&self, db: DB, evm_env: EvmEnvFor<Self>) -> EvmFor<Self, DB> {
        // self.inner.evm_with_env(db, evm_env)
        self.evm_factory().create_evm(db, evm_env)
    }

    fn evm_for_block<DB: Database>(
        &self,
        db: DB,
        header: &HeaderTy<Self::Primitives>,
    ) -> Result<EvmFor<Self, DB>, Self::Error> {
        // self.inner.evm_for_block(db, header)
        let evm_env = self.evm_env(header)?;
        Ok(self.evm_with_env(db, evm_env))
    }

    fn evm_with_env_and_inspector<DB, I>(
        &self,
        db: DB,
        evm_env: EvmEnvFor<Self>,
        inspector: I,
    ) -> EvmFor<Self, DB, I>
    where
        DB: Database,
        I: InspectorFor<Self, DB>,
    {
        // self.inner
        //     .evm_with_env_and_inspector(db, evm_env, inspector)
        self.evm_factory()
            .create_evm_with_inspector(db, evm_env, inspector)
    }

    fn create_executor<'a, DB, I>(
        &'a self,
        evm: EvmFor<Self, &'a mut State<DB>, I>,
        ctx: <Self::BlockExecutorFactory as BlockExecutorFactory>::ExecutionCtx<'a>,
    ) -> impl BlockExecutorFor<'a, Self::BlockExecutorFactory, DB, I>
    where
        DB: Database,
        I: InspectorFor<Self, &'a mut State<DB>> + 'a,
    {
        self.block_executor_factory().create_executor(evm, ctx)
    }

    fn executor_for_block<'a, DB: Database>(
        &'a self,
        db: &'a mut State<DB>,
        block: &'a SealedBlock<<Self::Primitives as NodePrimitives>::Block>,
    ) -> Result<impl BlockExecutorFor<'a, Self::BlockExecutorFactory, DB>, Self::Error> {
        let evm = self.evm_for_block(db, block.header())?;
        let ctx = self.context_for_block(block)?;
        Ok(self.create_executor(evm, ctx))
    }

    /// Creates a [`BlockBuilder`]. Should be used when building a new block.
    ///
    /// Block builder wraps an inner [`alloy_evm::block::BlockExecutor`] and has a similar
    /// interface. Builder collects all of the executed transactions, and once
    /// [`BlockBuilder::finish`] is called, it invokes the configured [`BlockAssembler`] to
    /// create a block.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Create a builder with specific EVM configuration
    /// let evm = evm_config.evm_with_env(&mut state_db, evm_env);
    /// let ctx = evm_config.context_for_next_block(&parent, attributes);
    /// let builder = evm_config.create_block_builder(evm, &parent, ctx);
    /// ```
    fn create_block_builder<'a, DB, I>(
        &'a self,
        evm: EvmFor<Self, &'a mut State<DB>, I>,
        parent: &'a SealedHeader<HeaderTy<Self::Primitives>>,
        ctx: <Self::BlockExecutorFactory as BlockExecutorFactory>::ExecutionCtx<'a>,
    ) -> impl BlockBuilder<
        Primitives = Self::Primitives,
        Executor: BlockExecutorFor<'a, Self::BlockExecutorFactory, DB, I>,
    >
    where
        DB: Database,
        I: InspectorFor<Self, &'a mut State<DB>> + 'a,
    {
        BasicBlockBuilder::<Self::BlockExecutorFactory, _, _, Self::Primitives> {
            executor: self.create_executor(evm, ctx.clone()),
            ctx,
            assembler: self.block_assembler(),
            parent,
            transactions: Vec::new(),
        }
    }

    /// Creates a [`BlockBuilder`] for building of a new block. This is a helper to invoke
    /// [`ConfigureEvm::create_block_builder`].
    ///
    /// This is the primary method for building new blocks. It combines:
    /// 1. Creating the EVM environment for the next block
    /// 2. Setting up the execution context from attributes
    /// 3. Initializing the block builder with proper configuration
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Build a block with specific attributes
    /// let mut builder = evm_config.builder_for_next_block(
    ///     &mut state_db,
    ///     &parent_header,
    ///     attributes
    /// )?;
    ///
    /// // Execute system calls (e.g., beacon root update)
    /// builder.apply_pre_execution_changes()?;
    ///
    /// // Execute transactions
    /// for tx in transactions {
    ///     builder.execute_transaction(tx)?;
    /// }
    ///
    /// // Complete block building
    /// let outcome = builder.finish(state_provider)?;
    /// ```
    fn builder_for_next_block<'a, DB: Database>(
        &'a self,
        db: &'a mut State<DB>,
        parent: &'a SealedHeader<<Self::Primitives as NodePrimitives>::BlockHeader>,
        attributes: Self::NextBlockEnvCtx,
    ) -> Result<impl BlockBuilder<Primitives = Self::Primitives>, Self::Error> {
        let evm_env = self.next_evm_env(parent, &attributes)?;
        let evm = self.evm_with_env(db, evm_env);
        let ctx = self.context_for_next_block(parent, attributes)?;
        Ok(self.create_block_builder(evm, parent, ctx))
    }

    /// Returns a new [`Executor`] for executing blocks.
    ///
    /// The executor processes complete blocks including:
    /// - All transactions in order
    /// - Block rewards and fees
    /// - Block level system calls
    /// - State transitions
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Create an executor
    /// let mut executor = evm_config.executor(state_db);
    ///
    /// // Execute a single block
    /// let output = executor.execute(&block)?;
    ///
    /// // Execute multiple blocks
    /// let batch_output = executor.execute_batch(&blocks)?;
    /// ```
    ///
    fn executor<DB: Database>(
        &self,
        db: DB,
    ) -> impl Executor<DB, Primitives = Self::Primitives, Error = BlockExecutionError> {
        BasicBlockExecutor::new(self, db)
    }

    /// Returns a new [`BasicBlockExecutor`].
    fn batch_executor<DB: Database>(
        &self,
        db: DB,
    ) -> impl Executor<DB, Primitives = Self::Primitives, Error = BlockExecutionError> {
        BasicBlockExecutor::new(self, db)
    }
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
