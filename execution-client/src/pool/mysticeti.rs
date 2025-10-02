use crate::pool::Pool;

use reth_extension::MysticetiCommittedSubdag;
use reth_transaction_pool::{
    BlobStore, EthPoolTransaction, PoolTransaction, TransactionOrdering, TransactionValidator,
};
pub trait MysticetiPool {
    type Transaction: PoolTransaction;
    fn add_committed_subdags(&self, committed_subdags: Vec<MysticetiCommittedSubdag<Self::T>>);
}
