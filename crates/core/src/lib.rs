//! Pure domain logic for phone-use: geometry, elements, refs, rendering, diff, settle, errors.

pub mod device;
pub mod diff;
pub mod element;
pub mod error;
pub mod geometry;
pub mod prune;
pub mod raw;
pub mod refs;
pub mod render;
pub mod settle;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
