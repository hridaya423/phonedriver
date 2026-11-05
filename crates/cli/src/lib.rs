//! The `phone-use` command line, daemon and their wire types.

pub mod client;
pub mod commands;
pub mod daemon;
#[cfg(windows)]
mod detach;
pub mod wire;
