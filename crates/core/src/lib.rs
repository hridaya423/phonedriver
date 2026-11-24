//! Pure domain logic for phonedriver: geometry, elements, refs, rendering, diff, settle, errors.

pub mod alert;
pub mod audit;
pub mod device;
pub mod diff;
pub mod element;
pub mod error;
pub mod files;
pub mod geometry;
pub mod gesture;
pub mod inject;
pub mod network;
pub mod notice;
pub mod ocr;
pub mod prune;
pub mod raw;
pub mod refs;
pub mod render;
pub mod scope;
pub mod settings;
pub mod settle;
pub mod similar;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod text;
