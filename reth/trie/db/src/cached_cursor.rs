//! Cached trie cursor implementations to reduce disk I/O during state root calculation.
//!
//! This module provides caching wrappers around database cursors to cache frequently
//! accessed trie nodes and hashed accounts/storage, significantly reducing disk I/O
//! when processing large blocks with many transactions.

use alloy_primitives::{map::DefaultHashBuilder, B256, U256};
use mini_moka::sync::{Cache, CacheBuilder};
use parking_lot::RwLock;
use reth_db_api::DatabaseError;
use reth_primitives_traits::Account;
use reth_trie::{
    hashed_cursor::{HashedCursor, HashedCursorFactory, HashedStorageCursor},
    trie_cursor::{TrieCursor, TrieCursorFactory},
    BranchNodeCompact, Nibbles,
};
use std::sync::Arc;
use std::time::Duration;

use super::{
    DatabaseAccountTrieCursor, DatabaseHashedAccountCursor, DatabaseHashedCursorFactory,
    DatabaseHashedStorageCursor, DatabaseStorageTrieCursor, DatabaseTrieCursorFactory,
};

/// Type alias for a cached trie node cache.
pub type TrieNodeCache = Cache<Nibbles, BranchNodeCompact, DefaultHashBuilder>;

/// Type alias for a cached hashed account cache.
pub type HashedAccountCache = Cache<B256, Account, DefaultHashBuilder>;

/// Type alias for a cached hashed storage cache.
pub type HashedStorageCache = Cache<(B256, B256), U256, DefaultHashBuilder>;

/// Shared cache container for trie operations.
#[derive(Debug, Clone)]
pub struct TrieCache {
    /// Cache for trie branch nodes (account and storage tries).
    pub trie_nodes: Arc<RwLock<TrieNodeCache>>,
    /// Cache for hashed accounts.
    pub hashed_accounts: Arc<RwLock<HashedAccountCache>>,
    /// Cache for hashed storage slots.
    pub hashed_storage: Arc<RwLock<HashedStorageCache>>,
}

impl TrieCache {
    /// Create a new `TrieCache` with default sizes.
    ///
    /// Default sizes:
    /// - Trie nodes: 200,000 entries
    /// - Hashed accounts: 50,000 entries
    /// - Hashed storage: 200,000 entries
    pub fn new() -> Self {
        Self::with_sizes(200_000, 50_000, 200_000)
    }

    /// Create a new `TrieCache` with custom sizes.
    pub fn with_sizes(
        trie_nodes_size: u64,
        hashed_accounts_size: u64,
        hashed_storage_size: u64,
    ) -> Self {
        const EXPIRY_TIME: Duration = Duration::from_secs(3600); // 1 hour

        let trie_nodes = Arc::new(RwLock::new(
            CacheBuilder::new(trie_nodes_size)
                .time_to_live(EXPIRY_TIME)
                .build_with_hasher(DefaultHashBuilder::default()),
        ));

        let hashed_accounts = Arc::new(RwLock::new(
            CacheBuilder::new(hashed_accounts_size)
                .time_to_live(EXPIRY_TIME)
                .build_with_hasher(DefaultHashBuilder::default()),
        ));

        let hashed_storage = Arc::new(RwLock::new(
            CacheBuilder::new(hashed_storage_size)
                .time_to_live(EXPIRY_TIME)
                .build_with_hasher(DefaultHashBuilder::default()),
        ));

        Self {
            trie_nodes,
            hashed_accounts,
            hashed_storage,
        }
    }

    /// Clear all caches.
    pub fn clear(&self) {
        self.trie_nodes.write().invalidate_all();
        self.hashed_accounts.write().invalidate_all();
        self.hashed_storage.write().invalidate_all();
    }
}

impl Default for TrieCache {
    fn default() -> Self {
        Self::new()
    }
}

/// A cached wrapper around `DatabaseTrieCursorFactory`.
#[derive(Debug)]
pub struct CachedTrieCursorFactory<'a, TX> {
    inner: DatabaseTrieCursorFactory<'a, TX>,
    cache: Arc<TrieCache>,
}

