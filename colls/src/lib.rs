#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../docs/examples.md")]
#![doc = include_str!("../docs/platforms.md")]

extern crate alloc;

pub mod cmap;
pub mod cshared;
mod storage;

#[doc(inline)]
pub use cmap::{CKey, CMap, CMapError};
#[doc(inline)]
pub use cshared::{CShared, CSharedGroup, CSharedGroupError};

#[doc(hidden)]
pub use cmap::input::CMapInput;
