use alloy_primitives::{B256, U256};
use reth_db_api::{
    cursor::{DbCursorRO, DbDupCursorRO},
    tables,
    transaction::DbTx,
    DatabaseError,
};
use reth_primitives_traits::Account;
use reth_trie::hashed_cursor::{HashedCursor, HashedCursorFactory, HashedStorageCursor};
use std::sync::Arc;

use crate::cached::TrieCache;

/// A struct wrapping database transaction that implements [`HashedCursorFactory`].
#[derive(Debug)]
pub struct DatabaseHashedCursorFactory<'a, TX> {
    tx: &'a TX,
    /// Optional cache for hashed accounts/storage. If None, no caching is performed.
    cache: Option<Arc<TrieCache>>,
}

impl<TX> Clone for DatabaseHashedCursorFactory<'_, TX> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx,
            cache: self.cache.clone(),
        }
    }
}

impl<'a, TX> DatabaseHashedCursorFactory<'a, TX> {
    /// Create new database hashed cursor factory without caching.
    pub fn new(tx: &'a TX) -> Self {
        Self {
            tx,
            cache: Some(Arc::new(TrieCache::large_cache())),
        }
    }
}

impl<TX: DbTx> HashedCursorFactory for DatabaseHashedCursorFactory<'_, TX> {
    type AccountCursor = crate::cached::CachedHashedAccountCursor<
        DatabaseHashedAccountCursor<<TX as DbTx>::Cursor<tables::HashedAccounts>>,
    >;
    type StorageCursor = crate::cached::CachedHashedStorageCursor<
        DatabaseHashedStorageCursor<<TX as DbTx>::DupCursor<tables::HashedStorages>>,
    >;

    fn hashed_account_cursor(&self) -> Result<Self::AccountCursor, DatabaseError> {
        let inner = DatabaseHashedAccountCursor(self.tx.cursor_read::<tables::HashedAccounts>()?);
        Ok(if let Some(cache) = &self.cache {
            crate::cached::CachedHashedAccountCursor::new(inner, cache.clone())
        } else {
            let dummy_cache = Arc::new(TrieCache::with_sizes(0, 0, 0));
            crate::cached::CachedHashedAccountCursor::new(inner, dummy_cache)
        })
    }

    fn hashed_storage_cursor(
        &self,
        hashed_address: B256,
    ) -> Result<Self::StorageCursor, DatabaseError> {
        let inner = DatabaseHashedStorageCursor::new(
            self.tx.cursor_dup_read::<tables::HashedStorages>()?,
            hashed_address,
        );
        Ok(if let Some(cache) = &self.cache {
            crate::cached::CachedHashedStorageCursor::new(inner, cache.clone(), hashed_address)
        } else {
            let dummy_cache = Arc::new(TrieCache::with_sizes(0, 0, 0));
            crate::cached::CachedHashedStorageCursor::new(inner, dummy_cache, hashed_address)
        })
    }
}

/// A struct wrapping database cursor over hashed accounts implementing [`HashedCursor`] for
/// iterating over accounts.
#[derive(Debug)]
pub struct DatabaseHashedAccountCursor<C>(C);

impl<C> DatabaseHashedAccountCursor<C> {
    /// Create new database hashed account cursor.
    pub const fn new(cursor: C) -> Self {
        Self(cursor)
    }
}

impl<C> HashedCursor for DatabaseHashedAccountCursor<C>
where
    C: DbCursorRO<tables::HashedAccounts>,
{
    type Value = Account;

    fn seek(&mut self, key: B256) -> Result<Option<(B256, Self::Value)>, DatabaseError> {
        self.0.seek(key)
    }

    fn next(&mut self) -> Result<Option<(B256, Self::Value)>, DatabaseError> {
        self.0.next()
    }
}

/// The structure wrapping a database cursor for hashed storage and
/// a target hashed address. Implements [`HashedCursor`] and [`HashedStorageCursor`]
/// for iterating over hashed storage.
#[derive(Debug)]
pub struct DatabaseHashedStorageCursor<C> {
    /// Database hashed storage cursor.
    cursor: C,
    /// Target hashed address of the account that the storage belongs to.
    hashed_address: B256,
}

impl<C> DatabaseHashedStorageCursor<C> {
    /// Create new [`DatabaseHashedStorageCursor`].
    pub const fn new(cursor: C, hashed_address: B256) -> Self {
        Self {
            cursor,
            hashed_address,
        }
    }
}

impl<C> HashedCursor for DatabaseHashedStorageCursor<C>
where
    C: DbCursorRO<tables::HashedStorages> + DbDupCursorRO<tables::HashedStorages>,
{
    type Value = U256;

    fn seek(&mut self, subkey: B256) -> Result<Option<(B256, Self::Value)>, DatabaseError> {
        Ok(self
            .cursor
            .seek_by_key_subkey(self.hashed_address, subkey)?
            .map(|e| (e.key, e.value)))
    }

    fn next(&mut self) -> Result<Option<(B256, Self::Value)>, DatabaseError> {
        Ok(self.cursor.next_dup_val()?.map(|e| (e.key, e.value)))
    }
}

impl<C> HashedStorageCursor for DatabaseHashedStorageCursor<C>
where
    C: DbCursorRO<tables::HashedStorages> + DbDupCursorRO<tables::HashedStorages>,
{
    fn is_storage_empty(&mut self) -> Result<bool, DatabaseError> {
        Ok(self.cursor.seek_exact(self.hashed_address)?.is_none())
    }
}
