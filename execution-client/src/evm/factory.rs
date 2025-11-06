use alloy_evm::{eth::EthEvmContext, precompiles::PrecompilesMap, EvmFactory};
use reth_ethereum::{
    chainspec::{Chain, ChainSpec},
    evm::{
        primitives::Database,
        revm::{
            context::{Context, TxEnv},
            context_interface::result::{EVMError, HaltReason},
            inspector::Inspector,
            primitives::hardfork::SpecId,
            MainBuilder, MainContext,
        },
        EthEvm,
    },
};
use reth_provider::LatestStateProviderRef;
use reth_revm::{cached::CachedReads, db::State};
use revm::context::{BlockEnv, CfgEnv};

/// Custom EVM factory (same as before)
#[derive(Debug, Clone, Default)]
pub struct ScalarEvmFactory;

impl EvmFactory for ScalarEvmFactory {
    type Evm<DB: Database, I: Inspector<EthEvmContext<DB>>> = EthEvm<DB, I, Self::Precompiles>;
    type Context<DB: Database> = Context<BlockEnv, TxEnv, CfgEnv, DB>;
    type Tx = TxEnv;
    type Error<DBError: core::error::Error + Send + Sync + 'static> = EVMError<DBError>;
    type HaltReason = HaltReason;
    type Spec = SpecId;
    type Precompiles = PrecompilesMap;

    fn create_evm<DB: Database>(
        &self,
        db: DB,
        input: reth_evm::EvmEnv,
    ) -> Self::Evm<DB, revm::inspector::NoOpInspector> {
        let mut evm = revm::context::Context::mainnet()
            .with_db(db)
            .with_cfg(input.cfg_env)
            .with_block(input.block_env)
            .build_mainnet_with_inspector(revm::inspector::NoOpInspector {})
            .with_precompiles(PrecompilesMap::from_static(
                alloy_evm::revm::handler::EthPrecompiles::default().precompiles,
            ));
        EthEvm::new(evm, false)
    }

    fn create_evm_with_inspector<DB: Database, I: Inspector<Self::Context<DB>>>(
        &self,
        db: DB,
        input: reth_evm::EvmEnv,
        inspector: I,
    ) -> Self::Evm<DB, I> {
        EthEvm::new(
            self.create_evm(db, input)
                .into_inner()
                .with_inspector(inspector),
            true,
        )
    }
}
