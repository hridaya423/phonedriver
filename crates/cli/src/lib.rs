//! The `phonedriver` command line, daemon and their wire types.

pub mod client;
pub mod commands;
pub mod compare;
pub mod daemon;
#[cfg(windows)]
mod detach;
pub mod doctor;
mod init;
mod ios_wifi;
mod lean;
pub mod mcp;
pub mod ocr;
mod private;
mod variables;
pub mod wire;
#[cfg(feature = "android")]
pub mod wireless;
