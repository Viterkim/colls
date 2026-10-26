use super::{
    CKey, CMap, CMapError, HashBuilder, HeldShard, MapEntry, MapInner, MapLookup, input::CMapInput,
};
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

    /// Returns your closure's result. Pass `&key` to reuse its hash.
    pub async fn with<I: CMapInput<K>, R>(
        &self,
        key: I,
        f: impl FnOnce(&mut V) -> R,
    ) -> Result<R, CMapError> {
        let hash = key.lookup_hash(self)?;
        let index = ((hash >> 32) as usize) & (self.inner.shards.len() - 1);
        let mut shard = borrow(&self.inner.shards[index]);
        let entry = shard
            .find_mut(hash, |entry| key.equivalent(&entry.key))
            .ok_or(CMapError::MissingKey)?;

        Ok(f(&mut entry.value))
    }

    /// Clone this entry's value using its own `Clone` implementation.
    pub async fn clone_inner<I: CMapInput<K>>(&self, key: I) -> Result<V, CMapError>
    where
        V: Clone,
    {
        self.with(key, |v| v.clone()).await
    }

    /// Remove an entry and get its value back.
    pub async fn remove<I: CMapInput<K>>(&self, key: I) -> Result<V, CMapError> {
        let hash = key.lookup_hash(self)?;
        let index = ((hash >> 32) as usize) & (self.inner.shards.len() - 1);

        let entry = {
            let mut shard = borrow(&self.inner.shards[index]);
            let entry = shard
                .find_entry(hash, |entry| key.equivalent(&entry.key))
                .ok()
                .ok_or(CMapError::MissingKey)?;

            entry.remove().0
        };

        Ok(entry.value)
    }

    /// Use several entries together, in the order you pass their names or keys.
    pub async fn with_many<I: CMapInput<K>, const N: usize, R>(
        &self,
        inputs: [I; N],
        f: impl FnOnce([&mut V; N]) -> R,
    ) -> Result<R, CMapError> {
        let mut hashes = [0; N];

        for (input, hash) in inputs.iter().zip(&mut hashes) {
            *hash = input.lookup_hash(self)?;
        }

        let lookups: [_; N] = array::from_fn(|index| MapLookup {
            input: &inputs[index],
            hash: hashes[index],
            shard: ((hashes[index] >> 32) as usize) & (self.inner.shards.len() - 1),
        });
        let keys = lookups.each_ref();

        if SHARD_COUNT == 1 && N > 0 {
            for (index, key) in keys.iter().enumerate() {
                if keys[index + 1..]
                    .iter()
                    .any(|other| key.hash == other.hash && key.input.equivalent_input(other.input))
                {
                    return Err(CMapError::DuplicateKey);
                }
            }

            let mut shard = borrow(&self.inner.shards[0]);
            let mut entries = shard.get_disjoint_mut(keys.map(|key| key.hash), |index, entry| {
                keys[index].input.equivalent(&entry.key)
            });

            if entries.iter().any(Option::is_none) {
                return Err(CMapError::MissingKey);
            }

            let values = entries
                .each_mut()
                .map(|entry| &mut entry.as_mut_slice()[0].value);

            return Ok(f(values));
        }

        let mut order: [_; N] = array::from_fn(|index| index);
        order.sort_unstable_by_key(|&index| (keys[index].shard, keys[index].hash));

        for (position, &index) in order.iter().enumerate() {
            let key = keys[index];

            if order[position + 1..]
                .iter()
                .map(|&other| keys[other])
                .take_while(|other| key.hash == other.hash)
                .any(|other| key.input.equivalent_input(other.input))
            {
                return Err(CMapError::DuplicateKey);
            }
        }

        // Lock each shard once, always in the same order.
        let mut held: [Option<HeldShard<'_, K, V>>; N] = array::from_fn(|index| {
            let shard = keys[order[index]].shard;

            if index > 0 && keys[order[index - 1]].shard == shard {
                return None;
            }

            let end = index + order[index..].partition_point(|&input| keys[input].shard == shard);

            Some(HeldShard {
                inputs: index..end,
                guard: borrow(&self.inner.shards[shard]),
            })
        });

        let mut values: [Option<&mut V>; N] = array::from_fn(|_| None);

        // A few entries per shard shouldn't search for every other key in the group.
        for shard in held.iter_mut().flatten() {
            let inputs = &order[shard.inputs.clone()];

            if let [index] = inputs {
                let key = keys[*index];

                values[*index] = shard
                    .guard
                    .find_mut(key.hash, |entry| key.input.equivalent(&entry.key))
                    .map(|entry| &mut entry.value);

                continue;
            }

            match *inputs {
                [a, b] => {
                    find_values(&mut shard.guard, &keys, [a, b], &mut values);
                    continue;
                }
                [a, b, c] => {
                    find_values(&mut shard.guard, &keys, [a, b, c], &mut values);
                    continue;
                }
                [a, b, c, d] => {
                    find_values(&mut shard.guard, &keys, [a, b, c, d], &mut values);
                    continue;
                }
                _ => {}
            }

            let shard_index = keys[inputs[0]].shard;
            let found = shard.guard.get_disjoint_mut(hashes, |index, entry| {
                keys[index].shard == shard_index && keys[index].input.equivalent(&entry.key)
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

fn find_values<'a, K, V, I: CMapInput<K>, const N: usize, const M: usize>(
    shard: &'a mut HashTable<MapEntry<K, V>>,
    keys: &[&MapLookup<'_, I>; N],
    inputs: [usize; M],
    values: &mut [Option<&'a mut V>; N],
) {
    let found = shard.get_disjoint_mut(inputs.map(|index| keys[index].hash), |index, entry| {
        keys[inputs[index]].input.equivalent(&entry.key)
    });

    for (index, entry) in inputs.into_iter().zip(found) {
        values[index] = entry.map(|entry| &mut entry.value);
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
