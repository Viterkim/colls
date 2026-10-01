#[cfg(feature = "std")]
pub use alloc::sync::{Arc as Ptr, Weak};
#[cfg(feature = "std")]
pub use parking_lot::{Mutex as Storage, MutexGuard as StorageGuard};

#[cfg(not(feature = "std"))]
pub use alloc::rc::{Rc as Ptr, Weak};
#[cfg(not(feature = "std"))]
pub use core::cell::{RefCell as Storage, RefMut as StorageGuard};

pub fn borrow<T>(storage: &Storage<T>) -> StorageGuard<'_, T> {
    #[cfg(feature = "std")]
    {
        storage.lock()
    }

    #[cfg(not(feature = "std"))]
    {
        storage.borrow_mut()
    }
}
