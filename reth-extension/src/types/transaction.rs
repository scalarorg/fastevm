use alloy_rlp::Decodable;
use consensus_config::AuthorityIndex;
use consensus_core::{BlockRef, CommitRef};
use jsonrpsee::SubscriptionMessage;
use reth_ethereum::primitives::Recovered;
use reth_transaction_pool::{PoolTransaction, ValidPoolTransaction};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, sync::Arc};

use crate::CommittedSubDag;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommittedTransactions<Transaction> {
    pub leader: BlockRef,
    pub transactions: Vec<Arc<Transaction>>,
    pub timestamp_ms: u64,
    pub commit_ref: CommitRef,
    pub reputation_scores_desc: Vec<(AuthorityIndex, u64)>,
}

impl<Transaction> TryFrom<CommittedSubDag> for CommittedTransactions<Transaction>
where
    Transaction: PoolTransaction,
{
    type Error = anyhow::Error;
    fn try_from(subdag: CommittedSubDag) -> Result<Self, anyhow::Error> {
        let CommittedSubDag {
            leader,
            blocks,
            timestamp_ms,
            commit_ref,
            reputation_scores_desc,
        } = subdag;
        let mut transactions = Vec::new();
        for block in blocks {
            for tx in block.block.transactions().into_iter() {
                let tx_data = tx.data().to_vec();
                let transaction = decode_transaction::<Transaction>(&mut tx_data.as_slice())?;
                transactions.push(Arc::new(transaction));
            }
        }
        //TODO: Improve ordering algorithm
        transactions.sort_by(|tx1, tx2| {
            if tx1.sender() == tx2.sender() {
                return tx1.nonce().cmp(&tx2.nonce());
            } else {
                //We don't care about the order of different senders
                return Ordering::Equal;
            }
        });
        // let subdag_txs = decode_transactions::<Transaction>(transactions)
        //     .map_err(|e| ErrorObjectOwned::owned(PARSE_ERROR_CODE, e.to_string(), None::<()>))?;
        // // Add subdag transactions to the pool
        // let pool = self.pool.clone();
        // let txs = subdag_txs.clone();
        // tokio::spawn(Box::pin(async move {
        //     let mut missing_transactions = Vec::new();
        //     for tx in txs.iter() {
        //         let tx_hash = tx.hash();
        //         if !pool.contains(tx_hash) {
        //             let highest_nonce = pool
        //                 .get_highest_transaction_by_sender(tx.sender())
        //                 .map(|tx| tx.nonce());
        //             debug!(
        //                 "Adding subdag transaction to pool: {:?}, sender: {:?}, nonce: {:?}, highest nonce: {:?}",
        //                 tx_hash,
        //                 tx.sender_ref(),
        //                 tx.nonce(),
        //                 highest_nonce
        //             );
        //             missing_transactions.push(tx.clone());
        //         } else {
        //             debug!("Transaction already in pool: {:?}", tx_hash);
        //         }
        //     }
        //     let result = pool
        //         .add_transactions(TransactionOrigin::External, missing_transactions)
        //         .await;
        //     debug!("Result of adding transactions to pool: {:?}", result);
        // }));
        Ok(Self {
            leader,
            transactions,
            timestamp_ms,
            commit_ref,
            reputation_scores_desc,
        })
    }
}

pub fn encode_transactions<T: PoolTransaction>(
    batch: Vec<Arc<ValidPoolTransaction<T>>>,
) -> SubscriptionMessage {
    let batch_data = batch
        .iter()
        .map(|tx| {
            // let consensus_transaction = ConsensusTransaction::from(Arc::clone(tx));
            // match bincode::encode_to_vec(consensus_transaction, bincode::config::standard()) {
            //     Ok(encoded) => encoded,
            //     Err(e) => {
            //         tracing::debug!("Failed to encode consensus transaction: {}", e);
            //         return Vec::new();
            //     }
            // }
            let consensus_transaction = tx.transaction.clone_into_consensus();
            consensus_transaction
                .into_encoded()
                .into_encoded_bytes()
                .to_vec()
        })
        .collect::<Vec<Vec<u8>>>();
    SubscriptionMessage::from(
        serde_json::value::to_raw_value(&batch_data).expect("serialize batch"),
    )
}

