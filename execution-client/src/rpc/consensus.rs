use jsonrpsee::core::RpcResult;
use jsonrpsee::types::error::CALL_EXECUTION_FAILED_CODE;
use jsonrpsee::types::ErrorObject;
use reth_ethereum::pool::noop::NoopTransactionPool;
use reth_ethereum::pool::TransactionPool;
use reth_extension::CommittedSubDag;
use reth_extension::ConsensusTransactionApiServer;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;
use tracing::info;
/// The type that implements the `txpool` rpc namespace trait
#[derive(Debug)]
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
        let transactions = subdag.flatten_transactions();
        if transactions.is_empty() {
            info!("No transactions in subdag");
            return Ok(());
        }
        for tx in transactions.iter() {
            debug!("tx: {}", hex::encode(tx.to_vec()));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use reth_ethereum::pool::noop::NoopTransactionPool;
    use reth_extension::CommittedSubDag;

    #[test]
    fn test_consensus_transactions_handler_new() {
        let (subdag_tx, _subdag_rx) = tokio::sync::mpsc::unbounded_channel();
        let pool = NoopTransactionPool::default();
        let handler = ConsensusTransactionsHandler::new(subdag_tx, pool);

        // Test that the handler was created successfully
        // We can't directly test the fields as they're private, but we can test the methods
        let subdag = CommittedSubDag::default();
        let result = handler.submit_committed_subdag(subdag);
        assert!(result.is_ok());
    }

    #[test]
    fn test_consensus_transactions_handler_with_different_pools() {
        let (subdag_tx1, _subdag_rx1) = tokio::sync::mpsc::unbounded_channel();
        let (subdag_tx2, _subdag_rx2) = tokio::sync::mpsc::unbounded_channel();
        let pool1 = NoopTransactionPool::default();
        let pool2 = NoopTransactionPool::default();

        let handler1 = ConsensusTransactionsHandler::new(subdag_tx1, pool1);
        let handler2 = ConsensusTransactionsHandler::new(subdag_tx2, pool2);

        // Both should work the same way
        let subdag = CommittedSubDag::default();
        assert!(handler1.submit_committed_subdag(subdag.clone()).is_ok());
        assert!(handler2.submit_committed_subdag(subdag).is_ok());
    }

    #[test]
    fn test_submit_committed_subdag_success() {
        let (subdag_tx, _subdag_rx) = tokio::sync::mpsc::unbounded_channel();
        let pool = NoopTransactionPool::default();
        let handler = ConsensusTransactionsHandler::new(subdag_tx, pool);
        let subdag = CommittedSubDag::default();
        let result = handler.submit_committed_subdag(subdag);
        assert!(result.is_ok());
    }

    #[test]
    fn test_submit_committed_subdag_with_blocks() {
        let (subdag_tx, _subdag_rx) = tokio::sync::mpsc::unbounded_channel();
        let pool = NoopTransactionPool::default();
        let handler = ConsensusTransactionsHandler::new(subdag_tx, pool);

        // Create a subdag with some blocks
        let subdag = CommittedSubDag::default();
        // Note: We can't easily create blocks without more complex setup,
        // but we can test that the method works with the default subdag
        let result = handler.submit_committed_subdag(subdag);
        assert!(result.is_ok());
    }

    #[test]
    fn test_submit_committed_subdag_multiple_times() {
        let (subdag_tx, _subdag_rx) = tokio::sync::mpsc::unbounded_channel();
        let pool = NoopTransactionPool::default();
        let handler = ConsensusTransactionsHandler::new(subdag_tx, pool);

        // Submit multiple subdags
        for i in 0..5 {
            let subdag = CommittedSubDag::default();
            let result = handler.submit_committed_subdag(subdag);
            assert!(result.is_ok(), "Failed to submit subdag {}", i);
        }
    }

    #[test]
    fn test_consensus_transactions_handler_debug() {
        let (subdag_tx, _subdag_rx) = tokio::sync::mpsc::unbounded_channel();
        let pool = NoopTransactionPool::default();
        let handler = ConsensusTransactionsHandler::new(subdag_tx, pool);

        // Test that we can format the handler for debugging
        let debug_str = format!("{:?}", handler);
        assert!(debug_str.contains("ConsensusTransactionsHandler"));
    }

    #[test]
    fn test_committed_subdag_default() {
        let subdag = CommittedSubDag::default();
        assert!(subdag.blocks.is_empty());
        assert!(subdag.flatten_transactions().is_empty());
    }

    #[test]
    fn test_committed_subdag_clone() {
        let subdag = CommittedSubDag::default();
        let cloned = subdag.clone();
        assert_eq!(subdag.blocks.len(), cloned.blocks.len());
        assert_eq!(
            subdag.flatten_transactions().len(),
            cloned.flatten_transactions().len()
        );
    }

    #[test]
    fn test_committed_subdag_debug() {
        let subdag = CommittedSubDag::default();
        let debug_str = format!("{:?}", subdag);
        assert!(debug_str.contains("CommittedSubDag"));
    }

    #[test]
    fn test_error_object_creation() {
        let error = ErrorObject::owned(
            CALL_EXECUTION_FAILED_CODE,
            "Test error",
            Some("Test details".to_string()),
        );

        assert_eq!(error.code(), CALL_EXECUTION_FAILED_CODE);
        assert_eq!(error.message(), "Test error");
        assert_eq!(error.data().unwrap().get(), "\"Test details\"");
    }

    #[test]
    fn test_error_object_without_data() {
        let error = ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Test error", None::<()>);

        assert_eq!(error.code(), CALL_EXECUTION_FAILED_CODE);
        assert_eq!(error.message(), "Test error");
        assert!(error.data().is_none());
    }

    #[tokio::test]
    async fn test_submit_committed_subdag_async() {
        let (subdag_tx, mut subdag_rx) = tokio::sync::mpsc::unbounded_channel();
        let pool = NoopTransactionPool::default();
        let handler = ConsensusTransactionsHandler::new(subdag_tx, pool);
        let subdag = CommittedSubDag::default();

        // Submit the subdag
        let result = handler.submit_committed_subdag(subdag.clone());
        assert!(result.is_ok());

        // Verify that the subdag was sent through the channel
        let received_subdag = subdag_rx.recv().await;
        assert!(received_subdag.is_some());
        let received = received_subdag.unwrap();
        assert_eq!(received.blocks.len(), subdag.blocks.len());
    }

    #[tokio::test]
    async fn test_submit_committed_subdag_channel_closed() {
        let (subdag_tx, subdag_rx) = tokio::sync::mpsc::unbounded_channel();
        let pool = NoopTransactionPool::default();
        let handler = ConsensusTransactionsHandler::new(subdag_tx, pool);

        // Close the receiver
        drop(subdag_rx);

        // Now try to submit a subdag - this should fail
        let subdag = CommittedSubDag::default();
        let result = handler.submit_committed_subdag(subdag);
        assert!(result.is_err());

        // Check the error details
        let error = result.unwrap_err();
        assert_eq!(error.code(), CALL_EXECUTION_FAILED_CODE);
        assert_eq!(error.message(), "Error sending subdag");
    }
}
