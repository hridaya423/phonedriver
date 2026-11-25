//! iOS devices: simulators through `simctl` and the PhonedriverRunner.

pub mod coredevice;
pub mod ddi;
pub mod device;
pub mod discovery;
pub mod lan;
pub mod map;
pub mod offer;
pub mod remote;
pub mod runner;
pub mod setup;
mod simfiles;
pub mod simio;
pub mod simulator;
pub mod team;
#[cfg(feature = "testing")]
pub mod testbed;
pub mod touch;
pub mod transport;
