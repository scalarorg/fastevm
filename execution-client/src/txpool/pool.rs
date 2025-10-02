use crate::txpool::config::TxPoolConfig;

use super::TxPool;
use alloy_eips::eip7594::BlobTransactionSidecarVariant;
use alloy_primitives::{Address, TxHash};
use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use reth_ethereum::primitives::Recovered;
use reth_primitives_traits::Block;
use reth_provider::ChangedAccount;
use reth_transaction_pool::{
    error::{PoolError, PoolErrorKind},
    identifier::{SenderId, SenderIdentifiers, TransactionId},
    metrics::BlobStoreMetrics,
    pool::{
        listener::{
            BlobTransactionSidecarListener, PendingTransactionHashListener, PoolEventBroadcast,
            TransactionListener,
        },
        AddedPendingTransaction, AddedTransaction, OnNewCanonicalStateOutcome, SenderInfo,
        UpdateOutcome,
    },
    validate::ValidTransaction,
    AllTransactionsEvents, BlobStore, BlockInfo, CanonicalStateUpdate, EthPoolTransaction,
    GetPooledTransactionLimit, NewBlobSidecar, NewTransactionEvent, PoolConfig, PoolResult,
    PoolSize, PoolTransaction, TransactionEvents, TransactionListenerKind, TransactionOrdering,
    TransactionOrigin, TransactionValidationOutcome, TransactionValidator, ValidPoolTransaction,
};
use rustc_hash::FxHashMap;
use std::{collections::HashSet, fmt, sync::Arc, time::Instant};
use tokio::sync::mpsc;
use tracing::{debug, trace};

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
    pool: RwLock<TxPool<T>>,
    /// Pool settings.
    config: TxPoolConfig,
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
    pub fn new(validator: V, ordering: T, blob_store: S, config: TxPoolConfig) -> Self {
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
        self.get_pool_data().size()
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
        self.get_pool_data().unique_senders()
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
    pub const fn config(&self) -> &TxPoolConfig {
        &self.config
    }

    /// Get the validator reference.
    pub const fn validator(&self) -> &V {
        &self.validator
    }

    /// Adds a new transaction listener to the pool that gets notified about every new _pending_
    /// transaction inserted into the pool
    pub fn add_pending_listener(&self, kind: TransactionListenerKind) -> mpsc::Receiver<TxHash> {
        let (sender, rx) = mpsc::channel(self.config.pending_tx_listener_buffer_size);
        let listener = PendingTransactionHashListener { sender, kind };
        self.pending_transaction_listener.lock().push(listener);
        rx
    }

    /// Adds a new transaction listener to the pool that gets notified about every new transaction.
    pub fn add_new_transaction_listener(
        &self,
        kind: TransactionListenerKind,
    ) -> mpsc::Receiver<NewTransactionEvent<T::Transaction>> {
        let (sender, rx) = mpsc::channel(self.config.new_tx_listener_buffer_size);
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
    pub fn add_all_transactions_event_listener(&self) -> AllTransactionsEvents<T::Transaction> {
        self.event_listener.write().subscribe_all()
    }

    /// Returns a read lock to the pool's data.
    pub fn get_pool_data(&self) -> RwLockReadGuard<'_, TxPool<T>> {
        self.pool.read()
    }

    /// Returns hashes of _all_ transactions in the pool.
    pub fn pooled_transactions_hashes(&self) -> Vec<TxHash> {
        self.get_pool_data()
            .all()
            .transactions_iter()
            .filter(|tx| tx.propagate)
            .map(|tx| *tx.hash())
            .collect()
    }

    /// Returns _all_ transactions in the pool.
    pub fn pooled_transactions(&self) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        self.get_pool_data()
            .all()
            .transactions_iter()
            .filter(|tx| tx.propagate)
            .cloned()
            .collect()
    }

    /// Returns only the first `max` transactions in the pool.
    pub fn pooled_transactions_max(
        &self,
        max: usize,
    ) -> Vec<Arc<ValidPoolTransaction<T::Transaction>>> {
        self.get_pool_data()
            .all()
            .transactions_iter()
            .filter(|tx| tx.propagate)
            .take(max)
            .cloned()
            .collect()
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

        // update the pool
        let outcome = self.pool.write().on_canonical_state_change(
            block_info,
            mined_transactions,
            changed_senders,
            update_kind,
        );

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
        let UpdateOutcome {
            promoted,
            discarded,
        } = self.pool.write().update_accounts(changed_senders);

        // Notify about promoted pending transactions (similar to notify_on_new_state)
        if !promoted.is_empty() {
            self.pending_transaction_listener
                .lock()
                .retain_mut(|listener| {
                    let promoted_hashes = promoted.iter().filter_map(|tx| {
                        if listener.kind.is_propagate_only() && !tx.propagate {
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

            for tx in &promoted {
                listener.pending(tx.hash(), None);
            }
            for tx in &discarded {
                listener.discarded(tx.hash());
            }
        }

        // This deletes outdated blob txs from the blob store, based on the account's nonce. This is
        // called during txpool maintenance when the pool drifted.
        self.delete_discarded_blobs(discarded.iter());
    }

    /// Add a single validated transaction into the pool.
    ///
    /// Note: this is only used internally by [`Self::add_transactions()`], all new transaction(s)
    /// come in through that function, either as a batch or `std::iter::once`.
    fn add_transaction(
        &self,
        pool: &mut RwLockWriteGuard<'_, TxPool<T>>,
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
                pool.discard_worst()
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
        unimplemented!()
        // let propagate_allowed = pending.is_propagate_allowed();

        // let mut transaction_listeners = self.pending_transaction_listener.lock();
        // transaction_listeners.retain_mut(|listener| {
        //     if listener.kind.is_propagate_only() && !propagate_allowed {
        //         // only emit this hash to listeners that are only allowed to receive propagate only
        //         // transactions, such as network
        //         return !listener.sender.is_closed();
        //     }

        //     // broadcast all pending transactions to the listener
        //     listener.send_all(pending.pending_transactions(listener.kind))
        // });
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
            .retain_mut(|listener| listener.send_all(outcome.pending_transactions(listener.kind)));

        // emit full transactions
        self.transaction_listener.lock().retain_mut(|listener| {
            listener.send_all(outcome.full_pending_transactions(listener.kind))
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
            listener.mined(tx, block_hash);
        }
        for tx in &promoted {
            listener.pending(tx.hash(), None);
        }
        for tx in &discarded {
            listener.discarded(tx.hash());
        }
    }

    /// Fire events for the newly added transaction if there are any.
    fn notify_event_listeners(&self, tx: &AddedTransaction<T::Transaction>) {
        unimplemented!()
        // let mut listener = self.event_listener.write();

        // match tx {
        //     AddedTransaction::Pending(tx) => {
        //         let AddedPendingTransaction {
        //             transaction,
        //             promoted,
        //             discarded,
        //             replaced,
        //         } = tx;

        //         listener.pending(transaction.hash(), replaced.clone());
        //         for tx in promoted {
        //             listener.pending(tx.hash(), None);
        //         }
        //         for tx in discarded {
        //             listener.discarded(tx.hash());
        //         }
        //     }
        //     AddedTransaction::Parked {
        //         transaction,
        //         replaced,
        //         ..
        //     } => {
        //         listener.queued(transaction.hash());
        //         if let Some(replaced) = replaced {
        //             listener.replaced(replaced.clone(), *transaction.hash());
        //         }
        //     }
        // }
    }
}
impl<V, T: TransactionOrdering, S> fmt::Debug for PoolInner<V, T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoolInner")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
