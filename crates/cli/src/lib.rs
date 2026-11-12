//! The `phonedriver` command line, daemon and their wire types.

pub mod client;
pub mod commands;
pub mod daemon;
#[cfg(windows)]
mod detach;
pub mod doctor;
mod ios_wifi;
mod lean;
pub mod mcp;
pub mod ocr;
pub mod wire;
#[cfg(feature = "android")]
pub mod wireless;
