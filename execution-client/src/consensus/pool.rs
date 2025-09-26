//! This module contains the implementation of the block creation from committed subdag.
//! Transactions in the subdag may are not ordered by nonce.
//! Or is not cons

use alloy_consensus::Transaction;
use alloy_primitives::Address;
use reth_ethereum::storage::StateProviderBox;
use reth_extension::CommittedTransactions;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use std::collections::{hash_map::Entry, HashMap, VecDeque};
use tracing::{debug, warn};

pub struct ConsensusPool<Pool: TransactionPool>
where
    Pool: TransactionPool,
{
    //Store committed transactions (converted from subdag) in queue
    pub commited_queue: VecDeque<CommittedTransactions<Pool::Transaction>>,
    // Transactions are not included into last payload due to missing of ancestors
    // pub pending_transactions: Vec<Pool::Transaction>,
    // latest_state: Option<StateProviderBox>,
}

impl<Pool: TransactionPool> ConsensusPool<Pool>
where
    Pool: TransactionPool,
{
    pub fn new() -> Self {
        Self {
            commited_queue: VecDeque::new(),
        }
    }
    pub fn queue_size(&self) -> usize {
        self.commited_queue.len()
    }
}

impl<Pool: TransactionPool> ConsensusPool<Pool>
where
    Pool: TransactionPool,
{
    pub fn add_committed_transactions(
        &mut self,
        committed_transactions: CommittedTransactions<Pool::Transaction>,
    ) {
        self.commited_queue.push_back(committed_transactions);
    }
    // pub fn set_latest_state(&mut self, latest_state: StateProviderBox) {
    //     self.latest_state = Some(latest_state);
    // }
    /// Loop through the pending transactions and next commited subdag in the queue and get the expected nonces for each sender
    /// Current version, expected nonces is min nonce of pending transactions and transactions in the next commited subdag
    // fn get_expected_nonces(&self) -> HashMap<Address, u64> {
    //     let mut expected_nonces = HashMap::<Address, u64>::new();
    //     for tx in self.pending_transactions.iter() {
    //         let sender = tx.sender_ref();
    //         let nonce = tx.nonce();
    //         if let Some(latest_state) = self.latest_state.as_ref() {
    //             let nonce = latest_state
    //                 .account_nonce(sender)
    //                 .ok()
    //                 .flatten()
    //                 .unwrap_or(0);
    //             debug!(
    //                 "Latest state# Nonce for sender: {:?}. Nonce: {:?}",
    //                 sender, nonce
    //             );
    //         }
    //         match expected_nonces.entry(sender.clone()) {
    //             Entry::Occupied(mut entry) => {
    //                 if entry.get() > &nonce {
    //                     *entry.get_mut() = nonce;
    //                 }
    //             }
    //             Entry::Vacant(entry) => {
    //                 entry.insert(nonce);
    //             }
    //         }
    //     }
    //     if let Some(committed_transactions) = self.commited_queue.front() {
    //         for tx in committed_transactions.transactions.iter() {
    //             let sender = tx.sender_ref();
    //             let nonce = tx.nonce();
    //             match expected_nonces.entry(sender.clone()) {
    //                 Entry::Occupied(mut entry) => {
    //                     if entry.get() > &nonce {
    //                         *entry.get_mut() = nonce;
    //                     }
    //                 }
    //                 Entry::Vacant(entry) => {
    //                     entry.insert(nonce);
    //                 }
    //             }
    //         }
    //     }
    //     expected_nonces
    // }
    /// Get the valid proposal transactions from the consensus pool
    /// This transactions are ordered and consecutive by nonce
    /// TODO: review orderer algorithm
    pub fn get_proposal_transactions(&mut self) -> Vec<Pool::Transaction> {
        if let Some(committed_transactions) = self.commited_queue.pop_front() {
            let CommittedTransactions { transactions, .. } = committed_transactions;
            //This transactions are ordered and consecutive by nonce
            debug!(
                "Committed transactions: {:?}. Number of transactions: {:?}",
                committed_transactions.leader,
                transactions.len()
            );
            return transactions;
        }
        return Vec::new();
    }
    // pub fn get_proposal_transactions(&mut self) -> Vec<Pool::Transaction> {
    //     let mut proposal_transactions = Vec::new();
    //     //this pending transactions stores all transactions whose nonce is not the next nonce of the sender

    //     if let Some(committed_transactions) = self.commited_queue.pop_front() {
    //         //Initially get all current pending transactions
    //         let mut pending_txs = self.pending_transactions.clone();
    //         debug!(
    //             "Processing committed transactions: {:?} from leader: {:?}. Current number of pending transactions: {:?}. Building proposal transactions",
    //             committed_transactions.transactions.len(),
    //             committed_transactions.leader,
    //             pending_txs.len()
    //         );
    //         //Get expected nonce for each sender
    //         let mut expected_nonces = self.get_expected_nonces();

    //         for tx in committed_transactions.transactions.into_iter() {
    //             try_add_transaction(
    //                 &mut expected_nonces,
    //                 &mut proposal_transactions,
    //                 &mut pending_txs,
    //                 tx,
    //             )
    //         }
    //         let mut pending_size = pending_txs.len();
    //         //Try pick consecutive transactions from pending transactions
    //         while pending_size > 0 {
    //             let mut new_pending_txs = Vec::<Pool::Transaction>::new();
    //             for tx in pending_txs {
    //                 try_add_transaction(
    //                     &mut expected_nonces,
    //                     &mut proposal_transactions,
    //                     &mut new_pending_txs,
    //                     tx,
    //                 )
    //             }
    //             pending_txs = new_pending_txs;
    //             if pending_size == pending_txs.len() {
    //                 //No more transactions can be picked
    //                 debug!(
    //                     "No more transactions can be picked. Number of pending transactions: {:?}",
    //                     pending_txs.len()
    //                 );
    //                 break;
    //             } else {
    //                 //Some transactions are picked, we continue to pick the next round
    //                 pending_size = pending_txs.len();
    //                 debug!("Some transactions are picked, new pending transactions: {:?}. We continue to pick the next round",
    //                 pending_txs.len());
    //             }
    //         }
    //         // Overwrite pending transactions.
    //         // This field contains all old pending transactions and new pending transactions from current
    //         self.pending_transactions = pending_txs;
    //     } else {
    //         debug!("No committed transactions to process");
    //     }
    //     proposal_transactions
    // }
}

