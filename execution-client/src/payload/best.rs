use reth_transaction_pool::{error::InvalidPoolTransactionError, BestTransactions, PoolTransaction};
use std::{collections::VecDeque, sync::Arc};

pub struct BestMysticetiTransactions<T: PoolTransaction> {
    //reth_best_txs: Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<T>>>>,
    mysticeti_txs: VecDeque<Arc<T>>,
}

impl<T: PoolTransaction> BestMysticetiTransactions<T> {
    pub fn new(
        //reth_best_txs: Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<T>>>>,
        mysticeti_txs: VecDeque<Arc<T>>,
    ) -> Self {
        Self {
            //reth_best_txs,
            mysticeti_txs,
        }
    }

    /// Returns the number of remaining transactions
    #[cfg(test)]
    pub fn remaining_count(&self) -> usize {
        self.mysticeti_txs.len()
    }
}

impl<T: PoolTransaction> BestTransactions for BestMysticetiTransactions<T> {
    fn mark_invalid(&mut self, _transaction: &Self::Item, _kind: InvalidPoolTransactionError) {
        // debug!(
        //     "Mark invalid transaction {:?}: sender {:?}, nonce {:?}, Invalid reason: {:?}",
        //     transaction.hash(),
        //     transaction.sender_ref(),
        //     transaction.nonce(),
        //     kind
        // );
        // self.reth_best_txs.mark_invalid(transaction, kind);
    }

    fn no_updates(&mut self) {
        // self.reth_best_txs.no_updates();
    }

    fn set_skip_blobs(&mut self, _skip_blobs: bool) {
        // self.reth_best_txs.set_skip_blobs(skip_blobs);
    }
}
impl<T: PoolTransaction> Iterator for BestMysticetiTransactions<T> {
    type Item = Arc<T>;

