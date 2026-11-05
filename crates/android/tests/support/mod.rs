#![allow(dead_code)]

pub mod agent;
pub mod testbed;

use phone_use_android::adb::Adb;

pub fn serial() -> String {
    let target = std::env::var("PHONE_USE_TEST_DEVICE")
        .expect("set PHONE_USE_TEST_DEVICE, for example android:emulator-5554");
    target
        .strip_prefix("android:")
        .unwrap_or(&target)
        .to_string()
}

pub fn adb() -> Adb {
    Adb::from_env()
}