impl<'a, TX> CachedTrieCursorFactory<'a, TX> {
    /// Create a new cached trie cursor factory.
    pub fn new(tx: &'a TX, cache: Arc<TrieCache>) -> Self {
        Self {
            inner: DatabaseTrieCursorFactory::with_cache(tx, cache.clone()),
            cache,
        }
    }
}

impl<'a, TX: reth_db_api::transaction::DbTx> TrieCursorFactory for CachedTrieCursorFactory<'a, TX> {
    type AccountTrieCursor = CachedAccountTrieCursor<
        DatabaseAccountTrieCursor<
            <TX as reth_db_api::transaction::DbTx>::Cursor<reth_db_api::tables::AccountsTrie>,
        >,
    >;
    type StorageTrieCursor = CachedStorageTrieCursor<
        DatabaseStorageTrieCursor<
            <TX as reth_db_api::transaction::DbTx>::DupCursor<reth_db_api::tables::StoragesTrie>,
        >,
    >;

    fn account_trie_cursor(&self) -> Result<Self::AccountTrieCursor, DatabaseError> {
        self.inner.account_trie_cursor()
    }

    fn storage_trie_cursor(
        &self,
        hashed_address: B256,
    ) -> Result<Self::StorageTrieCursor, DatabaseError> {
        self.inner.storage_trie_cursor(hashed_address)
    }
}

/// A cached wrapper around an account trie cursor.
#[derive(Debug)]
pub struct CachedAccountTrieCursor<C> {
    inner: C,
    cache: Arc<TrieCache>,
}

// Safety: CachedAccountTrieCursor is Send + Sync if C is Send + Sync
// because Arc<TrieCache> is Send + Sync and the inner cursor C is Send + Sync
unsafe impl<C: Send> Send for CachedAccountTrieCursor<C> {}
unsafe impl<C: Sync> Sync for CachedAccountTrieCursor<C> {}

impl<C> CachedAccountTrieCursor<C> {
    /// Create a new cached account trie cursor.
    pub fn new(inner: C, cache: Arc<TrieCache>) -> Self {
        Self { inner, cache }
    }
}

impl<C: TrieCursor + Send + Sync> TrieCursor for CachedAccountTrieCursor<C> {
    fn seek_exact(
        &mut self,
        key: Nibbles,
    ) -> Result<Option<(Nibbles, BranchNodeCompact)>, DatabaseError> {
        // Check cache first
        if let Some(node) = self.cache.trie_nodes.read().get(&key) {
            return Ok(Some((key, node.clone())));
        }

        // Fall back to database
        let result = self.inner.seek_exact(key)?;

        // Cache result if found
        if let Some((k, ref node)) = result {
            self.cache.trie_nodes.write().insert(k, node.clone());
        }

        Ok(result)
    }

    fn seek(
        &mut self,
        key: Nibbles,
    ) -> Result<Option<(Nibbles, BranchNodeCompact)>, DatabaseError> {
        // Check cache first
        if let Some(node) = self.cache.trie_nodes.read().get(&key) {
            return Ok(Some((key, node.clone())));
        }

        // Fall back to database
        let result = self.inner.seek(key)?;

        // Cache result if found
        if let Some((k, ref node)) = result {
            self.cache.trie_nodes.write().insert(k, node.clone());
        }

        Ok(result)
    }

    fn next(&mut self) -> Result<Option<(Nibbles, BranchNodeCompact)>, DatabaseError> {
        // For next(), we can't use cache effectively since we don't know the key
        // until after the operation. However, we can cache the result.
        let result = self.inner.next()?;

        // Cache result if found
        if let Some((k, ref node)) = result {
            self.cache.trie_nodes.write().insert(k, node.clone());
        }

        Ok(result)
    }

    fn current(&mut self) -> Result<Option<Nibbles>, DatabaseError> {
        self.inner.current()
    }
}

