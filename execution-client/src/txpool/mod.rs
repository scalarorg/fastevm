mod builder;
mod config;
mod consensus;
mod metrics;
mod pool;
use alloy_eips::{eip4844::BlobAndProofV2, eip7594::BlobTransactionSidecarVariant};
use alloy_primitives::{Address, TxHash, B256};
use alloy_rpc_types_engine::BlobAndProofV1;
pub use builder::MysticetiPoolBuilder;
pub use consensus::TxPool;
pub use pool::PoolInner;
use reth_eth_wire_types::HandleMempoolData;
use reth_ethereum::{chainspec::EthereumHardforks, primitives::Recovered};
use reth_primitives_traits::Block;
use reth_provider::{ChainSpecProvider, ChangedAccount, StateProviderFactory};
use reth_transaction_pool::{
    identifier::TransactionId, AllPoolTransactions, AllTransactionsEvents, BestTransactions,
    BestTransactionsAttributes, BlobStore, BlobStoreError, BlockInfo, CanonicalStateUpdate,
    CoinbaseTipOrdering, EthPoolTransaction, EthPooledTransaction, EthTransactionValidator,
    GetPooledTransactionLimit, NewBlobSidecar, NewTransactionEvent, PoolConfig, PoolResult,
    PoolSize, PoolTransaction, PropagatedTransactions, TransactionEvents, TransactionListenerKind,
    TransactionOrdering, TransactionOrigin, TransactionPool, TransactionPoolExt,
    TransactionValidationOutcome, TransactionValidationTaskExecutor, TransactionValidator,
    ValidPoolTransaction,
};
use std::{collections::HashSet, sync::Arc};
use tokio::sync::mpsc::Receiver;
use tracing::{instrument, trace};
/// Type alias for default ethereum transaction pool
pub type MysticetiPool<Client, S, T = EthPooledTransaction> = Pool<
    TransactionValidationTaskExecutor<EthTransactionValidator<Client, T>>,
    CoinbaseTipOrdering<T>,
    S,
>;

/// A shareable, generic, customizable `TransactionPool` implementation.
#[derive(Debug)]
pub struct Pool<V, T: TransactionOrdering, S> {
    /// Arc'ed instance of the pool internals
    pool: Arc<PoolInner<V, T, S>>,
}

// === impl Pool ===

impl<V, T, S> Pool<V, T, S>
where
    V: TransactionValidator,
    T: TransactionOrdering<Transaction = <V as TransactionValidator>::Transaction>,
    S: BlobStore,
{
    /// Create a new transaction pool instance.
    pub fn new(validator: V, ordering: T, blob_store: S, config: PoolConfig) -> Self {
        Self {
            pool: Arc::new(PoolInner::new(validator, ordering, blob_store, config)),
        }
    }

    /// Returns the wrapped pool.
    pub(crate) fn inner(&self) -> &PoolInner<V, T, S> {
        &self.pool
    }

    /// Get the config the pool was configured with.
    pub fn config(&self) -> &PoolConfig {
        self.inner().config()
    }

    /// Returns future that validates all transactions in the given iterator.
    ///
    /// This returns the validated transactions in the iterator's order.
    async fn validate_all(
        &self,
        origin: TransactionOrigin,
        transactions: impl IntoIterator<Item = V::Transaction> + Send,
    ) -> Vec<(TxHash, TransactionValidationOutcome<V::Transaction>)> {
        self.pool
            .validator()
            .validate_transactions_with_origin(origin, transactions)
            .await
            .into_iter()
            .map(|tx| (tx.tx_hash(), tx))
            .collect()
    }

    /// Validates the given transaction
    async fn validate(
        &self,
        origin: TransactionOrigin,
        transaction: V::Transaction,
    ) -> (TxHash, TransactionValidationOutcome<V::Transaction>) {
        let hash = *transaction.hash();

        let outcome = self
            .pool
            .validator()
            .validate_transaction(origin, transaction)
            .await;

        (hash, outcome)
    }

    /// Number of transactions in the entire pool
    pub fn len(&self) -> usize {
        self.pool.len()
    }

    /// Whether the pool is empty
    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    /// Returns whether or not the pool is over its configured size and transaction count limits.
    pub fn is_exceeded(&self) -> bool {
        self.pool.is_exceeded()
    }

    /// Returns the configured blob store.
    pub fn blob_store(&self) -> &S {
        self.pool.blob_store()
    }
}

