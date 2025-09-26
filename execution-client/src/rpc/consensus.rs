use crate::consensus::ConsensusPool;
use alloy_consensus::Transaction;
use anyhow::Result;
use jsonrpsee::core::RpcResult;
use reth_ethereum::chainspec::EthChainSpec;
use reth_extension::CommittedSubDag;
use reth_extension::CommittedTransactions;
use reth_extension::ConsensusTransactionApiServer;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use std::sync::Arc;
use tracing::{debug, error};

/// The type that implements the `txpool` rpc namespace trait
pub struct ConsensusTransactionsHandler<Pool: TransactionPool, ChainSpec: EthChainSpec> {
    /// Consensus pool keep committed transactions from mysticeti
    consensus_pool: Arc<ConsensusPool<Pool>>,
    /// Transaction pool keep transactions from reth
    tx_pool: Pool,
    chain_spec: Arc<ChainSpec>,
}
impl<Pool: TransactionPool, ChainSpec: EthChainSpec> ConsensusTransactionsHandler<Pool, ChainSpec> {
    pub fn new(
        consensus_pool: Arc<ConsensusPool<Pool>>,
        tx_pool: Pool,
        chain_spec: Arc<ChainSpec>,
    ) -> Self {
        Self {
            consensus_pool,
            tx_pool,
            chain_spec,
        }
    }
    pub fn clone(&self) -> Self {
        Self {
            consensus_pool: self.consensus_pool.clone(),
            tx_pool: self.tx_pool.clone(),
            chain_spec: self.chain_spec.clone(),
        }
    }
}
impl<Pool: TransactionPool, ChainSpec: EthChainSpec> ConsensusTransactionsHandler<Pool, ChainSpec> {
    /// Process a single subdag
    /// We add committed transactions to consensus pool
    /// Add missing transactions from consensus pool to transaction pool
    async fn process_single_subdag(&self, subdag: CommittedSubDag) -> Result<()> {
        let committed_transactions = CommittedTransactions::<Pool::Transaction>::try_from(subdag)?;
        //Update transaction pool with committed transactions
        if committed_transactions.transactions.len() > 0 {
            self.update_pool_with_transactions(&committed_transactions)
                .await?;
        }
        self.consensus_pool
            .add_committed_transactions(committed_transactions);
        Ok(())
    }

    async fn update_pool_with_transactions(
        &self,
        committed_transactions: &CommittedTransactions<Pool::Transaction>,
    ) -> Result<usize> {
        let mut added_count = 0;
        //Loop through all transactions in the subdag, add to pool if missing
        for tx in committed_transactions.transactions.iter() {
            let tx_hash = tx.hash();
            //Check if transaction is in pool
            let pooled_tx = self.tx_pool.get(tx_hash);
            //If transaction is not in pool, add to pool
            if pooled_tx.is_none() {
                debug!(
                    "Added subdag transaction to pool: {:?}, sender: {:?}, nonce: {:?}",
                    tx_hash,
                    tx.sender_ref(),
                    tx.nonce()
                );
                //Add transaction to pool
                let add_result = self
                    .tx_pool
                    .add_external_transaction(tx.as_ref().clone())
                    .await;
                if add_result.is_ok() {
                    added_count += 1;
                } else {
                    error!("Error adding transaction to pool: {:?}", add_result.err());
                }
            }
        }

        debug!(
            "Added {}/{} subdag transactions to pool",
            added_count,
            committed_transactions.transactions.len()
        );
        Ok(added_count)
    }
}
impl<Pool: TransactionPool + 'static, ChainSpec: EthChainSpec + 'static>
    ConsensusTransactionApiServer for ConsensusTransactionsHandler<Pool, ChainSpec>
{
    #[doc = " Submit commited subdag"]
    fn submit_committed_subdag(&self, subdag: CommittedSubDag) -> RpcResult<()> {
        //Log every 100 commits
        if subdag.commit_ref.index % 1000 == 0 {
            debug!("Processing committed subdag with timestamp: {:?}, Total committed subdags: {:?}, Commit Index {:?}, Commit Digest {:?}, Leader round {:?}, Leader Digest {:?}",
            subdag.timestamp_ms, self.consensus_pool.queue_size(),
            subdag.commit_ref.index,
            format!("{:?}", subdag.commit_ref.digest),
            subdag.leader.round,
            hex::encode(subdag.leader.digest.as_ref()));
        }
        let handler = self.clone();
        tokio::spawn(Box::pin(async move {
            if let Err(e) = handler.process_single_subdag(subdag).await {
                error!("Error processing subdag: {:?}", e);
            }
        }));

        Ok(())
    }
}
