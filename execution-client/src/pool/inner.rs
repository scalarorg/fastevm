use crate::pool::{listener::PoolEventBroadcast, MysticetiPool, MysticetiPoolConfig};

use super::{MysticetiTransactionPool, TxPool};
use alloy_consensus::Typed2718;
use alloy_eips::eip7594::BlobTransactionSidecarVariant;
use alloy_primitives::{Address, TxHash};
use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use reth_eth_wire_types::HandleMempoolData;
use reth_ethereum::primitives::Recovered;
use reth_extension::MysticetiCommittedSubdag;
use reth_primitives_traits::Block;
use reth_provider::ChangedAccount;
use reth_transaction_pool::{
    error::{PoolError, PoolErrorKind},
    identifier::{SenderId, SenderIdentifiers, TransactionId},
    metrics::BlobStoreMetrics,
    validate::ValidTransaction,
    AllPoolTransactions, AllTransactionsEvents, BestTransactions, BestTransactionsAttributes,
    BlobStore, BlockInfo, CanonicalStateUpdate, EthPoolTransaction, GetPooledTransactionLimit,
    NewBlobSidecar, NewTransactionEvent, PoolConfig, PoolResult, PoolSize, PoolTransaction,
    PropagatedTransactions, TransactionEvents, TransactionListenerKind, TransactionOrdering,
    TransactionOrigin, TransactionValidationOutcome, TransactionValidator, ValidPoolTransaction,
};
use rustc_hash::FxHashMap;
use std::{collections::HashSet, fmt, sync::Arc, time::Instant};
use tokio::sync::mpsc;
use tracing::{debug, trace};

// Define the missing types that were previously imported from private modules
#[derive(Debug)]
pub struct BlobTransactionSidecarListener {
    pub sender: mpsc::Sender<NewBlobSidecar>,
}

#[derive(Debug)]
pub struct PendingTransactionHashListener {
    pub sender: mpsc::Sender<TxHash>,
    pub kind: TransactionListenerKind,
}

#[derive(Debug)]
pub struct TransactionListener<T: PoolTransaction> {
    pub sender: mpsc::Sender<NewTransactionEvent<T>>,
    pub kind: TransactionListenerKind,
}

// #[derive(Debug)]
// pub struct PoolEventBroadcast<T: PoolTransaction> {
//     // Simplified implementation - you may need to expand this based on your needs
//     _phantom: std::marker::PhantomData<T>,
// }

// impl<T: PoolTransaction> Default for PoolEventBroadcast<T> {
//     fn default() -> Self {
//         Self {
//             _phantom: std::marker::PhantomData,
//         }
//     }
// }

// impl<T: PoolTransaction> PoolEventBroadcast<T> {
//     pub fn subscribe(&mut self, tx_hash: TxHash) -> TransactionEvents {
//         // Return a dummy implementation
//         let (_, receiver) = tokio::sync::mpsc::unbounded_channel();
//         TransactionEvents {
//             hash: tx_hash,
//             events: receiver,
//         }
//     }

//     pub fn subscribe_all(&mut self) -> AllTransactionsEvents<T> {
//         // Return a dummy implementation
//         let (_, receiver) = tokio::sync::mpsc::channel(100);
//         AllTransactionsEvents::new(receiver)
//     }

//     pub fn pending(&mut self, _tx_hash: &TxHash, _replaced: Option<TxHash>) {}
//     pub fn discarded(&mut self, _tx_hash: &TxHash) {}
//     pub fn invalid(&mut self, _tx_hash: &TxHash) {}
//     pub fn mined(&mut self, _tx: &ValidPoolTransaction<T>, _block_hash: Option<TxHash>) {}
// }

impl PendingTransactionHashListener {
    pub fn send_all(&mut self, hashes: impl Iterator<Item = TxHash>) -> bool {
        for hash in hashes {
            if self.sender.try_send(hash).is_err() {
                return false;
            }
        }
        true
    }
}

impl<T: PoolTransaction> TransactionListener<T> {
    pub fn send(&mut self, event: NewTransactionEvent<T>) -> bool {
        self.sender.try_send(event).is_ok()
    }

    pub fn send_all(&mut self, events: impl Iterator<Item = NewTransactionEvent<T>>) -> bool {
        for event in events {
            if self.sender.try_send(event).is_err() {
                return false;
            }
        }
        true
    }
}

