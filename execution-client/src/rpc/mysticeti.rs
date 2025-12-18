use crate::consensus::ConsensusPool;
use crate::rpc::api::MysticetiConsensusApiServer;
use alloy_rlp::Decodable;
use anyhow::Result;
use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use jsonrpsee::types::error::PARSE_ERROR_CODE;
use jsonrpsee::types::ErrorObjectOwned;
use parking_lot::RwLock;
use reth_ethereum::chainspec::EthChainSpec;
use reth_ethereum::primitives::Recovered;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use rpc_shared_api::{CommittedSubDag, MysticetiCommittedSubdag};
use std::sync::Arc;
use tracing::{debug, info};

/// Convert a CommittedSubDag from RPC to MysticetiCommittedSubdag with decoded transactions
fn convert_committed_subdag<T: PoolTransaction>(
    subdag: CommittedSubDag,
) -> Result<MysticetiCommittedSubdag<Arc<T>>> {
    let CommittedSubDag {
        leader,
        blocks,
        timestamp_ms,
        commit_ref,
        reputation_scores_desc,
    } = subdag;

    let mut transactions = Vec::new();
    for block in blocks {
        for tx in block.block.transactions().iter() {
            let tx_data = tx.data().to_vec();
            let recovered_transaction =
                Recovered::<<T as PoolTransaction>::Consensus>::decode(&mut tx_data.as_slice())
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to decode consensus transaction: {}", e)
                    })?;
            let transaction = T::try_from_consensus(recovered_transaction).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to convert consensus transaction to pool transaction: {}",
                    e
                )
            })?;
            transactions.push(Arc::new(transaction));
        }
    }

    Ok(MysticetiCommittedSubdag {
        leader,
        transactions,
        timestamp_ms,
        commit_ref,
        reputation_scores_desc,
    })
}

/// The type that implements the `txpool` rpc namespace trait
pub struct MysticetiConsensusHandler<Pool: TransactionPool, ChainSpec: EthChainSpec> {
    /// Consensus pool keep committed transactions from mysticeti
    consensus_pool: Arc<ConsensusPool<Pool>>,
    /// Transaction pool keep transactions from reth
    tx_pool: Pool,
    chain_spec: Arc<ChainSpec>,
    // For debugging
    total_txs: Arc<RwLock<u64>>,
}
impl<Pool: TransactionPool, ChainSpec: EthChainSpec> MysticetiConsensusHandler<Pool, ChainSpec> {
    pub fn new(
        consensus_pool: Arc<ConsensusPool<Pool>>,
        tx_pool: Pool,
        chain_spec: Arc<ChainSpec>,
    ) -> Self {
        Self {
            consensus_pool,
            tx_pool,
            chain_spec,
            total_txs: Arc::new(RwLock::new(0)),
        }
    }
    pub fn clone(&self) -> Self {
        Self {
            consensus_pool: self.consensus_pool.clone(),
            tx_pool: self.tx_pool.clone(),
            chain_spec: self.chain_spec.clone(),
            total_txs: self.total_txs.clone(),
        }
    }
}
impl<Pool: TransactionPool, ChainSpec: EthChainSpec> MysticetiConsensusHandler<Pool, ChainSpec> {
    /// Process a single subdag
    /// We add committed transactions to consensus pool
    /// Add missing transactions from consensus pool to transaction pool
    async fn process_subdags(&self, subdags: Vec<CommittedSubDag>) -> Result<()> {
        let mut committed_subdags = Vec::new();
        let fist_index = subdags.first().map(|subdag| subdag.commit_ref.round);
        let last_index = subdags.last().map(|subdag| subdag.commit_ref.round);
        let mut tx_counter = 0;
        for subdag in subdags {
            let committed_subdag = convert_committed_subdag::<Pool::Transaction>(subdag)?;
            //Update transaction pool with committed transactions
            tx_counter += committed_subdag.transactions.len();
            if committed_subdag.transactions.len() > 0 {
                self.update_pool_with_transactions(&committed_subdag)
                    .await?;
            }
            committed_subdags.push(committed_subdag);
        }
        let mut total_txs = self.total_txs.write();
        *total_txs += tx_counter as u64;
        info!(
            "Processed subdags from index {:?} to {:?}, Total transactions: {:?}",
            fist_index,
            last_index,
            *self.total_txs.read()
        );
        self.consensus_pool.add_committed_subdags(committed_subdags);

        Ok(())
    }