/// A cached wrapper around a storage trie cursor.
#[derive(Debug)]
pub struct CachedStorageTrieCursor<C> {
    inner: C,
    cache: Arc<TrieCache>,
}

// Safety: CachedStorageTrieCursor is Send + Sync if C is Send + Sync
unsafe impl<C: Send> Send for CachedStorageTrieCursor<C> {}
unsafe impl<C: Sync> Sync for CachedStorageTrieCursor<C> {}

impl<C> CachedStorageTrieCursor<C> {
    /// Create a new cached storage trie cursor.
    pub fn new(inner: C, cache: Arc<TrieCache>) -> Self {
        Self { inner, cache }
    }
}

impl<C: TrieCursor + Send + Sync> TrieCursor for CachedStorageTrieCursor<C> {
    fn seek_exact(
        &mut self,
        key: Nibbles,
    ) -> Result<Option<(Nibbles, BranchNodeCompact)>, DatabaseError> {
        // Check cache first
        if let Some(node) = self.cache.trie_nodes.read().get(&key) {
            return Ok(Some((key, node.clone())));
        }

        // Fall back to database
        let result = self.inner.seek_exact(key)?;

        // Cache result if found
        if let Some((k, ref node)) = result {
            self.cache.trie_nodes.write().insert(k, node.clone());
        }

        Ok(result)
    }

    fn seek(
        &mut self,
        key: Nibbles,
    ) -> Result<Option<(Nibbles, BranchNodeCompact)>, DatabaseError> {
        // Check cache first
        if let Some(node) = self.cache.trie_nodes.read().get(&key) {
            return Ok(Some((key, node.clone())));
        }

        // Fall back to database
        let result = self.inner.seek(key)?;

        // Cache result if found
        if let Some((k, ref node)) = result {
            self.cache.trie_nodes.write().insert(k, node.clone());
        }

        Ok(result)
    }

    fn next(&mut self) -> Result<Option<(Nibbles, BranchNodeCompact)>, DatabaseError> {
        // For next(), we can't use cache effectively since we don't know the key
        // until after the operation. However, we can cache the result.
        let result = self.inner.next()?;

        // Cache result if found
        if let Some((k, ref node)) = result {
            self.cache.trie_nodes.write().insert(k, node.clone());
        }

        Ok(result)
    }

    fn current(&mut self) -> Result<Option<Nibbles>, DatabaseError> {
        self.inner.current()
    }
}

/// A cached wrapper around `DatabaseHashedCursorFactory`.
#[derive(Debug, Clone)]
pub struct CachedHashedCursorFactory<'a, TX> {
    inner: DatabaseHashedCursorFactory<'a, TX>,
    cache: Arc<TrieCache>,
}

impl<'a, TX> CachedHashedCursorFactory<'a, TX> {
    /// Create a new cached hashed cursor factory.
    pub fn new(tx: &'a TX, cache: Arc<TrieCache>) -> Self {
        Self {
            inner: DatabaseHashedCursorFactory::with_cache(tx, cache.clone()),
            cache,
        }
    }
}

impl<'a, TX: reth_db_api::transaction::DbTx> HashedCursorFactory
    for CachedHashedCursorFactory<'a, TX>
{
    type AccountCursor = CachedHashedAccountCursor<
        DatabaseHashedAccountCursor<
            <TX as reth_db_api::transaction::DbTx>::Cursor<reth_db_api::tables::HashedAccounts>,
        >,
    >;
    type StorageCursor = CachedHashedStorageCursor<
        DatabaseHashedStorageCursor<
            <TX as reth_db_api::transaction::DbTx>::DupCursor<reth_db_api::tables::HashedStorages>,
        >,
    >;

    fn hashed_account_cursor(&self) -> Result<Self::AccountCursor, DatabaseError> {
        self.inner.hashed_account_cursor()
    }

    fn hashed_storage_cursor(
        &self,
        hashed_address: B256,
    ) -> Result<Self::StorageCursor, DatabaseError> {
        self.inner.hashed_storage_cursor(hashed_address)
    }
}

