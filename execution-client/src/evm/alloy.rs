use crate::evm::{ScalarBlockExecutor, ScalarEvmFactory};
use alloy_consensus::TxReceipt;
use alloy_eips::Encodable2718;
use alloy_evm::{Database, EvmFactory};
use alloy_primitives::Log;
use reth_ethereum::{evm::revm::Inspector, primitives::Transaction};
use reth_evm::{
    block::{BlockExecutorFactory, BlockExecutorFor},
    eth::{
        receipt_builder::{AlloyReceiptBuilder, ReceiptBuilder},
        spec::{EthExecutorSpec, EthSpec},
        EthBlockExecutionCtx,
    },
    FromRecoveredTx, FromTxWithEncoded,
};
use reth_revm::State;
use tracing::info;

/// Ethereum block executor factory.
#[derive(Debug, Clone, Default, Copy)]
pub struct ScalarBlockExecutorFactory<
    R = AlloyReceiptBuilder,
    Spec = EthSpec,
    EvmFactory = ScalarEvmFactory,
> {
    /// Receipt builder.
    receipt_builder: R,
    /// Chain specification.
    spec: Spec,
    /// EVM factory.
    evm_factory: EvmFactory,
}

impl<R, Spec, EvmFactory> ScalarBlockExecutorFactory<R, Spec, EvmFactory> {
    /// Creates a new [`ScalarBlockExecutorFactory`] with the given spec, [`EvmFactory`], and
    /// [`ReceiptBuilder`].
    pub const fn new(receipt_builder: R, spec: Spec, evm_factory: EvmFactory) -> Self {
        Self {
            receipt_builder,
            spec,
            evm_factory,
        }
    }

    /// Exposes the receipt builder.
    pub const fn receipt_builder(&self) -> &R {
        &self.receipt_builder
    }

    /// Exposes the chain specification.
    pub const fn spec(&self) -> &Spec {
        &self.spec
    }

    /// Exposes the EVM factory.
    pub const fn evm_factory(&self) -> &EvmFactory {
        &self.evm_factory
    }
}

impl<R, Spec, EvmF> BlockExecutorFactory for ScalarBlockExecutorFactory<R, Spec, EvmF>
where
    R: ReceiptBuilder<Transaction: Transaction + Encodable2718, Receipt: TxReceipt<Log = Log>>,
    Spec: EthExecutorSpec,
    EvmF: EvmFactory<Tx: FromRecoveredTx<R::Transaction> + FromTxWithEncoded<R::Transaction>>,
    Self: 'static,
{
    type EvmFactory = EvmF;
    type ExecutionCtx<'a> = EthBlockExecutionCtx<'a>;
    type Transaction = R::Transaction;
    type Receipt = R::Receipt;

    fn evm_factory(
        &self,
    ) -> &<ScalarBlockExecutorFactory<R, Spec, EvmF> as BlockExecutorFactory>::EvmFactory {
        &self.evm_factory
    }

    fn create_executor<'a, DB, I>(
        &'a self,
        evm: EvmF::Evm<&'a mut State<DB>, I>,
        ctx: <ScalarBlockExecutorFactory<R, Spec, EvmF> as BlockExecutorFactory>::ExecutionCtx<'a>,
    ) -> impl BlockExecutorFor<'a, Self, DB, I>
    where
        DB: Database + 'a,
        I: Inspector<EvmF::Context<&'a mut State<DB>>> + 'a,
    {
        info!("Creating scalar block executor");
        ScalarBlockExecutor::new(evm, ctx, &self.spec, &self.receipt_builder)
    }
}

// /// EVM context contains data that EVM needs for execution of [`CustomTxEnv`].
// pub type CustomContext<DB> =
//     Context<BlockEnv, OpTransaction<PaymentTxEnv>, CfgEnv<OpSpecId>, DB, Journal<DB>, L1BlockInfo>;

// pub struct CustomEvm<DB: Database, I, P = OpPrecompiles> {
//     inner: OpEvm<DB, I, P>,
// }

// impl<DB: Database, I, P> CustomEvm<DB, I, P> {
//     pub fn new(op: OpEvm<DB, I, P>) -> Self {
//         Self { inner: op }
//     }
// }

// impl<DB, I, P> Evm for CustomEvm<DB, I, P>
// where
//     DB: Database,
//     I: Inspector<OpContext<DB>>,
//     P: PrecompileProvider<OpContext<DB>, Output = InterpreterResult>,
// {
//     type DB = DB;
//     type Tx = CustomTxEnv;
//     type Error = EVMError<DB::Error, OpTransactionError>;
//     type HaltReason = OpHaltReason;
//     type Spec = OpSpecId;
//     type Precompiles = P;
//     type Inspector = I;

//     fn block(&self) -> &BlockEnv {
//         self.inner.block()
//     }

//     fn chain_id(&self) -> u64 {
//         self.inner.chain_id()
//     }

//     fn transact_raw(
//         &mut self,
//         tx: Self::Tx,
//     ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
//         match tx {
//             CustomTxEnv::Op(tx) => self.inner.transact_raw(tx),
//             CustomTxEnv::Payment(..) => todo!(),
//         }
//     }

//     fn transact_system_call(
//         &mut self,
//         caller: Address,
//         contract: Address,
//         data: Bytes,
//     ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
//         self.inner.transact_system_call(caller, contract, data)
//     }

//     fn finish(self) -> (Self::DB, EvmEnv<Self::Spec>) {
//         self.inner.finish()
//     }

//     fn set_inspector_enabled(&mut self, enabled: bool) {
//         self.inner.set_inspector_enabled(enabled)
//     }

//     fn components(&self) -> (&Self::DB, &Self::Inspector, &Self::Precompiles) {
//         self.inner.components()
//     }

//     fn components_mut(&mut self) -> (&mut Self::DB, &mut Self::Inspector, &mut Self::Precompiles) {
//         self.inner.components_mut()
//     }
// }
