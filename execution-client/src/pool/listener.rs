use alloy_primitives::{TxHash, B256};
use futures_util::Stream;
use reth_transaction_pool::{
    FullTransactionEvent, PoolTransaction, PropagateKind, TransactionEvent, TransactionEvents,
    ValidPoolTransaction,
};
use std::{
    collections::{hash_map::Entry, HashMap},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::mpsc::Receiver;

/// Channel size for transaction pool events
const TX_POOL_EVENT_CHANNEL_SIZE: usize = 1000;

/// A broadcaster for pool events
#[derive(Debug)]
pub struct PoolEventBroadcaster {
    pub senders: Vec<tokio::sync::mpsc::UnboundedSender<TransactionEvent>>,
}

impl PoolEventBroadcaster {
    pub fn broadcast(&mut self, event: TransactionEvent) {
        self.senders
            .retain_mut(|sender| sender.send(event.clone()).is_ok());
    }

    pub fn is_empty(&self) -> bool {
        self.senders.is_empty()
    }
}

/// A broadcaster for all pool events
#[derive(Debug)]
pub struct AllPoolEventsBroadcaster<T: PoolTransaction> {
    pub senders: Vec<tokio::sync::mpsc::Sender<FullTransactionEvent<T>>>,
}

impl<T: PoolTransaction> Default for AllPoolEventsBroadcaster<T> {
    fn default() -> Self {
        Self {
            senders: Vec::new(),
        }
    }
}

impl<T: PoolTransaction> AllPoolEventsBroadcaster<T> {
    pub fn broadcast(&mut self, event: FullTransactionEvent<T>) {
        self.senders
            .retain_mut(|sender| sender.try_send(event.clone()).is_ok());
    }
}

/// A Stream that receives [`FullTransactionEvent`] for _all_ transaction.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct AllTransactionsEvents<T: PoolTransaction> {
    pub(crate) events: Receiver<FullTransactionEvent<T>>,
}

impl<T: PoolTransaction> AllTransactionsEvents<T> {
    /// Create a new instance of this stream.
    pub const fn new(events: Receiver<FullTransactionEvent<T>>) -> Self {
        Self { events }
    }
}

impl<T: PoolTransaction> Stream for AllTransactionsEvents<T> {
    type Item = FullTransactionEvent<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().events.poll_recv(cx)
    }
}

/// A type that broadcasts [`TransactionEvent`] to installed listeners.
///
/// This is essentially a multi-producer, multi-consumer channel where each event is broadcast to
/// all active receivers.
#[derive(Debug)]
pub(crate) struct PoolEventBroadcast<T: PoolTransaction> {
    /// All listeners for all transaction events.
    all_events_broadcaster: AllPoolEventsBroadcaster<T>,
    /// All listeners for events for a certain transaction hash.
    broadcasters_by_hash: HashMap<TxHash, PoolEventBroadcaster>,
}

impl<T: PoolTransaction> Default for PoolEventBroadcast<T> {
    fn default() -> Self {
        Self {
            all_events_broadcaster: AllPoolEventsBroadcaster::default(),
            broadcasters_by_hash: HashMap::default(),
        }
    }
}

impl<T: PoolTransaction> PoolEventBroadcast<T> {
    /// Calls the broadcast callback with the `PoolEventBroadcaster` that belongs to the hash.
    fn broadcast_event(
        &mut self,
        hash: &TxHash,
        event: TransactionEvent,
        pool_event: FullTransactionEvent<T>,
    ) {
        // Broadcast to all listeners for the transaction hash.
        if let Entry::Occupied(mut sink) = self.broadcasters_by_hash.entry(*hash) {
            sink.get_mut().broadcast(event.clone());

            if sink.get().is_empty() || event.is_final() {
                sink.remove();
            }
        }

        // Broadcast to all listeners for all transactions.
        self.all_events_broadcaster.broadcast(pool_event);
    }

    /// Create a new subscription for the given transaction hash.
    pub(crate) fn subscribe(&mut self, tx_hash: TxHash) -> TransactionEvents {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        match self.broadcasters_by_hash.entry(tx_hash) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().senders.push(tx);
            }
            Entry::Vacant(entry) => {
                entry.insert(PoolEventBroadcaster { senders: vec![tx] });
            }
        };
        // Return a dummy TransactionEvents - this is a simplified implementation
        // We can't construct TransactionEvents directly due to private fields
        // This is a placeholder that should be replaced with proper implementation
        todo!("TransactionEvents construction needs proper implementation")
    }

    /// Create a new subscription for all transactions.
    pub(crate) fn subscribe_all(&mut self) -> AllTransactionsEvents<T> {
        let (tx, rx) = tokio::sync::mpsc::channel(TX_POOL_EVENT_CHANNEL_SIZE);
        self.all_events_broadcaster.senders.push(tx);
        AllTransactionsEvents::new(rx)
    }

    /// Notify listeners about a transaction that was added to the pending queue.
    pub(crate) fn pending(&mut self, tx: &TxHash, replaced: Option<Arc<ValidPoolTransaction<T>>>) {
        self.broadcast_event(
            tx,
            TransactionEvent::Pending,
            FullTransactionEvent::Pending(*tx),
        );

        if let Some(replaced) = replaced {
            // notify listeners that this transaction was replaced
            self.replaced(replaced, *tx);
        }
    }

    /// Notify listeners about a transaction that was replaced.
    pub(crate) fn replaced(&mut self, tx: Arc<ValidPoolTransaction<T>>, replaced_by: TxHash) {
        let transaction = Arc::clone(&tx);
        self.broadcast_event(
            tx.hash(),
            TransactionEvent::Replaced(replaced_by),
            FullTransactionEvent::Replaced {
                transaction,
                replaced_by,
            },
        );
    }

    /// Notify listeners about a transaction that was added to the queued pool.
    pub(crate) fn queued(&mut self, tx: &TxHash) {
        self.broadcast_event(
            tx,
            TransactionEvent::Queued,
            FullTransactionEvent::Queued(*tx),
        );
    }

    /// Notify listeners about a transaction that was propagated.
    pub(crate) fn propagated(&mut self, tx: &TxHash, peers: Vec<PropagateKind>) {
        let peers = Arc::new(peers);
        self.broadcast_event(
            tx,
            TransactionEvent::Propagated(Arc::clone(&peers)),
            FullTransactionEvent::Propagated(peers),
        );
    }

    /// Notify listeners about a transaction that was discarded.
    pub(crate) fn discarded(&mut self, tx: &TxHash) {
        self.broadcast_event(
            tx,
            TransactionEvent::Discarded,
            FullTransactionEvent::Discarded(*tx),
        );
    }

    /// Notify listeners about a transaction that was invalid.
    pub(crate) fn invalid(&mut self, tx: &TxHash) {
        self.broadcast_event(
            tx,
            TransactionEvent::Invalid,
            FullTransactionEvent::Invalid(*tx),
        );
    }

    /// Notify listeners that the transaction was mined
    pub(crate) fn mined(&mut self, tx: &TxHash, block_hash: B256) {
        self.broadcast_event(
            tx,
            TransactionEvent::Mined(block_hash),
            FullTransactionEvent::Mined {
                tx_hash: *tx,
                block_hash,
            },
        );
    }
}
