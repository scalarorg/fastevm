//! An integration of `reth-trie` with `reth-db`.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

#[cfg(feature = "metrics")]
#[macro_use]
extern crate metrics;

mod cached_cursor;
mod hashed_cursor;
mod prefix_set;
mod proof;
mod state;
mod storage;
mod trie_cursor;
mod witness;

pub use cached_cursor::{
    CachedAccountTrieCursor, CachedHashedAccountCursor, CachedHashedCursorFactory,
    CachedHashedStorageCursor, CachedStorageTrieCursor, CachedTrieCursorFactory, TrieCache,
};
pub use hashed_cursor::{
    DatabaseHashedAccountCursor, DatabaseHashedCursorFactory, DatabaseHashedStorageCursor,
};
pub use prefix_set::PrefixSetLoader;
pub use proof::{DatabaseProof, DatabaseStorageProof};
pub use state::{DatabaseHashedPostState, DatabaseStateRoot};
pub use storage::{DatabaseHashedStorage, DatabaseStorageRoot};
pub use trie_cursor::{
    DatabaseAccountTrieCursor, DatabaseStorageTrieCursor, DatabaseTrieCursorFactory,
};
pub use witness::DatabaseTrieWitness;