/// A cached wrapper around a hashed account cursor.
#[derive(Debug)]
pub struct CachedHashedAccountCursor<C> {
    inner: C,
    cache: Arc<TrieCache>,
}

// Safety: CachedHashedAccountCursor is Send + Sync if C is Send + Sync
unsafe impl<C: Send> Send for CachedHashedAccountCursor<C> {}
unsafe impl<C: Sync> Sync for CachedHashedAccountCursor<C> {}

impl<C> CachedHashedAccountCursor<C> {
    /// Create a new cached hashed account cursor.
    pub fn new(inner: C, cache: Arc<TrieCache>) -> Self {
        Self { inner, cache }
    }
}

impl<C: HashedCursor<Value = Account> + Send + Sync> HashedCursor for CachedHashedAccountCursor<C> {
    type Value = Account;

    fn seek(&mut self, key: B256) -> Result<Option<(B256, Self::Value)>, DatabaseError> {
        // Check cache first
        if let Some(account) = self.cache.hashed_accounts.read().get(&key) {
            return Ok(Some((key, account.clone())));
        }

        // Fall back to database
        let result = self.inner.seek(key)?;

        // Cache result if found
        if let Some((k, ref account)) = result {
            self.cache
                .hashed_accounts
                .write()
                .insert(k, account.clone());
        }

        Ok(result)
    }

    fn next(&mut self) -> Result<Option<(B256, Self::Value)>, DatabaseError> {
        // For next(), we can't use cache effectively since we don't know the key
        // until after the operation. However, we can cache the result.
        let result = self.inner.next()?;

        // Cache result if found
        if let Some((k, ref account)) = result {
            self.cache
                .hashed_accounts
                .write()
                .insert(k, account.clone());
        }

        Ok(result)
    }
}

/// A cached wrapper around a hashed storage cursor.
#[derive(Debug)]
pub struct CachedHashedStorageCursor<C> {
    inner: C,
    cache: Arc<TrieCache>,
    hashed_address: B256,
}

// Safety: CachedHashedStorageCursor is Send + Sync if C is Send + Sync
unsafe impl<C: Send> Send for CachedHashedStorageCursor<C> {}
unsafe impl<C: Sync> Sync for CachedHashedStorageCursor<C> {}

impl<C> CachedHashedStorageCursor<C> {
    /// Create a new cached hashed storage cursor.
    pub fn new(inner: C, cache: Arc<TrieCache>, hashed_address: B256) -> Self {
        Self {
            inner,
            cache,
            hashed_address,
        }
    }
}

impl<C: HashedStorageCursor<Value = U256> + Send + Sync> HashedCursor
    for CachedHashedStorageCursor<C>
{
    type Value = U256;

    fn seek(&mut self, subkey: B256) -> Result<Option<(B256, Self::Value)>, DatabaseError> {
        let cache_key = (self.hashed_address, subkey);

        // Check cache first
        if let Some(value) = self.cache.hashed_storage.read().get(&cache_key) {
            return Ok(Some((subkey, value.clone())));
        }

        // Fall back to database
        let result = self.inner.seek(subkey)?;

        // Cache result if found
        if let Some((k, value)) = result {
            self.cache
                .hashed_storage
                .write()
                .insert((self.hashed_address, k), value);
        }

        Ok(result)
    }

    fn next(&mut self) -> Result<Option<(B256, Self::Value)>, DatabaseError> {
        // For next(), we can't use cache effectively since we don't know the key
        // until after the operation. However, we can cache the result.
        let result = self.inner.next()?;

        // Cache result if found
        if let Some((k, value)) = result {
            self.cache
                .hashed_storage
                .write()
                .insert((self.hashed_address, k), value);
        }

        Ok(result)
    }
}

impl<C: HashedStorageCursor<Value = U256> + Send + Sync> HashedStorageCursor
    for CachedHashedStorageCursor<C>
{
    fn is_storage_empty(&mut self) -> Result<bool, DatabaseError> {
        self.inner.is_storage_empty()
    }
}
