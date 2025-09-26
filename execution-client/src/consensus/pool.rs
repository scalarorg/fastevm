//! This module contains the implementation of the block creation from committed subdag.
//! Transactions in the subdag may are not ordered by nonce.
//! Or is not cons

use alloy_primitives::{keccak256, Bytes, TxHash, B256};
use reth_ethereum::rpc::types::engine::ExecutionPayload;
use reth_extension::CommittedTransactions;
use reth_transaction_pool::{PoolTransaction, TransactionPool, ValidPoolTransaction};
use std::{
    cmp::Ordering,
    collections::{HashSet, VecDeque},
    sync::{Arc, Mutex},
};
use tracing::debug;

/// Struct to store committed transactions and pooled transactions
/// Pooled transactions are transactions from committed transactions that are added to the reth pool
struct PooledCommittedTransactions<Transaction: PoolTransaction> {
    // next mysticeti committed transactions
    committed_transactions: CommittedTransactions<Transaction>,
    // transactions already in reth pool
    pooled_transactions: Vec<Arc<ValidPoolTransaction<Transaction>>>,
}
pub struct ConsensusPool<Pool: TransactionPool>
where
    Pool: TransactionPool,
{
    //Store committed transactions (converted from subdag) in queue
    commited_queue: VecDeque<PooledCommittedTransactions<Pool::Transaction>>,
    // Transactions are not included into last payload due to missing of ancestors
    pending_transactions: Mutex<Vec<Arc<ValidPoolTransaction<Pool::Transaction>>>>,
    // latest_state: Option<StateProviderBox>,
}

impl<Pool: TransactionPool> ConsensusPool<Pool>
where
    Pool: TransactionPool,
{
    pub fn new() -> Self {
        Self {
            commited_queue: VecDeque::new(),
            pending_transactions: Mutex::new(Vec::new()),
        }
    }

    pub fn queue_size(&self) -> usize {
        self.commited_queue.len()
    }

    pub fn next_committed_transactions(&self) -> Option<&CommittedTransactions<Pool::Transaction>> {
        self.commited_queue
            .front()
            .map(|item| &item.committed_transactions)
    }
}

impl<Pool: TransactionPool> ConsensusPool<Pool>
where
    Pool: TransactionPool,
{
    pub fn add_committed_transactions(
        &mut self,
        committed_transactions: CommittedTransactions<Pool::Transaction>,
        pooled_transactions: Vec<Arc<ValidPoolTransaction<Pool::Transaction>>>,
    ) {
        self.commited_queue.push_back(PooledCommittedTransactions {
            committed_transactions,
            pooled_transactions,
        });
    }

    /// Get all pending transactions and transactions from next committed subdag
    /// Order them by sender nonce
    /// TODO: review orderer algorithm
    pub fn get_proposal_transactions(
        &mut self,
    ) -> Vec<Arc<ValidPoolTransaction<Pool::Transaction>>> {
        // 1. Append transactions from next committed subdag to pending transactions
        let mut pending_transactions = self.pending_transactions.lock().unwrap();
        let pending_size = pending_transactions.len();
        if let Some(PooledCommittedTransactions {
            committed_transactions,
            pooled_transactions,
        }) = self.commited_queue.pop_front()
        {
            //This transactions are ordered and consecutive by nonce
            debug!(
                "Committed transactions: {:?}. Number of pooled transactions: {:?}",
                committed_transactions.transactions.len(),
                pooled_transactions.len()
            );
            //TODO: find better way to sync committed transactions and pooled transactions
            pending_transactions.extend(pooled_transactions);
            //Sort transactions by sender nonce
            pending_transactions.sort_by(|tx1, tx2| {
                if tx1.sender() == tx2.sender() {
                    return tx1.nonce().cmp(&tx2.nonce());
                } else {
                    //We don't care about the order of different senders
                    return Ordering::Equal;
                }
            });
        }
        debug!(
            "Get proposal transactions. Last pending txs: {}. All pending txs (+ current committed): {:?}",
            pending_size,
            pending_transactions.len()
        );
        //Clone pending transactions for building a BestTransactions iterator
        return pending_transactions.clone();
    }
    /// Remove mined transactions from pending transactions
    pub fn remove_mined_transactions(&mut self, execution_payload: &ExecutionPayload) {
        let tx_hashes = execution_payload
            .transactions()
            .iter()
            .map(|tx| calculate_tx_hash(tx))
            .collect::<HashSet<TxHash>>();
        let mut pending_transactions = self.pending_transactions.lock().unwrap();
        debug!(
            "Remove mined transactions in block number {:?}. Mysticeti pending txs: {}. Mined txs: {:?}",
            execution_payload.block_number(),
            pending_transactions.len(),
            tx_hashes.len()
        );
        pending_transactions.retain(|tx| !tx_hashes.contains(tx.hash()));
    }
}

fn calculate_tx_hash(tx: &Bytes) -> TxHash {
    let tx_hash: B256 = keccak256(tx);

    // // Decode the transaction
    // let transaction: TransactionSigned = TransactionSigned::decode_2718(&mut tx)?;

    // // Get the hash
    // *transaction.tx_hash()
    tx_hash
}
// fn try_add_transaction<Transaction: PoolTransaction>(
//     expected_nonces: &mut HashMap<Address, u64>,
//     proposal_transactions: &mut Vec<Transaction>,
//     pending_txs: &mut Vec<Transaction>,
//     tx: Transaction,
// ) {
//     let sender = tx.sender();
//     let nonce = tx.nonce();
//     // Expected nonce
//     let expected_nonce = expected_nonces
//         .get(&sender)
//         .map(|nonce| *nonce)
//         .unwrap_or(0);
//     if expected_nonce == nonce {
//         debug!(
//             "Processing transaction: {:?} with sender: {:?}. Nonce: {:?}. Put to proposal transactions",
//             hex::encode(tx.hash()),
//             sender,
//             nonce
//         );
//         proposal_transactions.push(tx);
//         expected_nonces
//             .get_mut(&sender)
//             .map(|nonce| *nonce = expected_nonce + 1);
//     } else {
//         if expected_nonce < nonce {
//             //Nonce is not consecutive some transactions are missing
//             debug!(
//                 "Sender: {:?}. Nonce is not consecutive {:?} < {:?} some transactions are missing. Pushing to pending transactions: {:?}",
//                 sender,
//                 expected_nonce,
//                 nonce,
//                 hex::encode(tx.hash())
//             );
//             pending_txs.push(tx);
//         } else {
//             warn!("See to old transaction: {:?} with nonce: {:?}. Expected nonce from cached state: {:?}", hex::encode(tx.hash()), nonce, expected_nonce);
//         }
//     }
// }