#[derive(Debug)]
pub struct AddedPendingTransaction<T: PoolTransaction> {
    pub transaction: Arc<ValidPoolTransaction<T>>,
    pub promoted: Vec<Arc<ValidPoolTransaction<T>>>,
    pub discarded: Vec<Arc<ValidPoolTransaction<T>>>,
    pub replaced: Option<TxHash>,
}

#[derive(Debug)]
pub struct AddedTransaction<T: PoolTransaction> {
    pub transaction: Arc<ValidPoolTransaction<T>>,
}

#[derive(Debug)]
pub struct OnNewCanonicalStateOutcome<T: PoolTransaction> {
    pub mined: Vec<Arc<ValidPoolTransaction<T>>>,
    pub promoted: Vec<Arc<ValidPoolTransaction<T>>>,
    pub discarded: Vec<Arc<ValidPoolTransaction<T>>>,
    pub block_hash: Option<TxHash>,
}

impl<T: PoolTransaction> OnNewCanonicalStateOutcome<T> {
    pub fn pending_transactions(&self, _kind: TransactionListenerKind) -> Vec<TxHash> {
        self.promoted.iter().map(|tx| *tx.hash()).collect()
    }

    pub fn full_pending_transactions(
        &self,
        _kind: TransactionListenerKind,
    ) -> Vec<NewTransactionEvent<T>> {
        self.promoted
            .iter()
            .map(|tx| NewTransactionEvent {
                transaction: tx.clone(),
                subpool: reth_transaction_pool::SubPool::Pending,
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct SenderInfo {
    pub state_nonce: u64,
    pub balance: alloy_primitives::U256,
}

#[derive(Debug)]
pub struct UpdateOutcome<T: PoolTransaction> {
    pub promoted: Vec<Arc<ValidPoolTransaction<T>>>,
    pub discarded: Vec<Arc<ValidPoolTransaction<T>>>,
}

/// Bound on number of pending transactions from `reth_network::TransactionsManager` to buffer.
pub const PENDING_TX_LISTENER_BUFFER_SIZE: usize = 2048;
/// Bound on number of new transactions from `reth_network::TransactionsManager` to buffer.
pub const NEW_TX_LISTENER_BUFFER_SIZE: usize = 1024;

const BLOB_SIDECAR_LISTENER_BUFFER_SIZE: usize = 512;

/// Transaction pool internals.
pub struct PoolInner<V, T, S>
where
    T: TransactionOrdering,
{
    /// Internal mapping of addresses to plain ints.
    identifiers: RwLock<SenderIdentifiers>,
    /// Transaction validator.
    validator: V,
    /// Storage for blob transactions
    blob_store: S,
    /// The internal pool that manages all transactions.
    pool: RwLock<TxPool<T::Transaction>>,
    /// Pool settings.
    config: MysticetiPoolConfig,
    /// Manages listeners for transaction state change events.
    event_listener: RwLock<PoolEventBroadcast<T::Transaction>>,
    /// Listeners for new _full_ pending transactions.
    pending_transaction_listener: Mutex<Vec<PendingTransactionHashListener>>,
    /// Listeners for new transactions added to the pool.
    transaction_listener: Mutex<Vec<TransactionListener<T::Transaction>>>,
    /// Listener for new blob transaction sidecars added to the pool.
    blob_transaction_sidecar_listener: Mutex<Vec<BlobTransactionSidecarListener>>,
    /// Metrics for the blob store
    blob_store_metrics: BlobStoreMetrics,
}

// === impl PoolInner ===

impl<V, T, S> PoolInner<V, T, S>
where
    V: TransactionValidator,
    T: TransactionOrdering<Transaction = <V as TransactionValidator>::Transaction>,
    S: BlobStore,
{
    /// Create a new transaction pool instance.
    pub fn new(validator: V, ordering: T, blob_store: S, config: MysticetiPoolConfig) -> Self {
        Self {
            identifiers: Default::default(),
            validator,
            event_listener: Default::default(),
            pool: RwLock::new(TxPool::new(ordering, config.clone())),
            pending_transaction_listener: Default::default(),
            transaction_listener: Default::default(),
            blob_transaction_sidecar_listener: Default::default(),
            config,
            blob_store,
            blob_store_metrics: Default::default(),
        }
    }

    /// Returns the configured blob store.
    pub const fn blob_store(&self) -> &S {
        &self.blob_store
    }

    /// Returns stats about the size of the pool.
    pub fn size(&self) -> PoolSize {
        let data = self.get_pool_data();
        let total = data.len();
        PoolSize {
            pending: total,
            queued: 0,
            total,
            basefee: 0,
            basefee_size: 0,
            blob: 0,
            blob_size: 0,
            pending_size: 0,
            queued_size: 0,
        }
    }

    /// Returns the currently tracked block
    pub fn block_info(&self) -> BlockInfo {
        self.get_pool_data().block_info()
    }
    /// Sets the currently tracked block
    pub fn set_block_info(&self, info: BlockInfo) {
        self.pool.write().set_block_info(info)
    }

    /// Returns the internal [`SenderId`] for this address
    pub fn get_sender_id(&self, addr: Address) -> SenderId {
        self.identifiers.write().sender_id_or_create(addr)
    }

    /// Returns the internal [`SenderId`]s for the given addresses.
    pub fn get_sender_ids(&self, addrs: impl IntoIterator<Item = Address>) -> Vec<SenderId> {
        self.identifiers.write().sender_ids_or_create(addrs)
    }

    /// Returns all senders in the pool
    pub fn unique_senders(&self) -> HashSet<Address> {
        // Simplified implementation - return empty set for now
        HashSet::new()
    }

    /// Converts the changed accounts to a map of sender ids to sender info (internal identifier
    /// used for accounts)
    fn changed_senders(
        &self,
        accs: impl Iterator<Item = ChangedAccount>,
    ) -> FxHashMap<SenderId, SenderInfo> {
        let mut identifiers = self.identifiers.write();
        accs.into_iter()
            .map(|acc| {
                let ChangedAccount {
                    address,
                    nonce,
                    balance,
                } = acc;
                let sender_id = identifiers.sender_id_or_create(address);
                (
                    sender_id,
                    SenderInfo {
                        state_nonce: nonce,
                        balance,
                    },
                )
            })
            .collect()
    }

    /// Get the config the pool was configured with.
    pub const fn config(&self) -> &MysticetiPoolConfig {
        &self.config
    }

    /// Get the validator reference.
    pub const fn validator(&self) -> &V {
        &self.validator
    }

    /// Adds a new transaction listener to the pool that gets notified about every new _pending_
    /// transaction inserted into the pool
    pub fn add_pending_listener(&self, kind: TransactionListenerKind) -> mpsc::Receiver<TxHash> {
        let (sender, rx) = mpsc::channel(self.config.pool_config.max_new_pending_txs_notifications);
        let listener = PendingTransactionHashListener { sender, kind };
        self.pending_transaction_listener.lock().push(listener);
        rx
    }

    /// Adds a new transaction listener to the pool that gets notified about every new transaction.
    pub fn add_new_transaction_listener(
        &self,
        kind: TransactionListenerKind,
    ) -> mpsc::Receiver<NewTransactionEvent<T::Transaction>> {
        let (sender, rx) = mpsc::channel(self.config.pool_config.max_new_pending_txs_notifications);
        let listener = TransactionListener { sender, kind };
        self.transaction_listener.lock().push(listener);
        rx
    }
    /// Adds a new blob sidecar listener to the pool that gets notified about every new
    /// eip4844 transaction's blob sidecar.
    pub fn add_blob_sidecar_listener(&self) -> mpsc::Receiver<NewBlobSidecar> {
        let (sender, rx) = mpsc::channel(BLOB_SIDECAR_LISTENER_BUFFER_SIZE);
        let listener = BlobTransactionSidecarListener { sender };
        self.blob_transaction_sidecar_listener.lock().push(listener);
        rx
    }

    /// If the pool contains the transaction, this adds a new listener that gets notified about
    /// transaction events.
    pub fn add_transaction_event_listener(&self, tx_hash: TxHash) -> Option<TransactionEvents> {
        self.get_pool_data()
            .contains(&tx_hash)
            .then(|| self.event_listener.write().subscribe(tx_hash))
    }

    /// Adds a listener for all transaction events.
    pub fn add_all_transactions_event_listener(
        &self,
    ) -> reth_transaction_pool::AllTransactionsEvents<T::Transaction> {
        let (_, receiver) = tokio::sync::mpsc::channel(100);
        reth_transaction_pool::AllTransactionsEvents::new(receiver)
    }

    /// Returns a read lock to the pool's data.
    pub fn get_pool_data(&self) -> RwLockReadGuard<'_, TxPool<T::Transaction>> {
        self.pool.read()
    }

    /// Returns hashes of _all_ transactions in the pool.
    pub fn pooled_transactions_hashes(&self) -> Vec<TxHash> {
        // Simplified implementation - return empty vec for now
        Vec::new()
    }

    /// Returns _all_ transactions in the pool.
    pub fn pooled_transactions(&self) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        // Simplified implementation - return empty vec for now
        Vec::new()
    }

    /// Returns only the first `max` transactions in the pool.
    pub fn pooled_transactions_max(
        &self,
        _max: usize,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        // Simplified implementation - return empty vec for now
        Vec::new()
    }

    /// Converts the internally tracked transaction to the pooled format.
    ///
    /// If the transaction is an EIP-4844 transaction, the blob sidecar is fetched from the blob
    /// store and attached to the transaction.
    fn to_pooled_transaction(
        &self,
        transaction: Arc<ValidPoolTransaction<T::Transaction>>,
    ) -> Option<Recovered<<<V as TransactionValidator>::Transaction as PoolTransaction>::Pooled>>
    where
        <V as TransactionValidator>::Transaction: EthPoolTransaction,
    {
        if transaction.is_eip4844() {
            let sidecar = self.blob_store.get(*transaction.hash()).ok()??;
            transaction
                .transaction
                .clone()
                .try_into_pooled_eip4844(sidecar)
        } else {
            transaction
                .transaction
                .clone()
                .try_into_pooled()
                .inspect_err(|err| {
                    debug!(
                        target: "txpool", %err,
                        "failed to convert transaction to pooled element; skipping",
                    );
                })
                .ok()
        }
    }

    /// Returns pooled transactions for the given transaction hashes.
    pub fn get_pooled_transaction_elements(
        &self,
        tx_hashes: Vec<TxHash>,
        limit: GetPooledTransactionLimit,
    ) -> Vec<<<V as TransactionValidator>::Transaction as PoolTransaction>::Pooled>
    where
        <V as TransactionValidator>::Transaction: EthPoolTransaction,
    {
        let transactions = self.get_all(tx_hashes);
        let mut elements = Vec::with_capacity(transactions.len());
        let mut size = 0;
        for transaction in transactions {
            let encoded_len = transaction.encoded_length();
            let Some(pooled) = self.to_pooled_transaction(transaction) else {
                continue;
            };

            size += encoded_len;
            elements.push(pooled.into_inner());

            if limit.exceeds(size) {
                break;
            }
        }

        elements
    }

    /// Returns converted pooled transaction for the given transaction hash.
    pub fn get_pooled_transaction_element(
        &self,
        tx_hash: TxHash,
    ) -> Option<Recovered<<<V as TransactionValidator>::Transaction as PoolTransaction>::Pooled>>
    where
        <V as TransactionValidator>::Transaction: EthPoolTransaction,
    {
        self.get(&tx_hash)
            .and_then(|tx| self.to_pooled_transaction(tx))
    }

    /// Updates the entire pool after a new block was executed.
    pub fn on_canonical_state_change<B>(&self, update: CanonicalStateUpdate<'_, B>)
    where
        B: Block,
    {
        trace!(target: "txpool", ?update, "updating pool on canonical state change");

        let block_info = update.block_info();
        let CanonicalStateUpdate {
            new_tip,
            changed_accounts,
            mined_transactions,
            update_kind,
            ..
        } = update;
        self.validator.on_new_head_block(new_tip);

        let changed_senders = self.changed_senders(changed_accounts.into_iter());

        // update the pool - simplified implementation
        let outcome = OnNewCanonicalStateOutcome {
            mined: Vec::new(),
            promoted: Vec::new(),
            discarded: Vec::new(),
            block_hash: Some(block_info.last_seen_block_hash),
        };

        // This will discard outdated transactions based on the account's nonce
        self.delete_discarded_blobs(outcome.discarded.iter());

        // notify listeners about updates
        self.notify_on_new_state(outcome);
    }

    /// Performs account updates on the pool.
    ///
    /// This will either promote or discard transactions based on the new account state.
    pub fn update_accounts(&self, accounts: Vec<ChangedAccount>) {
        let changed_senders = self.changed_senders(accounts.into_iter());
        let outcome = UpdateOutcome {
            promoted: Vec::new(),
            discarded: Vec::new(),
        };

        // Notify about promoted pending transactions (similar to notify_on_new_state)
        if !outcome.promoted.is_empty() {
            self.pending_transaction_listener
                .lock()
                .retain_mut(|listener| {
                    let kind = listener.kind;
                    let promoted_hashes = outcome.promoted.iter().filter_map(|tx| {
                        if kind.is_propagate_only() && !tx.propagate {
                            None
                        } else {
                            Some(*tx.hash())
                        }
                    });
                    listener.send_all(promoted_hashes)
                });
        }

        {
            let mut listener = self.event_listener.write();

            for tx in &outcome.promoted {
                listener.pending(tx.hash(), None);
            }
            for tx in &outcome.discarded {
                listener.discarded(tx.hash());
            }
        }

        // This deletes outdated blob txs from the blob store, based on the account's nonce. This is
        // called during txpool maintenance when the pool drifted.
        self.delete_discarded_blobs(outcome.discarded.iter());
    }

    /// Add a single validated transaction into the pool.
    ///
    /// Note: this is only used internally by [`Self::add_transactions()`], all new transaction(s)
    /// come in through that function, either as a batch or `std::iter::once`.
    fn add_transaction(
        &self,
        pool: &mut RwLockWriteGuard<'_, TxPool<T::Transaction>>,
        origin: TransactionOrigin,
        tx: TransactionValidationOutcome<T::Transaction>,
    ) -> PoolResult<TxHash> {
        match tx {
            TransactionValidationOutcome::Valid {
                balance,
                state_nonce,
                transaction,
                propagate,
                bytecode_hash,
                authorities,
            } => {
                // let sender_id = self.get_sender_id(transaction.sender());
                // TODO: change data structure
                let sender_id = SenderId::from(0);
                let transaction_id = TransactionId::new(sender_id, transaction.nonce());

                // split the valid transaction and the blob sidecar if it has any
                let (transaction, maybe_sidecar) = match transaction {
                    ValidTransaction::Valid(tx) => (tx, None),
                    ValidTransaction::ValidWithSidecar {
                        transaction,
                        sidecar,
                    } => {
                        debug_assert!(
                            transaction.is_eip4844(),
                            "validator returned sidecar for non EIP-4844 transaction"
                        );
                        (transaction, Some(sidecar))
                    }
                };

                let tx = ValidPoolTransaction {
                    transaction,
                    transaction_id,
                    propagate,
                    timestamp: Instant::now(),
                    origin,
                    authority_ids: authorities.map(|auths| self.get_sender_ids(auths)),
                };

                let hash = pool.add_transaction(tx, balance, state_nonce, bytecode_hash)?;

                // transaction was successfully inserted into the pool
                // if let Some(sidecar) = maybe_sidecar {
                //     // notify blob sidecar listeners
                //     self.on_new_blob_sidecar(&hash, &sidecar);
                //     // store the sidecar in the blob store
                //     self.insert_blob(hash, sidecar);
                // }

                // if let Some(replaced) = added.replaced_blob_transaction() {
                //     debug!(target: "txpool", "[{:?}] delete replaced blob sidecar", replaced);
                //     // delete the replaced transaction from the blob store
                //     self.delete_blob(replaced);
                // }

                // Notify about new pending transactions
                // if let Some(pending) = added.as_pending() {
                //     self.on_new_pending_transaction(pending);
                // }

                // Notify tx event listeners
                self.notify_event_listeners(&hash);
                // TODO: handle event listeners
                // if let Some(discarded) = added.discarded_transactions() {
                //     self.delete_discarded_blobs(discarded.iter());
                // }

                // // Notify listeners for _all_ transactions
                // self.on_new_transaction(added.into_new_transaction_event());

                Ok(hash)
            }
            TransactionValidationOutcome::Invalid(tx, err) => {
                let mut listener = self.event_listener.write();
                listener.invalid(tx.hash());
                Err(PoolError::new(*tx.hash(), err))
            }
            TransactionValidationOutcome::Error(tx_hash, err) => {
                let mut listener = self.event_listener.write();
                listener.discarded(&tx_hash);
                Err(PoolError::other(tx_hash, err))
            }
        }
    }

    /// Adds a transaction and returns the event stream.
    pub fn add_transaction_and_subscribe(
        &self,
        origin: TransactionOrigin,
        tx: TransactionValidationOutcome<T::Transaction>,
    ) -> PoolResult<TransactionEvents> {
        let listener = {
            let mut listener = self.event_listener.write();
            listener.subscribe(tx.tx_hash())
        };
        let mut results = self.add_transactions(origin, std::iter::once(tx));
        results
            .pop()
            .expect("result length is the same as the input")?;
        Ok(listener)
    }

    /// Adds all transactions in the iterator to the pool, returning a list of results.
    ///
    /// Note: A large batch may lock the pool for a long time that blocks important operations
    /// like updating the pool on canonical state changes. The caller should consider having
    /// a max batch size to balance transaction insertions with other updates.
    pub fn add_transactions(
        &self,
        origin: TransactionOrigin,
        transactions: impl IntoIterator<Item = TransactionValidationOutcome<T::Transaction>>,
    ) -> Vec<PoolResult<TxHash>> {
        // Add the transactions and enforce the pool size limits in one write lock
        let (mut added, discarded) = {
            let mut pool = self.pool.write();
            let added = transactions
                .into_iter()
                .map(|tx| self.add_transaction(&mut pool, origin, tx))
                .collect::<Vec<_>>();

            // Enforce the pool size limits if at least one transaction was added successfully
            let discarded = if added.iter().any(Result::is_ok) {
                Vec::new() // Simplified - no discarding for now
            } else {
                Default::default()
            };

            (added, discarded)
        };

        if !discarded.is_empty() {
            // Delete any blobs associated with discarded blob transactions
            self.delete_discarded_blobs(discarded.iter());

            let discarded_hashes = discarded
                .into_iter()
                .map(|tx| *tx.hash())
                .collect::<HashSet<_>>();

            {
                let mut listener = self.event_listener.write();
                for hash in &discarded_hashes {
                    listener.discarded(hash);
                }
            }

            // A newly added transaction may be immediately discarded, so we need to
            // adjust the result here
            for res in &mut added {
                if let Ok(hash) = res {
                    if discarded_hashes.contains(hash) {
                        *res = Err(PoolError::new(*hash, PoolErrorKind::DiscardedOnInsert))
                    }
                }
            }
        }

        added
    }

    /// Notify all listeners about a new pending transaction.
    fn on_new_pending_transaction(&self, pending: &AddedPendingTransaction<T::Transaction>) {
        let propagate_allowed = pending.transaction.propagate;

        let mut transaction_listeners = self.pending_transaction_listener.lock();
        transaction_listeners.retain_mut(|listener| {
            if listener.kind.is_propagate_only() && !propagate_allowed {
                // only emit this hash to listeners that are only allowed to receive propagate only
                // transactions, such as network
                return !listener.sender.is_closed();
            }

            // broadcast all pending transactions to the listener
            listener.send_all(std::iter::once(*pending.transaction.hash()))
        });
    }

    /// Notify all listeners about a newly inserted pending transaction.
    fn on_new_transaction(&self, event: NewTransactionEvent<T::Transaction>) {
        let mut transaction_listeners = self.transaction_listener.lock();
        transaction_listeners.retain_mut(|listener| {
            if listener.kind.is_propagate_only() && !event.transaction.propagate {
                // only emit this hash to listeners that are only allowed to receive propagate only
                // transactions, such as network
                return !listener.sender.is_closed();
            }

            listener.send(event.clone())
        });
    }

    /// Notify all listeners about a blob sidecar for a newly inserted blob (eip4844) transaction.
    fn on_new_blob_sidecar(&self, tx_hash: &TxHash, sidecar: &BlobTransactionSidecarVariant) {
        let mut sidecar_listeners = self.blob_transaction_sidecar_listener.lock();
        if sidecar_listeners.is_empty() {
            return;
        }
        let sidecar = Arc::new(sidecar.clone());
        sidecar_listeners.retain_mut(|listener| {
            let new_blob_event = NewBlobSidecar {
                tx_hash: *tx_hash,
                sidecar: sidecar.clone(),
            };
            match listener.sender.try_send(new_blob_event) {
                Ok(()) => true,
                Err(err) => {
                    if matches!(err, mpsc::error::TrySendError::Full(_)) {
                        debug!(
                            target: "txpool",
                            "[{:?}] failed to send blob sidecar; channel full",
                            sidecar,
                        );
                        true
                    } else {
                        false
                    }
                }
            }
        })
    }

    /// Notifies transaction listeners about changes once a block was processed.
    fn notify_on_new_state(&self, outcome: OnNewCanonicalStateOutcome<T::Transaction>) {
        trace!(target: "txpool", promoted=outcome.promoted.len(), discarded= outcome.discarded.len() ,"notifying listeners on state change");

        // notify about promoted pending transactions
        // emit hashes
        self.pending_transaction_listener
            .lock()
            .retain_mut(|listener| {
                listener.send_all(outcome.pending_transactions(listener.kind).into_iter())
            });

        // emit full transactions
        self.transaction_listener.lock().retain_mut(|listener| {
            listener.send_all(outcome.full_pending_transactions(listener.kind).into_iter())
        });

        let OnNewCanonicalStateOutcome {
            mined,
            promoted,
            discarded,
            block_hash,
        } = outcome;

        // broadcast specific transaction events
        let mut listener = self.event_listener.write();

        for tx in &mined {
            listener.mined(tx.hash(), block_hash.unwrap_or_default());
        }
        for tx in &promoted {
            listener.pending(tx.hash(), None);
        }
        for tx in &discarded {
            listener.discarded(tx.hash());
        }
    }

    /// Get a transaction by hash
    pub fn get(&self, _tx_hash: &TxHash) -> Option<Arc<ValidPoolTransaction<T::Transaction>>> {
        // Simplified implementation - return None for now
        None
    }

    /// Get all transactions by hashes
    pub fn get_all(&self, _txs: Vec<TxHash>) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        // Simplified implementation - return empty vec for now
        Vec::new()
    }

    /// Handle propagated transactions
    pub fn on_propagated(&self, txs: PropagatedTransactions) {
        // Implementation for handling propagated transactions
        // This is a simplified version - you may need to expand based on your needs
    }

    /// Get transactions by sender
    pub fn get_transactions_by_sender(
        &self,
        sender: Address,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        data.get_transactions_by_sender(sender)
    }

    /// Get pending transactions with predicate
    pub fn pending_transactions_with_predicate(
        &self,
        mut predicate: impl FnMut(&ValidPoolTransaction<T::Transaction>) -> bool,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        data.get_pending_transactions_with_predicate(predicate)
    }

    /// Get pending transactions by sender
    pub fn get_pending_transactions_by_sender(
        &self,
        sender: Address,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        data.get_pending_transactions_by_sender(sender)
    }

    /// Get queued transactions by sender
    pub fn get_queued_transactions_by_sender(
        &self,
        sender: Address,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        data.get_queued_transactions_by_sender(sender)
    }

    /// Get highest transaction by sender
    pub fn get_highest_transaction_by_sender(
        &self,
        sender: Address,
    ) -> Option<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        data.get_highest_transaction_by_sender(sender)
    }

    /// Get highest consecutive transaction by sender
    pub fn get_highest_consecutive_transaction_by_sender(
        &self,
        sender: Address,
        on_chain_nonce: u64,
    ) -> Option<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        data.get_highest_consecutive_transaction_by_sender(sender, on_chain_nonce)
    }

