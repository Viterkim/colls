use super::{CKey, CMap, CMapError};
use crate::storage::{Ptr, Weak};
use alloc::string::String;
use core::hash::{BuildHasher, Hash};

mod sealed {
    use super::{CKey, String};

    pub trait Sealed<K> {}
    impl<K> Sealed<K> for K {}
    impl<K> Sealed<K> for &K {}
    impl<K> Sealed<K> for &CKey<K> {}
    impl Sealed<String> for &str {}
}

#[doc(hidden)]
pub trait CMapInput<K>: sealed::Sealed<K> {
    type Prepared: AsRef<CKey<K>>;

    fn prepare<V>(self, map: &CMap<K, V>) -> Self::Prepared
    where
        K: Clone;

    fn lookup_hash<V>(&self, map: &CMap<K, V>) -> Result<u64, CMapError>;

    fn equivalent(&self, key: &K) -> bool;

    fn equivalent_input(&self, other: &Self) -> bool;
}

impl<K: Eq + Hash> CMapInput<K> for K {
    type Prepared = CKey<K>;

    fn prepare<V>(self, map: &CMap<K, V>) -> Self::Prepared {
        map.prepare_key(self)
    }

    fn lookup_hash<V>(&self, map: &CMap<K, V>) -> Result<u64, CMapError> {
        Ok(map.inner.hash_builder.hash_one(self))
    }

    fn equivalent(&self, key: &K) -> bool {
        self == key
    }

    fn equivalent_input(&self, other: &Self) -> bool {
        self == other
    }
}

impl<K: Eq + Hash> CMapInput<K> for &K {
    type Prepared = CKey<K>;

    fn prepare<V>(self, map: &CMap<K, V>) -> Self::Prepared
    where
        K: Clone,
    {
        map.prepare_key(self.clone())
    }

    fn lookup_hash<V>(&self, map: &CMap<K, V>) -> Result<u64, CMapError> {
        Ok(map.inner.hash_builder.hash_one(*self))
    }

    fn equivalent(&self, key: &K) -> bool {
        *self == key
    }

    fn equivalent_input(&self, other: &Self) -> bool {
        self == other
    }
}

impl<'a, K: Eq + Hash> CMapInput<K> for &'a CKey<K> {
    type Prepared = &'a CKey<K>;

    fn prepare<V>(self, _map: &CMap<K, V>) -> Self::Prepared {
        self
    }

    fn lookup_hash<V>(&self, map: &CMap<K, V>) -> Result<u64, CMapError> {
        if Weak::as_ptr(&self.identity) != Ptr::as_ptr(&map.inner.identity) {
            return Err(CMapError::ForeignKey);
        }

        Ok(self.hash)
    }

    fn equivalent(&self, key: &K) -> bool {
        self.key == *key
    }

    fn equivalent_input(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl CMapInput<String> for &str {
    type Prepared = CKey<String>;

    fn prepare<V>(self, map: &CMap<String, V>) -> Self::Prepared {
        map.prepare_key(String::from(self))
    }

    fn lookup_hash<V>(&self, map: &CMap<String, V>) -> Result<u64, CMapError> {
        Ok(map.inner.hash_builder.hash_one(*self))
    }

    fn equivalent(&self, key: &String) -> bool {
        key == *self
    }

    fn equivalent_input(&self, other: &Self) -> bool {
        self == other
    }
}

impl<K: Eq + Hash, V> CMap<K, V> {
    fn prepare_key(&self, key: K) -> CKey<K> {
        let hash = self.inner.hash_builder.hash_one(&key);

        CKey {
            identity: Ptr::downgrade(&self.inner.identity),
            key,
            hash,
            shard: ((hash >> 32) as usize) & (self.inner.shards.len() - 1),
        }
    }
}