pub fn decode_transaction<T: PoolTransaction>(tx_data: &mut &[u8]) -> Result<T, anyhow::Error> {
    let recovered_transaction = Recovered::<<T as PoolTransaction>::Consensus>::decode(tx_data)
        .map_err(|e| anyhow::anyhow!("Failed to decode consensus transaction: {}", e))?;
    let transaction = T::try_from_consensus(recovered_transaction).map_err(|e| {
        anyhow::anyhow!(
            "Failed to convert consensus transaction to pool transaction: {}",
            e
        )
    })?;
    //let pool_transaction = ValidPoolTransaction::<T>::try_from(consensus_tx)?;
    Ok(transaction)
}

pub fn decode_transactions<T: PoolTransaction>(txs: Vec<Vec<u8>>) -> Result<Vec<T>, anyhow::Error> {
    txs.into_iter()
        .map(|tx_bytes| {
            // Create a Transaction from the raw bytes
            // Based on the codebase, it seems like Transaction might be a simple wrapper around Vec<u8>
            // let (consensus_tx, _bytes_read): (ConsensusTransaction, usize) =
            //     bincode::decode_from_slice(&tx_bytes.as_slice(), standard())
            //         .map_err(|e| format!("Failed to decode: {}", e))?;
            let recovered_transaction =
                Recovered::<<T as PoolTransaction>::Consensus>::decode(&mut tx_bytes.as_slice())
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to decode consensus transaction: {}", e)
                    })?;
            let transaction = T::try_from_consensus(recovered_transaction).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to convert consensus transaction to pool transaction: {}",
                    e
                )
            })?;
            //let pool_transaction = ValidPoolTransaction::<T>::try_from(consensus_tx)?;
            Ok(transaction)
        })
        .collect()
}

// impl ConsensusTransactions {
//     /// Convert the u64 timestamp to an Instant
//     pub fn timestamp_as_instant(&self) -> Instant {
//         Instant::now() - Duration::from_secs(self.timestamp)
//     }

//     /// Create a new ConsensusTransaction with current timestamp
//     pub fn with_current_timestamp(mut self) -> Self {
//         self.timestamp = 0; // Current time
//         self
//     }

//     /// Create a new ConsensusTransaction with a specific Instant
//     pub fn with_instant_timestamp(mut self, instant: Instant) -> Self {
//         self.timestamp = instant.elapsed().as_secs();
//         self
//     }
// }

// impl<T: PoolTransaction> From<Arc<ValidPoolTransaction<T>>> for ConsensusTransaction {
//     fn from(tx: Arc<ValidPoolTransaction<T>>) -> Self {
//         //Todo: extract sender from tx,
//         let sender = 0;
//         let authority_ids = tx
//             .authority_ids
//             .as_ref()
//             .map(|ids| ids.into_iter().map(|_id| 0).collect());
//         let is_local = match tx.origin {
//             TransactionOrigin::Local => true,
//             _ => false,
//         };
//         let consensus_transaction = tx.transaction.clone_into_consensus();
//         let transaction = consensus_transaction
//             .into_encoded()
//             .into_encoded_bytes()
//             .to_vec();
//         Self {
//             transaction,
//             sender,
//             nonce: tx.transaction.nonce(),
//             propagate: tx.propagate,
//             timestamp: tx.timestamp.elapsed().as_secs(),
//             is_local,
//             authority_ids,
//         }
//     }
// }

// impl<T: PoolTransaction> TryFrom<ConsensusTransaction> for ValidPoolTransaction<T> {
//     type Error = Box<dyn std::error::Error>;

//     fn try_from(tx: ConsensusTransaction) -> Result<Self, Self::Error> {
//         let ConsensusTransaction {
//             transaction,
//             sender,
//             nonce,
//             propagate,
//             timestamp,
//             is_local,
//             authority_ids,
//         } = tx;

//         let recovered_transaction =
//             Recovered::<<T as PoolTransaction>::Consensus>::decode(&mut transaction.as_slice())
//                 .map_err(|e| format!("Failed to decode consensus transaction: {}", e))?;
//         let transaction = T::try_from_consensus(recovered_transaction).map_err(|e| {
//             format!(
//                 "Failed to convert consensus transaction to pool transaction: {}",
//                 e
//             )
//         })?;

//         let origin = if is_local {
//             TransactionOrigin::Local
//         } else {
//             TransactionOrigin::External
//         };
//         let sender_id = SenderId::from(sender);
//         Ok(ValidPoolTransaction {
//             transaction,
//             transaction_id: sender_id.into_transaction_id(nonce),
//             propagate,
//             timestamp: Instant::now() - Duration::from_secs(timestamp),
//             origin,
//             authority_ids: authority_ids
//                 .map(|ids| ids.into_iter().map(|id| SenderId::from(id)).collect()),
//         })
//     }
// }
