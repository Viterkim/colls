use super::{CKey, CMap, CMapError, HashBuilder, HeldShard, MapEntry, MapInner, input::CMapInput};
use crate::storage::{Ptr, Storage, Weak, borrow};
use core::{array, fmt, hash::Hash, mem};
use hashbrown::{HashTable, hash_table::Entry};

// One shard is enough for local tasks. Native threads get independent shards.
pub const SHARD_COUNT: usize = if cfg!(feature = "std") && !cfg!(target_arch = "wasm32") {
    32
} else {
    1
};

impl<K, V> CMap<K, V> {
    pub fn new() -> Self {
        let shards = (0..SHARD_COUNT)
            .map(|_| Storage::new(HashTable::new()))
            .collect();

        Self {
            inner: Ptr::new(MapInner {
                identity: Ptr::new(()),
                hash_builder: HashBuilder::default(),
                shards,
            }),
        }
    }

    /// Another handle to the same map.
    #[must_use]
    pub fn clone_ptr(&self) -> Self {
        Self {
            inner: Ptr::clone(&self.inner),
        }
    }
}
impl<K, V> Default for CMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K: Eq + Hash, V> CMap<K, V> {
    /// Insert or replace a value and get its key back.
    pub async fn insert<I: CMapInput<K>>(&self, key: I, v: V) -> Result<I::Prepared, CMapError>
    where
        K: Clone,
    {
        let prepared = key.prepare(self);
        let key = prepared.as_ref();
        self.validate(key)?;

        let old = {
            let mut shard = borrow(&self.inner.shards[key.shard]);
            match shard.entry(key.hash, |entry| entry.key == key.key, |entry| entry.hash) {
                Entry::Occupied(mut entry) => Some(mem::replace(&mut entry.get_mut().value, v)),
                Entry::Vacant(entry) => {
                    entry.insert(MapEntry {
                        key: key.key.clone(),
                        hash: key.hash,
                        value: v,
                    });
                    None
                }
            }
        };

        drop(old);

        Ok(prepared)
    }

    /// Returns the key. Pass `&key` to reuse its hash.
    pub async fn with<I: CMapInput<K>>(
        &self,
        key: I,
        f: impl FnOnce(&mut V),
    ) -> Result<I::Prepared, CMapError> {
        let prepared = key.prepare(self);
        let key = prepared.as_ref();
        self.validate(key)?;

        {
            let mut shard = borrow(&self.inner.shards[key.shard]);
            let entry = shard
                .find_mut(key.hash, |entry| entry.key == key.key)
                .ok_or(CMapError::MissingKey)?;
            f(&mut entry.value);
        }

        Ok(prepared)
    }

    /// Clone this entry's value using its own `Clone` implementation.
    pub async fn clone_inner<I: CMapInput<K>>(&self, key: I) -> Result<V, CMapError>
    where
        V: Clone,
    {
        let prepared = key.prepare(self);
        let key = prepared.as_ref();
        self.validate(key)?;

        let shard = borrow(&self.inner.shards[key.shard]);
        let entry = shard
            .find(key.hash, |entry| entry.key == key.key)
            .ok_or(CMapError::MissingKey)?;

        Ok(entry.value.clone())
    }

    /// Remove an entry and get its value back.
    pub async fn remove<I: CMapInput<K>>(&self, key: I) -> Result<V, CMapError> {
        let prepared = key.prepare(self);
        let key = prepared.as_ref();
        self.validate(key)?;

        let entry = {
            let mut shard = borrow(&self.inner.shards[key.shard]);
            let entry = shard
                .find_entry(key.hash, |entry| entry.key == key.key)
                .map_err(|_| CMapError::MissingKey)?;
            entry.remove().0
        };

        Ok(entry.value)
    }

    /// Use several entries together, in the order you pass their keys.
    pub async fn with_many<const N: usize, R>(
        &self,
        keys: [&CKey<K>; N],
        f: impl FnOnce([&mut V; N]) -> R,
    ) -> Result<R, CMapError> {
        for key in keys {
            self.validate(key)?;
        }

        for (index, key) in keys.iter().enumerate() {
            if keys[index + 1..].iter().any(|other| key.key == other.key) {
                return Err(CMapError::DuplicateKey);
            }
        }

        let mut order = keys.map(|key| key.shard);
        order.sort_unstable();
        let mut held: [Option<HeldShard<'_, K, V>>; N] = array::from_fn(|_| None);
        let mut count = 0;
        let mut previous = None;

        for index in order {
            if previous != Some(index) {
                held[count] = Some(HeldShard {
                    index,
                    guard: borrow(&self.inner.shards[index]),
                });
                count += 1;
                previous = Some(index);
            }
        }

        let hashes = keys.map(|key| key.hash);
        let mut values: [Option<&mut V>; N] = array::from_fn(|_| None);

        for shard in held.iter_mut().flatten() {
            let found = shard.guard.get_disjoint_mut(hashes, |index, entry| {
                keys[index].shard == shard.index && entry.key == keys[index].key
            });

            for (index, entry) in found.into_iter().enumerate() {
                if let Some(entry) = entry {
                    values[index] = Some(&mut entry.value);
                }
            }
        }

        if values.iter().any(Option::is_none) {
            return Err(CMapError::MissingKey);
        }

        // Every slot has a value, so its slice has index 0.
        let values = values.each_mut().map(|value| &mut *value.as_mut_slice()[0]);

        Ok(f(values))
    }

    fn validate(&self, key: &CKey<K>) -> Result<(), CMapError> {
        // Old keys keep this address from being reused after their map is dropped.
        if Weak::as_ptr(&key.identity) == Ptr::as_ptr(&self.inner.identity) {
            Ok(())
        } else {
            Err(CMapError::ForeignKey)
        }
    }
}

impl<K> CKey<K> {
    /// Copy the key and its saved hash. The map's value stays where it is.
    pub fn clone_key(&self) -> Self
    where
        K: Clone,
    {
        Self {
            identity: Weak::clone(&self.identity),
            key: self.key.clone(),
            hash: self.hash,
            shard: self.shard,
        }
    }
}
impl<K> AsRef<CKey<K>> for CKey<K> {
    fn as_ref(&self) -> &CKey<K> {
        self
    }
}
impl<K: fmt::Debug> fmt::Debug for CKey<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CKey").field(&self.key).finish()
    }
}