impl<Client, S> MysticetiPool<Client, S>
where
    Client:
        ChainSpecProvider<ChainSpec: EthereumHardforks> + StateProviderFactory + Clone + 'static,
    S: BlobStore,
{
    /// Returns a new [`Pool`] that uses the default [`TransactionValidationTaskExecutor`] when
    /// validating [`EthPooledTransaction`]s and ords via [`CoinbaseTipOrdering`]
    ///
    /// # Example
    ///
    /// ```
    /// use reth_chainspec::MAINNET;
    /// use reth_storage_api::StateProviderFactory;
    /// use reth_tasks::TokioTaskExecutor;
    /// use reth_chainspec::ChainSpecProvider;
    /// use reth_transaction_pool::{
    ///     blobstore::InMemoryBlobStore, Pool, TransactionValidationTaskExecutor,
    /// };
    /// use reth_chainspec::EthereumHardforks;
    /// # fn t<C>(client: C)  where C: ChainSpecProvider<ChainSpec: EthereumHardforks> + StateProviderFactory + Clone + 'static {
    /// let blob_store = InMemoryBlobStore::default();
    /// let pool = Pool::eth_pool(
    ///     TransactionValidationTaskExecutor::eth(
    ///         client,
    ///         blob_store.clone(),
    ///         TokioTaskExecutor::default(),
    ///     ),
    ///     blob_store,
    ///     Default::default(),
    /// );
    /// # }
    /// ```
    pub fn eth_pool(
        validator: TransactionValidationTaskExecutor<
            EthTransactionValidator<Client, EthPooledTransaction>,
        >,
        blob_store: S,
        config: PoolConfig,
    ) -> Self {
        Self::new(
            validator,
            CoinbaseTipOrdering::default(),
            blob_store,
            config,
        )
    }
}