    async fn update_pool_with_transactions(
        &self,
        committed_transactions: &MysticetiCommittedSubdag<Arc<Pool::Transaction>>,
    ) -> Result<usize> {
        let mut added_count = 0;
        //Loop through all transactions in the subdag, add to pool if missing
        for tx in committed_transactions.transactions.iter() {
            let tx_hash = tx.hash();
            //Check if transaction is in pool
            let pooled_tx = self.tx_pool.get(tx_hash);
            //If transaction is not in pool, add to pool
            if pooled_tx.is_none() {
                // debug!(
                //     "Added subdag transaction to pool: {:?}, sender: {:?}, nonce: {:?}",
                //     tx_hash,
                //     tx.sender_ref(),
                //     tx.nonce()
                // );
                //Add transaction to pool
                let add_result = self
                    .tx_pool
                    .add_external_transaction(tx.as_ref().clone())
                    .await;
                if add_result.is_ok() {
                    added_count += 1;
                }
                // else {
                //     error!("Error adding transaction to pool: {:?}", add_result.err());
                // }
            }
        }

        // debug!(
        //     "Added {}/{} subdag transactions to pool",
        //     added_count,
        //     committed_transactions.transactions.len()
        // );
        Ok(added_count)
    }
    // async fn handle_raw_transaction(&self, tx: Bytes) -> Result<B256> {
    //     let recovered = recover_raw_transaction(&tx)?;

    //     // Simulate broadcast_raw_transaction by adding with Local origin
    //     // This automatically triggers the transaction pool's event system
    //     // which broadcasts to all subscribers via the TxpoolListener
    //     let pool_transaction = Pool::Transaction::from_pooled(recovered);
    //     let hash = self
    //         .tx_pool
    //         .add_transaction(TransactionOrigin::Local, pool_transaction)
    //         .await
    //         .map_err(|e| anyhow::anyhow!("Failed to add transaction to pool: {}", e))?;

    //     // The transaction is now automatically "broadcast" through the pool's event system
    //     // The TxpoolListener will pick up this transaction and send it to subscribers
    //     info!(
    //         "Transaction {} added to pool and will be broadcast to subscribers",
    //         hash
    //     );

    //     Ok(hash)
    // }
}

