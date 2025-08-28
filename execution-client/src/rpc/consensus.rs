use jsonrpsee::core::RpcResult;
use jsonrpsee::types::error::CALL_EXECUTION_FAILED_CODE;
use jsonrpsee::types::ErrorObject;
use reth_ethereum::pool::TransactionPool;
use reth_extension::CommittedSubDag;
use reth_extension::ConsensusTransactionApiServer;
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;
/// The type that implements the `txpool` rpc namespace trait
pub struct ConsensusTransactionsHandler<Pool> {
    subdag_tx: UnboundedSender<CommittedSubDag>,
    pool: Pool,
}
impl<Pool> ConsensusTransactionsHandler<Pool> {
    pub fn new(subdag_tx: UnboundedSender<CommittedSubDag>, pool: Pool) -> Self {
        Self { subdag_tx, pool }
    }
}

impl<Pool> ConsensusTransactionApiServer for ConsensusTransactionsHandler<Pool>
where
    Pool: TransactionPool + Clone + 'static,
{
    #[doc = " Submit commited subdag"]
    fn submit_committed_subdag(&self, subdag: CommittedSubDag) -> RpcResult<()> {
        info!("submit_committed_subdag: {:?}", subdag);
        // send the subdag to the consensus handler
        if let Err(e) = self.subdag_tx.send(subdag) {
            return Err(ErrorObject::owned(
                CALL_EXECUTION_FAILED_CODE,
                "Error sending subdag",
                Some(e.to_string()),
            ));
        }
        Ok(())
    }
}