fn try_add_transaction<Transaction: PoolTransaction>(
    expected_nonces: &mut HashMap<Address, u64>,
    proposal_transactions: &mut Vec<Transaction>,
    pending_txs: &mut Vec<Transaction>,
    tx: Transaction,
) {
    let sender = tx.sender();
    let nonce = tx.nonce();
    // Expected nonce
    let expected_nonce = expected_nonces
        .get(&sender)
        .map(|nonce| *nonce)
        .unwrap_or(0);
    if expected_nonce == nonce {
        debug!(
            "Processing transaction: {:?} with sender: {:?}. Nonce: {:?}. Put to proposal transactions",
            hex::encode(tx.hash()),
            sender,
            nonce
        );
        proposal_transactions.push(tx);
        expected_nonces
            .get_mut(&sender)
            .map(|nonce| *nonce = expected_nonce + 1);
    } else {
        if expected_nonce < nonce {
            //Nonce is not consecutive some transactions are missing
            debug!(
                "Sender: {:?}. Nonce is not consecutive {:?} < {:?} some transactions are missing. Pushing to pending transactions: {:?}",
                sender,
                expected_nonce,
                nonce,
                hex::encode(tx.hash())
            );
            pending_txs.push(tx);
        } else {
            warn!("See to old transaction: {:?} with nonce: {:?}. Expected nonce from cached state: {:?}", hex::encode(tx.hash()), nonce, expected_nonce);
        }
    }
}