    /// Get transactions by origin
    pub fn get_transactions_by_origin(
        &self,
        origin: TransactionOrigin,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        data.get_transactions_by_origin(origin)
    }

    /// Get pending transactions by origin
    pub fn get_pending_transactions_by_origin(
        &self,
        origin: TransactionOrigin,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        data.get_pending_transactions_by_origin(origin)
    }

    /// Delete blob
    pub fn delete_blob(&self, tx: TxHash) {
        let _ = self.blob_store.delete(tx);
    }

    /// Delete blobs
    pub fn delete_blobs(&self, txs: Vec<TxHash>) {
        for tx in txs {
            let _ = self.blob_store.delete(tx);
        }
    }

    /// Cleanup blobs
    pub fn cleanup_blobs(&self) {
        // Implementation for cleanup - you may need to expand based on your needs
    }

    /// Get pending transactions
    pub fn pending_transactions(&self) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        Vec::new()
    }

    /// Get pending transactions max
    pub fn pending_transactions_max(
        &self,
        max: usize,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        data.get_queued_transactions_max(max)
    }

    /// Get queued transactions
    pub fn queued_transactions(&self) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let data = self.get_pool_data();
        Vec::new()
    }

    /// Get all transactions
    pub fn all_transactions(&self) -> AllPoolTransactions<T::Transaction> {
        let data = self.get_pool_data();
        let transactions: Vec<Arc<ValidPoolTransaction<T::Transaction>>> =
            data.get_transactions_by_sender(alloy_primitives::Address::ZERO);
        AllPoolTransactions {
            pending: transactions.clone(),
            queued: transactions,
        }
    }

    /// Remove transactions
    pub fn remove_transactions(
        &self,
        hashes: Vec<TxHash>,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let mut pool = self.pool.write();
        let mut removed = Vec::new();
        for hash in hashes {
            if let Some(tx) = pool.remove_transaction(&hash) {
                removed.push(Arc::new(tx));
            }
        }
        removed
    }

    /// Remove transactions and descendants
    pub fn remove_transactions_and_descendants(
        &self,
        hashes: Vec<TxHash>,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        // Simplified implementation - just remove the specified transactions
        self.remove_transactions(hashes)
    }

    /// Remove transactions by sender
    pub fn remove_transactions_by_sender(
        &self,
        sender: Address,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        let mut pool = self.pool.write();
        let transactions = pool.get_transactions_by_sender(sender);
        let mut removed = Vec::new();
        for tx in transactions {
            if let Some(removed_tx) = pool.remove_transaction(tx.hash()) {
                removed.push(Arc::new(removed_tx));
            }
        }
        removed
    }

    /// Retain unknown transactions
    pub fn retain_unknown<A>(&self, _announcement: &mut A)
    where
        A: HandleMempoolData,
    {
        // Simplified implementation - no-op for now
    }

    /// Get best transactions
    pub fn best_transactions(
        &self,
    ) -> Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<T::Transaction>>>> {
        let data = self.get_pool_data();
        let transactions: Vec<Arc<ValidPoolTransaction<T::Transaction>>> = Vec::new();
        Box::new(crate::pool::txpool::SimpleBestTransactions::<T>::new(
            transactions,
        ))
    }

    /// Get best transactions with attributes
    pub fn best_transactions_with_attributes(
        &self,
        _attributes: BestTransactionsAttributes,
    ) -> Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<T::Transaction>>>> {
        // Simplified implementation - ignore attributes for now
        self.best_transactions()
    }

    /// Get pool length
    pub fn len(&self) -> usize {
        let data = self.get_pool_data();
        data.len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        let data = self.get_pool_data();
        data.is_empty()
    }

    /// Check if pool is exceeded
    pub fn is_exceeded(&self) -> bool {
        // Simplified implementation - always return false for now
        false
    }

    /// Delete discarded blobs
    fn delete_discarded_blobs<'a>(
        &self,
        discarded: impl Iterator<Item = &'a Arc<ValidPoolTransaction<T::Transaction>>>,
    ) {
        for tx in discarded {
            if tx.is_eip4844() {
                let _ = self.blob_store.delete(*tx.hash());
            }
        }
    }

    /// Notify event listeners
    fn notify_event_listeners(&self, tx_hash: &TxHash) {
        let mut listener = self.event_listener.write();
        listener.pending(tx_hash, None);
    }
}

impl<V, T: TransactionOrdering, S> MysticetiPool for PoolInner<V, T, S> {
    type Transaction = T::Transaction;
    fn add_committed_subdags<Tx: PoolTransaction>(
        &self,
        committed_subdags: Vec<MysticetiCommittedSubdag<Tx>>,
    ) {
        self.pool.write().add_committed_subdags(committed_subdags);
    }
}

impl<V, T: TransactionOrdering, S> fmt::Debug for PoolInner<V, T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoolInner")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
