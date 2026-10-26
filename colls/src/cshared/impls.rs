use super::{CShared, CSharedGroup, CSharedGroupError, HeldLock, SharedEntry};
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
        let mut ordered = array::from_fn(|input_index| SharedEntry {
            input_index,
            shared: shared[input_index],
        });

        ordered.sort_unstable_by_key(|entry| Ptr::as_ptr(&entry.shared.inner).addr());

        if ordered
            .windows(2)
            .any(|pair| pair[0].shared.is_same_ptr(pair[1].shared))
        {
            return Err(CSharedGroupError);
        }

        Ok(Self { ordered })
    }

    pub async fn with<R>(&self, f: impl FnOnce([&mut T; N]) -> R) -> R {
        // Same address order everywhere, even if the caller reverses the values.
        if N <= 2 {
            let mut held = array::from_fn(|index| {
                let entry = &self.ordered[index];

                HeldLock {
                    input_index: entry.input_index,
                    guard: borrow(&entry.shared.inner),
                }
            });

            held.sort_unstable_by_key(|lock| lock.input_index);

            return f(held.each_mut().map(|lock| &mut *lock.guard));
        }

        let mut held = self
            .ordered
            .each_ref()
            .map(|entry| borrow(&entry.shared.inner));
        let mut values: [Option<&mut T>; N] = array::from_fn(|_| None);

        for (entry, guard) in self.ordered.iter().zip(&mut held) {
            values[entry.input_index] = Some(&mut **guard);
        }

        // new() keeps exactly one entry for each input position.
        f(values.each_mut().map(|value| &mut *value.as_mut_slice()[0]))
    }
}
