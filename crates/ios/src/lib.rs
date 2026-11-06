//! iOS devices: simulators through `simctl` and the PhoneUseRunner.

pub mod coredevice;
pub mod device;
pub mod discovery;
pub mod map;
pub mod offer;
pub mod runner;
pub mod setup;
pub mod simulator;
#[cfg(feature = "testing")]
pub mod testbed;
pub mod touch;
pub mod transport;
