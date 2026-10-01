pub mod impls;
pub mod input;

#[cfg(test)]
mod tests;

use crate::storage::{Ptr, Storage, StorageGuard, Weak};
use alloc::boxed::Box;
use er::Er;
use hashbrown::HashTable;

#[cfg(not(feature = "std"))]
use hashbrown::DefaultHashBuilder as HashBuilder;
#[cfg(feature = "std")]
use std::hash::RandomState as HashBuilder;

/// Use an entry inside `.with(key, |v| ...)`.
pub struct CMap<K, V> {
    inner: Ptr<MapInner<K, V>>,
}

/// A key with its hash saved for its map.
pub struct CKey<K> {
    // Changing the key would leave its saved hash stale.
    identity: Weak<()>,
    key: K,
    hash: u64,
    shard: usize,
}

struct MapInner<K, V> {
    identity: Ptr<()>,
    hash_builder: HashBuilder,
    shards: Box<[MapShard<K, V>]>,
}

type MapShard<K, V> = Storage<HashTable<MapEntry<K, V>>>;

struct MapEntry<K, V> {
    key: K,
    hash: u64,
    value: V,
}

struct HeldShard<'a, K, V> {
    index: usize,
    guard: StorageGuard<'a, HashTable<MapEntry<K, V>>>,
}

/// Your closure hasn't run when you get one of these.
#[derive(Er, PartialEq, Eq)]
pub enum CMapError {
    #[er(format = "the key has no value in this CMap")]
    MissingKey,
    #[er(format = "the same key was passed more than once")]
    DuplicateKey,
    #[er(format = "the key belongs to another CMap")]
    ForeignKey,
}
