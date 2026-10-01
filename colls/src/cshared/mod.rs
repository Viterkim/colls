pub mod impls;

use crate::storage::{Ptr, Storage, StorageGuard};
use er::Er;

/// Your state, used inside `.with(|v| ...)`.
pub struct CShared<T> {
    inner: Ptr<Storage<T>>,
}

/// Edit several shared values together, in the order you pass them.
pub struct CSharedGroup<'a, T, const N: usize> {
    ordered: [SharedEntry<'a, T>; N],
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct LockKey(usize);

struct SharedEntry<'a, T> {
    key: LockKey,
    input_index: usize,
    shared: &'a CShared<T>,
}

struct HeldLock<'a, T> {
    input_index: usize,
    guard: StorageGuard<'a, T>,
}

/// Two handles in the group point to the same value.
#[derive(Er, PartialEq, Eq)]
#[er(format = "the same CShared value was passed more than once")]
pub struct CSharedGroupError;
