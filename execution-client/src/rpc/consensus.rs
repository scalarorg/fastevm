use crate::consensus::ConsensusPool;
use crate::pool::MysticetiPool;
use anyhow::Result;
use jsonrpsee::core::RpcResult;
use reth_ethereum::chainspec::EthChainSpec;
use reth_extension::CommittedSubDag;
use reth_extension::ConsensusTransactionApiServer;
use reth_extension::MysticetiCommittedSubdag;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error};

/// The type that implements the `txpool` rpc namespace trait
pub struct ConsensusTransactionsHandler<Pool, ChainSpec>
where
    Pool: TransactionPool + MysticetiPool,
    ChainSpec: EthChainSpec,
{
    /// Consensus pool keep committed transactions from mysticeti
    consensus_pool: Arc<ConsensusPool<Pool>>,
    /// Transaction pool keep transactions from reth
    tx_pool: Pool,
    chain_spec: Arc<ChainSpec>,
    // For debugging
    total_txs: Arc<Mutex<u64>>,
}
impl<Pool, ChainSpec: EthChainSpec> ConsensusTransactionsHandler<Pool, ChainSpec>
where
    Pool: TransactionPool + MysticetiPool,
{
    pub fn new(
        consensus_pool: Arc<ConsensusPool<Pool>>,
        tx_pool: Pool,
        chain_spec: Arc<ChainSpec>,
    ) -> Self {
        Self {
            consensus_pool,
            tx_pool,
            chain_spec,
            total_txs: Arc::new(Mutex::new(0)),
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
impl<Pool: TransactionPool + MysticetiPool, ChainSpec: EthChainSpec>
    ConsensusTransactionsHandler<Pool, ChainSpec>
{
    /// Process a single subdag
    /// We add committed transactions to consensus pool
    /// Add missing transactions from consensus pool to transaction pool
    async fn process_subdags(&self, subdags: Vec<CommittedSubDag>) -> Result<()> {
        let mut committed_subdags = Vec::new();
        for subdag in subdags {
            let committed_subdag = MysticetiCommittedSubdag::<Pool::Transaction>::try_from(subdag)?;
            //Update transaction pool with committed transactions
            if committed_subdag.transactions.len() > 0 {
                self.update_pool_with_transactions(&committed_subdag)
                    .await?;
                let mut total_txs = self.total_txs.lock().await;
                *total_txs += committed_subdag.transactions.len() as u64;
            }
            debug!(
                "Commited Index: {:?}, Total transactions: {:?}",
                committed_subdag.commit_ref.index,
                *self.total_txs.lock().await
            );
            committed_subdags.push(committed_subdag);
        }
        self.tx_pool
            .add_committed_subdags(committed_subdags.clone());
        self.consensus_pool.add_committed_subdags(committed_subdags);

        Ok(())
    }

    async fn update_pool_with_transactions(
        &self,
        committed_transactions: &MysticetiCommittedSubdag<Pool::Transaction>,
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
}
impl<Pool, ChainSpec> ConsensusTransactionApiServer
    for ConsensusTransactionsHandler<Pool, ChainSpec>
where
    Pool: TransactionPool + MysticetiPool + 'static,
    ChainSpec: EthChainSpec + 'static,
{
    #[doc = " Submit commited subdag"]
    fn submit_committed_subdags(&self, subdags: Vec<CommittedSubDag>) -> RpcResult<()> {
        // Log every 100 commits
        // if let Some(subdag) = subdags.first() {
        //     debug!(
        //         "Received {} committed subdags start from index {:?}",
        //         subdags.len(),
        //         subdag.commit_ref.index
        //     );
        // }

        let handler = self.clone();
        tokio::spawn(Box::pin(async move {
            if let Err(e) = handler.process_subdags(subdags).await {
                error!("Error processing subdag: {:?}", e);
            }
        }));

        Ok(())
    }
}
