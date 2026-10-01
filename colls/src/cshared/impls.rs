use super::{CShared, CSharedGroup, CSharedGroupError, HeldLock, LockKey, SharedEntry};
use crate::storage::{Ptr, Storage, borrow};
use core::array;

impl<T> CShared<T> {
    pub fn new(v: T) -> Self {
        Self {
            inner: Ptr::new(Storage::new(v)),
        }
    }

    /// Use the state here. Do your async work before calling this.
    pub async fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut borrow(&self.inner))
    }

    /// Another handle to the same state.
    #[must_use]
    pub fn clone_ptr(&self) -> Self {
        Self {
            inner: Ptr::clone(&self.inner),
        }
    }

    /// Clone the value inside using its own `Clone` implementation.
    pub async fn clone_inner(&self) -> T
    where
        T: Clone,
    {
        self.with(|v| v.clone()).await
    }

    pub fn is_same_ptr(&self, other: &Self) -> bool {
        Ptr::ptr_eq(&self.inner, &other.inner)
    }
}
impl<T: Default> Default for CShared<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<'a, T, const N: usize> CSharedGroup<'a, T, N> {
    pub fn new(shared: [&'a CShared<T>; N]) -> Result<Self, CSharedGroupError> {
        let mut ordered = array::from_fn(|input_index| {
            let shared = shared[input_index];

            SharedEntry {
                key: LockKey::new(shared),
                input_index,
                shared,
            }
        });

        ordered.sort_unstable_by_key(|entry| entry.key);

        if ordered.windows(2).any(|pair| pair[0].key == pair[1].key) {
            Err(CSharedGroupError)
        } else {
            Ok(Self { ordered })
        }
    }

    pub async fn with<R>(&self, f: impl FnOnce([&mut T; N]) -> R) -> R {
        // Same address order everywhere, even if the caller reverses the values.
        let mut held = array::from_fn(|index| {
            let entry = &self.ordered[index];

            HeldLock {
                input_index: entry.input_index,
                guard: borrow(&entry.shared.inner),
            }
        });

        held.sort_unstable_by_key(|lock| lock.input_index);

        f(held.each_mut().map(|lock| &mut *lock.guard))
    }
}

impl LockKey {
    fn new<T>(shared: &CShared<T>) -> Self {
        Self(Ptr::as_ptr(&shared.inner).addr())
    }
}
