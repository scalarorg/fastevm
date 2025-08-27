use alloy_primitives::Bytes;
use jsonrpsee::core::RpcResult;
use reth_ethereum::{pool::TransactionPool, tasks::TaskExecutor};
use reth_extension::ConsensusTransactionApiServer;
use tracing::info;

/// The type that implements the `txpool` rpc namespace trait
pub struct ConsensusTransactionsHandler<Pool> {
    #[allow(unused)]
    task_executor: TaskExecutor,
    pool: Pool,
}
impl<Pool> ConsensusTransactionsHandler<Pool> {
    pub fn new(task_executor: TaskExecutor, pool: Pool) -> Self {
        Self {
            task_executor,
            pool,
        }
    }
}

impl<Pool> ConsensusTransactionApiServer for ConsensusTransactionsHandler<Pool>
where
    Pool: TransactionPool + Clone + 'static,
{
    #[doc = " Submit commited transactions"]
    fn submit_committed_transactions(&self, transactions: Vec<Bytes>) -> RpcResult<()> {
        info!("submit_committed_transactions: {:?}", transactions);
        //1. Store transactions in the custom pool with payloadId
        //2. Create a new payload with generated payloadId and committed transactions
        //3. Call method newPayload to execute the payload
        Ok(())
    }
}