#[async_trait]
impl<Pool: TransactionPool + 'static, ChainSpec: EthChainSpec + 'static> MysticetiConsensusApiServer
    for MysticetiConsensusHandler<Pool, ChainSpec>
{
    #[doc = " Submit commited subdag"]
    fn submit_committed_subdag(&self, subdag: CommittedSubDag) -> RpcResult<()> {
        // This method is called by consensus client to submit committed subdags each 100ms
        // We don't need to process in separate thread
        let mut committed_subdags = Vec::new();
        let mut tx_counter = 0;
        let start_time = std::time::Instant::now();
        let commited_index = subdag.commit_ref.round;
        let committed_subdag =
            convert_committed_subdag::<Pool::Transaction>(subdag).map_err(|e| {
                ErrorObjectOwned::owned(
                    PARSE_ERROR_CODE,
                    format!(
                        "Failed to convert committed subdag to MysticetiCommittedSubdag: {}",
                        e
                    ),
                    None::<()>,
                )
            })?;
        tx_counter += committed_subdag.transactions.len();
        committed_subdags.push(committed_subdag);

        self.consensus_pool.add_committed_subdags(committed_subdags);
        let mut total_txs = self.total_txs.write();
        *total_txs += tx_counter as u64;
        debug!(
            "Processed subdag index {:?}  with {:?} transactions, Total transactions: {:?}. Time taken: {:?}",
            commited_index,
            tx_counter,
            *total_txs,
            start_time.elapsed()
        );
        Ok(())
    }

    #[doc = " Submit commited subdags"]
    fn submit_committed_subdags(&self, subdags: Vec<CommittedSubDag>) -> RpcResult<()> {
        // This method is called by consensus client to submit committed subdags each 100ms
        // We don't need to process in separate thread
        let mut committed_subdags = Vec::new();
        let mut tx_counter = 0;
        let start_time = std::time::Instant::now();
        for subdag in subdags {
            let committed_subdag =
                convert_committed_subdag::<Pool::Transaction>(subdag).map_err(|e| {
                    ErrorObjectOwned::owned(
                        PARSE_ERROR_CODE,
                        format!(
                            "Failed to convert committed subdag to MysticetiCommittedSubdag: {}",
                            e
                        ),
                        None::<()>,
                    )
                })?;
            tx_counter += committed_subdag.transactions.len();
            committed_subdags.push(committed_subdag);
        }
        let fist_index = committed_subdags
            .first()
            .map(|subdag| subdag.commit_ref.round);
        let last_index = committed_subdags
            .last()
            .map(|subdag| subdag.commit_ref.round);
        self.consensus_pool.add_committed_subdags(committed_subdags);
        let mut total_txs = self.total_txs.write();
        *total_txs += tx_counter as u64;
        info!(
            "Processed subdags from index {:?} to {:?} with {:?} transactions, Total transactions: {:?}. Time taken: {:?}",
            fist_index,
            last_index,
            tx_counter,
            *total_txs,
            start_time.elapsed()
        );
        // let handler = self.clone();
        // tokio::spawn(Box::pin(async move {
        //     if let Err(e) = handler.process_subdags(subdags).await {
        //         error!("Error processing subdag: {:?}", e);
        //     }
        // }));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, U256};
    use reth_chainspec::MAINNET;
    use reth_ethereum::pool::noop::NoopTransactionPool;
    use rpc_shared_api::{BlockRef, CommitRef};

    type TestPool = NoopTransactionPool;

    fn create_test_handler() -> MysticetiConsensusHandler<TestPool, reth_chainspec::ChainSpec> {
        let consensus_pool = Arc::new(ConsensusPool::<TestPool>::new(1));
        let tx_pool = TestPool::default();
        let chain_spec = MAINNET.clone();

        MysticetiConsensusHandler::new(consensus_pool, tx_pool, chain_spec)
    }

    fn create_empty_subdag(round: u64) -> CommittedSubDag {
        CommittedSubDag {
            leader: BlockRef::default(),
            blocks: Vec::new(),
            timestamp_ms: 0,
            commit_ref: CommitRef {
                round,
                digest: [0u8; 32],
            },
            reputation_scores_desc: Vec::new(),
        }
    }

    #[test]
    fn test_mysticeti_consensus_handler_new() {
        let handler = create_test_handler();

        // Verify handler is created successfully
        assert_eq!(*handler.total_txs.read(), 0);
    }

    #[test]
    fn test_mysticeti_consensus_handler_clone() {
        let handler = create_test_handler();
        let cloned = handler.clone();

        // Both should share the same Arc for total_txs
        assert_eq!(*handler.total_txs.read(), *cloned.total_txs.read());
    }

    #[test]
    fn test_submit_committed_subdag_empty() {
        let handler = create_test_handler();
        let subdag = create_empty_subdag(1);

        let result = MysticetiConsensusApiServer::submit_committed_subdag(&handler, subdag);

        assert!(result.is_ok());
        assert_eq!(*handler.total_txs.read(), 0);
    }

    #[test]
    fn test_submit_committed_subdags_empty() {
        let handler = create_test_handler();
        let subdags = vec![
            create_empty_subdag(1),
            create_empty_subdag(2),
            create_empty_subdag(3),
        ];

        let result = MysticetiConsensusApiServer::submit_committed_subdags(&handler, subdags);

        assert!(result.is_ok());
        assert_eq!(*handler.total_txs.read(), 0);
    }

    #[test]
    fn test_submit_committed_subdags_multiple() {
        let handler = create_test_handler();

        // Submit multiple empty subdags
        for i in 1..=5 {
            let subdag = create_empty_subdag(i);
            let result = MysticetiConsensusApiServer::submit_committed_subdag(&handler, subdag);
            assert!(result.is_ok());
        }

        // Verify all subdags were added to consensus pool
        assert_eq!(handler.consensus_pool.queue_size(), 5);
    }

    #[test]
    fn test_consensus_pool_integration() {
        let handler = create_test_handler();

        // Submit a batch of subdags
        let subdags = vec![create_empty_subdag(1), create_empty_subdag(2)];

        let result = MysticetiConsensusApiServer::submit_committed_subdags(&handler, subdags);
        assert!(result.is_ok());

        // Verify queue size
        assert_eq!(handler.consensus_pool.queue_size(), 2);
    }

    #[test]
    fn test_committed_subdag_default() {
        let subdag = CommittedSubDag::default();

        assert!(subdag.blocks.is_empty());
        assert_eq!(subdag.timestamp_ms, 0);
        assert_eq!(subdag.commit_ref.round, 0);
    }

    #[test]
    fn test_block_ref_default() {
        let block_ref = BlockRef::default();

        assert_eq!(block_ref.round, 0);
        assert_eq!(block_ref.digest, [0u8; 32]);
    }

    #[test]
    fn test_commit_ref_default() {
        let commit_ref = CommitRef::default();

        assert_eq!(commit_ref.round, 0);
        assert_eq!(commit_ref.digest, [0u8; 32]);
    }
}
