use std::time::Instant;

use alloy_consensus::TxReceipt;
use alloy_eips::Encodable2718;
use alloy_evm::{
    block::{BlockExecutionError, BlockExecutionResult, BlockExecutor, ExecutableTx, OnStateHook},
    Database, Evm,
};
use alloy_primitives::Log;
use reth_ethereum::primitives::Transaction;
use reth_evm::{
    eth::{
        receipt_builder::ReceiptBuilder, spec::EthExecutorSpec, EthBlockExecutionCtx,
        EthBlockExecutor,
    },
    FromRecoveredTx, FromTxWithEncoded,
};
use revm::{context::result::ResultAndState, database::State};
use tracing::info;

// /// A generic block executor that uses a [`BlockExecutor`] to
// /// execute blocks.
// #[expect(missing_debug_implementations)]
// pub struct ScalarBlockExecutor<F, DB> {
//     /// Block execution strategy.
//     pub(crate) strategy_factory: F,
//     /// Database.
//     pub(crate) db: State<DB>,
// }

// impl<F, DB: Database> ScalarBlockExecutor<F, DB> {
//     /// Creates a new `BasicBlockExecutor` with the given strategy.
//     pub fn new(strategy_factory: F, db: DB) -> Self {
//         let db = State::builder()
//             .with_database(db)
//             .with_bundle_update()
//             .without_state_clear()
//             .build();
//         Self {
//             strategy_factory,
//             db,
//         }
//     }
// }

// impl<F, DB> Executor<DB> for ScalarBlockExecutor<F, DB>
// where
//     F: ConfigureEvm,
//     DB: Database,
// {
//     type Primitives = F::Primitives;
//     type Error = BlockExecutionError;

//     fn execute_one(
//         &mut self,
//         block: &RecoveredBlock<<Self::Primitives as NodePrimitives>::Block>,
//     ) -> Result<BlockExecutionResult<<Self::Primitives as NodePrimitives>::Receipt>, Self::Error>
//     {
//         let result = self
//             .strategy_factory
//             .executor_for_block(&mut self.db, block)
//             .map_err(BlockExecutionError::other)?
//             .execute_block(block.transactions_recovered())?;

//         self.db.merge_transitions(BundleRetention::Reverts);

//         Ok(result)
//     }

//     fn execute_one_with_state_hook<H>(
//         &mut self,
//         block: &RecoveredBlock<<Self::Primitives as NodePrimitives>::Block>,
//         state_hook: H,
//     ) -> Result<BlockExecutionResult<<Self::Primitives as NodePrimitives>::Receipt>, Self::Error>
//     where
//         H: OnStateHook + 'static,
//     {
//         let result = self
//             .strategy_factory
//             .executor_for_block(&mut self.db, block)
//             .map_err(BlockExecutionError::other)?
//             .with_state_hook(Some(Box::new(state_hook)))
//             .execute_block(block.transactions_recovered())?;

//         self.db.merge_transitions(BundleRetention::Reverts);

//         Ok(result)
//     }

//     fn into_state(self) -> State<DB> {
//         self.db
//     }

//     fn size_hint(&self) -> usize {
//         self.db.bundle_state.size_hint()
//     }
// }

pub struct ScalarBlockExecutor<'a, Evm, Spec, R: ReceiptBuilder> {
    inner: EthBlockExecutor<'a, Evm, Spec, R>,
    gas_used: u64,
}
impl<'a, Evm, Spec, R> ScalarBlockExecutor<'a, Evm, Spec, R>
where
    Spec: Clone,
    R: ReceiptBuilder,
{
    /// Creates a new [`ScalarBlockExecutor`]
    pub fn new(evm: Evm, ctx: EthBlockExecutionCtx<'a>, spec: Spec, receipt_builder: R) -> Self {
        Self {
            inner: EthBlockExecutor::new(evm, ctx, spec, receipt_builder),
            gas_used: 0,
        }
    }
}
impl<'db, DB, E, Spec, R> BlockExecutor for ScalarBlockExecutor<'db, E, Spec, R>
where
    DB: Database + 'db,
    E: Evm<
        DB = &'db mut State<DB>,
        Tx: FromRecoveredTx<R::Transaction> + FromTxWithEncoded<R::Transaction>,
    >,
    Spec: EthExecutorSpec,
    R: ReceiptBuilder<Transaction: Transaction + Encodable2718, Receipt: TxReceipt<Log = Log>>,
{
    type Transaction = R::Transaction;
    type Receipt = R::Receipt;
    type Evm = E;

    fn apply_pre_execution_changes(&mut self) -> Result<(), BlockExecutionError> {
        self.inner.apply_pre_execution_changes()
    }

    fn execute_block(
        mut self,
        transactions: impl IntoIterator<Item = impl ExecutableTx<Self>>,
    ) -> Result<BlockExecutionResult<Self::Receipt>, BlockExecutionError>
    where
        Self: Sized,
    {
        info!("[ScalarBlockExecutor] Executing block");
        self.apply_pre_execution_changes()?;
        let start = Instant::now();
        let mut count = 0;
        for tx in transactions {
            self.execute_transaction(tx)?;
            count += 1;
        }
        info!(
            "[ScalarBlockExecutor] Executed block with {} transactions in {:?}. Total gas used: {}.",
            count,
            start.elapsed(),
            self.gas_used
        );
        let result = self.apply_post_execution_changes()?;
        Ok(result)
    }

    fn execute_transaction_without_commit(
        &mut self,
        tx: impl ExecutableTx<Self>,
    ) -> Result<ResultAndState<<Self::Evm as Evm>::HaltReason>, BlockExecutionError> {
        self.inner.execute_transaction_without_commit(tx)
        // // The sum of the transaction's gas limit, Tg, and the gas utilized in this block prior,
        // // must be no greater than the block's gasLimit.
        // let block_available_gas = self.evm().block().gas_limit - self.gas_used;

        // if tx.tx().gas_limit() > block_available_gas {
        //     return Err(
        //         BlockValidationError::TransactionGasLimitMoreThanAvailableBlockGas {
        //             transaction_gas_limit: tx.tx().gas_limit(),
        //             block_available_gas,
        //         }
        //         .into(),
        //     );
        // }

        // // Execute transaction and return the result
        // self.evm_mut().transact(&tx).map_err(|err| {
        //     let hash = tx.tx().trie_hash();
        //     BlockExecutionError::evm(err, hash)
        // })
    }

    fn commit_transaction(
        &mut self,
        output: ResultAndState<<Self::Evm as Evm>::HaltReason>,
        tx: impl ExecutableTx<Self>,
    ) -> Result<u64, BlockExecutionError> {
        let result = self.inner.commit_transaction(output, tx)?;
        info!(
            "[ScalarBlockExecutor] Committing transaction with gas used: {}",
            result
        );
        self.gas_used += result;
        Ok(result)
    }

    fn finish(self) -> Result<(Self::Evm, BlockExecutionResult<R::Receipt>), BlockExecutionError> {
        info!("[ScalarBlockExecutor] Finish");
        self.inner.finish()
    }

    fn set_state_hook(&mut self, _hook: Option<Box<dyn OnStateHook>>) {
        self.inner.set_state_hook(_hook)
    }

    fn evm_mut(&mut self) -> &mut Self::Evm {
        self.inner.evm_mut()
    }

    fn evm(&self) -> &Self::Evm {
        self.inner.evm()
    }
}