/// implements the `TransactionPool` interface for various transaction pool API consumers.
impl<V, T, S> TransactionPool for Pool<V, T, S>
where
    V: TransactionValidator,
    <V as TransactionValidator>::Transaction: EthPoolTransaction,
    T: TransactionOrdering<Transaction = <V as TransactionValidator>::Transaction>,
    S: BlobStore,
{
    type Transaction = T::Transaction;

    fn pool_size(&self) -> PoolSize {
        self.pool.size()
    }

    fn block_info(&self) -> BlockInfo {
        self.pool.block_info()
    }

    async fn add_transaction_and_subscribe(
        &self,
        origin: TransactionOrigin,
        transaction: Self::Transaction,
    ) -> PoolResult<TransactionEvents> {
        let (_, tx) = self.validate(origin, transaction).await;
        self.pool.add_transaction_and_subscribe(origin, tx)
    }

    async fn add_transaction(
        &self,
        origin: TransactionOrigin,
        transaction: Self::Transaction,
    ) -> PoolResult<TxHash> {
        let (_, tx) = self.validate(origin, transaction).await;
        let mut results = self.pool.add_transactions(origin, std::iter::once(tx));
        results
            .pop()
            .expect("result length is the same as the input")
    }

    async fn add_transactions(
        &self,
        origin: TransactionOrigin,
        transactions: Vec<Self::Transaction>,
    ) -> Vec<PoolResult<TxHash>> {
        if transactions.is_empty() {
            return Vec::new();
        }
        let validated = self.validate_all(origin, transactions).await;

        self.pool
            .add_transactions(origin, validated.into_iter().map(|(_, tx)| tx))
    }

    fn transaction_event_listener(&self, tx_hash: TxHash) -> Option<TransactionEvents> {
        self.pool.add_transaction_event_listener(tx_hash)
    }

    fn all_transactions_event_listener(&self) -> AllTransactionsEvents<Self::Transaction> {
        self.pool.add_all_transactions_event_listener()
    }

    fn pending_transactions_listener_for(&self, kind: TransactionListenerKind) -> Receiver<TxHash> {
        self.pool.add_pending_listener(kind)
    }

    fn blob_transaction_sidecars_listener(&self) -> Receiver<NewBlobSidecar> {
        self.pool.add_blob_sidecar_listener()
    }

    fn new_transactions_listener_for(
        &self,
        kind: TransactionListenerKind,
    ) -> Receiver<NewTransactionEvent<Self::Transaction>> {
        self.pool.add_new_transaction_listener(kind)
    }

    fn pooled_transaction_hashes(&self) -> Vec<TxHash> {
        self.pool.pooled_transactions_hashes()
    }

    fn pooled_transaction_hashes_max(&self, max: usize) -> Vec<TxHash> {
        self.pooled_transaction_hashes()
            .into_iter()
            .take(max)
            .collect()
    }

    fn pooled_transactions(&self) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.pooled_transactions()
    }

    fn pooled_transactions_max(
        &self,
        max: usize,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.pooled_transactions_max(max)
    }

    fn get_pooled_transaction_elements(
        &self,
        tx_hashes: Vec<TxHash>,
        limit: GetPooledTransactionLimit,
    ) -> Vec<<<V as TransactionValidator>::Transaction as PoolTransaction>::Pooled> {
        self.pool.get_pooled_transaction_elements(tx_hashes, limit)
    }

    fn get_pooled_transaction_element(
        &self,
        tx_hash: TxHash,
    ) -> Option<Recovered<<<V as TransactionValidator>::Transaction as PoolTransaction>::Pooled>>
    {
        self.pool.get_pooled_transaction_element(tx_hash)
    }

    fn best_transactions(
        &self,
    ) -> Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<Self::Transaction>>>> {
        Box::new(self.pool.best_transactions())
    }

    fn best_transactions_with_attributes(
        &self,
        best_transactions_attributes: BestTransactionsAttributes,
    ) -> Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<Self::Transaction>>>> {
        self.pool
            .best_transactions_with_attributes(best_transactions_attributes)
    }

    fn pending_transactions(&self) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.pending_transactions()
    }

    fn pending_transactions_max(
        &self,
        max: usize,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.pending_transactions_max(max)
    }

    fn queued_transactions(&self) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.queued_transactions()
    }

    fn pending_and_queued_txn_count(&self) -> (usize, usize) {
        let data = self.pool.get_pool_data();
        let pending = data.pending_transactions_count();
        let queued = data.queued_transactions_count();
        (pending, queued)
    }

    fn all_transactions(&self) -> AllPoolTransactions<Self::Transaction> {
        self.pool.all_transactions()
    }

    fn remove_transactions(
        &self,
        hashes: Vec<TxHash>,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.remove_transactions(hashes)
    }

    fn remove_transactions_and_descendants(
        &self,
        hashes: Vec<TxHash>,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.remove_transactions_and_descendants(hashes)
    }

    fn remove_transactions_by_sender(
        &self,
        sender: Address,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.remove_transactions_by_sender(sender)
    }

    fn retain_unknown<A>(&self, announcement: &mut A)
    where
        A: HandleMempoolData,
    {
        self.pool.retain_unknown(announcement)
    }

    fn get(&self, tx_hash: &TxHash) -> Option<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.inner().get(tx_hash)
    }

    fn get_all(&self, txs: Vec<TxHash>) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.inner().get_all(txs)
    }

    fn on_propagated(&self, txs: PropagatedTransactions) {
        self.inner().on_propagated(txs)
    }

    fn get_transactions_by_sender(
        &self,
        sender: Address,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.get_transactions_by_sender(sender)
    }

    fn get_pending_transactions_with_predicate(
        &self,
        predicate: impl FnMut(&ValidPoolTransaction<Self::Transaction>) -> bool,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.pending_transactions_with_predicate(predicate)
    }

    fn get_pending_transactions_by_sender(
        &self,
        sender: Address,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.get_pending_transactions_by_sender(sender)
    }

    fn get_queued_transactions_by_sender(
        &self,
        sender: Address,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.get_queued_transactions_by_sender(sender)
    }

    fn get_highest_transaction_by_sender(
        &self,
        sender: Address,
    ) -> Option<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.get_highest_transaction_by_sender(sender)
    }

    fn get_highest_consecutive_transaction_by_sender(
        &self,
        sender: Address,
        on_chain_nonce: u64,
    ) -> Option<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool
            .get_highest_consecutive_transaction_by_sender(sender, on_chain_nonce)
    }

    fn get_transaction_by_sender_and_nonce(
        &self,
        sender: Address,
        nonce: u64,
    ) -> Option<Arc<ValidPoolTransaction<Self::Transaction>>> {
        let transaction_id = TransactionId::new(self.pool.get_sender_id(sender), nonce);

        self.inner()
            .get_pool_data()
            .all()
            .get(&transaction_id)
            .map(|tx| tx.transaction.clone())
    }

    fn get_transactions_by_origin(
        &self,
        origin: TransactionOrigin,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.get_transactions_by_origin(origin)
    }

    /// Returns all pending transactions filtered by [`TransactionOrigin`]
    fn get_pending_transactions_by_origin(
        &self,
        origin: TransactionOrigin,
    ) -> Vec<Arc<ValidPoolTransaction<Self::Transaction>>> {
        self.pool.get_pending_transactions_by_origin(origin)
    }

    fn unique_senders(&self) -> HashSet<Address> {
        self.pool.unique_senders()
    }

    fn get_blob(
        &self,
        tx_hash: TxHash,
    ) -> Result<Option<Arc<BlobTransactionSidecarVariant>>, BlobStoreError> {
        self.pool.blob_store().get(tx_hash)
    }

    fn get_all_blobs(
        &self,
        tx_hashes: Vec<TxHash>,
    ) -> Result<Vec<(TxHash, Arc<BlobTransactionSidecarVariant>)>, BlobStoreError> {
        self.pool.blob_store().get_all(tx_hashes)
    }

    fn get_all_blobs_exact(
        &self,
        tx_hashes: Vec<TxHash>,
    ) -> Result<Vec<Arc<BlobTransactionSidecarVariant>>, BlobStoreError> {
        self.pool.blob_store().get_exact(tx_hashes)
    }

    fn get_blobs_for_versioned_hashes_v1(
        &self,
        versioned_hashes: &[B256],
    ) -> Result<Vec<Option<BlobAndProofV1>>, BlobStoreError> {
        self.pool
            .blob_store()
            .get_by_versioned_hashes_v1(versioned_hashes)
    }

    fn get_blobs_for_versioned_hashes_v2(
        &self,
        versioned_hashes: &[B256],
    ) -> Result<Option<Vec<BlobAndProofV2>>, BlobStoreError> {
        self.pool
            .blob_store()
            .get_by_versioned_hashes_v2(versioned_hashes)
    }
}

impl<V, T, S> TransactionPoolExt for Pool<V, T, S>
where
    V: TransactionValidator,
    <V as TransactionValidator>::Transaction: EthPoolTransaction,
    T: TransactionOrdering<Transaction = <V as TransactionValidator>::Transaction>,
    S: BlobStore,
{
    #[instrument(skip(self), target = "txpool")]
    fn set_block_info(&self, info: BlockInfo) {
        trace!(target: "txpool", "updating pool block info");
        self.pool.set_block_info(info)
    }

    fn on_canonical_state_change<B>(&self, update: CanonicalStateUpdate<'_, B>)
    where
        B: Block,
    {
        self.pool.on_canonical_state_change(update);
    }

    fn update_accounts(&self, accounts: Vec<ChangedAccount>) {
        self.pool.update_accounts(accounts);
    }

    fn delete_blob(&self, tx: TxHash) {
        self.pool.delete_blob(tx)
    }

    fn delete_blobs(&self, txs: Vec<TxHash>) {
        self.pool.delete_blobs(txs)
    }

    fn cleanup_blobs(&self) {
        self.pool.cleanup_blobs()
    }
}

impl<V, T: TransactionOrdering, S> Clone for Pool<V, T, S> {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
        }
    }
}