    fn next(&mut self) -> Option<Self::Item> {
        //1 Pick first transaction from mysticeti transactions
        let consensus_tx = self.mysticeti_txs.pop_front()?;
        Some(consensus_tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_consensus::Transaction;
    use alloy_primitives::{Address, Bytes, TxHash, U256};
    use reth_ethereum::pool::noop::NoopTransactionPool;
    use reth_ethereum::rpc::eth::utils::recover_raw_transaction;
    use reth_transaction_pool::TransactionPool;
    use std::sync::Arc;

    type TestPool = NoopTransactionPool;
    type TestTransaction = <TestPool as TransactionPool>::Transaction;

    /// Helper function to create a mock transaction for testing
    fn create_mock_transaction(
        sender: Address,
        nonce: u64,
    ) -> Arc<TestTransaction> {
        use alloy_network::{eip2718::Encodable2718, EthereumWallet, TransactionBuilder};
        use alloy_rpc_types_eth::TransactionRequest;
        use sha2::{Digest, Sha256};
        use tokio::runtime::Runtime;

        let key_hash = Sha256::digest(&sender[..]);

        use secp256k1::SecretKey;
        let secret_key = SecretKey::from_slice(&key_hash).unwrap_or_else(|_| {
            let mut modified = key_hash;
            modified[0] = modified[0].wrapping_add(1);
            SecretKey::from_slice(&modified).expect("Failed to create valid secret key")
        });

        let rt = Runtime::new().unwrap();

        rt.block_on(async {
            use alloy_primitives::FixedBytes;
            use alloy_signer_local::PrivateKeySigner;

            let key_bytes: FixedBytes<32> = FixedBytes::from(secret_key.secret_bytes());
            let signer = PrivateKeySigner::from_bytes(&key_bytes).unwrap();

            let tx_req = TransactionRequest::default()
                .with_to(Address::ZERO)
                .with_value(U256::ZERO)
                .with_gas_limit(21000)
                .with_max_priority_fee_per_gas(1_000_000_000u128)
                .with_max_fee_per_gas(20_000_000_000u128)
                .with_nonce(nonce)
                .with_chain_id(1);

            let ethereum_wallet = EthereumWallet::from(signer);
            let tx_envelope = tx_req.build(&ethereum_wallet).await.unwrap();

            let encoded = tx_envelope.encoded_2718();
            
            let recovered = recover_raw_transaction(&alloy_primitives::Bytes::from(encoded))
                .expect("Failed to recover transaction");
            TestTransaction::from_pooled(recovered)
        })
        .into()
    }

    #[test]
    fn test_best_mysticeti_transactions_new_empty() {
        let txs: VecDeque<Arc<TestTransaction>> = VecDeque::new();
        let best = BestMysticetiTransactions::new(txs);
        
        assert_eq!(best.remaining_count(), 0);
    }

    #[test]
    fn test_best_mysticeti_transactions_new_with_items() {
        let sender = Address::from([1u8; 20]);
        let tx1 = create_mock_transaction(sender, 0);
        let tx2 = create_mock_transaction(sender, 1);
        
        let mut txs = VecDeque::new();
        txs.push_back(tx1);
        txs.push_back(tx2);
        
        let best = BestMysticetiTransactions::new(txs);
        
        assert_eq!(best.remaining_count(), 2);
    }

    #[test]
    fn test_best_mysticeti_transactions_iterator_empty() {
        let txs: VecDeque<Arc<TestTransaction>> = VecDeque::new();
        let mut best = BestMysticetiTransactions::new(txs);
        
        assert!(best.next().is_none());
    }

    #[test]
    fn test_best_mysticeti_transactions_iterator_single() {
        let sender = Address::from([1u8; 20]);
        let tx = create_mock_transaction(sender, 0);
        let tx_nonce = tx.nonce();
        
        let mut txs = VecDeque::new();
        txs.push_back(tx);
        
        let mut best = BestMysticetiTransactions::new(txs);
        
        let first = best.next();
        assert!(first.is_some());
        assert_eq!(first.unwrap().nonce(), tx_nonce);
        
        assert!(best.next().is_none());
    }

    #[test]
    fn test_best_mysticeti_transactions_iterator_order() {
        let sender1 = Address::from([1u8; 20]);
        let sender2 = Address::from([2u8; 20]);
        
        let tx1 = create_mock_transaction(sender1, 0);
        let tx2 = create_mock_transaction(sender2, 0);
        let tx3 = create_mock_transaction(sender1, 1);
        
        let tx1_nonce = tx1.nonce();
        let tx2_nonce = tx2.nonce();
        let tx3_nonce = tx3.nonce();
        
        let mut txs = VecDeque::new();
        txs.push_back(tx1);
        txs.push_back(tx2);
        txs.push_back(tx3);
        
        let mut best = BestMysticetiTransactions::new(txs);
        
        // First transaction should be tx1
        let first = best.next().unwrap();
        assert_eq!(first.nonce(), tx1_nonce);
        
        // Second transaction should be tx2
        let second = best.next().unwrap();
        assert_eq!(second.nonce(), tx2_nonce);
        
        // Third transaction should be tx3
        let third = best.next().unwrap();
        assert_eq!(third.nonce(), tx3_nonce);
        
        // No more transactions
        assert!(best.next().is_none());
    }

    #[test]
    fn test_best_mysticeti_transactions_collect() {
        let sender = Address::from([1u8; 20]);
        
        let tx1 = create_mock_transaction(sender, 0);
        let tx2 = create_mock_transaction(sender, 1);
        let tx3 = create_mock_transaction(sender, 2);
        
        let mut txs = VecDeque::new();
        txs.push_back(tx1);
        txs.push_back(tx2);
        txs.push_back(tx3);
        
        let best = BestMysticetiTransactions::new(txs);
        
        let collected: Vec<_> = best.collect();
        assert_eq!(collected.len(), 3);
    }

    #[test]
    fn test_best_mysticeti_transactions_mark_invalid_no_panic() {
        let sender = Address::from([1u8; 20]);
        let tx = create_mock_transaction(sender, 0);
        
        let mut txs = VecDeque::new();
        txs.push_back(tx.clone());
        
        let mut best = BestMysticetiTransactions::new(txs);
        
        // mark_invalid should not panic and is a no-op
        best.mark_invalid(
            &tx,
            InvalidPoolTransactionError::ExceedsGasLimit(21000, 20000),
        );
        
        // Transaction should still be available
        assert!(best.next().is_some());
    }

    #[test]
    fn test_best_mysticeti_transactions_no_updates_no_panic() {
        let txs: VecDeque<Arc<TestTransaction>> = VecDeque::new();
        let mut best = BestMysticetiTransactions::new(txs);
        
        // no_updates should not panic
        best.no_updates();
    }

    #[test]
    fn test_best_mysticeti_transactions_set_skip_blobs_no_panic() {
        let txs: VecDeque<Arc<TestTransaction>> = VecDeque::new();
        let mut best = BestMysticetiTransactions::new(txs);
        
        // set_skip_blobs should not panic
        best.set_skip_blobs(true);
        best.set_skip_blobs(false);
    }

    #[test]
    fn test_best_transactions_trait_methods() {
        let sender = Address::from([1u8; 20]);
        let tx = create_mock_transaction(sender, 0);
        
        let mut txs = VecDeque::new();
        txs.push_back(tx.clone());
        
        let mut best: Box<dyn BestTransactions<Item = Arc<TestTransaction>>> =
            Box::new(BestMysticetiTransactions::new(txs));
        
        // Test BestTransactions methods via trait object
        best.no_updates();
        best.set_skip_blobs(true);
        
        let item = best.next();
        assert!(item.is_some());
    }
}
