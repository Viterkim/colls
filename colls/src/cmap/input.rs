use super::{CKey, CMap};
use crate::storage::Ptr;
use alloc::string::String;
use core::hash::{BuildHasher, Hash};

mod sealed {
    use super::{CKey, String};

    pub trait Sealed<K> {}
    impl<K> Sealed<K> for K {}
    impl<K> Sealed<K> for &CKey<K> {}
    impl Sealed<String> for &str {}
}

// Keys get prepared here, storage stays inside the transactions.
#[doc(hidden)]
pub trait CMapInput<K>: sealed::Sealed<K> {
    type Prepared: AsRef<CKey<K>>;

    fn prepare<V>(self, map: &CMap<K, V>) -> Self::Prepared;
}

impl<K: Eq + Hash> CMapInput<K> for K {
    type Prepared = CKey<K>;

    fn prepare<V>(self, map: &CMap<K, V>) -> Self::Prepared {
        map.prepare_key(self)
    }
}

impl<'a, K> CMapInput<K> for &'a CKey<K> {
    type Prepared = &'a CKey<K>;

    fn prepare<V>(self, _map: &CMap<K, V>) -> Self::Prepared {
        self
    }
}

impl CMapInput<String> for &str {
    type Prepared = CKey<String>;

    fn prepare<V>(self, map: &CMap<String, V>) -> Self::Prepared {
        map.prepare_key(String::from(self))
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
